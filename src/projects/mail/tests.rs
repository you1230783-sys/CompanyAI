//! 固定假郵件來源；不連接真實帳號，驗證同意、分層閱讀、快照及去重界線。
use super::*;
#[derive(Default)]
struct Fixture {
    calls: usize,
    bodies: usize,
    incomplete: bool,
}
fn folder(id: &str) -> Folder {
    Folder {
        id: id.into(),
        name: "工作".into(),
        path: format!("本地/{id}"),
        scope: "local_inbox".into(),
        store: "PRIVATE_STORE_ID".into(),
        entry: format!("PRIVATE_FOLDER_{id}"),
        children: 0,
        readable: true,
        excluded: vec![],
    }
}
impl Source for Fixture {
    fn folders(
        &mut self,
        _: &str,
        _: Option<&Folder>,
        _: &AtomicBool,
    ) -> AppResult<(Vec<Folder>, Vec<String>)> {
        self.calls += 1;
        Ok((vec![folder("first"), folder("second")], vec![]))
    }
    fn headers(
        &mut self,
        folder: &Folder,
        start: NaiveDate,
        end: NaiveDate,
        _: &AtomicBool,
    ) -> AppResult<Scan> {
        self.calls += 1;
        assert_eq!(start.to_string(), "2026-06-22");
        assert_eq!(end.to_string(), "2026-06-28");
        let mut headers = Vec::new();
        for index in 0..85 {
            let sent = format!("2026-06-23 12:{:02}:{:02}.000", index / 60, index % 60);
            let sender = "sender@example.test".to_owned();
            let recipients = vec!["to:reader@example.test".into()];
            headers.push(Header {
                thread_id: "thread".into(),
                id: jobs::new_id()?,
                folder_id: folder.id.clone(),
                subject: format!("工作 {index}"),
                sender: sender.clone(),
                recipients: recipients.clone(),
                sent_at: sent.clone(),
                received_at: sent.clone(),
                entry: format!("PRIVATE_MAIL_{index}"),
                modified: sent.clone(),
                duplicate_key: duplicate_key(&sent, &sender, &recipients),
            });
        }
        let mut copy = headers[0].clone();
        copy.id = jobs::new_id()?;
        copy.entry = "PRIVATE_COPY".into();
        headers.push(copy);
        Ok(Scan {
            headers,
            complete: !self.incomplete,
            notices: if self.incomplete {
                vec!["測試讀取失敗".into()]
            } else {
                vec![]
            },
        })
    }
    fn body(&mut self, _: &Folder, _: &Header, _: &AtomicBool) -> AppResult<String> {
        self.calls += 1;
        self.bodies += 1;
        Ok("中".repeat(20_000))
    }
}
fn allowed() -> Session {
    let mut session = Session::default();
    session.set_consent(Some(Box::new(|_, _| {
        Ok(Some(crate::outlook::privacy::Policy::default()))
    })));
    assert!(session
        .authorize(&AtomicBool::new(false), Instant::now())
        .unwrap());
    session
}
#[test]
fn refusal_and_restoration_never_grant_mail_access() {
    let cancel = AtomicBool::new(false);
    let mut source = Fixture::default();
    let mut session = Session::default();
    assert!(!session.authorize(&cancel, Instant::now()).unwrap());
    assert!(session
        .folders(&mut source, "local_inbox", None, 0, &cancel)
        .is_err());
    assert!(session
        .headers(
            &mut source,
            "first",
            "2026-06-22",
            "2026-06-28",
            None,
            &cancel
        )
        .is_err());
    assert_eq!(source.calls, 0);
    let mut session = allowed();
    session
        .folders(&mut source, "local_inbox", None, 0, &cancel)
        .unwrap();
    let saved = serde_json::to_vec(&session.saved).unwrap();
    let mut restored = Session {
        saved: serde_json::from_slice(&saved).unwrap(),
        ..Default::default()
    };
    assert!(restored
        .headers(
            &mut source,
            "first",
            "2026-06-22",
            "2026-06-28",
            None,
            &cancel
        )
        .is_err());
    assert_eq!(source.calls, 1);
}
#[test]
fn folder_then_headers_then_body_are_bounded_and_deduplicated() {
    let cancel = AtomicBool::new(false);
    let mut source = Fixture::default();
    let mut session = allowed();
    let folders = session
        .folders(&mut source, "local_inbox", None, 0, &cancel)
        .unwrap();
    assert!(!folders.to_string().contains("PRIVATE_"));
    assert!(session
        .folders(&mut source, "online_sent", Some("first"), 0, &cancel)
        .is_err());
    assert!(session
        .headers(
            &mut source,
            "unknown",
            "2026-06-22",
            "2026-06-28",
            None,
            &cancel
        )
        .is_err());
    let mut page = session
        .headers(
            &mut source,
            "first",
            "2026-06-22",
            "2026-06-28",
            None,
            &cancel,
        )
        .unwrap();
    assert_eq!(page["total_unique"], 85);
    assert_eq!(page["duplicates_omitted"], 1);
    assert_eq!(source.bodies, 0);
    let mail = page["headers"][0]["mail_id"].as_str().unwrap().to_owned();
    let mut ids = std::collections::BTreeSet::new();
    loop {
        assert!(!page.to_string().contains("PRIVATE_"));
        for h in page["headers"].as_array().unwrap() {
            assert!(ids.insert(h["mail_id"].as_str().unwrap().to_owned()));
        }
        let Some(cursor) = page["next_cursor"].as_str() else {
            break;
        };
        assert!(session
            .headers(
                &mut source,
                "second",
                "2026-06-22",
                "2026-06-28",
                Some(cursor),
                &cancel
            )
            .is_err());
        page = session
            .headers(
                &mut source,
                "first",
                "2026-06-22",
                "2026-06-28",
                Some(cursor),
                &cancel,
            )
            .unwrap();
    }
    assert_eq!(ids.len(), 85);
    assert_eq!(page["complete"], true);
    assert_eq!(source.calls, 2);
    let duplicates = session
        .headers(
            &mut source,
            "second",
            "2026-06-22",
            "2026-06-28",
            None,
            &cancel,
        )
        .unwrap();
    assert_eq!(duplicates["total_unique"], 0);
    assert_eq!(duplicates["duplicates_omitted"], 86);
    let repeat = session
        .headers(
            &mut source,
            "first",
            "2026-06-22",
            "2026-06-28",
            None,
            &cancel,
        )
        .unwrap();
    assert_eq!(repeat["total_unique"], 85);
    assert!(session.body(&mut source, "unknown", 0, &cancel).is_err());
    let first = session.body(&mut source, &mail, 0, &cancel).unwrap();
    let next = session.body(&mut source, &mail, 12_000, &cancel).unwrap();
    assert_eq!(first["text"].as_str().unwrap().chars().count(), 12_000);
    assert_eq!(next["text"].as_str().unwrap().chars().count(), 8_000);
    assert_eq!(next["has_more"], false);
    assert_eq!(source.bodies, 1);
}
#[test]
fn dedup_uses_exact_time_and_addresses_but_ignores_order_and_case() {
    let a = duplicate_key(
        "2026-06-23 12:25:48.084",
        "SENDER@example.test",
        &["to:A@example.test".into(), "cc:b@example.test".into()],
    );
    let b = duplicate_key(
        "2026-06-23 12:25:48.084",
        "sender@example.test",
        &["cc:B@example.test".into(), "to:a@example.test".into()],
    );
    assert_eq!(a, b);
    assert_ne!(
        a,
        duplicate_key(
            "2026-06-23 12:25:48.085",
            "sender@example.test",
            &["to:a@example.test".into(), "cc:b@example.test".into()]
        )
    );
    assert!(duplicate_key("time", "/O=EXCHANGE", &["to:a@example.test".into()]).is_none());
}
#[test]
fn incomplete_scan_is_not_a_complete_weekly_report() {
    let cancel = AtomicBool::new(false);
    let mut source = Fixture {
        incomplete: true,
        ..Default::default()
    };
    let mut session = allowed();
    session
        .folders(&mut source, "local_inbox", None, 0, &cancel)
        .unwrap();
    let page = session
        .headers(
            &mut source,
            "first",
            "2026-06-22",
            "2026-06-28",
            None,
            &cancel,
        )
        .unwrap();
    assert_eq!(page["scan_complete"], false);
    assert_eq!(page["notices"][0], "測試讀取失敗");
}

