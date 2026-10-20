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
    /// UI 每次 tick 都可呼叫；相同 request ID 只提醒一次，閱讀到提示後再縮小不重複通知。
    pub(in crate::ui) fn project_attention_tick(&mut self) {
        if !self.logged_in() || self.versions.blocked() {
            return;
        }
        let Some(run) = self
            .projects
            .running
            .as_ref()
            .filter(|r| !r.cancel.load(Ordering::Relaxed))
        else {
            return;
        };
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
        let foreground = self.is_foreground();
        if !record_requests(&mut self.projects.attention_seen, keys, foreground) {
            return;
        }
        self.projects.attention_conversation = Some(conversation);
        if self.config.notification_popups {
            tray(
                self.window,
                NIM_MODIFY,
                Some("專案工作需要您確認，請回到 LM_AI 查看待處理的問題。"),
            );
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
