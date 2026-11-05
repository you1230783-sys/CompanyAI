//! 0.9.3文件工作實測：固定譯文不代表真模型品質；所有文件只建於新的測試目錄。
use company_ai::{
    projects::{
        self,
        documents::{Command, Translated, TranslationMode},
        files::{export, Broker},
        office,
        sandbox::Worker,
        Project, Tool,
    },
    protocol::Message,
    AppResult,
};
use serde_json::{json, Value};
use std::{path::Path, sync::atomic::AtomicBool};

fn call(broker: &mut Broker, worker: &mut Worker, tool: Tool) -> AppResult<Value> {
    let result = broker.execute(
        &company_ai::jobs::new_id()?,
        &tool,
        worker,
        &AtomicBool::new(false),
    )?;
    if result["ok"] != true {
        return Err(result.to_string());
    }
    Ok(result["result"].clone())
}
fn task(broker: &mut Broker, worker: &mut Worker, command: Command) -> AppResult<Value> {
    call(broker, worker, Tool::DocumentTask { command })
}
fn run() -> AppResult<()> {
    let args: Vec<_> = std::env::args().collect();
    let exe = Path::new(args.get(1).ok_or("需要EXE路徑。")?);
    let root = Path::new(args.get(2).ok_or("需要測試目錄。")?).join(company_ai::jobs::new_id()?);
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let cancel = AtomicBool::new(false);
    let project = Project {
        id: "documents".into(),
        name: "文件測試".into(),
        root: root.clone(),
        imports: Default::default(),
    };
    let mut worker = Worker::start(exe, &cancel)?;
    let mut checks = Vec::new();
    // Markdown建立使用正式固定工具與AppContainer編輯，接觸受保護文字後仍不得轉存。
    let mut broker = Broker::new(project.clone(), "markdown".into())?;
    let created = call(
        &mut broker,
        &mut worker,
        Tool::CreateWorkingCopy {
            source: None,
            name: "new.md".into(),
        },
    )?;
    let id = created["copy_id"].as_str().ok_or("缺少copy_id")?.to_owned();
    let content = "# 測試\n\n保留 😀 與表格。";
    let edit = call(
        &mut broker,
        &mut worker,
        Tool::EditText {
            copy_id: id.clone(),
            revision: created["revision"].as_str().unwrap().into(),
            start: 0,
            expected: String::new(),
            replacement: content.into(),
        },
    )?;
    let saved = call(
        &mut broker,
        &mut worker,
        Tool::SaveCopy {
            copy_id: id.clone(),
            revision: edit["revision"].as_str().unwrap().into(),
        },
    )?;
    assert_eq!(
        projects::files::read(&project, saved["path"].as_str().unwrap())?.0,
        content
    );
    std::fs::write(root.join("protected.txt"), "sensitive").map_err(|e| e.to_string())?;
    call(
        &mut broker,
        &mut worker,
        Tool::ReadFile {
            path: "protected.txt".into(),
            offset: 0,
        },
    )?;
    let next = call(
        &mut broker,
        &mut worker,
        Tool::CreateWorkingCopy {
            source: None,
            name: "blocked.md".into(),
        },
    )?;
    assert!(call(
        &mut broker,
        &mut worker,
        Tool::SaveCopy {
            copy_id: next["copy_id"].as_str().unwrap().into(),
            revision: next["revision"].as_str().unwrap().into()
        }
    )
    .is_err());
    checks.push("new Markdown edit/save/readback and protected text export refusal");
    office::create_translation_fixture(&root)?;
    let source = std::fs::read(root.join("source.docx")).map_err(|e| e.to_string())?;
    for mode in [
        TranslationMode::NewDocument,
        TranslationMode::PreserveStyles,
    ] {
        let mut broker = Broker::new(project.clone(), company_ai::jobs::new_id()?)?;
        let mut page = task(
            &mut broker,
            &mut worker,
            Command::PrepareTranslation {
                path: "source.docx".into(),
                language: "English test".into(),
                mode,
                terms: vec![],
            },
        )?;
        let id = page["id"].as_str().unwrap().to_owned();
        let name = if mode == TranslationMode::NewDocument {
            "translated-new.docx"
        } else {
            "translated-styled.docx"
        };
        assert!(task(
            &mut broker,
            &mut worker,
            Command::FinishTranslation {
                id: id.clone(),
                name: name.into()
            }
        )
        .is_err());
        let total = page["total"].as_u64().unwrap() as usize;
        let mut offset = 0;
        while offset < total {
            if offset > 0 {
                page = task(
                    &mut broker,
                    &mut worker,
                    Command::TranslationPage {
                        id: id.clone(),
                        offset,
                    },
                )?;
            }
            let items: Vec<_> = page["segments"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| Translated {
                    id: s["id"].as_str().unwrap().into(),
                    source: s["text"].as_str().unwrap().into(),
                    translation: format!("譯{}", s["text"].as_str().unwrap()),
                })
                .collect();
            offset += items.len();
            let revision = page["revision"].as_str().unwrap().to_owned();
            page = task(
                &mut broker,
                &mut worker,
                Command::ApplyTranslation {
                    id: id.clone(),
                    revision: revision.clone(),
                    items: items.clone(),
                },
            )?;
            assert!(task(
                &mut broker,
                &mut worker,
                Command::ApplyTranslation {
                    id: id.clone(),
                    revision,
                    items
                }
            )
            .is_err());
        }
        let saved = task(
            &mut broker,
            &mut worker,
            Command::FinishTranslation {
                id: id.clone(),
                name: name.into(),
            },
        )?;
        let replay = task(
            &mut broker,
            &mut worker,
            Command::FinishTranslation {
                id,
                name: name.into(),
            },
        )?;
        assert_eq!(saved["path"], replay["path"]);
        let output = root.join(saved["path"].as_str().unwrap());
        let read = office::process(&output, None, None, None, &cancel)?;
        assert!(read.blocks.iter().any(|b| b.text.contains("譯")));
        if mode == TranslationMode::PreserveStyles {
            let original = office::process(&root.join("source.docx"), None, None, None, &cancel)?;
            let stable_structure = |snapshot: &office::Snapshot| {
                let mut value = snapshot.structure.clone();
                for image in value["images"].as_array_mut().into_iter().flatten() {
                    // 文字變長使Word字元座標及分頁移動；核對圖片尺寸、順序、雜湊及表格。
                    image.as_object_mut().unwrap().remove("start");
                    image.as_object_mut().unwrap().remove("page");
                }
                value
            };
            assert_eq!(stable_structure(&read), stable_structure(&original));
            assert_eq!(read.blocks.len(), original.blocks.len());
            let styled = office::translation::inspect(&output, &cancel)?;
            let input = office::translation::inspect(&root.join("source.docx"), &cancel)?;
            let formats = |s: &office::Snapshot| {
                s.blocks
                    .iter()
                    .filter(|b| b.kind == "text" && !b.text.trim().is_empty())
                    .map(|b| s.formats[b.format.as_u64().unwrap() as usize].clone())
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                formats(&styled),
                formats(&input),
                "mixed fonts and paragraph formats survive final reopen"
            );
        }
        assert_eq!(std::fs::read(root.join("source.docx")).unwrap(), source);
        checks.push(if mode==TranslationMode::NewDocument{"new Word translation, incomplete/revision gates, idempotent publish, source unchanged"}else{"styled Word translation, mixed font/table/image structure after reopen, source unchanged"});
        let difference = task(
            &mut broker,
            &mut worker,
            Command::Compare {
                left: "source.docx".into(),
                right: saved["path"].as_str().unwrap().into(),
                key_column: None,
            },
        )?;
        assert!(difference["total_changes"].as_u64().unwrap() > 0);
        checks.push("Word native comparison reports changed paragraphs");
    }
    let mut message=Message::assistant("# 匯出標題\n中文 **粗體😀**、*斜體*、`inline_code` 後接一般文字。\n\n| 項目 | 數值 |\n| --- | --- |\n| **測試** | *42* |\n\n```python\n    print(42)\n\n    # 保留縮排與空行\n```\n程式碼後的一般文字。\n\n來源：[文件](https://example.com/reference)\n\n![圖表](chart.png)".into());
    // 真Excel公式的計算值與公式文字分開比較，不把純數字變更誤稱為公式變更。
    for (name, number, formula) in [("before.xlsx", 10, "=B2*2"), ("after.xlsx", 11, "=B2*3")] {
        office::render(
            None,
            Path::new(name),
            None,
            &[
                office::Action::ExcelWrite {
                    sheet: 1,
                    cell: "A1".into(),
                    rows: vec![
                        vec![json!("ID"), json!("Value"), json!("Calculation")],
                        vec![json!("alpha"), json!(number), Value::Null],
                    ],
                },
                office::Action::ExcelFormulas {
                    sheet: 1,
                    cell: "C2".into(),
                    rows: vec![vec![formula.into()]],
                    dynamic: false,
                },
            ],
            Some(&root.join(name)),
            &cancel,
        )?;
    }
    let mut comparison = Broker::new(project.clone(), "comparison".into())?;
    let diff = task(
        &mut comparison,
        &mut worker,
        Command::Compare {
            left: "before.xlsx".into(),
            right: "after.xlsx".into(),
            key_column: Some("A".into()),
        },
    )?;
    assert_eq!(diff["total_changes"], 2);
    assert!(diff["changes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["before"]["text"] == "=B2*2"
            && c["after"]["text"] == "=B2*3"
            // Office快照以字串保存Value2，避免JSON數字轉換改變原始精度。
            && c["before"]["calculation"]["value"] == "20"
            && c["after"]["calculation"]["value"] == "33"));
    checks.push("real Excel comparison separates unique key, value, formula and calculated result");
    std::fs::copy(root.join("source.docx"), root.join("changed.docx"))
        .map_err(|e| e.to_string())?;
    let changed = task(
        &mut comparison,
        &mut worker,
        Command::PrepareTranslation {
            path: "changed.docx".into(),
            language: "English".into(),
            mode: TranslationMode::NewDocument,
            terms: vec![],
        },
    )?;
    std::fs::write(root.join("changed.docx"), b"changed after read").map_err(|e| e.to_string())?;
    assert!(task(
        &mut comparison,
        &mut worker,
        Command::ApplyTranslation {
            id: changed["id"].as_str().unwrap().into(),
            revision: changed["revision"].as_str().unwrap().into(),
            items: vec![]
        }
    )
    .unwrap_err()
    .contains("來源已修改"));
    checks.push("translation refuses source changed after prepare");
    message.incomplete = true;
    for pdf in [false, true] {
        let mut export_message = message.clone();
        if pdf {
            // 本機Word會將彩色emoji轉成圖像，PDF文字層無法完整讀回。
            // 保留產品的完整性拒絕；DOCX仍測補充平面UTF-16座標，PDF則
            // 分別驗證可抽取的中文與Markdown格式，不把圖像化符號當成通過。
            export_message.content = export_message.content.replace('😀', "");
        }
        let plan = export::prepare(&project, &[export_message])?;
        assert!(!plan.warnings.is_empty());
        let name = if pdf { "export.pdf" } else { "export.docx" };
        let value = export::save(
            &project,
            exe,
            "export-conversation",
            name,
            pdf,
            &plan,
            &cancel,
        )?;
        assert_eq!(value["verified"], true);
        println!(
            "EXPORT {}",
            root.join(value["path"].as_str().unwrap()).display()
        );
    }
    checks.push("native answer export DOCX/PDF with Chinese text, table, code, URL, embedded PNG and readback");
    checks.push("DOCX supplementary Unicode formatting; PDF uses selectable-text fixture, color-emoji extraction limitation remains explicit");
    let report = json!({"version":env!("CARGO_PKG_VERSION"),"result":"PASS","checks":checks,"fixture":root,"real_model_tested":false,"company_encryption_tested":false});
    std::fs::write(
        Path::new(args.get(2).unwrap()).join("document-work-verification.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    println!("{report}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("FAIL: {error}");
        std::process::exit(1);
    }
}
