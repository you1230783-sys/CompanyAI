//! 真實 HTTP／DPAPI／工具流程：暫時故障、重試耗盡與重啟皆不重做已成功的修改。
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
    collections::{BTreeMap, BTreeSet},
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
    // 預設入口使用同一實作，測試目錄限定本次 fixture；不在真正使用者 profile 留檔。
    let profile = root.join("profile-fixture");
    std::fs::create_dir(&profile).map_err(|e| e.to_string())?;
    let a = company_ai::projects::setup::create_in_profile(&profile)?;
    std::fs::write(a.join("keep.txt"), "保留").map_err(|e| e.to_string())?;
    let b = company_ai::projects::setup::create_in_profile(&profile)?;
    assert_eq!(a.parent(), Some(profile.join("LM_AI_Projects").as_path()));
    assert_eq!(
        b.file_name().unwrap().to_string_lossy(),
        format!("{}_2", a.file_name().unwrap().to_string_lossy())
    );
    assert_eq!(std::fs::read_to_string(a.join("keep.txt")).unwrap(), "保留");
    assert!(company_ai::projects::setup::create_default("desktop").is_err());
    for case in 0..9 {
        verify_case(root, case)?;
    }
    Ok(())
}

fn verify_case(root: &Path, case: usize) -> AppResult<()> {
    let id = format!("resilience{case}");
    let workspace = root.join(&id);
    std::fs::create_dir(&workspace).map_err(|e| e.to_string())?;
    std::fs::write(workspace.join("source.txt"), "原始文字").map_err(|e| e.to_string())?;
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let config = Config {
        server_url: format!("http://{}", listener.local_addr().unwrap()),
        model: "quality".into(),
        ..Config::default()
    };
    let stop = Arc::new(AtomicBool::new(false));
    let ready = Arc::new(AtomicBool::new(false));
    let cancelled = Arc::new(AtomicBool::new(false));
    let stopped = stop.clone();
    let resumed = ready.clone();
    let server = std::thread::spawn(move || -> AppResult<(usize, usize, usize)> {
        let (mut posts, mut failures, mut step) = (0, 0, 0);
        let mut cancels = 0;
        let mut statuses = BTreeMap::<String, Value>::new();
        let mut ids = BTreeSet::new();
        let mut turns = BTreeSet::new();
        let mut copy = String::new();
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
            let mut http = 200;
            let response = if route.contains("/agent/capabilities") {
                super::native::capability("quality", true)
            } else if route.contains("/capabilities") {
                json!({"contract_version":1,"principal_id":"fixture_owner","execution_modes":["background"],"attachments":{"enabled":false,"max_count":0,"max_file_bytes":0,"max_total_bytes":0,"allowed_extensions":[]}})
            } else if route.ends_with("conversations") {
                json!({"conversation_id":"resilience_remote"})
            } else if route.ends_with("agent/turns") {
                posts += 1;
                assert!(ids.insert(body["client_request_id"].as_str().unwrap().to_owned()));
                assert!(turns.insert(body["context"]["turn_index"].as_u64().unwrap()));
                let previous = super::native::last_result(&body);
                let failing = step == 4
                    && match case {
                        0 | 3 => failures < 2,
                        1 => !resumed.load(Ordering::Relaxed),
                        2 | 5 | 6 => true,
                        _ => false,
                    };
                let mut status = json!({"contract_version":"desktop-agent-v1","task_id":format!("t{posts}"),
                    "client_request_id":body["client_request_id"],"conversation_id":body["conversation_id"],"context":body["context"],
                    "state":"completed","error":null,"error_message":""});
                if failing {
                    failures += 1;
                    status["state"] = json!("failed");
                    status["result"] = Value::Null;
                    status["error"] = json!({"error_code":match case {2=>"QUOTA_EXCEEDED",6=>"UPSTREAM_RESULT_UNKNOWN",_=>"AI_BACKEND_ERROR"},"message":"fixture upstream failure","retryable":false,"details":{}});
                } else {
                    let (name, args) = match step {
                        0 => ("load_skill", json!({"id":"text-edit"})),
                        1 => ("read_file", json!({"path":"source.txt","offset":0})),
                        2 => (
                            "create_working_copy",
                            json!({"source":"source.txt","name":"retry-output.txt"}),
                        ),
                        3 => {
                            copy = previous["result"]["copy_id"].as_str().unwrap().into();
                            (
                                "edit_text",
                                json!({"copy_id":copy,"revision":previous["result"]["revision"],"start":0,"expected":"原始","replacement":"修改"}),
                            )
                        }
                        4 => (
                            "save_copy",
                            json!({"copy_id":copy,"revision":previous["result"]["revision"]}),
                        ),
                        _ => ("finish", json!({"message":"恢復後完成","artifacts":[copy]})),
                    };
                    status["result"] = json!({"id":format!("c{posts}"),"object":"chat.completion","created":1,"model":"quality",
                        "choices":[{"index":0,"finish_reason":"tool_calls","message":super::native::call(&body,name,args)}]});
                    step += 1;
                }
                if case == 8 {
                    status["state"] = json!("queued");
                    status["result"] = Value::Null;
                }
                statuses.insert(
                    body["client_request_id"].as_str().unwrap().into(),
                    status.clone(),
                );
                statuses.insert(format!("t{posts}"), status.clone());
                if matches!(case, 4 | 7) && posts == 1 {
                    continue;
                } // 已受理但 POST 回覆遺失；不得換 ID 重送。
                status
            } else if route.ends_with("/cancel") {
                cancels += 1;
                json!({"ok":true})
            } else if route.contains("/tasks/") {
                if case == 8 {
                    http = 503;
                    json!({"error_code":"UPSTREAM_UNAVAILABLE"})
                } else if case == 7 {
                    http = 401;
                    json!({"error_code":"UNAUTHORIZED"})
                } else if case == 4 && !resumed.load(Ordering::Relaxed) {
                    http = 404;
                    json!({"error_code":"TASK_NOT_FOUND"})
                } else {
                    statuses
                        .get(route.rsplit('/').next().unwrap())
                        .cloned()
                        .ok_or("unexpected lookup")?
                }
            } else {
                return Err(format!("unexpected route {route}"));
            };
            let bytes = response.to_string();
            write!(stream,"HTTP/1.1 {http} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{bytes}",bytes.len()).map_err(|e|e.to_string())?;
        }
        Ok((posts, failures, cancels))
    });
    let session = Session::from_token(
        TokenResponse {
            access_token: "fixture_token".into(),
            token_type: "Bearer".into(),
            expires_in: 3600,
        },
        &config,
    )?;
    let make = |resume| Run {
        id: id.clone(),
        resume,
        project: Project {
            id: id.clone(),
            name: "恢復測試".into(),
            root: workspace.clone(),
            imports: BTreeMap::new(),
        },
        conversation: id.clone(),
        messages: vec![Message::user("只修改副本一次並保存")],
        config: config.clone(),
        session: session.clone(),
        root: root.join("resilience-app"),
        cancel: cancelled.clone(),
        instructions: None,
        outlook_consent: None,
        file_waiter: None,
    };
    let mut activity = vec![];
    let mut result = if case == 3 {
        let stopped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            runner::run(make(false), |s| {
                if s.contains("AI 連線恢復") {
                    panic!("fixture: restart during retry wait");
                }
                activity.push(s);
            })
        }));
        assert!(stopped.is_err());
        assert!(runner::paused_available(&root.join("resilience-app"), &id));
        runner::run(make(true), |s| activity.push(s))
    } else {
        runner::run(make(false), |s| {
            if matches!(case, 5 | 8) && s.contains("AI 連線恢復") {
                cancelled.store(true, Ordering::Relaxed);
            }
            activity.push(s);
        })
    };
    if matches!(case, 1 | 4) {
        assert!(result.as_ref().unwrap().contains("繼續"), "{result:?}");
        assert!(runner::paused_available(&root.join("resilience-app"), &id));
        ready.store(true, Ordering::Relaxed);
        result = runner::run(make(true), |s| activity.push(s));
    }
    stop.store(true, Ordering::Relaxed);
    let (posts, failures, cancels) = server.join().map_err(|_| "retry server failed")??;
    if case == 8 {
        assert!(result.is_err());
        assert_eq!((posts, failures, cancels), (1, 0, 1));
    } else if case == 7 {
        assert!(result?.contains("未自動重試"));
        assert_eq!((posts, failures), (1, 0));
        assert!(activity.iter().all(|s| !s.contains("AI 連線恢復")));
        assert!(runner::paused_available(&root.join("resilience-app"), &id));
    } else if matches!(case, 2 | 5 | 6) {
        assert!(result.is_err());
        assert_eq!(failures, 1);
    } else {
        assert!(result?.contains("恢復後完成"));
        assert_eq!(posts, 6 + failures);
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
    if case == 1 {
        assert_eq!(failures, 6);
    }
    assert_eq!(
        std::fs::read_to_string(workspace.join("source.txt")).unwrap(),
        "原始文字"
    );
    println!("PASS resilience {case}: {posts} POST, {failures} confirmed failures, no repeated local edit; accelerated wait clock.");
    Ok(())
}
