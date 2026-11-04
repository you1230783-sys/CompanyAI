//! 原生儲存對話框決定輸出位置；網頁訊息不能指定任意磁碟路徑。
use super::*;

impl App {
    pub(super) fn friendly_error(&self, stage: &str, error: &str) -> String {
        let token = self
            .session
            .as_ref()
            .map(|s| s.access_token.as_str())
            .unwrap_or("");
        crate::errors::report(&self.root, stage, error, &[token])
    }
    fn export_error_file(&self, bytes: &[u8]) -> AppResult<bool> {
        use windows::{
            core::{w, HSTRING},
            Win32::{
                Foundation::{ERROR_CANCELLED, HWND as WinHwnd},
                System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER},
                UI::Shell::{
                    Common::COMDLG_FILTERSPEC, FileSaveDialog, IFileSaveDialog,
                    FOS_FORCEFILESYSTEM, FOS_OVERWRITEPROMPT, FOS_PATHMUSTEXIST, SIGDN_FILESYSPATH,
                },
            },
        };
        // UI 主執行緒已有 COM STA。取消是正常操作，不產生錯誤紀錄。
        let path = unsafe {
            let dialog: IFileSaveDialog =
                CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)
                    .map_err(|e| e.to_string())?;
            dialog
                .SetTitle(w!("匯出錯誤診斷檔"))
                .map_err(|e| e.to_string())?;
            dialog
                .SetOptions(FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST | FOS_OVERWRITEPROMPT)
                .map_err(|e| e.to_string())?;
            dialog
                .SetFileTypes(&[COMDLG_FILTERSPEC {
                    pszName: w!("診斷 JSON"),
                    pszSpec: w!("*.json"),
                }])
                .map_err(|e| e.to_string())?;
            dialog
                .SetDefaultExtension(w!("json"))
                .map_err(|e| e.to_string())?;
            dialog
                .SetFileName(&HSTRING::from(format!(
                    "LM_AI_Diagnostics_{}.json",
                    crate::unix_now()
                )))
                .map_err(|e| e.to_string())?;
            if let Err(error) = dialog.Show(Some(WinHwnd(self.window))) {
                if error.code() == windows::core::HRESULT::from_win32(ERROR_CANCELLED.0) {
                    return Ok(false);
                }
                return Err(error.to_string());
            }
            let value = dialog
                .GetResult()
                .and_then(|item| item.GetDisplayName(SIGDN_FILESYSPATH))
                .map_err(|e| e.to_string())?;
            let path = value.to_string();
            CoTaskMemFree(Some(value.0.cast()));
            PathBuf::from(path.map_err(|e| e.to_string())?)
        };
        // bytes來自已預覽的原生快照，不重新收集，也不接受前端傳回的檔案內容。
        storage::atomic_write(&path, bytes)?;
        Ok(true)
    }
    pub(super) fn export_diagnostics(&mut self, include_details: bool) -> AppResult<()> {
        self.diagnostic_preview = None;
        let bytes = crate::errors::export(&self.root, include_details)?;
        let preview_id = crate::jobs::new_id()?;
        let text = std::str::from_utf8(&bytes).map_err(|_| "診斷文字編碼錯誤。")?;
        let help: Vec<_> = [
            "01", "02", "03", "04", "05", "06", "07", "08", "09", "10", "11", "12", "13", "99",
        ]
        .into_iter()
        .map(|code| {
            let (message, suggestion) = crate::errors::description(code);
            json!({"code":code,"message":message,"suggestion":suggestion})
        })
        .collect();
        self.view.post(&json!({"type":"diagnostics_preview","preview_id":preview_id,"text":text,"include_details":include_details,"help":help}))?;
        self.diagnostic_preview = Some((preview_id, bytes));
        Ok(())
    }

    pub(super) fn save_diagnostics(&mut self, preview_id: &str) -> AppResult<()> {
        let (id, bytes) = self
            .diagnostic_preview
            .as_ref()
            .ok_or("請先預覽診斷內容。")?;
        if id != preview_id {
            return Err("診斷預覽已過期，請重新預覽。".into());
        }
        let text = match self.export_error_file(bytes) {
            Ok(true) => "錯誤診斷檔已儲存；可將此 JSON 交給維護者。".into(),
            Ok(false) => "已取消匯出。".into(),
            Err(error) => self.friendly_error("diagnostics_export", &error),
        };
        self.view
            .post(&json!({"type":"diagnostics_exported","text":text}))
    }

    /// 非預期panic後隔離所有背景結果；不將部分完成的本機操作重跑。
    /// 介面保留診斷、複製與離開；重新啟動後由原請求／成果核對機制處理未知結果。
    pub(super) fn fault_tick(&mut self) {
        if !self.panic_fault {
            self.panic_fault = true;
            self.cancelled.store(true, Ordering::Relaxed);
            self.socket_cancel.store(true, Ordering::Relaxed);
            self.mail_flow.cancel.store(true, Ordering::Relaxed);
            self.vnc.cancel_sync();
            for cancel in self.work.cancellations.values() {
                cancel.store(true, Ordering::Relaxed);
            }
            self.projects.quarantine();
            self.work.incoming = None;
            self.work.polling = false;
            self.work.status = "工作狀態待確認，重新啟動後查回原請求。".into();
            self.mail_flow.phase = "idle";
            self.busy = "none";
            self.mail_busy = false;
            self.update_busy = false;
            self.services_loading = false;
            self.notifications_loading = false;
            self.site.loading = false;
            self.site.mutating = false;
            self.update_consent = None;
            self.status_notice_id = self.status_notice_id.wrapping_add(1);
            self.error = true;
            self.status = "Error Code: 13\n發生非預期錯誤，已停止接受新工作。請先預覽／匯出診斷，再離開並重新啟動；已寫入的成果需核對，不要直接重做。".into();
            if crate::reliability::log_failed() {
                self.status
                    .push_str("\n異常紀錄未能完整保存，請保留此畫面。");
            }
            self.publish();
        }
        // 延遲事件不得恢復busy或覆寫成果狀態；消耗通道以釋放待回覆的資源。
        while self.rx.try_recv().is_ok() {}
        while let Ok(message) = self.commands.try_recv() {
            if let Ok(command) = serde_json::from_str::<Command>(&message) {
                if let Err(error) = self.command(command) {
                    let _ = self.view.post(&json!({"type":"toast","text":error}));
                }
            }
        }
    }
}
