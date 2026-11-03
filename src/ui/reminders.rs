//! 本機提醒的介面命令與計時器。保存成功後才更新記憶體或送出通知。
use super::*;
use crate::reminders::{Channel, Store};

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub(super) enum ReminderCommand {
    State,
    Save {
        id: Option<u64>,
        title: String,
        due_at: u64,
        channel: Channel,
    },
    Delete {
        id: u64,
    },
}

pub(super) struct ReminderRuntime {
    store: AppResult<Store>,
    next_check: Instant,
}
impl ReminderRuntime {
    pub(super) fn load(root: &Path) -> Self {
        Self {
            store: Store::load(root),
            next_check: Instant::now(),
        }
    }
}

impl App {
    pub(super) fn reminder_command(&mut self, command: ReminderCommand) -> AppResult<()> {
        let result = (|| {
            let store = self.reminders.store.as_ref().map_err(Clone::clone)?;
            let mut next = store.clone();
            let changed = match command {
                ReminderCommand::State => false,
                ReminderCommand::Save {
                    id,
                    title,
                    due_at,
                    channel,
                } => {
                    next.upsert(id, &title, due_at, channel, crate::unix_now())?;
                    true
                }
                ReminderCommand::Delete { id } => {
                    next.delete(id)?;
                    true
                }
            };
            if changed {
                next.save(&self.root)?;
                self.reminders.store = Ok(next);
            }
            self.publish_reminders(true)
        })();
        if let Err(error) = &result {
            let _ = self
                .view
                .post(&json!({"type":"reminder_error", "message":error}));
        }
        result
    }

    fn publish_reminders(&self, acknowledgement: bool) -> AppResult<()> {
        let store = self.reminders.store.as_ref().map_err(Clone::clone)?;
        self.view.post(&json!({"type":"reminder_state", "entries":store.entries, "acknowledgement":acknowledgement}))
    }

    pub(super) fn tick_reminders(&mut self) {
        // 自檢／模型測試不觸發真實通知；正式程式即使未登入也可提醒。
        if self.smoke || self.demo || !self.ready || Instant::now() < self.reminders.next_check {
            return;
        }
        self.reminders.next_check = Instant::now() + Duration::from_secs(1);
        let Ok(store) = &self.reminders.store else {
            return;
        };
        let now = crate::unix_now();
        let Some(entry) = store.next_due(now) else {
            return;
        };
        let previous = store.clone();
        let mut next = previous.clone();
        let result = next
            .mark_notified(entry.id, now)
            .and_then(|()| next.save(&self.root));
        if let Err(error) = result {
            self.reminders.next_check = Instant::now() + Duration::from_secs(30);
            self.fail(error);
            self.publish();
            return;
        }
        // 先保存送出記錄，避免重開後每次重播。Windows是否顯示仍由通知／勿擾設定決定。
        self.reminders.store = Ok(next);
        self.work.tray_restore_only = true;
        let pet = self.pet.settings();
        let delivered =
            if entry.channel == Channel::Pet && pet.notice_seconds > 0 && pet.bubble_opacity > 0 {
                self.desktop_notice(&format!("提醒：{}", entry.title));
                true
            } else {
                windows_notice(self.window, &entry.title)
            };
        if !delivered {
            // Shell拒絕接收時保留未送出狀態，稍後再試；不可一直每50ms重試。
            if previous.save(&self.root).is_ok() {
                self.reminders.store = Ok(previous);
            }
            self.fail("Windows未接受提醒通知，請確認系統通知設定；提醒清單仍可查看。".into());
            self.publish();
        }
        self.reminders.next_check =
            Instant::now() + Duration::from_secs(if delivered { 6 } else { 30 });
        let _ = self.publish_reminders(false);
    }
}

/// 沿用應用程式托盤圖示，文字只來自使用者明確設定的提醒。
fn windows_notice(window: HWND, title: &str) -> bool {
    let mut data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: window,
        uID: 1,
        uFlags: NIF_INFO,
        dwInfoFlags: NIIF_INFO,
        ..Default::default()
    };
    copy_notice_text(&mut data.szInfoTitle, "LM_AI 提醒事項");
    copy_notice_text(&mut data.szInfo, title);
    unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) != 0 }
}

/// 依字元截斷，保留結尾NUL且不切斷emoji的UTF-16代理對。
fn copy_notice_text(buffer: &mut [u16], text: &str) {
    let mut at = 0;
    for character in text.chars() {
        let mut units = [0; 2];
        let encoded = character.encode_utf16(&mut units);
        if at + encoded.len() >= buffer.len() {
            break;
        }
        buffer[at..at + encoded.len()].copy_from_slice(encoded);
        at += encoded.len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_shell_accepts_native_reminder_notification() {
        unsafe {
            let window = CreateWindowExW(
                0,
                wide("STATIC").as_ptr(),
                wide("LM_AI notification test").as_ptr(),
                WS_POPUP,
                0,
                0,
                1,
                1,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null(),
            );
            assert!(!window.is_null());
            let data = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: window,
                uID: 1,
                uFlags: NIF_ICON,
                hIcon: LoadIconW(ptr::null_mut(), IDI_INFORMATION),
                ..Default::default()
            };
            let added = Shell_NotifyIconW(NIM_ADD, &data);
            let delivered = added != 0 && windows_notice(window, "LM_AI 本機提醒功能測試");
            Shell_NotifyIconW(NIM_DELETE, &data);
            DestroyWindow(window);
            assert_ne!(added, 0, "Windows未接受測試托盤圖示");
            assert!(delivered, "Windows未接受提醒通知");
        }
    }

    #[test]
    fn notification_truncation_preserves_utf16_and_termination() {
        let mut buffer = [0; 4];
        copy_notice_text(&mut buffer, "中😀文");
        assert_eq!(String::from_utf16(&buffer[..3]).unwrap(), "中😀");
        assert_eq!(buffer[3], 0);
    }
}
