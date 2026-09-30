//! 契約伺服器回傳固定 Markdown，驗證 broker 真正走上傳、版本快取與 TXT 成果。
use company_ai::{
    config::Config,
    projects::{files::Broker, sandbox::Worker, text, Project, Tool},
    protocol::TokenResponse,
    storage::Session,
    AppResult,
};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::TcpListener,
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

pub fn verify(exe: &Path, parent: &Path) -> AppResult<()> {
    let root = parent.join("server-pdf");
    std::fs::create_dir(&root).map_err(|e| e.to_string())?;
    let source = root.join("測試.pdf");
    std::fs::write(&source, b"opaque-original-1").map_err(|e| e.to_string())?;
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let config = Config {
        server_url: format!(
            "http://{}/company/",
            listener.local_addr().map_err(|e| e.to_string())?
        ),
        token_path: "api/desktop/oauth/token".into(),
        ..Config::default()
    };
    let session = Session::from_token(
        TokenResponse {
            access_token: "fixture-token".into(),
            token_type: "Bearer".into(),
            expires_in: 3600,
        },
        &config,
    )?;
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let done = Arc::new(AtomicBool::new(false));
    let stop = done.clone();
    let server = std::thread::spawn(move || -> AppResult<()> {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !stop.load(Ordering::Relaxed) && Instant::now() < deadline {
            let (mut stream, _) = match listener.accept() {
                Ok(pair) => pair,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(e) => return Err(e.to_string()),
            };
            // Windows accept 可能繼承 listener 的非阻塞模式；此 fixture 逐筆同步處理。
            stream.set_nonblocking(false).map_err(|e| e.to_string())?;
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .map_err(|e| e.to_string())?;
            let mut bytes = Vec::new();
            loop {
                let mut byte = [0];
                stream.read_exact(&mut byte).map_err(|e| e.to_string())?;
                bytes.push(byte[0]);
                if bytes.ends_with(b"\r\n\r\n") {
                    break;
                }
                if bytes.len() > 8192 {
                    return Err("fixture header too large".into());
                }
            }
            let header = String::from_utf8(bytes).map_err(|e| e.to_string())?;
            assert!(header.starts_with("POST /company/api/desktop/documents/pdf-to-markdown "));
            let length: usize = header
                .lines()
                .find_map(|line| {
                    line.split_once(':')
                        .filter(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                        .and_then(|(_, v)| v.trim().parse().ok())
                })
                .ok_or("missing length")?;
            let mut body = vec![0; length];
            stream.read_exact(&mut body).map_err(|e| e.to_string())?;
            let call = calls.fetch_add(1, Ordering::SeqCst) + 1;
            assert!(String::from_utf8_lossy(&body).contains("opaque-original-"));
            let markdown = format!("# 轉換 {call}\n\nPDF 中文內容\n");
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: text/markdown; charset=utf-8\r\nX-Request-ID: test-{call}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",markdown.len(),markdown).map_err(|e|e.to_string())?;
        }
        Ok(())
    });
    let result: AppResult<()> = (|| {
        let mut broker = Broker::new(
            Project {
                id: "server-pdf".into(),
                name: "server-pdf".into(),
                root: root.clone(),
                imports: BTreeMap::new(),
            },
            "pdf-task".into(),
        )?;
        broker.enable_server_pdf(config.clone(), session.clone())?;
        let cancel = AtomicBool::new(false);
        let mut worker = Worker::start(exe, &cancel)?;
        let mut call = |id: &str, tool: Tool| -> AppResult<serde_json::Value> {
            let result = broker.execute(id, &tool, &mut worker, &cancel)?;
            if result["ok"] != true {
                return Err(result.to_string());
            }
            Ok(result["result"].clone())
        };
        let read = call(
            "read1",
            Tool::ReadFile {
                path: "測試.pdf".into(),
                offset: 0,
            },
        )?;
        assert!(read["text"].as_str().unwrap().starts_with("# 轉換 1"));
        call(
            "read2",
            Tool::ReadFile {
                path: "測試.pdf".into(),
                offset: 3,
            },
        )?;
        call(
            "find",
            Tool::FindText {
                path: "測試.pdf".into(),
                text: "中文".into(),
            },
        )?;
        let copy = call(
            "copy",
            Tool::CreateWorkingCopy {
                source: Some("測試.pdf".into()),
                name: "摘要.txt".into(),
            },
        )?;
        assert_eq!(
            count.load(Ordering::SeqCst),
            1,
            "閱讀／搜尋／副本不得重複轉換"
        );
        let id = copy["copy_id"].as_str().unwrap().to_owned();
        let revision = copy["revision"].as_str().unwrap().to_owned();
        let edited = call(
            "edit",
            Tool::EditText {
                copy_id: id.clone(),
                revision,
                start: 0,
                expected: "# 轉換 1".into(),
                replacement: "已整理".into(),
            },
        )?;
        let saved = call(
            "save",
            Tool::SaveCopy {
                copy_id: id.clone(),
                revision: edited["revision"].as_str().unwrap().into(),
            },
        )?;
        assert!(saved.to_string().contains("摘要.txt"));
        assert_eq!(
            std::fs::read(&source).map_err(|e| e.to_string())?,
            b"opaque-original-1"
        );
        std::fs::write(&source, b"opaque-original-2").map_err(|e| e.to_string())?;
        let changed = call(
            "changed",
            Tool::ReadFile {
                path: "測試.pdf".into(),
                offset: 0,
            },
        )?;
        assert!(changed["text"].as_str().unwrap().starts_with("# 轉換 2"));
        assert_eq!(count.load(Ordering::SeqCst), 2);
        let outputs = broker.finish(&[id])?;
        let bytes = std::fs::read(root.join(&outputs[0])).map_err(|e| e.to_string())?;
        assert_eq!(text::decode(&bytes)?.0, "已整理\n\nPDF 中文內容\n");
        assert_eq!(
            std::fs::read_dir(&root).map_err(|e| e.to_string())?.count(),
            3,
            "只新增加密 .lmai，不得產生明文 MD 快取"
        );
        let mut reopened = Broker::new(
            Project {
                id: "reopened".into(),
                name: "reopened".into(),
                root: root.clone(),
                imports: BTreeMap::new(),
            },
            "second_task".into(),
        )?;
        reopened.enable_server_pdf(config.clone(), session.clone())?;
        let cached = reopened.execute(
            "cached",
            &Tool::ReadFile {
                path: "測試.pdf".into(),
                offset: 0,
            },
            &mut worker,
            &cancel,
        )?;
        assert_eq!(cached["ok"], true);
        assert_eq!(count.load(Ordering::SeqCst), 2, "第二次任務應重用磁碟快取");
        let profile = format!("{}:pdf-markdown-v1-no-images", config.pdf_endpoint()?);
        let key = text::revision(&format!(
            "{}:{profile}",
            text::revision("opaque-original-2")
        ));
        let cache_file = root.join(".lmai/cache").join(format!("{key}.dpapi"));
        let encrypted = std::fs::read(&cache_file).map_err(|e| e.to_string())?;
        assert!(!String::from_utf8_lossy(&encrypted).contains("PDF 中文內容"));
        std::fs::write(cache_file, b"corrupt cache").map_err(|e| e.to_string())?;
        let mut recovery = Broker::new(
            Project {
                id: "recover".into(),
                name: "recover".into(),
                root: root.clone(),
                imports: BTreeMap::new(),
            },
            "third_task".into(),
        )?;
        recovery.enable_server_pdf(config, session)?;
        let fresh = recovery.execute(
            "fresh",
            &Tool::ReadFile {
                path: "測試.pdf".into(),
                offset: 0,
            },
            &mut worker,
            &cancel,
        )?;
        assert_eq!(fresh["ok"], true);
        assert_eq!(count.load(Ordering::SeqCst), 3, "損毀快取應重新轉換");
        Ok(())
    })();
    done.store(true, Ordering::Relaxed);
    server.join().map_err(|_| "PDF fixture thread failed")??;
    result?;
    println!("PASS: PDF multipart broker route, memory cache, changed-source invalidation, TXT edit/publish and source preserved.");
    Ok(())
}
