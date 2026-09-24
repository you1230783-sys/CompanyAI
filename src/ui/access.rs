//! 原生命令的登入與版本門檻。前端 disabled 不能取代此檢查。
use super::{AppResult, Command};

impl Command {
    pub(super) fn check_access(
        &self,
        logged_in: bool,
        update_required: bool,
        smoke: bool,
    ) -> AppResult<()> {
        let login_flow = matches!(self, Self::Login | Self::CancelLogin | Self::ReopenLogin);
        let lifecycle = matches!(self, Self::Ready | Self::Exit | Self::SelfTestResult { .. });
        // --self-check 既有的原生鍵盤錄製測試不讀真實帳密，例外僅限這三個錄製命令。
        let smoke_recording = smoke
            && matches!(
                self,
                Self::StartHotkeyRecording
                    | Self::CancelHotkeyRecording
                    | Self::RecordedHotkey { .. }
            );
        if !logged_in && !login_flow && !lifecycle && !smoke_recording {
            return Err("請先完成瀏覽器登入，再使用此功能。".into());
        }
        // 未登入仍能完成登入；登入後再顯示既有的強制更新畫面，避免兩種門檻互相卡住。
        if update_required
            && !login_flow
            && !lifecycle
            && !matches!(self, Self::Draft { .. } | Self::Refresh | Self::Download)
        {
            return Err("此版本已停止支援，安裝更新完成前無法使用功能。".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unauthenticated_features_are_rejected_even_when_called_directly() {
        let commands = [
            Command::ReadMail,
            Command::ReadMailBody,
            Command::NewChat,
            Command::Vnc {
                command: super::super::vnc::VncCommand::Open,
            },
            Command::Preferences {
                font_size: 14,
                sidebar_collapsed: false,
                notification_popups: true,
                dark_mode: None,
            },
            Command::Copy {
                text: "test".into(),
            },
            Command::Refresh,
            Command::Download,
        ];
        for command in commands {
            assert!(command.check_access(false, false, false).is_err());
            assert!(command.check_access(false, false, true).is_err());
            assert!(command.check_access(true, false, false).is_ok());
        }
    }

    #[test]
    fn login_remains_available_without_bypassing_required_update() {
        for command in [
            Command::Login,
            Command::CancelLogin,
            Command::ReopenLogin,
            Command::Ready,
            Command::Exit,
        ] {
            assert!(command.check_access(false, true, false).is_ok());
        }
        assert!(Command::ReadMail.check_access(true, true, false).is_err());
        assert!(Command::Download.check_access(true, true, false).is_ok());
        assert!(Command::StartHotkeyRecording
            .check_access(false, false, false)
            .is_err());
        assert!(Command::StartHotkeyRecording
            .check_access(false, false, true)
            .is_ok());
    }
}
