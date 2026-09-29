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

fn request(stream: &mut TcpStream) -> AppResult<(String, Value)> {
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
    let workspace = root.join("roundtrip");
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
                assert_eq!(body["model"], "quality");
                let previous: Value = if rounds >= 1 {
                    let content = body["messages"]
                        .as_array()
                        .and_then(|m| m.last())
                        .and_then(|m| m["content"].as_str())
                        .ok_or("缺少工具結果。")?;
                    serde_json::from_str(content.split_once('\n').ok_or("工具結果格式錯誤。")?.1)
                        .map_err(|e| e.to_string())?
                } else {
                    json!({})
                };
                if rounds >= 1 {
                    assert_eq!(previous["ok"], true, "{previous}");
                }
                let decision = match rounds {
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
                        json!({"action":"finish","message":"本機測試已完成修訂。","artifacts":[copy_id]})
                    }
                    _ => return Err("不應出現額外模型請求。".into()),
                };
                rounds += 1;
                last_status = json!({"task_id":format!("task_{rounds}"),"client_request_id":body["client_request_id"],"state":"completed","result":{"choices":[{"message":{"role":"assistant","content":decision.to_string()}}]}});
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
            id: "roundtrip".into(),
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
    let answer = result?;
    assert_eq!(
        runner::recover(&root.join("app-data"), "roundtrip")?,
        answer
    );
    assert_eq!(rounds, 5);
    assert!(answer.contains("本機測試已完成修訂"));
    assert_eq!(
        std::fs::read_to_string(workspace.join("source.txt")).map_err(|e| e.to_string())?,
        "原始文字"
    );
    println!("PASS: five-round HTTP/skills/tool loop, verified artifact and original preserved.");
    Ok(())
}
