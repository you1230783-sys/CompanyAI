//! 以縮短的測試時鐘驗證逾時／壞回覆續接；正式 EXE 仍固定兩小時。
//! 真實 HTTP、DPAPI 與 AppContainer，確認既有副本及待查模型請求不被重做。
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

#[derive(Clone, Copy, Debug)]
enum Case {
    Deadline,
    LateCompleted,
    Malformed,
    UnknownRequest,
}

pub fn verify(root: &Path) -> AppResult<()> {
    for case in [
        Case::Deadline,
        Case::LateCompleted,
        Case::Malformed,
        Case::UnknownRequest,
    ] {
        verify_case(root, case)?;
    }
    Ok(())
}

fn verify_case(root: &Path, case: Case) -> AppResult<()> {
    let id = format!("interrupt_{case:?}");
    let workspace = root.join(&id);
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
    let ready = Arc::new(AtomicBool::new(false));
    let stopped = stop.clone();
    let resumed = ready.clone();
    let server = std::thread::spawn(move || -> AppResult<(usize, usize)> {
        let mut posts = 0;
        let mut gets = 0;
        let mut pending = Value::Null;
        let mut copy_id = String::new();
        while !stopped.load(Ordering::Relaxed) {
            let (mut stream, _) = match listener.accept() {
                Ok(pair) => pair,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(e) => return Err(e.to_string()),
            };
            let (route, body) = super::roundtrip::request(&mut stream)?;
            let mut http = 200;
            let response = if route.contains("capabilities") {
                json!({"contract_version":1,"principal_id":"interrupt_owner","execution_modes":["background"],"attachments":{"enabled":false,"max_count":0,"max_file_bytes":0,"max_total_bytes":0,"allowed_extensions":[]}})
            } else if route.ends_with("conversations") {
                json!({"conversation_id":"interrupt_conversation"})
            } else if route.ends_with("chat/completions") {
                let content = body["messages"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find_map(|m| {
                        m["content"]
                            .as_str()
                            .filter(|s| s.starts_with("本機續接資料"))
                    })
                    .unwrap();
                let state: Value =
                    serde_json::from_str(content.split_once('\n').unwrap().1).unwrap();
                let copy = &state["copies"][0];
                if posts > 0 {
                    if copy_id.is_empty() {
                        copy_id = copy["copy_id"].as_str().unwrap().into();
                    }
                    assert_eq!(copy["copy_id"], copy_id, "續接不得建立另一份副本");
                }
                let finish_at = if matches!(case, Case::Malformed) {
                    6
                } else {
                    3
                };
                let decision = match posts {
                    0 => {
                        json!({"action":"tool","operation_id":"new","request":{"tool":"create_working_copy","source":null,"name":"長任務.txt"}})
                    }
                    1 => {
                        json!({"action":"tool","operation_id":"edit","request":{"tool":"edit_text","copy_id":copy_id,"revision":copy["revision"],"start":0,"expected":"","replacement":"保留的未儲存進度"}})
                    }
                    n if n == finish_at => {
                        assert_eq!(copy["saved_revision"], text::revision("保留的未儲存進度"));
                        json!({"action":"finish","message":"中斷後續接完成","artifacts":[copy_id]})
                    }
                    _ => {
                        assert_eq!(copy["revision"], text::revision("保留的未儲存進度"));
                        json!({"action":"tool","operation_id":"save","request":{"tool":"save_copy","copy_id":copy_id,"revision":copy["revision"]}})
                    }
                };
                let malformed = matches!(case, Case::Malformed) && (2..=4).contains(&posts);
                let content = if malformed {
                    "done".into()
                } else {
                    decision.to_string()
                };
                let mut response = json!({"task_id":format!("interrupt_{posts}"),"client_request_id":body["client_request_id"],"state":"completed","result":{"choices":[{"message":{"content":content}}]}});
                if posts == 2 && !matches!(case, Case::Malformed) {
                    pending = response.clone();
                    match case {
                        Case::Deadline => {
                            response["state"] = json!("running");
                            response["result"] = Value::Null;
                        }
                        Case::LateCompleted => std::thread::sleep(Duration::from_millis(2200)),
                        Case::UnknownRequest => {
                            http = 503;
                        }
                        Case::Malformed => unreachable!(),
                    }
                }
                posts += 1;
                response
            } else if route.contains("/tasks/") {
                gets += 1;
                // 恢復時必須仍查同一個 ID；後端記錄缺失不可換 ID 提交。
                assert!(
                    route.ends_with(pending["task_id"].as_str().unwrap())
                        || route.ends_with(pending["client_request_id"].as_str().unwrap())
                );
                let mut response = pending.clone();
                if !resumed.load(Ordering::Relaxed) {
                    if matches!(case, Case::UnknownRequest) {
                        http = 404;
                    } else {
                        response["state"] = json!("running");
                        response["result"] = Value::Null;
                    }
                }
                response
            } else {
                return Err(format!("未知測試路由 {route}"));
            };
            let body = response.to_string();
            write!(stream,"HTTP/1.1 {http} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).map_err(|e|e.to_string())?;
        }
        Ok((posts, gets))
    });
    let session = Session::from_token(
        TokenResponse {
            access_token: "fixture-only".into(),
            token_type: "Bearer".into(),
            expires_in: 3600,
        },
        &config,
    )?;
    let run = |resume| Run {
        resume,
        id: id.clone(),
        project: Project {
            id: "interrupt_project".into(),
            name: "測試".into(),
            root: workspace.clone(),
            imports: BTreeMap::new(),
        },
        conversation: "interrupt_conversation".into(),
        messages: vec![Message::user("建立並保存長任務.txt")],
        config: config.clone(),
        session: session.clone(),
        root: root.join("app-data"),
        cancel: Arc::new(AtomicBool::new(false)),
    };
    let result = (|| -> AppResult<()> {
        let budget = if matches!(case, Case::Deadline | Case::LateCompleted) {
            2
        } else {
            30
        };
        let paused =
            runner::run_legacy_with_test_budget(run(false), |_| {}, Duration::from_secs(budget))?;
        assert!(paused.contains("請按「繼續」"), "{paused}");
        assert!(runner::paused_available(&root.join("app-data"), &id));
        assert!(!workspace.join("_AI_Output").exists(), "暫停前尚未儲存");
        ready.store(true, Ordering::Relaxed);
        assert!(runner::run_legacy_test(run(true), |_| {})?.contains("中斷後續接完成"));
        assert!(!runner::paused_available(&root.join("app-data"), &id));
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
            1
        );
        assert_eq!(
            std::fs::read_to_string(folder.join("長任務.txt"))
                .map_err(|e| e.to_string())?
                .trim_start_matches('\u{feff}'),
            "保留的未儲存進度"
        );
        Ok(())
    })();
    stop.store(true, Ordering::Relaxed);
    let (posts, gets) = server.join().map_err(|_| "中斷測試伺服器失敗。")??;
    result?;
    assert_eq!(
        posts,
        if matches!(case, Case::Malformed) {
            7
        } else {
            4
        }
    );
    if matches!(case, Case::Deadline | Case::UnknownRequest) {
        assert!(gets > 0);
    }
    if matches!(case, Case::LateCompleted) {
        assert_eq!(gets, 0, "已取得終態直接沿用，不再查詢");
    }
    println!("PASS interruption {case:?}: encrypted pause, same unsaved copy, {posts} POSTs, {gets} GETs, one output.");
    Ok(())
}
