//! 對話內容直接排成Office操作，沒有模型重寫，也不接收HTML、遠端圖片或任意COM。
use super::*;
use crate::protocol::Message;
mod markdown;

#[derive(Clone)]
pub struct Plan {
    pub actions: Vec<office::Action>,
    pub warnings: Vec<String>,
    pub messages: usize,
}

fn paragraph(text: String, format: office::Format) -> office::Action {
    office::Action::WordParagraph {
        before: None,
        text,
        format,
    }
}
fn cells(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|s| s.trim().to_owned())
        .collect()
}

/// 支援常見區塊Markdown；無法可靠判讀的語法原字保留，不猜內容或下載外部資源。
pub fn prepare(project: &Project, messages: &[Message]) -> AppResult<Plan> {
    let mut plan = Plan {
        actions: Vec::new(),
        warnings: Vec::new(),
        messages: messages.len(),
    };
    let bytes: usize = messages.iter().map(|m| m.content.len()).sum();
    if messages.is_empty() || bytes > 100_000 {
        return Err("匯出內容需為1則以上、合計不超過100KB；請改選單則回答。".into());
    }
    for message in messages {
        let title = format!(
            "{}{}",
            if message.role == "user" {
                "使用者"
            } else {
                "AI回答"
            },
            message
                .local_time
                .as_ref()
                .map(|t| format!(" · {t}"))
                .unwrap_or_default()
        );
        plan.actions.push(paragraph(
            title,
            office::Format {
                size: Some(14.),
                bold: Some(true),
                ..Default::default()
            },
        ));
        if message.incomplete || message.project_paused {
            plan.warnings
                .push("包含未完成／暫停的回答，匯出不代表任務完成。".into());
        }
        if !message.project_charts.is_empty() || !message.project_diagrams.is_empty() {
            plan.warnings.push(
                "互動圖表／Mermaid未自動轉圖片；請先匯出PNG並在內容中引用專案相對路徑。".into(),
            );
        }
        if !message.attachments.is_empty() {
            plan.warnings
                .push("附件檔案本身未嵌入；匯出僅包含對話文字及明確引用的本機PNG。".into());
        }
        let lines: Vec<_> = message.content.lines().collect();
        let mut at = 0;
        let mut code = false;
        let mut fence: Option<(char, usize)> = None;
        while at < lines.len() {
            let line = lines[at];
            let trimmed = line.trim_start();
            if let Some((marker, count)) = fence {
                let length = trimmed.chars().take_while(|c| *c == marker).count();
                if length >= count && trimmed[length..].trim().is_empty() {
                    code = false;
                    fence = None;
                    at += 1;
                    continue;
                }
            } else if line.len() - trimmed.len() <= 3 {
                if let Some(marker) = trimmed.chars().next().filter(|c| matches!(c, '`' | '~')) {
                    let length = trimmed.chars().take_while(|c| *c == marker).count();
                    if length >= 3 {
                        code = true;
                        fence = Some((marker, length));
                        at += 1;
                        continue;
                    }
                }
            }
            if !code
                && at + 1 < lines.len()
                && line.contains('|')
                && cells(lines[at + 1])
                    .iter()
                    .all(|c| c.len() >= 3 && c.chars().all(|c| c == '-' || c == ':'))
            {
                let mut rows = vec![cells(line)];
                at += 2;
                while at < lines.len() && lines[at].contains('|') && !lines[at].trim().is_empty() {
                    rows.push(cells(lines[at]));
                    at += 1;
                }
                if rows.iter().any(|r| r.len() != rows[0].len())
                    || rows.len() > 100
                    || rows[0].len() > 10
                {
                    return Err(
                        "表格欄數不一致或超過100列／10欄；請先修正回答或改匯出較短範圍。".into(),
                    );
                }
                let formatted: Vec<Vec<Vec<office::TextRun>>> = rows
                    .iter()
                    .map(|row| row.iter().map(|cell| markdown::inline(cell)).collect())
                    .collect();
                if formatted
                    .iter()
                    .flatten()
                    .flatten()
                    .any(|run| run.bold || run.italic || run.code)
                {
                    plan.actions
                        .push(office::Action::WordRichTable { rows: formatted });
                } else {
                    let rows = formatted
                        .iter()
                        .map(|row| {
                            row.iter()
                                .map(|runs| runs.iter().map(|run| run.text.as_str()).collect())
                                .collect()
                        })
                        .collect();
                    plan.actions
                        .push(office::Action::WordTable { before: None, rows });
                }
                continue;
            }
            if !code && line.trim().starts_with("![") && line.trim().ends_with(')') {
                if let Some((_, path)) = line.trim().split_once("](") {
                    let path = &path[..path.len() - 1];
                    if path.to_ascii_lowercase().ends_with(".png") && relative(path).is_ok() {
                        let (_, _, hash) = locked_image(project, path)?;
                        let bytes = fs::read(project.root.join(relative(path)?))
                            .map_err(|e| e.to_string())?;
                        let (w, h) = super::super::charts::png::dimensions(&bytes)?;
                        let width = 360f64.min(480. * f64::from(w) / f64::from(h));
                        plan.actions.push(office::Action::InsertImage {
                            path: path.into(),
                            target: office::images::Placement::Word { before: None },
                            width,
                            sha256: Some(hash),
                        });
                    } else {
                        plan.warnings
                            .push("遠端或非PNG圖片只保留原引用文字，不連線下載。".into());
                    }
                }
            }
            let (text, format) = if code {
                (
                    line.to_owned(),
                    office::Format {
                        font: Some("Consolas".into()),
                        size: Some(9.),
                        fill: Some("#F3F4F6".into()),
                        space_after: Some(0.),
                        space_before: Some(0.),
                        ..Default::default()
                    },
                )
            } else {
                let level = line.chars().take_while(|c| *c == '#').count();
                if (1..=6).contains(&level) && line.as_bytes().get(level) == Some(&b' ') {
                    (
                        line[level + 1..].to_owned(),
                        office::Format {
                            bold: Some(true),
                            size: Some((20 - level * 2).max(12) as f64),
                            ..Default::default()
                        },
                    )
                } else {
                    (
                        line.to_owned(),
                        office::Format {
                            size: Some(11.),
                            ..Default::default()
                        },
                    )
                }
            };
            if code || !text.is_empty() {
                text::validate(&text)?;
                if code {
                    plan.actions.push(paragraph(text, format));
                } else {
                    let runs = markdown::inline(&text);
                    if runs.iter().any(|run| run.bold || run.italic || run.code) {
                        plan.actions
                            .push(office::Action::WordRichParagraph { runs, format });
                    } else {
                        plan.actions.push(paragraph(
                            runs.iter().map(|run| run.text.as_str()).collect(),
                            format,
                        ));
                    }
                }
            }
            at += 1;
        }
    }
    plan.warnings.sort();
    plan.warnings.dedup();
    if !plan.warnings.is_empty() {
        plan.actions.push(paragraph(
            format!("匯出範圍說明：{}", plan.warnings.join("；")),
            office::Format {
                size: Some(9.),
                ..Default::default()
            },
        ));
    }
    if plan.actions.len() > 200 {
        return Err("匯出超過200個內容區塊，請分次選擇回答。".into());
    }
    Ok(plan)
}

