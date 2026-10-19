//! Mermaid 的來源、歷史版本與輸出；模型不能指定 SVG、任意副檔名或覆寫舊檔。
use super::*;
use crate::projects::diagrams::{self, Diagram, Format};

const MAX_SOURCE_SLICES: usize = 6;
const MAX_SOURCE_BYTES: usize = 16_000;
const MAX_DIAGRAM_CONTEXT_BYTES: usize = 32_000;

/// 已讀原文的有界快照，只作下一輪畫圖依據，不恢復任何檔案寫入授權。
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceSlice {
    path: String,
    revision: String,
    first_line: usize,
    last_line: usize,
    total_lines: usize,
    text: String,
}

pub(super) fn validate_sources(sources: &[SourceSlice]) -> AppResult<()> {
    if sources.len() > MAX_SOURCE_SLICES
        || sources.iter().map(|s| s.text.len()).sum::<usize>() > MAX_SOURCE_BYTES
    {
        return Err("Mermaid 已讀來源快照超過上限。".into());
    }
    for source in sources {
        relative(&source.path)?;
        if source.revision.len() != 64
            || !source.revision.bytes().all(|b| b.is_ascii_hexdigit())
            || source.first_line == 0
            || source.last_line < source.first_line
            || source.last_line - source.first_line >= 200
            || source.last_line > source.total_lines
            || source.text.is_empty()
            || source.text.split_inclusive('\n').count() != source.last_line - source.first_line + 1
        {
            return Err("Mermaid 已讀來源快照不合法。".into());
        }
    }
    Ok(())
}

/// 與圖表共用目錄鎖定及 create_new 規則；讀回逐位元核對後才回報路徑。
pub fn save(project: &Project, name: &str, bytes: &[u8]) -> AppResult<String> {
    let relative_name = relative(name)?;
    if relative_name.components().count() != 1 || bytes.len() > 16 * 1024 * 1024 {
        return Err("圖形成果名稱或大小不合法。".into());
    }
    validate_root(&project.root)?;
    let _root = pin(&project.root)?;
    let base = project.root.join("_AI_Output");
    match fs::create_dir(&base) {
        Ok(()) => (),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(error) => return Err(error.to_string()),
    }
    let _base = pin(&base)?;
    let folder_name = create_output_folder(&base)?;
    let folder = base.join(&folder_name);
    let _folder = pin(&folder)?;
    let (name, mut file) = reserve_output(&folder, name)?;
    let path = format!("_AI_Output/{folder_name}/{name}");
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("可能部分寫入 {path}，尚未完成：{error}"))?;
    file.rewind().map_err(|e| e.to_string())?;
    let mut actual = Vec::new();
    Read::by_ref(&mut file)
        .take((bytes.len() + 1) as u64)
        .read_to_end(&mut actual)
        .map_err(|e| e.to_string())?;
    if actual != bytes {
        return Err(format!("{path} 讀回不一致。"));
    }
    Ok(path)
}

impl Broker {
    /// 直接讀取與run_batch主程序提交都走相同登記；只接受真正成功的唯讀結果。
    pub(super) fn retain_diagram_read(&mut self, tool: &Tool, result: &Value) {
        if matches!(tool, Tool::ReadCodeSection { .. }) {
            self.retain_diagram_source(result);
        } else if matches!(tool, Tool::ReadFile { .. })
            && result["offset"] == 0
            && result["truncated"] == false
            && result["next_offset"] == result["total"]
        {
            let Some(path) = result["path"].as_str() else {
                return;
            };
            let name = self
                .copies
                .get(path)
                .map(|c| c.name.as_str())
                .unwrap_or(path);
            if !source_code::supported(Path::new(name))
                && !name.to_ascii_lowercase().ends_with(".md")
            {
                return;
            }
            let Some(text) = result["text"].as_str() else {
                return;
            };
            let lines = text.split_inclusive('\n').count();
            self.retain_diagram_source(&json!({"path":path,"revision":result["revision"],
                "first_line":1,"last_line":lines,"total_lines":lines,"text":text}));
        }
    }

