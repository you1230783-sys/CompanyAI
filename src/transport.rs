//! 使用 Windows 原生 WinHTTP：沿用系統憑證信任與代理設定，不需附帶 OpenSSL DLL。
//! 所有同步請求都在背景執行緒呼叫，避免凍結介面。
use crate::{wide, AppResult};
use std::{ffi::c_void, ptr};
use url::Url;
use windows_sys::Win32::Networking::WinHttp::*;

pub struct HttpResponse {
    pub status: u32,
    pub body: String,
}

/// WinHTTP handle 與建立它的請求一起存活；Drop 確保每個錯誤路徑都會關閉。
struct Handle(*mut c_void);
impl Handle {
    fn checked(operation: &str, value: *mut c_void) -> AppResult<Self> {
        if value.is_null() {
            Err(network_error(operation))
        } else {
            Ok(Self(value))
        }
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: handle 由 WinHTTP 建立，僅由此擁有者關閉一次。
        unsafe {
            WinHttpCloseHandle(self.0);
        }
    }
}

/// 必須緊接失敗的 WinHTTP 呼叫，先取得錯誤碼，再組合說明。
/// 只記錄固定的 API 名稱；不附上網址、Header 或本文，避免洩漏登入碼與 Token。
fn network_error(operation: &str) -> String {
    let code = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
    let detail = match code {
        10022 => "網路元件回報參數或連線狀態無效；請提供失敗步驟及錯誤碼供排查。",
        12002 => "連線逾時。請確認 API 可從這台電腦連線；請求可能已送達，重試前請留意。",
        12007 => "無法解析主機名稱，請檢查內網 DNS 與網站網址。",
        12175 => "HTTPS 憑證驗證失敗。請由公司 IT 安裝正確的信任憑證；程式不會略過憑證驗證。",
        _ => "請確認網址、VPN、代理伺服器與防火牆。",
    };
    format!("Windows 網路錯誤 {code}（{operation}）。{detail}")
}

/// 發送一次 HTTP 請求，不自動重試或跟隨重新導向，避免重複呼叫或把憑證帶到別站。
pub fn request(
    url: &Url,
    content_type: &str,
    body: &str,
    auth: Option<(&str, &str)>,
    timeout_ms: i32,
) -> AppResult<HttpResponse> {
    request_method(url, "POST", content_type, body, auth, timeout_ms)
}

/// 查詢版本與模型清單只發送 GET，沒有聊天本文。
pub fn get(url: &Url, auth: Option<(&str, &str)>) -> AppResult<HttpResponse> {
    request_method(url, "GET", "application/json", "", auth, 15_000)
}

