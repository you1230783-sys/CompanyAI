//! 真實 loopback HTTP 整合測試；不呼叫公司服務或使用個人憑證。
use crate::{
    attachments::{Attachment, AttachmentStatus},
    jobs::{self, StreamUpdate, Task, TaskStatus, WorkStore},
    protocol::{Message, TokenResponse},
    storage::Session,
};
use std::io::Cursor;

#[test]
fn attachments_background_stream_recovery_cancel_and_ownership() {
    let server = super::super::DemoServer::start().unwrap();
    let config = server.config();
    server
        .state
        .lock()
        .unwrap()
        .tokens
        .extend(["work-owner".into(), "other-owner".into()]);
    let session = Session::from_token(
        TokenResponse {
            access_token: "work-owner".into(),
            token_type: "Bearer".into(),
            expires_in: 3600,
        },
        &config,
    )
    .unwrap();
    let other = Session::from_token(
        TokenResponse {
            access_token: "other-owner".into(),
            token_type: "Bearer".into(),
            expires_in: 3600,
        },
        &config,
    )
    .unwrap();
    let caps = jobs::capabilities(&config, &session).unwrap();
    assert!(caps.supports("stream"));
    assert_eq!(caps.attachments.max_count, 20);
    let local = jobs::new_id().unwrap();
    let remote = jobs::conversation(&config, &session, &local).unwrap();
    assert_eq!(
        remote,
        jobs::conversation(&config, &session, &local).unwrap()
    );
    let mut file = Attachment {
        id: jobs::new_id().unwrap(),
        conversation_id: local.clone(),
        name: "圖.png".into(),
        size: 6,
        mime_type: "image/png".into(),
        remote: None,
        state: "upload_pending".into(),
        message: String::new(),
        sent: false,
        removed: false,
        uploaded_bytes: 0,
    };
    let reserved = jobs::reserve_attachment(&config, &session, &remote, &file).unwrap();
    assert_eq!(
        reserved.job_id,
        jobs::reserve_attachment(&config, &session, &remote, &file)
            .unwrap()
            .job_id
    );
    file.apply(reserved).unwrap();
    let path = format!(
        "{}/attachments/{}",
        jobs::PREFIX,
        file.remote.as_ref().unwrap().job_id
    );
    // 非 UTF-8 的原始位元組也必須原封不動上傳。
    let bytes = [0, 255, 128, 13, 10, 1];
    let mut reader = Cursor::new(bytes);
    let response = crate::transport::exchange(
        &config.endpoint(&format!("{path}/content")).unwrap(),
        "PUT",
        "application/octet-stream",
        crate::transport::Payload {
            reader: &mut reader,
            length: 6,
        },
        Some(("Authorization", "Bearer work-owner")),
        5000,
        None,
    )
    .unwrap();
    let uploaded: AttachmentStatus = jobs::decode(response, &session).unwrap();
    assert_eq!(uploaded.state, "queued");
    file.apply(jobs::get(&config, &session, &path).unwrap())
        .unwrap();
    assert_eq!(file.state, "processing");
    file.apply(jobs::get(&config, &session, &path).unwrap())
        .unwrap();
    assert_eq!(file.state, "ready");
    assert!(jobs::get::<AttachmentStatus>(&config, &other, &path).is_err());
    let make_task = |mode: &str| {
        let id = jobs::new_id().unwrap();
        Task {
            request_id: id.clone(),
            conversation_id: local.clone(),
            request: jobs::chat_request(
                "quality",
                &[Message::user("附件測試")],
                &remote,
                &id,
                mode,
                vec![file.token().unwrap().into()],
            )
            .unwrap(),
            mode: mode.into(),
            title: "測試".into(),
            created_at: crate::unix_now(),
            remote: None,
            applied: false,
            message: String::new(),
            mail_analysis: false,
            partial: String::new(),
        }
    };
    let mut task = make_task("background");
    jobs::submit(&config, &session, &task.clone(), |event| {
        if let StreamUpdate::Status(status) = event {
            task.apply_status(*status).unwrap();
        }
    })
    .unwrap();
    let first = task.remote.as_ref().unwrap().task_id.clone();
    assert_eq!(task.remote.as_ref().unwrap().state, "queued");
    jobs::submit(&config, &session, &task.clone(), |event| {
        if let StreamUpdate::Status(status) = event {
            assert_eq!(status.task_id, first);
        }
    })
    .unwrap();
    let root = std::env::temp_dir().join(format!("lm-jobs-{}", jobs::new_id().unwrap()));
    let store = WorkStore {
        principal_id: caps.principal_id.clone(),
        tasks: vec![task.clone()],
        ..WorkStore::default()
    };
    store.save(&root, &config).unwrap();
    let restored = WorkStore::load(&root, &config, &caps.principal_id).unwrap();
    assert_eq!(restored.tasks[0].request_id, task.request_id);
    assert!(WorkStore::load(&root, &config, "another_account")
        .unwrap()
        .tasks
        .is_empty());
    for _ in 0..3 {
        task.apply_status(jobs::task_status(&config, &session, &task).unwrap())
            .unwrap();
    }
    assert_eq!(task.remote.as_ref().unwrap().state, "completed");
    assert!(jobs::task_status(&config, &other, &task).is_err());
    // 模擬歷史已寫入、任務 applied 尚未寫入時當機，重啟後不能再加一份回答。
    let mut archive = crate::history::Archive::default();
    let mut user_message = Message::user("附件測試");
    user_message.request_id = Some(task.request_id.clone());
    let local_history = archive.insert(vec![user_message]).unwrap();
    let mut delivery = task.clone();
    delivery.conversation_id = local_history;
    assert!(jobs::apply_reply(&mut archive, &delivery).unwrap());
    crate::history::save(&root, &archive).unwrap();
    let mut recovered = crate::history::load(&root).unwrap();
    assert!(!jobs::apply_reply(&mut recovered, &delivery).unwrap());
    assert_eq!(recovered.conversations[0].messages.len(), 2);
    let mut missing_ack = task.clone();
    missing_ack.remote = None;
    assert_eq!(
        jobs::task_status(&config, &session, &missing_ack)
            .unwrap()
            .task_id,
        first
    );
    let mut stream_task = make_task("stream");
    let mut text = String::new();
    jobs::submit(
        &config,
        &session,
        &stream_task.clone(),
        |event| match event {
            StreamUpdate::Status(status) => stream_task.apply_status(*status).unwrap(),
            StreamUpdate::Delta(delta) => text.push_str(&delta),
        },
    )
    .unwrap();
    assert!(text.contains("附件測試"));
    let result = jobs::task_status(&config, &session, &stream_task).unwrap();
    assert_eq!(result.state, "completed");
    assert_eq!(
        crate::protocol::assistant_text(&result.result.unwrap().to_string()).unwrap(),
        text
    );
    let mut interrupted = make_task("stream");
    interrupted.request["messages"][0]["content"] = serde_json::json!("[demo:disconnect] 測試中斷");
    let mut partial = String::new();
    assert!(jobs::submit(
        &config,
        &session,
        &interrupted.clone(),
        |event| match event {
            StreamUpdate::Status(status) => interrupted.apply_status(*status).unwrap(),
            StreamUpdate::Delta(delta) => partial.push_str(&delta),
        }
    )
    .is_err());
    assert!(!partial.is_empty());
    assert_eq!(
        jobs::task_status(&config, &session, &interrupted)
            .unwrap()
            .state,
        "completed"
    );
    use sha2::Digest;
    assert_eq!(
        server
            .state
            .lock()
            .unwrap()
            .work
            .files
            .get(&file.remote.as_ref().unwrap().job_id)
            .unwrap()
            .digest,
        format!("{:x}", sha2::Sha256::digest(bytes))
    );
    let mut cancel = make_task("background");
    jobs::submit(&config, &session, &cancel.clone(), |event| {
        if let StreamUpdate::Status(status) = event {
            cancel.apply_status(*status).unwrap();
        }
    })
    .unwrap();
    let result: TaskStatus = jobs::post(
        &config,
        &session,
        &format!(
            "{}/tasks/{}/cancel",
            jobs::PREFIX,
            cancel.remote.as_ref().unwrap().task_id
        ),
        &serde_json::json!({}),
    )
    .unwrap();
    assert_eq!(result.state, "cancelled");
    std::fs::remove_dir_all(root).unwrap();
}
