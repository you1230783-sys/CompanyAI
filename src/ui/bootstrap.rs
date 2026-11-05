//! 原生視窗、COM生命週期與啟動入口；保留既有回呼邊界。
use super::*;

pub(super) fn open_browser(value: &str) -> AppResult<()> {
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
        // ShellExecuteW 直接以回傳值表示失敗；不可把 GetLastError 當成其錯誤碼。
        Err(format!(
            "無法開啟預設瀏覽器（ShellExecuteW：{}）。請確認 Windows 的預設瀏覽器設定。",
            result as isize
        ))
    } else {
        Ok(())
    }
}
pub(super) fn show_window(window: HWND) -> bool {
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
/// 從 EXE 的多尺寸圖示選取大／小圖案，避免托盤直接縮小視窗的大圖示。
/// LR_SHARED 讓 Windows 管理圖示生命週期，重複更新托盤時不需自行釋放。
pub(super) fn app_icon(small: bool) -> HICON {
    unsafe {
        let (width, height) = if small {
            (GetSystemMetrics(SM_CXSMICON), GetSystemMetrics(SM_CYSMICON))
        } else {
            (GetSystemMetrics(SM_CXICON), GetSystemMetrics(SM_CYICON))
        };
        // Win32 MAKEINTRESOURCEW(1)：低位數值代表資源 ID，API 不會將它解參考。
        let resource_id = ptr::without_provenance::<u16>(1);
        let icon = LoadImageW(
            GetModuleHandleW(ptr::null()),
            resource_id,
            IMAGE_ICON,
            width,
            height,
            LR_SHARED,
        ) as HICON;
        if icon.is_null() {
            // 資源載入失敗時仍保留可辨識的系統圖示，讓托盤還原操作繼續可用。
            LoadIconW(ptr::null_mut(), IDI_APPLICATION)
        } else {
            icon
        }
    }
}
pub(super) fn taskbar_created_message() -> u32 {
    static MESSAGE: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *MESSAGE.get_or_init(|| unsafe { RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()) })
}
/// 原生通知不搶焦點，點擊後才顯示主視窗；Windows 可自行停用彈出提示。
pub(super) fn tray(window: HWND, operation: u32) {
    let mut data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: window,
        uID: 1,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
        uCallbackMessage: TRAY_MESSAGE,
        ..Default::default()
    };
    data.hIcon = app_icon(true);
    for (dest, source) in data.szTip.iter_mut().zip(wide("LM_AI")) {
        *dest = source;
    }
    unsafe {
        Shell_NotifyIconW(operation, &data);
    }
}
pub(super) unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    crate::reliability::win32(if message == WM_CREATE { -1 } else { 0 }, || {
        // 僅獨立驗證入口建立的smoke視窗允許故障注入，正式UI沒有對應命令。
        if message == WM_APP + 999 {
            let pointer = GetWindowLongPtrW(window, GWLP_USERDATA) as *const RefCell<App>;
            if !pointer.is_null()
                && (&*pointer)
                    .try_borrow()
                    .is_ok_and(|app| app.smoke && app.panic_probe)
            {
                panic!("fixture-private-user-path-must-not-be-recorded");
            }
        }
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
                if message == taskbar_created_message() && !app.smoke {
                    // Explorer 重啟會清除通知區圖案；隱藏中的 App 必須重新加入，才能讓使用者還原。
                    tray(window, NIM_ADD);
                    return 0;
                }
                match message {
                    crate::single_instance::RESTORE_MESSAGE => {
                        app.restore_window();
                        return 0;
                    }
                    #[cfg(feature = "pet")]
                    crate::pet::MESSAGE => {
                        match wparam {
                            1 => {
                                let target = app.pet.settings().open_action;
                                app.pet_navigate(target);
                            }
                            6 => app.restore_window(),
                            7 => {
                                if let Some(target) = app.pet.take_navigation() {
                                    app.pet_navigate(target);
                                }
                            }
                            2 => {
                                PostMessageW(
                                    window,
                                    TRAY_MESSAGE,
                                    0,
                                    NIN_BALLOONUSERCLICK as isize,
                                );
                            }
                            3 | 4 => {
                                show_window(window);
                                let _=app.view.post(&json!({"type":if wparam==3{"show_tarot"}else{"show_pet_settings"}}));
                            }
                            5 => {
                                let mut settings = app.pet.settings();
                                settings.enabled = false;
                                if let Err(error) = app.pet.configure(settings) {
                                    app.fail(error);
                                }
                            }
                            _ => (),
                        }
                        return 0;
                    }
                    crate::selection_popup::CLICK_MESSAGE => {
                        let source = app.selection_popup.source;
                        app.selection_popup.update(None);
                        if source != 0
                            && app.logged_in()
                            && app.busy == "none"
                            && !app.versions.blocked()
                        {
                            let tx = app.tx.clone();
                            let generation = app.generation;
                            app.busy = "capture";
                            crate::reliability::spawn(move || {
                                let hotkey = Hotkey {
                                    modifiers: 0,
                                    key: 0,
                                };
                                let _ = tx.send(Event::Capture(
                                    generation,
                                    selection::capture(source as HWND, hotkey),
                                ));
                            });
                        }
                        return 0;
                    }
                    WM_CLOSE => {
                        if crate::reliability::faulted() {
                            // 狀態可能部分修改，不再保存／覆寫歷史；明確離開以便重新啟動。
                            DestroyWindow(window);
                            return 0;
                        }
                        if let Err(e) = app.preserve_draft() {
                            app.fail(e);
                            app.publish();
                            return 0;
                        }
                        if wparam == 0 {
                            app.hide_to_tray();
                            return 0;
                        }
                        unsafe {
                            DestroyWindow(window);
                        }
                        return 0;
                    }
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
                    WM_SIZE if wparam == SIZE_MINIMIZED as usize => {
                        if let Err(e) = app.preserve_draft() {
                            app.fail(e);
                        }

                        // Windows 已完成最小化；保留工作列時不要再呼叫 SW_HIDE。
                        if !app.config.minimize_to_taskbar {
                            app.hide_to_tray();
                        }
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
                    WM_HOTKEY if wparam == crate::screenshot::HOTKEY_ID as usize => {
                        if let Err(error) =
                            app.screenshot_command(screenshot::ScreenshotCommand::Prepare)
                        {
                            app.fail(error);
                            app.publish();
                        }
                        return 0;
                    }
                    WM_HOTKEY if wparam == selection::HOTKEY_ID as usize => {
                        app.capture();
                        return 0;
                    }
                    TRAY_MESSAGE
                        if matches!(
                            lparam as u32,
                            WM_LBUTTONUP | WM_LBUTTONDBLCLK | NIN_BALLOONUSERCLICK
                        ) =>
                    {
                        if lparam as u32 == NIN_BALLOONUSERCLICK {
                            show_window(window);
                        } else {
                            app.restore_window();
                        }
                        if lparam as u32 == NIN_BALLOONUSERCLICK
                            && !app.open_schedule_notice()
                            && !app.work.tray_restore_only
                            && !app.open_project_attention().unwrap_or(false)
                        {
                            let _ = app.view.post(&json!({"type":if app.work.task_balloon {"show_tasks"} else {"show_notifications"}}));
                        }
                        return 0;
                    }
                    TRAY_MESSAGE if matches!(lparam as u32, WM_RBUTTONUP | WM_CONTEXTMENU) => {
                        unsafe {
                            let menu = CreatePopupMenu();
                            if !menu.is_null() {
                                AppendMenuW(menu, MF_STRING, 1, wide("還原視窗").as_ptr());
                                AppendMenuW(menu, MF_SEPARATOR, 0, ptr::null());
                                AppendMenuW(menu, MF_STRING, 2, wide("離開 LM_AI").as_ptr());
                                let mut point = POINT::default();
                                GetCursorPos(&mut point);
                                SetForegroundWindow(window);
                                let action = TrackPopupMenu(
                                    menu,
                                    TPM_RETURNCMD | TPM_RIGHTBUTTON,
                                    point.x,
                                    point.y,
                                    0,
                                    window,
                                    ptr::null(),
                                );
                                DestroyMenu(menu);
                                PostMessageW(window, WM_NULL, 0, 0);
                                match action {
                                    1 => {
                                        app.restore_window();
                                    }
                                    2 => {
                                        PostMessageW(window, WM_CLOSE, 1, 0);
                                    }
                                    _ => {}
                                }
                            }
                        }
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
    })
}
/// 自我檢查使用隔離資料夾，實際載入 WebView2 與 Markdown DOM，不連公司服務。
pub fn run(demo: Option<&DemoServer>, smoke: bool) -> AppResult<()> {
    run_with_local(demo, smoke, None, None, None)
}

/// 獨立程序驗證背景／WndProc／WebView2 panic；只使用指定的全新測試目錄。
pub fn verify_panic_recovery(mode: &str, root: &Path) -> AppResult<()> {
    if !matches!(mode, "background" | "callback" | "com") {
        return Err("未知異常測試模式。".into());
    }
    run_with_local(None, true, None, None, Some((mode, root)))
}

/// 手動體驗互動教學，不啟動正式 App、登入流程、模型或外部工具。
pub fn run_tutorial_preview() -> AppResult<()> {
    tutorial_preview::run()
}

/// 本機模型測試保留正式畫面與工具流程，所有個人資料寫入獨立目錄。
pub fn run_local(local: crate::local_test::LocalTest) -> AppResult<()> {
    run_with_local(None, false, Some(local), None, None)
}

/// 本機模型情境沿用正式控制器與離線呈現器；不使用一般使用者資料目錄。
pub fn run_local_scenario(mut local: crate::local_test::LocalTest, path: &Path) -> AppResult<()> {
    local.config.notification_popups = false;
    run_with_local(None, false, Some(local), Some(path), None)
}

pub(super) fn run_with_local(
    demo: Option<&DemoServer>,
    smoke: bool,
    local: Option<crate::local_test::LocalTest>,
    scenario_path: Option<&Path>,
    panic_probe: Option<(&str, &Path)>,
) -> AppResult<()> {
    let mut scenario = match (scenario_path, local.as_ref()) {
        (Some(path), Some(local)) => {
            Some(projects::local_scenario::Scenario::load(path, &local.root)?)
        }
        (None, _) => None,
        _ => return Err("情境只允許明確的本機測試連線。".into()),
    };
    let _instance = if !smoke && demo.is_none() && local.is_none() {
        match crate::single_instance::acquire()? {
            Some(instance) => Some(instance),
            None => return Ok(()),
        }
    } else {
        None
    };
    let _apartment = crate::com_apartment::Apartment::sta().map_err(|_| "無法初始化桌面介面。")?;
    let root = if let Some((_, root)) = panic_probe {
        root.to_path_buf()
    } else if let Some(local) = &local {
        local.root.clone()
    } else if smoke {
        std::env::temp_dir().join("CompanyAI-ui-smoke")
    } else {
        let root = storage::data_dir()?;
        if demo.is_some() {
            root.join("demo")
        } else {
            root
        }
    };
    let config = if let Some(local) = &local {
        local.config.clone()
    } else if let Some(demo) = demo {
        demo.config()
    } else if smoke {
        Config::default()
    } else {
        storage::load_config(&root)?
    };
    let session = if let Some(local) = &local {
        Some(local.session.clone())
    } else if demo.is_none() && !smoke {
        storage::load_session(&root, &config)?
    } else {
        None
    };
    let (archive, history_error) = match history::load(&root) {
        Ok(a) => (a, None),
        Err(e) => (Archive::default(), Some(e)),
    };
    let versions = VersionState::load(&root)?;
    let (tx, rx) = mpsc::channel();
    let (command_tx, commands) = mpsc::channel();
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance = GetModuleHandleW(ptr::null());
        let name = wide(if demo.is_some() || smoke || local.is_some() {
            "LM_AI_TestWindow"
        } else {
            "LM_AI_Window"
        });
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: name.as_ptr(),
            hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
            hIcon: app_icon(false),
            hIconSm: app_icon(true),
            ..Default::default()
        };
        if RegisterClassExW(&class) == 0 {
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
        apply_window_theme(window, config.dark_mode);
        let view = WebView::create(window, &root.join("webview"), command_tx)?;
        let mut rect = RECT::default();
        GetClientRect(window, &mut rect);
        view.resize(rect.right, rect.bottom);
        let selection_tx = tx.clone();
        let selection_popup = crate::selection_popup::Popup::new(window, move |rect| {
            let _ = selection_tx.send(Event::SelectionRect(rect));
        })?;
        selection_popup.enabled.store(
            config.selection_icon
                && !smoke
                && session.as_ref().is_some_and(|s| s.valid_for(&config)),
            Ordering::Relaxed,
        );
        #[cfg(feature = "pet")]
        let pet = crate::pet::Pet::new(window, &root, smoke || local.is_some())?;
        let mut app = App {
            panic_probe: panic_probe.is_some(),
            panic_fault: false,
            diagnostic_preview: None,
            reminders: reminders::ReminderRuntime::load(&root),
            schedules: schedules::Runtime::load(&root),
            templates: templates::TemplateRuntime::load(&root),
            screenshot: screenshot::Runtime::default(),
            handoff: None,
            workspace_import: None,
            document_export: Default::default(),
            #[cfg(feature = "pet")]
            pet,
            #[cfg(feature = "tarot")]
            tarot: crate::tarot::Round::default(),
            onboarding: onboarding::Onboarding::load(smoke || local.is_some()),
            projects: projects::ProjectRuntime::load(&root),
            #[cfg(feature = "vnc")]
            vnc: vnc::VncRuntime::default(),
            site: site::SiteRuntime::default(),
            mail_flow: mail_batch::MailRuntime::default(),
            work: work::WorkRuntime::default(),
            window,
            selection_popup,
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
            status_notice_id: 0,
            error: false,
            models: None,
            versions,
            version_status: "正在檢查版本".into(),
            update_ready: None,
            update_busy: false,
            update_consent: None,
            update_status: String::new(),
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
            demo: demo.is_some() || local.is_some(),
            smoke,
            ready: false,
            smoke_result: None,
        };
        if !smoke {
            if let Err(error) = app.recover_project_history() {
                app.fail(error);
            }
        }
        if !smoke && !app.config.always_new_chat {
            if let Some(c) = app
                .archive
                .conversations
                .iter()
                .max_by_key(|c| c.updated_at)
            {
                app.active_id = Some(c.id.clone());
                app.messages = c.messages.clone();
                let draft = c.draft.clone();
                app.set_draft(draft);
            }
        }
        if !smoke {
            if let Err(e) = app.apply_hotkey(app.config.hotkey.clone()) {
                app.fail(e);
            }
            app.refresh_services();
            app.start_notifications();
            tray(window, NIM_ADD);
        }
        let state = Box::new(RefCell::new(app));
        SetWindowLongPtrW(
            window,
            GWLP_USERDATA,
            (&*state as *const RefCell<App>) as isize,
        );
        SetTimer(window, 1, 50, None);
        if !smoke && scenario.is_none() {
            ShowWindow(window, SW_SHOW);
        }
        let started = Instant::now();
        let mut panic_injected = false;
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
            if let Some(scenario) = scenario.as_mut() {
                if let Some(done) = scenario.tick(&mut state.borrow_mut()) {
                    result = done;
                    break;
                }
                if started.elapsed() > Duration::from_secs(3 * 60 * 60) {
                    result = Err("本機情境逾時，保留未完成資料。".into());
                    break;
                }
            }
            if smoke {
                if let Some((mode, _)) = panic_probe {
                    if !panic_injected && state.borrow().ready {
                        panic_injected = true;
                        state.borrow_mut().busy = "capture";
                        match mode {
                            "background" => {
                                crate::reliability::spawn(|| {
                                    panic!("fixture-private-user-path-must-not-be-recorded")
                                });
                            }
                            "callback" => {
                                PostMessageW(window, WM_APP + 999, 0, 0);
                            }
                            "com" => state.borrow().view.probe_panic()?,
                            _ => unreachable!(),
                        }
                    }
                    if state.borrow().panic_fault {
                        let mut app = state.borrow_mut();
                        if app.busy != "none" || app.can_send() || app.projects.is_running() {
                            result = Err("異常後未正確解除busy或撤銷工作能力。".into());
                        } else if app.command(Command::NewChat).is_ok() {
                            result = Err("異常後仍允許新工作。".into());
                        } else {
                            // 在正式控制器證實診斷仍可預覽；晚到服務事件不得清除保護狀態。
                            app.tx
                                .send(Event::Services(
                                    app.generation,
                                    Err("late".into()),
                                    Err("late".into()),
                                ))
                                .map_err(|_| "測試通道關閉。")?;
                            app.fault_tick();
                            app.export_diagnostics(false)?;
                            if app.diagnostic_preview.is_none()
                                || !app.status.starts_with("Error Code: 13")
                            {
                                result = Err("異常後診斷入口或晚到事件隔離失敗。".into());
                            }
                        }
                        break;
                    }
                }
                let done = state.borrow_mut().smoke_result.take();
                if let Some(done) = done {
                    // 放開 RefCell 借用後，真正送出原生最小化／關閉訊息。
                    result = done.and_then(|()| window_behavior::verify(&state));
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
/// Windows 11 標題列同步使用灰藍色；切回淺色時還原系統預設。
pub(super) fn apply_window_theme(window: HWND, dark: bool) {
    use windows_sys::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR,
    };
    // COLORREF 是 0x00BBGGRR，而不是 CSS 的 RGB 順序。
    let background: u32 = if dark { 0x0030251d } else { 0xffffffff };
    let foreground: u32 = if dark { 0x00efe9e3 } else { 0xffffffff };
    unsafe {
        // 外觀設定失敗不應阻止使用者聊天；WebView2 仍會套用自己的完整色票。
        DwmSetWindowAttribute(
            window,
            DWMWA_CAPTION_COLOR as u32,
            (&background as *const u32).cast(),
            4,
        );
        DwmSetWindowAttribute(
            window,
            DWMWA_TEXT_COLOR as u32,
            (&foreground as *const u32).cast(),
            4,
        );
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
