//! 可選的 UI Automation 選取範圍提示。只讀範圍位置，不在背景取得文字。
//! 不支援 TextPattern 的程式安靜略過；點擊浮動視窗後才沿用使用者觸發的複製流程。
use crate::{wide, AppResult};
use std::{
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};
use windows::Win32::{
    System::{Com::*, Ole::*},
    UI::Accessibility::*,
};
use windows_sys::Win32::{
    Foundation::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

pub const CLICK_MESSAGE: u32 = WM_APP + 21;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectionRect {
    pub source: usize,
    pub x: i32,
    pub y: i32,
}
pub struct Popup {
    window: HWND,
    pub enabled: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    pub source: usize,
}
impl Drop for Popup {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        unsafe {
            DestroyWindow(self.window);
        }
    }
}
unsafe extern "system" fn popup_proc(window: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match message {
        WM_MOUSEACTIVATE => MA_NOACTIVATE as isize,
        WM_LBUTTONUP => {
            unsafe {
                PostMessageW(GetWindow(window, GW_OWNER), CLICK_MESSAGE, 0, 0);
            }
            0
        }
        WM_PAINT => unsafe {
            use windows_sys::Win32::Graphics::Gdi::*;
            let mut paint = PAINTSTRUCT::default();
            let dc = BeginPaint(window, &mut paint);
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: 36,
                bottom: 30,
            };
            SetBkMode(dc, TRANSPARENT as i32);
            DrawTextW(
                dc,
                wide("AI").as_ptr(),
                2,
                &mut rect,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
            EndPaint(window, &paint);
            0
        },
        _ => unsafe { DefWindowProcW(window, message, w, l) },
    }
}
impl Popup {
    pub(crate) fn new(
        owner: HWND,
        send: impl Fn(Option<SelectionRect>) + Send + 'static,
    ) -> AppResult<Self> {
        let name = wide("LM_AI_SelectionPopup");
        let window = unsafe {
            let instance = GetModuleHandleW(ptr::null());
            let class = WNDCLASSW {
                lpfnWndProc: Some(popup_proc),
                hInstance: instance,
                lpszClassName: name.as_ptr(),
                hCursor: LoadCursorW(ptr::null_mut(), IDC_HAND),
                hbrBackground: (windows_sys::Win32::Graphics::Gdi::COLOR_WINDOW + 1) as _,
                ..Default::default()
            };
            RegisterClassW(&class);
            CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                name.as_ptr(),
                wide("AI 選字帶入").as_ptr(),
                WS_POPUP | WS_BORDER,
                0,
                0,
                36,
                30,
                owner,
                ptr::null_mut(),
                instance,
                ptr::null(),
            )
        };
        if window.is_null() {
            return Err("無法建立選字圖示。".into());
        }
        let enabled = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let (on, end, own) = (enabled.clone(), stop.clone(), owner as usize);
        thread::spawn(move || unsafe {
            if CoInitializeEx(None, COINIT_MULTITHREADED).is_err() {
                return;
            }
            let automation: windows::core::Result<IUIAutomation> =
                CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER);
            if let Ok(automation) = automation {
                let mut previous = None;
                let mut dismissed = None;
                while !end.load(Ordering::Relaxed) {
                    let mut selection =
                        if on.load(Ordering::Relaxed) && GetAsyncKeyState(VK_ESCAPE as i32) >= 0 {
                            if GetAsyncKeyState(VK_LBUTTON as i32) < 0 {
                                previous
                            } else {
                                selection_rect(&automation, own).ok()
                            }
                        } else {
                            None
                        };
                    if previous.is_some()
                        && on.load(Ordering::Relaxed)
                        && GetAsyncKeyState(VK_ESCAPE as i32) < 0
                    {
                        dismissed = previous;
                    }
                    if selection.is_some() && selection == dismissed {
                        selection = None;
                    } else if selection.is_some() {
                        dismissed = None;
                    }
                    if selection != previous {
                        send(selection);
                        previous = selection;
                    }
                    thread::sleep(Duration::from_millis(350));
                }
            }
            CoUninitialize();
        });
        Ok(Self {
            window,
            enabled,
            stop,
            source: 0,
        })
    }
    pub fn update(&mut self, selection: Option<SelectionRect>) {
        unsafe {
            if let Some(rect) = selection.filter(|_| self.enabled.load(Ordering::Relaxed)) {
                if GetForegroundWindow() as usize == rect.source {
                    self.source = rect.source;
                    // 以來源螢幕的工作區限制位置，避免多螢幕／負座標時落在畫面外。
                    use windows_sys::Win32::Graphics::Gdi::*;
                    let monitor = MonitorFromPoint(
                        POINT {
                            x: rect.x,
                            y: rect.y,
                        },
                        MONITOR_DEFAULTTONEAREST,
                    );
                    let mut info = MONITORINFO {
                        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                        ..Default::default()
                    };
                    GetMonitorInfoW(monitor, &mut info);
                    let x = rect.x.clamp(
                        info.rcWork.left,
                        (info.rcWork.right - 36).max(info.rcWork.left),
                    );
                    let y = rect.y.clamp(
                        info.rcWork.top,
                        (info.rcWork.bottom - 30).max(info.rcWork.top),
                    );
                    SetWindowPos(
                        self.window,
                        HWND_TOPMOST,
                        x,
                        y,
                        36,
                        30,
                        SWP_NOACTIVATE | SWP_SHOWWINDOW,
                    );
                    return;
                }
            }
            self.source = 0;
            ShowWindow(self.window, SW_HIDE);
        }
    }
}

