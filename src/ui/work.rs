//! 主視窗的附件／任務控制器。只有 UI 執行緒修改狀態與保存檔案；網路執行緒只回傳事件。
use super::*;
use crate::{
    attachments::{self, Attachment, AttachmentStatus, Incoming},
    jobs::{self, Capabilities, Task, TaskStatus, Timing, WorkStore},
};
use serde_json::Value;
use std::collections::HashSet;

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub(super) enum WorkCommand {
    FileBegin {
        name: String,
        size: u64,
        mime_type: String,
    },
    FileChunk {
        id: String,
        offset: u64,
        data: String,
    },
    FileFinish {
        id: String,
    },
    FileAbort {
        id: String,
    },
    RemoveAttachment {
        id: String,
    },
    RetryUpload {
        id: String,
    },
    Mode {
        mode: String,
    },
    Estimate {
        text: String,
    },
    CancelTask {
        id: String,
    },
    RetryTask {
        id: String,
    },
    Refresh,
}
pub(super) enum WorkEvent {
    Capabilities(AppResult<Capabilities>),
    Reserved(String, AppResult<(String, AttachmentStatus)>),
    Uploaded(String, AppResult<AttachmentStatus>),
    Attachment(String, AppResult<AttachmentStatus>),
    Progress(String, u64),
    Update(String, jobs::StreamUpdate),
    Submitted(String, AppResult<()>),
    Polled(String, AppResult<TaskStatus>),
    PollFinished,
    Estimated(u64, AppResult<Timing>),
}
pub(super) struct WorkRuntime {
    pub caps: Option<Capabilities>,
    pub store: WorkStore,
    pub incoming: Option<Incoming>,
    pub mode: String,
    pub status: String,
    pub estimate: Option<Timing>,
    pub estimate_revision: u64,
    pub uploads: HashSet<String>,
    pub streams: HashSet<String>,
    pub polling: bool,
    pub last_poll: Instant,
    pub poll_delay: u64,
    pub capability_loading: bool,
    pub storage_error: bool,
    pub task_balloon: bool,
}
impl Default for WorkRuntime {
    fn default() -> Self {
        Self {
            caps: None,
            store: WorkStore::default(),
            incoming: None,
            mode: "sync".into(),
            status: "網站尚未啟用進階聊天；純文字聊天仍可使用".into(),
            estimate: None,
            estimate_revision: 0,
            uploads: HashSet::new(),
            streams: HashSet::new(),
            polling: false,
            last_poll: Instant::now(),
            poll_delay: 5,
            capability_loading: false,
            storage_error: false,
            task_balloon: false,
        }
    }
}
impl App {
    pub(super) fn work_state(&self) -> Value {
        let attachments:Vec<_>=self.work.store.drafts(self.active_id.as_deref()).iter().map(|a|json!({
            "id":a.id,"name":a.name,"size":a.size,"state":if a.state=="ready"&&a.token().is_none(){"expired"}else{&a.state},
            "message":a.message,"uploaded_bytes":a.uploaded_bytes,
            "progress":a.remote.as_ref().and_then(|r|r.progress),"queue_position":a.remote.as_ref().and_then(|r|r.queue_position),
            "timing":a.remote.as_ref().map(|r|&r.timing)
        })).collect();
        let tasks:Vec<_>=self.work.store.tasks.iter().rev().map(|t|json!({
            "id":t.request_id,"conversation_id":t.conversation_id,"title":t.title,"mode":t.mode,"created_at":t.created_at,
            "state":t.remote.as_ref().map(|r|r.state.as_str()).unwrap_or("submitting"),"active":t.active(),"message":t.message,
            "progress":t.remote.as_ref().and_then(|r|r.progress),"queue_position":t.remote.as_ref().and_then(|r|r.queue_position),
            "timing":t.remote.as_ref().map(|r|&r.timing),"partial":if Some(&t.conversation_id)==self.active_id.as_ref(){t.partial.as_str()}else{""},
            "can_retry":t.remote.is_none()&&!self.work.streams.contains(&t.request_id)
        })).collect();
        // 身分代號、附件 Token 與完整請求不傳入 WebView2。
        json!({"attachments":attachments,"tasks":tasks,"rules":self.work.caps.as_ref().map(|c|&c.attachments),
            "modes":self.work.caps.as_ref().map(|c|&c.execution_modes),"mode":self.work.mode,"status":self.work.status,
            "draft_error":self.check_draft_files().err(),"estimate":self.work.estimate,"can_estimate":self.work.caps.as_ref().is_some_and(|c|c.timing_estimates),
            "pending":self.work.store.pending(self.active_id.as_deref()),"transferring":self.work.incoming.is_some()})
    }
    fn work_save(&mut self) -> AppResult<()> {
        if self.work.storage_error {
            return Err("任務紀錄無法保存，請重新啟動後檢查；不會送出新工作。".into());
        }
        if let Err(e) = self.work.store.save(&self.root, &self.config) {
            self.work.storage_error = true;
            return Err(e);
        }
        Ok(())
    }
    fn check_draft_files(&self) -> AppResult<()> {
        let files = self.work.store.drafts(self.active_id.as_deref());
        if files.is_empty() {
            return Ok(());
        }
        let cap = self
            .work
            .caps
            .as_ref()
            .ok_or("請重新整理網站能力後再送出附件。")?;
        let mut total = 0;
        for (index, file) in files.iter().enumerate() {
            cap.attachments.check(&file.name, file.size, index, total)?;
            total += file.size;
            if file.token().is_none() {
                return Err("附件尚未就緒或已到期；請查看附件狀態，完成處理後再送出。".into());
            }
        }
        Ok(())
    }
    pub(super) fn work_ready(&self) -> bool {
        !self.work.storage_error
            && self.work.incoming.is_none()
            && !self.work.store.pending(self.active_id.as_deref())
            && self.check_draft_files().is_ok()
    }
    fn ensure_local_conversation(&mut self) -> AppResult<String> {
        if self.active_id.is_none() {
            self.save_history()?;
        }
        self.active_id.clone().ok_or("無法建立本機對話。".into())
    }
    pub(super) fn refresh_work_capabilities(&mut self) {
        if self.smoke || self.work.capability_loading || !self.logged_in() {
            return;
        }
        self.work.capability_loading = true;
        let (config, session, tx, generation) = (
            self.config.clone(),
            self.session.clone(),
            self.tx.clone(),
            self.generation,
        );
        if let Some(session) = session {
            thread::spawn(move || {
                let _ = tx.send(Event::Work(
                    generation,
                    WorkEvent::Capabilities(jobs::capabilities(&config, &session)),
                ));
            });
        }
    }
    pub(super) fn work_command(&mut self, command: WorkCommand) -> AppResult<()> {
        match command {
            WorkCommand::FileBegin {
                name,
                size,
                mime_type,
            } => {
                let result = (|| {
                    if !self.logged_in()
                        || self.versions.blocked()
                        || self.busy != "none"
                        || self.work.incoming.is_some()
                        || self.work.store.pending(self.active_id.as_deref())
                    {
                        return Err("目前無法加入附件，請先完成操作或新增對話。".into());
                    }
                    let cap = self.work.caps.as_ref().ok_or("網站尚未提供附件能力。")?;
                    if self.work.mode == "sync" {
                        return Err("請先選擇串流或背景模式，再加入附件。".into());
                    }
                    let files = self.work.store.drafts(self.active_id.as_deref());
                    cap.attachments.check(
                        &name,
                        size,
                        files.len(),
                        files.iter().map(|a| a.size).sum(),
                    )?;
                    if mime_type.len() > 100 || mime_type.chars().any(char::is_control) {
                        return Err("附件類型不正確。".into());
                    }
                    let local = self.ensure_local_conversation()?;
                    let id = jobs::new_id()?;
                    let incoming = Incoming::new(&self.root, id.clone(), size)?;
                    self.work.store.attachments.push(Attachment {
                        id: id.clone(),
                        conversation_id: local,
                        name,
                        size,
                        mime_type,
                        remote: None,
                        state: "reading".into(),
                        message: String::new(),
                        sent: false,
                        removed: false,
                        uploaded_bytes: 0,
                    });
                    self.work_save()?;
                    self.work.incoming = Some(incoming);
                    Ok(id)
                })();
                match result {
                    Ok(id) => self
                        .view
                        .post(&json!({"type":"file_ack","id":id,"offset":0}))?,
                    Err(e) => {
                        self.view.post(&json!({"type":"file_error","message":e}))?;
                        return Err(e);
                    }
                }
            }
            WorkCommand::FileChunk { id, offset, data } => {
                let result = self
                    .work
                    .incoming
                    .as_mut()
                    .filter(|i| i.id == id)
                    .ok_or("找不到接收中的附件。".to_string())
                    .and_then(|i| {
                        i.append(offset, &data)?;
                        Ok(i.received)
                    });
                match result {
                    Ok(offset) => self
                        .view
                        .post(&json!({"type":"file_ack","id":id,"offset":offset}))?,
                    Err(e) => {
                        self.view.post(&json!({"type":"file_error","message":e}))?;
                        return Err(e);
                    }
                }
            }
            WorkCommand::FileFinish { id } => {
                let result = (|| {
                    if self.work.incoming.as_ref().is_none_or(|i| i.id != id) {
                        return Err("找不到接收中的附件。".into());
                    }
                    self.work
                        .incoming
                        .take()
                        .ok_or("附件接收已結束。")?
                        .finish()?;
                    let file = self
                        .work
                        .store
                        .attachments
                        .iter_mut()
                        .find(|a| a.id == id)
                        .ok_or("找不到附件。")?;
                    file.state = "upload_pending".into();
                    self.work_save()
                })();
                match result {
                    Ok(()) => self
                        .view
                        .post(&json!({"type":"file_ack","id":id,"finished":true}))?,
                    Err(e) => {
                        self.view.post(&json!({"type":"file_error","message":e}))?;
                        return Err(e);
                    }
                }
            }
            WorkCommand::FileAbort { id } => {
                if self.work.incoming.as_ref().is_some_and(|i| i.id == id) {
                    self.work.incoming = None;
                }
                if let Some(a) = self
                    .work
                    .store
                    .attachments
                    .iter_mut()
                    .find(|a| a.id == id && a.state == "reading")
                {
                    a.state = "failed".into();
                    a.message = "附件接收中止，請移除後重新選取。".into();
                }
                self.work_save()?;
            }
            WorkCommand::RemoveAttachment { id } => {
                if self.work.incoming.as_ref().is_some_and(|i| i.id == id) {
                    return Err("請等檔案接收完成。".into());
                }
                let a = self
                    .work
                    .store
                    .attachments
                    .iter_mut()
                    .find(|a| a.id == id && !a.sent)
                    .ok_or("找不到草稿附件。")?;
                a.removed = true;
                let remote = a.remote.clone();
                self.work_save()?;
                if !self.work.uploads.contains(&id) {
                    let _ = std::fs::remove_file(attachments::spool_path(&self.root, &id)?);
                }
                if let (Some(remote), Some(session)) = (remote, self.session.clone()) {
                    let config = self.config.clone();
                    thread::spawn(move || {
                        let _: AppResult<Value> = jobs::post(
                            &config,
                            &session,
                            &format!("{}/attachments/{}/cancel", jobs::PREFIX, remote.job_id),
                            &json!({}),
                        );
                    });
                }
            }
            WorkCommand::RetryUpload { id } => {
                if !self.logged_in() || self.versions.blocked() || self.work.uploads.contains(&id) {
                    return Err("目前無法重試附件。".into());
                }
                let a = self
                    .work
                    .store
                    .attachments
                    .iter_mut()
                    .find(|a| a.id == id && !a.sent && !a.removed)
                    .ok_or("找不到附件。")?;
                attachments::SpoolReader::open(&self.root, &id)?;
                a.state = "upload_pending".into();
                a.message.clear();
                self.work_save()?;
            }
            WorkCommand::Mode { mode } => {
                if mode == "sync" && !self.work.store.drafts(self.active_id.as_deref()).is_empty() {
                    return Err("附件請使用串流或背景模式；移除草稿附件後可切回一般回覆。".into());
                }
                if self.work.caps.as_ref().is_some_and(|c| c.supports(&mode))
                    && (mode != "sync"
                        || self.work.store.drafts(self.active_id.as_deref()).is_empty())
                {
                    self.work.mode = mode;
                    self.work.estimate = None;
                    self.work.estimate_revision += 1;
                }
            }
            WorkCommand::Estimate { text } => self.estimate_work(&text)?,
            WorkCommand::CancelTask { id } => {
                let task = self
                    .work
                    .store
                    .tasks
                    .iter()
                    .find(|t| t.request_id == id && t.active())
                    .ok_or("找不到進行中的工作。")?;
                let remote = task
                    .remote
                    .as_ref()
                    .ok_or("尚未確認伺服器是否接受工作，請先重新整理。")?;
                let path = format!("{}/tasks/{}/cancel", jobs::PREFIX, remote.task_id);
                let (config, session, tx, generation) = (
                    self.config.clone(),
                    self.session.clone().ok_or("請先登入。")?,
                    self.tx.clone(),
                    self.generation,
                );
                thread::spawn(move || {
                    let result = jobs::post(&config, &session, &path, &json!({}));
                    let _ = tx.send(Event::Work(generation, WorkEvent::Polled(id, result)));
                });
            }
            WorkCommand::RetryTask { id } => {
                if self.versions.blocked() {
                    return Err("請先更新版本。".into());
                }
                let task = self
                    .work
                    .store
                    .tasks
                    .iter()
                    .find(|t| t.request_id == id && t.active() && t.remote.is_none())
                    .ok_or("這個工作不可重試送出。")?
                    .clone();
                if !self.work.streams.contains(&id) {
                    self.submit_work(task)?;
                }
            }
            WorkCommand::Refresh => {
                self.refresh_work_capabilities();
                self.poll_work(true);
            }
        }
        Ok(())
    }
    pub(super) fn begin_work_chat(
        &mut self,
        messages: Vec<Message>,
        action: &str,
    ) -> AppResult<()> {
        let local = self.ensure_local_conversation()?;
        let request_id = jobs::new_id()?;
        let files = self.work.store.drafts(Some(&local));
        let tokens: Vec<String> = files
            .iter()
            .map(|a| {
                a.token()
                    .map(str::to_owned)
                    .ok_or("附件尚未就緒或已到期。".to_string())
            })
            .collect::<AppResult<_>>()?;
        let names: Vec<_> = files.iter().map(|a| a.name.clone()).collect();
        let mut messages = messages;
        let last = messages.last_mut().ok_or("缺少使用者訊息。")?;
        last.request_id = Some(request_id.clone());
        last.attachments = names;
        // 尚無 server conversation 時，送出執行緒先用相同 local ID 建立／取得；重試仍固定 ID。
        let request = jobs::chat_request(
            &self.config.model,
            &messages,
            &local,
            &request_id,
            &self.work.mode,
            tokens,
        )?;
        if self.work.store.tasks.iter().filter(|t| t.active()).count() >= 8 {
            return Err("最多同時追蹤 8 個聊天任務，請先等待部分任務完成。".into());
        }
        let task = Task {
            request_id: request_id.clone(),
            conversation_id: local.clone(),
            request,
            mode: self.work.mode.clone(),
            title: messages
                .last()
                .map(|m| m.content.chars().take(50).collect())
                .unwrap_or_default(),
            created_at: crate::unix_now(),
            remote: None,
            applied: false,
            message: "正在送出；關閉後會恢復查詢".into(),
            mail_analysis: action == "mail",
            partial: String::new(),
        };
        self.messages = messages;
        self.save_history()?;
        for a in &mut self.work.store.attachments {
            if a.conversation_id == local && !a.removed {
                a.sent = true;
            }
        }
        self.work.store.tasks.push(task.clone());
        self.work_save()?;
        self.set_draft(String::new());
        self.work.estimate = None;
        self.status = "工作已保存，正在交給伺服器；可切換到其他對話".into();
        self.submit_work(task)
    }
    fn submit_work(&mut self, task: Task) -> AppResult<()> {
        let (config, session, tx, generation) = (
            self.config.clone(),
            self.session
                .clone()
                .filter(|s| s.valid_for(&self.config))
                .ok_or("請先登入。")?,
            self.tx.clone(),
            self.generation,
        );
        self.work.streams.insert(task.request_id.clone());
        thread::spawn(move || {
            let id = task.request_id.clone();
            let result = (|| {
                let mut task = task;
                let remote = jobs::conversation(&config, &session, &task.conversation_id)?;
                task.request["conversation_id"] = json!(remote);
                jobs::submit(&config, &session, &task, |update| {
                    let _ = tx.send(Event::Work(
                        generation,
                        WorkEvent::Update(id.clone(), update),
                    ));
                })
            })();
            let _ = tx.send(Event::Work(generation, WorkEvent::Submitted(id, result)));
        });
        Ok(())
    }
    fn estimate_work(&mut self, text: &str) -> AppResult<()> {
        if !self.can_send() || !self.work.caps.as_ref().is_some_and(|c| c.timing_estimates) {
            return Err("目前無法估時。".into());
        }
        let local = self.ensure_local_conversation()?;
        let mut messages = self.messages.clone();
        messages.push(Message::user(text));
        let tokens = self
            .work
            .store
            .drafts(Some(&local))
            .iter()
            .map(|a| {
                a.token()
                    .map(str::to_owned)
                    .ok_or("附件尚未就緒。".to_string())
            })
            .collect::<AppResult<Vec<_>>>()?;
        let body = jobs::chat_request(
            &self.config.model,
            &messages,
            &local,
            &jobs::new_id()?,
            &self.work.mode,
            tokens,
        )?;
        self.work.estimate_revision += 1;
        let revision = self.work.estimate_revision;
        self.work.status = "正在估算排隊與處理時間…".into();
        let (config, session, tx, generation) = (
            self.config.clone(),
            self.session.clone().ok_or("請先登入。")?,
            self.tx.clone(),
            self.generation,
        );
        thread::spawn(move || {
            let result = (|| {
                let remote = jobs::conversation(&config, &session, &local)?;
                let mut body = body;
                body["conversation_id"] = json!(remote);
                let timing: Timing = jobs::post(
                    &config,
                    &session,
                    &format!("{}/chat/estimate", jobs::PREFIX),
                    &body,
                )?;
                timing.validate()?;
                Ok(timing)
            })();
            let _ = tx.send(Event::Work(
                generation,
                WorkEvent::Estimated(revision, result),
            ));
        });
        Ok(())
    }
    fn start_uploads(&mut self) {
        if !self.logged_in() || self.versions.blocked() || self.work.storage_error {
            return;
        }
        let available = 2usize.saturating_sub(self.work.uploads.len());
        let files: Vec<_> = self
            .work
            .store
            .attachments
            .iter()
            .filter(|a| {
                a.state == "upload_pending" && !a.removed && !self.work.uploads.contains(&a.id)
            })
            .take(available)
            .cloned()
            .collect();
        for a in files {
            self.work.uploads.insert(a.id.clone());
            let (config, session, tx, generation) = (
                self.config.clone(),
                self.session.clone(),
                self.tx.clone(),
                self.generation,
            );
            if let Some(session) = session {
                thread::spawn(move || {
                    let result = (|| {
                        let remote = jobs::conversation(&config, &session, &a.conversation_id)?;
                        let status = jobs::reserve_attachment(&config, &session, &remote, &a)?;
                        Ok((remote, status))
                    })();
                    let _ = tx.send(Event::Work(generation, WorkEvent::Reserved(a.id, result)));
                });
            }
        }
    }
    pub(super) fn poll_work(&mut self, force: bool) {
        self.start_uploads();
        if self.smoke
            || !self.logged_in()
            || self.work.caps.is_none()
            || self.work.polling
            || (!force && self.work.last_poll.elapsed() < Duration::from_secs(self.work.poll_delay))
        {
            return;
        }
        self.work.polling = true;
        self.work.last_poll = Instant::now();
        let tasks: Vec<_> = self
            .work
            .store
            .tasks
            .iter()
            .filter(|t| t.active())
            .cloned()
            .collect();
        let files: Vec<_> = self
            .work
            .store
            .attachments
            .iter()
            .filter(|a| {
                !a.removed
                    && !a.sent
                    && a.remote.is_some()
                    && !self.work.uploads.contains(&a.id)
                    && matches!(a.state.as_str(), "uploaded" | "queued" | "processing")
            })
            .cloned()
            .collect();
        let (config, session, tx, generation) = (
            self.config.clone(),
            self.session.clone(),
            self.tx.clone(),
            self.generation,
        );
        if let Some(session) = session {
            thread::spawn(move || {
                for task in tasks {
                    let result = jobs::task_status(&config, &session, &task);
                    let _ = tx.send(Event::Work(
                        generation,
                        WorkEvent::Polled(task.request_id, result),
                    ));
                }
                for a in files {
                    if let Some(remote) = a.remote {
                        let result = jobs::get(
                            &config,
                            &session,
                            &format!("{}/attachments/{}", jobs::PREFIX, remote.job_id),
                        );
                        let _ =
                            tx.send(Event::Work(generation, WorkEvent::Attachment(a.id, result)));
                    }
                }
                let _ = tx.send(Event::Work(generation, WorkEvent::PollFinished));
            });
        }
    }
    pub(super) fn work_event(&mut self, event: WorkEvent) -> AppResult<()> {
        match event {
            WorkEvent::Capabilities(result) => {
                self.work.capability_loading = false;
                match result {
                    Ok(cap) => {
                        if self.work.store.principal_id != cap.principal_id {
                            self.work.store =
                                WorkStore::load(&self.root, &self.config, &cap.principal_id)?;
                            self.work.storage_error = false;
                            // 完整接收的附件可重新上傳；未完整接收的只能重新選取。
                            for a in &mut self.work.store.attachments {
                                if !a.removed
                                    && matches!(a.state.as_str(), "uploading" | "upload_pending")
                                {
                                    a.state = "upload_pending".into();
                                }
                            }
                        }
                        if self.work.caps.is_none() || !cap.supports(&self.work.mode) {
                            self.work.mode = if cap.supports("stream") {
                                "stream"
                            } else if cap.supports("background") {
                                "background"
                            } else {
                                "sync"
                            }
                            .into();
                        }
                        self.work.caps = Some(cap);
                        self.work.status = "附件與執行模式由網站提供".into();
                        self.work_save()?;
                        self.finish_tasks()?;
                        self.poll_work(true);
                    }
                    Err(_) => {
                        self.work.status = "網站能力暫不可用；未啟用時仍可純文字對話".into();
                    }
                }
            }
            WorkEvent::Reserved(id, result) => {
                let (remote, status) = match result {
                    Ok(v) => v,
                    Err(e) => {
                        self.work.uploads.remove(&id);
                        if let Some(a) = self.work.store.attachments.iter_mut().find(|a| a.id == id)
                        {
                            a.state = "failed".into();
                            a.message = e;
                        }
                        self.work_save()?;
                        return Ok(());
                    }
                };
                let a = self
                    .work
                    .store
                    .attachments
                    .iter_mut()
                    .find(|a| a.id == id)
                    .ok_or("找不到上傳附件。")?;
                self.work
                    .store
                    .conversations
                    .insert(a.conversation_id.clone(), remote);
                a.apply(status.clone())?;
                let size = a.size;
                let removed = a.removed;
                if status.state == "awaiting_upload" && !removed {
                    a.state = "uploading".into();
                }
                self.work_save()?;
                if removed || status.state != "awaiting_upload" {
                    self.work.uploads.remove(&id);
                    let _ = std::fs::remove_file(attachments::spool_path(&self.root, &id)?);
                    if removed {
                        if let Some(session) = self.session.clone() {
                            let config = self.config.clone();
                            thread::spawn(move || {
                                let _: AppResult<Value> = jobs::post(
                                    &config,
                                    &session,
                                    &format!(
                                        "{}/attachments/{}/cancel",
                                        jobs::PREFIX,
                                        status.job_id
                                    ),
                                    &json!({}),
                                );
                            });
                        }
                    }
                    return Ok(());
                }
                let (config, session, tx, generation, root) = (
                    self.config.clone(),
                    self.session.clone().ok_or("請先登入。")?,
                    self.tx.clone(),
                    self.generation,
                    self.root.clone(),
                );
                thread::spawn(move || {
                    let result = (|| {
                        struct ProgressReader<'a> {
                            reader: attachments::SpoolReader,
                            sent: u64,
                            notify: &'a mut dyn FnMut(u64),
                        }
                        impl std::io::Read for ProgressReader<'_> {
                            fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
                                let n = self.reader.read(b)?;
                                self.sent += n as u64;
                                (self.notify)(self.sent);
                                Ok(n)
                            }
                        }
                        let mut last = Instant::now();
                        let mut progress = |sent| {
                            if last.elapsed() > Duration::from_millis(250) || sent == size {
                                let _ = tx.send(Event::Work(
                                    generation,
                                    WorkEvent::Progress(id.clone(), sent),
                                ));
                                last = Instant::now();
                            }
                        };
                        let mut reader = ProgressReader {
                            reader: attachments::SpoolReader::open(&root, &id)?,
                            sent: 0,
                            notify: &mut progress,
                        };
                        let response = transport::exchange(
                            &config.endpoint(&format!(
                                "{}/attachments/{}/content",
                                jobs::PREFIX,
                                status.job_id
                            ))?,
                            "PUT",
                            "application/octet-stream",
                            transport::Payload {
                                reader: &mut reader,
                                length: size as u32,
                            },
                            Some(("Authorization", &format!("Bearer {}", session.access_token))),
                            120_000,
                            None,
                        )?;
                        jobs::decode(response, &session)
                    })();
                    let _ = tx.send(Event::Work(generation, WorkEvent::Uploaded(id, result)));
                });
            }
            WorkEvent::Uploaded(id, result) => {
                self.work.uploads.remove(&id);
                let a = self
                    .work
                    .store
                    .attachments
                    .iter_mut()
                    .find(|a| a.id == id)
                    .ok_or("找不到附件。")?;
                match result {
                    Ok(status) => {
                        a.apply(status)?;
                        let _ = std::fs::remove_file(attachments::spool_path(&self.root, &id)?);
                    }
                    Err(e) => {
                        a.message = format!("{e} 可重試相同附件，不會建立另一份工作。");
                        a.state = "failed".into();
                    }
                }
                self.work_save()?;
            }
            WorkEvent::Attachment(id, result) => {
                if let Some(a) = self
                    .work
                    .store
                    .attachments
                    .iter_mut()
                    .find(|a| a.id == id && !a.removed)
                {
                    match result {
                        Ok(status) => {
                            a.apply(status)?;
                            self.work.poll_delay = 5;
                        }
                        Err(e) => {
                            a.message = e;
                            self.work.poll_delay = (self.work.poll_delay * 2).min(30);
                        }
                    }
                    self.work_save()?;
                }
            }
            WorkEvent::Progress(id, bytes) => {
                if let Some(a) = self.work.store.attachments.iter_mut().find(|a| a.id == id) {
                    a.uploaded_bytes = bytes;
                }
            }
            WorkEvent::Update(id, update) => {
                let task = self
                    .work
                    .store
                    .tasks
                    .iter_mut()
                    .find(|t| t.request_id == id)
                    .ok_or("找不到聊天工作。")?;
                match update {
                    jobs::StreamUpdate::Status(status) => {
                        task.apply_status(*status)?;
                        self.work_save()?;
                        self.finish_tasks()?;
                    }
                    jobs::StreamUpdate::Delta(text) => {
                        if task.active() && task.partial.len() + text.len() <= 1_048_576 {
                            task.partial.push_str(&text);
                        }
                    }
                }
            }
            WorkEvent::Submitted(id, result) => {
                self.work.streams.remove(&id);
                if let Some(task) = self
                    .work
                    .store
                    .tasks
                    .iter_mut()
                    .find(|t| t.request_id == id && t.active())
                {
                    task.message = match result {
                        Ok(()) => "正在確認伺服器保存的結果".into(),
                        Err(e) => format!("{e} 將查詢任務，請勿另建重複請求。"),
                    };
                    self.work_save()?;
                }
                self.poll_work(true);
            }
            WorkEvent::Polled(id, result) => {
                if let Some(task) = self
                    .work
                    .store
                    .tasks
                    .iter_mut()
                    .find(|t| t.request_id == id && t.active())
                {
                    match result {
                        Ok(status) => {
                            task.apply_status(status)?;
                            self.work.poll_delay = 5;
                        }
                        Err(e) => {
                            task.message = e;
                            self.work.poll_delay = (self.work.poll_delay * 2).min(30);
                        }
                    }
                    self.work_save()?;
                    self.finish_tasks()?;
                }
            }
            WorkEvent::PollFinished => self.work.polling = false,
            WorkEvent::Estimated(revision, result) if revision == self.work.estimate_revision => {
                match result {
                    Ok(timing) => {
                        self.work.estimate = Some(timing);
                        self.work.status = "估時僅供參考，實際排隊以伺服器為準".into();
                    }
                    Err(error) => {
                        self.work.estimate = None;
                        self.work.status = format!("估時暫不可用：{error}");
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
    /// 先原子保存歷史，再記錄已套用。若中途當機，以 request_id 避免重複加入回答。
    fn finish_tasks(&mut self) -> AppResult<()> {
        let pending: Vec<_> = self
            .work
            .store
            .tasks
            .iter()
            .filter(|t| !t.applied && t.remote.as_ref().is_some_and(TaskStatus::terminal))
            .cloned()
            .collect();
        for task in pending {
            let remote = task.remote.as_ref().ok_or("缺少任務狀態。")?;
            if remote.state == "completed" {
                let mut archive = self.archive.clone();
                jobs::apply_reply(&mut archive, &task)?;
                history::save(&self.root, &archive)?;
                self.archive = archive;
                if self.active_id.as_ref() == Some(&task.conversation_id) {
                    if let Some(c) = self
                        .archive
                        .conversations
                        .iter()
                        .find(|c| c.id == task.conversation_id)
                    {
                        self.messages = c.messages.clone();
                    }
                }
            }
            if let Some(saved) = self
                .work
                .store
                .tasks
                .iter_mut()
                .find(|t| t.request_id == task.request_id)
            {
                saved.applied = true;
                saved.partial.clear();
                saved.request = json!({});
            }
            self.work_save()?;
            let label = if remote.state == "completed" {
                "回覆完成"
            } else if remote.state == "cancelled" {
                "工作已取消"
            } else {
                "工作失敗，請查看任務"
            };
            self.toast(label);
            if self.config.notification_popups {
                self.work.task_balloon = true;
                tray(self.window, NIM_MODIFY, Some(label));
            }
        }
        Ok(())
    }
}
