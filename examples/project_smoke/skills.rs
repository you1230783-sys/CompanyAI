//! 固定技能、搜尋、圖表與快速模型委派的真實 HTTP／DPAPI 整合測試。
use company_ai::{
    config::Config,
    projects::{
        files::Broker,
        runner::{self, Run},
        sandbox::Worker,
        Project, Tool,
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
pub fn verify(exe: &Path, root: &Path) -> AppResult<()> {
    let workspace = root.join("skills");
    std::fs::create_dir(&workspace).map_err(|e| e.to_string())?;
    std::fs::write(
        workspace.join("a.txt"),
        "研究速度 12 m/min。\n限制：只有一個樣本。",
    )
    .map_err(|e| e.to_string())?;
    let project = Project {
        id: "skills".into(),
        name: "測試".into(),
        root: workspace.clone(),
        imports: BTreeMap::new(),
    };
    let cancel = AtomicBool::new(false);
    let mut worker = Worker::start(exe, &cancel)?;
    let mut broker = Broker::new(project, "skills_run".into())?;
    broker.enable_memory("skills_chat")?;
    let tool = Tool::SearchFiles {
        paths: vec![
            "a.txt".into(),
            ".lmai/private.txt".into(),
            "../outside.txt".into(),
        ],
        query: "12".into(),
    };
    let found = broker.execute("search", &tool, &mut worker, &cancel)?;
    assert_eq!(found["result"]["matches"].as_array().unwrap().len(), 1);
    assert_eq!(found["result"]["errors"].as_array().unwrap().len(), 2);
    assert_eq!(found["result"]["complete"], false);
    let loaded = broker.execute(
        "skill",
        &Tool::LoadSkill {
            id: "paper-evidence".into(),
        },
        &mut worker,
        &cancel,
    )?;
    assert_eq!(
        loaded["result"]["loaded"],
        json!(["research", "notes", "paper-evidence"])
    );
    assert!(
        loaded["result"].get("instructions").is_none(),
        "工具結果不重複 system 說明"
    );
    assert_eq!(
        broker.execute(
            "bad_skill",
            &Tool::LoadSkill {
                id: "../secret".into()
            },
            &mut worker,
            &cancel
        )?["ok"],
        false
    );
    let chart=Tool::CreateChart{chart:serde_json::from_value(json!({"kind":"bar","title":"測試","x_label":"s","y_label":"V","x":[1,2],"series":[{"name":"A","values":[4,null]}],"source":"測試"})).unwrap()};
    broker.execute("chart", &chart, &mut worker, &cancel)?;
    broker.execute("chart", &chart, &mut worker, &cancel)?;
    assert_eq!(broker.charts().len(), 1, "去重不增加圖表");
    drop(worker);
    for pending in [false, true] {
        delegation(root, pending)?;
    }
    println!("PASS skills: restricted search, built-in loading, charts, delegated fast-only summaries, cache, pause and no repost.");
    Ok(())
}
fn delegation(root: &Path, pending: bool) -> AppResult<()> {
    let id = format!("delegate_{pending}");
    let workspace = root.join(&id);
    std::fs::create_dir(&workspace).map_err(|e| e.to_string())?;
    std::fs::write(
        workspace.join("paper.txt"),
        "研究速度 12 m/min。限制：只有一個樣本。"
            .repeat(500)
            .chars()
            .take(9000)
            .collect::<String>(),
    )
    .map_err(|e| e.to_string())?;
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
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = stop.clone();
    let resumed = Arc::new(AtomicBool::new(false));
    let ready = resumed.clone();
    let server = std::thread::spawn(move || -> AppResult<(usize, usize)> {
        let mut parent = 0;
        let mut child = 0;
        let mut last = Value::Null;
        while !stopped.load(Ordering::Relaxed) {
            let (mut stream, _) = match listener.accept() {
                Ok(v) => v,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(e) => return Err(e.to_string()),
            };
            let (route, body) = super::roundtrip::request(&mut stream)?;
            let response = if route.contains("capabilities") {
                json!({"contract_version":1,"principal_id":"skills_owner","execution_modes":["background"],"attachments":{"enabled":false,"max_count":0,"max_file_bytes":0,"max_total_bytes":0,"allowed_extensions":[]}})
            } else if route.ends_with("/models") {
                json!({"models":[{"id":"quality","label":"品質"},{"id":"fast","label":"快速"}]})
            } else if route.ends_with("/conversations") {
                json!({"conversation_id":format!("conversation_{}",company_ai::jobs::new_id()?)})
            } else if route.ends_with("/chat/completions") {
                assert_eq!(body["skills"], false);
                assert!(body.get("tools").is_none());
                let content = if body["model"] == "fast" {
                    child += 1;
                    assert_eq!(body["messages"].as_array().unwrap().len(), 2);
                    assert!(!body["messages"][0]["content"]
                        .as_str()
                        .unwrap()
                        .contains("\"parameters\""));
                    assert!(body["messages"][1]["content"]
                        .as_str()
                        .unwrap()
                        .contains("12 m/min"));
                    "局部研究速度為 12 m/min，只有一個樣本；原文：研究速度 12 m/min。".to_string()
                } else {
                    let decision = match parent {
                        0 => {
                            json!({"action":"tool","operation_id":"load","request":{"tool":"load_skill","id":"paper-evidence"}})
                        }
                        1 | 2 => {
                            assert!(body["messages"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .any(|m| m["role"] == "system"
                                    && m["content"]
                                        .as_str()
                                        .unwrap_or("")
                                        .contains("# 論文閱讀與證據整理")));
                            if parent == 2 {
                                assert!(body.to_string().contains("快速模型摘要，主模型尚未核實"));
                            }
                            json!({"action":"tool","operation_id":format!("summary_{parent}"),"request":{"tool":"summarize_document","path":"paper.txt","focus":"速度與限制"}})
                        }
                        _ => {
                            json!({"action":"finish","message":"已取得快速模型摘要，數字待原文核實。","artifacts":[]})
                        }
                    };
                    parent += 1;
                    decision.to_string()
                };
                let response = json!({"task_id":format!("task_{}",body["client_request_id"].as_str().unwrap()),"client_request_id":body["client_request_id"],"state":"completed","result":{"choices":[{"message":{"content":content}}]}});
                if body["model"] == "fast" && pending && child == 2 {
                    last = response.clone();
                    let mut running = response;
                    running["state"] = json!("running");
                    running["result"] = Value::Null;
                    running
                } else {
                    response
                }
            } else if route.contains("/tasks/") {
                assert!(route.ends_with(last["task_id"].as_str().unwrap()));
                let mut response = last.clone();
                if !ready.load(Ordering::Relaxed) {
                    response["state"] = json!("running");
                    response["result"] = Value::Null;
                }
                response
            } else {
                return Err(format!("未知路由 {route}"));
            };
            let response = response.to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",response.len()).map_err(|e|e.to_string())?;
        }
        Ok((parent, child))
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
        id: id.clone(),
        resume,
        project: Project {
            id: id.clone(),
            name: "摘要測試".into(),
            root: workspace.clone(),
            imports: BTreeMap::new(),
        },
        conversation: id.clone(),
        messages: vec![Message::user("摘要論文")],
        config: config.clone(),
        session: session.clone(),
        root: root.join("app-data"),
        cancel: Arc::new(AtomicBool::new(false)),
    };
    let result = (|| -> AppResult<()> {
        if pending {
            let text =
                runner::run_legacy_with_test_budget(run(false), |_| {}, Duration::from_secs(2))?;
            assert!(text.contains("請按「繼續」"), "{text}");
            resumed.store(true, Ordering::Relaxed);
            assert!(runner::run_legacy_test(run(true), |_| {})?.contains("已取得快速模型摘要"));
        } else {
            assert!(runner::run_legacy_test(run(false), |_| {})?.contains("已取得快速模型摘要"));
        }
        Ok(())
    })();
    stop.store(true, Ordering::Relaxed);
    let (parent, child) = server.join().map_err(|_| "測試服務失敗。")??;
    result?;
    assert_eq!(parent, 4);
    assert_eq!(child, 3, "相同內容快取與續接不得再 POST 快速模型");
    Ok(())
}
