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
    fn export_error_file(&self, include_details: bool) -> AppResult<bool> {
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
        let bytes = crate::errors::export(&self.root, include_details)?;
        storage::atomic_write(&path, &bytes)?;
        Ok(true)
    }
    pub(super) fn export_diagnostics(&self, include_details: bool) -> AppResult<()> {
        let text = match self.export_error_file(include_details) {
            Ok(true) => "錯誤診斷檔已儲存；可將此 JSON 交給維護者。".into(),
            Ok(false) => "已取消匯出。".into(),
            Err(error) => self.friendly_error("diagnostics_export", &error),
        };
        self.view
            .post(&json!({"type":"diagnostics_exported","text":text}))
    }
}
