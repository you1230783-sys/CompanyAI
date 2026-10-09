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
    for case in 0..13 {
        verify_case(root, case)?;
    }
    Ok(())
}

fn verify_case(root: &Path, case: usize) -> AppResult<()> {
    let id = format!("vision_{case}");
    let workspace = root.join(&id);
    std::fs::create_dir(&workspace).map_err(|e| e.to_string())?;
    let jpeg = matches!(case, 1 | 5);
    let bytes: &[u8] = if jpeg {
        include_bytes!("../fixtures/vision.jpg")
    } else {
        include_bytes!("../fixtures/vision.png")
    };
    let bytes = if case == 5 {
        jpeg_with_comments(bytes, 6_000_000)
    } else {
        bytes.to_vec()
    };
    let name = if jpeg { "sample.jpg" } else { "sample.png" };
    std::fs::write(workspace.join(name), &bytes).map_err(|e| e.to_string())?;
    if matches!(case, 8 | 9 | 12) {
        std::fs::write(workspace.join("second.png"), &bytes).map_err(|e| e.to_string())?;
    }
    if case == 4 {
        std::fs::write(workspace.join(name), vec![0; 5_000_001]).map_err(|e| e.to_string())?;
    }
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let config = Config {
        server_url: format!(
            "http://{}",
            listener.local_addr().map_err(|e| e.to_string())?
        ),
        model: if case == 6 { "fast" } else { "quality" }.into(),
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
                let mut caps =
                    super::native::capability(if case == 6 { "fast" } else { "quality" }, true);
                caps["limits"]["request_bytes"] = json!(10_000_000);
                caps
            } else if route.contains("/capabilities") {
                json!({"contract_version":1,"principal_id":"fixture_owner","execution_modes":["background"],"attachments":{"enabled":false,"max_count":0,"max_file_bytes":0,"max_total_bytes":0,"allowed_extensions":[]}})
            } else if route.ends_with("/conversations") {
                json!({"conversation_id":format!("remote_{}", body["client_conversation_id"].as_str().unwrap())})
            } else if route.ends_with("/agent/turns") {
                assert_eq!(body["model"], if case == 6 { "fast" } else { "quality" });
                assert_eq!(body["skills"], false);
                if let Some(parent) = body["context"]["parent_request_id"].as_str() {
                    let accepted = statuses.get(parent).expect("父請求必須已受理");
                    assert_eq!(accepted["principal_id"], "fixture_owner");
                    assert_eq!(
                        accepted["conversation_id"], body["conversation_id"],
                        "父子請求必須同一對話"
                    );
                    for key in ["project_id", "run_id"] {
                        assert_eq!(accepted["context"][key], body["context"][key]);
                    }
                }
                let child = body["tool_choice"] == "none";
                let message = if child {
                    children += 1;
                    assert!(
                        case < 3 || matches!(case, 5 | 7 | 8 | 9 | 10 | 12),
                        "無效來源與快速模型不得送圖"
                    );
                    if case == 7 {
                        let error = json!({"error_code":"INVALID_REQUEST","task_accepted":false,"message":"fixture: 明確拒絕圖片"}).to_string();
                        write!(stream,"HTTP/1.1 422 Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{error}",error.len()).map_err(|e|e.to_string())?;
                        continue;
                    }
                    assert_eq!(body["tools"], json!([]));
                    assert!(body["context"]["parent_request_id"].is_string());
                    let content = &body["messages"][1]["content"];
                    assert_eq!(
                        content.as_array().unwrap().len(),
                        if matches!(case, 9 | 12) { 3 } else { 2 }
                    );
                    assert_eq!(content[0]["type"], "text");
                    assert_eq!(content[1]["type"], "image_url");
                    let url = content[1]["image_url"]["url"].as_str().unwrap();
                    let (prefix, encoded) = url.split_once(',').unwrap();
                    assert_eq!(prefix, "data:image/jpeg;base64");
                    if matches!(case, 9 | 12) {
                        assert!(content[2]["image_url"]["url"]
                            .as_str()
                            .unwrap()
                            .starts_with("data:image/jpeg;base64,"));
                    }
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
                    assert!(decoded[..size as usize].starts_with(&[0xff, 0xd8]));
                    assert!(decoded[..size as usize].ends_with(&[0xff, 0xd9]));
                    assert!(size as usize <= 5_000_000);
                    if case == 5 {
                        assert!(size < 50_000, "metadata padding is not uploaded");
                    }
                    assert_eq!(
                        body["messages"].as_array().unwrap().len(),
                        2,
                        "圖片子請求沒有其他對話或圖片"
                    );
                    child_id = body["client_request_id"].as_str().unwrap().into();
                    json!({"role":"assistant","content":"測試回覆：左紅右藍，未辨識文字。"})
                } else {
                    parents += 1;
                    assert!(
                        !body.to_string().contains("data:image/"),
                        "主上下文不得重複攜帶圖片"
                    );
                    if case == 6 {
                        let wire = body.to_string();
                        assert!(!wire.contains("image-read") && !wire.contains("analyze_image"));
                        assert!(wire.contains("此模型不支援圖片傳入"));
                        super::native::call(
                            &body,
                            "finish",
                            json!({"message":"圖片測試完成","artifacts":[]}),
                        )
                    } else if case == 9 {
                        // 正常任務可逐張／逐焦點辨識，額度仍有界；50次雙圖共100張，第51次不得送出。
                        match parents {
                            1 => super::native::call(&body, "list_files", json!({"path":""})),
                            2..=52 => super::native::call(
                                &body,
                                "analyze_image",
                                json!({"path":name,"compare_path":"second.png","focus":format!("核對第{}個細節", parents-1)}),
                            ),
                            _ => {
                                let result = super::native::last_result(&body);
                                assert_eq!(result["ok"], false);
                                assert!(result["error"]
                                    .as_str()
                                    .unwrap()
                                    .contains("最多辨識100張"));
                                super::native::call(
                                    &body,
                                    "finish",
                                    json!({"message":"圖片測試完成","artifacts":[]}),
                                )
                            }
                        }
                    } else if matches!(case, 10 | 11) {
                        // 從普通檔案清單發現圖片；不必使用 UI 圖片入口。
                        match parents {
                            1 => super::native::call(&body, "list_files", json!({"path":""})),
                            2 => {
                                let result = super::native::last_result(&body);
                                let image = result["result"]["entries"]
                                    .as_array()
                                    .unwrap()
                                    .iter()
                                    .find(|e| e["name"] == name)
                                    .unwrap();
                                assert_eq!(image["kind"], "image");
                                assert_eq!(image["read_tool"], "analyze_image");
                                assert_eq!(children, 0, "列清單不可自動送圖");
                                if case == 11 {
                                    super::native::call(
                                        &body,
                                        "finish",
                                        json!({"message":"圖片測試完成：僅列檔案","artifacts":[]}),
                                    )
                                } else {
                                    super::native::call(&body, "read_file", json!({"path":name}))
                                }
                            }
                            3 => {
                                let result = super::native::last_result(&body);
                                assert_eq!(result["ok"], true);
                                assert_eq!(result["result"]["kind"], "image");
                                assert_eq!(result["result"]["content_read"], false);
                                assert_eq!(children, 0, "read_file 導引不可冒充已辨識或自動送圖");
                                assert!(body["tools"]
                                    .as_array()
                                    .unwrap()
                                    .iter()
                                    .any(|t| t["function"]["name"] == "analyze_image"));
                                super::native::call(
                                    &body,
                                    "analyze_image",
                                    json!({"path":name,"focus":"描述素材重點"}),
                                )
                            }
                            _ => {
                                assert_eq!(super::native::last_result(&body)["ok"], true);
                                super::native::call(
                                    &body,
                                    "finish",
                                    json!({"message":"圖片測試完成","artifacts":[]}),
                                )
                            }
                        }
                    } else {
                        match parents {
                            1 => super::native::call(&body, "list_files", json!({"path":""})),
                            2 | 3 => {
                                if parents == 3 {
                                    let result = super::native::last_result(&body);
                                    assert_eq!(
                                        result["ok"],
                                        case < 3 || matches!(case, 5 | 8 | 12),
                                        "{result}"
                                    );
                                    if case < 3 || matches!(case, 5 | 8 | 12) {
                                        assert!(result["result"]["image"]["sha256"]
                                            .as_str()
                                            .is_some_and(|s| s.len() == 64));
                                    }
                                }
                                if parents == 3 && matches!(case, 3 | 4 | 7) {
                                    super::native::call(
                                        &body,
                                        "finish",
                                        json!({"message":"圖片測試完成","artifacts":[]}),
                                    )
                                } else {
                                    super::native::call(
                                        &body,
                                        "analyze_image",
                                        json!({"compare_path":if case==12 {Some("second.png")}else{None},"path":if case==3 {"../outside.png"} else if case==8 && parents==3 {"second.png"} else {name},"focus":"描述顏色"}),
                                    )
                                }
                            }
                            _ => {
                                if case == 8 {
                                    let result = super::native::last_result(&body);
                                    assert_eq!(result["ok"], true, "一般任務可按需閱讀第二張圖片");
                                    assert_eq!(result["result"]["image"]["path"], "second.png");
                                }
                                super::native::call(
                                    &body,
                                    "finish",
                                    json!({"message":"圖片測試完成","artifacts":[]}),
                                )
                            }
                        }
                    }
                };
                let request = body["client_request_id"].as_str().unwrap();
                let completed = json!({"contract_version":"desktop-agent-v1","principal_id":"fixture_owner","task_id":format!("task_{request}"),"client_request_id":request,"conversation_id":body["conversation_id"],"context":body["context"],"state":"completed","result":{"id":"fixture_completion","object":"chat.completion","created":1,"model":body["model"],"choices":[{"index":0,"finish_reason":if child {"stop"} else {"tool_calls"},"message":message}]}});
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
            name: "圖片驗證".into(),
            root: workspace.clone(),
            imports: BTreeMap::new(),
        },
        conversation: format!("chat_{id}"),
        messages: vec![Message::user(if matches!(case, 10 | 11) {
            "整理專案素材"
        } else {
            "描述專案圖片"
        })],
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
    assert_eq!(
        children,
        match case {
            8 => 2,
            9 => 50,
            10 => 1,
            _ => usize::from(case < 3 || matches!(case, 5 | 7 | 12)),
        },
        "快取與續接不得重送圖片"
    );
    assert_eq!(
        parents,
        if case == 9 {
            53
        } else if case == 10 {
            4
        } else if case == 11 {
            2
        } else if case < 3 || matches!(case, 5 | 8 | 12) {
            4
        } else if case == 6 {
            1
        } else {
            3
        }
    );
    println!("PASS vision case {case}: {parents} parent POST, {children} image POST; converted JPEG, cache and request identity checked.");
    Ok(())
}

/// 加入合法 JPEG 註解區段，保留原始像素；測試真正的 5 MB 邊界而非假影像標頭。
fn jpeg_with_comments(source: &[u8], size: usize) -> Vec<u8> {
    let mut result = source[..2].to_vec();
    let mut remaining = size - source.len();
    while remaining > 0 {
        let mut chunk = remaining.min(65_000);
        if (1..4).contains(&(remaining - chunk)) {
            chunk -= 4;
        }
        assert!(chunk >= 4);
        result.extend_from_slice(&[0xff, 0xfe]);
        result.extend_from_slice(&((chunk - 2) as u16).to_be_bytes());
        result.resize(result.len() + chunk - 4, 0);
        remaining -= chunk;
    }
    result.extend_from_slice(&source[2..]);
    assert_eq!(result.len(), size);
    result
}
