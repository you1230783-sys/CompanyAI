//! 真實 HTTP 與隔離工具的續接測試。伺服器故意注入壞回覆，不使用公司模型。
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
    LongRead,
    LongReadClean,
    TransientSubmission,
    Replay,
    EmptyForever,
    WrongIdentity,
    Cancelled,
    UnknownSubmission,
    NoProgress,
}

/// 從真實送出訊息擷取工具狀態；測試不直接接觸 broker 內部。
fn progress(body: &Value) -> Value {
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
    serde_json::from_str(content.split_once('\n').unwrap().1).unwrap()
}
fn tool(id: &str, request: Value) -> Value {
    json!({"action":"tool","operation_id":id,"request":request})
}
fn read(id: &str, offset: usize) -> Value {
    tool(
        id,
        json!({"tool":"read_file","path":"source.txt","offset":offset}),
    )
}
fn finish(message: &str, artifacts: Value) -> Value {
    json!({"action":"finish","message":message,"artifacts":artifacts})
}

/// None 表示伺服器宣稱 completed，卻未提供 result 正文。
fn answer(case: Case, round: usize, body: &Value) -> Option<String> {
    let state = progress(body);
    let all = body["messages"].to_string();
    assert!(all.contains("原始任務"));
    assert!(all.contains("補充：保留數字與限制"));
    assert_eq!(body["skills"], false);
    let decision = match case {
        Case::LongRead | Case::LongReadClean | Case::TransientSubmission => {
            let reads = state["operations"].as_array().unwrap().len();
            // 第五次有效閱讀才提供一次可選技能；正常情境完全略過筆記也須能讀完。
            assert_eq!(all.contains("可選技能：長文件閱讀筆記"), round == 5);
            assert!(!all.contains("已累積三段新閱讀或六次有效操作"));
            if round == 6 && matches!(case, Case::LongRead) {
                return Some("done".into());
            }
            if round == 7 && matches!(case, Case::LongRead) {
                return None;
            }
            if round == 8 && matches!(case, Case::LongRead) {
                assert_eq!(state["total_repairs"], 2);
                assert!(state["note"].as_str().unwrap().contains("來源 source.txt"));
                let results: Vec<_> = body["messages"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|m| m["content"].as_str().unwrap().starts_with("工具結果"))
                    .collect();
                assert_eq!(results.len(), 3, "保留兩段已摘要和一段未摘要原文");
                assert!(!results
                    .iter()
                    .any(|m| m["content"].as_str().unwrap().contains("segment_00")));
                assert!(results
                    .iter()
                    .any(|m| m["content"].as_str().unwrap().contains("segment_05")));
            }
            if reads == 23 {
                assert_eq!(state["readings"][0]["fully_read"], true);
                assert_eq!(state["readings"][0]["next_unread_offset"], 138000);
                assert_eq!(state["readings"][0]["read_count"], 0);
                if matches!(case, Case::LongReadClean) {
                    assert!(state["note"].is_null(), "不寫筆記也可正常完成");
                    assert_eq!(state["total_repairs"], 0);
                }
                finish("已讀完 23 段；測試摘要含數字與限制。", json!([]))
            } else {
                let mut next = read(&format!("read_{reads}"), reads * 6000);
                if matches!(case, Case::LongRead) && reads > 0 && reads.is_multiple_of(5) {
                    next["progress_note"] = json!(format!(
                        "來源 source.txt；版本 {}；已確認前 {reads} 段，數值 42，保留限制並繼續。",
                        text::revision(&paper())
                    ));
                }
                next
            }
        }
        Case::Replay => {
            let copies = state["copies"].as_array().unwrap();
            let copy = copies.first().cloned().unwrap_or(json!({}));
            let id = &copy["copy_id"];
            let edit = tool(
                "edit_once",
                json!({"tool":"edit_text","copy_id":id,"revision":text::revision("原始文字"),"start":0,"expected":"","replacement":"新增"}),
            );
            match round {
                0 => read("read", 0),
                1 => tool(
                    "copy",
                    json!({"tool":"create_working_copy","source":"source.txt","name":"修改.txt"}),
                ),
                2 => edit,
                3 => {
                    assert_eq!(copy["revision"], text::revision("新增原始文字"));
                    let save = tool(
                        "should_not_save",
                        json!({"tool":"save_copy","copy_id":id,"revision":copy["revision"]}),
                    );
                    return Some(format!("現在儲存。{save} 等等，我重新輸出。{save}"));
                }
                4 => {
                    assert!(copy["paths"].as_array().unwrap().is_empty());
                    return None;
                }
                5 => {
                    assert_eq!(state["total_repairs"], 2);
                    assert_eq!(copy["revision"], text::revision("新增原始文字"));
                    assert!(copy["paths"].as_array().unwrap().is_empty());
                    edit // 相同 operation_id、相同舊 revision，必須回傳原結果，不能再次插入。
                }
                6 => {
                    assert_eq!(copy["revision"], text::revision("新增原始文字"));
                    tool(
                        "save",
                        json!({"tool":"save_copy","copy_id":id,"revision":copy["revision"]}),
                    )
                }
                7 => finish("", json!([id])),
                8 => {
                    assert_eq!(copy["saved_revision"], copy["revision"]);
                    assert_eq!(copy["paths"].as_array().unwrap().len(), 1);
                    finish("已完成", json!([id]))
                }
                _ => panic!("不應繼續呼叫模型"),
            }
        }
        Case::NoProgress => tool("list_same", json!({"tool":"list_files","path":""})),
        _ => return Some("done".into()),
    };
    Some(decision.to_string())
}
fn paper() -> String {
    (0..23)
        .map(|i| {
            let tag = format!("segment_{i:02}");
            format!("{tag}{}", "x".repeat(6000 - tag.len()))
        })
        .collect()
}

