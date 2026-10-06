//! 真實 loopback HTTP 驗證圖片 JSON、來源 bytes、快取及未知提交續接。
//! 回覆由測試伺服器固定產生；這不是公司模型的視覺品質驗收。
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
    for case in 0..5 {
        verify_case(root, case)?;
    }
    Ok(())
}

fn verify_case(root: &Path, case: usize) -> AppResult<()> {
    let id = format!("vision_{case}");
    let workspace = root.join(&id);
    std::fs::create_dir(&workspace).map_err(|e| e.to_string())?;
    let jpeg = case == 1;
    let bytes: &[u8] = if jpeg {
        include_bytes!("../fixtures/vision.jpg")
    } else {
        include_bytes!("../fixtures/vision.png")
    };
    let name = if jpeg { "sample.jpg" } else { "sample.png" };
    std::fs::write(workspace.join(name), bytes).map_err(|e| e.to_string())?;
    if case == 4 {
        std::fs::write(workspace.join(name), vec![0; 1024 * 1024 + 1])
            .map_err(|e| e.to_string())?;
    }
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
    let ready = resumed.clone();
    let server = std::thread::spawn(move || -> AppResult<(usize, usize)> {
        let (mut parents, mut children) = (0, 0);
        let mut statuses = BTreeMap::<String, Value>::new();
        let mut child_id = String::new();
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
                super::native::capability("quality", true)
            } else if route.contains("/capabilities") {
                json!({"contract_version":1,"principal_id":"fixture_owner","execution_modes":["background"],"attachments":{"enabled":false,"max_count":0,"max_file_bytes":0,"max_total_bytes":0,"allowed_extensions":[]}})
            } else if route.ends_with("/conversations") {
                json!({"conversation_id":"fixture_conversation"})
            } else if route.ends_with("/agent/turns") {
                assert_eq!(body["model"], "quality");
                assert_eq!(body["skills"], false);
                let child = body["tool_choice"] == "none";
                let message = if child {
                    children += 1;
                    assert!(case < 3, "無效來源不得送圖");
                    assert_eq!(body["tools"], json!([]));
                    assert!(body["context"]["parent_request_id"].is_string());
                    let content = &body["messages"][1]["content"];
                    assert_eq!(content.as_array().unwrap().len(), 2);
                    assert_eq!(content[0]["type"], "text");
                    assert_eq!(content[1]["type"], "image_url");
                    let url = content[1]["image_url"]["url"].as_str().unwrap();
                    let (prefix, encoded) = url.split_once(',').unwrap();
                    assert_eq!(
                        prefix,
                        if jpeg {
                            "data:image/jpeg;base64"
                        } else {
                            "data:image/png;base64"
                        }
                    );
                    // Windows 的反向解碼驗證完整來源 bytes，而不只驗證前綴。
                    use windows_sys::Win32::Security::Cryptography::*;
                    let mut decoded = vec![0u8; encoded.len()];
                    let mut size = decoded.len() as u32;
                    assert_ne!(
                        unsafe {
                            CryptStringToBinaryA(
                                encoded.as_ptr(),
                                encoded.len() as u32,
                                CRYPT_STRING_BASE64,
                                decoded.as_mut_ptr(),
                                &mut size,
                                std::ptr::null_mut(),
                                std::ptr::null_mut(),
                            )
                        },
                        0
                    );
                    assert_eq!(&decoded[..size as usize], bytes);
                    child_id = body["client_request_id"].as_str().unwrap().into();
                    json!({"role":"assistant","content":"測試回覆：左紅右藍，未辨識文字。"})
                } else {
                    parents += 1;
                    assert!(
                        !body.to_string().contains("data:image/"),
                        "主上下文不得重複攜帶圖片"
                    );
                    match parents {
                        1 => super::native::call(&body, "load_skill", json!({"id":"image-read"})),
                        2 | 3 => {
                            if parents == 3 {
                                let result = super::native::last_result(&body);
                                assert_eq!(result["ok"], case < 3, "{result}");
                                if case < 3 {
                                    assert!(result["result"]["image"]["sha256"]
                                        .as_str()
                                        .is_some_and(|s| s.len() == 64));
                                }
                            }
                            if parents == 3 && case >= 3 {
                                super::native::call(
                                    &body,
                                    "finish",
                                    json!({"message":"圖片測試完成","artifacts":[]}),
                                )
                            } else {
                                super::native::call(
                                    &body,
                                    "analyze_image",
                                    json!({"path":if case==3 {"../outside.png"} else {name},"focus":"描述顏色"}),
                                )
                            }
                        }
                        _ => super::native::call(
                            &body,
                            "finish",
                            json!({"message":"圖片測試完成","artifacts":[]}),
                        ),
                    }
                };
                let request = body["client_request_id"].as_str().unwrap();
                let completed = json!({"contract_version":"desktop-agent-v1","principal_id":"fixture_owner","task_id":format!("task_{request}"),"client_request_id":request,"conversation_id":body["conversation_id"],"context":body["context"],"state":"completed","result":{"id":"fixture_completion","object":"chat.completion","created":1,"model":"quality","choices":[{"index":0,"finish_reason":if child {"stop"} else {"tool_calls"},"message":message}]}});
                statuses.insert(request.into(), completed.clone());
                if case == 2 && child {
                    continue;
                } // POST 已收妥，但回覆遺失。
                completed
            } else if route.contains("/tasks/") {
                let request = route.rsplit('/').next().unwrap();
                if case == 2 && request == child_id && !ready.load(Ordering::Relaxed) {
                    http = 404;
                    json!({"error_code":"TASK_NOT_FOUND","message":"暫未可查"})
                } else {
                    statuses.get(request).cloned().ok_or("非原請求查回")?
                }
            } else {
                return Err(format!("非预期圖片路由：{route}"));
            };
            let data = response.to_string();
            write!(stream,"HTTP/1.1 {http} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{data}",data.len()).map_err(|e|e.to_string())?;
        }
        Ok((parents, children))
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
            name: "圖片試驗".into(),
            root: workspace.clone(),
            imports: BTreeMap::new(),
        },
        conversation: format!("chat_{id}"),
        messages: vec![Message::user("描述專案圖片")],
        config: config.clone(),
        session: session.clone(),
        root: root.join("vision-app"),
        cancel: Arc::new(AtomicBool::new(false)),
        instructions: None,
        outlook_consent: None,
        file_waiter: None,
    };
    let mut result = runner::run(run(false), |_| {});
    if case == 2 && result.is_ok() {
        assert!(
            runner::paused_available(&root.join("vision-app"), &id),
            "{result:?}"
        );
        resumed.store(true, Ordering::Relaxed);
        result = runner::run(run(true), |_| {});
    }
    stopped.store(true, Ordering::Relaxed);
    let (parents, children) = server.join().map_err(|_| "圖片測試伺服器中斷")??;
    assert!(result?.contains("圖片測試完成"));
    assert_eq!(children, usize::from(case < 3), "快取與續接不得重送圖片");
    assert_eq!(parents, if case < 3 { 4 } else { 3 });
    println!("PASS vision case {case}: {parents} parent POST, {children} image POST; exact bytes, cache and request identity checked.");
    Ok(())
}
