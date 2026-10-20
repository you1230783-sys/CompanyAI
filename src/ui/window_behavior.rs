//! 最小化與關閉共用的托盤行為；自檢只操作隔離程式自己的視窗。
use super::*;

impl App {
    /// 先由呼叫端保存草稿；提示不受一般網站通知設定影響，也不代表程式已退出。
    pub(super) fn hide_to_tray(&mut self) {
        unsafe { ShowWindow(self.window, SW_HIDE) };
        self.work.tray_restore_only = true;
        if !self.smoke {
            tray(
                self.window,
                NIM_MODIFY,
                Some("應用程式已縮小至系統托盤，如需離開，請在托盤圖示按右鍵 → 離開。"),
            );
        }
    }
}

/// 使用實際視窗訊息驗證兩種偏好與 X，避免只測到純函式分支。
/// smoke 不發送真實 balloon；Windows 勿擾模式仍可抑制正式提示。
pub(super) fn verify(state: &RefCell<App>) -> AppResult<()> {
    let window = state.borrow().window;
    let previous = state.borrow().config.minimize_to_taskbar;
    let result = (|| {
        for keep_taskbar in [true, false] {
            state.borrow_mut().config.minimize_to_taskbar = keep_taskbar;
            unsafe {
                ShowWindow(window, SW_SHOWNOACTIVATE);
                SendMessageW(window, WM_SYSCOMMAND, SC_MINIMIZE as usize, 0);
                if IsWindow(window) == 0
                    || IsIconic(window) == 0
                    || (IsWindowVisible(window) != 0) != keep_taskbar
                {
                    return Err(format!("原生最小化行為不符設定：{keep_taskbar}"));
                }
                ShowWindow(window, SW_SHOWNOACTIVATE);
                SendMessageW(window, WM_CLOSE, 0, 0);
                if IsWindow(window) == 0 || IsWindowVisible(window) != 0 {
                    return Err("按 X 應保留程式並收到托盤。".into());
                }
            }
            if !state.borrow().work.tray_restore_only {
                return Err("托盤操作提示不得導向舊任務。".into());
            }
        }
        Ok(())
    })();
    state.borrow_mut().config.minimize_to_taskbar = previous;
    unsafe { ShowWindow(window, SW_HIDE) };
    result
}
