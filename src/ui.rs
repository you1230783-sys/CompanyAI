//! 主視窗與工作流程。背景工作只透過訊息傳回資料，不跨執行緒操作 WebView2。
use crate::{
    auth,
    config::{Config, DOWNLOAD_PATH},
    demo::DemoServer,
    history::{self, Archive},
    notifications::{self, Inbox},
    outlook::{self, MailPreview},
    protocol::{self, DeviceGrant, Message},
    selection::{self, Hotkey},
    service::{self, ModelCatalog, VersionState},
    storage::{self, Session},
    transport,
    webview::WebView,
    wide, AppResult,
};
use serde::Deserialize;
use serde_json::json;
use std::{
    cell::RefCell,
    path::PathBuf,
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{HiDpi::*, Shell::*, WindowsAndMessaging::*},
};
const TRAY_MESSAGE: u32 = WM_APP + 4;

/// 明確列舉介面命令，不提供任意檔案、程式或任意 API 執行入口。
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Command {
    StartHotkeyRecording,
    CancelHotkeyRecording,
    RecordedHotkey {
        modifiers: u32,
        key: u32,
    },
    Ready,
    Draft {
        text: String,
    },
    Chat {
        text: String,
        action: String,
    },
    NewChat,
    Preferences {
        font_size: u8,
        sidebar_collapsed: bool,
        notification_popups: bool,
    },
    Hotkey {
        value: String,
    },
    Login,
    Logout,
    CancelLogin,
    ReopenLogin,
    Refresh,
    Download,
    Model {
        id: String,
    },
    SelectChat {
        id: String,
    },
    DeleteChat {
        id: String,
    },
    Copy {
        text: String,
    },
    RefreshEvents,
    ReadEvent {
        id: String,
    },
    ReadMail,
    ReadMailBody,
    OmitMailBody,
    AnalyzeMail,
    OpenLink {
        url: String,
    },
    SelfTestResult {
        ok: bool,
        detail: String,
    },
}
enum Event {
    Services(
        u64,
        AppResult<service::VersionInfo>,
        AppResult<ModelCatalog>,
    ),
    Grant(u64, AppResult<DeviceGrant>),
    Login(u64, AppResult<Session>),
    Chat(u64, auth::ChatOutcome),
    Capture(AppResult<String>),
    Mail(u64, AppResult<MailPreview>),
    Events(u64, AppResult<notifications::EventPage>),
    Read(u64, String, AppResult<()>),
    Socket(u64, bool),
}
struct App {
    window: HWND,
    view: WebView,
    root: PathBuf,
    config: Config,
    session: Option<Session>,
    archive: Archive,
    active_id: Option<String>,
    messages: Vec<Message>,
    history_error: Option<String>,
    draft: String,
    draft_revision: u64,
    focus_draft: bool,
    busy: &'static str,
    status: String,
    error: bool,
    models: Option<ModelCatalog>,
    versions: VersionState,
    version_status: String,
    services_loading: bool,
    generation: u64,
    login_operation: u64,
    cancelled: Arc<AtomicBool>,
    grant: Option<DeviceGrant>,
    inbox: Inbox,
    notifications_loading: bool,
    notifications_pending: bool,
    notification_status: String,
    socket_cancel: Arc<AtomicBool>,
    last_events: Instant,
    last_services: Instant,
    mail: Option<MailPreview>,
    mail_busy: bool,
    hotkey: Option<Hotkey>,
    recording_since: Option<Instant>,
    suppress_hotkey: bool,
    tx: mpsc::Sender<Event>,
    rx: mpsc::Receiver<Event>,
    commands: mpsc::Receiver<String>,
    demo: bool,
    smoke: bool,
    ready: bool,
    smoke_result: Option<AppResult<()>>,
}
impl Drop for App {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
        self.socket_cancel.store(true, Ordering::Relaxed);
        selection::unregister(self.window);
        tray(self.window, NIM_DELETE, None);
    }
}
impl App {
    fn logged_in(&self) -> bool {
        self.session
            .as_ref()
            .is_some_and(|s| s.valid_for(&self.config))
    }
    fn can_send(&self) -> bool {
        self.logged_in()
            && self.busy == "none"
            && !self.versions.blocked()
            && self
                .models
                .as_ref()
                .is_some_and(|c| c.models.iter().any(|m| m.id == self.config.model))
    }
    fn publish(&mut self) {
        if !self.ready {
            return;
        }
        let models: Vec<_> = self
            .models
            .as_ref()
            .map(|c| {
                c.models
                    .iter()
                    .map(|m| json!({"id":m.id,"label":m.label,"description":m.description}))
                    .collect()
            })
            .unwrap_or_default();
        let conversations: Vec<_> = self
            .archive
            .conversations
            .iter()
            .map(|c| json!({"id":c.id,"title":c.title,"updated_at":c.updated_at}))
            .collect();
        let events: Vec<_> = self.inbox.events.iter().filter(|e| !e.expired()).collect();
        let _=self.view.post(&json!({"type":"state","state":{
            "version":service::CURRENT_VERSION,"config":self.config,"status":self.status,"error":self.error,
            "busy":self.busy,"logged_in":self.logged_in(),"can_send":self.can_send(),"update_required":self.versions.blocked(),
            "models":models,"conversations":conversations,"active_id":self.active_id,"messages":self.messages,
            "draft":self.draft,"draft_revision":self.draft_revision,"focus_draft":self.focus_draft,
            "notifications":events,"notification_status":self.notification_status,"unread_count":self.inbox.unread_count(),"notifications_loading":self.notifications_loading,
            "mail":self.mail,"mail_busy":self.mail_busy,"history_error":self.history_error,
            "version_status":self.version_status,"login_code":self.grant.as_ref().map(|g|&g.user_code)
        }}));
        self.focus_draft = false;
    }
    fn fail(&mut self, error: String) {
        self.status = error;
        self.error = true;
    }
    fn toast(&self, text: &str) {
        let _ = self.view.post(&json!({"type":"toast","text":text}));
    }
    fn set_draft(&mut self, text: String) {
        self.draft = text;
        self.draft_revision += 1;
    }
    fn new_chat(&mut self) {
        self.active_id = None;
        self.messages.clear();
        self.set_draft(String::new());
        self.focus_draft = true;
    }
    /// 先保存副本，成功後採用；失敗仍保留畫面的回覆並標示未保存。
    fn save_history(&mut self) -> AppResult<()> {
        if self.history_error.is_some() {
            return Err("對話暫未保存，請先處理本機紀錄讀寫問題後重新啟動。".into());
        }
        let mut archive = self.archive.clone();
        let id = if let Some(id) = &self.active_id {
            archive.update(id, self.messages.clone())?;
            id.clone()
        } else {
            archive.insert(self.messages.clone())?
        };
        history::save(&self.root, &archive)?;
        self.archive = archive;
        self.active_id = Some(id);
        Ok(())
    }
    fn apply_hotkey(&mut self, value: String) -> AppResult<()> {
        let next = Hotkey::parse(&value)?;
        if self.recording_since.is_some() {
            self.stop_recording()?;
        }
        selection::unregister(self.window);
        if let Err(e) = selection::register(self.window, next) {
            if let Some(old) = self.hotkey {
                let _ = selection::register(self.window, old);
            }
            return Err(e);
        }
        let mut config = self.config.clone();
        config.hotkey = next.label();
        if let Err(e) = storage::save_config(&self.root, &config) {
            selection::unregister(self.window);
            if let Some(old) = self.hotkey {
                let _ = selection::register(self.window, old);
            }
            return Err(e);
        }
        self.hotkey = Some(next);
        self.config = config;
        Ok(())
    }
    fn start_recording(&mut self) -> AppResult<()> {
        if self.recording_since.is_some() {
            return Ok(());
        }
        if self.busy != "none" {
            return Err("請等目前操作完成後再錄製快捷鍵。".into());
        }
        selection::unregister(self.window);
        self.recording_since = Some(Instant::now());
        self.view.set_recording(true);
        self.view
            .post(&json!({"type":"hotkey_recording","active":true}))
    }
    /// 結束錄製立即恢復舊快捷鍵。新組合只有按「套用」且註冊成功才生效。
    fn stop_recording(&mut self) -> AppResult<()> {
        self.view.set_recording(false);
        if self.recording_since.take().is_none() {
            return Ok(());
        }
        self.suppress_hotkey = true;
        let _ = self
            .view
            .post(&json!({"type":"hotkey_recording","active":false}));
        if let Some(old) = self.hotkey {
            if let Err(e) = selection::register(self.window, old) {
                self.hotkey = None;
                return Err(format!("錄製已結束，但原快捷鍵無法恢復：{e}"));
            }
        }
        Ok(())
    }
    fn refresh_services(&mut self) {
        if self.services_loading || self.smoke {
            return;
        }
        self.services_loading = true;
        self.last_services = Instant::now();
        let (config, session, tx, generation) = (
            self.config.clone(),
            self.session.clone(),
            self.tx.clone(),
            self.generation,
        );
        thread::spawn(move || {
            let version = service::fetch_version(&config);
            let models = service::fetch_models(&config, session.as_ref());
            let _ = tx.send(Event::Services(generation, version, models));
        });
    }
    fn start_notifications(&mut self) {
        self.socket_cancel.store(true, Ordering::Relaxed);
        self.socket_cancel = Arc::new(AtomicBool::new(false));
        let Some(session) = self.session.clone().filter(|s| s.valid_for(&self.config)) else {
            return;
        };
        self.inbox = notifications::load(&self.root, &session)
            .unwrap_or_else(|_| Inbox::for_session(&session));
        self.fetch_events();
        let (config, tx, cancel, generation) = (
            self.config.clone(),
            self.tx.clone(),
            self.socket_cancel.clone(),
            self.generation,
        );
        thread::spawn(move || {
            let Ok(url) = config.endpoint(notifications::SOCKET_PATH) else {
                return;
            };
            let mut backoff = 5;
            while !cancel.load(Ordering::Relaxed) && session.valid_for(&config) {
                let _ = transport::watch_notifications(
                    &url,
                    &session.access_token,
                    &cancel,
                    |connected| {
                        let _ = tx.send(Event::Socket(generation, connected));
                    },
                );
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                // 即時通道失敗仍定期補查；退避避免持續請求尚未部署的路由。
                for _ in 0..backoff * 10 {
                    if cancel.load(Ordering::Relaxed) {
                        return;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                backoff = (backoff * 2).min(60);
            }
        });
    }
    fn fetch_events(&mut self) {
        if !self.logged_in() || self.smoke {
            return;
        }
        if self.notifications_loading {
            self.notifications_pending = true;
            return;
        }
        self.notifications_loading = true;
        self.last_events = Instant::now();
        let (config, session, cursor, tx, generation) = (
            self.config.clone(),
            self.session.clone(),
            self.inbox.cursor.clone(),
            self.tx.clone(),
            self.generation,
        );
        if let Some(session) = session {
            thread::spawn(move || {
                let result = notifications::fetch_page(&config, &session, cursor.as_deref());
                let _ = tx.send(Event::Events(generation, result));
            });
        }
    }
    fn logout(&mut self) -> AppResult<()> {
        self.cancelled.store(true, Ordering::Relaxed);
        self.socket_cancel.store(true, Ordering::Relaxed);
        self.generation += 1;
        self.login_operation += 1;
        self.session = None;
        self.models = None;
        self.grant = None;
        self.services_loading = false;
        self.notifications_loading = false;
        self.notifications_pending = false;
        self.inbox = Inbox::default();
        self.mail = None;
        self.mail_busy = false;
        self.busy = "none";
        self.new_chat();
        self.notification_status = "登入後同步網站通知".into();
        // 歷史屬於 Windows 使用者；下一次登入從空白對話開始，避免隱性沿用舊內容。
        if !self.demo {
            storage::clear_session(&self.root)?;
        }
        Ok(())
    }
    fn begin_chat(&mut self, text: String, action: &str) -> AppResult<()> {
        if !self.can_send() {
            return Err("請確認登入、可用模型與版本狀態後再送出。".into());
        }
        if text.trim().is_empty() {
            return Err("請輸入文字。".into());
        }
        let content = match action {
            "send" | "mail" => text.clone(),
            "translate" => {
                format!("請將以下文字翻譯為繁體中文；若原文為中文則翻譯成英文：\n\n{text}")
            }
            "summarize" => format!("請用繁體中文整理以下內容的重點：\n\n{text}"),
            "polish" => format!("請保留原意並潤飾以下文字：\n\n{text}"),
            _ => return Err("不支援的聊天操作。".into()),
        };
        let mut messages = self.messages.clone();
        messages.push(Message::user(&content));
        if messages.len() >= 40 {
            return Err("此對話已達 20 輪，請新增對話。".into());
        }
        let body = protocol::chat_json(&self.config.model, &messages)?;
        // 送出時就分配穩定的對話 ID；回覆完成時不換 ID，避免誤判切換對話而跳到底。
        if self.active_id.is_none() {
            self.active_id = Some(self.archive.insert(messages.clone())?);
        }
        self.messages = messages;
        self.set_draft(String::new());
        self.busy = "chat";
        self.status = "正在等待 AI 回覆…".into();
        self.error = false;
        let (config, session, tx, generation) = (
            self.config.clone(),
            self.session.clone().ok_or("請先登入。")?,
            self.tx.clone(),
            self.generation,
        );
        let mail_analysis = action == "mail";
        thread::spawn(move || {
            let mut outcome = auth::send_chat(&config, &session, &body);
            if mail_analysis {
                outcome.reply = outcome.reply.map(outlook::format_analysis);
            }
            let _ = tx.send(Event::Chat(generation, outcome));
        });
        Ok(())
    }
    fn read_mail(&mut self, body: bool) -> AppResult<()> {
        if self.mail_busy {
            return Ok(());
        }
        let expected = if body {
            Some(
                self.mail
                    .as_ref()
                    .ok_or("請先讀取郵件基本資訊。")?
                    .entry_id
                    .clone(),
            )
        } else {
            None
        };
        self.mail_busy = true;
        if !body {
            self.mail = None;
        }
        let (tx, generation, demo) = (self.tx.clone(), self.generation, self.demo);
        thread::spawn(move || {
            let result = if demo {
                Ok(outlook::demo_mail(body))
            } else {
                outlook::read_selected(expected.as_deref(), body)
            };
            let _ = tx.send(Event::Mail(generation, result));
        });
        Ok(())
    }
    fn command(&mut self, command: Command) -> AppResult<()> {
        match command {
            Command::StartHotkeyRecording => {
                if let Err(message) = self.start_recording() {
                    // 設定對話框會遮住主畫面的狀態列，錯誤須直接顯示在錄製欄位旁。
                    self.view
                        .post(&json!({"type":"hotkey_error","message":message}))?;
                    return Err(message);
                }
            }
            Command::CancelHotkeyRecording => self.stop_recording()?,
            Command::RecordedHotkey { modifiers, key } => {
                if self.recording_since.is_some() {
                    let hotkey = match Hotkey::from_keys(modifiers, key) {
                        Ok(value) => value,
                        Err(message) => {
                            self.view
                                .post(&json!({"type":"hotkey_error","message":message}))?;
                            return Ok(());
                        }
                    };
                    self.stop_recording()?;
                    self.view
                        .post(&json!({"type":"hotkey_recorded","value":hotkey.label()}))?;
                }
            }
            Command::Ready => {
                self.ready = true;
                self.publish();
                if self.smoke {
                    self.view.post(&json!({"type":"self_test"}))?;
                }
            }
            Command::SelfTestResult { ok, detail } => {
                if self.smoke {
                    self.smoke_result = Some(if ok { Ok(()) } else { Err(detail) });
                }
            }
            Command::Draft { text } => {
                if self.busy != "chat" && text.encode_utf16().count() <= 16_000 {
                    self.draft = text;
                }
            }
            Command::Chat { text, action } => self.begin_chat(text, &action)?,
            Command::NewChat => {
                if self.busy == "none" {
                    self.new_chat();
                }
            }
            Command::Preferences {
                font_size,
                sidebar_collapsed,
                notification_popups,
            } => {
                self.config.font_size = font_size.clamp(12, 20);
                self.config.sidebar_collapsed = sidebar_collapsed;
                self.config.notification_popups = notification_popups;
                storage::save_config(&self.root, &self.config)?;
            }
            Command::Hotkey { value } => {
                if let Err(message) = self.apply_hotkey(value) {
                    self.view
                        .post(&json!({"type":"hotkey_error","message":message}))?;
                    return Err(message);
                }
                self.view
                    .post(&json!({"type":"hotkey_saved","value":self.config.hotkey}))?;
                self.toast("快捷鍵已更新");
            }
            Command::Refresh => self.refresh_services(),
            Command::Download => open_browser(self.config.endpoint(DOWNLOAD_PATH)?.as_str())?,
            Command::OpenLink { url } => open_browser(&url)?,
            Command::Copy { text } => {
                if text.len() > 1_048_576 {
                    return Err("內容過長，無法複製。".into());
                }
                selection::copy_text(self.window, &text)?;
                self.toast("已複製");
            }
            Command::Model { id } => {
                if self.busy == "none"
                    && self
                        .models
                        .as_ref()
                        .is_some_and(|c| c.models.iter().any(|m| m.id == id))
                {
                    self.config.model = id;
                    storage::save_config(&self.root, &self.config)?;
                }
            }
            Command::SelectChat { id } => {
                if self.busy == "none" {
                    let c = self
                        .archive
                        .conversations
                        .iter()
                        .find(|c| c.id == id)
                        .ok_or("找不到對話。")?;
                    self.messages = c.messages.clone();
                    self.active_id = Some(id);
                    self.set_draft(String::new());
                }
            }
            Command::DeleteChat { id } => {
                if self.busy == "none" {
                    let mut archive = self.archive.clone();
                    archive.remove(&id)?;
                    history::save(&self.root, &archive)?;
                    self.archive = archive;
                    if self.active_id.as_ref() == Some(&id) {
                        self.new_chat();
                    }
                }
            }
            Command::Login => {
                if self.busy != "none" || self.versions.blocked() {
                    return Err("請先完成目前操作或更新版本。".into());
                }
                self.busy = "login";
                self.grant = None;
                self.login_operation += 1;
                self.cancelled = Arc::new(AtomicBool::new(false));
                self.status = "正在申請一次性登入碼…".into();
                self.error = false;
                let (config, tx, id) = (self.config.clone(), self.tx.clone(), self.login_operation);
                thread::spawn(move || {
                    let _ = tx.send(Event::Grant(id, auth::request_device(&config)));
                });
            }
            Command::CancelLogin => {
                if self.busy != "login" {
                    return Ok(());
                }
                self.cancelled.store(true, Ordering::Relaxed);
                self.login_operation += 1;
                self.busy = "none";
                self.grant = None;
                self.status = "已取消登入".into();
            }
            Command::ReopenLogin => {
                if let Some(grant) = &self.grant {
                    open_browser(self.config.verification_url(grant.browser_url())?.as_str())?;
                }
            }
            Command::Logout => {
                if self.busy == "none" {
                    self.logout()?;
                    self.status = "已登出".into();
                    self.refresh_services();
                }
            }
            Command::RefreshEvents => self.fetch_events(),
            Command::ReadEvent { id } => {
                if !self.inbox.events.iter().any(|e| e.id == id) {
                    return Err("找不到通知。".into());
                }
                let (config, session, tx, generation) = (
                    self.config.clone(),
                    self.session.clone().ok_or("請先登入。")?,
                    self.tx.clone(),
                    self.generation,
                );
                thread::spawn(move || {
                    let result = notifications::mark_read(&config, &session, &id);
                    let _ = tx.send(Event::Read(generation, id, result));
                });
            }
            Command::ReadMail => self.read_mail(false)?,
            Command::ReadMailBody => self.read_mail(true)?,
            Command::OmitMailBody => {
                if let Some(mail) = &mut self.mail {
                    mail.body = None;
                }
            }
            Command::AnalyzeMail => {
                if !self.can_send() {
                    return Err("請先登入並選擇可用模型。".into());
                }
                let prompt = outlook::analysis_prompt(self.mail.as_ref().ok_or("請先選取郵件。")?)?;
                self.new_chat();
                self.begin_chat(prompt, "mail")?;
            }
        }
        Ok(())
    }
    fn event(&mut self, event: Event) -> AppResult<()> {
        match event {
            Event::Services(generation, version, models) if generation == self.generation => {
                self.services_loading = false;
                self.version_status = match self.versions.apply(version) {
                    Ok(()) => {
                        let info = self.versions.known.as_ref().ok_or("版本狀態錯誤。")?;
                        if info.required() {
                            "此版本已停止支援，請下載更新".into()
                        } else if info.available() {
                            format!("可更新至 {}", info.latest_version)
                        } else {
                            "目前為最新版本".into()
                        }
                    }
                    Err(_) => {
                        if self.versions.blocked() {
                            "請更新至已確認的最低版本".into()
                        } else {
                            "暫時無法檢查版本，允許使用".into()
                        }
                    }
                };
                match models {
                    Ok(c) => {
                        self.config.model =
                            c.models[c.selected_index(&self.config.model)].id.clone();
                        self.models = Some(c);
                    }
                    Err(e) => {
                        self.models = None;
                        self.fail(e);
                    }
                }
                if self.versions.blocked() {
                    self.fail(self.version_status.clone());
                    self.toast("目前版本需更新，請由設定下載新版");
                }
            }
            Event::Grant(id, result) if id == self.login_operation => match result {
                Ok(grant) => {
                    let browser = self.config.verification_url(grant.browser_url())?;
                    self.grant = Some(grant.clone());
                    let (config, tx, cancel) =
                        (self.config.clone(), self.tx.clone(), self.cancelled.clone());
                    thread::spawn(move || {
                        let _ = tx.send(Event::Login(
                            id,
                            auth::wait_for_login(&config, &grant, &cancel),
                        ));
                    });
                    self.status = "請在瀏覽器完成登入授權".into();
                    open_browser(browser.as_str())?;
                }
                Err(e) => {
                    self.busy = "none";
                    return Err(e);
                }
            },
            Event::Login(id, result) if id == self.login_operation => {
                self.busy = "none";
                self.grant = None;
                let session = result?;
                if !self.demo {
                    storage::save_session(&self.root, &session)?;
                }
                self.generation += 1;
                self.session = Some(session);
                self.services_loading = false;
                self.notifications_loading = false;
                self.new_chat();
                self.status = "登入成功".into();
                self.refresh_services();
                self.start_notifications();
            }
            Event::Chat(generation, outcome) if generation == self.generation => {
                self.busy = "none";
                match outcome.reply {
                    Ok(reply) => {
                        self.messages.push(Message::assistant(reply));
                        self.status = "回覆完成".into();
                        if let Err(e) = self.save_history() {
                            self.history_error = Some(e.clone());
                            return Err(e);
                        }
                    }
                    Err(e) => {
                        if let Some(last) = self.messages.pop() {
                            self.set_draft(last.content);
                        }
                        if outcome.unauthorized {
                            self.session = None;
                            self.generation += 1;
                            self.services_loading = false;
                            self.notifications_loading = false;
                            self.notifications_pending = false;
                            self.inbox = Inbox::default();
                            self.models = None;
                            self.mail = None;
                            self.mail_busy = false;
                            self.notification_status = "請重新登入以同步通知".into();
                            self.socket_cancel.store(true, Ordering::Relaxed);
                            if !self.demo {
                                let _ = storage::clear_session(&self.root);
                            }
                        }
                        return Err(e);
                    }
                }
            }
            Event::Capture(result) => {
                self.busy = "none";
                let text = result?;
                let joined = if self.draft.trim().is_empty() {
                    text
                } else {
                    format!("{}\n\n{text}", self.draft)
                };
                if joined.encode_utf16().count() > 16_000 {
                    return Err("目前草稿加上選字太長，原草稿已保留。".into());
                }
                self.set_draft(joined);
                self.focus_draft = true;
                self.status = "剪貼簿文字已帶入，確認後再送出".into();
                // 先將草稿送到介面，前景切換成功與否都不影響已讀取的文字。
                self.publish();
                if show_window(self.window) {
                    self.view.focus();
                } else {
                    self.status = "文字已帶入；請點工作列的 LM_AI 查看草稿".into();
                }
            }
            Event::Mail(generation, result) if generation == self.generation => {
                self.mail_busy = false;
                self.mail = Some(result?);
                self.status = "已讀取郵件預覽，尚未傳給 AI".into();
            }
            Event::Socket(generation, connected) if generation == self.generation => {
                if connected {
                    self.notification_status = "即時通知已連線".into();
                }
                self.fetch_events();
            }
            Event::Events(generation, result) if generation == self.generation => {
                self.notifications_loading = false;
                match result {
                    Ok(page) => {
                        let more = page.has_more;
                        let mut inbox = self.inbox.clone();
                        let added = inbox.merge(page)?;
                        notifications::save(&self.root, &inbox)?;
                        self.inbox = inbox;
                        self.notification_status = "通知已同步；即時通道與定期補查並行".into();
                        if added > 0 && self.config.notification_popups {
                            tray(
                                self.window,
                                NIM_MODIFY,
                                Some(&format!("收到 {added} 則新通知，點一下查看。")),
                            );
                        }
                        if more || self.notifications_pending {
                            self.notifications_pending = false;
                            self.fetch_events();
                        }
                    }
                    Err(e) => {
                        self.notifications_pending = false;
                        self.notification_status = e;
                    }
                }
            }
            Event::Read(generation, id, result) if generation == self.generation => {
                result?;
                let mut inbox = self.inbox.clone();
                if let Some(event) = inbox.events.iter_mut().find(|e| e.id == id) {
                    event.read_at = Some(notifications::now_text());
                }
                notifications::save(&self.root, &inbox)?;
                self.inbox = inbox;
            }
            _ => {} // 登出或重新登入前的工作，不得更新新登入工作階段。
        }
        Ok(())
    }
    fn tick(&mut self) {
        let mut changed = false;
        if self.suppress_hotkey && crate::hotkey::pressed_modifiers() == 0 {
            self.suppress_hotkey = false;
        }
        if self
            .recording_since
            .is_some_and(|start| start.elapsed() > Duration::from_secs(15))
        {
            if let Err(e) = self.stop_recording() {
                self.fail(e);
            }
            self.toast("錄製已逾時，原快捷鍵維持不變");
            changed = true;
        }
        while let Ok(message) = self.commands.try_recv() {
            changed = true;
            match serde_json::from_str::<Command>(&message) {
                Ok(command) => {
                    if let Err(e) = self.command(command) {
                        self.fail(e);
                    }
                }
                Err(_) => self.fail("介面操作格式不正確。".into()),
            }
        }
        while let Ok(event) = self.rx.try_recv() {
            changed = true;
            if let Err(e) = self.event(event) {
                self.fail(e);
            }
        }
        if !self.smoke {
            if self.session.is_some() && !self.logged_in() && self.busy == "none" {
                let _ = self.logout();
                self.fail("登入已到期，請重新登入。".into());
                changed = true;
            }
            if self.last_events.elapsed() > Duration::from_secs(60) {
                self.last_events = Instant::now();
                self.fetch_events();
                changed = true;
            }
            if self.last_services.elapsed() > Duration::from_secs(300) {
                self.refresh_services();
                changed = true;
            }
        }
        // 自我檢查開始後由前端管理虛構資料，避免主程式的空白狀態覆蓋測試畫面。
        if changed && !self.smoke {
            self.publish();
        }
    }
    fn capture(&mut self) {
        if self.recording_since.is_some() || self.suppress_hotkey {
            return;
        }
        if self.busy != "none" {
            self.toast("請等待目前操作完成後再擷取");
            return;
        }
        if let Some(hotkey) = self.hotkey {
            let source = unsafe { GetForegroundWindow() } as usize;
            let tx = self.tx.clone();
            self.busy = "capture";
            thread::spawn(move || {
                let _ = tx.send(Event::Capture(selection::capture(source as HWND, hotkey)));
            });
        }
    }
}
fn open_browser(value: &str) -> AppResult<()> {
    let url = url::Url::parse(value).map_err(|_| "連結格式不正確。")?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || value.chars().any(char::is_control)
    {
        return Err("只允許 HTTP(S) 網頁連結。".into());
    }
    let result = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            wide("open").as_ptr(),
            wide(value).as_ptr(),
            ptr::null(),
            ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if result as isize <= 32 {
        Err("無法開啟預設瀏覽器。".into())
    } else {
        Ok(())
    }
}
fn show_window(window: HWND) -> bool {
    unsafe {
        ShowWindow(window, SW_SHOWNOACTIVATE);
        let activated = SetForegroundWindow(window) != 0;
        if !activated {
            let info = FLASHWINFO {
                cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
                hwnd: window,
                dwFlags: FLASHW_TRAY | FLASHW_TIMERNOFG,
                uCount: 3,
                dwTimeout: 0,
            };
            FlashWindowEx(&info);
        }
        activated
    }
}
/// 原生通知不搶焦點，點擊後才顯示主視窗；Windows 可自行停用彈出提示。
fn tray(window: HWND, operation: u32, message: Option<&str>) {
    let mut data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: window,
        uID: 1,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
        uCallbackMessage: TRAY_MESSAGE,
        ..Default::default()
    };
    data.hIcon = unsafe { LoadIconW(ptr::null_mut(), IDI_APPLICATION) };
    for (dest, source) in data.szTip.iter_mut().zip(wide("LM_AI")) {
        *dest = source;
    }
    if let Some(message) = message {
        data.uFlags = NIF_INFO;
        data.dwInfoFlags = NIIF_INFO;
        for (dest, source) in data.szInfoTitle.iter_mut().zip(wide("LM_AI 通知")) {
            *dest = source;
        }
        for (dest, source) in data.szInfo.iter_mut().take(255).zip(wide(message)) {
            *dest = source;
        }
    }
    unsafe {
        Shell_NotifyIconW(operation, &data);
    }
}
unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_DESTROY {
        unsafe {
            PostQuitMessage(0);
        }
        return 0;
    }
    let pointer = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) } as *const RefCell<App>;
    if !pointer.is_null() {
        // COM 可能重入視窗程序，try_borrow_mut 避免重疊的可變參照。
        if let Ok(mut app) = unsafe { &*pointer }.try_borrow_mut() {
            match message {
                WM_ACTIVATEAPP if wparam == 0 => {
                    if let Err(e) = app.stop_recording() {
                        app.fail(e);
                    }
                    return 0;
                }
                WM_TIMER => {
                    app.tick();
                    return 0;
                }
                WM_SIZE => {
                    let mut rect = RECT::default();
                    unsafe {
                        GetClientRect(window, &mut rect);
                    }
                    app.view.resize(rect.right, rect.bottom);
                    return 0;
                }
                WM_HOTKEY if wparam == selection::HOTKEY_ID as usize => {
                    app.capture();
                    return 0;
                }
                TRAY_MESSAGE if matches!(lparam as u32, WM_LBUTTONUP | NIN_BALLOONUSERCLICK) => {
                    show_window(window);
                    let _ = app.view.post(&json!({"type":"show_notifications"}));
                    return 0;
                }
                WM_GETMINMAXINFO => {
                    let info = unsafe { &mut *(lparam as *mut MINMAXINFO) };
                    info.ptMinTrackSize = POINT { x: 640, y: 500 };
                    return 0;
                }
                WM_DPICHANGED => {
                    let rect = unsafe { &*(lparam as *const RECT) };
                    unsafe {
                        SetWindowPos(
                            window,
                            ptr::null_mut(),
                            rect.left,
                            rect.top,
                            rect.right - rect.left,
                            rect.bottom - rect.top,
                            SWP_NOZORDER | SWP_NOACTIVATE,
                        );
                    }
                    return 0;
                }
                _ => {}
            }
        }
    }
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}
struct ComApartment;
impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe {
            windows::Win32::System::Com::CoUninitialize();
        }
    }
}
/// 自我檢查使用隔離資料夾，實際載入 WebView2 與 Markdown DOM，不連公司服務。
pub fn run(demo: Option<&DemoServer>, smoke: bool) -> AppResult<()> {
    unsafe {
        windows::Win32::System::Com::CoInitializeEx(
            None,
            windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
        )
        .ok()
    }
    .map_err(|_| "無法初始化桌面介面。")?;
    let _apartment = ComApartment;
    let root = if smoke {
        std::env::temp_dir().join("CompanyAI-ui-smoke")
    } else {
        let root = storage::data_dir()?;
        if demo.is_some() {
            root.join("demo")
        } else {
            root
        }
    };
    let config = if let Some(demo) = demo {
        demo.config()
    } else if smoke {
        Config::default()
    } else {
        storage::load_config(&root)?
    };
    let session = if demo.is_none() && !smoke {
        storage::load_session(&root, &config)?
    } else {
        None
    };
    let (archive, history_error) = match history::load(&root) {
        Ok(a) => (a, None),
        Err(e) => (Archive::default(), Some(e)),
    };
    let (tx, rx) = mpsc::channel();
    let (command_tx, commands) = mpsc::channel();
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance = GetModuleHandleW(ptr::null());
        let name = wide("LM_AI_Window");
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: name.as_ptr(),
            hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
            hIcon: LoadIconW(ptr::null_mut(), IDI_APPLICATION),
            ..Default::default()
        };
        if RegisterClassW(&class) == 0 {
            return Err("無法註冊視窗。".into());
        }
        let window = CreateWindowExW(
            0,
            name.as_ptr(),
            wide("LM_AI").as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1120,
            800,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null(),
        );
        if window.is_null() {
            return Err("無法建立視窗。".into());
        }
        let view = WebView::create(window, &root.join("webview"), command_tx)?;
        let mut rect = RECT::default();
        GetClientRect(window, &mut rect);
        view.resize(rect.right, rect.bottom);
        let mut app = App {
            window,
            view,
            root,
            config,
            session,
            archive,
            active_id: None,
            messages: Vec::new(),
            history_error,
            draft: String::new(),
            draft_revision: 0,
            focus_draft: false,
            busy: "none",
            status: "準備就緒".into(),
            error: false,
            models: None,
            versions: VersionState::default(),
            version_status: "正在檢查版本".into(),
            services_loading: false,
            generation: 0,
            login_operation: 0,
            cancelled: Arc::new(AtomicBool::new(false)),
            grant: None,
            inbox: Inbox::default(),
            notifications_loading: false,
            notifications_pending: false,
            notification_status: "登入後同步網站通知".into(),
            socket_cancel: Arc::new(AtomicBool::new(false)),
            last_events: Instant::now(),
            last_services: Instant::now(),
            mail: None,
            mail_busy: false,
            hotkey: None,
            recording_since: None,
            suppress_hotkey: false,
            tx,
            rx,
            commands,
            demo: demo.is_some(),
            smoke,
            ready: false,
            smoke_result: None,
        };
        if !smoke {
            if let Err(e) = app.apply_hotkey(app.config.hotkey.clone()) {
                app.fail(e);
            }
            app.refresh_services();
            app.start_notifications();
            tray(window, NIM_ADD, None);
        }
        let state = Box::new(RefCell::new(app));
        SetWindowLongPtrW(
            window,
            GWLP_USERDATA,
            (&*state as *const RefCell<App>) as isize,
        );
        SetTimer(window, 1, 50, None);
        if !smoke {
            ShowWindow(window, SW_SHOW);
        }
        let started = Instant::now();
        let mut result = Ok(());
        loop {
            let mut message = MSG::default();
            if PeekMessageW(&mut message, ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                if message.message == WM_QUIT {
                    break;
                }
                TranslateMessage(&message);
                DispatchMessageW(&message);
            } else {
                thread::sleep(Duration::from_millis(5));
            }
            if smoke {
                if let Some(done) = state.borrow_mut().smoke_result.take() {
                    result = done;
                    break;
                }
                if started.elapsed() > Duration::from_secs(45) {
                    result = Err("WebView2 介面自我檢查逾時。".into());
                    break;
                }
            }
        }
        KillTimer(window, 1);
        SetWindowLongPtrW(window, GWLP_USERDATA, 0);
        drop(state);
        if IsWindow(window) != 0 {
            DestroyWindow(window);
        }
        UnregisterClassW(name.as_ptr(), instance);
        result
    }
}
pub fn show_fatal_error(error: &str) {
    unsafe {
        MessageBoxW(
            ptr::null_mut(),
            wide(error).as_ptr(),
            wide("LM_AI 啟動失敗").as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}
