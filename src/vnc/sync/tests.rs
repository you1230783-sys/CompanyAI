use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::Arc,
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".build/vnc-sync-tests")
            .join(crate::jobs::new_id().unwrap());
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn manager(&self) -> Manager {
        Manager::load(self.0.join("machines.json")).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn response(status: u32, cookie: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status} Test\r\nConnection: close\r\nContent-Length: {}\r\n{cookie}\r\n{body}",
        body.len()
    )
}
fn machine(id: &str, name: &str, ip: &str) -> String {
    json!({"ok":true,"code":0,"data":{"machines":[{"machine_id":id,"machine_name":name,"machine_ip":ip,"eq_type":"加工","tip":["ignored-secret"]}]}}).to_string()
}

/// 真正 WinHTTP 對 loopback，逐筆接收且有截止時間，避免失敗時測試永久卡住。
fn server(responses: Vec<String>) -> (Settings, thread::JoinHandle<Vec<(Instant, String)>>) {
    server_with_cancel(responses, None)
}

fn server_with_cancel(
    responses: Vec<String>,
    cancel_after_login: Option<Arc<AtomicBool>>,
) -> (Settings, thread::JoinHandle<Vec<(Instant, String)>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let settings = Settings {
        root: format!("http://{}/root", listener.local_addr().unwrap()),
        username: "user&1".into(),
        password: "test+密碼".into(),
        ..Settings::default()
    };
    let worker = thread::spawn(move || {
        let mut requests = Vec::new();
        for response in responses {
            let until = Instant::now() + Duration::from_secs(10);
            let mut socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < until =>
                    {
                        thread::sleep(Duration::from_millis(10))
                    }
                    Err(e) => panic!("loopback accept: {e}"),
                }
            };
            let time = Instant::now();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut buffer = [0u8; 4096];
                let n = socket.read(&mut buffer).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buffer[..n]);
                if let Some(end) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
                    let length = headers
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .map(|s| s.trim().parse::<usize>().unwrap())
                        .unwrap_or(0);
                    if bytes.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            requests.push((time, String::from_utf8(bytes).unwrap()));
            if requests.len() == 2 {
                if let Some(cancel) = &cancel_after_login {
                    cancel.store(true, Ordering::Relaxed);
                }
            }
            socket.write_all(response.as_bytes()).unwrap();
        }
        requests
    });
    (settings, worker)
}

#[test]
fn cancellation_after_login_logs_out_without_fetching_apis() {
    let cancel = Arc::new(AtomicBool::new(false));
    let (settings, worker) = server_with_cancel(
        vec![
            response(200, "Set-Cookie: PHPSESSID=one\r\n", ""),
            response(200, "Set-Cookie: PHPSESSID=two\r\n", ""),
            response(200, "", ""),
        ],
        Some(cancel.clone()),
    );
    assert!(download(&settings, &cancel).is_err());
    let requests = worker.join().unwrap();
    assert!(
        requests[2].1.starts_with("POST /root/logout.php ")
            && requests[2].1.contains("PHPSESSID=two")
    );
}

