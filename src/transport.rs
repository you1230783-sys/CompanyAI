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
    fn checked(value: *mut c_void) -> AppResult<Self> {
        if value.is_null() {
            Err(network_error())
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

fn network_error() -> String {
    let code = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
    match code {
        12002 => "連線逾時。請確認 API 可從這台電腦連線；請求可能已送達，重試前請留意。".into(),
        12007 => "無法解析主機名稱，請檢查內網 DNS 與網站網址。".into(),
        12175 => {
            "HTTPS 憑證驗證失敗。請由公司 IT 安裝正確的信任憑證；程式不會略過憑證驗證。".into()
        }
        _ => format!("Windows 網路錯誤 {code}。請確認網址、VPN、代理伺服器與防火牆。"),
    }
}

/// 發送一次 HTTP 請求，不自動重試或跟隨重新導向，避免重複呼叫或把憑證帶到別站。
pub fn request(
    url: &Url,
    content_type: &str,
    body: &str,
    auth: Option<(&str, &str)>,
    timeout_ms: i32,
) -> AppResult<HttpResponse> {
    let host = wide(url.host_str().ok_or("網址缺少主機名稱。")?);
    let path = match url.query() {
        Some(query) => format!("{}?{query}", url.path()),
        None => url.path().to_string(),
    };
    let mut headers = format!("Content-Type: {content_type}\r\nAccept: application/json\r\n");
    if let Some((name, value)) = auth {
        if !matches!(name, "Authorization" | "X-API-Key") || value.contains(['\r', '\n', '\0']) {
            return Err("驗證 Header 無效。".into());
        }
        headers.push_str(&format!("{name}: {value}\r\n"));
    }
    // 本機示範不經代理，避免系統 PAC 或企業代理把 loopback 請求轉送出去。
    let proxy_mode = if matches!(url.host_str(), Some("127.0.0.1" | "[::1]" | "localhost")) {
        WINHTTP_ACCESS_TYPE_NO_PROXY
    } else {
        WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY
    };
    // SAFETY: 傳入的 UTF-16 字串與 body 在整次同步請求期間有效；所有輸出緩衝區有明確長度。
    unsafe {
        let session = Handle::checked(WinHttpOpen(
            wide("CompanyAI/0.2").as_ptr(),
            proxy_mode,
            ptr::null(),
            ptr::null(),
            0,
        ))?;
        if WinHttpSetTimeouts(session.0, 10_000, 10_000, 15_000, timeout_ms) == 0 {
            return Err(network_error());
        }
        let connection = Handle::checked(WinHttpConnect(
            session.0,
            host.as_ptr(),
            url.port_or_known_default().ok_or("網址埠號無效。")?,
            0,
        ))?;
        let flags = if url.scheme() == "https" {
            WINHTTP_FLAG_SECURE
        } else {
            0
        };
        let request = Handle::checked(WinHttpOpenRequest(
            connection.0,
            wide("POST").as_ptr(),
            wide(&path).as_ptr(),
            ptr::null(),
            ptr::null(),
            ptr::null(),
            flags,
        ))?;
        let policy = WINHTTP_OPTION_REDIRECT_POLICY_NEVER;
        if WinHttpSetOption(
            request.0,
            WINHTTP_OPTION_REDIRECT_POLICY,
            (&policy as *const u32).cast(),
            4,
        ) == 0
        {
            return Err(network_error());
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
            return Err(network_error());
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
            || WinHttpReceiveResponse(request.0, ptr::null_mut()) == 0
        {
            return Err(network_error());
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
            return Err(network_error());
        }
        if (300..400).contains(&status) {
            return Err(format!(
                "HTTP {status} 重新導向已停止。請填寫 API 的最終網址，不可導向登入 HTML 頁面。"
            ));
        }
        let mut bytes = Vec::new();
        loop {
            let mut chunk = [0_u8; 8192];
            let mut read = 0_u32;
            if WinHttpReadData(
                request.0,
                chunk.as_mut_ptr().cast(),
                chunk.len() as u32,
                &mut read,
            ) == 0
            {
                return Err(network_error());
            }
            if read == 0 {
                break;
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
