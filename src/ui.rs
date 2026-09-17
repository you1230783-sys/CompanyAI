//! Windows 原生桌面介面。UI 只在主執行緒存取；背景工作透過 channel 回報。
//! 路由藏在 Config 的程式預設值；一般使用者只操作登入、模型與自己的草稿。
use crate::{
    appearance::{self, Appearance},
    auth,
    config::{Config, DOWNLOAD_PATH},
    demo::DemoServer,
    protocol::{self, DeviceGrant, Message},
    selection::{self, Hotkey},
    service::{self, ModelCatalog, VersionInfo, VersionState},
    storage::{self, Session},
    wide, AppResult,
};
use std::{
    cell::RefCell,
    collections::HashMap,
    path::PathBuf,
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Controls::*, HiDpi::*, Input::KeyboardAndMouse::*, Shell::ShellExecuteW,
        WindowsAndMessaging::*,
    },
};

const LOGIN: usize = 101;
const LOGOUT: usize = 102;
const CLEAR: usize = 103;
const REFRESH: usize = 104;
const DOWNLOAD: usize = 105;
const HOTKEY_EDIT: usize = 106;
const HOTKEY_SAVE: usize = 107;
const MODEL: usize = 108;
const PROMPT: usize = 109;
const REPLY: usize = 110;
const SEND: usize = 111;
const TRANSLATE: usize = 112;
const SUMMARIZE: usize = 113;
const POLISH: usize = 114;
const COPY: usize = 115;
const STATUS: usize = 116;
const ACCOUNT: usize = 117;
const VERSION: usize = 118;
const CANCEL: usize = 119;
const REOPEN: usize = 120;
const FOOTER: usize = 121;

