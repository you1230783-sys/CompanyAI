//! 真正 loopback HTTP 驗證代理往返，不使用公司服務或真實模型。
use company_ai::{
    config::Config,
    projects::{
        runner::{self, Run},
        Project,
    },
    protocol::{Message, TokenResponse},
    storage::Session,
    AppResult,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

pub(super) fn request(stream: &mut TcpStream) -> AppResult<(String, Value)> {
    stream.set_nonblocking(false).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    let mut data = Vec::new();
    let end;
    loop {
        let mut byte = [0];
        stream.read_exact(&mut byte).map_err(|e| e.to_string())?;
        data.push(byte[0]);
        if data.ends_with(b"\r\n\r\n") {
            end = data.len();
            break;
        }
        if data.len() > 32_000 {
            return Err("測試 Header 過大。".into());
        }
    }
    let header = String::from_utf8(data.clone()).map_err(|e| e.to_string())?;
    let length: usize = header
        .lines()
        .find_map(|line| {
            line.split_once(':')
                .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                .and_then(|(_, value)| value.trim().parse().ok())
        })
        .unwrap_or(0);
    data.resize(end + length, 0);
    stream
        .read_exact(&mut data[end..])
        .map_err(|e| e.to_string())?;
    let route = header
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .ok_or("測試 URL 缺失。")?
        .to_owned();
    let value = if length == 0 {
        json!({})
    } else {
        serde_json::from_slice(&data[end..]).map_err(|e| e.to_string())?
    };
    Ok((route, value))
}

pub fn verify(root: &Path) -> AppResult<()> {
    for mode in 0..=7 {
        verify_case(root, mode)?;
    }
    Ok(())
}

/// 將既有測試步驟轉成文字 tool_calls；不依賴產品的轉換層產生預期資料。
pub(super) fn tool_call(mut decision: Value, object_arguments: bool) -> Value {
    let action = decision["action"].as_str().unwrap().to_owned();
    let (id, name, mut arguments) = if action == "tool" {
        let mut arguments = decision["request"].clone();
        let name = arguments.as_object_mut().unwrap().remove("tool").unwrap();
        (decision["operation_id"].clone(), name, arguments)
    } else {
        decision.as_object_mut().unwrap().remove("action");
        (json!("terminal_call"), json!(action), decision.clone())
    };
    if let Some(note) = decision.get("progress_note") {
        arguments["progress_note"] = note.clone();
    }
    let arguments = if object_arguments {
        arguments
    } else {
        json!(arguments.to_string())
    };
    json!({"content":"依工具結果繼續處理。","tool_calls":[{
        "id":id,"type":"function","function":{"name":name,"arguments":arguments}
    }]})
}

/// 0–3 保留舊格式；4 標準字串參數；5 物件參數；6 新格式詢問；7 多呼叫拒絕後修復。
fn verify_case(root: &Path, mode: u8) -> AppResult<()> {
    let run_id = format!("roundtrip_{mode}");
    let workspace = root.join(&run_id);
    std::fs::create_dir(&workspace).map_err(|e| e.to_string())?;
    std::fs::write(workspace.join("source.txt"), "原始文字").map_err(|e| e.to_string())?;
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
        let mut step = 0;
        let mut repair_sent = false;
        let mut request_ids = std::collections::HashSet::new();
        let mut last_status = json!({});
        let mut copy_id = String::new();
        while !stop.load(Ordering::Relaxed) {
            let (mut stream, _) = match listener.accept() {
                Ok(connection) => connection,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(e) => return Err(e.to_string()),
            };
            let (route, body) = request(&mut stream)?;
            let result = if route.contains("/capabilities") {
                json!({"contract_version":1,"principal_id":"fixture_owner","execution_modes":["background"],"attachments":{"enabled":false,"max_count":0,"max_file_bytes":0,"max_total_bytes":0,"allowed_extensions":[]}})
            } else if route.ends_with("/conversations") {
                json!({"conversation_id":"fixture_conversation"})
            } else if route.ends_with("/chat/completions") {
                assert_eq!(body["skills"], false);
                assert!(body.get("outlook_triage").is_none());
                assert!(body.get("auto_generate_title").is_none());
                assert_eq!(body["messages"][0]["role"], "system");
                assert!(body.get("tools").is_none());
                assert!(body.get("tool_choice").is_none());
                assert!(body["messages"][0]["content"]
                    .as_str()
                    .unwrap()
                    .contains("\"parameters\""));
                assert_eq!(body["model"], "quality");
                assert!(request_ids.insert(body["client_request_id"].as_str().unwrap().to_string()));
                if mode == 2 && step == 2 && repair_sent {
                    assert!(
                        body["messages"].as_array().unwrap().last().unwrap()["content"]
                            .as_str()
                            .unwrap()
                            .contains("上一則工具要求尚未執行")
                    );
                }
                let previous: Value = if step >= 1 {
                    let content = body["messages"]
                        .as_array()
                        .and_then(|m| {
                            m.iter().rev().find(|message| {
                                message["role"] == "user"
                                    && message["content"]
                                        .as_str()
                                        .is_some_and(|s| s.starts_with("工具結果"))
                            })
                        })
                        .and_then(|m| m["content"].as_str())
                        .ok_or("缺少工具結果。")?;
                    let envelope: Value = serde_json::from_str(
                        content.split_once('\n').ok_or("工具結果格式錯誤。")?.1,
                    )
                    .map_err(|e| e.to_string())?;
                    assert_eq!(envelope["role"], "tool");
                    assert_eq!(
                        envelope["tool_call_id"],
                        ["read1", "copy1", "edit1", "save1"][step - 1]
                    );
                    serde_json::from_str(envelope["content"].as_str().ok_or("工具結果內容缺失。")?)
                        .map_err(|e| e.to_string())?
                } else {
                    json!({})
                };
                if step >= 1 {
                    assert_eq!(previous["ok"], true, "{previous}");
                }
                let decision = if matches!(mode, 3 | 6) && step == 1 {
                    json!({"action":"ask_user","message":"請匯入兩篇 PDF 的文字。"})
                } else {
                    match step {
                        0 => {
                            json!({"action":"tool","operation_id":"read1","request":{"tool":"read_file","path":"source.txt","offset":0}})
                        }
                        1 => {
                            assert_eq!(previous["result"]["text"], "原始文字");
                            json!({"action":"tool","operation_id":"copy1","request":{"tool":"create_working_copy","source":"source.txt","name":"修改.txt"}})
                        }
                        2 => {
                            copy_id = previous["result"]["copy_id"]
                                .as_str()
                                .ok_or("缺少副本 ID。")?
                                .into();
                            json!({"action":"tool","operation_id":"edit1","request":{"tool":"edit_text","copy_id":copy_id,"revision":previous["result"]["revision"],"start":0,"expected":"原始","replacement":"修訂"}})
                        }
                        3 => {
                            json!({"action":"tool","operation_id":"save1","request":{"tool":"save_copy","copy_id":copy_id,"revision":previous["result"]["revision"]}})
                        }
                        4 => {
                            json!({"action":"finish","message":"本機測試已完成修訂。","artifacts":[copy_id],"task_summary":"已完成來源修訂與副本交付；來源保持不變。"})
                        }
                        _ => return Err("不應出現額外模型請求。".into()),
                    }
                };
                let decision = if mode >= 4 {
                    tool_call(decision, mode == 5)
                } else {
                    decision
                };
                let content = if mode == 7 && step == 2 && !repair_sent {
                    let mut batch = decision.clone();
                    batch["tool_calls"]
                        .as_array_mut()
                        .unwrap()
                        .push(decision["tool_calls"][0].clone());
                    repair_sent = true;
                    batch.to_string()
                } else if step == 2 && mode == 2 {
                    repair_sent = true;
                    "工具：{\"action\":\"tool\"".into()
                } else {
                    let content = if mode == 3 && step == 1 {
                        format!("文件尚缺 PDF 文字，需要請你協助：\n{decision}")
                    } else if mode == 1 && step == 2 {
                        format!("現在修改工作副本。\n{decision}")
                    } else {
                        decision.to_string()
                    };
                    step += 1;
                    content
                };
                rounds += 1;
                last_status = json!({"task_id":format!("task_{rounds}"),"client_request_id":body["client_request_id"],"state":"completed","result":{"choices":[{"message":{"role":"assistant","content":content}}]}});
                last_status.clone()
            } else if route.contains("/tasks/") {
                last_status.clone()
            } else {
                return Err(format!("非預期測試路由：{route}"));
            };
            let response = result.to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",response.len(),response).map_err(|e|e.to_string())?;
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
            conversation: "roundtrip_conversation".into(),
            messages: vec![Message::user("修訂來源 TXT 並保存副本")],
            config,
            session,
            root: root.join("app-data"),
            cancel: Arc::new(AtomicBool::new(false)),
        },
        |_| {},
    );
    stopped.store(true, Ordering::Relaxed);
    let rounds = server.join().map_err(|_| "測試伺服器失敗。")??;
    let activity = runner::recover_activity(&root.join("app-data"), &run_id)?;
    if matches!(mode, 3 | 6) {
        assert_eq!(result?, "需要你的補充：請匯入兩篇 PDF 的文字。");
        assert_eq!(rounds, 2);
        if mode == 3 {
            assert!(activity.iter().any(|step| step.contains("文件尚缺 PDF")));
        } else {
            assert!(activity
                .iter()
                .any(|step| step.contains("依工具結果繼續處理")));
        }
        assert!(!workspace.join("_AI_Output").exists());
        println!("PASS: wrapped ask_user displayed without JSON retry or false failure.");
        return Ok(());
    }
    assert!(activity.iter().any(|step| step == "建立副本：完成"));
    if mode == 2 {
        assert!(result?.contains("重試兩次"));
        assert!(runner::paused_available(&root.join("app-data"), &run_id));
        assert_eq!(rounds, 5);
        assert!(!activity.iter().any(|step| step == "編輯文字：完成"));
        assert!(!workspace.join("_AI_Output").exists());
    } else {
        let answer = result?;
        assert_eq!(runner::recover(&root.join("app-data"), &run_id)?, answer);
        assert_eq!(rounds, if mode == 7 { 6 } else { 5 });
        if mode == 1 {
            assert!(activity
                .iter()
                .any(|text| text == "AI 說明：現在修改工作副本。"));
        }
        assert!(answer.contains("本機測試已完成修訂"));
        let memory = company_ai::projects::memory::Memory::open(
            Project {
                id: "test".into(),
                name: "測試".into(),
                root: workspace.clone(),
                imports: BTreeMap::new(),
            },
            "roundtrip_conversation",
        )?;
        let mut user = Message::user("修訂來源 TXT 並保存副本");
        user.request_id = Some(run_id.clone());
        let next = memory.context(&[
            user,
            Message::assistant(answer.clone()),
            Message::user("接著修改成果"),
        ])?;
        assert!(next[0].content.contains("已完成來源修訂與副本交付"));
        assert!(!next[0].content.contains("本機測試已完成修訂"));
        assert!(memory.read_task_result(&run_id, "result", 0)?["text"]
            .as_str()
            .unwrap()
            .contains("本機測試已完成修訂"));

        assert_eq!(
            activity
                .iter()
                .filter(|step| step.as_str() == "編輯文字：完成")
                .count(),
            1
        );
        assert_eq!(
            activity
                .iter()
                .filter(|step| step.as_str() == "儲存副本：完成")
                .count(),
            1
        );
    }
    assert_eq!(
        std::fs::read_to_string(workspace.join("source.txt")).map_err(|e| e.to_string())?,
        "原始文字"
    );
    println!("PASS: HTTP/skills/tool loop case {mode}, bounded JSON repair, activity history and original preserved.");
    Ok(())
}
