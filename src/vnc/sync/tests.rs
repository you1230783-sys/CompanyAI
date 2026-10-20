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

/// 重現使用者提供的完整欄位與大型回應；IP 位於每筆資料後半部，不能因分段讀取而遺失。
fn detailed_machines(count: usize, prefix: &str) -> String {
    let machines: Vec<_> = (0..count)
        .map(|index| {
            json!({
                "machine_id":format!("{prefix}-{index}"), "machine_name":format!("{prefix}_{index}"),
                "label":"AAA_1", "model":"機台01", "eq_type":prefix,
                "x":"Z", "y":1, "w":2, "h":3, "floor":"2F",
                "status":"OK", "status_label":"OK", "machine_ip":"192.168.0.0",
                "robot_version":"1", "image_version":"2", "turnkey_version":"3",
                "last_report_time":"2026-09-23 18:59:53", "online":true, "tank":false,
                "tip":["忽略的資料", {"machine_ip":"這個欄位不可誤用"}]
            })
        })
        .collect();
    serde_json::to_string_pretty(&json!({
        "ok":true, "code":0, "message":"",
        "data":{"machines":machines,"other":{"machine_ip":null}}
    }))
    .unwrap()
}

#[test]
fn large_and_small_api_responses_preserve_every_ip_through_preview_and_import() {
    let large = detailed_machines(300, "AAA");
    let small = detailed_machines(70, "BBB");
    assert!(large.lines().count() > 7_000);
    assert!(small.lines().count() > 1_800);
    assert!(large.len() > 8192 * 10);
    let (settings, worker) = server(vec![
        response(200, "Set-Cookie: PHPSESSID=first\r\n", ""),
        response(200, "Set-Cookie: PHPSESSID=second\r\n", ""),
        response(200, "", &large),
        response(200, "", &small),
        response(200, "", ""),
    ]);
    let data = download(&settings, &AtomicBool::new(false)).unwrap();
    assert_eq!(worker.join().unwrap().len(), 5);
    assert_eq!(data.machines.len(), 370);
    // 與原生層提供給預覽的 JSON 相同；逐筆檢查而非只檢查第一台。
    let preview = json!({"id":"test-preview","machines":data.machines});
    for (index, entry) in preview["machines"].as_array().unwrap().iter().enumerate() {
        let (prefix, number) = if index < 300 {
            ("AAA", index)
        } else {
            ("BBB", index - 300)
        };
        assert_eq!(entry["name"], format!("{prefix}_{number}"));
        assert_eq!(entry["ip"], "192.168.0.0");
    }
    let fixture = Fixture::new();
    let mut manager = fixture.manager();
    manager
        .import(&data, &(0..370).collect::<Vec<_>>())
        .unwrap();
    let restored = fixture.manager();
    assert_eq!(restored.machines["AAA"].len(), 300);
    assert_eq!(restored.machines["BBB"].len(), 70);
    assert!(restored
        .machines
        .values()
        .flatten()
        .all(|m| m.ip == "192.168.0.0"));
    eprintln!(
        "large API: {} lines / {} bytes; small API: {} lines / {} bytes; all 370 IPs retained",
        large.lines().count(),
        large.len(),
        small.lines().count(),
        small.len()
    );
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
            // Windows 接受的 socket 可能繼承 listener 的 nonblocking 狀態；
            // 讀取完整測試請求前切回 blocking，避免並行測試偶發 WouldBlock。
            socket.set_nonblocking(false).unwrap();
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

#[test]
fn natural_sort_and_name_comparison_control_imports() {
    let mut groups = vec!["未分類", "Z99", "A10", "A2", "B01", "A01"];
    groups.sort_by(|a, b| group_cmp(a, b));
    assert_eq!(groups, ["A01", "A2", "A10", "B01", "Z99", "未分類"]);
    let mut names = vec!["A01-AB", "A01-10", "A01-AA", "A01-2", "A01-01"];
    names.sort_by(|a, b| natural_cmp(a, b));
    assert_eq!(names, ["A01-01", "A01-2", "A01-10", "A01-AA", "A01-AB"]);
    let fixture = Fixture::new();
    let mut manager = fixture.manager();
    manager
        .save_machines(BTreeMap::from([(
            "自訂分類".into(),
            vec![Machine {
                name: "A01-01".into(),
                ip: "192.0.2.1".into(),
                password: "keep-me".into(),
                extra: Map::new(),
            }],
        )]))
        .unwrap();
    let mut data = Download {
        machines: vec![RemoteMachine {
            id: "new-id".into(),
            group: "A01".into(),
            name: "A01-01".into(),
            ip: "192.0.2.1".into(),
        }],
        source: "http://example.invalid/".into(),
        warning: String::new(),
    };
    assert_eq!(manager.comparison(&data.machines[0]), "same");
    assert_eq!(manager.import(&data, &[0]).unwrap(), 0);
    data.machines[0].ip = "192.0.2.2".into();
    assert_eq!(manager.comparison(&data.machines[0]), "changed");
    assert_eq!(manager.import(&data, &[0]).unwrap(), 1);
    assert_eq!(manager.machines["自訂分類"][0].ip, "192.0.2.2");
    assert_eq!(manager.machines["自訂分類"][0].password, "keep-me");
    assert!(!manager.machines.contains_key("A01"));
    data.machines[0].id = "another".into();
    data.machines[0].name = "A01-AB".into();
    assert_eq!(manager.comparison(&data.machines[0]), "new");
    manager.import(&data, &[0]).unwrap();
    data.machines[0].id = "numeric".into();
    data.machines[0].name = "A01-10".into();
    manager.import(&data, &[0]).unwrap();
    assert_eq!(manager.machines["A01"][0].name, "A01-10");
    assert_eq!(manager.machines["A01"][1].name, "A01-AB");
    // 網站不同 ID 的同名新機台不可在同一批匯入時互相覆蓋。
    data.machines = ["C01", "C02"]
        .iter()
        .map(|group| RemoteMachine {
            id: group.to_string(),
            group: group.to_string(),
            name: "同名新機台".into(),
            ip: "192.0.2.3".into(),
        })
        .collect();
    assert_eq!(manager.import(&data, &[0, 1]).unwrap(), 2);
    assert_eq!(manager.machines["C01"].len(), 1);
    assert_eq!(manager.machines["C02"].len(), 1);
    // 沒有 ID 可判別時，多筆同名不得任意選一台覆寫。
    data.machines[0].id = "unknown".into();
    data.machines[0].ip = "192.0.2.4".into();
    let before = fs::read(&manager.path).unwrap();
    assert!(manager.import(&data, &[0]).is_err());
    assert_eq!(fs::read(&manager.path).unwrap(), before);
}

#[test]
fn preview_keeps_session_auto_retries_once_and_manual_refresh_does_not_login_again() {
    use std::sync::mpsc;
    let mut unclassified: Value = serde_json::from_str(&machine("1", "A01-01", "")).unwrap();
    unclassified["data"]["machines"][0]["eq_type"] = json!("");
    let unclassified = unclassified.to_string();
    let normal = machine("2", "B01-01", "192.0.2.2");
    let (settings, server_worker) = server(vec![
        response(200, "Set-Cookie: PHPSESSID=first\r\n", ""),
        response(200, "Set-Cookie: PHPSESSID=second\r\n", ""),
        response(200, "", &unclassified),
        response(200, "", &normal),
        // 第二輪仍有 50% 未分類，也必須停止自動請求。
        response(200, "Set-Cookie: PHPSESSID=third\r\n", &unclassified),
        response(200, "", &normal),
        response(200, "", &unclassified),
        response(200, "", &normal),
        response(200, "", ""),
    ]);
    let (commands, receiver) = mpsc::channel();
    let (events, results) = mpsc::channel();
    let worker = thread::spawn(move || {
        run_session(&settings, &AtomicBool::new(false), receiver, |event| {
            events.send(event).is_ok()
        })
    });
    let SessionEvent::Ready(first) = results.recv_timeout(Duration::from_secs(10)).unwrap() else {
        panic!("expected preview");
    };
    assert!(first.warning.contains("已自動重新取得一次"));
    assert_eq!(first.machines.last().unwrap().group, "未分類");
    assert!(results.recv_timeout(Duration::from_millis(400)).is_err());
    assert!(
        !server_worker.is_finished(),
        "must not log out while preview is open"
    );
    commands.send(SessionCommand::Refresh).unwrap();
    let SessionEvent::Ready(second) = results.recv_timeout(Duration::from_secs(10)).unwrap() else {
        panic!("expected refreshed preview");
    };
    assert!(second.warning.is_empty());
    commands.send(SessionCommand::Close).unwrap();
    assert!(matches!(
        results.recv_timeout(Duration::from_secs(10)).unwrap(),
        SessionEvent::Finished(Ok(()))
    ));
    worker.join().unwrap();
    let requests = server_worker.join().unwrap();
    assert_eq!(requests.len(), 9);
    for request in &requests[2..8] {
        assert!(request
            .1
            .starts_with("GET /root/api/machine/info_map.php?floor="));
    }
    assert!(requests[6].1.contains("PHPSESSID=third"));
    assert!(requests[8].1.starts_with("POST /root/logout.php "));
}

#[test]
fn refresh_failure_and_abandoned_preview_both_log_out() {
    use std::sync::mpsc;
    for fail_refresh in [false, true] {
        let mut responses = vec![
            response(200, "Set-Cookie: PHPSESSID=first\r\n", ""),
            response(200, "Set-Cookie: PHPSESSID=second\r\n", ""),
            response(200, "", &machine("1", "A01-01", "192.0.2.1")),
            response(200, "", &machine("2", "B01-01", "192.0.2.2")),
        ];
        if fail_refresh {
            responses.push(response(401, "", ""));
        }
        responses.push(response(200, "", ""));
        let (settings, server_worker) = server(responses);
        let (commands, receiver) = mpsc::channel();
        let (events, results) = mpsc::channel();
        let worker = thread::spawn(move || {
            run_session(&settings, &AtomicBool::new(false), receiver, |event| {
                events.send(event).is_ok()
            })
        });
        assert!(matches!(
            results.recv_timeout(Duration::from_secs(10)).unwrap(),
            SessionEvent::Ready(_)
        ));
        if fail_refresh {
            commands.send(SessionCommand::Refresh).unwrap();
        }
        drop(commands); // 模擬停用或關閉 UI 通道，背景不能遺留登入。
        let SessionEvent::Finished(result) = results.recv_timeout(Duration::from_secs(10)).unwrap()
        else {
            panic!("expected finish");
        };
        assert_eq!(result.is_err(), fail_refresh);
        worker.join().unwrap();
        assert!(server_worker
            .join()
            .unwrap()
            .last()
            .unwrap()
            .1
            .starts_with("POST /root/logout.php "));
    }
}