/// 每個可取消工作攜帶流水號，避免舊登入或舊擷取覆蓋新操作。
enum Event {
    Device(u64, DeviceGrant),
    Login(u64, AppResult<Session>),
    Chat(u64, auth::ChatOutcome, String),
    Capture(u64, AppResult<String>),
    Version(AppResult<VersionInfo>),
    Models(u64, AppResult<ModelCatalog>),
}
#[derive(Clone, Copy, PartialEq)]
enum Busy {
    None,
    Login,
    Chat,
    Capture,
}
struct App {
    window: HWND,
    controls: HashMap<usize, HWND>,
    style: Appearance,
    scale: f32,
    root: PathBuf,
    config: Config,
    session: Option<Session>,
    history: Vec<Message>,
    verification_url: Option<String>,
    busy: Busy,
    operation: u64,
    cancelled: Arc<AtomicBool>,
    tx: Sender<Event>,
    rx: Receiver<Event>,
    demo: bool,
    hotkey: Option<Hotkey>,
    versions: VersionState,
    version_loading: bool,
    model_loading: bool,
    model_generation: u64,
    catalog: Option<ModelCatalog>,
    last_check: Instant,
    last_update_notice: Option<String>,
}
impl App {
    fn control(&self, id: usize) -> HWND {
        self.controls.get(&id).copied().unwrap_or(ptr::null_mut())
    }
    fn set(&self, id: usize, text: &str) {
        unsafe {
            SetWindowTextW(
                self.control(id),
                wide(&text.replace('\n', "\r\n").replace("\r\r\n", "\r\n")).as_ptr(),
            );
        }
    }
    fn text(&self, id: usize) -> String {
        // 同一 UI 執行緒讀值，先配置含結尾零的 UTF-16 緩衝區。
        unsafe {
            let control = self.control(id);
            let length = GetWindowTextLengthW(control).max(0) as usize;
            let mut buffer = vec![0; length + 1];
            let read =
                GetWindowTextW(control, buffer.as_mut_ptr(), buffer.len() as i32).max(0) as usize;
            String::from_utf16_lossy(&buffer[..read]).replace("\r\n", "\n")
        }
    }
    fn status(&self, text: &str) {
        self.set(STATUS, text);
    }
    fn alert(&self, text: &str) {
        self.status(text);
        unsafe {
            MessageBoxW(
                self.window,
                wide(text).as_ptr(),
                wide("Company AI").as_ptr(),
                MB_OK | MB_ICONINFORMATION,
            );
        }
    }
    fn create_control(&mut self, id: usize, class: &str, text: &str, style: u32) -> AppResult<()> {
        // 父視窗擁有子控制項；字型由 Appearance 保持至所有視窗銷毀。
        let control = unsafe {
            CreateWindowExW(
                0,
                wide(class).as_ptr(),
                wide(text).as_ptr(),
                WS_CHILD | WS_VISIBLE | style,
                0,
                0,
                1,
                1,
                self.window,
                id as HMENU,
                GetModuleHandleW(ptr::null()),
                ptr::null(),
            )
        };
        if control.is_null() {
            return Err(format!("無法建立介面控制項 {id}。"));
        }
        let font = if matches!(id, 201 | 203) {
            self.style.title
        } else if matches!(id, 202 | 204 | 205 | VERSION | FOOTER) {
            self.style.small
        } else {
            self.style.font
        };
        unsafe {
            SendMessageW(control, WM_SETFONT, font as usize, 1);
        }
        self.controls.insert(id, control);
        Ok(())
    }
    fn build_controls(&mut self) -> AppResult<()> {
        for (id, text) in [
            (201, "Company AI"),
            (202, "你的工作助理"),
            (203, "今天想一起完成什麼？"),
            (204, "選取文字快捷鍵"),
            (
                205,
                "在其他程式選取文字，按快捷鍵即可帶入草稿。\n確認後才會傳送。",
            ),
            (206, "快速處理"),
            (STATUS, "正在取得服務資訊…"),
            (ACCOUNT, "尚未登入"),
            (VERSION, "版本檢查中…"),
            (
                FOOTER,
                "Enter 換行 · Ctrl+Enter 送出 · 對話僅保留於本次視窗",
            ),
        ] {
            self.create_control(id, "STATIC", text, 0)?;
        }
        for (id, text) in [
            (LOGIN, "瀏覽器登入"),
            (LOGOUT, "登出"),
            (CLEAR, "＋  新對話"),
            (REFRESH, "重新整理服務"),
            (DOWNLOAD, "下載更新"),
            (HOTKEY_SAVE, "套用快捷鍵"),
            (SEND, "送出  ↑"),
            (TRANSLATE, "翻譯"),
            (SUMMARIZE, "摘要"),
            (POLISH, "潤飾"),
            (COPY, "複製回覆"),
            (CANCEL, "取消登入"),
            (REOPEN, "重開登入網頁"),
        ] {
            self.create_control(id, "BUTTON", text, WS_TABSTOP | BS_OWNERDRAW as u32)?;
        }
        self.create_control(HOTKEY_EDIT, "EDIT", "", WS_TABSTOP | ES_AUTOHSCROLL as u32)?;
        for id in [PROMPT, REPLY] {
            self.create_control(
                id,
                "EDIT",
                "",
                WS_TABSTOP
                    | WS_VSCROLL
                    | ES_MULTILINE as u32
                    | ES_AUTOVSCROLL as u32
                    | ES_WANTRETURN as u32
                    | if id == REPLY { ES_READONLY as u32 } else { 0 },
            )?;
            unsafe {
                SendMessageW(
                    self.control(id),
                    EM_SETLIMITTEXT,
                    if id == PROMPT { 16_000 } else { 2_000_000 },
                    0,
                );
            }
        }
        self.create_control(
            MODEL,
            "COMBOBOX",
            "",
            WS_TABSTOP | CBS_DROPDOWNLIST as u32 | WS_VSCROLL,
        )?;
        unsafe {
            SendMessageW(self.control(HOTKEY_EDIT), EM_SETLIMITTEXT, 64, 0);
        }
        self.set(HOTKEY_EDIT, &self.config.hotkey);
        if self.demo {
            self.set(202, "本機示範 · 非真實 AI");
        }
        self.render_history();
        self.show_session();
        self.update_enabled();
        Ok(())
    }
    fn place(&self, id: usize, x: i32, y: i32, width: i32, height: i32) {
        let px = |n: i32| (n as f32 * self.scale).round() as i32;
        unsafe {
            MoveWindow(
                self.control(id),
                px(x),
                px(y),
                px(width.max(1)),
                px(height.max(1)),
                1,
            );
        }
    }
    fn layout(&self) {
        let mut rect = RECT::default();
        unsafe {
            GetClientRect(self.window, &mut rect);
        }
        let w = (rect.right as f32 / self.scale) as i32;
        let h = (rect.bottom as f32 / self.scale) as i32;
        self.place(201, 24, 30, 180, 32);
        self.place(202, 24, 68, 175, 24);
        self.place(ACCOUNT, 24, 103, 175, 26);
        self.place(LOGIN, 24, 139, 172, 36);
        self.place(CLEAR, 24, 189, 172, 36);
        self.place(LOGOUT, 24, 235, 172, 34);
        self.place(204, 24, 289, 175, 24);
        self.place(HOTKEY_EDIT, 24, 316, 172, 29);
        self.place(HOTKEY_SAVE, 24, 358, 172, 32);
        self.place(205, 24, 408, 175, 78);
        self.place(VERSION, 24, h - 172, 175, 46);
        self.place(REFRESH, 24, h - 114, 172, 34);
        self.place(DOWNLOAD, 24, h - 66, 172, 34);
        self.place(203, 250, 28, w - 430, 38);
        self.place(COPY, w - 152, 28, 128, 34);
        self.place(STATUS, 250, 77, w - 274, 44);
        self.place(REPLY, 258, 146, w - 300, h - 455);
        self.place(206, 250, h - 273, 78, 24);
        self.place(TRANSLATE, 332, h - 279, 85, 32);
        self.place(SUMMARIZE, 429, h - 279, 85, 32);
        self.place(POLISH, 526, h - 279, 85, 32);
        self.place(CANCEL, 332, h - 279, 114, 32);
        self.place(REOPEN, 460, h - 279, 164, 32);
        self.place(PROMPT, 256, h - 209, w - 294, 111);
        self.place(MODEL, 258, h - 78, 210, 180);
        self.place(SEND, w - 152, h - 82, 114, 36);
        self.place(FOOTER, 250, h - 25, w - 275, 20);
        appearance::invalidate(self.window);
    }
    fn show_session(&self) {
        self.set(
            ACCOUNT,
            if self
                .session
                .as_ref()
                .is_some_and(|s| s.valid_for(&self.config))
            {
                "●  已登入"
            } else {
                "○  尚未登入"
            },
        );
    }
    fn can_send(&self) -> bool {
        self.busy == Busy::None
            && !self.versions.blocked()
            && self.catalog.is_some()
            && !self.model_loading
            && self
                .session
                .as_ref()
                .is_some_and(|s| s.valid_for(&self.config))
    }
    fn update_enabled(&self) {
        let idle = self.busy == Busy::None;
        unsafe {
            for id in [LOGIN, LOGOUT, CLEAR, HOTKEY_EDIT, HOTKEY_SAVE, PROMPT] {
                EnableWindow(
                    self.control(id),
                    i32::from(idle && (id != LOGIN || !self.versions.blocked())),
                );
            }
            EnableWindow(
                self.control(MODEL),
                i32::from(idle && self.catalog.is_some() && !self.model_loading),
            );
            for id in [SEND, TRANSLATE, SUMMARIZE, POLISH] {
                EnableWindow(self.control(id), i32::from(self.can_send()));
            }
            EnableWindow(
                self.control(REFRESH),
                i32::from(!self.version_loading && !self.model_loading),
            );
            EnableWindow(
                self.control(COPY),
                i32::from(self.history.iter().any(|m| m.role == "assistant")),
            );
            for id in [CANCEL, REOPEN] {
                ShowWindow(
                    self.control(id),
                    if self.busy == Busy::Login {
                        SW_SHOW
                    } else {
                        SW_HIDE
                    },
                );
            }
            EnableWindow(
                self.control(REOPEN),
                i32::from(self.verification_url.is_some()),
            );
            for id in [TRANSLATE, SUMMARIZE, POLISH] {
                ShowWindow(
                    self.control(id),
                    if self.busy == Busy::Login {
                        SW_HIDE
                    } else {
                        SW_SHOW
                    },
                );
            }
        }
        self.show_session();
        appearance::invalidate(self.window);
    }
    fn save_preferences(&mut self) -> AppResult<()> {
        if let Some(catalog) = &self.catalog {
            let index = unsafe { SendMessageW(self.control(MODEL), CB_GETCURSEL, 0, 0) };
            if let Some(model) = catalog.models.get(index as usize) {
                self.config.model = model.id.clone();
            }
        }
        storage::save_config(&self.root, &self.config)
    }
    /// 先驗證新組合，再更換註冊；衝突時盡力恢復原快捷鍵。
    fn apply_hotkey(&mut self, save: bool) -> AppResult<()> {
        let value = self.text(HOTKEY_EDIT).trim().to_string();
        let updated = Hotkey::parse(&value)?;
        let old = self.hotkey.take();
        selection::unregister(self.window);
        if let Err(error) = selection::register(self.window, updated) {
            if let Some(old) = old {
                if selection::register(self.window, old).is_ok() {
                    self.hotkey = Some(old);
                }
            }
            self.set(HOTKEY_EDIT, &self.config.hotkey);
            return Err(error);
        }
        self.hotkey = Some(updated);
        self.config.hotkey = value;
        if save {
            self.save_preferences()?;
            self.status("快捷鍵已套用。選取文字後即可帶入草稿。");
        }
        Ok(())
    }
    /// 版本查詢與模型查詢各自回報，版本斷線不會阻擋可用的模型服務。
    fn refresh_services(&mut self) {
        if !self.version_loading {
            self.version_loading = true;
            let (config, tx) = (self.config.clone(), self.tx.clone());
            thread::spawn(move || {
                let _ = tx.send(Event::Version(service::fetch_version(&config)));
            });
        }
        if !self.model_loading {
            self.refresh_models();
        }
        self.last_check = Instant::now();
        self.update_enabled();
    }
    fn refresh_models(&mut self) {
        self.model_generation += 1;
        self.model_loading = true;
        let (config, session, tx, generation) = (
            self.config.clone(),
            self.session.clone(),
            self.tx.clone(),
            self.model_generation,
        );
        thread::spawn(move || {
            let _ = tx.send(Event::Models(
                generation,
                service::fetch_models(&config, session.as_ref()),
            ));
        });
        self.update_enabled();
    }
    fn begin_login(&mut self) -> AppResult<()> {
        if self.versions.blocked() {
            return Err("請先更新應用程式。".into());
        }
        self.busy = Busy::Login;
        self.operation += 1;
        self.cancelled = Arc::new(AtomicBool::new(false));
        self.verification_url = None;
        self.status("準備登入；瀏覽器開啟後，請核對代碼並確認授權。");
        self.update_enabled();
        let (config, tx, operation, cancelled) = (
            self.config.clone(),
            self.tx.clone(),
            self.operation,
            self.cancelled.clone(),
        );
        thread::spawn(move || {
            let result = (|| {
                let grant = auth::request_device(&config)?;
                if cancelled.load(Ordering::Relaxed) {
                    return Err("已取消登入。".into());
                }
                let _ = tx.send(Event::Device(operation, grant.clone()));
                auth::wait_for_login(&config, &grant, &cancelled)
            })();
            let _ = tx.send(Event::Login(operation, result));
        });
        Ok(())
    }
    fn begin_chat(&mut self, action: usize) -> AppResult<()> {
        if !self.can_send() {
            return Err("請確認已登入、取得模型選單，並完成必要更新。".into());
        }
        self.save_preferences()?;
        let session = self.session.clone().ok_or("請先登入。")?;
        let draft = self.text(PROMPT);
        if draft.trim().is_empty() {
            return Err("請先輸入訊息，或用快捷鍵帶入選取文字。".into());
        }
        let instruction = match action {
            TRANSLATE => {
                "請翻譯以下文字：外語翻成繁體中文；中文翻成英文。保留原意，不補充未提供的事實。"
            }
            SUMMARIZE => "請以繁體中文摘要以下文字，列出主要重點，不補充未提供的事實。",
            POLISH => "請潤飾以下文字，保留原本語言、事實與立場，讓表達清楚自然。",
            _ => "",
        };
        let prompt = if instruction.is_empty() {
            draft
        } else {
            format!("{instruction}\n\n{draft}")
        };
        let mut messages = self.history.clone();
        messages.push(Message::user(&prompt));
        let body = protocol::chat_json(&self.config.model, &messages)?;
        self.busy = Busy::Chat;
        self.operation += 1;
        self.status("正在整理回覆…");
        self.update_enabled();
        let (config, tx, operation) = (self.config.clone(), self.tx.clone(), self.operation);
        thread::spawn(move || {
            let result = auth::send_chat(&config, &session, &body);
            let _ = tx.send(Event::Chat(operation, result, prompt));
        });
        Ok(())
    }
    /// 先在來源程式完成複製，背景結果回來才顯示本視窗；絕不自動送出。
    fn begin_capture(&mut self) {
        if self.busy != Busy::None {
            return;
        }
        let Some(hotkey) = self.hotkey else {
            return;
        };
        let source = unsafe { GetForegroundWindow() };
        if source == self.window {
            unsafe {
                SetFocus(self.control(PROMPT));
            }
            return;
        }
        self.busy = Busy::Capture;
        self.operation += 1;
        self.update_enabled();
        let (tx, operation, source) = (self.tx.clone(), self.operation, source as usize);
        thread::spawn(move || {
            let result = selection::capture(source as HWND, hotkey);
            let _ = tx.send(Event::Capture(operation, result));
        });
    }
    fn cancel_login(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
        self.operation += 1;
        self.busy = Busy::None;
        self.verification_url = None;
        self.update_enabled();
    }
    fn on_button(&mut self, id: usize) -> AppResult<()> {
        if self.busy != Busy::None && !matches!(id, CANCEL | REOPEN | DOWNLOAD | COPY | REFRESH) {
            return Ok(());
        }
        match id {
            LOGIN => self.begin_login()?,
            LOGOUT => {
                storage::clear_session(&self.root)?;
                self.session = None;
                self.history.clear();
                self.render_history();
                self.refresh_models();
                self.status("已清除本機登入與對話。");
            }
            CLEAR => {
                self.history.clear();
                self.render_history();
                self.status("已開始新對話；未送出的草稿保留。");
            }
            SEND | TRANSLATE | SUMMARIZE | POLISH => self.begin_chat(id)?,
            CANCEL => {
                self.cancel_login();
                self.status("已取消登入。");
            }
            REOPEN => {
                if let Some(url) = &self.verification_url {
                    open_browser(url)?;
                }
            }
            REFRESH => self.refresh_services(),
            DOWNLOAD => open_browser(self.config.endpoint(DOWNLOAD_PATH)?.as_str())?,
            HOTKEY_SAVE => self.apply_hotkey(true)?,
            COPY => {
                if let Some(reply) = self.history.iter().rev().find(|m| m.role == "assistant") {
                    selection::copy_text(self.window, &reply.content)?;
                    self.status("回覆已複製，可以貼到其他程式。");
                }
            }
            _ => {}
        }
        self.update_enabled();
        Ok(())
    }
    fn render_history(&self) {
        let text = if self.history.is_empty() {
            "把想法變成下一步。\n\n直接提問，或在其他程式選取文字，再用快捷鍵帶入。\n你可以翻譯、摘要、潤飾，也可以接著追問。".into()
        } else {
            self.history
                .iter()
                .map(|m| {
                    format!(
                        "{}\n\n{}",
                        if m.role == "user" {
                            "你"
                        } else {
                            "Company AI"
                        },
                        m.content
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n────────────────────\n\n")
        };
        self.set(REPLY, &text);
        if !self.history.is_empty() {
            unsafe {
                SendMessageW(self.control(REPLY), EM_SETSEL, usize::MAX, -1);
                SendMessageW(self.control(REPLY), EM_SCROLLCARET, 0, 0);
            }
        }
    }
    fn apply_version(&mut self, result: AppResult<VersionInfo>) {
        self.version_loading = false;
        let failed = self.versions.apply(result).is_err();
        let label = if self.versions.blocked() {
            "需要更新"
        } else if failed {
            "檢查失敗 · 暫時可用"
        } else if self
            .versions
            .known
            .as_ref()
            .is_some_and(VersionInfo::available)
        {
            "有新版本可下載"
        } else {
            "已是可用版本"
        };
        self.set(VERSION, &format!("v{}\n{label}", service::CURRENT_VERSION));
        if self.versions.blocked() {
            if self.busy == Busy::Login {
                self.cancel_login();
            }
            let info = self
                .versions
                .known
                .as_ref()
                .expect("blocked requires known version");
            self.status("此版本已停止支援，請下載新版後繼續使用。");
            if self.last_update_notice.as_ref() != Some(&info.minimum_version) {
                self.last_update_notice = Some(info.minimum_version.clone());
                let text = format!(
                    "請更新到 {} 或更新版本後繼續使用。\n{}\n\n現在開啟下載頁？",
                    info.minimum_version, info.message
                );
                let answer = unsafe {
                    MessageBoxW(
                        self.window,
                        wide(&text).as_ptr(),
                        wide("Company AI 需要更新").as_ptr(),
                        MB_YESNO | MB_ICONINFORMATION,
                    )
                };
                if answer == IDYES {
                    if let Err(error) = self.on_button(DOWNLOAD) {
                        self.alert(&error);
                    }
                }
            }
        }
        self.update_enabled();
    }
    fn apply_models(&mut self, result: AppResult<ModelCatalog>) {
        self.model_loading = false;
        unsafe {
            SendMessageW(self.control(MODEL), CB_RESETCONTENT, 0, 0);
        }
        match result {
            Ok(catalog) => {
                let index = catalog.selected_index(&self.config.model);
                for model in &catalog.models {
                    unsafe {
                        SendMessageW(
                            self.control(MODEL),
                            CB_ADDSTRING,
                            0,
                            wide(&model.label).as_ptr() as isize,
                        );
                    }
                }
                unsafe {
                    SendMessageW(self.control(MODEL), CB_SETCURSEL, index, 0);
                }
                self.config.model = catalog.models[index].id.clone();
                self.catalog = Some(catalog);
                if self.busy == Busy::None && !self.versions.blocked() {
                    self.status(if self.session.is_some() {
                        "準備好了，從一個問題開始吧。"
                    } else {
                        "請先使用瀏覽器登入。"
                    });
                }
            }
            Err(error) => {
                self.catalog = None;
                self.status(&error);
            }
        }
        self.update_enabled();
    }
    fn finish_capture(&mut self, result: AppResult<String>) {
        self.busy = Busy::None;
        self.update_enabled();
        unsafe {
            ShowWindow(self.window, SW_RESTORE);
            SetForegroundWindow(self.window);
        }
        match result {
            Ok(text) => {
                let current = self.text(PROMPT);
                let draft = if current.trim().is_empty() {
                    text
                } else {
                    format!("{current}\n\n{text}")
                };
                if draft.encode_utf16().count() > 16_000 {
                    self.alert("草稿合併後太長，原草稿已保留。擷取內容仍可從剪貼簿手動貼上。");
                    return;
                }
                self.set(PROMPT, &draft);
                self.status("選取文字已帶入草稿，尚未傳送。請選擇處理方式或直接送出。");
                unsafe {
                    SetFocus(self.control(PROMPT));
                    SendMessageW(self.control(PROMPT), EM_SETSEL, usize::MAX, -1);
                }
            }
            Err(error) => self.alert(&error),
        }
    }
    fn poll_events(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                Event::Version(result) => self.apply_version(result),
                Event::Models(generation, result) if generation == self.model_generation => {
                    self.apply_models(result)
                }
                Event::Capture(operation, result) if operation == self.operation => {
                    self.finish_capture(result)
                }
                Event::Device(operation, grant) if operation == self.operation => {
                    let url = grant.browser_url().to_string();
                    self.verification_url = Some(url.clone());
                    self.status(&format!(
                        "請在瀏覽器核對代碼 {} 並授權；有效 {} 秒。",
                        grant.user_code, grant.expires_in
                    ));
                    if let Err(error) = open_browser(&url) {
                        self.alert(&error);
                    }
                    self.update_enabled();
                }
                Event::Login(operation, result) if operation == self.operation => {
                    self.busy = Busy::None;
                    self.verification_url = None;
                    match result {
                        Ok(session) => {
                            let saved = storage::save_session(&self.root, &session);
                            self.session = Some(session);
                            self.history.clear();
                            self.render_history();
                            self.status("登入完成，正在取得可用模型…");
                            self.refresh_models();
                            if let Err(error) = saved {
                                self.alert(&format!(
                                    "已登入，但無法保存；下次需重新登入。\n{error}"
                                ));
                            }
                        }
                        Err(error) => self.alert(&error),
                    }
                    self.update_enabled();
                }
                Event::Chat(operation, outcome, prompt) if operation == self.operation => {
                    self.busy = Busy::None;
                    if outcome.unauthorized {
                        self.session = None;
                        if let Err(error) = storage::clear_session(&self.root) {
                            self.alert(&error);
                        }
                    }
                    match outcome.reply {
                        Ok(reply) => {
                            self.history.push(Message::user(&prompt));
                            self.history.push(Message::assistant(reply));
                            self.render_history();
                            self.set(PROMPT, "");
                            self.status("回覆已完成。可以繼續追問，或複製到其他程式。");
                        }
                        Err(error) => self.alert(&error),
                    }
                    self.update_enabled();
                }
                _ => {} // 已取消或已被新登入／新模型查詢取代的結果不採用。
            }
        }
        if self.last_check.elapsed() >= Duration::from_secs(15 * 60) {
            self.refresh_services();
        }
    }

    /// 驗證實際 Win32 選單與送出按鈕的接線；所有假資料只留在記憶體。
    fn self_check(&mut self) -> AppResult<()> {
        if self.controls.len() != 27
            || self.text(HOTKEY_EDIT) != self.config.hotkey
            || self.can_send()
        {
            return Err("介面控制項自我檢查失敗。".into());
        }
        let catalog: ModelCatalog = serde_json::from_str(r#"{"models":[{"id":"fast","label":"快速"},{"id":"quality","label":"品質"}],"default_model":"quality"}"#).map_err(|e|e.to_string())?;
        self.apply_models(Ok(catalog));
        self.session = Some(Session {
            access_token: "self-check-only".into(),
            expires_at: crate::unix_now() + 60,
            binding: self.config.binding()?,
        });
        self.apply_version(Err("模擬版本服務斷線".into()));
        if self.config.model != "quality"
            || self.text(MODEL) != "品質"
            || !self.can_send()
            || unsafe { IsWindowEnabled(self.control(SEND)) } == 0
        {
            return Err("模型選單或版本檢查失敗時允許使用的接線有誤。".into());
        }
        // 不經提示框，只驗證已知強制更新與後續斷線時的按鈕門檻。
        self.versions.apply(Ok(VersionInfo {
            latest_version: "99.0.0".into(),
            minimum_version: "99.0.0".into(),
            message: String::new(),
        }))?;
        let _ = self.versions.apply(Err("模擬再次斷線".into()));
        self.update_enabled();
        if self.can_send() || unsafe { IsWindowEnabled(self.control(SEND)) } != 0 {
            return Err("已知強制更新未正確阻擋送出。".into());
        }
        Ok(())
    }
}

fn open_browser(url: &str) -> AppResult<()> {
    // 網址由固定設定或同站登入驗證限制；直接 ShellExecute，不拼接命令列。
    let result = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            wide("open").as_ptr(),
            wide(url).as_ptr(),
            ptr::null(),
            ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if result as isize <= 32 {
        Err("無法開啟預設瀏覽器，請確認 Windows 預設應用程式。".into())
    } else {
        Ok(())
    }
}
/// SetWindowText 等呼叫可能重入；try_borrow_mut 避免重疊的可變 Rust 參照。
unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_DESTROY {
        PostQuitMessage(0);
        return 0;
    }
    let state = GetWindowLongPtrW(window, GWLP_USERDATA) as *const RefCell<App>;
    if !state.is_null() {
        if let Ok(mut app) = (*state).try_borrow_mut() {
            match message {
                WM_PAINT => {
                    let mut paint = PAINTSTRUCT::default();
                    let dc = BeginPaint(window, &mut paint);
                    let mut rect = RECT::default();
                    GetClientRect(window, &mut rect);
                    app.style.background(dc, rect.right, rect.bottom, app.scale);
                    EndPaint(window, &paint);
                    return 0;
                }
                WM_ERASEBKGND => return 1,
                WM_DRAWITEM => {
                    let draw = &*(lparam as *const DRAWITEMSTRUCT);
                    let id = draw.CtlID as usize;
                    app.style.button(
                        draw,
                        &app.text(id),
                        matches!(id, LOGIN | SEND),
                        matches!(
                            id,
                            LOGIN | LOGOUT | CLEAR | REFRESH | DOWNLOAD | HOTKEY_SAVE
                        ),
                    );
                    return 1;
                }
                WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
                    let id = GetDlgCtrlID(lparam as HWND) as usize;
                    let dc = wparam as HDC;
                    let nav = matches!(id, 201 | 202 | 204 | 205 | ACCOUNT | VERSION);
                    let surface = matches!(id, PROMPT | REPLY | HOTKEY_EDIT | MODEL)
                        || message == WM_CTLCOLORLISTBOX;
                    let color = if nav {
                        appearance::NAV
                    } else if surface {
                        appearance::SURFACE
                    } else {
                        appearance::CANVAS
                    };
                    SetBkColor(dc, color);
                    SetTextColor(
                        dc,
                        if nav {
                            appearance::rgb(221, 226, 234)
                        } else if matches!(id, STATUS | FOOTER | 206) {
                            appearance::MUTED
                        } else {
                            appearance::INK
                        },
                    );
                    return if nav {
                        app.style.nav
                    } else if surface {
                        app.style.surface
                    } else {
                        app.style.canvas
                    } as isize;
                }
                WM_SIZE => {
                    app.layout();
                    return 0;
                }
                WM_GETMINMAXINFO => {
                    let info = &mut *(lparam as *mut MINMAXINFO);
                    info.ptMinTrackSize.x = (960.0 * app.scale) as i32;
                    info.ptMinTrackSize.y = (730.0 * app.scale) as i32;
                    return 0;
                }
                WM_HOTKEY if wparam == selection::HOTKEY_ID as usize => {
                    app.begin_capture();
                    return 0;
                }
                WM_TIMER => {
                    app.poll_events();
                    return 0;
                }
                WM_COMMAND => {
                    let id = wparam & 0xffff;
                    let notification = (wparam >> 16) as u32;
                    let result = if id == MODEL && notification == CBN_SELCHANGE {
                        app.save_preferences()
                    } else if notification == BN_CLICKED {
                        app.on_button(id)
                    } else {
                        Ok(())
                    };
                    if let Err(error) = result {
                        app.alert(&error);
                    }
                    return 0;
                }
                WM_CLOSE => {
                    app.cancelled.store(true, Ordering::Relaxed);
                    selection::unregister(window);
                }
                _ => {}
            }
        }
    }
    DefWindowProcW(window, message, wparam, lparam)
}

