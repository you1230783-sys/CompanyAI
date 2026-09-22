//! Outlook 多封郵件流程：預覽 → 初篩 → 本機受限匯出 → 共用附件 → 持久聊天任務。
//! 初篩／COM 階段需 App 保持開啟；退出後不自動恢復對信箱的存取。
use super::*;
use crate::{
    attachments::{self, Attachment},
    jobs,
    outlook::batch::{self, Mail, MailList},
};

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub(super) enum MailCommand {
    /// 進入助理頁時重查網站模型清單，不能只沿用開啟 App 時的快取。
    RefreshModels,
    List {
        period: String,
        unread: bool,
        #[serde(default)]
        scope: batch::SearchScope,
    },
    Analyze {
        ids: Vec<String>,
        allow_export: bool,
    },
    Stop,
}
pub(super) enum MailEvent {
    Listed(AppResult<MailList>),
    Progress(String),
    Prepared(AppResult<Prepared>),
}
/// 事件被登出／停止丟棄時也會清理尚未交接的加密匯出，避免孤立暫存。
pub(super) struct Prepared {
    root: PathBuf,
    summary: String,
    files: Vec<Attachment>,
    analysis_caps: Option<jobs::Capabilities>,
}
impl Drop for Prepared {
    fn drop(&mut self) {
        for file in &self.files {
            if let Ok(path) = attachments::spool_path(&self.root, &file.id) {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}
pub(super) struct MailRuntime {
    pub list: MailList,
    pub operation: u64,
    pub cancel: Arc<AtomicBool>,
    pub phase: &'static str,
    pub status: String,
    pub conversation: Option<String>,
    pub file_ids: Vec<String>,
    pub summary: String,
    pub model: String,
    /// 品質模型的能力獨立保存，不能套用初篩模型或一般聊天的附件規則。
    pub analysis_caps: Option<jobs::Capabilities>,
}
impl Default for MailRuntime {
    fn default() -> Self {
        Self {
            list: Default::default(),
            operation: 0,
            cancel: Arc::new(AtomicBool::new(false)),
            phase: "idle",
            status: "選取多封郵件，或選擇查詢範圍後依日期讀取。".into(),
            conversation: None,
            file_ids: vec![],
            summary: String::new(),
            model: String::new(),
            analysis_caps: None,
        }
    }
}
impl Drop for MailRuntime {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
impl App {
    pub(super) fn mail_batch_state(&self) -> serde_json::Value {
        let quality_status = if !self.logged_in() {
            "signed_out"
        } else if self.services_loading {
            "checking"
        } else if self.models.is_none() {
            "error"
        } else if self.require_mail_analysis_model().is_ok() {
            "available"
        } else {
            "unavailable"
        };
        json!({"list":self.mail_flow.list,"phase":self.mail_flow.phase,"status":self.mail_flow.status,
            "busy":self.mail_flow.phase!="idle","conversation_id":self.mail_flow.conversation,
            "quality_status":quality_status})
    }
    pub(super) fn mail_batch_command(&mut self, command: MailCommand) -> AppResult<()> {
        match command {
            MailCommand::RefreshModels => self.refresh_services(),
            MailCommand::Stop => {
                self.mail_flow.cancel.store(true, Ordering::Relaxed);
                self.mail_flow.operation += 1;
                self.mail_flow.phase = "idle";
                self.mail_flow.status =
                    "已停止後續郵件存取與自動送出；已交給伺服器的資料不會自動撤回。".into();
                for id in std::mem::take(&mut self.mail_flow.file_ids) {
                    self.work_command(work::WorkCommand::RemoveAttachment { id })?;
                }
            }
            MailCommand::List {
                period,
                unread,
                scope,
            } => {
                if self.mail_flow.phase != "idle" {
                    return Err("請先停止目前郵件流程。".into());
                }
                self.mail_flow.cancel = Arc::new(AtomicBool::new(false));
                self.mail_flow.operation += 1;
                self.mail_flow.phase = "listing";
                self.mail_flow.status = "正在讀取基本資訊，尚未送交 AI…".into();
                self.mail_flow.list = MailList::default();
                let (cancel, tx, generation, operation, demo) = (
                    self.mail_flow.cancel.clone(),
                    self.tx.clone(),
                    self.generation,
                    self.mail_flow.operation,
                    self.demo,
                );
                thread::spawn(move || {
                    let result = if demo {
                        Ok(MailList {
                            mails: (0..3)
                                .map(|i| Mail {
                                    id: format!("demo_mail_{i}"),
                                    folder: "本機模擬／分類郵件".into(),
                                    preview: outlook::demo_mail(false),
                                    store_id: "demo-store".into(),
                                })
                                .collect(),
                            scope: "本機模擬郵件".into(),
                            truncated: false,
                            notice: String::new(),
                        })
                    } else {
                        batch::list(&period, unread, scope, &cancel)
                    };
                    let _ = tx.send(Event::MailBatch(
                        generation,
                        operation,
                        MailEvent::Listed(result),
                    ));
                });
            }
            MailCommand::Analyze { ids, allow_export } => {
                if self.mail_flow.phase != "idle" || !self.can_send() {
                    return Err("請先完成目前操作、登入並選擇可用模型。".into());
                }
                if ids.is_empty() || ids.len() > batch::MAX_MAILS {
                    return Err("請勾選 1 至 50 封本批郵件。".into());
                }
                let unique: std::collections::HashSet<_> = ids.iter().collect();
                if unique.len() != ids.len() {
                    return Err("郵件不可重複。".into());
                }
                let mails: Vec<Mail> = ids
                    .iter()
                    .map(|id| {
                        self.mail_flow
                            .list
                            .mails
                            .iter()
                            .find(|m| &m.id == id)
                            .cloned()
                            .ok_or("郵件清單已改變，請重新勾選。".to_string())
                    })
                    .collect::<AppResult<_>>()?;
                if allow_export {
                    if self.services_loading {
                        return Err("正在確認品質模型是否可用，請稍候再啟用自動補充內文。".into());
                    }
                    self.require_mail_analysis_model()?;
                    if self.work.capability_loading
                        || self.work.capability_model != self.config.model
                        || self.work.caps.is_none()
                        || self.work.storage_error
                    {
                        return Err("請先重新整理網站能力，確認本機任務紀錄可用。".into());
                    }
                }
                // 初篩也留有本機對話，不會把結果加到使用者後來切換的另一個對話。
                if !self
                    .work
                    .caps
                    .as_ref()
                    .is_some_and(|c| c.supports("background"))
                {
                    return Err("Outlook 初篩需要網站支援背景處理。".into());
                }
                self.work.mode = "background".into();
                let request_id = jobs::new_id()?;
                let mut intro = Message::user(&batch::conversation_intro(&mails)?);
                intro.request_id = Some(request_id.clone());
                let messages = vec![intro];
                let mut archive = self.archive.clone();
                let local = archive.insert(messages)?;
                history::save(&self.root, &archive)?;
                self.archive = archive;
                self.mail_flow.operation += 1;
                self.mail_flow.cancel = Arc::new(AtomicBool::new(false));
                self.mail_flow.phase = "analyzing";
                self.mail_flow.conversation = Some(local.clone());
                self.mail_flow.model = self.config.model.clone();
                self.mail_flow.analysis_caps = None;
                self.mail_flow.file_ids.clear();
                self.mail_flow.status = "只傳基本資訊進行初篩；可停止後續匯出…".into();
                let (config, session, root, tx, generation, operation, cancel, demo) = (
                    self.config.clone(),
                    self.session.clone().ok_or("請先登入。")?,
                    self.root.clone(),
                    self.tx.clone(),
                    self.generation,
                    self.mail_flow.operation,
                    self.mail_flow.cancel.clone(),
                    self.demo,
                );
                let principal = self.work.store.principal_id.clone();
                let prompt = batch::prompt(&mails, allow_export, 20)?;
                let mut request = jobs::chat_request(
                    &config.model,
                    &[Message::user(&prompt)],
                    &local,
                    &request_id,
                    "background",
                    vec![],
                )?;
                jobs::set_purpose(&mut request, true, false)?;
                let triage = jobs::Task {
                    request_id,
                    conversation_id: local.clone(),
                    request,
                    mode: "background".into(),
                    title: "Outlook 郵件初篩".into(),
                    created_at: crate::unix_now(),
                    remote: None,
                    applied: false,
                    message: "背景初篩中；退出後仍可查詢原始結果，郵件匯出需重新授權。".into(),
                    mail_analysis: false,
                    title_generation: false,
                    tool_events: Vec::new(),
                    partial: String::new(),
                };
                self.work.store.tasks.push(triage.clone());
                self.work_save()?;
                self.submit_work(triage.clone())?;
                self.preserve_draft()?;
                self.active_id = Some(local.clone());
                self.messages = self
                    .archive
                    .conversations
                    .iter()
                    .find(|c| c.id == local)
                    .ok_or("缺少初篩對話。")?
                    .messages
                    .clone();
                self.set_draft(String::new());
                self.focus_draft = true;
                thread::spawn(move || {
                    let result = (|| {
                        let mut prepared = Prepared {
                            root: root.clone(),
                            summary: String::new(),
                            files: vec![],
                            analysis_caps: None,
                        };
                        batch::check_cancel(&cancel)?;
                        let mut rules = attachments::AttachmentRules::default();
                        if allow_export {
                            let mut analysis_config = config.clone();
                            analysis_config.model = batch::ANALYSIS_MODEL.into();
                            let caps = jobs::capabilities(&analysis_config, &session)?;
                            if caps.principal_id != principal {
                                return Err(
                                    "品質模型的帳號識別與目前任務不一致，請重新登入。".into()
                                );
                            }
                            if !caps.supports("stream") && !caps.supports("background") {
                                return Err("品質模型尚未提供串流或背景附件分析。".into());
                            }
                            caps.attachments.check("mail.msg", 1, 0, 0).map_err(|_| {
                                "品質模型目前不支援 MSG 附件，請聯絡網站管理者。".to_string()
                            })?;
                            rules = caps.attachments.clone();
                            prepared.analysis_caps = Some(caps);
                        }
                        batch::check_cancel(&cancel)?;
                        // 初篩已透過共用持久任務送出；此執行緒只等待結果及本次 COM 授權。
                        // 即使 App 退出，伺服器工作仍保留；重啟不自動恢復郵件匯出權限。
                        let reply = if demo {
                            json!({"schema_version":1,"summary":"本機示範：已完成基本資訊初篩。","requests":[]}).to_string()
                        } else {
                            let deadline = Instant::now() + Duration::from_secs(3600);
                            loop {
                                batch::check_cancel(&cancel)?;
                                match jobs::task_status(&config, &session, &triage) {
                                    Ok(status) if status.state == "completed" => {
                                        let reply = protocol::assistant_text(
                                            &status
                                                .result
                                                .as_ref()
                                                .ok_or("初篩缺少結果。")?
                                                .to_string(),
                                        )?;
                                        let _ = tx.send(Event::Work(
                                            generation,
                                            work::WorkEvent::Polled(
                                                triage.request_id.clone(),
                                                Ok(status),
                                            ),
                                        ));
                                        break reply;
                                    }
                                    Ok(status) if status.terminal() => {
                                        return Err(format!(
                                            "初篩已結束：{} {}",
                                            status.state, status.error_message
                                        ))
                                    }
                                    _ => {}
                                }
                                if Instant::now() >= deadline {
                                    return Err("初篩等待超過一小時；任務仍可在工作列表查詢，後續匯出已暫停。".into());
                                }
                                for _ in 0..25 {
                                    batch::check_cancel(&cancel)?;
                                    thread::sleep(Duration::from_millis(200));
                                }
                            }
                        };
                        batch::check_cancel(&cancel)?;
                        let decision =
                            match batch::decision(&reply, &mails, allow_export, rules.max_count) {
                                Ok(decision) => decision,
                                Err(reason) => {
                                    // 不能解析或越權時仍顯示完整原文，但絕不猜測並執行匯出。
                                    prepared.summary = format!("{reply}\n\n---\n{reason}");
                                    return Ok(prepared);
                                }
                            };
                        prepared.summary = decision.summary;
                        let mut total = 0;
                        for (index, request) in decision.requests.iter().enumerate() {
                            batch::check_cancel(&cancel)?;
                            let _ = tx.send(Event::MailBatch(
                                generation,
                                operation,
                                MailEvent::Progress(format!(
                                    "AI 要求補充第 {} / {} 封：{}",
                                    index + 1,
                                    decision.requests.len(),
                                    request.reason
                                )),
                            ));
                            let mail = mails
                                .iter()
                                .find(|m| m.id == request.mail_id)
                                .ok_or("郵件不在本批範圍。")?;
                            let file = batch::export(
                                &root,
                                &local,
                                mail,
                                &rules,
                                (index, total),
                                &cancel,
                            )?;
                            total += file.size;
                            prepared.files.push(file);
                        }
                        batch::check_cancel(&cancel)?;
                        Ok(prepared)
                    })();
                    let _ = tx.send(Event::MailBatch(
                        generation,
                        operation,
                        MailEvent::Prepared(result),
                    ));
                });
            }
        }
        Ok(())
    }
    pub(super) fn mail_batch_event(&mut self, event: MailEvent) -> AppResult<()> {
        match event {
            MailEvent::Listed(result) => {
                self.mail_flow.phase = "idle";
                match result {
                    Ok(list) => {
                        self.mail_flow.status = format!(
                            "{}。已讀取 {} 封基本資訊；勾選後再分析。{}{}",
                            list.scope,
                            list.mails.len(),
                            list.notice,
                            if list.truncated && list.notice.is_empty() {
                                "清單已截斷，請縮小範圍或在 Outlook 選取其他郵件。"
                            } else {
                                ""
                            }
                        );
                        self.mail_flow.list = list;
                    }
                    Err(e) => self.mail_flow.status = e,
                }
            }
            MailEvent::Progress(status) => self.mail_flow.status = status,
            MailEvent::Prepared(result) => {
                let mut prepared = match result {
                    Ok(value) => value,
                    Err(e) => {
                        self.mail_flow.phase = "idle";
                        self.mail_flow.status = e;
                        return Ok(());
                    }
                };
                let local = self
                    .mail_flow
                    .conversation
                    .as_ref()
                    .ok_or("缺少郵件對話。")?;
                let mut archive = self.archive.clone();
                let conversation = archive
                    .conversations
                    .iter_mut()
                    .find(|c| &c.id == local)
                    .ok_or("郵件對話已刪除。")?;
                conversation
                    .messages
                    .push(Message::assistant(prepared.summary.clone()));
                history::save(&self.root, &archive)?;
                self.archive = archive;
                if self.active_id.as_ref() == Some(local) {
                    self.messages = self
                        .archive
                        .conversations
                        .iter()
                        .find(|c| &c.id == local)
                        .ok_or("缺少對話。")?
                        .messages
                        .clone();
                }
                self.mail_flow.summary = prepared.summary.clone();
                self.mail_flow.analysis_caps = prepared.analysis_caps.take();
                if prepared.files.is_empty() {
                    self.mail_flow.phase = "idle";
                    self.mail_flow.status =
                        "初篩回覆已保存，未執行郵件匯出。可開啟分析對話查看原文與說明。".into();
                } else {
                    // 先持久保存附件清單才交接暫存檔所有權並開始網路上傳。
                    let mut store = self.work.store.clone();
                    store.attachments.extend(prepared.files.iter().cloned());
                    store.save(&self.root, &self.config)?;
                    self.mail_flow.file_ids = prepared.files.iter().map(|f| f.id.clone()).collect();
                    self.work.store = store;
                    prepared.files.clear();
                    self.mail_flow.phase = "uploading";
                    self.mail_flow.status =
                        "MSG 已加密暫存，正在自動上傳／等待網站轉檔。完成後由品質模型整理。".into();
                    self.poll_work(true);
                }
            }
        }
        Ok(())
    }
    pub(super) fn mail_batch_tick(&mut self) -> AppResult<bool> {
        if self.mail_flow.phase != "uploading" {
            return Ok(false);
        }
        let files: Vec<_> = self
            .mail_flow
            .file_ids
            .iter()
            .filter_map(|id| self.work.store.attachments.iter().find(|a| &a.id == id))
            .collect();
        if files.len() != self.mail_flow.file_ids.len() || files.iter().any(|a| a.removed) {
            self.mail_flow.phase = "idle";
            self.mail_flow.status = "附件已移除，不再自動整理。".into();
            return Ok(true);
        }
        if !files.iter().all(|a| a.token().is_some()) {
            return Ok(false);
        }
        // 送出前再次套用品質模型規則，不使用一般聊天／初篩模型的規則。
        let rules = &self
            .mail_flow
            .analysis_caps
            .as_ref()
            .ok_or("品質模型附件能力尚未就緒。")?
            .attachments;
        let mut total = 0;
        for (index, file) in files.iter().enumerate() {
            rules.check(&file.name, file.size, index, total)?;
            total += file.size;
        }
        let tokens = files
            .iter()
            .map(|f| f.token().unwrap_or_default().to_string())
            .collect();
        let names = files.iter().map(|f| f.name.clone()).collect();
        let local = self
            .mail_flow
            .conversation
            .clone()
            .ok_or("缺少郵件對話。")?;
        self.queue_mail_result(&local, tokens, names)?;
        self.mail_flow.phase = "idle";
        self.mail_flow.file_ids.clear();
        self.mail_flow.status =
            "附件已就緒，已交由品質模型最終整理；可在工作任務停止或移除。".into();
        Ok(true)
    }
    /// 不從顯示名稱猜模型；品質代號未授權時明確停止，不默默改用其他模型。
    fn require_mail_analysis_model(&self) -> AppResult<()> {
        if !self.models.as_ref().is_some_and(|catalog| {
            catalog
                .models
                .iter()
                .any(|model| model.id == batch::ANALYSIS_MODEL)
        }) {
            return Err("目前品質模型維護中，暫時停用自動補充內文功能。".into());
        }
        Ok(())
    }
    /// 最終分析沿用 0.5 持久任務，不依賴目前畫面正開啟哪個對話。
    fn queue_mail_result(
        &mut self,
        local: &str,
        tokens: Vec<String>,
        names: Vec<String>,
    ) -> AppResult<()> {
        if !self.logged_in() || self.versions.blocked() || self.config.model != self.mail_flow.model
        {
            return Err("登入／版本／模型已改變；請重新確認郵件流程。".into());
        }
        self.require_mail_analysis_model()?;
        if self.work.store.tasks.iter().filter(|t| t.active()).count() >= 8 {
            return Err("任務已滿，附件保留；請稍後重新整理。".into());
        }
        let request_id = jobs::new_id()?;
        let mut archive = self.archive.clone();
        let conversation = archive
            .conversations
            .iter_mut()
            .find(|c| c.id == local)
            .ok_or("找不到郵件對話。")?;
        let mut message = Message::user(&batch::analysis_prompt(&names)?);
        message.request_id = Some(request_id.clone());
        message.attachments = names;
        conversation.messages.push(message);
        let messages = conversation.messages.clone();
        let mode = if self
            .mail_flow
            .analysis_caps
            .as_ref()
            .is_some_and(|c| c.supports("background"))
        {
            "background"
        } else {
            "stream"
        };
        let request = jobs::chat_request(
            batch::ANALYSIS_MODEL,
            &messages,
            local,
            &request_id,
            mode,
            tokens,
        )?;
        let task = jobs::Task {
            request_id,
            conversation_id: local.into(),
            request,
            mode: mode.into(),
            title: "Outlook 多封郵件最終整理".into(),
            created_at: crate::unix_now(),
            remote: None,
            applied: false,
            message: "MSG 已處理，正在送出整理".into(),
            mail_analysis: false,
            title_generation: false,
            tool_events: Vec::new(),
            partial: String::new(),
        };
        history::save(&self.root, &archive)?;
        self.archive = archive;
        self.work.store.tasks.push(task.clone());
        for file in &mut self.work.store.attachments {
            if self.mail_flow.file_ids.contains(&file.id) {
                file.sent = true;
            }
        }
        self.work.store.save(&self.root, &self.config)?;
        if self.active_id.as_deref() == Some(local) {
            self.messages = messages;
        }
        self.submit_work(task)
    }
}