    /// 來源先讀、技能後載入也保留；相同範圍去重，新revision淘汰同檔舊片段。
    pub(super) fn retain_diagram_source(&mut self, result: &Value) {
        let value = json!({"path":result["path"],"revision":result["revision"],
            "first_line":result["first_line"],"last_line":result["last_line"],
            "total_lines":result["total_lines"],"text":result["text"]});
        let Ok(source) = serde_json::from_value::<SourceSlice>(value) else {
            return;
        };
        if validate_sources(std::slice::from_ref(&source)).is_err() {
            return;
        }
        self.diagram_sources.retain(|old| {
            !old.path.eq_ignore_ascii_case(&source.path)
                || old.revision == source.revision
                    && (old.first_line != source.first_line || old.last_line != source.last_line)
        });
        self.diagram_sources.push(source);
        while self.diagram_sources.len() > MAX_SOURCE_SLICES
            || self
                .diagram_sources
                .iter()
                .map(|s| s.text.len())
                .sum::<usize>()
                > MAX_SOURCE_BYTES
        {
            self.diagram_sources.remove(0);
        }
    }

    /// 與已壓縮的工具歷史分開固定帶入：不從模型摘要重建剛畫好的圖或程式。
    /// 大圖保留metadata並明示按需inspect，不截斷Mermaid語法冒充完整來源。
    pub(crate) fn diagram_work_context(&self) -> Option<Value> {
        if !self.loaded_skills.iter().any(|id| id == "mermaid") {
            return None;
        }
        let describe = |message: Option<usize>, index: usize, diagram: &Diagram| {
            json!({"message_index":message,"diagram_index":index,"name":diagram.name,
                "path":diagram.path,"revision":diagram.revision,"based_on":diagram.based_on})
        };
        let mut recent: Vec<_> = self
            .diagrams
            .iter()
            .enumerate()
            .rev()
            .take(2)
            .map(|(i, d)| (None, i, d))
            .collect();
        if recent.is_empty() {
            recent = self
                .previous_diagrams
                .iter()
                .rev()
                .flat_map(|(m, ds)| {
                    ds.iter()
                        .enumerate()
                        .rev()
                        .map(move |(i, d)| (Some(*m), i, d))
                })
                .take(2)
                .collect();
        }
        let mut remaining = MAX_DIAGRAM_CONTEXT_BYTES;
        let latest: Vec<_> = recent
            .into_iter()
            .map(|(message, index, diagram)| {
                let mut row = describe(message, index, diagram);
                if diagram.source.len() <= remaining {
                    remaining -= diagram.source.len();
                    row["source"] = json!(diagram.source);
                } else {
                    row["source_omitted"] =
                        json!("超過常駐預算；需要修改時依索引inspect_mermaid讀完整原碼");
                }
                row
            })
            .collect();
        Some(
            json!({"current":self.diagrams.iter().enumerate().map(|(i,d)|describe(None,i,d)).collect::<Vec<_>>(),
            "latest":latest,"source_snapshots":self.diagram_sources,
            "verified_exports":self.diagram_exports.iter().rev().take(12).collect::<Vec<_>>(),
            "notice":"以上是本任務實際讀過的程式版本與已保存圖源，不是即時檔案。相同revision沿用行號與原文；修改圖從source局部調整，不另猜程式。來源真的變更或矛盾才補讀。已完成所要求匯出就finish，不再進入重讀核對循環。"}),
        )
    }

    pub fn set_diagram_renderer(&mut self, renderer: diagrams::Renderer) {
        self.diagram_renderer = Some(renderer);
    }
    pub fn diagrams(&self) -> &[Diagram] {
        &self.diagrams
    }

    pub(in crate::projects) fn set_diagram_history(
        &mut self,
        messages: &[crate::protocol::Message],
    ) {
        self.previous_diagrams = messages
            .iter()
            .enumerate()
            .filter(|(_, m)| !m.project_diagrams.is_empty())
            .map(|(i, m)| (i, m.project_diagrams.clone()))
            .collect();
    }

