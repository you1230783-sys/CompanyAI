//! PDF 專用 multipart client。WinHTTP 使用非同步 I/O，讓等待轉換時可取消。
//! 對呼叫端仍為背景執行緒中的同步函式；不影響既有聊天／附件的 HTTP 行為。
use super::{network_error, uses_direct_connection, Handle};
use crate::{wide, AppResult};
use std::{
    ffi::c_void,
    io::Read,
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use url::Url;
use windows_sys::Win32::Networking::WinHttp::*;

pub const MAX_FILE: u64 = 52_428_800;
const TOTAL_TIMEOUT: Duration = Duration::from_secs(300);

pub struct Response {
    pub status: u32,
    pub body: Vec<u8>,
    pub content_type: String,
    pub request_id: String,
}

enum Event {
    Complete(u32, u32),
    Error(u32),
    Closed,
}

/// Callback 只傳回狀態，不操作檔案或 UI，也不在 callback 中遞迴呼叫 WinHTTP。
unsafe extern "system" fn callback(
    _: *mut c_void,
    context: usize,
    status: u32,
    info: *mut c_void,
    length: u32,
) {
    if context == 0 {
        return;
    }
    // 所有權由 Request 保留到 HANDLE_CLOSING。先 clone sender，送出 Closed 後
    // 不再接觸 context，避免接收端立即釋放時與 callback 尾端競爭。
    let sender = unsafe { &*(context as *const mpsc::Sender<Event>) }.clone();
    let event = match status {
        WINHTTP_CALLBACK_STATUS_HANDLE_CLOSING => Event::Closed,
        WINHTTP_CALLBACK_STATUS_REQUEST_ERROR
            if !info.is_null()
                && length as usize >= std::mem::size_of::<WINHTTP_ASYNC_RESULT>() =>
        {
            Event::Error(unsafe { (*(info as *const WINHTTP_ASYNC_RESULT)).dwError })
        }
        WINHTTP_CALLBACK_STATUS_READ_COMPLETE => Event::Complete(status, length),
        WINHTTP_CALLBACK_STATUS_WRITE_COMPLETE if !info.is_null() && length >= 4 => {
            Event::Complete(status, unsafe { *(info as *const u32) })
        }
        WINHTTP_CALLBACK_STATUS_SENDREQUEST_COMPLETE
        | WINHTTP_CALLBACK_STATUS_HEADERS_AVAILABLE => Event::Complete(status, 0),
        _ => return,
    };
    let _ = sender.send(event);
}

struct Request {
    raw: *mut c_void,
    _context: Box<mpsc::Sender<Event>>,
    events: mpsc::Receiver<Event>,
    // I/O buffer 必須活到完成 callback，取消時亦不能先釋放。
    buffer: Box<[u8; 65_536]>,
}
impl Request {
    fn new(handle: Handle) -> AppResult<Self> {
        let (sender, events) = mpsc::channel();
        let context = Box::new(sender);
        let pointer = (&*context as *const mpsc::Sender<Event>) as usize;
        unsafe {
            if WinHttpSetOption(
                handle.0,
                WINHTTP_OPTION_CONTEXT_VALUE,
                (&pointer as *const usize).cast(),
                std::mem::size_of::<usize>() as u32,
            ) == 0
            {
                return Err(network_error("WinHttpSetOption/PDF-context"));
            }
            let previous = WinHttpSetStatusCallback(
                handle.0,
                Some(callback),
                WINHTTP_CALLBACK_STATUS_SENDREQUEST_COMPLETE
                    | WINHTTP_CALLBACK_STATUS_HEADERS_AVAILABLE
                    | WINHTTP_CALLBACK_STATUS_READ_COMPLETE
                    | WINHTTP_CALLBACK_STATUS_WRITE_COMPLETE
                    | WINHTTP_CALLBACK_STATUS_REQUEST_ERROR
                    | WINHTTP_CALLBACK_STATUS_HANDLE_CLOSING,
                0,
            );
            if previous.map(|f| f as usize) == Some(usize::MAX) {
                return Err(network_error("WinHttpSetStatusCallback/PDF"));
            }
        }
        let raw = handle.0;
        std::mem::forget(handle);
        Ok(Self {
            raw,
            _context: context,
            events,
            buffer: Box::new([0; 65_536]),
        })
    }

    fn wait(&self, expected: u32, cancel: &AtomicBool, deadline: Instant) -> AppResult<u32> {
        loop {
            check(cancel, deadline)?;
            match self.events.recv_timeout(Duration::from_millis(50)) {
                Ok(Event::Complete(kind, length)) if kind == expected => return Ok(length),
                Ok(Event::Error(code)) => {
                    return Err(format!("PDF 轉換連線失敗（Windows {code}）；未自動重送。"))
                }
                Ok(_) => return Err("PDF 轉換連線狀態不符，已停止。".into()),
                Err(mpsc::RecvTimeoutError::Timeout) => (),
                Err(_) => return Err("PDF 轉換連線已關閉。".into()),
            }
        }
    }
}
impl Drop for Request {
    fn drop(&mut self) {
        // 只取消非同步 request。最後的 HANDLE_CLOSING 回呼抵達後，才可釋放
        // context 與 I/O buffer；不得在其他執行緒關閉同步 WinHTTP handle。
        unsafe {
            WinHttpCloseHandle(self.raw);
        }
        while let Ok(event) = self.events.recv() {
            if matches!(event, Event::Closed) {
                break;
            }
        }
    }
}

fn check(cancel: &AtomicBool, deadline: Instant) -> AppResult<()> {
    if cancel.load(Ordering::Relaxed) {
        return Err("PDF 轉換已取消；伺服器可能仍在處理，未自動重送。".into());
    }
    if Instant::now() >= deadline {
        return Err("PDF 上傳與轉換超過 300 秒；伺服器可能仍在處理，未自動重送。".into());
    }
    Ok(())
}

/// 僅讀取小型 Header；伺服器檔名不作為磁碟路徑，Markdown 留在任務記憶體快取。
fn header(request: &Request, name: &str) -> String {
    let mut buffer = [0u16; 1024];
    let mut size = (buffer.len() * 2) as u32;
    let name = wide(name);
    let ok = unsafe {
        WinHttpQueryHeaders(
            request.raw,
            WINHTTP_QUERY_CUSTOM,
            name.as_ptr(),
            buffer.as_mut_ptr().cast(),
            &mut size,
            ptr::null_mut(),
        )
    };
    if ok == 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buffer[..(size as usize / 2).min(buffer.len())])
        .trim_end_matches('\0')
        .to_string()
}

