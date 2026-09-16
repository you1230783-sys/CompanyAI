//! Windows 原生介面。Win32 呼叫集中在此模組，其餘業務邏輯不依賴控制項。
//! UI 只在主執行緒存取；背景執行緒以 channel 回報結果，不碰 HWND。
use crate::{
    auth,
    config::{AuthHeader, Config},
    demo::DemoServer,
    protocol::{self, DeviceGrant, Message},
    storage::{self, Session},
    unix_now, wide, AppResult,
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
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Controls::*, HiDpi::*, Input::KeyboardAndMouse::EnableWindow, Shell::ShellExecuteW,
        WindowsAndMessaging::*,
    },
};

// 控制項 ID 使用有意義的名稱，新增功能時不必猜數字代表哪個按鈕。
const SERVER: usize = 101;
const ROUTE: usize = 102;
const MODEL: usize = 103;
const HEADER: usize = 104;
const HTTP: usize = 105;
const SAVE: usize = 106;
const LOGIN: usize = 107;
const CANCEL: usize = 108;
const LOGOUT: usize = 109;
const CODE: usize = 110;
const STATUS: usize = 111;
const PROMPT: usize = 112;
const PREVIEW: usize = 113;
const SEND: usize = 114;
const CLEAR: usize = 115;
const REPLY: usize = 116;
const REOPEN: usize = 117;

enum Event {
    Device(DeviceGrant),
    Login(AppResult<Session>),
    Chat(auth::ChatOutcome, String),
}
struct Envelope {
    operation: u64,
    event: Event,
}
#[derive(Clone, Copy, PartialEq)]
enum Busy {
    None,
    Login,
    Chat,
}

struct App {
    window: HWND,
    controls: HashMap<usize, HWND>,
    font: HFONT,
    scale: f32,
    root: PathBuf,
    config: Config,
    session: Option<Session>,
    history: Vec<Message>,
    verification_url: Option<String>,
    busy: Busy,
    operation: u64,
    cancelled: Arc<AtomicBool>,
    tx: Sender<Envelope>,
    rx: Receiver<Envelope>,
    demo: bool,
}

