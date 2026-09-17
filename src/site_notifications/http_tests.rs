//! 真實 WinHTTP loopback 測試，驗證 Header、opaque ID、分頁及操作回應，不連公司網站。
use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};
fn backend(replies: Vec<(u32, String)>) -> (Config, Session, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let config = Config {
        server_url: format!("http://{}", listener.local_addr().unwrap()),
        ..Default::default()
    };
    let session = Session {
        access_token: "test-notification-secret".into(),
        expires_at: crate::unix_now() + 3600,
        binding: config.binding().unwrap(),
    };
    let worker = thread::spawn(move || {
        let mut requests = vec![];
        for (status, body) in replies {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut request = vec![];
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
                assert!(request.len() < 16000);
            }
            let request = String::from_utf8(request).unwrap();
            assert!(request.contains("Authorization: Bearer test-notification-secret"));
            assert!(request.contains(&format!("X-Client-Version: {}", env!("CARGO_PKG_VERSION"))));
            requests.push(request.lines().next().unwrap().to_string());
            let length = request
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .and_then(|v| v.trim().parse::<usize>().ok())
                })
                .unwrap_or(0);
            let mut body_bytes = vec![0; length];
            stream.read_exact(&mut body_bytes).unwrap();
            stream.write_all(format!("HTTP/1.1 {status} Result\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).unwrap();
        }
        requests
    });
    (config, session, worker)
}
fn response(id: &str, cursor: &str, more: bool, read: bool) -> String {
    serde_json::json!({"notifications":[{"id":id,"source":"kanban","type":"mention","title":"title","body":"body","created_at":"2026-09-17T12:00:00Z","is_read":read}],"next_cursor":cursor,"has_more":more,"unread_count":if read{0}else{2}}).to_string()
}
#[test]
fn paginated_sync_encoded_actions_and_server_read_state() {
    let (config, session, worker) = backend(vec![
        (200, response("kanban:1", "next+opaque", true, false)),
        (200, response("briefing:2", "done", false, false)),
        (204, String::new()),
        (204, String::new()),
        (204, String::new()),
        (200, response("kanban:1", "read-update", false, true)),
    ]);
    let cache = sync(
        &config,
        &session,
        &Cache::for_session(&config, &session).unwrap(),
    )
    .unwrap();
    assert_eq!(cache.items.len(), 2);
    assert_eq!(cache.unread_count, 2);
    action(
        &config,
        &session,
        &Action::Read {
            id: "kanban:1/a?b".into(),
            open: false,
        },
    )
    .unwrap();
    action(&config, &session, &Action::ReadAll).unwrap();
    action(&config, &session, &Action::DeleteAll).unwrap();
    let updated = sync(&config, &session, &cache).unwrap();
    assert!(
        updated
            .items
            .iter()
            .find(|n| n.id == "kanban:1")
            .unwrap()
            .is_read
    );
    let requests = worker.join().unwrap();
    assert!(requests[1].contains("after=next%2Bopaque"));
    assert!(requests[2].contains("kanban:1%2Fa%3Fb/read"));
    assert!(requests[3].starts_with("POST /lm_server/api/desktop/notifications/read-all"));
    assert!(requests[4].starts_with("DELETE /lm_server/api/desktop/notifications "));
}
#[test]
fn expired_cursor_rebuilds_without_losing_old_cache_on_failure() {
    let (config, session, worker) = backend(vec![
        (410, "{}".into()),
        (200, response("new:1", "fresh", false, false)),
        (500, "test-notification-secret".into()),
    ]);
    let mut old = Cache::for_session(&config, &session).unwrap();
    old.cursor = Some("expired".into());
    old.initialized = true;
    old.items = serde_json::from_str::<Page>(&response("old:1", "old", false, false))
        .unwrap()
        .notifications;
    let updated = sync(&config, &session, &old).unwrap();
    assert_eq!(updated.items[0].id, "new:1");
    assert_eq!(old.items[0].id, "old:1");
    assert!(sync(&config, &session, &updated).is_err());
    assert_eq!(updated.items[0].id, "new:1");
    let requests = worker.join().unwrap();
    assert!(!requests[1].contains("after="));
}
#[test]
fn status_codes_do_not_echo_server_secrets_or_mutate_cache() {
    let (config, session, worker) = backend(
        [401, 403, 404, 429, 500]
            .into_iter()
            .map(|s| (s, "test-notification-secret".into()))
            .collect(),
    );
    let cache = Cache::for_session(&config, &session).unwrap();
    for status in [401, 403, 404, 429, 500] {
        let error = sync(&config, &session, &cache).err().unwrap();
        assert_eq!(error.status, status);
        assert!(!error.message.contains(&session.access_token));
        assert!(!cache.initialized);
    }
    worker.join().unwrap();
    assert!(sync(&config, &session, &cache).is_err()); // 連線已關閉，同樣不破壞快取。
}