    pub(super) fn inspect_mermaid(
        &self,
        message: Option<usize>,
        index: Option<usize>,
    ) -> AppResult<Value> {
        if let Some(index) = index {
            let list = match message {
                Some(i) => self
                    .previous_diagrams
                    .get(&i)
                    .ok_or("找不到此對話的 Mermaid 訊息。")?,
                None => &self.diagrams,
            };
            let diagram = list.get(index).ok_or("找不到 Mermaid 圖形編號。")?;
            diagram.validate()?;
            return Ok(json!({"message_index":message,"diagram_index":index,"diagram":diagram}));
        }
        let describe = |message: Option<usize>, index: usize, diagram: &Diagram| json!({"message_index":message,"diagram_index":index,"name":diagram.name,"revision":diagram.revision,"path":diagram.path});
        Ok(
            json!({"current":self.diagrams.iter().enumerate().map(|(i,d)|describe(None,i,d)).collect::<Vec<_>>(),
            "previous":self.previous_diagrams.iter().filter(|(m,_)| message.is_none_or(|i|i==**m)).flat_map(|(m,ds)|ds.iter().enumerate().map(move |(i,d)|describe(Some(*m),i,d))).collect::<Vec<_>>(),
            "notice":"指定 diagram_index 讀取完整原始碼；修改須帶 based_on 版本，另建新圖，舊圖保留。"}),
        )
    }

