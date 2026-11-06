//! 待用戶確認的提示與完成通知分開；只在背景提醒，不搶焦點、不替用戶同意。
use super::*;

/// 看過的前景提示也記住，避免用戶讀到問題後切去別的程式又收到同一通知。
fn record_requests(
    seen: &mut std::collections::BTreeSet<String>,
    keys: Vec<String>,
    foreground: bool,
) -> bool {
    let mut added = false;
    for key in keys {
        added |= seen.insert(key);
    }
    added && !foreground
}

impl App {
    /// 終態事件先保存通知，再依背景／彈出偏好提醒；暫停點擊直接回到可繼續的對話。
    /// 呼叫者已核對有效 Running ID 並排除手動取消，重送 Finished 不會重複提醒。
    pub(super) fn project_finished_notice(
        &mut self,
        id: &str,
        conversation: &str,
        final_text: &str,
        paused: bool,
        waiting: bool,
        succeeded: bool,
    ) -> AppResult<()> {
        let notice = crate::notifications::Notification {
            id: format!("project_{id}"),
            kind: if paused {
                "project.paused"
            } else if waiting {
                "project.waiting_user"
            } else if succeeded {
                "project.completed"
            } else {
                "project.failed"
            }
            .into(),
            title: self.projects.status.clone(),
            summary: final_text.chars().take(50).collect(),
            created_at: crate::notifications::now_text(),
            expires_at: None,
            resource_id: Some(id.to_owned()),
            read_at: if self.is_foreground() && self.active_id.as_deref() == Some(conversation) {
                Some(crate::notifications::now_text())
            } else {
                None
            },
            dismissed: false,
        };
        self.inbox.merge(crate::notifications::EventPage {
            events: vec![notice],
            next_cursor: None,
            has_more: false,
        })?;
        crate::notifications::save(&self.root, &self.inbox)?;
        self.toast(&self.projects.status.clone());
        if self.config.notification_popups
            && !(self.is_foreground() && self.active_id.as_deref() == Some(conversation))
        {
            self.projects.attention_conversation =
                (paused || waiting).then(|| conversation.to_owned());
            self.work.tray_restore_only = false;
            self.work.task_balloon = true;
            self.desktop_notice(&self.projects.status.clone());
        }
        Ok(())
    }

    /// UI 每次 tick 都可呼叫；相同 request ID 只提醒一次，閱讀到提示後再縮小不重複通知。
    pub(in crate::ui) fn project_attention_tick(&mut self) {
        if !self.logged_in() || self.versions.blocked() {
            return;
        }
        let foreground = self.is_foreground();
        let mut notifications = Vec::new();
        for run in self
            .projects
            .running
            .values()
            .filter(|r| !r.cancel.load(Ordering::Relaxed))
        {
            let mut keys = Vec::new();
            if let Some(p) = &run.pending_cad {
                keys.push(format!("{}:cad:{}", run.id, p.id));
            }
            if let Some(p) = &run.pending_file {
                keys.push(format!("{}:file:{}", run.id, p.id));
            }
            if let Some(p) = &run.pending_outlook {
                keys.push(format!("{}:outlook:{}", run.id, p.id));
            }
            if let Some(p) = &run.pending_chart {
                keys.push(format!("{}:chart:{}", run.id, p.id));
            }
            if let Ok(questions) = run.instructions.questions() {
                keys.extend(
                    questions
                        .iter()
                        .filter(|q| q.state == "pending")
                        .map(|q| format!("{}:question:{}", run.id, q.id)),
                );
            }
            let conversation = run.conversation.clone();
            let viewing = foreground && self.active_id.as_deref() == Some(&conversation);
            if record_requests(&mut self.projects.attention_seen, keys, viewing) {
                notifications.push(conversation);
            }
        }
        let count = notifications.len();
        let Some(conversation) = notifications.into_iter().next() else {
            return;
        };
        self.projects.attention_conversation = Some(conversation);
        if self.config.notification_popups {
            self.desktop_notice(&format!(
                "{count} 個專案工作需要您確認，請回到 LM_AI 切換執行中的對話查看。"
            ));
        }
        let info = FLASHWINFO {
            cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
            hwnd: self.window,
            dwFlags: FLASHW_TRAY | FLASHW_TIMERNOFG,
            uCount: 3,
            dwTimeout: 0,
        };
        unsafe {
            FlashWindowEx(&info);
        }
    }

    /// 通知點擊沿用一般對話切換，不放寬登入或目前前景工作限制。
    pub(in crate::ui) fn open_project_attention(&mut self) -> AppResult<bool> {
        let Some(id) = self.projects.attention_conversation.take() else {
            return Ok(false);
        };
        self.command(Command::SelectChat { id })?;
        self.publish();
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn background_prompts_notify_once_without_treating_foreground_as_consent() {
        let mut seen = Default::default();
        assert!(record_requests(&mut seen, vec!["run:cad:1".into()], false));
        assert!(!record_requests(&mut seen, vec!["run:cad:1".into()], false));
        assert!(!record_requests(
            &mut seen,
            vec!["run:question:2".into()],
            true
        ));
        assert!(!record_requests(
            &mut seen,
            vec!["run:question:2".into()],
            false
        ));
        assert!(record_requests(
            &mut seen,
            vec!["run:file:3".into(), "run:outlook:4".into()],
            false
        ));
        assert!(!record_requests(&mut seen, vec![], false));
    }
}