/// 全程鎖住根目錄與輸出資料夾；完成前讀回DOCX及PDF文字。失敗保留明確的部分輸出訊息。
pub fn save(
    project: &Project,
    worker_exe: &Path,
    conversation: &str,
    name: &str,
    pdf: bool,
    plan: &Plan,
    cancel: &AtomicBool,
) -> AppResult<Value> {
    if relative(name)?.components().count() != 1
        || !name
            .to_ascii_lowercase()
            .ends_with(if pdf { ".pdf" } else { ".docx" })
    {
        return Err("請指定單一檔名與正確的DOCX／PDF副檔名。".into());
    }
    let _root = pin(&project.root)?;
    let base = project.root.join("_AI_Output");
    ensure_output_folder(&project.root, "_AI_Output")?;
    let _base = pin(&base)?;
    let folder = shared_output_folder(&base, &mut None, Some(conversation))?;
    let directory = base.join(&folder);
    let _directory = pin(&directory)?;
    let stage = directory.join(format!(".export_{}", crate::jobs::new_id()?));
    fs::create_dir(&stage).map_err(|e| e.to_string())?;
    // Office儲存需要暫存目錄的寫入共用；沿用既有副本的anchor鎖，仍禁止替換目錄。
    let stage_guard = pin_stage(&stage)?;
    let docx = stage.join("content.docx");
    let staged_pdf = stage.join("content.pdf");
    let result = (|| -> AppResult<Value> {
        let blank = office::render(None, Path::new("content.docx"), None, &[], None, cancel)?;
        let copy = OfficeCopy {
            source: None,
            fingerprint: String::new(),
            original: None,
            desired: blank,
            actions: plan.actions.clone(),
        };
        let expected = render_office(project, "content.docx", &copy, Some(&docx), cancel)?;
        let actual = office::process(&docx, None, None, None, cancel)?;
        if expected != actual {
            return Err("DOCX匯出讀回內容／結構不符。".into());
        }
        let source = if pdf {
            office::export::pdf(&docx, &staged_pdf, cancel)?;
            &staged_pdf
        } else {
            &docx
        };
        let mut input = checked_file(source)?;
        let limit = if pdf { 20_000_000 } else { 50_000_000 };
        if input.metadata().map_err(|e| e.to_string())?.len() > limit {
            return Err("匯出檔案超過允許大小。".into());
        }
        let (name, mut output) = reserve_output(&directory, name)?;
        let path = format!("_AI_Output/{folder}/{name}");
        std::io::copy(&mut input, &mut output)
            .and_then(|_| output.sync_all())
            .map_err(|e| format!("可能有部分輸出{path}：{e}"))?;
        drop(output);
        let mut worker = if pdf {
            Some(Worker::start(worker_exe, cancel)?)
        } else {
            None
        };
        let verified = read_cancel(project, &path, cancel, worker.as_mut(), None)
            .map_err(|e| format!("已建立{path}但未通過讀回：{e}"))?
            .0;
        if pdf {
            let normalized = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
            let actual = normalized(&verified);
            if expected
                .blocks
                .iter()
                .filter(|b| b.kind == "text" && !b.text.trim().is_empty())
                .any(|b| !actual.contains(&normalized(&b.text)))
            {
                return Err(format!(
                    "已建立{path}，PDF文字讀回不完整，請人工檢查，尚未標示完成。"
                ));
            }
        } else if verified != expected.serialize()? {
            return Err(format!("已建立{path}，DOCX發布後讀回不同。"));
        }
        let memory = super::super::memory::Memory::open(project.clone(), conversation)?;
        // 登錄成功後才宣告交付；索引失敗仍明確保留已建立的檔案路徑。
        let entry = super::super::artifacts::entry(
            project,
            &path,
            &crate::jobs::new_id()?,
            conversation,
            BTreeMap::new(),
            false,
        )?;
        memory
            .store_artifacts(&[entry])
            .map_err(|e| format!("已建立{path}，但成果索引失敗：{e}"))?;
        Ok(
            json!({"path":path,"verified":true,"warnings":plan.warnings,"notice":"文字／結構讀回已通過；字型替換、分頁、圖片畫面仍請人工檢視。"}),
        )
    })();
    let _ = fs::remove_file(&docx);
    let _ = fs::remove_file(&staged_pdf);
    drop(stage_guard);
    let _ = fs::remove_dir(&stage);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exporter_preserves_code_urls_and_tables_without_fetching_remote_resources() {
        let project = Project {
            id: "export".into(),
            name: "test".into(),
            root: std::env::temp_dir(),
            imports: Default::default(),
        };
        let message=Message::assistant("# Title\n| A | B |\n| --- | --- |\n| x | 3 |\n```text\n# literal\n```\n![remote](https://example.com/image.png)\n[source](https://example.com/reference)".into());
        let plan = prepare(&project, &[message]).unwrap();
        assert!(plan
            .actions
            .iter()
            .any(|a| matches!(a,office::Action::WordTable{rows,..} if rows[1][1]=="3")));
        assert!(plan
            .actions
            .iter()
            .any(|a| matches!(a,office::Action::WordParagraph{text,..} if text=="# literal")));
        assert!(!plan
            .actions
            .iter()
            .any(|a| matches!(a, office::Action::InsertImage { .. })));
        assert!(plan.warnings.iter().any(|w| w.contains("遠端")));
    }
}