/// 通知專用 WebSocket。只接收喚醒訊號，事件內容仍經 REST 補查、去重及落盤。
/// 取消時由小型守護執行緒關閉 handle，讓阻塞中的 Receive 立即返回。
pub fn watch_notifications(
    url: &Url,
    token: &str,
    cancelled: &std::sync::Arc<std::sync::atomic::AtomicBool>,
    mut changed: impl FnMut(bool),
) -> AppResult<()> {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use std::{thread, time::Duration};
    struct Socket(Arc<AtomicUsize>);
    impl Drop for Socket {
        fn drop(&mut self) {
            let raw = self.0.swap(0, Ordering::SeqCst);
            if raw != 0 {
                unsafe {
                    WinHttpCloseHandle(raw as *mut c_void);
                }
            }
        }
    }
    if token.is_empty() || !token.bytes().all(|b| b.is_ascii_graphic()) {
        return Err("通知驗證資料不正確。".into());
    }
    let host = wide(url.host_str().ok_or("通知主機無效。")?);
    let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    unsafe {
        let session = Handle::checked(
            "WinHttpOpen",
            WinHttpOpen(
                wide("LM_AI/notifications").as_ptr(),
                if local {
                    WINHTTP_ACCESS_TYPE_NO_PROXY
                } else {
                    WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY
                },
                ptr::null(),
                ptr::null(),
                0,
            ),
        )?;
        if WinHttpSetTimeouts(session.0, 10_000, 10_000, 15_000, 15_000) == 0 {
            return Err(network_error("WinHttpSetTimeouts"));
        }
        let connection = Handle::checked(
            "WinHttpConnect",
            WinHttpConnect(
                session.0,
                host.as_ptr(),
                url.port_or_known_default().ok_or("通知埠號無效。")?,
                0,
            ),
        )?;
        let request = Handle::checked(
            "WinHttpOpenRequest",
            WinHttpOpenRequest(
                connection.0,
                wide("GET").as_ptr(),
                wide(url.path()).as_ptr(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                if url.scheme() == "https" {
                    WINHTTP_FLAG_SECURE
                } else {
                    0
                },
            ),
        )?;
        let disable =
            WINHTTP_DISABLE_REDIRECTS | WINHTTP_DISABLE_COOKIES | WINHTTP_DISABLE_AUTHENTICATION;
        if WinHttpSetOption(
            request.0,
            WINHTTP_OPTION_DISABLE_FEATURE,
            &disable as *const u32 as *const c_void,
            4,
        ) == 0
        {
            return Err(network_error("WinHttpSetOption/notifications"));
        }
        if WinHttpSetOption(
            request.0,
            WINHTTP_OPTION_UPGRADE_TO_WEB_SOCKET,
            ptr::null(),
            0,
        ) == 0
        {
            return Err(network_error("WinHttpSetOption/websocket"));
        }
        let headers = wide(&format!(
            "Authorization: Bearer {token}\r\nAccept: application/json\r\nX-Client-Version: {}\r\n",
            env!("CARGO_PKG_VERSION")
        ));
        if WinHttpSendRequest(
            request.0,
            headers.as_ptr(),
            (headers.len() - 1) as u32,
            ptr::null(),
            0,
            0,
            0,
        ) == 0
        {
            return Err(network_error("WinHttpSendRequest"));
        }
        if WinHttpReceiveResponse(request.0, ptr::null_mut()) == 0 {
            return Err(network_error("WinHttpReceiveResponse"));
        }
        let mut status = 0u32;
        let mut length = 4;
        if WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            ptr::null(),
            &mut status as *mut u32 as *mut c_void,
            &mut length,
            ptr::null_mut(),
        ) == 0
        {
            return Err(network_error("WinHttpQueryHeaders/status"));
        }
        if status != 101 {
            return Err("即時通知尚未連線，先使用定時補查。".into());
        }
        let raw = WinHttpWebSocketCompleteUpgrade(request.0, 0);
        if raw.is_null() {
            return Err(network_error("WinHttpWebSocketCompleteUpgrade"));
        }
        let socket = Socket(Arc::new(AtomicUsize::new(raw as usize)));
        let shared = socket.0.clone();
        let cancel = cancelled.clone();
        thread::spawn(move || {
            while shared.load(Ordering::SeqCst) != 0 {
                if cancel.load(Ordering::Relaxed) {
                    let raw = shared.swap(0, Ordering::SeqCst);
                    if raw != 0 {
                        WinHttpCloseHandle(raw as *mut c_void);
                    }
                    break;
                }
                thread::sleep(Duration::from_millis(100));
            }
        });
        changed(true);
        let mut message = Vec::new();
        let mut buffer = [0u8; 4096];
        while !cancelled.load(Ordering::Relaxed) {
            let mut count = 0;
            let mut kind = 0;
            let result = WinHttpWebSocketReceive(
                raw,
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
                &mut count,
                &mut kind,
            );
            if result != 0 {
                return Err("即時通知已中斷，將自動重連。".into());
            }
            if kind == WINHTTP_WEB_SOCKET_CLOSE_BUFFER_TYPE {
                return Err("通知連線已結束，將自動重連。".into());
            }
            if !matches!(
                kind,
                WINHTTP_WEB_SOCKET_UTF8_FRAGMENT_BUFFER_TYPE
                    | WINHTTP_WEB_SOCKET_UTF8_MESSAGE_BUFFER_TYPE
            ) {
                return Err("通知服務應傳送文字 JSON。".into());
            }
            message.extend_from_slice(&buffer[..count as usize]);
            if message.len() > 65_536 {
                return Err("通知訊號過大。".into());
            }
            if kind == WINHTTP_WEB_SOCKET_UTF8_MESSAGE_BUFFER_TYPE {
                if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&message) {
                    if value.get("type").and_then(|v| v.as_str()) != Some("ping") {
                        changed(false);
                    }
                }
                message.clear();
            }
        }
    }
    Ok(())
}