impl App {
    fn control(&self, id: usize) -> HWND {
        self.controls.get(&id).copied().unwrap_or(ptr::null_mut())
    }
    fn set(&self, id: usize, text: &str) {
        // SAFETY: 控制項隸屬仍存活的主視窗，字串在同步呼叫期間有效。
        unsafe {
            SetWindowTextW(
                self.control(id),
                wide(&text.replace('\n', "\r\n").replace("\r\r\n", "\r\n")).as_ptr(),
            );
        }
    }
    fn text(&self, id: usize) -> String {
        // SAFETY: 依照 UTF-16 長度配置含結尾零的緩衝區；同一 UI 執行緒不會有並行修改。
        unsafe {
            let control = self.control(id);
            let length = GetWindowTextLengthW(control).max(0) as usize;
            let mut text = vec![0_u16; length + 1];
            let read =
                GetWindowTextW(control, text.as_mut_ptr(), text.len() as i32).max(0) as usize;
            String::from_utf16_lossy(&text[..read]).replace("\r\n", "\n")
        }
    }
    fn status(&self, text: &str) {
        self.set(STATUS, text);
    }
    fn alert(&self, text: &str) {
        self.status(text);
        // SAFETY: MessageBox 在同一 UI 執行緒顯示；文字不含憑證。
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
        // SAFETY: 使用 Windows 內建控制項類別；父視窗負責銷毀子控制項。
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
            return Err(format!("無法建立 Windows 控制項 {id}。"));
        }
        unsafe {
            SendMessageW(control, WM_SETFONT, self.font as usize, 1);
        }
        self.controls.insert(id, control);
        Ok(())
    }

    fn build_controls(&mut self) -> AppResult<()> {
        let title = if self.demo {
            "Company AI  ·  本機示範（非真實 AI）"
        } else {
            "Company AI  ·  連線測試版"
        };
        for (id, text) in [
            (201, title),
            (202, "網站網址"),
            (203, "API 路徑"),
            (204, "模型"),
            (205, "驗證 Header"),
            (206, "登入碼／網址"),
            (207, "訊息"),
            (208, "將送出的 JSON（不含 API Key）"),
            (209, "對話回覆（僅存於本次視窗）"),
        ] {
            self.create_control(id, "STATIC", text, 0)?;
        }
        for id in [SERVER, ROUTE, MODEL, CODE] {
            let read_only = if id == CODE { ES_READONLY } else { 0 };
            self.create_control(
                id,
                "EDIT",
                "",
                WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL as u32 | read_only as u32,
            )?;
            unsafe {
                SendMessageW(self.control(id), EM_SETLIMITTEXT, 2048, 0);
            }
        }
        self.create_control(
            HEADER,
            "COMBOBOX",
            "",
            WS_TABSTOP | CBS_DROPDOWNLIST as u32 | WS_VSCROLL,
        )?;
        unsafe {
            SendMessageW(
                self.control(HEADER),
                CB_ADDSTRING,
                0,
                wide("Authorization: Bearer").as_ptr() as isize,
            );
            SendMessageW(
                self.control(HEADER),
                CB_ADDSTRING,
                0,
                wide("X-API-Key").as_ptr() as isize,
            );
        }
        self.create_control(
            HTTP,
            "BUTTON",
            "允許內網 HTTP（會明文傳輸憑證）",
            WS_TABSTOP | BS_AUTOCHECKBOX as u32,
        )?;
        for (id, text) in [
            (SAVE, "儲存設定"),
            (LOGIN, "瀏覽器登入"),
            (CANCEL, "取消登入"),
            (LOGOUT, "清除本機登入"),
            (SEND, "送出訊息"),
            (CLEAR, "清除對話"),
            (REOPEN, "重開登入網頁"),
        ] {
            self.create_control(id, "BUTTON", text, WS_TABSTOP | BS_PUSHBUTTON as u32)?;
        }
        self.create_control(
            STATUS,
            "STATIC",
            "請設定網站與模型，儲存後按「瀏覽器登入」。",
            0,
        )?;
        for id in [PROMPT, PREVIEW, REPLY] {
            let read_only = if id == PROMPT { 0 } else { ES_READONLY };
            self.create_control(
                id,
                "EDIT",
                "",
                WS_BORDER
                    | WS_TABSTOP
                    | WS_VSCROLL
                    | ES_MULTILINE as u32
                    | ES_AUTOVSCROLL as u32
                    | ES_WANTRETURN as u32
                    | read_only as u32,
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
        self.set(SERVER, &self.config.server_url);
        self.set(ROUTE, &self.config.chat_path);
        self.set(MODEL, &self.config.model);
        self.set(REPLY, "在上方輸入訊息，按「送出訊息」測試 API。\n\n程式不會自動讀取剪貼簿、檔案或其他視窗內容。");
        unsafe {
            SendMessageW(
                self.control(HEADER),
                CB_SETCURSEL,
                usize::from(self.config.auth_header == AuthHeader::XApiKey),
                0,
            );
            SendMessageW(
                self.control(HTTP),
                BM_SETCHECK,
                usize::from(self.config.allow_http),
                0,
            );
            SendMessageW(
                self.control(SERVER),
                EM_SETCUEBANNER,
                0,
                wide("https://ai.company.example").as_ptr() as isize,
            );
            SendMessageW(
                self.control(MODEL),
                EM_SETCUEBANNER,
                0,
                wide("公司 API 的模型名稱").as_ptr() as isize,
            );
        }
        self.refresh_preview();
        self.update_enabled();
        self.show_session();
        Ok(())
    }

    fn place(&self, id: usize, x: i32, y: i32, width: i32, height: i32) {
        let px = |value: i32| (value as f32 * self.scale).round() as i32;
        // SAFETY: 只調整本視窗建立的控制項；Windows 負責裁切與重繪。
        unsafe {
            MoveWindow(self.control(id), px(x), px(y), px(width), px(height), 1);
        }
    }

    fn layout(&self) {
        let mut rect = RECT::default();
        unsafe {
            GetClientRect(self.window, &mut rect);
        }
        let width = (rect.right as f32 / self.scale) as i32;
        let height = (rect.bottom as f32 / self.scale) as i32;
        self.place(201, 20, 14, width - 40, 26);
        self.place(202, 20, 56, 110, 24);
        self.place(SERVER, 140, 50, width - 160, 30);
        self.place(203, 20, 95, 110, 24);
        self.place(ROUTE, 140, 89, width - 465, 30);
        self.place(204, width - 305, 95, 50, 24);
        self.place(MODEL, width - 250, 89, 230, 30);
        self.place(205, 20, 134, 110, 24);
        self.place(HEADER, 140, 128, 205, 150);
        self.place(HTTP, 360, 129, width - 505, 30);
        self.place(SAVE, width - 130, 128, 110, 32);
        self.place(LOGIN, 20, 173, 125, 34);
        self.place(CANCEL, 155, 173, 110, 34);
        self.place(LOGOUT, 275, 173, 140, 34);
        self.place(REOPEN, 425, 173, 140, 34);
        self.place(206, 20, 224, 110, 24);
        self.place(CODE, 140, 216, width - 160, 30);
        self.place(STATUS, 20, 257, width - 40, 44);
        let column = (width - 55) / 2;
        self.place(207, 20, 307, column, 24);
        self.place(208, column + 35, 307, column, 24);
        self.place(PROMPT, 20, 335, column, 136);
        self.place(PREVIEW, column + 35, 335, column, 136);
        self.place(SEND, 20, 481, 125, 34);
        self.place(CLEAR, 155, 481, 125, 34);
        self.place(209, 20, 531, width - 40, 24);
        self.place(REPLY, 20, 559, width - 40, (height - 579).max(80));
    }

    fn read_config(&self) -> Config {
        Config {
            server_url: self.text(SERVER).trim().trim_end_matches('/').into(),
            chat_path: self.text(ROUTE).trim().into(),
            model: self.text(MODEL).trim().into(),
            auth_header: if unsafe { SendMessageW(self.control(HEADER), CB_GETCURSEL, 0, 0) } == 1 {
                AuthHeader::XApiKey
            } else {
                AuthHeader::Bearer
            },
            allow_http: unsafe { SendMessageW(self.control(HTTP), BM_GETCHECK, 0, 0) }
                == BST_CHECKED as isize,
        }
    }

    fn save_settings(&mut self) -> AppResult<()> {
        let updated = self.read_config();
        updated.validate()?;
        let changed = self.config.binding().ok() != updated.binding().ok();
        // 先清除舊憑證再切換網站，即使後續保存失敗，也不會把舊 Key 送到新站。
        if changed {
            storage::clear_session(&self.root)?;
            self.session = None;
            self.history.clear();
            self.verification_url = None;
            self.set(CODE, "");
            self.set(REPLY, "連線設定已改變，對話已清除。請重新登入。");
        }
        storage::save_config(&self.root, &updated)?;
        self.config = updated;
        self.refresh_preview();
        self.update_enabled();
        Ok(())
    }

    fn current_messages(&self) -> Vec<Message> {
        let mut messages = self.history.clone();
        let prompt = self.text(PROMPT);
        if !prompt.trim().is_empty() {
            messages.push(Message::user(&prompt));
        }
        messages
    }
    fn refresh_preview(&self) {
        let model = self.text(MODEL);
        let messages = self.current_messages();
        let text = if messages.is_empty() {
            "輸入訊息後，這裡會顯示實際送出的 JSON。".into()
        } else {
            protocol::chat_json(&model, &messages).unwrap_or_else(|error| error)
        };
        self.set(PREVIEW, &text);
    }
    fn show_session(&self) {
        if let Some(session) = self
            .session
            .as_ref()
            .filter(|session| session.valid_for(&self.config))
        {
            let hours = (session.expires_at.saturating_sub(unix_now())).div_ceil(3600);
            self.status(&format!(
                "已登入 · 授權約剩 {hours} 小時。訊息只會在按下「送出訊息」後傳送。"
            ));
        } else {
            self.status("尚未登入。請確認網站與模型，按「瀏覽器登入」取得授權。");
        }
    }
    fn update_enabled(&self) {
        let idle = self.busy == Busy::None;
        for id in [
            SERVER, ROUTE, MODEL, HEADER, HTTP, SAVE, LOGIN, LOGOUT, CLEAR, PROMPT,
        ] {
            unsafe {
                EnableWindow(self.control(id), i32::from(idle));
            }
        }
        unsafe {
            EnableWindow(
                self.control(SEND),
                i32::from(idle && self.session.is_some()),
            );
            EnableWindow(self.control(CANCEL), i32::from(self.busy == Busy::Login));
            EnableWindow(
                self.control(REOPEN),
                i32::from(self.busy == Busy::Login && self.verification_url.is_some()),
            );
        }
    }

    fn begin_login(&mut self) -> AppResult<()> {
        self.save_settings()?;
        self.busy = Busy::Login;
        self.operation += 1;
        self.cancelled = Arc::new(AtomicBool::new(false));
        self.verification_url = None;
        self.set(CODE, "正在向網站取得一次性登入碼…");
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
                let _ = tx.send(Envelope {
                    operation,
                    event: Event::Device(grant.clone()),
                });
                auth::wait_for_login(&config, &grant, &cancelled)
            })();
            let _ = tx.send(Envelope {
                operation,
                event: Event::Login(result),
            });
        });
        Ok(())
    }

    fn begin_chat(&mut self) -> AppResult<()> {
        self.save_settings()?;
        let session = self.session.clone().ok_or("請先完成瀏覽器登入。")?;
        if !session.valid_for(&self.config) {
            return Err("登入已到期，請重新登入。".into());
        }
        let prompt = self.text(PROMPT);
        if prompt.trim().is_empty() {
            return Err("請先輸入訊息。".into());
        }
        let body = protocol::chat_json(&self.config.model, &self.current_messages())?;
        self.set(PREVIEW, &body);
        self.busy = Busy::Chat;
        self.operation += 1;
        self.status("正在等待 API 回覆…（不會自動重試，避免重複送出）");
        self.update_enabled();
        let (config, tx, operation) = (self.config.clone(), self.tx.clone(), self.operation);
        thread::spawn(move || {
            let outcome = auth::send_chat(&config, &session, &body);
            let _ = tx.send(Envelope {
                operation,
                event: Event::Chat(outcome, prompt),
            });
        });
        Ok(())
    }

    fn on_button(&mut self, id: usize) -> AppResult<()> {
        if self.busy != Busy::None && !matches!(id, CANCEL | REOPEN) {
            return Ok(());
        }
        match id {
            SAVE => {
                self.save_settings()?;
                self.status("設定已儲存。變更網站、路由或 Header 類型後，請重新登入。");
            }
            LOGIN => self.begin_login()?,
            CANCEL => {
                self.cancelled.store(true, Ordering::Relaxed);
                self.operation += 1; // 舊背景工作即使稍後完成，也不能覆蓋新的登入狀態。
                self.busy = Busy::None;
                self.verification_url = None;
                self.set(CODE, "");
                self.status("已取消登入。若網頁仍開著，可以直接關閉。");
                self.update_enabled();
            }
            LOGOUT => {
                storage::clear_session(&self.root)?;
                self.session = None;
                self.history.clear();
                self.set(REPLY, "本機登入與對話已清除。伺服器端撤銷請由網站管理。");
                self.set(CODE, "");
                self.show_session();
                self.refresh_preview();
                self.update_enabled();
            }
            SEND => self.begin_chat()?,
            CLEAR => {
                self.history.clear();
                self.set(REPLY, "對話已清除，下次送出會開始新對話。");
                self.refresh_preview();
            }
            REOPEN => {
                if let Some(url) = &self.verification_url {
                    open_browser(url)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn poll_events(&mut self) {
        while let Ok(envelope) = self.rx.try_recv() {
            if envelope.operation != self.operation {
                continue;
            }
            match envelope.event {
                Event::Device(grant) => {
                    let url = grant.browser_url().to_string();
                    self.set(CODE, &format!("{}   |   {}", grant.user_code, url));
                    self.verification_url = Some(url.clone());
                    self.status(&format!(
                        "請在瀏覽器核對代碼 {} 並授權；登入碼有效 {} 秒。",
                        grant.user_code, grant.expires_in
                    ));
                    self.update_enabled();
                    if let Err(error) = open_browser(&url) {
                        self.alert(&format!("{error}\n可複製上方網址手動開啟。"));
                    }
                }
                Event::Login(result) => {
                    self.busy = Busy::None;
                    self.verification_url = None;
                    self.set(CODE, "");
                    match result {
                        Ok(session) => {
                            let saved = storage::save_session(&self.root, &session);
                            // 網頁可能登入另一個帳號；不把前一帳號的聊天歷史帶入新授權。
                            self.history.clear();
                            self.set(REPLY, "登入完成，已開始新的對話。");
                            self.session = Some(session);
                            self.refresh_preview();
                            self.show_session();
                            if let Err(error) = saved {
                                self.alert(&format!(
                                    "已登入，但無法保存，下次需重新登入。\n{error}"
                                ));
                            }
                        }
                        Err(error) => self.alert(&error),
                    }
                    self.update_enabled();
                }
                Event::Chat(outcome, prompt) => {
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
                            let transcript = self
                                .history
                                .iter()
                                .map(|message| {
                                    format!(
                                        "{}\n{}",
                                        if message.role == "user" { "你" } else { "AI" },
                                        message.content
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join("\n\n──────────\n\n");
                            self.set(REPLY, &transcript);
                            self.set(PROMPT, "");
                            self.status("已收到 API 回覆。可繼續提問，或清除對話重新測試。");
                        }
                        Err(error) => self.alert(&error),
                    }
                    self.update_enabled();
                }
            }
        }
    }
}

fn open_browser(url: &str) -> AppResult<()> {
    // SAFETY: 網址已由 Config 限制為同站 http/https，不經 cmd.exe 或 shell 字串拼接。
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
        Err("無法啟動預設瀏覽器。".into())
    } else {
        Ok(())
    }
}

/// Windows 可能在 SetWindowText / MessageBox 中重入訊息處理。
/// RefCell::try_borrow_mut 讓重入通知安全略過，不產生重疊的 Rust 可變參照。
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
                WM_SIZE => {
                    app.layout();
                    return 0;
                }
                WM_GETMINMAXINFO => {
                    let info = &mut *(lparam as *mut MINMAXINFO);
                    info.ptMinTrackSize.x = (900.0 * app.scale) as i32;
                    info.ptMinTrackSize.y = (720.0 * app.scale) as i32;
                    return 0;
                }
                WM_TIMER => {
                    app.poll_events();
                    return 0;
                }
                WM_COMMAND => {
                    let id = wparam & 0xffff;
                    let notification = (wparam >> 16) as u32;
                    if notification == EN_CHANGE && matches!(id, PROMPT | MODEL) {
                        app.refresh_preview();
                    }
                    if notification == BN_CLICKED {
                        if let Err(error) = app.on_button(id) {
                            app.alert(&error);
                        }
                    }
                    return 0;
                }
                WM_CLOSE => {
                    app.cancelled.store(true, Ordering::Relaxed);
                }
                _ => {}
            }
        }
    }
    DefWindowProcW(window, message, wparam, lparam)
}