#[test]
fn selected_folders_are_filtered_before_model_output_and_changed_scope_rejects_resume() {
    let mut source = Fixture::default();
    let allowed_folder = folder("first");
    let policy = crate::outlook::privacy::Policy {
        configured: true,
        allowed: [crate::outlook::privacy::key(
            &allowed_folder.store,
            &allowed_folder.entry,
        )]
        .into_iter()
        .collect(),
    };
    let mut session = Session::default();
    session.set_consent(Some(Box::new(move |_, _| Ok(Some(policy.clone())))));
    let cancel = AtomicBool::new(false);
    session.authorize(&cancel, Instant::now()).unwrap();
    let result = session
        .folders(&mut source, "local_inbox", None, 0, &cancel)
        .unwrap();
    assert_eq!(result["total"], 1);
    assert!(!result.to_string().contains("second"));
    assert!(!result.to_string().contains("PRIVATE_STORE"));
    let before = source.calls;
    assert!(session
        .headers(
            &mut source,
            "second",
            "2026-06-22",
            "2026-06-28",
            None,
            &cancel
        )
        .is_err());
    assert_eq!(source.calls, before);
    let mut resumed = Session {
        saved: session.saved,
        ..Default::default()
    };
    resumed.set_consent(Some(Box::new(|_, _| {
        Ok(Some(crate::outlook::privacy::Policy {
            configured: true,
            allowed: Default::default(),
        }))
    })));
    assert!(resumed.authorize(&cancel, Instant::now()).is_err());
    assert!(!resumed.is_allowed());
}

#[test]
fn empty_cursor_and_saved_header_search_recover_exact_ids_without_body_reads() {
    let cancel = AtomicBool::new(false);
    let mut source = Fixture::default();
    let mut session = allowed();
    session
        .folders(&mut source, "local_inbox", None, 0, &cancel)
        .unwrap();
    let page = session
        .headers(
            &mut source,
            "first",
            "2026-06-22",
            "2026-06-28",
            Some(""),
            &cancel,
        )
        .unwrap();
    let id = page["headers"][0]["mail_id"].as_str().unwrap();
    let found = session.index(&id[..8], 0).unwrap();
    assert_eq!(found["headers"][0]["mail_id"], id);
    assert!(!found.to_string().contains("PRIVATE_"));
    assert_eq!(source.bodies, 0);
    let calls = source.calls;
    session.index("工作", 20).unwrap();
    assert_eq!(source.calls, calls);
    let restored = Session {
        saved: session.saved,
        ..Default::default()
    };
    assert!(restored.index("", 0).is_err());
}
