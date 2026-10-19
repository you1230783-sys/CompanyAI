//! 原生選檔與兩階段匯入。JavaScript 只收到數量，不會取得 JSON 原文或密碼。
use super::*;
use crate::vnc::json_transfer::{export as export_json_file, Preview};
use windows::Win32::UI::Shell::FOS_PICKFOLDERS;

fn choose_json_location(window: HWND, folder: bool) -> AppResult<Option<PathBuf>> {
    // SAFETY: 與 Viewer 選擇器相同，由已初始化 COM STA 的視窗執行緒同步執行。
    unsafe {
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
            .map_err(|_| "無法建立 JSON 選擇器。")?;
        dialog
            .SetTitle(if folder {
                w!("選擇 VNC JSON 匯出資料夾")
            } else {
                w!("匯入 VNC 機台 JSON")
            })
            .map_err(|e| e.to_string())?;
        let options = FOS_PATHMUSTEXIST
            | FOS_FORCEFILESYSTEM
            | if folder {
                FOS_PICKFOLDERS
            } else {
                FOS_FILEMUSTEXIST
            };
        dialog.SetOptions(options).map_err(|e| e.to_string())?;
        if !folder {
            dialog
                .SetFileTypes(&[COMDLG_FILTERSPEC {
                    pszName: w!("JSON 機台清單"),
                    pszSpec: w!("*.json"),
                }])
                .map_err(|e| e.to_string())?;
        }
        if let Err(error) = dialog.Show(Some(WinHwnd(window))) {
            if error.code() == windows::core::HRESULT::from_win32(ERROR_CANCELLED.0) {
                return Ok(None);
            }
            return Err(format!("無法選取 JSON 位置：{error}"));
        }
        let name = dialog
            .GetResult()
            .and_then(|item| item.GetDisplayName(SIGDN_FILESYSPATH))
            .map_err(|e| e.to_string())?;
        let path = name.to_string();
        CoTaskMemFree(Some(name.0.cast()));
        Ok(Some(PathBuf::from(
            path.map_err(|_| "JSON 路徑無法辨識。")?,
        )))
    }
}

impl App {
    pub(super) fn vnc_json_command(&mut self, command: &VncCommand) -> AppResult<bool> {
        if !matches!(
            command,
            VncCommand::ChooseJsonImport { .. }
                | VncCommand::ApplyJsonImport { .. }
                | VncCommand::CancelJsonImport { .. }
                | VncCommand::ExportJson { .. }
        ) {
            return Ok(false);
        }
        if matches!(command, VncCommand::CancelJsonImport { .. }) {
            if let VncCommand::CancelJsonImport { preview_id } = command {
                if self
                    .vnc
                    .json_preview
                    .as_ref()
                    .is_some_and(|p| &p.id == preview_id)
                {
                    self.vnc.json_preview = None;
                }
            }
            return Ok(true);
        }
        // 網站同步預覽與本機檔案匯入分開；同步寫入中不接受第二個匯入。
        if self.vnc.busy || self.vnc.sync_id.is_some() {
            return Err("請先完成或捨棄網站更新，再匯入／匯出 JSON。".into());
        }
        match command {
            VncCommand::ChooseJsonImport { revision } => {
                self.vnc_manager(*revision)?.ensure_unchanged()?;
                if let Some(path) = choose_json_location(self.window, false)? {
                    self.vnc.json_preview = Some(Preview::load(&path, *revision)?);
                    self.vnc.status = "JSON 已驗證，請選擇加入或覆蓋；尚未修改本機清單。".into();
                }
            }
            VncCommand::ApplyJsonImport {
                revision,
                preview_id,
                mode,
            } => {
                let preview = self
                    .vnc
                    .json_preview
                    .as_ref()
                    .ok_or("匯入預覽已失效，請重新選檔。")?;
                if &preview.id != preview_id
                    || preview.revision != *revision
                    || self.vnc.revision != *revision
                {
                    return Err("預覽後機台清單已變更，請取消並重新匯入。".into());
                }
                let manager = self.vnc.manager.as_ref().ok_or("尚未載入機台設定。")?;
                let (machines, renamed) = preview.merged(&manager.machines, *mode)?;
                self.vnc_manager(*revision)?.save_machines(machines)?;
                self.vnc.revision += 1;
                self.vnc.json_preview = None;
                self.vnc.status = format!("JSON 匯入完成；同分類重名機台自動改名 {renamed} 台。Viewer 與連線選項維持原設定。");
                self.view.post(&json!({"type":"vnc_saved"}))?;
            }
            VncCommand::ExportJson { revision } => {
                self.vnc_manager(*revision)?.ensure_unchanged()?;
                if let Some(folder) = choose_json_location(self.window, true)? {
                    let path = export_json_file(self.vnc_manager(*revision)?, &folder)?;
                    self.vnc.status = format!("機台 JSON 已匯出至 {}。", path.display());
                }
            }
            _ => unreachable!(),
        }
        Ok(true)
    }
}
