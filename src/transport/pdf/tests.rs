//! 真正 WinHTTP loopback：驗證 wire 格式與等待中的取消，沒有使用公司憑證。
use super::*;
use std::{
    io::{Cursor, Write},
    net::{TcpListener, TcpStream},
    sync::Arc,
    thread,
};

fn receive(stream: &mut TcpStream) -> (String, Vec<u8>) {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut header = Vec::new();
    while !header.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        stream.read_exact(&mut byte).unwrap();
        header.push(byte[0]);
        assert!(header.len() < 8192);
    }
    let header = String::from_utf8(header).unwrap();
    let length: usize = header
        .lines()
        .find_map(|line| {
            line.split_once(':')
                .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                .and_then(|(_, value)| value.trim().parse().ok())
        })
        .unwrap();
    let mut body = vec![0; length];
    stream.read_exact(&mut body).unwrap();
    (header, body)
}

#[test]
fn posts_one_multipart_file_and_receives_raw_markdown() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = Url::parse(&format!(
        "http://{}/company/api/desktop/documents/pdf-to-markdown",
        listener.local_addr().unwrap()
    ))
    .unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let (header, body) = receive(&mut stream);
        assert!(header.starts_with("POST /company/api/desktop/documents/pdf-to-markdown "));
        assert!(header.contains("Authorization: Bearer fixture-token\r\n"));
        assert!(header.contains(&format!(
            "X-Client-Version: {}\r\n",
            env!("CARGO_PKG_VERSION")
        )));
        let boundary = header
            .lines()
            .find_map(|l| l.split_once("boundary=").map(|(_, b)| b))
            .unwrap();
        let expected = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"公司 文件.pdf\"\r\nContent-Type: application/pdf\r\n\r\nopaque bytes\r\n--{boundary}--\r\n");
        assert_eq!(body, expected.as_bytes());
        let markdown = "# 測試\n\n| 項目 | 數值 |\n|---|---|\n| 中文 | 123 |\n";
        write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: text/markdown; charset=utf-8\r\nX-Request-ID: fixture-id\r\nContent-Disposition: attachment; filename*=UTF-8''report.md\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",markdown.len(),markdown).unwrap();
    });
    let reply = convert(
        &url,
        "fixture-token",
        "公司 文件.pdf",
        &mut Cursor::new(b"opaque bytes"),
        12,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(reply.status, 200);
    assert_eq!(reply.request_id, "fixture-id");
    assert!(String::from_utf8(reply.body).unwrap().contains("中文"));
    server.join().unwrap();
}

#[test]
fn cancel_and_deadline_close_an_inflight_conversion_without_retry() {
    for cancel_request in [true, false] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = Url::parse(&format!("http://{}/pdf", listener.local_addr().unwrap())).unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let signal = cancel.clone();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            receive(&mut stream);
            if cancel_request {
                signal.store(true, Ordering::Relaxed);
            }
            // client 必須關閉正在等 Header 的連線，不等伺服器 120 秒才返回。
            let mut byte = [0];
            let result = stream.read(&mut byte);
            assert!(
                matches!(result, Ok(0))
                    || result.is_err_and(|e| matches!(
                        e.kind(),
                        std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted
                    ))
            );
            listener.set_nonblocking(true).unwrap();
            assert!(listener.accept().is_err());
        });
        let started = Instant::now();
        let result = convert_with_timeout(
            &url,
            "fixture-token",
            "test.pdf",
            &mut Cursor::new(b"pdf"),
            3,
            &cancel,
            if cancel_request {
                Duration::from_secs(10)
            } else {
                Duration::from_millis(500)
            },
        );
        assert!(result.is_err());
        let error = result.err().unwrap();
        assert!(
            error.contains(if cancel_request { "取消" } else { "超過" }),
            "{error}"
        );
        assert!(started.elapsed() < Duration::from_secs(4));
        server.join().unwrap();
    }
}

#[test]
fn rejects_oversize_before_connecting_and_preserves_error_response() {
    let url = Url::parse("http://127.0.0.1:1/pdf").unwrap();
    let result = convert(
        &url,
        "fixture-token",
        "test.pdf",
        &mut std::io::empty(),
        MAX_FILE + 1,
        &AtomicBool::new(false),
    );
    assert!(result.err().unwrap().contains("50 MiB"));
    for status in [400, 401, 403, 413, 422, 502, 503, 504, 302] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = Url::parse(&format!("http://{}/pdf", listener.local_addr().unwrap())).unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            receive(&mut stream);
            let body = br#"{"error_code":"FIXTURE","message":"failed","request_id":"r1"}"#;
            write!(stream,"HTTP/1.1 {status} Error\r\nContent-Type: application/json\r\nLocation: http://127.0.0.1:1/leak\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).unwrap();
            stream.write_all(body).unwrap();
        });
        let reply = convert(
            &url,
            "fixture-token",
            "test.pdf",
            &mut Cursor::new(b"pdf"),
            3,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(reply.status, status);
        assert!(String::from_utf8(reply.body).unwrap().contains("FIXTURE"));
        server.join().unwrap();
    }
}

#[test]
fn streams_the_exact_fifty_mib_limit_and_bounds_markdown() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = Url::parse(&format!("http://{}/pdf", listener.local_addr().unwrap())).unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let (_, body) = receive(&mut stream);
        let start = body.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
        assert!(body[start..start + MAX_FILE as usize]
            .iter()
            .all(|b| *b == 0xa5));
        assert!(body[start + MAX_FILE as usize..].starts_with(b"\r\n--CompanyAI"));
        let response = vec![b'x'; crate::projects::text::MAX_TEXT + 4];
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/markdown\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", response.len()).unwrap();
        let _ = stream.write_all(&response);
    });
    let result = convert(
        &url,
        "fixture-token",
        "maximum.pdf",
        &mut std::io::repeat(0xa5),
        MAX_FILE,
        &AtomicBool::new(false),
    );
    assert!(result.err().unwrap().contains("200 KB"));
    server.join().unwrap();
}
