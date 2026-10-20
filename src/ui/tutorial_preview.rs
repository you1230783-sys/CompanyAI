//! 使用正式內嵌前端的獨立教學視窗。沒有 App 控制器，不能執行專案命令；
//! WebView2 profile 放在獨立暫存目錄，不載入帳號、正式對話與使用者設定。
use crate::{webview::WebView, wide, AppResult};
use std::{cell::RefCell, ptr};
use windows_sys::Win32::{
    Foundation::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{HiDpi::*, WindowsAndMessaging::*},
};

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let view = GetWindowLongPtrW(window, GWLP_USERDATA) as *const RefCell<WebView>;
    match message {
        WM_SIZE if !view.is_null() => {
            let mut bounds = RECT::default();
            GetClientRect(window, &mut bounds);
            (*view).borrow().resize(bounds.right, bounds.bottom);
            0
        }
        WM_CLOSE => {
            // 教學預覽關閉即離開，不進正式程式的托盤或背景工作。
            DestroyWindow(window);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(window, message, wparam, lparam),
    }
}

pub(super) fn run() -> AppResult<()> {
    unsafe {
        windows::Win32::System::Com::CoInitializeEx(
            None,
            windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
        )
        .ok()
    }
    .map_err(|_| "無法初始化教學預覽。")?;
    let _apartment = super::ComApartment;
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance = GetModuleHandleW(ptr::null());
        let name = wide("LM_AI_ProjectTutorialPreview");
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: name.as_ptr(),
            hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
            hIcon: super::app_icon(false),
            hIconSm: super::app_icon(true),
            ..Default::default()
        };
        if RegisterClassExW(&class) == 0 {
            return Err("無法註冊教學預覽視窗。".into());
        }
        let window = CreateWindowExW(
            0,
            name.as_ptr(),
            wide("LM_AI — 全功能互動教學預覽（假資料／不呼叫 AI）").as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1240,
            860,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null(),
        );
        if window.is_null() {
            UnregisterClassW(name.as_ptr(), instance);
            return Err("無法建立教學預覽視窗。".into());
        }
        let profile = std::env::temp_dir().join("CompanyAI-project-tutorial-preview");
        let view = match WebView::create_tutorial_preview(window, &profile) {
            Ok(view) => Box::new(RefCell::new(view)),
            Err(error) => {
                DestroyWindow(window);
                UnregisterClassW(name.as_ptr(), instance);
                return Err(error);
            }
        };
        SetWindowLongPtrW(
            window,
            GWLP_USERDATA,
            (&*view as *const RefCell<WebView>) as isize,
        );
        let mut bounds = RECT::default();
        GetClientRect(window, &mut bounds);
        view.borrow().resize(bounds.right, bounds.bottom);
        ShowWindow(window, SW_SHOW);
        let mut message = MSG::default();
        while GetMessageW(&mut message, ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        // WM_DESTROY 前不釋放 view，避免 WM_SIZE 持有失效指標。
        drop(view);
        UnregisterClassW(name.as_ptr(), instance);
        Ok(())
    }
}