pub fn verify(root: &Path) -> AppResult<()> {
    for case in [
        Case::LongRead,
        Case::LongReadClean,
        Case::TransientSubmission,
        Case::Replay,
        Case::EmptyForever,
        Case::WrongIdentity,
        Case::Cancelled,
        Case::UnknownSubmission,
        Case::NoProgress,
    ] {
        verify_case(root, case)?;
    }
    Ok(())
}
fn verify_case(root: &Path, case: Case) -> AppResult<()> {
    let run_id = format!("continue_{case:?}");
    let workspace = root.join(&run_id);
    std::fs::create_dir(&workspace).map_err(|e| e.to_string())?;
    let original = if matches!(
        case,
        Case::LongRead | Case::LongReadClean | Case::TransientSubmission
    ) {
        paper()
    } else {
        "原始文字".into()
    };
    std::fs::write(workspace.join("source.txt"), &original).map_err(|e| e.to_string())?;
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let config = Config {
        server_url: format!(
            "http://{}",
            listener.local_addr().map_err(|e| e.to_string())?
        ),
        model: "quality".into(),
        ..Config::default()
    };
    let stopped = Arc::new(AtomicBool::new(false));
    let stop = stopped.clone();
    let server = std::thread::spawn(move || -> AppResult<usize> {
        let mut rounds = 0;
        let mut missing = false;
        let mut last = json!({});
        while !stop.load(Ordering::Relaxed) {
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
            let response = if route.contains("/capabilities") {
                json!({"contract_version":1,"principal_id":"fixture_owner","execution_modes":["background"],"attachments":{"enabled":false,"max_count":0,"max_file_bytes":0,"max_total_bytes":0,"allowed_extensions":[]}})
            } else if route.ends_with("/conversations") {
                json!({"conversation_id":"fixture_conversation"})
            } else if route.ends_with("/chat/completions") {
                let content = answer(case, rounds, &body);
                rounds += 1;
                last = json!({"task_id":format!("task_{rounds}"),"client_request_id":body["client_request_id"],"state":"completed","result":content.map(|text|json!({"choices":[{"message":{"role":"assistant","content":text}}]}))});
                match case {
                    Case::WrongIdentity => last["client_request_id"] = json!("wrong_request"),
                    Case::Cancelled => {
                        last["state"] = json!("cancelled");
                        last["error_message"] = json!("已取消");
                    }
                    Case::TransientSubmission if rounds == 20 => {
                        http = 503;
                        missing = true;
                    }
                    Case::UnknownSubmission => http = 503,
                    _ => (),
                }
                last.clone()
            } else if route.contains("/tasks/") {
                if missing {
                    http = 404;
                    missing = false;
                }
                if matches!(case, Case::UnknownSubmission) {
                    http = 404;
                }
                last.clone()
            } else {
                return Err(format!("未知測試路由 {route}"));
            };
            let response = response.to_string();
            write!(stream,"HTTP/1.1 {http} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",response.len()).map_err(|e|e.to_string())?;
        }
        Ok(rounds)
    });
    let session = Session::from_token(
        TokenResponse {
            access_token: "fixture-only".into(),
            token_type: "Bearer".into(),
            expires_in: 3600,
        },
        &config,
    )?;
    let result = runner::run(
        Run {
            resume: false,
            id: run_id.clone(),
            project: Project {
                id: "test".into(),
                name: "測試".into(),
                root: workspace.clone(),
                imports: BTreeMap::new(),
            },
            conversation: "fixture_conversation".into(),
            messages: vec![
                Message::user("原始任務"),
                Message::user("補充：保留數字與限制"),
            ],
            config,
            session,
            root: root.join("app-data"),
            cancel: Arc::new(AtomicBool::new(false)),
        },
        |_| {},
    );
    stopped.store(true, Ordering::Relaxed);
    let rounds = server.join().map_err(|_| "續接測試伺服器失敗。")??;
    match case {
        Case::LongRead => {
            assert!(result?.contains("23 段"));
            assert_eq!(rounds, 26);
        }
        Case::LongReadClean | Case::TransientSubmission => {
            assert!(result?.contains("23 段"));
            assert_eq!(rounds, 24);
        }
        Case::Replay => {
            let result = result?;
            assert!(result.contains("修改.txt"));
            assert_eq!(rounds, 9);
            let output = std::fs::read_dir(workspace.join("_AI_Output"))
                .map_err(|e| e.to_string())?
                .next()
                .unwrap()
                .map_err(|e| e.to_string())?
                .path();
            let files: Vec<_> = std::fs::read_dir(output)
                .map_err(|e| e.to_string())?
                .collect();
            assert_eq!(files.len(), 1);
            let bytes =
                std::fs::read(files[0].as_ref().unwrap().path()).map_err(|e| e.to_string())?;
            let decoded = String::from_utf8(bytes).unwrap();
            assert_eq!(decoded.trim_start_matches('\u{feff}'), "新增原始文字");
        }
        Case::EmptyForever => {
            assert!(result?.contains("重試兩次"));
            assert!(runner::paused_available(&root.join("app-data"), &run_id));
            assert_eq!(rounds, 3);
        }
        Case::WrongIdentity => {
            assert!(result.unwrap_err().contains("識別碼"));
            assert_eq!(rounds, 1);
        }
        Case::Cancelled => {
            assert!(result.unwrap_err().contains("已取消"));
            assert_eq!(rounds, 1);
        }
        Case::UnknownSubmission => {
            let error = result?;
            assert!(runner::paused_available(&root.join("app-data"), &run_id));
            assert!(
                error.contains("未另建請求")
                    && error.contains("提交 POST")
                    && error.contains("503")
                    && error.contains("查詢 GET")
                    && error.contains("404"),
                "{error}"
            );
            assert_eq!(rounds, 1);
        }
        Case::NoProgress => {
            assert!(result?.contains("八次"));
            assert!(runner::paused_available(&root.join("app-data"), &run_id));
            assert_eq!(rounds, 9);
        }
    }
    assert_eq!(
        std::fs::read_to_string(workspace.join("source.txt")).map_err(|e| e.to_string())?,
        original
    );
    println!("PASS: continuation {case:?}, {rounds} model requests, original unchanged.");
    Ok(())
}
