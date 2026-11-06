//! 主視窗與工作流程。背景工作只透過訊息傳回資料，不跨執行緒操作 WebView2。

mod bootstrap;
mod commands;
mod events;
mod session;
mod updates;
use crate::{
    auth,
    config::Config,
    demo::DemoServer,
    history::{self, Archive},
    notifications::{self, Inbox},
    outlook::{self, MailPreview},
    protocol::Message,
    selection::{self, Hotkey},
    service::{self, ModelCatalog, VersionState},
    storage::{self, Session},
    transport,
    webview::WebView,
    wide, AppResult,
};
use bootstrap::*;
pub use bootstrap::{
    run, run_local, run_local_scenario, run_tutorial_preview, show_fatal_error,
    verify_panic_recovery,
};
use serde::Deserialize;
use serde_json::json;
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
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
mod access;
mod branches;
mod diagnostics;
mod document_export;
mod handoff;
mod mail_batch;
mod onboarding;
mod pet;
mod projects;
mod reminders;
mod retry;
mod schedules;
mod screenshot;
mod site;
#[cfg(feature = "tarot")]
mod tarot;
mod templates;
mod tutorial_preview;
#[cfg(feature = "vnc")]
mod vnc;
mod window_behavior;
mod work;
mod workspace;
const TRAY_MESSAGE: u32 = WM_APP + 4;

