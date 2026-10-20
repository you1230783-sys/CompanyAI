//! 只讀測試腳本建立的 MSG，確認標頭／正文與 TXT 副本，不接觸信箱。
use company_ai::{
    projects::{files::Broker, sandbox::Worker, Project, Tool},
    AppResult,
};
use std::{collections::BTreeMap, path::PathBuf, sync::atomic::AtomicBool};
fn run() -> AppResult<()> {
    let args: Vec<_> = std::env::args().collect();
    let exe = PathBuf::from(args.get(1).ok_or("需要 EXE")?);
    let root = PathBuf::from(args.get(2).ok_or("需要 MSG 測試資料夾")?);
    let before = std::fs::read(root.join("fixture.msg")).map_err(|e| e.to_string())?;
    let cancel = AtomicBool::new(false);
    let mut worker = Worker::start(&exe, &cancel)?;
    let mut broker = Broker::new(
        Project {
            id: "msg-test".into(),
            name: "MSG".into(),
            root: root.clone(),
            imports: BTreeMap::new(),
        },
        "msg-task".into(),
    )?;
    let mut call = |id: &str, tool: Tool| -> AppResult<serde_json::Value> {
        let result = broker.execute(id, &tool, &mut worker, &cancel)?;
        if result["ok"] != true {
            return Err(result.to_string());
        }
        Ok(result["result"].clone())
    };
    let read = call(
        "read",
        Tool::ReadFile {
            path: "fixture.msg".into(),
            offset: 0,
        },
    )?;
    let text = read["text"].as_str().ok_or("缺少正文")?;
    assert!(
        text.contains("CompanyAI MSG fixture") && text.contains("測試正文 W40"),
        "{text}"
    );
    let copy = call(
        "copy",
        Tool::CreateWorkingCopy {
            source: Some("fixture.msg".into()),
            name: "信件整理.txt".into(),
        },
    )?;
    let id = copy["copy_id"].as_str().unwrap().to_owned();
    call(
        "save",
        Tool::SaveCopy {
            copy_id: id.clone(),
            revision: copy["revision"].as_str().unwrap().into(),
        },
    )?;
    assert_eq!(broker.finish(&[id])?.len(), 1);
    assert_eq!(
        before,
        std::fs::read(root.join("fixture.msg")).map_err(|e| e.to_string())?
    );
    println!(
        "PASS: Classic Outlook MSG body/header read, TXT copy verified, original bytes unchanged."
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