    /// 模型與使用者使用同一呈現器；原始碼取自本機版本，不接收模型提供的 SVG／PNG。
    pub(super) fn export_mermaid(
        &mut self,
        message: Option<usize>,
        index: usize,
        format: Format,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        let value = self.inspect_mermaid(message, Some(index))?;
        let diagram: Diagram =
            serde_json::from_value(value["diagram"].clone()).map_err(|e| e.to_string())?;
        let bytes = if matches!(format, Format::Mmd) {
            diagram.source.as_bytes().to_vec()
        } else {
            self.diagram_renderer
                .as_mut()
                .ok_or("桌面 Mermaid 呈現器尚未可用。")?(
                &diagram.source, format, cancel
            )?
        };
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("匯出已取消，未儲存。".into());
        }
        let stem = Path::new(&diagram.name)
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or("圖形檔名無法辨識。")?;
        let path = save(
            &self.project,
            &format!("{stem}.{}", format.extension()),
            &bytes,
        )?;
        self.published.push(path.clone());
        use sha2::{Digest, Sha256};
        self.diagram_exports.push(diagrams::Export {
            path: path.clone(),
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            format,
        });
        Ok(json!({"path":path,"revision":diagram.revision,"saved":true,"verified":true}))
    }

    pub(super) fn verify_diagram_export(&self, export: &diagrams::Export) -> AppResult<()> {
        use sha2::{Digest, Sha256};
        let path = relative(&export.path)?;
        if path.components().count() != 3
            || !path.starts_with("_AI_Output")
            || path.extension().and_then(|s| s.to_str()) != Some(export.format.extension())
        {
            return Err("Mermaid 匯出路徑不合法。".into());
        }
        let target = self.project.root.join(path);
        let _guards = pin(target.parent().ok_or("缺少圖形目錄。")?)?;
        let mut file = checked_file(&target)?;
        if file.metadata().map_err(|e| e.to_string())?.len() > 16 * 1024 * 1024 {
            return Err("Mermaid 匯出檔過大。".into());
        }
        let mut hash = Sha256::new();
        std::io::copy(&mut file, &mut hash).map_err(|e| e.to_string())?;
        if format!("{:x}", hash.finalize()) != export.sha256 {
            return Err("Mermaid 匯出檔已變更，不能交付。".into());
        }
        Ok(())
    }

    pub(super) fn create_mermaid(
        &mut self,
        name: &str,
        source: &str,
        based_on: Option<&str>,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        diagrams::validate_name(name)?;
        diagrams::validate_source(source)?;
        if let Some(diagram) = self
            .diagrams
            .iter()
            .find(|d| d.name == name && d.source == source && d.based_on.as_deref() == based_on)
        {
            if read_cancel(&self.project, &diagram.path, cancel, None, None)?.0 != source {
                return Err("已保存的 Mermaid 成果遭外部變更，未重用。".into());
            }
            return Ok(
                json!({"path":diagram.path,"revision":diagram.revision,"source_saved":true,"render_verified":true,"reused":true}),
            );
        }
        if self.diagrams.len() >= diagrams::MAX_DIAGRAMS {
            return Err("每個任務最多 12 張 Mermaid 圖形。".into());
        }
        if let Some(revision) = based_on {
            if !self
                .diagrams
                .iter()
                .chain(self.previous_diagrams.values().flatten())
                .any(|d| d.revision == revision)
            {
                return Err("找不到來源版本；請先 inspect_mermaid 讀取原始碼與 revision。".into());
            }
        }
        // 真正呼叫同一離線引擎，語法錯誤會回到模型，不把儲存文字當作畫圖成功。
        let renderer = self
            .diagram_renderer
            .as_mut()
            .ok_or("桌面 Mermaid 呈現器尚未可用。")?;
        let _svg = renderer(source, Format::Svg, cancel)?;
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("Mermaid 已取消，未儲存。".into());
        }
        let path = save(&self.project, name, source.as_bytes())?;
        self.published.push(path.clone());
        let diagram = Diagram {
            name: name.into(),
            source: source.into(),
            revision: text::revision(source),
            path,
            based_on: based_on.map(str::to_owned),
        };
        diagram.validate()?;
        self.diagrams.push(diagram.clone());
        Ok(
            json!({"diagram_index":self.diagrams.len()-1,"revision":diagram.revision,"path":diagram.path,"render_verified":true,"source_saved":true,"notice":"完成訊息會顯示圖形與原始碼／PNG／SVG／MMD按鈕；不必再貼同一份程式碼。"}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_versions_exports_and_checkpoint_preserve_originals() {
        let root = std::env::temp_dir().join(format!("mermaid-{}", crate::jobs::new_id().unwrap()));
        fs::create_dir(&root).unwrap();
        let project = Project {
            id: "mermaid".into(),
            name: "Mermaid測試".into(),
            root: root.clone(),
            imports: Default::default(),
        };
        let cancel = AtomicBool::new(false);
        let mut broker = Broker::new(project.clone(), "diagram-run".into()).unwrap();
        broker.set_diagram_renderer(Box::new(|_, format, _| {
            Ok(if matches!(format, Format::Png) {
                crate::projects::charts::png::fixture()
            } else {
                b"<svg></svg>".to_vec()
            })
        }));
        let source = "flowchart TD\n A[開始] --> B[完成]";
        let first = broker
            .create_mermaid("流程.mmd", source, None, &cancel)
            .unwrap();
        assert_eq!(
            fs::read_to_string(root.join(first["path"].as_str().unwrap())).unwrap(),
            source
        );
        let repeated = broker
            .create_mermaid("流程.mmd", source, None, &cancel)
            .unwrap();
        assert_eq!(repeated["reused"], true);
        assert_eq!(broker.diagrams.len(), 1);
        assert!(broker.diagram_work_context().is_none());
        broker.loaded_skills.push("mermaid".into());
        let code = "def run_job():\n    return 'done'\n";
        let slice = json!({"path":"main.py","revision":text::revision(code),
            "first_line":1,"last_line":2,"total_lines":2,"text":code});
        broker.retain_diagram_source(&slice);
        broker.retain_diagram_source(&slice);
        let context = broker.diagram_work_context().unwrap();
        assert_eq!(context["latest"][0]["source"], source);
        assert_eq!(context["source_snapshots"][0]["text"], code);
        assert_eq!(broker.diagram_sources.len(), 1);
        // 工具歷史整理與checkpoint還原，不移除精確的程式／圖形工作資料。
        broker.archive_results().unwrap();
        assert_eq!(broker.diagram_work_context().unwrap(), context);
        let saved = broker.saved().unwrap();
        let mut restored = Broker::new(project.clone(), "resume".into()).unwrap();
        restored.restore(saved, &cancel).unwrap();
        assert_eq!(restored.diagrams[0].source, source);
        assert_eq!(restored.diagram_work_context().unwrap(), context);
        let mut message = crate::protocol::Message::assistant("完成".into());
        message.project_diagrams = broker.diagrams.clone();
        let mut next = Broker::new(project, "next".into()).unwrap();
        next.set_diagram_history(&[message]);
        next.set_diagram_renderer(Box::new(|_, _, _| Ok(b"<svg></svg>".to_vec())));
        let inspected = next.inspect_mermaid(Some(0), Some(0)).unwrap();
        assert_eq!(inspected["diagram"]["source"], source);
        let revision = inspected["diagram"]["revision"].as_str().unwrap();
        let second = next
            .create_mermaid(
                "流程.mmd",
                "flowchart TD\n A[開始] --> B[核對] --> C[完成]",
                Some(revision),
                &cancel,
            )
            .unwrap();
        assert_ne!(first["path"], second["path"]);
        assert_ne!(first["revision"], second["revision"]);
        assert!(next
            .create_mermaid("錯誤.mmd", source, Some("unknown"), &cancel)
            .is_err());
        let exported = next
            .export_mermaid(Some(0), 0, Format::Mmd, &cancel)
            .unwrap();
        assert_eq!(
            fs::read_to_string(root.join(exported["path"].as_str().unwrap())).unwrap(),
            source
        );
        next.set_diagram_renderer(Box::new(|_, _, _| Err("syntax line 2".into())));
        assert!(next
            .create_mermaid("壞圖.mmd", "bad syntax", None, &cancel)
            .unwrap_err()
            .contains("line 2"));
        assert_eq!(next.diagrams.len(), 1);
        fs::write(
            root.join(first["path"].as_str().unwrap()),
            "external change",
        )
        .unwrap();
        assert!(restored.restore(broker.saved().unwrap(), &cancel).is_err());
        drop(next);
        drop(restored);
        drop(broker);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn source_snapshots_are_bounded_and_new_revisions_remove_stale_ranges() {
        let root = std::env::temp_dir().join(format!(
            "lmai-mermaid-context-{}",
            crate::jobs::new_id().unwrap()
        ));
        fs::create_dir(&root).unwrap();
        let project = Project {
            id: "context".into(),
            name: "來源快照".into(),
            root: root.clone(),
            imports: Default::default(),
        };
        let mut broker = Broker::new(project, "context".into()).unwrap();
        let markdown = "# 範例\n這是來源說明。\n";
        let read = json!({"path":"README.md","revision":text::revision(markdown),
            "offset":0,"next_offset":markdown.chars().count(),"total":markdown.chars().count(),
            "truncated":false,"text":markdown});
        broker.retain_diagram_read(
            &Tool::ReadFile {
                path: "README.md".into(),
                offset: 0,
            },
            &read,
        );
        assert_eq!(broker.diagram_sources[0].last_line, 2);
        let mut partial = read.clone();
        partial["truncated"] = json!(true);
        partial["path"] = json!("partial.md");
        broker.retain_diagram_read(
            &Tool::ReadFile {
                path: "partial.md".into(),
                offset: 0,
            },
            &partial,
        );
        assert_eq!(broker.diagram_sources.len(), 1);
        for i in 0..9 {
            broker.retain_diagram_source(
                &json!({"path":format!("file{i}.py"),"revision":text::revision("pass\n"),
                "first_line":1,"last_line":1,"total_lines":2,"text":"pass\n"}),
            );
        }
        assert_eq!(broker.diagram_sources.len(), MAX_SOURCE_SLICES);
        broker.retain_diagram_source(
            &json!({"path":"file8.py","revision":text::revision("new\n"),
            "first_line":2,"last_line":2,"total_lines":2,"text":"new\n"}),
        );
        assert_eq!(
            broker
                .diagram_sources
                .iter()
                .filter(|s| s.path == "file8.py")
                .count(),
            1
        );
        let previous = broker.diagram_sources.len();
        broker.retain_diagram_source(&json!({"path":"large.py","revision":text::revision("x"),
            "first_line":1,"last_line":1,"total_lines":1,"text":"x".repeat(MAX_SOURCE_BYTES+1)}));
        assert_eq!(broker.diagram_sources.len(), previous);
        validate_sources(&broker.diagram_sources).unwrap();
        let mut invalid = broker.diagram_sources.clone();
        invalid[0].first_line = 0;
        assert!(validate_sources(&invalid).is_err());
        drop(broker);
        fs::remove_dir_all(root).unwrap();
    }
}
