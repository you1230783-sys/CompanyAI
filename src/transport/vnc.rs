//! VNC 清單網站專用的單次請求。Cookie 只在同步工作中手動傳遞，不與 AI 登入共用。
use super::*;
use std::time::{Duration, Instant};

pub(crate) struct Response {
    pub status: u32,
    pub body: String,
}

/// 每個回應都先更新 PHPSESSID，再讀本文；即使本文無效，呼叫端仍可用新 session 登出。
/// 不自動轉址／重試／使用 Windows 登入資訊，錯誤不包含帳密、Cookie 或回應內容。
pub(crate) fn request(
    url: &Url,
    method: &str,
    body: &str,
    cookie: &mut Option<String>,
) -> AppResult<Response> {
    let host = wide(url.host_str().ok_or("機台網站缺少主機名稱。")?);
    let path = match url.query() {
        Some(query) => format!("{}?{query}", url.path()),
        None => url.path().into(),
    };
    let mut headers = "Content-Type: application/x-www-form-urlencoded\r\nAccept: application/json, text/html\r\n".to_string();
    if let Some(value) = cookie.as_ref() {
        if value.is_empty()
            || value.len() > 256
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-,".contains(&b))
        {
            return Err("機台網站的 session 格式不正確。".into());
        }
        headers.push_str(&format!("Cookie: PHPSESSID={value}\r\n"));
    }
    // SAFETY: 所有輸入緩衝區在同步呼叫期間有效；Handle 擁有並關閉 WinHTTP 資源。
    unsafe {
        let session = open_session(url, "LM_AI/vnc-sync")?;
        if WinHttpSetTimeouts(session.0, 10_000, 10_000, 15_000, 15_000) == 0 {
            return Err(network_error("WinHttpSetTimeouts/vnc"));
        }
        let connection = Handle::checked(
            "WinHttpConnect/vnc",
            WinHttpConnect(
                session.0,
                host.as_ptr(),
                url.port_or_known_default().ok_or("機台網站埠號無效。")?,
                0,
            ),
        )?;
        let request = Handle::checked(
            "WinHttpOpenRequest/vnc",
            WinHttpOpenRequest(
                connection.0,
                wide(method).as_ptr(),
                wide(&path).as_ptr(),
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
        let disabled =
            WINHTTP_DISABLE_COOKIES | WINHTTP_DISABLE_REDIRECTS | WINHTTP_DISABLE_AUTHENTICATION;
        if WinHttpSetOption(
            request.0,
            WINHTTP_OPTION_DISABLE_FEATURE,
            (&disabled as *const u32).cast(),
            4,
        ) == 0
        {
            return Err(network_error("WinHttpSetOption/vnc"));
        }
        let headers = wide(&headers);
        if WinHttpSendRequest(
            request.0,
            headers.as_ptr(),
            (headers.len() - 1) as u32,
            body.as_ptr().cast(),
            body.len() as u32,
            body.len() as u32,
            0,
        ) == 0
        {
            return Err(network_error("WinHttpSendRequest/vnc"));
        }
        if WinHttpReceiveResponse(request.0, ptr::null_mut()) == 0 {
            return Err(network_error("WinHttpReceiveResponse/vnc"));
        }
        let mut status = 0u32;
        let mut size = 4;
        if WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            ptr::null(),
            (&mut status as *mut u32).cast(),
            &mut size,
            ptr::null_mut(),
        ) == 0
        {
            return Err(network_error("WinHttpQueryHeaders/vnc-status"));
        }
        let mut index = 0;
        for _ in 0..32 {
            let mut buffer = [0u16; 4096];
            let mut size = (buffer.len() * 2) as u32;
            if WinHttpQueryHeaders(
                request.0,
                WINHTTP_QUERY_SET_COOKIE,
                ptr::null(),
                buffer.as_mut_ptr().cast(),
                &mut size,
                &mut index,
            ) == 0
            {
                if std::io::Error::last_os_error().raw_os_error()
                    == Some(ERROR_WINHTTP_HEADER_NOT_FOUND as i32)
                {
                    break;
                }
                return Err(network_error("WinHttpQueryHeaders/vnc-cookie"));
            }
            let value = String::from_utf16_lossy(&buffer[..size as usize / 2]);
            if let Some(value) = value
                .trim_end_matches('\0')
                .split(';')
                .next()
                .and_then(|v| v.trim().strip_prefix("PHPSESSID="))
            {
                // 清空也是一種更新（登出）；不可沿用伺服器已撤銷的 session。
                *cookie = (!value.is_empty()).then(|| value.to_string());
            }
        }
        let started = Instant::now();
        let mut bytes = Vec::new();
        loop {
            let mut buffer = [0u8; 8192];
            let mut count = 0;
            if WinHttpReadData(
                request.0,
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
                &mut count,
            ) == 0
            {
                return Err(network_error("WinHttpReadData/vnc"));
            }
            if count == 0 {
                break;
            }
            if bytes.len() + count as usize > 4 * 1024 * 1024
                || started.elapsed() > Duration::from_secs(30)
            {
                return Err("機台網站回應超過 4 MiB 或讀取逾時。".into());
            }
            bytes.extend_from_slice(&buffer[..count as usize]);
        }
        // 首頁可能使用其他編碼；登入及資料 JSON 另由同步模組嚴格驗證結構。
        Ok(Response {
            status,
            body: String::from_utf8_lossy(&bytes).into_owned(),
        })
    }
}
