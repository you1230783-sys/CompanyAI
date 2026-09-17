//! 僅供 --demo 使用的本機模擬網站：不會呼叫真實 AI，也不接受外部網卡連線。
//! 讓網站尚未完成時，仍可手動測試瀏覽器授權與 API 往返；不可拿來當正式授權服務。
use crate::{
    config::{
        Config, CHAT_PATH, CLIENT_ID, DEVICE_PATH, DOWNLOAD_PATH, MAX_SESSION_SECONDS, MODELS_PATH,
        TOKEN_PATH, VERSION_PATH,
    },
    AppResult,
};
use serde_json::json;
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::Security::Cryptography::{
    BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
};

mod work;

struct Grant {
    user_code: String,
    csrf: String,
    decision: Option<bool>,
    created: Instant,
}
#[derive(Default)]
struct State {
    grants: HashMap<String, Grant>,
    tokens: Vec<String>,
    notification_read: bool,
    work: work::DemoWork,
    /// 測試時可只開放指定的三條路由，確認客戶端真的使用自訂路徑。
    #[cfg(test)]
    custom_routes: Option<[String; 3]>,
}

pub struct DemoServer {
    pub origin: String,
    stop: Arc<AtomicBool>,
    state: Arc<Mutex<State>>,
}
impl Drop for DemoServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn random_code() -> AppResult<String> {
    let mut bytes = [0_u8; 24];
    // SAFETY: 提供有效的可寫緩衝區，使用 Windows 系統亂數產生器。
    let status = unsafe {
        BCryptGenRandom(
            std::ptr::null_mut(),
            bytes.as_mut_ptr(),
            bytes.len() as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status < 0 {
        return Err("Windows 無法產生示範用隨機碼。".into());
    }
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

impl DemoServer {
    pub fn start() -> AppResult<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|error| format!("無法啟動本機示範：{error}"))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| error.to_string())?;
        let origin = format!(
            "http://{}",
            listener.local_addr().map_err(|error| error.to_string())?
        );
        let state = Arc::new(Mutex::new(State::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let server = Self {
            origin: origin.clone(),
            stop: stop.clone(),
            state: state.clone(),
        };
        thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let state = state.clone();
                        let origin = origin.clone();
                        thread::spawn(move || {
                            if let Err(error) = serve(stream, &origin, &state) {
                                // 本機示範的診斷只記錄原因，不輸出請求本文、Header 或憑證。
                                eprintln!("Demo server: {error}");
                            }
                        });
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(20))
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(server)
    }
    pub fn config(&self) -> Config {
        Config {
            server_url: self.origin.clone(),
            model: "demo-echo".into(),
            ..Config::default()
        }
    }
    /// 供自我檢查確認服務仍持有自己的狀態；正式使用者不會經過此測試伺服器。
    pub fn is_running(&self) -> bool {
        !self.stop.load(Ordering::Relaxed) && Arc::strong_count(&self.state) > 1
    }
}

fn form(body: &str) -> HashMap<String, String> {
    url::form_urlencoded::parse(body.as_bytes())
        .into_owned()
        .collect()
}

fn serve(mut stream: TcpStream, origin: &str, state: &Mutex<State>) -> AppResult<()> {
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|error| error.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    let header_end = loop {
        let mut chunk = [0_u8; 4096];
        let read = stream.read(&mut chunk).map_err(|error| error.to_string())?;
        if read == 0 {
            return Ok(());
        }
        bytes.extend_from_slice(&chunk[..read]);
        if bytes.len() > 65_536 {
            return Err("示範請求過大。".into());
        }
        if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break end + 4;
        }
    };
    let header_text = String::from_utf8_lossy(&bytes[..header_end]).to_string();
    let mut lines = header_text.lines();
    let first = lines.next().ok_or("缺少 HTTP request line。")?;
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    let headers: HashMap<String, String> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().into()))
        .collect();
    let length: usize = headers
        .get("content-length")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    if length > 10 * 1024 * 1024 {
        return Err("示範請求過大。".into());
    }
    while bytes.len() < header_end + length {
        let mut chunk = [0_u8; 4096];
        let read = stream.read(&mut chunk).map_err(|error| error.to_string())?;
        if read == 0 {
            return Err("請求內容不完整。".into());
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
    let body = String::from_utf8_lossy(&bytes[header_end..header_end + length]);
    let url = url::Url::parse(&format!("{origin}{target}")).map_err(|error| error.to_string())?;
    let fields = form(&body);
    let mut state = state.lock().map_err(|_| "示範服務狀態錯誤。".to_string())?;
    let route = url.path();
    #[cfg(test)]
    let route = match &state.custom_routes {
        Some(routes) => routes
            .iter()
            .position(|path| path == route)
            .map(|index| [DEVICE_PATH, TOKEN_PATH, CHAT_PATH][index])
            .unwrap_or(""),
        None => route,
    };
    let mut status = 200;
    let mut content_type = "application/json; charset=utf-8";
    // 示範通知使用與正式版相同的個人 Bearer 驗證，沒有匿名通知通道。
    let authorized = headers
        .get("authorization")
        .and_then(|s| s.strip_prefix("Bearer "))
        .is_some_and(|token| state.tokens.iter().any(|known| known == token));
    let owner = if authorized {
        headers
            .get("authorization")
            .and_then(|s| s.strip_prefix("Bearer "))
    } else {
        None
    };
    if work::serve_work(
        &mut stream,
        method,
        route,
        &bytes[header_end..header_end + length],
        owner,
        &mut state.work,
    )? {
        return Ok(());
    }
    if method == "GET" && route == crate::notifications::SOCKET_PATH && authorized {
        let key = headers
            .get("sec-websocket-key")
            .ok_or("缺少 WebSocket Key。")?;
        let accept = websocket_accept(key)?;
        drop(state);
        stream.write_all(format!("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n").as_bytes()).map_err(|e|e.to_string())?;
        let payload = b"{\"type\":\"events_available\"}";
        stream
            .write_all(&[0x81, payload.len() as u8])
            .and_then(|()| stream.write_all(payload))
            .map_err(|e| e.to_string())?;
        // 維持連線直到客戶端關閉；測試會確認取消可中斷等待。
        stream
            .set_read_timeout(Some(Duration::from_secs(60)))
            .map_err(|e| e.to_string())?;
        let mut buffer = [0u8; 128];
        let _ = stream.read(&mut buffer);
        return Ok(());
    }
    let reply = match (method, route) {
        ("GET",crate::notifications::EVENTS_PATH) if authorized=>{
            let cursor=if state.notification_read{"demo-2"}else{"demo-1"};
            let after=url.query_pairs().find(|(key,_)|key=="after").map(|(_,value)|value.into_owned());
            let events=if after.as_deref()==Some(cursor){vec![]}else{vec![json!({"id":"demo_notice","type":"notice","title":"歡迎使用 LM_AI","summary":"這是一則本機示範通知，可測試補查與標記已讀。","created_at":crate::notifications::now_text(),"expires_at":null,"resource_id":null,"read_at":if state.notification_read{Some(crate::notifications::now_text())}else{None}})]};
            json!({"events":events,"next_cursor":cursor,"has_more":false}).to_string()
        },
        ("POST","/lm_server/api/desktop/events/demo_notice/read") if authorized=>{state.notification_read=true;json!({"ok":true}).to_string()},
        (_,route) if route.starts_with(crate::notifications::EVENTS_PATH)&&!authorized=>{status=401;json!({"error":{"message":"Please log in."}}).to_string()},
        ("GET", VERSION_PATH) => json!({"latest_version":env!("CARGO_PKG_VERSION"),"minimum_version":"0.3.0","message":""}).to_string(),
        ("GET", MODELS_PATH) => json!({"models":[{"id":"fast","label":"快速"},{"id":"quality","label":"品質"},{"id":"ultra","label":"Ultra"}],"default_model":"fast"}).to_string(),
        ("GET", DOWNLOAD_PATH) => {content_type="text/plain; charset=utf-8";"這是本機示範，不提供真實更新檔案。".into()},
        ("POST", DEVICE_PATH) if fields.get("client_id").map(String::as_str) == Some(CLIENT_ID) => {
            let device = random_code()?;
            let user_code = random_code()?[..8].to_ascii_uppercase();
            let csrf = random_code()?;
            state.grants.insert(
                device.clone(),
                Grant {
                    user_code: user_code.clone(),
                    csrf,
                    decision: None,
                    created: Instant::now(),
                },
            );
            json!({"device_code":device,"user_code":user_code,"verification_uri":format!("{origin}/activate"),"verification_uri_complete":format!("{origin}/activate?user_code={user_code}"),"expires_in":300,"interval":1}).to_string()
        }
        ("GET", "/activate") => {
            content_type = "text/html; charset=utf-8";
            let code = url
                .query_pairs()
                .find(|(key, _)| key == "user_code")
                .map(|(_, value)| value.into_owned())
                .unwrap_or_default();
            if let Some(grant) = state.grants.values().find(|grant| grant.user_code == code) {
                format!("<!doctype html><html lang=zh-Hant><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'><title>LM_AI 本機示範授權</title><style>body{{font:18px 'Segoe UI',sans-serif;background:#f1f5f9;color:#172554;max-width:620px;margin:10vh auto;padding:32px}}main{{background:white;padding:40px;border-radius:18px}}button{{font:inherit;padding:12px 20px;margin:8px;border:0;border-radius:8px;background:#1d4ed8;color:white}}code{{font-size:32px;letter-spacing:4px}}</style><main><h1>允許這次桌面登入？</h1><p>這是本機示範，不會登入公司帳號或呼叫真實 AI。</p><p>請確認 EXE 顯示相同代碼：</p><p><code>{}</code></p><form method=post action=/demo/approve><input type=hidden name=user_code value='{}'><input type=hidden name=csrf value='{}'><button name=decision value=allow>允許登入 · 30 天</button><button name=decision value=deny>拒絕</button></form></main></html>", grant.user_code, grant.user_code, grant.csrf)
            } else {
                status = 400;
                "找不到登入碼，請回 EXE 重新登入。".into()
            }
        }
        ("POST", "/demo/approve") => {
            content_type = "text/html; charset=utf-8";
            let valid_origin = headers.get("origin").is_none_or(|value| value == origin);
            let grant = state.grants.values_mut().find(|grant| {
                Some(&grant.user_code) == fields.get("user_code")
                    && Some(&grant.csrf) == fields.get("csrf")
            });
            if let Some(grant) = grant.filter(|grant| {
                valid_origin && grant.created.elapsed().as_secs() < 300 && grant.decision.is_none()
            }) {
                grant.decision = Some(fields.get("decision").map(String::as_str) == Some("allow"));
                "<!doctype html><meta charset=utf-8><title>已完成授權操作</title><h1>已完成操作，請回到 LM_AI 視窗。</h1><p>這是本機示範，不是真實公司登入。</p>".into()
            } else {
                status = 400;
                "登入碼無效、已使用或已過期。".into()
            }
        }
        ("POST", TOKEN_PATH) => {
            let device = fields.get("device_code").cloned().unwrap_or_default();
            let valid = fields.get("client_id").map(String::as_str) == Some(CLIENT_ID)
                && fields.get("grant_type").map(String::as_str)
                    == Some("urn:ietf:params:oauth:grant-type:device_code");
            match state
                .grants
                .get(&device)
                .filter(|grant| valid && grant.created.elapsed().as_secs() < 300)
                .map(|grant| grant.decision)
            {
                Some(Some(true)) => {
                    state.grants.remove(&device);
                    let token = random_code()?;
                    state.tokens.push(token.clone());
                    json!({"access_token":token,"token_type":"Bearer","expires_in":MAX_SESSION_SECONDS}).to_string()
                }
                Some(Some(false)) => {
                    state.grants.remove(&device);
                    status = 400;
                    json!({"error":"access_denied"}).to_string()
                }
                Some(None) => {
                    status = 400;
                    json!({"error":"authorization_pending"}).to_string()
                }
                None => {
                    status = 400;
                    json!({"error":"expired_token"}).to_string()
                }
            }
        }
        ("POST", CHAT_PATH) => {
            let token = headers
                .get("authorization")
                .and_then(|value| value.strip_prefix("Bearer "))
                .or_else(|| headers.get("x-api-key").map(String::as_str));
            if !token.is_some_and(|token| state.tokens.iter().any(|known| known == token)) {
                status = 401;
                json!({"error":{"message":"Demo token is invalid."}}).to_string()
            } else {
                let request: serde_json::Value =
                    serde_json::from_str(&body).map_err(|_| "JSON 錯誤。")?;
                if request["stream"] != false || !matches!(request["model"].as_str(), Some("demo-echo"|"fast"|"quality"|"ultra"))
                    || headers.get("x-client-version").map(String::as_str) != Some(env!("CARGO_PKG_VERSION")) {
                    status = 400;
                    json!({"error":{"message":"Use an available model, stream=false and X-Client-Version."}}).to_string()
                } else {
                    let input = request["messages"]
                        .as_array()
                        .and_then(|messages| messages.last())
                        .and_then(|message| message["content"].as_str())
                        .unwrap_or("");
                    json!({"id":"demo-response","object":"chat.completion","model":"demo-echo","choices":[{"index":0,"message":{"role":"assistant","content":format!("【本機模擬回覆，非真實 AI】\n\n已收到你的訊息：\n{input}\n\n瀏覽器授權、API Key Header、JSON 請求與回覆解析皆已完成。")},"finish_reason":"stop"}]}).to_string()
                }
            }
        }
        _ => {
            status = 404;
            json!({"error":{"message":"Unknown demo route."}}).to_string()
        }
    };
    drop(state);
    let response = format!("HTTP/1.1 {status} Result\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n\r\n{reply}", reply.len());
    stream
        .write_all(response.as_bytes())
        .map_err(|error| error.to_string())
}

/// RFC 6455 握手：Windows 內建 SHA-1 僅用於協定校驗，不用來保存憑證。
fn websocket_accept(key: &str) -> AppResult<String> {
    use windows_sys::Win32::Security::Cryptography::*;
    let input = format!("{key}258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    let mut digest = [0u8; 20];
    let mut encoded = [0u8; 64];
    let mut length = encoded.len() as u32;
    unsafe {
        if BCryptHash(
            BCRYPT_SHA1_ALG_HANDLE,
            std::ptr::null(),
            0,
            input.as_ptr(),
            input.len() as u32,
            digest.as_mut_ptr(),
            20,
        ) < 0
        {
            return Err("WebSocket 校驗失敗。".into());
        }
        if CryptBinaryToStringA(
            digest.as_ptr(),
            20,
            CRYPT_STRING_BASE64 | CRYPT_STRING_NOCRLF,
            encoded.as_mut_ptr(),
            &mut length,
        ) == 0
        {
            return Err("WebSocket 編碼失敗。".into());
        }
    }
    String::from_utf8(encoded[..length as usize].to_vec())
        .map(|s| s.trim_end_matches('\0').to_owned())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn notification_replay_read_and_websocket_cancel() {
        use crate::{notifications, protocol::TokenResponse, storage::Session};
        use std::sync::mpsc;
        assert_eq!(
            super::websocket_accept("dGhlIHNhbXBsZSBub25jZQ==").unwrap(),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
        let server = super::DemoServer::start().unwrap();
        let config = server.config();
        server
            .state
            .lock()
            .unwrap()
            .tokens
            .push("notification-test-token".into());
        let session = Session::from_token(
            TokenResponse {
                access_token: "notification-test-token".into(),
                token_type: "Bearer".into(),
                expires_in: 3600,
            },
            &config,
        )
        .unwrap();
        let page = notifications::fetch_page(&config, &session, None).unwrap();
        assert_eq!(page.events.len(), 1);
        let cursor = page.next_cursor.unwrap();
        assert!(notifications::fetch_page(&config, &session, Some(&cursor))
            .unwrap()
            .events
            .is_empty());
        notifications::mark_read(&config, &session, "demo_notice").unwrap();
        assert!(notifications::fetch_page(&config, &session, Some(&cursor))
            .unwrap()
            .events[0]
            .read_at
            .is_some());
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let (tx, rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            crate::transport::watch_notifications(
                &config.endpoint(notifications::SOCKET_PATH).unwrap(),
                &session.access_token,
                &worker_cancel,
                |connected| {
                    let _ = tx.send(connected);
                },
            )
        });
        assert!(rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap());
        assert!(!rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap());
        let start = std::time::Instant::now();
        cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = worker.join();
        assert!(start.elapsed() < std::time::Duration::from_secs(3));
    }
    #[test]
    fn desktop_metadata_and_model_alias_complete_a_chat() {
        let server = super::DemoServer::start().unwrap();
        let mut config = server.config();
        let version = crate::service::fetch_version(&config).unwrap();
        assert!(!version.required());
        let catalog = crate::service::fetch_models(&config, None).unwrap();
        assert_eq!(catalog.models[0].label, "快速");
        config.model = catalog.models[0].id.clone();
        let grant = crate::auth::request_device(&config).unwrap();
        server
            .state
            .lock()
            .unwrap()
            .grants
            .get_mut(&grant.device_code)
            .unwrap()
            .decision = Some(true);
        let crate::auth::PollResult::Granted(session) =
            crate::auth::poll_once(&config, &grant).unwrap()
        else {
            panic!("grant expected")
        };
        let body = crate::protocol::chat_json(
            &config.model,
            &[crate::protocol::Message::user("模型代號測試")],
        )
        .unwrap();
        assert!(crate::auth::send_chat(&config, &session, &body)
            .reply
            .unwrap()
            .contains("模型代號測試"));
    }
    use super::*;
    use crate::{
        auth::{self, PollResult},
        config::AuthHeader,
        protocol::{chat_json, Message},
        transport,
    };

    #[test]
    fn login_once_then_chat_with_both_header_formats() {
        let server = DemoServer::start().unwrap();
        let mut config = server.config();
        let grant = auth::request_device(&config).unwrap();
        assert!(matches!(
            auth::poll_once(&config, &grant).unwrap(),
            PollResult::Pending
        ));
        let csrf = server.state.lock().unwrap().grants[&grant.device_code]
            .csrf
            .clone();
        let url = config.endpoint("/demo/approve").unwrap();
        let response = transport::post_form(
            &url,
            &[
                ("user_code", &grant.user_code),
                ("csrf", &csrf),
                ("decision", "allow"),
            ],
        )
        .unwrap();
        assert_eq!(response.status, 200);
        let PollResult::Granted(mut session) = auth::poll_once(&config, &grant).unwrap() else {
            panic!("expected granted");
        };
        assert!(
            auth::poll_once(&config, &grant).is_err(),
            "one-time code must not be reusable"
        );
        let body = chat_json("demo-echo", &[Message::user("繁體中文測試")]).unwrap();
        let result = auth::send_chat(&config, &session, &body);
        assert!(result.reply.unwrap().contains("繁體中文測試"));
        config.auth_header = AuthHeader::XApiKey;
        assert!(!session.valid_for(&config));
        session.binding = config.binding().unwrap();
        assert!(auth::send_chat(&config, &session, &body).reply.is_ok());
        session.access_token = "wrong-token".into();
        assert!(auth::send_chat(&config, &session, &body).unauthorized);
    }

    #[test]
    fn custom_nested_routes_complete_login_and_nonstream_chat() {
        let server = DemoServer::start().unwrap();
        let mut config = Config {
            server_url: format!("{}/gateway/api/v1/desktop", server.origin),
            chat_path: "v1/chat/completions".into(),
            device_path: "oauth/device".into(),
            token_path: "oauth/token".into(),
            ..server.config()
        };
        // 舊的根目錄路由不再開放；若 auth 模組仍使用固定路徑，登入就會失敗。
        server.state.lock().unwrap().custom_routes = Some([
            "/gateway/api/v1/desktop/oauth/device".into(),
            "/gateway/api/v1/desktop/oauth/token".into(),
            "/gateway/api/v1/desktop/v1/chat/completions".into(),
        ]);
        let grant = auth::request_device(&config).unwrap();
        server
            .state
            .lock()
            .unwrap()
            .grants
            .get_mut(&grant.device_code)
            .unwrap()
            .decision = Some(true);
        let PollResult::Granted(session) = auth::poll_once(&config, &grant).unwrap() else {
            panic!("expected granted");
        };
        // 改用相同端點的完整網址，應仍能使用剛才取得的憑證。
        config.chat_path = format!(
            "{}/gateway/api/v1/desktop/v1/chat/completions",
            server.origin
        );
        let body = chat_json("demo-echo", &[Message::user("多層路徑連線測試")]).unwrap();
        let result = auth::send_chat(&config, &session, &body);
        assert!(!result.unauthorized);
        assert!(result.reply.unwrap().contains("多層路徑連線測試"));
    }

    #[test]
    fn denied_login_never_returns_a_token() {
        let server = DemoServer::start().unwrap();
        let config = server.config();
        let grant = auth::request_device(&config).unwrap();
        server
            .state
            .lock()
            .unwrap()
            .grants
            .get_mut(&grant.device_code)
            .unwrap()
            .decision = Some(false);
        assert!(auth::poll_once(&config, &grant).is_err());
        assert!(auth::poll_once(&config, &grant).is_err());
    }

    #[test]
    fn expired_grant_and_cancelled_poll_do_not_create_sessions() {
        let server = DemoServer::start().unwrap();
        let config = server.config();
        let grant = auth::request_device(&config).unwrap();
        assert!(auth::wait_for_login(&config, &grant, &AtomicBool::new(true)).is_err());
        server
            .state
            .lock()
            .unwrap()
            .grants
            .get_mut(&grant.device_code)
            .unwrap()
            .created = Instant::now() - Duration::from_secs(301);
        assert!(auth::poll_once(&config, &grant).is_err());
        assert!(server.state.lock().unwrap().tokens.is_empty());
    }
}