/// 取得選取範圍最後一行的矩形；不呼叫 GetText，不讀密碼控制項。
unsafe fn selection_rect(
    automation: &IUIAutomation,
    owner: usize,
) -> windows::core::Result<SelectionRect> {
    unsafe {
        let source = GetForegroundWindow();
        if source.is_null() || source as usize == owner {
            return Err(windows::core::Error::from_hresult(windows::core::HRESULT(
                0x80004005u32 as i32,
            )));
        }
        let element = automation.GetFocusedElement()?;
        if element.CurrentIsPassword()?.as_bool() {
            return Err(windows::core::Error::from_hresult(windows::core::HRESULT(
                0x80004005u32 as i32,
            )));
        }
        let pattern: IUIAutomationTextPattern = element.GetCurrentPatternAs(UIA_TextPatternId)?;
        let ranges = pattern.GetSelection()?;
        if ranges.Length()? < 1 {
            return Err(windows::core::Error::from_hresult(windows::core::HRESULT(
                0x80004005u32 as i32,
            )));
        }
        let range = ranges.GetElement(0)?;
        if range.CompareEndpoints(
            TextPatternRangeEndpoint_Start,
            &range,
            TextPatternRangeEndpoint_End,
        )? == 0
        {
            return Err(windows::core::Error::from_hresult(windows::core::HRESULT(
                0x80004005u32 as i32,
            )));
        }
        let array = range.GetBoundingRectangles()?;
        struct ArrayGuard(*mut SAFEARRAY);
        impl Drop for ArrayGuard {
            fn drop(&mut self) {
                unsafe {
                    let _ = SafeArrayDestroy(self.0);
                }
            }
        }
        let guard = ArrayGuard(array);
        let lower = SafeArrayGetLBound(guard.0, 1)?;
        let upper = SafeArrayGetUBound(guard.0, 1)?;
        if upper - lower + 1 < 4 {
            return Err(windows::core::Error::from_hresult(windows::core::HRESULT(
                0x80004005u32 as i32,
            )));
        }
        let mut rect = [0f64; 4];
        for (n, value) in rect.iter_mut().enumerate() {
            let index = upper - 3 + n as i32;
            SafeArrayGetElement(guard.0, &index, (value as *mut f64).cast())?;
        }
        if rect.iter().any(|v| !v.is_finite())
            || rect[2] <= 0.0
            || rect[3] <= 0.0
            || GetForegroundWindow() != source
        {
            return Err(windows::core::Error::from_hresult(windows::core::HRESULT(
                0x80004005u32 as i32,
            )));
        }
        Ok(SelectionRect {
            source: source as usize,
            x: (rect[0] + rect[2] + 6.0) as i32,
            y: (rect[1] + rect[3] + 4.0) as i32,
        })
    }
}
