//! 合成模型只決定固定步驟；HTTP、原生唯讀閘門、DPAPI 續接與檔案成果使用正式路徑。
use company_ai::{
    config::Config,
    projects::{
        runner::{self, Run},
        task_plan::{Output, Plan, Step},
        Project,
    },
    protocol::{Message, TokenResponse},
    storage::Session,
    AppResult,
};
use serde_json::json;
use std::{
    io::Write,
    net::TcpListener,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

fn plan() -> Plan {
    Plan {
        goal: "先確認再建立文字成果".into(),
        steps: vec![Step {
            method: "建立受控文字副本".into(),
            tools: vec![
                "create_working_copy".into(),
                "edit_text".into(),
                "save_copy".into(),
            ],
            parameters: "寫入核准內容".into(),
        }],
        sources: vec![],
        outputs: vec![Output {
            id: "report".into(),
            description: "核准的文字成果".into(),
            kind: "file".into(),
        }],
        assumptions: vec![],
        questions: vec![],
    }
}
pub fn verify(root: &Path) -> AppResult<()> {
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).map_err(|e| e.to_string())?;
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
    let server_stop = stop.clone();
    let server = std::thread::spawn(move || -> AppResult<usize> {
        let mut step = 0;
        let mut copy = String::new();
        let mut ids = std::collections::BTreeSet::new();
        while !server_stop.load(Ordering::Relaxed) {
            let (mut stream, _) = match listener.accept() {
                Ok(pair) => pair,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(e) => return Err(e.to_string()),
            };
            let (route, body) = super::roundtrip::request(&mut stream)?;
            let response = if route.contains("/agent/capabilities") {
                super::native::capability("quality", true)
            } else if route.contains("/capabilities") {
                json!({"contract_version":1,"principal_id":"fixture_owner","execution_modes":["background"],"attachments":{"enabled":false,"max_count":0,"max_file_bytes":0,"max_total_bytes":0,"allowed_extensions":[]}})
            } else if route.ends_with("/conversations") {
                json!({"conversation_id":"remote_plan"})
            } else if route.ends_with("/models") {
                json!({"models":[{"id":"quality","label":"品質"}],"default_model":"quality"})
            } else if route.ends_with("/agent/turns") {
                assert!(
                    ids.insert(body["client_request_id"].as_str().unwrap().to_owned()),
                    "續接必須使用新請求，不重播已完成模型回合"
                );
                let offered = |name: &str| {
                    body["tools"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|t| t["function"]["name"] == name)
                };
                let previous = super::native::last_result(&body);
                let message = match step {
                    0 => {
                        assert!(!offered("create_working_copy"));
                        json!({"role":"assistant","content":"","tool_calls":[{"id":"call_001","type":"function","function":{"name":"create_working_copy","arguments":"{\"source\":null,\"name\":\"forbidden.txt\"}"}}]})
                    }
                    1 => super::native::call(&body, "load_skill", json!({"id":"text-edit"})),
                    2 => {
                        assert!(!offered("create_working_copy"));
                        super::native::call(&body, "propose_plan", json!({"plan":plan()}))
                    }
                    3 => {
                        assert!(body["messages"].to_string().contains("退回後重新規劃"));
                        super::native::call(&body, "propose_plan", json!({"plan":plan()}))
                    }
                    4 => {
                        assert!(body["messages"].to_string().contains("使用者編輯的目標"));
                        assert!(offered("create_working_copy"));
                        super::native::call(
                            &body,
                            "create_working_copy",
                            json!({"source":null,"name":"plan.txt"}),
                        )
                    }
                    5 => {
                        copy = previous["result"]["copy_id"].as_str().unwrap().into();
                        let revision = previous["result"]["revision"].as_str().unwrap();
                        super::native::call(
                            &body,
                            "edit_text",
                            json!({"copy_id":copy,"revision":revision,"start":0,"expected":"","replacement":"已核准的成果"}),
                        )
                    }
                    6 => {
                        let revision = previous["result"]["revision"].as_str().unwrap();
                        super::native::call(
                            &body,
                            "save_copy",
                            json!({"copy_id":copy,"revision":revision}),
                        )
                    }
                    7 => super::native::call(
                        &body,
                        "bind_deliverable",
                        json!({"bindings":[{"id":"report","target":copy}]}),
                    ),
                    8 => {
                        let mut revised = plan();
                        revised.goal = "使用者編輯的目標".into();
                        revised.steps[0].method = "新增成果檢閱方法".into();
                        super::native::call(
                            &body,
                            "revise_plan",
                            json!({"plan":revised,"reason":"主要方法改變"}),
                        )
                    }
                    9 => {
                        assert!(body["messages"].to_string().contains("新增成果檢閱方法"));
                        super::native::call(
                            &body,
                            "finish",
                            json!({"message":"計畫驗證完成","artifacts":[copy]}),
                        )
                    }
                    _ => return Err(format!("計畫測試出現非預期額外模型回合 {step}：{previous}")),
                };
                step += 1;
                json!({"contract_version":"desktop-agent-v1","task_id":format!("plan_{step}"),"client_request_id":body["client_request_id"],"conversation_id":body["conversation_id"],"context":body["context"],"state":"completed","result":{"id":format!("completion_{step}"),"object":"chat.completion","created":1,"model":"quality","choices":[{"index":0,"message":message,"finish_reason":"tool_calls"}]},"error":null,"error_message":""})
            } else {
                return Err(format!("不應呼叫的路由：{route}"));
            };
            let bytes = response.to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{bytes}",bytes.len()).map_err(|e|e.to_string())?;
        }
        Ok(step)
    });
    let session = Session::from_token(
        TokenResponse {
            access_token: "fixture_token".into(),
            token_type: "Bearer".into(),
            expires_in: 3600,
        },
        &config,
    )?;
    let project = Project {
        id: "plan_project".into(),
        name: "計畫驗證".into(),
        root: workspace.clone(),
        imports: Default::default(),
    };
    let app = root.join("app");
    let cancel = Arc::new(AtomicBool::new(false));
    let run = |resume: bool| Run {
        plan_mode: !resume,
        id: "plan_run".into(),
        resume,
        project: project.clone(),
        conversation: "plan_chat".into(),
        messages: vec![Message::user("先提出計畫，核准後建立文字成果")],
        config: config.clone(),
        session: session.clone(),
        root: app.clone(),
        cancel: cancel.clone(),
        instructions: None,
        outlook_consent: None,
        cad_waiter: None,
        cad_progress: None,
        file_waiter: None,
    };
    let checked = (|| -> AppResult<()> {
        let mut activity = Vec::new();
        let result = runner::run(run(false), |a| activity.push(a))?;
        assert!(result.contains("計畫等待"));
        assert!(activity.iter().any(|a| a.contains("Error Code: 14")));
        assert!(!workspace.join("forbidden.txt").exists());
        let pending = runner::pending_plan(&app, "plan_run").unwrap();
        assert_eq!(pending.history.len(), 1);
        let journal = app.join("project-runs/plan_run.dpapi");
        let before = std::fs::read(&journal).map_err(|e| e.to_string())?;
        assert!(runner::run(run(true), |_| {})
            .unwrap_err()
            .contains("等待核准"));
        assert_eq!(std::fs::read(&journal).map_err(|e| e.to_string())?, before);

        runner::review_plan(
            &app,
            "plan_run",
            &project,
            "plan_chat",
            "fixture_owner",
            pending.pending.unwrap().revision,
            None,
            "退回後重新規劃",
        )?;
        assert!(runner::run(run(true), |_| {})?.contains("計畫等待"));
        let pending = runner::pending_plan(&app, "plan_run").unwrap();
        let mut edited = plan();
        edited.goal = "使用者編輯的目標".into();
        runner::review_plan(
            &app,
            "plan_run",
            &project,
            "plan_chat",
            "fixture_owner",
            pending.pending.unwrap().revision,
            Some(edited),
            "",
        )?;
        assert!(runner::run(run(true), |_| {})?.contains("計畫等待"));
        let pending = runner::pending_plan(&app, "plan_run").unwrap();
        assert!(pending.read_only);
        let revision = pending.pending.unwrap();
        runner::review_plan(
            &app,
            "plan_run",
            &project,
            "plan_chat",
            "fixture_owner",
            revision.revision,
            Some(revision.plan),
            "",
        )?;
        assert!(runner::run(run(true), |_| {})?.contains("計畫驗證完成"));
        let overview = runner::recover_overview(&app, "plan_run").unwrap();
        assert_eq!(overview.outputs.len(), 1, "保存成果不可因續接重做");
        assert_eq!(
            std::fs::read_to_string(workspace.join(&overview.outputs[0]))
                .map_err(|e| e.to_string())?
                .trim_start_matches('\u{feff}'),
            "已核准的成果"
        );
        assert_eq!(overview.task_plan.unwrap().history.len(), 6);
        Ok(())
    })();
    stop.store(true, Ordering::Relaxed);
    let count = server.join().map_err(|_| "計畫測試服務異常。")??;
    checked?;
    assert_eq!(count, 10);
    println!("PASS M9 native HTTP: preapproval Code14, loaded-skill write denial, rejection, edited approval, exact delivery, major revision pause, encrypted resume and single preserved artifact; 10 unique model turns.");
    Ok(())
}
