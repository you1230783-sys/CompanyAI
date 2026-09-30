//! 固定工具真的經 broker 往返，包含私有資料排除、筆記與重啟後的文件摘要。
use company_ai::{
    projects::{files::Broker, sandbox::Worker, Project, Tool},
    protocol::Message,
    AppResult,
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path, sync::atomic::AtomicBool};
pub fn verify(exe: &Path, parent: &Path) -> AppResult<()> {
    let root = parent.join("memory-tools");
    std::fs::create_dir(&root).map_err(|e| e.to_string())?;
    let source = "甲😀".repeat(4001);
    std::fs::write(root.join("source.txt"), &source).map_err(|e| e.to_string())?;
    let project = Project {
        id: "memory".into(),
        name: "memory".into(),
        root: root.clone(),
        imports: BTreeMap::new(),
    };
    let mut broker = Broker::new(project.clone(), "first".into())?;
    broker.enable_memory("chat")?;
    let cancel = AtomicBool::new(false);
    let mut worker = Worker::start(exe, &cancel)?;
    let mut call = |id: &str, value: Value| -> AppResult<Value> {
        let request: Tool = serde_json::from_value(value).map_err(|e| e.to_string())?;
        let result = broker.execute(id, &request, &mut worker, &cancel)?;
        Ok(result)
    };
    let listed = call("list", json!({"tool":"list_files","path":""}))?;
    assert_eq!(listed["result"]["entries"].as_array().unwrap().len(), 1);
    assert!(!listed.to_string().contains(".lmai"));
    for (i, path) in [".lmai", ".LMAI/documents", "sub/.lmai"].iter().enumerate() {
        assert_eq!(
            call(
                &format!("deny{i}"),
                json!({"tool":"list_files","path":path})
            )?["ok"],
            false
        );
    }
    assert_eq!(
        call(
            "deny_read",
            json!({"tool":"read_file","path":".lmai/project/index.dpapi","offset":0})
        )?["ok"],
        false
    );
    let first = call(
        "read",
        json!({"tool":"read_file","path":"source.txt","offset":0}),
    )?;
    assert_eq!(first["ok"], true);
    let revision = first["result"]["revision"].clone();
    let second = call(
        "rest",
        json!({"tool":"read_file","path":"source.txt","offset":6000}),
    )?;
    assert_eq!(second["result"]["document"]["summary_needed"], true);
    let updated = call(
        "summary",
        json!({"tool":"update_document_note","path":"source.txt","revision":revision,"note_revision":"1","section_id":null,"summary":"這份文件是繁體字與 emoji 測試資料。"}),
    )?;
    assert_eq!(updated["ok"], true);
    let created = call(
        "note",
        json!({"tool":"create_note","scope":"conversation","title":"輸出規則","body":"使用者要求保留 emoji。"}),
    )?;
    assert_eq!(created["ok"], true);
    let id = created["result"]["id"].clone();
    let duplicate = call(
        "note",
        json!({"tool":"create_note","scope":"conversation","title":"輸出規則","body":"使用者要求保留 emoji。"}),
    )?;
    assert_eq!(created, duplicate);
    assert_eq!(
        call(
            "edit_note",
            json!({"tool":"update_note","id":id,"revision":"1","title":"輸出規則","body":"保留 emoji 與標點。"})
        )?["ok"],
        true
    );
    assert_eq!(
        call(
            "delete_note",
            json!({"tool":"delete_note","id":id,"revision":"2"})
        )?["ok"],
        true
    );
    assert_eq!(
        call(
            "restore_note",
            json!({"tool":"restore_note","id":id,"revision":"3"})
        )?["ok"],
        true
    );
    assert_eq!(
        call("read_note", json!({"tool":"read_note","id":id}))?["result"]["body"],
        "保留 emoji 與標點。"
    );
    let mut reopened = Broker::new(project, "second".into())?;
    reopened.enable_memory("chat")?;
    let context = reopened
        .memory()?
        .context(&[Message::user("整理 source.txt")])?;
    assert!(context[0]
        .content
        .contains("這份文件是繁體字與 emoji 測試資料"));
    assert!(!context[0].content.contains(&source));
    let sections = reopened.execute(
        "sections",
        &Tool::ListDocumentSections {
            path: "source.txt".into(),
            offset: 0,
        },
        &mut worker,
        &cancel,
    )?;
    assert_eq!(sections["ok"], true);
    let part = reopened.execute(
        "part",
        &Tool::ReadDocumentSection {
            path: "source.txt".into(),
            revision: revision.as_str().unwrap().into(),
            section_id: "section_002".into(),
        },
        &mut worker,
        &cancel,
    )?;
    assert_eq!(part["result"]["offset"], 4000);
    assert_eq!(
        part["result"]["text"].as_str().unwrap().chars().count(),
        4000
    );
    assert_eq!(
        std::fs::read_to_string(root.join("source.txt")).map_err(|e| e.to_string())?,
        source
    );
    println!("PASS: .lmai hidden/denied, note CRUD/replay/restore, persistent document summary, selective raw section and unchanged source.");
    Ok(())
}