/// 單一 file 欄位，分塊讀取已由 broker 鎖定的 PDF。不輪詢、不自動重送。
pub fn convert(
    url: &Url,
    token: &str,
    name: &str,
    file: &mut dyn Read,
    length: u64,
    cancel: &AtomicBool,
) -> AppResult<Response> {
    convert_with_timeout(url, token, name, file, length, cancel, TOTAL_TIMEOUT)
}

#[allow(clippy::too_many_arguments)]
fn convert_with_timeout(
    url: &Url,
    token: &str,
    name: &str,
    file: &mut dyn Read,
    length: u64,
    cancel: &AtomicBool,
    timeout: Duration,
) -> AppResult<Response> {
    if length == 0 || length > MAX_FILE {
        return Err("PDF 須為 1 byte 至 50 MiB（52,428,800 bytes）。".into());
    }
    if token.is_empty() || !token.bytes().all(|b| b.is_ascii_graphic()) {
        return Err("PDF 轉換登入資料無效。".into());
    }
    if !name.to_ascii_lowercase().ends_with(".pdf")
        || name.len() > 1024
        || name.chars().any(|c| c.is_control() || "\"\\/".contains(c))
    {
        return Err("PDF 檔名無效。".into());
    }
    let deadline = Instant::now() + timeout;
    check(cancel, deadline)?;
    // 原生 WinHTTP 沒有 FormData：同一個 builder 同時產生 boundary、本文與 Header，
    // 不使用固定 boundary，也不讓呼叫端手動填入不相符的 Content-Type。
    let boundary = format!("CompanyAI{}", crate::jobs::new_id()?);
    let prefix = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: application/pdf\r\n\r\n");
    let suffix = format!("\r\n--{boundary}--\r\n");
    let total = prefix.len() as u64 + length + suffix.len() as u64;
    let mut payload = std::io::Cursor::new(prefix.as_bytes())
        .chain(file.take(length))
        .chain(std::io::Cursor::new(suffix.as_bytes()));
    unsafe {
        let session = Handle::checked(
            "WinHttpOpen/PDF",
            WinHttpOpen(
                wide(concat!("LM_AI/", env!("CARGO_PKG_VERSION"))).as_ptr(),
                if uses_direct_connection(url) {
                    WINHTTP_ACCESS_TYPE_NO_PROXY
                } else {
                    WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY
                },
                ptr::null(),
                ptr::null(),
                WINHTTP_FLAG_ASYNC,
            ),
        )?;
        if WinHttpSetTimeouts(session.0, 10_000, 15_000, 120_000, 180_000) == 0 {
            return Err(network_error("WinHttpSetTimeouts/PDF"));
        }
        let connection = Handle::checked(
            "WinHttpConnect/PDF",
            WinHttpConnect(
                session.0,
                wide(url.host_str().ok_or("PDF 主機無效。")?).as_ptr(),
                url.port_or_known_default().ok_or("PDF 埠號無效。")?,
                0,
            ),
        )?;
        let handle = Handle::checked(
            "WinHttpOpenRequest/PDF",
            WinHttpOpenRequest(
                connection.0,
                wide("POST").as_ptr(),
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
        let disabled =
            WINHTTP_DISABLE_REDIRECTS | WINHTTP_DISABLE_COOKIES | WINHTTP_DISABLE_AUTHENTICATION;
        if WinHttpSetOption(
            handle.0,
            WINHTTP_OPTION_DISABLE_FEATURE,
            (&disabled as *const u32).cast(),
            4,
        ) == 0
        {
            return Err(network_error("WinHttpSetOption/PDF-policy"));
        }
        let mut request = Request::new(handle)?;
        let headers = wide(&format!("Authorization: Bearer {token}\r\nX-Client-Version: {}\r\nAccept: text/markdown\r\nContent-Type: multipart/form-data; boundary={boundary}\r\n", env!("CARGO_PKG_VERSION")));
        if WinHttpSendRequest(
            request.raw,
            headers.as_ptr(),
            (headers.len() - 1) as u32,
            ptr::null(),
            0,
            total as u32,
            (&*request._context as *const mpsc::Sender<Event>) as usize,
        ) == 0
        {
            return Err(network_error("WinHttpSendRequest/PDF"));
        }
        request.wait(
            WINHTTP_CALLBACK_STATUS_SENDREQUEST_COMPLETE,
            cancel,
            deadline,
        )?;
        let mut remaining = total as usize;
        while remaining > 0 {
            check(cancel, deadline)?;
            let wanted = remaining.min(request.buffer.len());
            let count = payload
                .read(&mut request.buffer[..wanted])
                .map_err(|e| format!("PDF 上傳讀取失敗：{e}"))?;
            if count == 0 {
                return Err("PDF 上傳內容不完整。".into());
            }
            let mut offset = 0;
            while offset < count {
                if WinHttpWriteData(
                    request.raw,
                    request.buffer[offset..count].as_ptr().cast(),
                    (count - offset) as u32,
                    ptr::null_mut(),
                ) == 0
                {
                    return Err(network_error("WinHttpWriteData/PDF"));
                }
                let written =
                    request.wait(WINHTTP_CALLBACK_STATUS_WRITE_COMPLETE, cancel, deadline)?
                        as usize;
                if written == 0 || written > count - offset {
                    return Err("PDF 上傳進度無效。".into());
                }
                offset += written;
            }
            remaining -= count;
        }
        if WinHttpReceiveResponse(request.raw, ptr::null_mut()) == 0 {
            return Err(network_error("WinHttpReceiveResponse/PDF"));
        }
        request.wait(WINHTTP_CALLBACK_STATUS_HEADERS_AVAILABLE, cancel, deadline)?;
        let mut status = 0u32;
        let mut size = 4;
        if WinHttpQueryHeaders(
            request.raw,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            ptr::null(),
            (&mut status as *mut u32).cast(),
            &mut size,
            ptr::null_mut(),
        ) == 0
        {
            return Err(network_error("WinHttpQueryHeaders/PDF-status"));
        }
        let content_type = header(&request, "Content-Type");
        let request_id = header(&request, "X-Request-ID");
        let limit = if status == 200 {
            crate::projects::text::MAX_TEXT + 3
        } else {
            65_536
        };
        let mut body = Vec::new();
        loop {
            check(cancel, deadline)?;
            if WinHttpReadData(
                request.raw,
                request.buffer.as_mut_ptr().cast(),
                request.buffer.len() as u32,
                ptr::null_mut(),
            ) == 0
            {
                return Err(network_error("WinHttpReadData/PDF"));
            }
            let count =
                request.wait(WINHTTP_CALLBACK_STATUS_READ_COMPLETE, cancel, deadline)? as usize;
            if count == 0 {
                break;
            }
            if count > request.buffer.len() || body.len() + count > limit {
                return Err(
                    "PDF 轉換回應超過大小限制（Markdown 上限 200 KB），請拆分文件。".into(),
                );
            }
            body.extend_from_slice(&request.buffer[..count]);
        }
        check(cancel, deadline)?;
        Ok(Response {
            status,
            body,
            content_type,
            request_id,
        })
    }
}

#[cfg(test)]
mod tests;