/// 自我檢查只建立控制項，不註冊快捷鍵、不讀剪貼簿、不登入或連線。
pub fn run(demo: Option<&DemoServer>, smoke_check: bool) -> AppResult<()> {
    let root = if smoke_check {
        std::env::temp_dir().join("CompanyAI-ui-smoke")
    } else {
        let root = storage::data_dir()?;
        if demo.is_some() {
            root.join("demo")
        } else {
            root
        }
    };
    let mut startup_error = None;
    let config = if let Some(server) = demo {
        server.config()
    } else if smoke_check {
        Config::default()
    } else {
        storage::load_config(&root).unwrap_or_else(|error| {
            startup_error = Some(error);
            Config::default()
        })
    };
    let session = if demo.is_some() || smoke_check {
        None
    } else {
        storage::load_session(&root, &config).unwrap_or_else(|error| {
            startup_error = Some(error);
            None
        })
    };
    // 本函式擁有視窗與 RefCell，直到訊息迴圈結束後才釋放；背景執行緒不持有 App。
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_SYSTEM_AWARE);
        let scale = GetDpiForSystem() as f32 / 96.0;
        let style = Appearance::new(scale)?;
        let instance = GetModuleHandleW(ptr::null());
        let class_name = wide("CompanyAIWindowV3");
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: class_name.as_ptr(),
            hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
            hbrBackground: style.canvas,
            ..std::mem::zeroed()
        };
        if RegisterClassW(&class) == 0 {
            return Err("無法註冊 Windows 視窗。".into());
        }
        let mut work = RECT::default();
        SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut work as *mut RECT as *mut _, 0);
        let window = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            class_name.as_ptr(),
            wide(concat!("Company AI ", env!("CARGO_PKG_VERSION"))).as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            ((1120.0 * scale) as i32).min(work.right - work.left - 40),
            ((820.0 * scale) as i32).min(work.bottom - work.top - 40),
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null(),
        );
        if window.is_null() {
            UnregisterClassW(class_name.as_ptr(), instance);
            return Err("無法建立應用程式視窗。".into());
        }
        let (tx, rx) = mpsc::channel();
        let mut app = App {
            window,
            controls: HashMap::new(),
            style,
            scale,
            root,
            config,
            session,
            history: Vec::new(),
            verification_url: None,
            busy: Busy::None,
            operation: 0,
            cancelled: Arc::new(AtomicBool::new(false)),
            tx,
            rx,
            demo: demo.is_some(),
            hotkey: None,
            versions: VersionState::default(),
            version_loading: false,
            model_loading: false,
            model_generation: 0,
            catalog: None,
            last_check: Instant::now(),
            last_update_notice: None,
        };
        if let Err(error) = app.build_controls() {
            DestroyWindow(window);
            UnregisterClassW(class_name.as_ptr(), instance);
            return Err(error);
        }
        app.layout();
        let state = Box::new(RefCell::new(app));
        SetWindowLongPtrW(
            window,
            GWLP_USERDATA,
            (&*state as *const RefCell<App>) as isize,
        );
        let mut result = Ok(());
        if smoke_check {
            result = state.borrow_mut().self_check();
        } else {
            {
                let mut app = state.borrow_mut();
                if let Err(error) = app.apply_hotkey(false) {
                    app.set(205, &error);
                }
                app.refresh_services();
            }
            SetTimer(window, 1, 100, None);
            ShowWindow(window, SW_SHOW);
            UpdateWindow(window);
            if let Some(error) = startup_error {
                state.borrow().alert(&error);
            }
            let mut message = MSG::default();
            loop {
                let received = GetMessageW(&mut message, ptr::null_mut(), 0, 0);
                if received <= 0 {
                    if received < 0 {
                        result = Err("Windows 訊息處理失敗。".into());
                    }
                    break;
                }
                let send = message.message == WM_KEYDOWN
                    && message.wParam == VK_RETURN as usize
                    && GetAsyncKeyState(VK_CONTROL as i32) < 0
                    && message.hwnd == state.borrow().control(PROMPT);
                if send {
                    let mut app = state.borrow_mut();
                    if let Err(error) = app.on_button(SEND) {
                        app.alert(&error);
                    }
                    continue;
                }
                if IsDialogMessageW(window, &message) == 0 {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
        }
        state.borrow().cancelled.store(true, Ordering::Relaxed);
        selection::unregister(window);
        SetWindowLongPtrW(window, GWLP_USERDATA, 0);
        if IsWindow(window) != 0 {
            DestroyWindow(window);
        }
        UnregisterClassW(class_name.as_ptr(), instance);
        result
    }
}
pub fn show_fatal_error(error: &str) {
    unsafe {
        MessageBoxW(
            ptr::null_mut(),
            wide(error).as_ptr(),
            wide("Company AI 啟動失敗").as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}
