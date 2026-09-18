//! 同一登入工作階段只啟動一個正式實例；第二次啟動只通知既有視窗。
use crate::{wide, AppResult};
use std::{ptr, thread, time::Duration};
use windows_sys::Win32::{Foundation::*, System::Threading::*, UI::WindowsAndMessaging::*};

pub const RESTORE_MESSAGE: u32 = WM_APP + 20;
pub struct Instance(HANDLE);
impl Drop for Instance {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
/// Mutex 的存活期涵蓋整個主視窗生命週期，避免兩次啟動同時通過程序名稱檢查。
pub fn acquire() -> AppResult<Option<Instance>> {
    let handle =
        unsafe { CreateMutexW(ptr::null(), 0, wide("Local\\LARGAN.LM_AI.Desktop").as_ptr()) };
    if handle.is_null() {
        return Err("無法建立單一實例鎖。".into());
    }
    let existing = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    let instance = Instance(handle);
    if !existing {
        return Ok(Some(instance));
    }
    // 首次實例可能尚在建立 WebView2；等待視窗就緒，避免連點時漏掉叫出通知。
    for _ in 0..300 {
        let window = unsafe { FindWindowW(wide("LM_AI_Window").as_ptr(), ptr::null()) };
        if !window.is_null() {
            unsafe {
                let mut process = 0;
                GetWindowThreadProcessId(window, &mut process);
                AllowSetForegroundWindow(process);
                if PostMessageW(window, RESTORE_MESSAGE, 0, 0) != 0 {
                    return Ok(None);
                }
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err("LM_AI 已在啟動或結束中，請稍後從系統托盤開啟。".into())
}
