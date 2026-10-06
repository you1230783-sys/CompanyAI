//! 在呼叫端指定的測試分享內驗證正式 broker；不連公司服務、不讀既有使用者文件。
use company_ai::{
    projects::{files::Broker, sandbox::Worker, setup, Project, Tool},
    AppResult,
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::PathBuf, sync::atomic::AtomicBool};

fn call(broker: &mut Broker, worker: &mut Worker, id: &str, request: Value) -> AppResult<Value> {
    let tool: Tool = serde_json::from_value(request).map_err(|e| e.to_string())?;
    let result = broker.execute(id, &tool, worker, &AtomicBool::new(false))?;
    if result["ok"] != true {
        return Err(result.to_string());
    }
    Ok(result["result"].clone())
}

fn run() -> AppResult<()> {
    let exe = PathBuf::from(std::env::args().nth(1).ok_or("缺少 EXE")?);
    let parent = PathBuf::from(std::env::args().nth(2).ok_or("缺少測試根目錄")?);
    let root = setup::create_unique(&parent, "網路專案測試")?;
    let other = setup::create_unique(&parent, "網路專案測試")?;
    assert_ne!(root, other, "重名不能覆寫或共用");
    let original = "網路上的原文😀";
    std::fs::write(root.join("source.txt"), original).map_err(|e| e.to_string())?;
    let project = Project {
        id: "network".into(),
        name: "網路測試".into(),
        root: root.clone(),
        imports: BTreeMap::new(),
    };
    let mut worker = Worker::start(&exe, &AtomicBool::new(false))?;
    let mut broker = Broker::new(project.clone(), "network-run".into())?;
    broker.enable_memory("network-chat")?;
    let read = call(
        &mut broker,
        &mut worker,
        "read",
        json!({"tool":"read_file","path":"source.txt","offset":0}),
    )?;
    assert_eq!(read["text"], original);
    let copy = call(
        &mut broker,
        &mut worker,
        "copy",
        json!({"tool":"create_working_copy","source":"source.txt","name":"網路副本.txt"}),
    )?;
    let saved = call(
        &mut broker,
        &mut worker,
        "save",
        json!({"tool":"save_copy","copy_id":copy["copy_id"],"revision":copy["revision"]}),
    )?;
    assert_eq!(
        std::fs::read_to_string(root.join(saved["path"].as_str().ok_or("缺少成果")?))
            .map_err(|e| e.to_string())?,
        original
    );
    // 重新建立 broker 並讀回加密筆記；不只測單次檔案 IO。
    let note = call(
        &mut broker,
        &mut worker,
        "note",
        json!({"tool":"create_note","scope":"project","title":"網路保存","body":"加密筆記測試"}),
    )?;
    drop(broker);
    let mut restored = Broker::new(project, "network-run".into())?;
    restored.enable_memory("network-chat")?;
    let note = call(
        &mut restored,
        &mut worker,
        "note-read",
        json!({"tool":"read_note","id":note["id"]}),
    )?;
    assert_eq!(note["body"], "加密筆記測試");
    assert_eq!(
        call(
            &mut restored,
            &mut worker,
            "read",
            json!({"tool":"read_file","path":"source.txt","offset":0})
        )?["text"],
        original
    );
    for (ext, action) in [
        (
            "docx",
            json!({"kind":"word_paragraph","text":"網路週報測試"}),
        ),
        (
            "xlsx",
            json!({"kind":"excel_write","sheet":1,"cell":"A1","rows":[["項目","數值"],["測試",42]]}),
        ),
    ] {
        let copy = call(
            &mut restored,
            &mut worker,
            &format!("new-{ext}"),
            json!({"tool":"create_working_copy","source":null,"name":format!("網路成果.{ext}")}),
        )?;
        let edit = call(
            &mut restored,
            &mut worker,
            &format!("edit-{ext}"),
            json!({"tool":"office_action","copy_id":copy["copy_id"],"revision":copy["revision"],"operation":action}),
        )?;
        let output = call(
            &mut restored,
            &mut worker,
            &format!("save-{ext}"),
            json!({"tool":"save_copy","copy_id":copy["copy_id"],"revision":edit["revision"]}),
        )?;
        let read = call(
            &mut restored,
            &mut worker,
            &format!("read-{ext}"),
            json!({"tool":"read_file","path":output["path"],"offset":0}),
        )?;
        assert!(read.to_string().contains(if ext == "docx" {
            "網路週報測試"
        } else {
            "42"
        }));
    }
    let escape = restored.execute(
        "escape",
        &Tool::ReadFile {
            path: "../outside.txt".into(),
            offset: 0,
        },
        &mut worker,
        &AtomicBool::new(false),
    )?;
    assert_eq!(escape["ok"], false);
    assert_eq!(
        std::fs::read_to_string(root.join("source.txt")).map_err(|e| e.to_string())?,
        original
    );
    println!("PASS network: unique folders, TXT read/publish, DPAPI note reload, Word/Excel save/reopen, path escape rejection, original preserved: {}", root.display());
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