pub(crate) fn request_method(
    url: &Url,
    method: &str,
    content_type: &str,
    body: &str,
    auth: Option<(&str, &str)>,
    timeout_ms: i32,
) -> AppResult<HttpResponse> {
    let mut reader = std::io::Cursor::new(body.as_bytes());
    exchange(
        url,
        method,
        content_type,
        Payload {
            reader: &mut reader,
            length: body.len() as u32,
        },
        auth,
        timeout_ms,
        None,
    )
}

/// 已知長度的輸入串流：大檔案逐段解密、上傳，不一次載入記憶體。
pub struct Payload<'a> {
    pub reader: &'a mut dyn std::io::Read,
    pub length: u32,
}
pub type ResponseChunks<'a> = &'a mut dyn FnMut(&[u8]) -> AppResult<bool>;

/// JSON、附件與 SSE 共用憑證、代理與禁止轉址規則。
/// on_chunk 僅接受 200 text/event-stream；其他 HTTP 回應仍以有界 JSON 回傳。
pub fn exchange(
    url: &Url,
    method: &str,
    content_type: &str,
    payload: Payload<'_>,
    auth: Option<(&str, &str)>,
    timeout_ms: i32,
    on_chunk: Option<ResponseChunks<'_>>,
) -> AppResult<HttpResponse> {
    exchange_inner(
        url,
        method,
        content_type,
        payload,
        auth,
        timeout_ms,
        on_chunk,
        "text/event-stream",
    )
}

/// 有界二進位下載，沿用禁止重新導向／系統 TLS 驗證，不傳登入 Token。
pub fn download_file(url: &Url, path: &std::path::Path, expected_size: u64) -> AppResult<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("無法建立更新暫存：{e}"))?;
    let mut received = 0u64;
    let started = std::time::Instant::now();
    let mut chunk = |bytes: &[u8]| {
        received += bytes.len() as u64;
        if received > expected_size || started.elapsed().as_secs() > 1800 {
            return Err("更新檔超過指定長度或下載逾時。".into());
        }
        file.write_all(bytes).map_err(|e| e.to_string())?;
        Ok(false)
    };
    let response = exchange_inner(
        url,
        "GET",
        "application/octet-stream",
        Payload {
            reader: &mut std::io::empty(),
            length: 0,
        },
        None,
        30_000,
        Some(&mut chunk),
        "application/octet-stream",
    )?;
    if response.status != 200 || received != expected_size {
        return Err(format!(
            "更新下載不完整（HTTP {}），請重試。",
            response.status
        ));
    }
    file.sync_all().map_err(|e| e.to_string())
}

