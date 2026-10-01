//! 原生代理的正式入口整合測試：真實 HTTP、DPAPI 與受限檔案 worker。
//! 每轮故意重用 call_001，確認只按請求範圍去重，不跳過後續合法修改。
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
    for case in 0..=10 {
        verify_case(root, case)?;
    }
    Ok(())
}

fn capability(model: &str, strict: bool) -> Value {
    let mut caps: Value = serde_json::from_str(include_str!(
        "../../src/projects/agent/test_capabilities.json"
    ))
    .unwrap();
    caps["model"] = json!(model);
    caps["principal_id"] = json!("fixture_owner");
    caps["strict_tool_arguments"] = json!(strict);
    caps
}
fn fill_optional(args: &mut Value, schema: &Value) {
    if let Some(props) = schema["properties"].as_object() {
        for (name, _) in props {
            if args.get(name).is_none() {
                args[name] = Value::Null;
            }
        }
    }
}
fn call(body: &Value, name: &str, mut args: Value) -> Value {
    let tool = body["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["function"]["name"] == name)
        .unwrap();
    fill_optional(&mut args, &tool["function"]["parameters"]);
    json!({"role":"assistant","content":"**處理中**","tool_calls":[{"id":"call_001","type":"function",
        "function":{"name":name,"arguments":args.to_string()}}]})
}
fn last_result(body: &Value) -> Value {
    body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .find(|m| m["role"] == "tool")
        .map(|m| serde_json::from_str(m["content"].as_str().unwrap()).unwrap())
        .unwrap_or(Value::Null)
}

/// 0 正常；1 非 strict 格式修復；2 原 POST 回覆遺失＋暫停續接；3 截斷不執行；
/// 4 能力缺少不降級；5 原生快速委派；6 身分錯誤；7 多工具不執行。
/// 8 子請求未知後續接；9 明確拒絕不輪詢；10 取消查回本請求後取消。
fn verify_case(root: &Path, case: usize) -> AppResult<()> {
    let id = format!("native_{case}");
    let workspace = root.join(&id);
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
    let resumed = Arc::new(AtomicBool::new(false));
    let resumed_server = resumed.clone();
    let cancel = Arc::new(AtomicBool::new(false));
    let server_cancel = cancel.clone();
    let cancel_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let server_cancel_count = cancel_count.clone();
    let server = std::thread::spawn(move || -> AppResult<(usize, usize)> {
        let mut posts = 0;
        let mut fast = 0;
        let mut step = 0;
        let mut statuses = BTreeMap::<String, Value>::new();
        let mut copy = String::new();
        let mut repaired = false;
        let mut ids = std::collections::BTreeSet::new();
        while !stop.load(Ordering::Relaxed) {
            let (mut stream, _) = match listener.accept() {
                Ok(v) => v,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(e) => return Err(e.to_string()),
            };
            let (route, body) = super::roundtrip::request(&mut stream)?;
            let mut http = 200;
            let response = if route.contains("/agent/capabilities") {
                let mut cap = capability(
                    if route.ends_with("=fast") {
                        "fast"
                    } else {
                        "quality"
                    },
                    case != 1,
                );
                if case == 4 {
                    cap["native_tool_calls"] = json!(false);
                }
                cap
            } else if route.contains("/capabilities") {
                json!({"contract_version":1,"principal_id":"fixture_owner","execution_modes":["background"],"attachments":{"enabled":false,"max_count":0,"max_file_bytes":0,"max_total_bytes":0,"allowed_extensions":[]}})
            } else if route.ends_with("/conversations") {
                json!({"conversation_id":"fixture_conversation"})
            } else if route.ends_with("/models") {
                json!({"models":[{"id":"quality","label":"品質"},{"id":"fast","label":"快速"}],"default_model":"quality"})
            } else if route.ends_with("/agent/turns") {
                posts += 1;
                assert_eq!(body["contract_version"], "desktop-agent-v1");
                assert_eq!(body["skills"], false);
                assert_eq!(body["context"]["context_policy"], "client_snapshot");
                assert_eq!(body["parallel_tool_calls"], false);
                assert!(body.get("attachment_tokens").is_none());
                assert!(ids.insert(body["client_request_id"].as_str().unwrap().to_string()));
                let messages = body["messages"].as_array().unwrap();
                assert_eq!(messages.iter().filter(|m| m["role"] == "system").count(), 1);
                for (i, message) in messages.iter().enumerate() {
                    assert!(message.get("provider_specific_fields").is_none());
                    if let Some(calls) = message["tool_calls"].as_array() {
                        for call in calls {
                            assert!(call.get("provider_specific_fields").is_none());
                            assert!(call["function"].get("provider_specific_fields").is_none());
                        }
                    }
                    if message["role"] == "tool" {
                        assert_eq!(
                            message["tool_call_id"],
                            messages[i - 1]["tool_calls"][0]["id"]
                        );
                    }
                }
                let previous = last_result(&body);
                let mut message = if body["model"] == "fast" {
                    fast += 1;
                    assert_eq!(body["tools"], json!([]));
                    assert_eq!(body["tool_choice"], "none");
                    assert!(body["context"]["parent_request_id"].is_string());
                    json!({"role":"assistant","content":"原文為原始文字，無其他數據。"})
                } else if case == 3 {
                    call(&body, "read_file", json!({"path":"source.txt","offset":0}))
                } else if matches!(case, 5 | 8) {
                    if step == 0 {
                        step += 1;
                        call(
                            &body,
                            "summarize_document",
                            json!({"path":"source.txt","focus":"文字摘要"}),
                        )
                    } else {
                        assert_eq!(previous["ok"], true, "{previous}");
                        call(
                            &body,
                            "finish",
                            json!({"message":"委派完成","artifacts":[]}),
                        )
                    }
                } else if case == 1 && step == 1 && !repaired {
                    repaired = true;
                    let mut m = call(&body, "read_file", json!({"path":"source.txt","offset":0}));
                    m["tool_calls"][0]["function"]["arguments"] = json!("{bad");
                    m
                } else {
                    if case == 1 && step == 1 {
                        assert_eq!(previous["executed"], false);
                    }
                    let message = match step {
                        0 => call(&body, "read_file", json!({"path":"source.txt","offset":0})),
                        1 => call(
                            &body,
                            "create_working_copy",
                            json!({"source":"source.txt","name":"修訂.txt"}),
                        ),
                        2 => {
                            copy = previous["result"]["copy_id"].as_str().unwrap().into();
                            call(
                                &body,
                                "edit_text",
                                json!({"copy_id":copy,"revision":previous["result"]["revision"],"start":0,"expected":"原始","replacement":"修改"}),
                            )
                        }
                        3 => call(
                            &body,
                            "save_copy",
                            json!({"copy_id":copy,"revision":previous["result"]["revision"]}),
                        ),
                        _ => call(
                            &body,
                            "finish",
                            json!({"message":"已完成原生工具修訂","artifacts":[copy]}),
                        ),
                    };
                    step += 1;
                    message
                };
                if case == 7 {
                    let calls = message["tool_calls"].as_array_mut().unwrap();
                    calls.push(calls[0].clone());
                }
                let reason = if case == 3 {
                    "length"
                } else if body["model"] == "fast" {
                    "stop"
                } else {
                    "tool_calls"
                };
                let task_id = format!("task_{posts}");
                let mut completed = json!({"contract_version":"desktop-agent-v1","task_id":task_id,
                    "client_request_id":body["client_request_id"],"conversation_id":body["conversation_id"],"context":body["context"],
                    "state":"completed","result":{"id":format!("completion_{posts}"),"object":"chat.completion","created":1,"model":body["model"],
                    "choices":[{"index":0,"message":message,"finish_reason":reason}]},"error":null,"error_message":""});
                if case == 0 {
                    // 正式 HTTP 往返同時帶上游額外欄位，下一輪只能回送白名單訊息。
                    for pointer in [
                        "",
                        "/context",
                        "/result",
                        "/result/choices/0",
                        "/result/choices/0/message",
                        "/result/choices/0/message/tool_calls/0",
                        "/result/choices/0/message/tool_calls/0/function",
                    ] {
                        completed.pointer_mut(pointer).unwrap()["provider_specific_fields"] =
                            json!({"ignored":true});
                    }
                }
                if case == 10 {
                    completed["state"] = json!("running");
                    completed["result"] = Value::Null;
                    server_cancel.store(true, Ordering::Relaxed);
                }
                statuses.insert(
                    body["client_request_id"].as_str().unwrap().into(),
                    completed.clone(),
                );
                statuses.insert(task_id, completed.clone());
                if (case == 2 && posts == 1) || (case == 8 && body["model"] == "fast" && fast == 1)
                {
                    continue;
                }
                let mut accepted = completed;
                accepted["state"] = json!("queued");
                accepted["result"] = Value::Null;
                if case == 6 {
                    accepted["context"]["run_id"] = json!("another_run");
                }
                http = if case == 9 { 409 } else { 202 };
                if case == 9 {
                    json!({"error_code":"CAPABILITY_CHANGED","message":"設定已變更","task_accepted":false})
                } else {
                    accepted
                }
            } else if route.ends_with("/cancel") {
                server_cancel_count.fetch_add(1, Ordering::Relaxed);
                let task_id = route.split('/').nth_back(1).unwrap();
                let mut status = statuses[task_id].clone();
                status["state"] = json!("cancelled");
                status
            } else if route.contains("/tasks/") {
                assert_ne!(case, 9, "明確拒絕不可當成未知提交持續輪詢");
                if matches!(case, 2 | 8)
                    && !resumed_server.load(Ordering::Relaxed)
                    && (case == 2 || fast > 0)
                {
                    http = 404;
                    json!({"error_code":"TASK_NOT_FOUND","message":"暫未可查"})
                } else {
                    statuses
                        .get(route.rsplit('/').next().unwrap())
                        .cloned()
                        .ok_or("查詢未知 task")?
                }
            } else {
                return Err(format!("原生測試收到不應使用的路由：{route}"));
            };
            let bytes = response.to_string();
            write!(stream,"HTTP/1.1 {http} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{bytes}",bytes.len()).map_err(|e|e.to_string())?;
        }
        Ok((posts, fast))
    });
    let session = Session::from_token(
        TokenResponse {
            access_token: "fixture_token".into(),
            token_type: "Bearer".into(),
            expires_in: 3600,
        },
        &config,
    )?;
    let make_run = |resume| Run {
        id: id.clone(),
        resume,
        project: Project {
            id: id.clone(),
            name: "原生測試".into(),
            root: workspace.clone(),
            imports: BTreeMap::new(),
        },
        conversation: format!("conversation_{id}"),
        messages: vec![Message::user("請修訂來源並交付副本")],
        config: config.clone(),
        session: session.clone(),
        root: root.join("native-app"),
        cancel: cancel.clone(),
    };
    let mut activity = vec![];
    let mut result = runner::run(make_run(false), |s| activity.push(s));
    if matches!(case, 2 | 8) {
        assert!(result.as_ref().unwrap().contains("繼續"));
        assert!(runner::paused_available(&root.join("native-app"), &id));
        resumed.store(true, Ordering::Relaxed);
        result = runner::run(make_run(true), |s| activity.push(s));
    }
    stopped.store(true, Ordering::Relaxed);
    let (posts, fast) = server.join().map_err(|_| "原生測試 server 中斷")??;
    match case {
        0..=2 => {
            let answer = result?;
            assert!(answer.contains("已完成原生工具修訂"), "{answer}");
            assert_eq!(posts, if case == 1 { 6 } else { 5 });
            assert_eq!(
                activity
                    .iter()
                    .filter(|s| s.as_str() == "編輯文字：完成")
                    .count(),
                1
            );
            assert_eq!(
                activity
                    .iter()
                    .filter(|s| s.as_str() == "儲存副本：完成")
                    .count(),
                1
            );
        }
        3 => {
            assert!(result?.contains("繼續"));
            assert!(!workspace.join("_AI_Output").exists());
        }
        4 => {
            assert!(result.is_err());
            assert_eq!(posts, 0);
        }
        5 | 8 => {
            assert!(result?.contains("委派完成"));
            assert_eq!(fast, 1);
        }
        6 | 7 | 9 | 10 => {
            assert!(result.is_err());
            assert_eq!(posts, 1);
            assert!(!workspace.join("_AI_Output").exists());
        }
        _ => unreachable!(),
    }
    if case == 10 {
        assert_eq!(cancel_count.load(Ordering::Relaxed), 1);
    }
    assert_eq!(
        std::fs::read_to_string(workspace.join("source.txt")).map_err(|e| e.to_string())?,
        "原始文字"
    );
    println!(
        "PASS native agent case {case}: {posts} POST, {fast} delegated calls; original preserved."
    );
    Ok(())
}