/// 執行原生介面；smoke_check 只建立並驗證控制項，不顯示視窗、登入或寫入設定。
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
    // SAFETY: 視窗、字型、狀態全部由本函式擁有，維持至訊息迴圈結束後才釋放。
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_SYSTEM_AWARE);
        let scale = GetDpiForSystem() as f32 / 96.0;
        let instance = GetModuleHandleW(ptr::null());
        let class_name = wide("CompanyAIWindowV1");
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: class_name.as_ptr(),
            hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
            hbrBackground: (COLOR_BTNFACE + 1) as HBRUSH,
            ..std::mem::zeroed()
        };
        if RegisterClassW(&class) == 0 {
            return Err("無法註冊 Windows 視窗類別。".into());
        }
        let window = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            class_name.as_ptr(),
            wide("Company AI · 連線測試版").as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            (980.0 * scale) as i32,
            (800.0 * scale) as i32,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null(),
        );
        if window.is_null() {
            return Err("無法建立應用程式視窗。".into());
        }
        let font = CreateFontW(
            (-17.0 * scale) as i32,
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            0,
            0,
            CLEARTYPE_QUALITY as u32,
            0,
            wide("Microsoft JhengHei UI").as_ptr(),
        );
        if font.is_null() {
            DestroyWindow(window);
            return Err("無法建立介面字型。".into());
        }
        let (tx, rx) = mpsc::channel();
        let mut app = App {
            window,
            controls: HashMap::new(),
            font,
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
        };
        if let Err(error) = app.build_controls() {
            DestroyWindow(window);
            DeleteObject(font);
            return Err(error);
        }
        app.layout();
        let state = Box::new(RefCell::new(app));
        SetWindowLongPtrW(
            window,
            GWLP_USERDATA,
            (&*state as *const RefCell<App>) as isize,
        );
        if smoke_check {
            let app = state.borrow();
            if app.controls.len() != 26 || app.text(ROUTE) != "/v1/chat/completions" {
                return Err("介面控制項自我檢查失敗。".into());
            }
        } else {
            SetTimer(window, 1, 100, None);
            ShowWindow(window, SW_SHOW);
            UpdateWindow(window);
            if let Some(error) = startup_error {
                state.borrow().alert(&error);
            }
            let mut message = MSG::default();
            loop {
                let result = GetMessageW(&mut message, ptr::null_mut(), 0, 0);
                if result <= 0 {
                    break;
                }
                if IsDialogMessageW(window, &message) == 0 {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
        }
        state.borrow().cancelled.store(true, Ordering::Relaxed);
        SetWindowLongPtrW(window, GWLP_USERDATA, 0);
        if IsWindow(window) != 0 {
            DestroyWindow(window);
        }
        DeleteObject(font);
        UnregisterClassW(class_name.as_ptr(), instance);
    }
    Ok(())
}

pub fn show_fatal_error(error: &str) {
    // SAFETY: 應用啟動失敗時尚未建立視窗，使用無 parent 的 Windows 訊息框。
    unsafe {
        MessageBoxW(
            ptr::null_mut(),
            wide(error).as_ptr(),
            wide("Company AI 啟動失敗").as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}
