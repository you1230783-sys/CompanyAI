//! 桌寵偏好與圖片匯入只由使用者介面發起；模型沒有此命令。
use super::*;
#[cfg(feature = "pet")]
#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub(super) enum PetCommand {
    State,
    Configure {
        enabled: bool,
        skin: String,
        size: u16,
        reduced_motion: bool,
        bubble_style: String,
        bubble_color: String,
        bubble_opacity: u8,
        notice_seconds: u8,
        font_size: Option<u8>,
    },
    Import {
        state: String,
    },
    Preview,
    CopyPrompt {
        text: String,
    },
}
impl App {
    #[cfg(feature = "pet")]
    pub(super) fn desktop_notice(&mut self, message: &str) {
        if !self.smoke {
            self.pet.typography(self.config.ui_font);
            self.pet.notify(message);
        }
    }
    #[cfg(feature = "pet")]
    pub(super) fn pet_command(&mut self, command: PetCommand) -> AppResult<()> {
        let result = (|| {
            let mut include_images = true;
            match command {
                PetCommand::State => (),
                PetCommand::Configure {
                    enabled,
                    skin,
                    size,
                    reduced_motion,
                    bubble_style,
                    bubble_color,
                    bubble_opacity,
                    notice_seconds,
                    font_size,
                } => {
                    let mut settings = self.pet.settings();
                    // 精細圖檔較大；滑桿調整只回偏好，換角色或匯入時才傳六張預覽。
                    include_images = settings.skin != skin;
                    settings.enabled = enabled;
                    settings.skin = skin;
                    settings.size = size;
                    settings.reduced_motion = reduced_motion;
                    settings.bubble_style = bubble_style;
                    settings.bubble_color = bubble_color;
                    settings.bubble_opacity = bubble_opacity;
                    settings.notice_seconds = notice_seconds;
                    if let Some(font_size) = font_size {
                        settings.font_size = font_size;
                    }
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
                PetCommand::CopyPrompt { text } => {
                    if text.chars().count() > 8000 {
                        return Err("提示詞請勿超過8000字。".into());
                    }
                    selection::copy_text(self.window, &text)?;
                    self.view
                        .post(&json!({"type":"pet_imported","message":"提示詞已複製。"}))?;
                }
                PetCommand::Preview => {
                    self.work.tray_restore_only = true;
                    self.desktop_notice("這是桌寵通知。點一下可回到主視窗；拖曳角色可移動位置。");
                }
            }
            let state = if include_images {
                self.pet.state()?
            } else {
                json!({"settings": self.pet.settings()})
            };
            self.view.post(&json!({"type":"pet_state","state":state}))
        })();
        if let Err(error) = &result {
            let _ = self.view.post(&json!({"type":"pet_error","message":error}));
        }
        result
    }
    #[cfg(feature = "pet")]
    pub(super) fn tick_pet(&mut self) {
        self.pet.typography(self.config.ui_font);
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
#[cfg(feature = "pet")]
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

// 核心通知中心仍保存通知；未編入桌寵時不建立透明視窗或載入角色圖片。
#[cfg(not(feature = "pet"))]
impl App {
    pub(super) fn desktop_notice(&mut self, _message: &str) {}
    pub(super) fn tick_pet(&mut self) {}
}