/// 明確列舉介面命令，不提供任意檔案、程式或任意 API 執行入口。
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
// 停用VNC後次大variant縮小，但Project命令仍只有312 bytes。
// 保留既有按值派送，避免單為變體比例增加每個專案命令的堆積配置。
#[cfg_attr(not(feature = "vnc"), allow(clippy::large_enum_variant))]
enum Command {
    Schedule {
        command: schedules::ScheduleCommand,
    },
    DocumentExport {
        command: document_export::ExportCommand,
    },
    Workspace {
        command: workspace::WorkspaceCommand,
    },
    Branch {
        command: branches::BranchCommand,
    },
    Handoff {
        command: handoff::HandoffCommand,
    },
    Template {
        command: templates::TemplateCommand,
    },
    Screenshot {
        command: screenshot::ScreenshotCommand,
    },
    Reminder {
        command: reminders::ReminderCommand,
    },
    #[cfg(feature = "pet")]
    Pet {
        command: pet::PetCommand,
    },
    #[cfg(feature = "tarot")]
    Tarot {
        command: tarot::TarotCommand,
    },
    OnboardingWelcomeDismiss,
    OnboardingAck {
        topic: String,
    },
    Project {
        command: projects::ProjectCommand,
    },
    #[cfg(feature = "vnc")]
    Vnc {
        command: vnc::VncCommand,
    },
    SiteAction {
        command: crate::site_notifications::Action,
    },
    MailBatch {
        command: mail_batch::MailCommand,
    },
    Work {
        command: work::WorkCommand,
    },
    ExportDiagnostics {
        include_details: bool,
    },
    SaveDiagnostics {
        preview_id: String,
    },
    CancelDiagnostics,
    CopyError,
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
    RetryChat {
        conversation_id: String,
        user_index: usize,
        message_count: usize,
        request_id: Option<String>,
    },
    NewChat,
    Behavior {
        hotkey_enabled: bool,
        selection_icon: bool,
        enter_sends: bool,
        quick_actions_fast: bool,
        always_new_chat: bool,
        #[serde(default)]
        vnc_enabled: bool,
        #[serde(default)]
        extra_features_enabled: bool,
    },
    RenameChat {
        id: String,
        title: String,
    },
    PinChat {
        id: String,
        pinned: bool,
    },
    CollapseSection {
        section: String,
        collapsed: bool,
    },
    Preferences {
        font_size: u8,
        #[serde(default)]
        ui_font: Option<crate::config::UiFont>,
        #[serde(default)]
        interface_scale: Option<u16>,
        sidebar_collapsed: bool,
        notification_popups: bool,
        #[serde(default)]
        minimize_to_taskbar: Option<bool>,
        #[serde(default)]
        dark_mode: Option<bool>,
    },
    Hotkey {
        value: String,
    },
    ChartPalette {
        colors: Vec<String>,
    },
    Login,
    Logout,
    CancelLogin,
    ReopenLogin,
    Refresh,
    Download,
    Exit,
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
    AllEvents {
        dismiss: bool,
    },
    NotificationsLeft,
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
    DocumentExport(String, u64, AppResult<serde_json::Value>),
    Screenshot(String, AppResult<Vec<u8>>),
    SmokePng(AppResult<()>),
    Project(projects::ProjectEvent),
    #[cfg(feature = "vnc")]
    VncSync(String, crate::vnc::sync::SessionEvent),
    #[cfg(feature = "vnc")]
    VncSearch(String, AppResult<PathBuf>),
    Site(u64, site::SiteEvent),
    MailBatch(u64, u64, mail_batch::MailEvent),
    Work(u64, work::WorkEvent),
    Services(
        u64,
        AppResult<service::VersionInfo>,
        AppResult<ModelCatalog>,
    ),
    Grant(u64, AppResult<auth::LoginGrant>),
    Login(u64, AppResult<auth::LoginResult>),
    Capture(u64, AppResult<String>),
    UpdateReady(AppResult<crate::deployment::ReadyUpdate>),
    SelectionRect(Option<crate::selection_popup::SelectionRect>),
    Mail(u64, AppResult<MailPreview>),
    Events(u64, AppResult<notifications::EventPage>),
    Read(u64, String, AppResult<()>),
    Socket(u64, bool),
    AllRead(u64, usize),
}
struct App {
    panic_probe: bool,
    panic_fault: bool,
    diagnostic_preview: Option<(String, Vec<u8>)>,
    reminders: reminders::ReminderRuntime,
    schedules: schedules::Runtime,
    templates: templates::TemplateRuntime,
    screenshot: screenshot::Runtime,
    handoff: Option<handoff::Pending>,
    workspace_import: Option<workspace::Pending>,
    document_export: document_export::Runtime,
    #[cfg(feature = "pet")]
    pet: crate::pet::Pet,
    #[cfg(feature = "tarot")]
    tarot: crate::tarot::Round,
    onboarding: onboarding::Onboarding,
    projects: projects::ProjectRuntime,
    #[cfg(feature = "vnc")]
    vnc: vnc::VncRuntime,
    site: site::SiteRuntime,
    mail_flow: mail_batch::MailRuntime,
    work: work::WorkRuntime,
    window: HWND,
    selection_popup: crate::selection_popup::Popup,
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
    /// 每次錯誤都有新序號，讓同一操作再次失敗時仍顯示通知。
    status_notice_id: u64,
    error: bool,
    models: Option<ModelCatalog>,
    versions: VersionState,
    version_status: String,
    update_ready: Option<crate::deployment::ReadyUpdate>,
    update_busy: bool,
    /// 僅當次啟動、指定版本的下載及安裝同意；不跨重啟保存。
    update_consent: Option<String>,
    update_status: String,
    services_loading: bool,
    generation: u64,
    login_operation: u64,
    cancelled: Arc<AtomicBool>,
    grant: Option<auth::LoginGrant>,
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
        // SAFETY: 固定識別碼只屬於本視窗；尚未註冊或視窗已銷毀時呼叫會安全失敗。
        unsafe {
            windows_sys::Win32::UI::Input::KeyboardAndMouse::UnregisterHotKey(
                self.window,
                crate::screenshot::HOTKEY_ID,
            );
        }
        self.projects.cancel();
        self.schedules.cancel();
        #[cfg(feature = "vnc")]
        self.vnc.shutdown();
        let _ = self.preserve_draft();
        self.mail_flow.cancel.store(true, Ordering::Relaxed);
        self.cancelled.store(true, Ordering::Relaxed);
        self.socket_cancel.store(true, Ordering::Relaxed);
        selection::unregister(self.window);
        tray(self.window, NIM_DELETE);
    }
}
impl App {
    /// WebView 子視窗與主視窗視為同一個前景 App。
    fn is_foreground(&self) -> bool {
        unsafe { GetAncestor(GetForegroundWindow(), GA_ROOT) == self.window }
    }
    fn logged_in(&self) -> bool {
        self.session
            .as_ref()
            .is_some_and(|s| s.valid_for(&self.config))
    }
    fn can_send(&self) -> bool {
        !crate::reliability::faulted()
            && self.logged_in()
            && self.busy == "none"
            && !self
                .active_id
                .as_deref()
                .is_some_and(|id| self.schedules.active_conversation(id))
            && !(self
                .active_id
                .as_deref()
                .and_then(|id| self.projects.store.project_for(id))
                .is_some()
                && self.project_slots_full())
            && !self.projects.active(self.active_id.as_deref())
            && self.work_ready()
            && !self.versions.blocked()
            && self
                .models
                .as_ref()
                .is_some_and(|c| c.models.iter().any(|m| m.id == self.config.model))
    }
    fn publish(&mut self) {
        if !self.logged_in() || self.versions.blocked() {
            self.projects.cancel();
            self.schedules.cancel();
        }
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
            .map(|c| json!({"id":c.id,"title":c.title,"updated_at":c.updated_at,"pinned":c.pinned,"project_id":self.projects.store.conversations.get(&c.id),"handoff_source":c.handoff_source}))
            .collect();
        let mut events:Vec<serde_json::Value>=self.inbox.events.iter().filter(|e|!e.expired()&&!e.dismissed&&e.visible_ai()&&!crate::projects::events::internal(&self.root,e.resource_id.as_deref())).map(|e|json!({
            "id":e.id,"source":"ai","type":e.kind,"title":e.title,"summary":e.summary,"created_at":e.created_at,"read_at":e.read_at,"notification_key":format!("ai:{}",e.id)
        })).collect();
        events.extend(self.site.cache.items.iter().filter(|e|!crate::projects::events::internal(&self.root,e.resource_id.as_deref())).map(|e|json!({"id":e.id,"source":"site","origin":e.source,"type":e.kind,"title":e.title,"summary":e.body,"created_at":e.created_at,"read_at":if e.is_read{Some(e.read_at.clone().unwrap_or_default())}else{None},"is_read":e.is_read,"url":e.url,"resource_id":e.resource_id,"received_at":e.received_at,"notification_key":format!("site:{}",e.id)})));
        events.sort_by(|a, b| {
            b["created_at"]
                .as_str()
                .cmp(&a["created_at"].as_str())
                .then_with(|| {
                    a["notification_key"]
                        .as_str()
                        .cmp(&b["notification_key"].as_str())
                })
        });
        let branches = self.branch_state();
        let mut state_payload = json!({"type":"state","state":{
            "update_available":self.versions.known.as_ref().is_some_and(|info| info.available()),
            "update_status":self.update_status,"update_ready":self.update_ready.is_some(),"update_busy":self.update_busy,
            "update_kind":self.update_ready.as_ref().map(|ready|ready.artifact.kind.as_str()),
            "version":service::CURRENT_VERSION,"onboarding":self.onboarding.state(),"config":self.config,"status":self.status,"status_notice_id":self.status_notice_id,"error":self.error,
            "busy":self.busy,"logged_in":self.logged_in(),"can_send":self.can_send(),"update_required":self.versions.blocked(),
            "retry":self.retry_state(),"projects":self.project_state(),"models":models,"conversations":conversations,"active_id":self.active_id,"messages":self.messages,
            "draft":self.draft,"draft_revision":self.draft_revision,"focus_draft":self.focus_draft,
            "notifications":events,"notification_status":self.notification_status,"unread_count":self.inbox.events.iter().filter(|e| e.visible_ai()&&!e.dismissed&&!e.expired()&&e.read_at.is_none()&&!crate::projects::events::internal(&self.root,e.resource_id.as_deref())).count()+self.site.cache.unread_count.saturating_sub(self.site.cache.items.iter().filter(|e|!e.is_read&&crate::projects::events::internal(&self.root,e.resource_id.as_deref())).count()),"site_status":self.site.status,"site_loading":self.site.loading,"site_mutating":self.site.mutating,"notifications_loading":self.notifications_loading,
            "mail":self.mail,"mail_busy":self.mail_busy,"mail_batch":self.mail_batch_state(),"history_error":self.history_error,
            "work":self.work_state(),"vnc":self.vnc_state(),"version_status":self.version_status,"login_code":self.grant.as_ref().map(|g|&g.grant.user_code)
        }});
        state_payload["state"]["branches"] = branches;
        // 額外能力以獨立欄位加入，避免大型json!巨集超過展開深度。
        state_payload["state"]["build_features"] = crate::build_features::state();
        state_payload["state"]["schedule_running"] = json!(self.schedules.active());
        // 原生目錄亦套用個人偏好；前端同規則保護搜尋與直接開啟。
        if let Some(tutorials) = state_payload["state"]["onboarding"]["tutorials"].as_array_mut() {
            tutorials.retain(|entry| {
                let id = entry["id"].as_str().unwrap_or_default();
                (!id.starts_with("vnc.") || self.config.vnc_enabled)
                    && (!matches!(id, "settings.pet" | "settings.tarot")
                        || self.config.extra_features_enabled)
            });
        }
        let _ = self.view.post(&state_payload);
        self.focus_draft = false;
    }
    fn fail(&mut self, error: String) {
        let error = if error.starts_with("Error Code: ") {
            error
        } else {
            self.friendly_error("interface", &error)
        };
        self.status_notice_id = self.status_notice_id.wrapping_add(1);
        // 強制更新對話框會遮住主畫面狀態列，下載／安裝錯誤須在對話框內可見。
        if self.versions.blocked() {
            self.update_status = error.clone();
        }
        self.status = error;
        self.error = true;
    }
    fn toast(&self, text: &str) {
        let _ = self.view.post(&json!({"type":"toast","text":text}));
    }
    fn set_draft(&mut self, text: String) {
        self.draft = text;
        if let Some(c) = self
            .archive
            .conversations
            .iter_mut()
            .find(|c| Some(&c.id) == self.active_id.as_ref())
        {
            c.draft = self.draft.clone();
        }
        self.draft_revision += 1;
    }
    /// 換頁前保留草稿，避免托盤還原或快捷鍵開新對話時丟失輸入。
    fn preserve_draft(&mut self) -> AppResult<()> {
        if self.draft.is_empty() && self.active_id.is_none() {
            return Ok(());
        }
        if self.history_error.is_some() {
            return Err("本機紀錄無法保存，請先處理後再切換對話。".into());
        }
        if self.active_id.is_none() {
            self.active_id = Some(self.archive.insert(Vec::new())?);
        }
        if let Some(c) = self
            .archive
            .conversations
            .iter_mut()
            .find(|c| Some(&c.id) == self.active_id.as_ref())
        {
            c.draft = self.draft.clone();
            if c.messages.is_empty() && !c.title_manual {
                c.title = format!("草稿：{}", self.draft.chars().take(30).collect::<String>());
            }
        }
        history::save(&self.root, &self.archive)
    }
    fn restore_window(&mut self) {
        if self.logged_in()
            && self.config.always_new_chat
            && self.work.incoming.is_none()
            && !self.versions.blocked()
        {
            self.new_chat();
        }
        self.publish();
        show_window(self.window);
    }
    fn new_chat(&mut self) {
        if let Err(e) = self.preserve_draft() {
            self.fail(e);
            return;
        }
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
        if self.recording_since.is_some() {
            self.stop_recording()?;
        }
        self.unregister_screenshot_hotkey();
        let result = self.apply_selection_hotkey(value);
        self.sync_screenshot_hotkey();
        result
    }
    fn apply_selection_hotkey(&mut self, value: String) -> AppResult<()> {
        let next = Hotkey::parse(&value)?;
        selection::unregister(self.window);
        if let Err(e) = if self.config.hotkey_enabled && self.logged_in() {
            selection::register(self.window, next)
        } else {
            Ok(())
        } {
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
        self.hotkey = (self.config.hotkey_enabled && self.logged_in()).then_some(next);
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
        self.unregister_screenshot_hotkey();
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
        self.sync_screenshot_hotkey();
        Ok(())
    }

    fn capture(&mut self) {
        if !self.logged_in()
            || self.versions.blocked()
            || self.recording_since.is_some()
            || self.suppress_hotkey
        {
            return;
        }
        if self.busy != "none" {
            self.toast("請等待目前操作完成後再擷取");
            return;
        }
        if let Some(hotkey) = self.hotkey {
            let source = unsafe { GetForegroundWindow() } as usize;
            let tx = self.tx.clone();
            let generation = self.generation;
            self.busy = "capture";
            crate::reliability::spawn(move || {
                let _ = tx.send(Event::Capture(
                    generation,
                    selection::capture(source as HWND, hotkey),
                ));
            });
        }
    }
}

#[cfg(not(feature = "vnc"))]
impl App {
    fn vnc_state(&self) -> serde_json::Value {
        json!({"available":false,"enabled":false})
    }
}
