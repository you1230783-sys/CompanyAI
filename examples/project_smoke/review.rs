//! 真實 AppContainer、broker 與 DPAPI 快照往返，不以模型手填資料取代原文。
use company_ai::{
    projects::{files::Broker, sandbox::Worker, Project, Tool},
    AppResult,
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path, sync::atomic::AtomicBool};
pub fn verify(exe: &Path, parent: &Path) -> AppResult<()> {
    let root = parent.join("review-tools");
    std::fs::create_dir(&root).map_err(|e| e.to_string())?;
    let original = "原文😀\n待改文字\n保留行";
    std::fs::write(root.join("source.txt"), original).map_err(|e| e.to_string())?;
    std::fs::write(root.join("many.txt"), "命中\n".repeat(65)).map_err(|e| e.to_string())?;
    std::fs::write(
        root.join("events.log"),
        "2026/10/12, 10:03:00.000 Start\n2026/10/12, 10:04:00.000 End\n",
    )
    .map_err(|e| e.to_string())?;
    let project = Project {
        id: "review".into(),
        name: "review".into(),
        root: root.clone(),
        imports: BTreeMap::new(),
    };
    let mut broker = Broker::new(project.clone(), "review-run".into())?;
    broker.enable_memory("review-chat")?;
    let cancel = AtomicBool::new(false);
    let mut worker = Worker::start(exe, &cancel)?;
    let mut call = |id: &str, request: Value| -> AppResult<Value> {
        let tool: Tool = serde_json::from_value(request).map_err(|e| e.to_string())?;
        let value = broker.execute(id, &tool, &mut worker, &cancel)?;
        assert_eq!(value["ok"], true, "{value}");
        Ok(value["result"].clone())
    };
    let found = call(
        "search",
        json!({"tool":"search_files","paths":["source.txt","missing.txt"],"query":"待改"}),
    )?;
    assert_eq!(found["complete"], false);
    assert_eq!(found["matches"][0]["line"], 2);
    assert!(found["matches"][0]["excerpt"]
        .as_str()
        .unwrap()
        .contains("原文😀"));
    let capped = call(
        "cap",
        json!({"tool":"search_files","paths":["many.txt"],"query":"命中"}),
    )?;
    assert_eq!(capped["matches"].as_array().unwrap().len(), 60);
    assert_eq!(capped["coverage"][0]["omitted"], 5);
    assert_eq!(capped["complete"], false);
    let copy = call(
        "copy",
        json!({"tool":"create_working_copy","source":"source.txt","name":"修改.txt"}),
    )?;
    let edit = call(
        "edit",
        json!({"tool":"edit_text","copy_id":copy["copy_id"],"revision":copy["revision"],"start":4,"expected":"待改文字","replacement":"修訂完成"}),
    )?;
    let first = call(
        "save1",
        json!({"tool":"save_copy","copy_id":copy["copy_id"],"revision":edit["revision"]}),
    )?;
    let edit = call(
        "edit2",
        json!({"tool":"edit_text","copy_id":copy["copy_id"],"revision":edit["revision"],"start":4,"expected":"修訂完成","replacement":"第二次修訂"}),
    )?;
    let second = call(
        "save2",
        json!({"tool":"save_copy","copy_id":copy["copy_id"],"revision":edit["revision"]}),
    )?;
    call(
        "log",
        json!({"tool":"read_log","path":"events.log","revision":null,"start_line":1,"start_column":0,"line_count":10}),
    )?;
    let events = call(
        "events",
        json!({"tool":"record_log_events","events":[
            {"label":"開始","group":"A/1","phase":"start","evidence":{"operation_id":"log","pointer":"/lines/0"}},
            {"label":"結束","group":"A/1","phase":"end","evidence":{"operation_id":"log","pointer":"/lines/1"}}
        ]}),
    )?;
    assert_eq!(events["needs_review"], 0);
    // 原件後來改變，歷史比較仍使用當時基準及各次成果，重啟後也不能混用。
    std::fs::write(root.join("source.txt"), "外部變更").map_err(|e| e.to_string())?;
    let snapshots = broker.review_snapshots();
    let first_snapshot = snapshots
        .iter()
        .find(|s| s.path == first["path"].as_str().unwrap())
        .unwrap();
    let second_snapshot = snapshots
        .iter()
        .find(|s| s.path == second["path"].as_str().unwrap())
        .unwrap();
    assert_eq!(first_snapshot.before.as_deref(), Some(original));
    assert!(first_snapshot.after.contains("修訂完成") && !first_snapshot.after.contains("第二次"));
    assert!(second_snapshot.after.contains("第二次修訂"));
    println!("PASS review tools: real source search/cap/error, two versioned copies, fixed baseline after source change, source-bound LOG pairing.");
    Ok(())
}
