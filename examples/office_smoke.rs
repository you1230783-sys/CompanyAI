//! 僅處理呼叫端指定的 Office 測試資料夾；實際經過 broker 與 AppContainer。
use company_ai::{
    projects::{files::Broker, sandbox::Worker, text, Project, Tool},
    AppResult,
};
use std::{collections::BTreeMap, path::PathBuf, sync::atomic::AtomicBool};
fn run() -> AppResult<()> {
    let args: Vec<_> = std::env::args().collect();
    let exe = PathBuf::from(args.get(1).ok_or("需要 EXE。")?);
    let root = PathBuf::from(args.get(2).ok_or("需要固定測試檔資料夾。")?);
    let cancel = AtomicBool::new(false);
    let mut worker = Worker::start(&exe, &cancel)?;
    // Word 實際輸出的繁體中文 PDF，驗證嵌入字型／ToUnicode，不只測 ASCII fixture。
    let mut pdf = Broker::new(
        Project {
            id: "office-pdf".into(),
            name: "PDF".into(),
            root: root.clone(),
            imports: BTreeMap::new(),
        },
        "pdf".into(),
    )?;
    let read = pdf.execute(
        "read",
        &Tool::ReadFile {
            path: "source.pdf".into(),
            offset: 0,
        },
        &mut worker,
        &cancel,
    )?;
    let content = read["result"]["text"]
        .as_str()
        .ok_or_else(|| read.to_string())?;
    assert!(
        content.contains("原始文字") && content.contains("第二段"),
        "{content}"
    );
    println!("PASS PDF: Chinese Word-exported PDF via AppContainer.");
    for ext in [
        "docx", "doc", "docm", "xlsx", "xls", "xlsm", "xlsb", "pptx", "ppt", "pptm",
    ] {
        let source = format!("source.{ext}");
        let before = std::fs::read(root.join(&source)).map_err(|e| e.to_string())?;
        let project = Project {
            id: "office-smoke".into(),
            name: "Office 測試".into(),
            root: root.clone(),
            imports: BTreeMap::new(),
        };
        let mut broker = Broker::new(project, "office-test".into())?;
        let mut call = |id: &str, tool: Tool| -> AppResult<serde_json::Value> {
            let response = broker.execute(id, &tool, &mut worker, &cancel)?;
            if response["ok"] != true {
                return Err(format!("{ext} {id}: {response}"));
            }
            Ok(response["result"].clone())
        };
        let copy = call(
            "copy",
            Tool::CreateWorkingCopy {
                source: Some(source.clone()),
                name: format!("修訂.{ext}"),
            },
        )?;
        let id = copy["copy_id"].as_str().ok_or("缺少 ID")?.to_owned();
        let read = call(
            "read",
            Tool::ReadFile {
                path: id.clone(),
                offset: 0,
            },
        )?;
        let content = read["text"].as_str().ok_or("缺少文字")?;
        let snapshot: serde_json::Value =
            serde_json::from_str(content).map_err(|e| e.to_string())?;
        let first = snapshot["blocks"][0]["id"]
            .as_str()
            .ok_or("缺少區塊")?
            .to_owned();
        assert_eq!(snapshot["blocks"][0]["text"], "原始文字");
        let mut edited = call(
            "edit",
            Tool::EditOffice {
                copy_id: id.clone(),
                revision: text::revision(content),
                block_id: first.clone(),
                expected: "原始文字".into(),
                replacement: "修改😀內容".into(),
            },
        )?;
        if ext.starts_with("xls") {
            assert!(call(
                "formula",
                Tool::EditOffice {
                    copy_id: id.clone(),
                    revision: edited["revision"].as_str().unwrap().into(),
                    block_id: "s1:$C$1".into(),
                    expected: "=B1*2".into(),
                    replacement: "=99".into()
                }
            )
            .is_err());
            edited = call(
                "number",
                Tool::EditOffice {
                    copy_id: id.clone(),
                    revision: edited["revision"].as_str().unwrap().into(),
                    block_id: "s1:$B$1".into(),
                    expected: "12".into(),
                    replacement: "24".into(),
                },
            )?;
            edited = call(
                "literal",
                Tool::EditOffice {
                    copy_id: id.clone(),
                    revision: edited["revision"].as_str().unwrap().into(),
                    block_id: first.clone(),
                    expected: "修改😀內容".into(),
                    replacement: "=1+1".into(),
                },
            )?;
        }
        let result = call(
            "save",
            Tool::SaveCopy {
                copy_id: id.clone(),
                revision: edited["revision"].as_str().ok_or("缺少版本")?.into(),
            },
        )?;
        let path = result["path"].as_str().ok_or("缺少成果")?;
        assert!(path.ends_with(&format!("/修訂.{ext}")), "{path}");
        let same = call(
            "save-again",
            Tool::SaveCopy {
                copy_id: id.clone(),
                revision: edited["revision"].as_str().unwrap().into(),
            },
        )?;
        assert_eq!(same["path"], path, "相同版本不得另建檔案");
        let revision = call(
            "second-edit",
            Tool::EditOffice {
                copy_id: id.clone(),
                revision: edited["revision"].as_str().unwrap().into(),
                block_id: first,
                expected: if ext.starts_with("xls") {
                    "=1+1"
                } else {
                    "修改😀內容"
                }
                .into(),
                replacement: "最終文字".into(),
            },
        )?["revision"]
            .as_str()
            .ok_or("缺少版本")?
            .to_owned();
        let second = call(
            "second-save",
            Tool::SaveCopy {
                copy_id: id.clone(),
                revision,
            },
        )?;
        let second_path = second["path"].as_str().ok_or("缺少第二版")?;
        assert!(
            second_path.ends_with(&format!("/修訂_2.{ext}")),
            "{second_path}"
        );
        assert!(root.join(path).is_file(), "舊成果必須保留");
        assert_eq!(broker.finish(&[id])?, vec![second_path]);

        assert_eq!(
            std::fs::read(root.join(&source)).map_err(|e| e.to_string())?,
            before,
            "原檔不可改變"
        );
        println!("PASS {ext}: {path}");
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