#[allow(clippy::too_many_arguments)]
fn exchange_inner(
    url: &Url,
    method: &str,
    content_type: &str,
    payload: Payload<'_>,
    auth: Option<(&str, &str)>,
    timeout_ms: i32,
    mut on_chunk: Option<ResponseChunks<'_>>,
    response_type: &str,
) -> AppResult<HttpResponse> {
    let host = wide(url.host_str().ok_or("網址缺少主機名稱。")?);
    let path = match url.query() {
        Some(query) => format!("{}?{query}", url.path()),
        None => url.path().to_string(),
    };
    let mut headers = format!(
        "Content-Type: {content_type}\r\nAccept: application/json\r\nX-Client-Version: {}\r\n",
        env!("CARGO_PKG_VERSION")
    );
    if let Some((name, value)) = auth {
        if !matches!(name, "Authorization" | "X-API-Key") || value.contains(['\r', '\n', '\0']) {
            return Err("驗證 Header 無效。".into());
        }
        headers.push_str(&format!("{name}: {value}\r\n"));
    }
    if on_chunk.is_some() {
        headers = headers.replace(
            "Accept: application/json",
            &format!("Accept: {response_type}"),
        );
    }
    // 本機示範不經代理，避免系統 PAC 或企業代理把 loopback 請求轉送出去。
    let proxy_mode = if matches!(url.host_str(), Some("127.0.0.1" | "[::1]" | "localhost")) {
        WINHTTP_ACCESS_TYPE_NO_PROXY
    } else {
        WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY
    };
    // SAFETY: 傳入的 UTF-16 字串與 body 在整次同步請求期間有效；所有輸出緩衝區有明確長度。
    unsafe {
        let session = Handle::checked(
            "WinHttpOpen",
            WinHttpOpen(
                wide(concat!("CompanyAI/", env!("CARGO_PKG_VERSION"))).as_ptr(),
                proxy_mode,
                ptr::null(),
                ptr::null(),
                0,
            ),
        )?;
        if WinHttpSetTimeouts(session.0, 10_000, 10_000, timeout_ms, timeout_ms) == 0 {
            return Err(network_error("WinHttpSetTimeouts"));
        }
        let connection = Handle::checked(
            "WinHttpConnect",
            WinHttpConnect(
                session.0,
                host.as_ptr(),
                url.port_or_known_default().ok_or("網址埠號無效。")?,
                0,
            ),
        )?;
        let flags = if url.scheme() == "https" {
            WINHTTP_FLAG_SECURE
        } else {
            0
        };
        let request = Handle::checked(
            "WinHttpOpenRequest",
            WinHttpOpenRequest(
                connection.0,
                wide(method).as_ptr(),
                wide(&path).as_ptr(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                flags,
            ),
        )?;
        let policy = WINHTTP_OPTION_REDIRECT_POLICY_NEVER;
        if WinHttpSetOption(
            request.0,
            WINHTTP_OPTION_REDIRECT_POLICY,
            (&policy as *const u32).cast(),
            4,
        ) == 0
        {
            return Err(network_error("WinHttpSetOption/redirect"));
        }
        // 不重用網站 cookies；本應用只使用明確取得的 API 憑證。
        let disabled = WINHTTP_DISABLE_COOKIES;
        if WinHttpSetOption(
            request.0,
            WINHTTP_OPTION_DISABLE_FEATURE,
            (&disabled as *const u32).cast(),
            4,
        ) == 0
        {
            return Err(network_error("WinHttpSetOption/cookies"));
        }
        let headers = wide(&headers);
        if WinHttpSendRequest(
            request.0,
            headers.as_ptr(),
            (headers.len() - 1) as u32,
            ptr::null(),
            0,
            payload.length,
            0,
        ) == 0
        {
            return Err(network_error("WinHttpSendRequest"));
        }
        let mut remaining = payload.length as usize;
        let mut upload_buffer = [0u8; 48 * 1024];
        while remaining > 0 {
            let limit = remaining.min(upload_buffer.len());
            let count = payload
                .reader
                .read(&mut upload_buffer[..limit])
                .map_err(|_| "無法讀取附件暫存資料。")?;
            if count == 0 {
                return Err("附件資料不完整，請重新選取。".into());
            }
            let mut offset = 0;
            while offset < count {
                let mut written = 0;
                if WinHttpWriteData(
                    request.0,
                    upload_buffer[offset..count].as_ptr().cast(),
                    (count - offset) as u32,
                    &mut written,
                ) == 0
                {
                    return Err(network_error("WinHttpWriteData"));
                }
                // API 成功但沒有寫入資料時，GetLastError 不代表本次結果。
                // 明確報告無進度，避免顯示其他呼叫殘留的 10022 等錯誤碼。
                if written == 0 {
                    return Err(
                        "傳送資料失敗（WinHttpWriteData）：未寫入任何資料，已停止傳送。".into(),
                    );
                }
                offset += written as usize;
            }
            remaining -= count;
        }
        if WinHttpReceiveResponse(request.0, ptr::null_mut()) == 0 {
            return Err(network_error("WinHttpReceiveResponse"));
        }
        let mut status = 0_u32;
        let mut size = 4_u32;
        if WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            ptr::null(),
            (&mut status as *mut u32).cast(),
            &mut size,
            ptr::null_mut(),
        ) == 0
        {
            return Err(network_error("WinHttpQueryHeaders/status"));
        }
        if (300..400).contains(&status) {
            return Err(format!(
                "HTTP {status} 重新導向已停止。請填寫 API 的最終網址，不可導向登入 HTML 頁面。"
            ));
        }
        if on_chunk.is_some() && status == 200 {
            let mut content_type = [0u16; 128];
            let mut size = (content_type.len() * 2) as u32;
            if WinHttpQueryHeaders(
                request.0,
                WINHTTP_QUERY_CONTENT_TYPE,
                ptr::null(),
                content_type.as_mut_ptr().cast(),
                &mut size,
                ptr::null_mut(),
            ) == 0
            {
                return Err(network_error("WinHttpQueryHeaders/content-type"));
            }
            if !String::from_utf16_lossy(&content_type)
                .to_ascii_lowercase()
                .starts_with(response_type)
            {
                return Err(format!("回應 Content-Type 必須為 {response_type}。"));
            }
        }
        let started = std::time::Instant::now();
        let mut bytes = Vec::new();
        loop {
            let mut chunk = [0_u8; 8192];
            let mut read = 0_u32;
            // SSE 小事件不能直接要求填滿 8 KB；WinHTTP 可能等待更多資料才返回。
            // 先查目前可讀長度，再只讀該段，工具進度及首段文字才會立即送到介面。
            let mut wanted = chunk.len() as u32;
            if on_chunk.is_some() && status == 200 {
                let mut available = 0;
                if WinHttpQueryDataAvailable(request.0, &mut available) == 0 {
                    return Err(network_error("WinHttpQueryDataAvailable"));
                }
                if available == 0 {
                    break;
                }
                wanted = available.min(wanted);
            }
            if WinHttpReadData(request.0, chunk.as_mut_ptr().cast(), wanted, &mut read) == 0 {
                return Err(network_error("WinHttpReadData"));
            }
            if read == 0 {
                break;
            }
            if status == 200 {
                if let Some(callback) = on_chunk.as_mut() {
                    if callback(&chunk[..read as usize])? {
                        break;
                    }
                    // 十分鐘後轉為 REST 查詢；伺服器繼續執行，不取消長任務。
                    if response_type == "text/event-stream"
                        && started.elapsed() > std::time::Duration::from_secs(600)
                    {
                        return Err("長任務已轉為背景查詢。".into());
                    }
                    continue;
                }
            }
            if bytes.len() + read as usize > 1_048_576 {
                return Err("API 回應超過 1 MB，已停止讀取。".into());
            }
            bytes.extend_from_slice(&chunk[..read as usize]);
        }
        let body =
            String::from_utf8(bytes).map_err(|_| "API 回應必須為 UTF-8 JSON。".to_string())?;
        Ok(HttpResponse { status, body })
    }
}

