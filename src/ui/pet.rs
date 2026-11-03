//! 桌寵偏好與圖片匯入只由使用者介面發起；模型沒有此命令。
use super::*;
#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub(super) enum PetCommand {
    State,
    Configure {
        enabled: bool,
        skin: String,
        size: u16,
        reduced_motion: bool,
    },
    Import {
        state: String,
    },
    Preview,
}
impl App {
    pub(super) fn desktop_notice(&mut self, message: &str) {
        if !self.smoke {
            self.pet.notify(message);
        }
    }
    pub(super) fn pet_command(&mut self, command: PetCommand) -> AppResult<()> {
        let result = (|| {
            match command {
                PetCommand::State => (),
                PetCommand::Configure {
                    enabled,
                    skin,
                    size,
                    reduced_motion,
                } => {
                    let mut settings = self.pet.settings();
                    settings.enabled = enabled;
                    settings.skin = skin;
                    settings.size = size;
                    settings.reduced_motion = reduced_motion;
                    self.pet.configure(settings)?;
                }
                PetCommand::Import { state } => {
                    if !crate::pet::ACTIONS.contains(&state.as_str()) {
                        return Err("未知的桌寵動作。".into());
                    }
                    if let Some(path) = choose_png(self.window)? {
                        let bytes = self.pet.import(&state, &path)?;
                        self.view.post(&json!({"type":"pet_imported","message":format!("已保留透明度並壓縮為 {:.1} KB，原圖未變更。",bytes as f64/1000.0)}))?;
                    }
                }
                PetCommand::Preview => {
                    self.work.tray_restore_only = true;
                    self.desktop_notice("這是桌寵通知。點一下可回到主視窗；拖曳角色可移動位置。");
                }
            }
            self.view
                .post(&json!({"type":"pet_state","state":self.pet.state()?}))
        })();
        if let Err(error) = &result {
            let _ = self.view.post(&json!({"type":"pet_error","message":error}));
        }
        result
    }
    pub(super) fn tick_pet(&mut self) {
        let active = self.work.store.tasks.iter().any(|t| t.active());
        let waiting = self
            .work
            .store
            .tasks
            .iter()
            .any(|t| t.active() && t.project_retry.due_at_millis > 0);
        let phase = if waiting {
            "waiting"
        } else if self.projects.is_running() || active {
            "working"
        } else if self.busy != "none" {
            "thinking"
        } else {
            "idle"
        };
        let _ = self.pet.tick(phase);
    }
}
fn choose_png(owner: HWND) -> AppResult<Option<PathBuf>> {
    use windows::{
        core::w,
        Win32::{
            Foundation::{ERROR_CANCELLED, HWND as WinHwnd},
            System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER},
            UI::Shell::{
                Common::COMDLG_FILTERSPEC, FileOpenDialog, IFileOpenDialog, FOS_FILEMUSTEXIST,
                FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST, SIGDN_FILESYSPATH,
            },
        },
    };
    unsafe {
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| e.to_string())?;
        dialog
            .SetTitle(w!("選擇桌寵PNG，建議已去背；匯入後自動壓縮至500KB以下"))
            .map_err(|e| e.to_string())?;
        dialog
            .SetFileTypes(&[COMDLG_FILTERSPEC {
                pszName: w!("PNG圖片（建議透明背景）"),
                pszSpec: w!("*.png"),
            }])
            .map_err(|e| e.to_string())?;
        dialog
            .SetOptions(FOS_FILEMUSTEXIST | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST)
            .map_err(|e| e.to_string())?;
        if let Err(error) = dialog.Show(Some(WinHwnd(owner))) {
            if error.code() == windows::core::HRESULT::from_win32(ERROR_CANCELLED.0) {
                return Ok(None);
            }
            return Err(error.to_string());
        }
        let value = dialog
            .GetResult()
            .and_then(|item| item.GetDisplayName(SIGDN_FILESYSPATH))
            .map_err(|e| e.to_string())?;
        let path = value.to_string();
        CoTaskMemFree(Some(value.0.cast()));
        Ok(Some(PathBuf::from(path.map_err(|e| e.to_string())?)))
    }
}
