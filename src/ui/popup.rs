//! 通知方式統一由此套用。呼叫者仍負責前景抑制、去重及通知中心的保存。
use super::*;
use crate::notification_preferences::{Category, Delivery};

pub(in crate::ui) enum Target<'a> {
    Window,
    Tasks,
    Inbox,
    Project(&'a str),
    Schedule,
}

impl App {
    pub(super) fn pet_notification_option(&self) -> bool {
        cfg!(feature = "pet") && self.config.extra_features_enabled
    }

    /// 使用原通知的點擊導覽；關閉通知時不覆蓋上一則通知的導覽。
    /// 回傳 false 僅表示 Windows 未接受送出。提醒可沿用原有的稍後重試。
    pub(in crate::ui) fn popup(
        &mut self,
        category: Category,
        message: &str,
        legacy: Delivery,
        target: Target<'_>,
    ) -> bool {
        let delivery = self.config.notification_preferences.delivery(
            category,
            legacy,
            self.pet_notification_option(),
        );
        if delivery == Delivery::Silent {
            return true;
        }
        self.schedules.notice = matches!(target, Target::Schedule);
        self.work.tray_restore_only = matches!(target, Target::Window);
        self.work.task_balloon = matches!(target, Target::Tasks | Target::Project(_));
        self.projects.attention_conversation = match target {
            Target::Project(conversation) => Some(conversation.into()),
            _ => None,
        };
        if self.smoke {
            return true;
        }
        #[cfg(feature = "pet")]
        if matches!(delivery, Delivery::Pet | Delivery::Both) {
            if category == Category::Reminder {
                self.pet_notice(&format!("提醒：{message}"));
            } else {
                self.pet_notice(message);
            }
        }
        if matches!(delivery, Delivery::Windows | Delivery::Both) {
            let title = if category == Category::Reminder
                || (category == Category::Schedule
                    && self.config.notification_preferences.mode
                        == crate::notification_preferences::Mode::Legacy)
            {
                "LM_AI 提醒事項"
            } else {
                "LM_AI"
            };
            return super::reminders::windows_notice(self.window, title, message);
        }
        true
    }
}
