//! 真正達到 60 次工具上限後銷毀執行器，再由 DPAPI 暫存恢復，不縮小正式上限。
use company_ai::{
    config::Config,
    projects::{
        runner::{self, Run},
        text, Project,
    },
    protocol::{Message, TokenResponse},
    storage::Session,
    AppResult,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    io::Write,
    net::TcpListener,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

pub fn verify(root: &Path) -> AppResult<()> {
    let workspace = root.join("pause-limit");
    std::fs::create_dir(&workspace).map_err(|e| e.to_string())?;
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let config = Config {
        server_url: format!(
            "http://{}",
            listener.local_addr().map_err(|e| e.to_string())?
        ),
        ..Config::default()
    };
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = stop.clone();
    let server = std::thread::spawn(move || -> AppResult<usize> {
        let mut round = 0;
        let mut original_save = Value::Null;
        let mut copy_id = String::new();
        while !stopped.load(Ordering::Relaxed) {
            let (mut stream, _) = match listener.accept() {
                Ok(s) => s,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(e) => return Err(e.to_string()),
            };
            let (route, body) = super::roundtrip::request(&mut stream)?;
            let response = if route.contains("capabilities") {
                json!({"contract_version":1,"principal_id":"pause_owner","execution_modes":["background"],"attachments":{"enabled":false,"max_count":0,"max_file_bytes":0,"max_total_bytes":0,"allowed_extensions":[]}})
            } else if route.ends_with("conversations") {
                json!({"conversation_id":"pause_conversation"})
            } else {
                assert!(route.ends_with("chat/completions"));
                let progress: Value = serde_json::from_str(
                    body["messages"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find_map(|m| {
                            m["content"]
                                .as_str()
                                .filter(|s| s.starts_with("本機續接資料"))
                        })
                        .unwrap()
                        .split_once('\n')
                        .unwrap()
                        .1,
                )
                .unwrap();
                let copy = &progress["copies"][0];
                if round > 0 {
                    if copy_id.is_empty() {
                        copy_id = copy["copy_id"].as_str().unwrap().into();
                    }
                    assert_eq!(copy["copy_id"], copy_id);
                }
                if round == 60 {
                    assert!(
                        body["messages"].as_array().unwrap().len() < 12,
                        "續接不可重送全部 60 輪歷程"
                    );
                }
                if round == 56 {
                    let reminder = body["messages"].to_string();
                    assert!(reminder.contains("arguments.progress_note"));
                    assert!(reminder.contains("剩餘 24 次模型回覆、4 次工具操作"));
                }
                let request = match round {
                    0 => json!({"tool":"create_working_copy","source":null,"name":"接續.txt"}),
                    2 => {
                        original_save = json!({"tool":"save_copy","copy_id":copy_id,"revision":copy["revision"]});
                        original_save.clone()
                    }
                    60 => {
                        assert_eq!(copy["revision"], text::revision(&"x".repeat(58)));
                        original_save.clone()
                    }
                    61 => json!({"tool":"save_copy","copy_id":copy_id,"revision":copy["revision"]}),
                    62 => Value::Null,
                    _ => {
                        json!({"tool":"edit_text","copy_id":copy_id,"revision":copy["revision"],"start":0,"expected":"","replacement":"x"})
                    }
                };
                let decision = if round == 62 {
                    json!({"action":"finish","message":"續接完成","artifacts":[copy_id]})
                } else {
                    json!({"action":"tool","operation_id":format!("op{}",if round==60 {2} else {round}),"request":request})
                };
                // 故意不寫模型筆記：程式狀態仍需足以保存及恢复工作副本。
                round += 1;
                json!({"task_id":format!("pause_{round}"),"client_request_id":body["client_request_id"],"state":"completed","result":{"choices":[{"message":{"content":super::roundtrip::tool_call(decision,true).to_string()}}]}})
            };
            let body = response.to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).map_err(|e|e.to_string())?;
        }
        Ok(round)
    });
    let session = Session::from_token(
        TokenResponse {
            access_token: "test".into(),
            token_type: "Bearer".into(),
            expires_in: 3600,
        },
        &config,
    )?;
    let run = |resume| Run {
        resume,
        id: "pause_limit".into(),
        project: Project {
            id: "pause_project".into(),
            name: "測試".into(),
            root: workspace.clone(),
            imports: BTreeMap::new(),
        },
        conversation: "pause_conversation".into(),
        messages: vec![Message::user("新增檔案並保留工作進度")],
        config: config.clone(),
        session: session.clone(),
        root: root.join("app-data"),
        cancel: Arc::new(AtomicBool::new(false)),
        instructions: None,
        outlook_consent: None,
        file_waiter: None,
    };
    let result = (|| -> AppResult<()> {
        let paused = runner::run_legacy_test(run(false), |_| {})?;
        assert!(paused.contains("請按「繼續」"), "{paused}");
        assert!(runner::paused_available(
            &root.join("app-data"),
            "pause_limit"
        ));
        let bytes = std::fs::read(root.join("app-data/project-runs/pause_limit.resume.dpapi"))
            .map_err(|e| e.to_string())?;
        assert!(
            serde_json::from_slice::<Value>(&bytes).is_err(),
            "暫存不能是明文 JSON"
        );
        let mut wrong_project = run(true);
        wrong_project.project.id = "another-project".into();
        assert!(runner::run_legacy_test(wrong_project, |_| {})
            .unwrap_err()
            .contains("授權已變更"));
        assert!(runner::paused_available(
            &root.join("app-data"),
            "pause_limit"
        ));
        let finished = runner::run_legacy_test(run(true), |_| {})?;
        assert!(finished.contains("續接完成"), "{finished}");
        assert!(!runner::paused_available(
            &root.join("app-data"),
            "pause_limit"
        ));
        let folder = std::fs::read_dir(workspace.join("_AI_Output"))
            .map_err(|e| e.to_string())?
            .next()
            .unwrap()
            .map_err(|e| e.to_string())?
            .path();
        assert_eq!(
            std::fs::read_dir(&folder)
                .map_err(|e| e.to_string())?
                .count(),
            2,
            "去重不能多發布一份"
        );
        let content =
            std::fs::read_to_string(folder.join("接續_2.txt")).map_err(|e| e.to_string())?;
        assert_eq!(content.trim_start_matches('\u{feff}'), "x".repeat(58));
        Ok(())
    })();
    stop.store(true, Ordering::Relaxed);
    let rounds = server.join().map_err(|_| "暫停測試伺服器失敗。")??;
    result?;
    assert_eq!(rounds, 63);
    println!("PASS pause: 60 real tools, DPAPI restart, unsaved copy restoration, saved operation dedup.");
    Ok(())
}