#[test]
fn rejected_login_is_not_retried_and_logout_failure_is_visible() {
    let (settings, worker) = server(vec![
        response(200, "Set-Cookie: PHPSESSID=one\r\n", ""),
        response(200, "", r#"{"ok":false,"message":"private-error"}"#),
        response(500, "", ""),
    ]);
    let error = download(&settings, &AtomicBool::new(false)).err().unwrap();
    assert!(error.contains("登入") && error.contains("登出"));
    assert!(!error.contains("private-error"));
    assert_eq!(worker.join().unwrap().len(), 3);
}

#[test]
fn malformed_json_and_conflicting_duplicate_ids_are_rejected() {
    assert!(parse_machines("<html>login</html>").is_err());
    assert!(parse_machines(r#"{"ok":true,"code":0,"data":{}}"#).is_err());
    assert!(parse_machines(&machine("1", "有名稱", "/listen")).is_err());
    let empty = r#"{"ok":true,"code":0,"data":{"machines":[{"machine_name":"","machine_ip":""}]}}"#;
    assert!(parse_machines(empty).unwrap().is_empty());
    let (settings, worker) = server(vec![
        response(200, "Set-Cookie: PHPSESSID=one\r\n", ""),
        response(200, "Set-Cookie: PHPSESSID=two\r\n", ""),
        response(200, "", &machine("1", "同一台", "192.0.2.1")),
        response(200, "", &machine("1", "同一台", "192.0.2.2")),
        response(200, "", ""),
    ]);
    assert!(download(&settings, &AtomicBool::new(false))
        .err()
        .unwrap()
        .contains("衝突"));
    assert!(worker.join().unwrap()[4]
        .1
        .starts_with("POST /root/logout.php "));
}

#[test]
fn real_http_cookie_rotation_spacing_form_and_logout() {
    let (settings, worker) = server(vec![
        response(200, "Set-Cookie: PHPSESSID=first; Path=/root\r\n", "home"),
        response(
            302,
            "Set-Cookie: PHPSESSID=second; Path=/root\r\nLocation: /never-follow\r\n",
            "",
        ),
        response(
            200,
            "Set-Cookie: PHPSESSID=third; Path=/root\r\n",
            &machine("1", "機台一", "192.0.2.1"),
        ),
        response(200, "", &machine("2", "無IP機台", "")),
        response(200, "Set-Cookie: PHPSESSID=; Max-Age=0\r\n", ""),
    ]);
    let data = download(&settings, &AtomicBool::new(false)).unwrap();
    let requests = worker.join().unwrap();
    assert_eq!(data.machines.len(), 2);
    assert_eq!(data.machines[1].ip, "");
    assert!(data.warning.is_empty());
    assert!(!serde_json::to_string(&data.machines)
        .unwrap()
        .contains("ignored-secret"));
    for pair in requests.windows(2) {
        assert!(pair[1].0.duration_since(pair[0].0) >= Duration::from_millis(200));
    }
    assert!(requests[0]
        .1
        .starts_with("GET /root/?p=machineInfoMap&v=2 "));
    assert!(!requests[0].1.contains("Cookie:"));
    assert!(requests[1].1.contains("Cookie: PHPSESSID=first"));
    assert!(requests[1]
        .1
        .contains("uid=user%261&pwd=test%2B%E5%AF%86%E7%A2%BC&type=1"));
    assert!(requests[2].1.contains("Cookie: PHPSESSID=second"));
    assert!(requests[3].1.contains("Cookie: PHPSESSID=third"));
    assert!(requests[4].1.starts_with("POST /root/logout.php "));
    assert!(requests[4].1.contains("Cookie: PHPSESSID=third"));
}

#[test]
fn api_failure_still_logs_out_and_does_not_return_partial_data() {
    let (settings, worker) = server(vec![
        response(200, "Set-Cookie: PHPSESSID=first\r\n", ""),
        response(200, "Set-Cookie: PHPSESSID=second\r\n", ""),
        response(401, "", "secret-server-error"),
        response(200, "", ""),
    ]);
    let error = download(&settings, &AtomicBool::new(false)).err().unwrap();
    assert!(error.contains("401"));
    assert!(!error.contains("secret-server-error"));
    assert!(worker.join().unwrap()[3]
        .1
        .starts_with("POST /root/logout.php "));
}

#[test]
fn settings_are_encrypted_and_links_cannot_escape_root() {
    let fixture = Fixture::new();
    let settings = Settings {
        username: "remember-user".into(),
        password: "remember-secret".into(),
        ..Settings::default()
    };
    settings.save(&fixture.0).unwrap();
    assert!(
        !String::from_utf8_lossy(&fs::read(fixture.0.join("vnc-sync.dpapi")).unwrap())
            .contains("remember-secret")
    );
    let loaded = Settings::load(&fixture.0).unwrap();
    assert_eq!(loaded.password, settings.password);
    assert_eq!(loaded.endpoints.len(), 10);
    assert!(!loaded.public().to_string().contains("remember-secret"));
    for path in [
        "../login.php",
        "https://evil.invalid/x",
        "//evil.invalid/x",
        "/outside",
        "..%2flogin.php",
        "login.php#fragment",
    ] {
        // 百分比編碼的分隔符也不能繞過根目錄界線。
        assert!(loaded.endpoint(path).is_err(), "{path}");
    }
    assert_eq!(
        loaded
            .endpoint("api/machine/info_map.php?floor=2F")
            .unwrap()
            .path(),
        "/transdata/api/machine/info_map.php"
    );
}

#[test]
fn selected_import_preserves_manual_data_password_and_stable_identity() {
    let fixture = Fixture::new();
    let mut manager = fixture.manager();
    let mut machines = Machines::new();
    machines.insert(
        "手動".into(),
        vec![Machine {
            name: "自訂".into(),
            ip: "192.0.2.9".into(),
            password: "custom".into(),
            extra: Map::new(),
        }],
    );
    manager.save_machines(machines).unwrap();
    let mut data = Download {
        machines: vec![
            RemoteMachine {
                id: "1".into(),
                group: "加工".into(),
                name: "新機台".into(),
                ip: "".into(),
            },
            RemoteMachine {
                id: "2".into(),
                group: "不要匯入".into(),
                name: "其他".into(),
                ip: "".into(),
            },
        ],
        source: "http://example.invalid/root/".into(),
        warning: String::new(),
    };
    manager.import(&data, &[0]).unwrap();
    assert!(!manager.machines.contains_key("不要匯入"));
    assert_eq!(manager.machines["加工"][0].password, "1234");
    assert!(connection_args(&manager.machines["加工"][0], &Options::default()).is_err());
    let mut changed = manager.machines.clone();
    changed.get_mut("加工").unwrap()[0].password = "my-password".into();
    manager.save_machines(changed).unwrap();
    data.machines[0].name = "新名稱".into();
    data.machines[0].ip = "192.0.2.3".into();
    manager.import(&data, &[0]).unwrap();
    let restored = fixture.manager();
    assert_eq!(restored.machines["加工"].len(), 1);
    assert_eq!(restored.machines["加工"][0].password, "my-password");
    assert_eq!(restored.machines["加工"][0].name, "新名稱");
    assert_eq!(restored.machines["手動"][0].password, "custom");
    let before = fs::read(&manager.path).unwrap();
    assert!(manager.import(&data, &[100]).is_err());
    assert_eq!(fs::read(&manager.path).unwrap(), before);
}

#[test]
fn batch_order_and_delete_persist_and_reject_stale_indices() {
    let fixture = Fixture::new();
    let mut manager = fixture.manager();
    let mut machines = Machines::new();
    for group in ["A", "B", "C"] {
        machines.insert(
            group.into(),
            (0..4)
                .map(|i| Machine {
                    name: i.to_string(),
                    ip: String::new(),
                    password: String::new(),
                    extra: Map::new(),
                })
                .collect(),
        );
    }
    manager.save_machines(machines).unwrap();
    let selection = Selection {
        machines: vec![
            MachineKey {
                group: "A".into(),
                index: 1,
            },
            MachineKey {
                group: "A".into(),
                index: 2,
            },
        ],
        groups: vec![],
    };
    manager.batch_machines(&selection, "up").unwrap();
    assert_eq!(
        manager.machines["A"]
            .iter()
            .map(|m| m.name.as_str())
            .collect::<Vec<_>>(),
        vec!["1", "2", "0", "3"]
    );
    manager
        .move_groups(&["B".into(), "C".into()], true)
        .unwrap();
    assert_eq!(fixture.manager().ordered_groups(), vec!["B", "C", "A"]);
    manager
        .batch_machines(
            &Selection {
                machines: vec![],
                groups: vec!["B".into()],
            },
            "delete",
        )
        .unwrap();
    assert!(!fixture.manager().machines.contains_key("B"));
    let bytes = fs::read(&manager.path).unwrap();
    assert!(manager
        .batch_machines(
            &Selection {
                machines: vec![MachineKey {
                    group: "A".into(),
                    index: 99
                }],
                groups: vec![]
            },
            "delete"
        )
        .is_err());
    assert_eq!(bytes, fs::read(&manager.path).unwrap());
}