/// OAuth 標準使用 form-urlencoded；聊天介面才使用 application/json。
pub fn post_form(url: &Url, fields: &[(&str, &str)]) -> AppResult<HttpResponse> {
    let body = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(fields.iter().copied())
        .finish();
    request(
        url,
        "application/x-www-form-urlencoded",
        &body,
        None,
        15_000,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    #[test]
    fn invalid_timeout_reports_winhttp_stage_without_sending_credentials() {
        // -2 是 WinHTTP 不接受的逾時值；用真正的 API 失敗驗證診斷，
        // 並確認錯誤在建立網路連線前返回，不會傳送測試憑證或登入本文。
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = Url::parse(&format!(
            "http://{}/device?code=private-query",
            listener.local_addr().unwrap()
        ))
        .unwrap();
        let error = request_method(
            &url,
            "POST",
            "application/x-www-form-urlencoded",
            "device_code=private-body",
            Some(("Authorization", "Bearer private-token")),
            -2,
        )
        .err()
        .expect("invalid timeout must fail");
        assert!(
            error.contains("Windows 網路錯誤 87（WinHttpSetTimeouts）"),
            "{error}"
        );
        assert!(!error.contains("private-"));
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn binary_download_is_bounded_and_rejects_html_redirect_and_truncation() {
        for (status, mime, body, expected, success) in [
            (200, "application/octet-stream", "MZpayload", 9, true),
            (200, "application/octet-stream", "MZpayload", 8, false),
            (200, "application/octet-stream", "MZpayload", 10, false),
            (200, "text/html", "MZpayload", 9, false),
            (302, "application/octet-stream", "MZpayload", 9, false),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = Url::parse(&format!(
                "http://{}/installer",
                listener.local_addr().unwrap()
            ))
            .unwrap();
            let server = thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buffer = [0u8; 4096];
                let n = stream.read(&mut buffer).unwrap();
                assert!(!String::from_utf8_lossy(&buffer[..n]).contains("Authorization:"));
                let response = format!("HTTP/1.1 {status} Test\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                stream.write_all(response.as_bytes()).unwrap();
            });
            let path = std::env::temp_dir().join(format!(
                "LM_AI-download-test-{}",
                crate::jobs::new_id().unwrap()
            ));
            assert_eq!(download_file(&url, &path, expected).is_ok(), success);
            if success {
                assert_eq!(std::fs::read(&path).unwrap(), body.as_bytes());
            }
            std::fs::remove_file(path).unwrap();
            server.join().unwrap();
        }
    }

    #[test]
    fn sse_delivers_first_event_before_server_finishes() {
        // 回授握手比固定延遲更能識別緩衝：伺服器必須等客戶端收到首筆事件才送 done。
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url =
            url::Url::parse(&format!("http://{}/stream", listener.local_addr().unwrap())).unwrap();
        let (seen, received) = std::sync::mpsc::channel();
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 1024];
            loop {
                let n = stream.read(&mut buffer).unwrap();
                assert!(n > 0);
                request.extend_from_slice(&buffer[..n]);
                if request
                    .windows(4)
                    .position(|b| b == b"\r\n\r\n")
                    .is_some_and(|end| request.len() >= end + 6)
                {
                    break;
                }
            }
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n").unwrap();
            let first =
                "event: tool_status\ndata: {\"tool_name\":\"search\",\"status\":\"started\"}\n\n";
            write!(stream, "{:x}\r\n{}\r\n", first.len(), first).unwrap();
            stream.flush().unwrap();
            let immediate = received
                .recv_timeout(std::time::Duration::from_secs(3))
                .is_ok();
            let last = "event: delta\ndata: {\"text\":\"中文\"}\n\nevent: done\ndata: {\"event\":\"done\"}\n\n";
            write!(stream, "{:x}\r\n{}\r\n0\r\n\r\n", last.len(), last).unwrap();
            immediate
        });
        let mut parser = crate::jobs::SseDecoder::default();
        let mut text = String::new();
        let mut callback = |bytes: &[u8]| {
            parser.push(bytes, |event, data| {
                if event == "tool_status" {
                    let _ = seen.send(());
                }
                if event == "delta" {
                    text.push_str(
                        serde_json::from_str::<serde_json::Value>(data).unwrap()["text"]
                            .as_str()
                            .unwrap(),
                    );
                }
                Ok(())
            })?;
            Ok(parser.done)
        };
        let mut body = std::io::Cursor::new(b"{}");
        exchange(
            &url,
            "POST",
            "application/json",
            Payload {
                reader: &mut body,
                length: 2,
            },
            None,
            5000,
            Some(&mut callback),
        )
        .unwrap();
        assert!(worker.join().unwrap(), "SSE was buffered until completion");
        assert_eq!(text, "中文");
        assert!(parser.done);
    }

    #[test]
    fn redirect_is_rejected_before_a_second_request() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = Url::parse(&format!("http://{}/chat", listener.local_addr().unwrap())).unwrap();
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            loop {
                let mut bytes = [0; 4096];
                let count = stream.read(&mut bytes).unwrap();
                assert!(count > 0);
                request.extend_from_slice(&bytes[..count]);
                if request
                    .windows(4)
                    .position(|part| part == b"\r\n\r\n")
                    .is_some_and(|end| request.len() >= end + 4 + 2)
                {
                    break;
                }
            }
            stream.write_all(b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/leak\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
        });
        let result = request(
            &url,
            "application/json",
            "{}",
            Some(("Authorization", "Bearer fake-test-key")),
            1000,
        );
        assert!(result.err().unwrap().contains("302"));
        worker.join().unwrap();
    }
}
