//! 計畫卡片的原生身分與版本檢查；核准只影響原任務，不允許跨對話套用。
use super::*;

impl App {
    pub(super) fn task_plan_command(&mut self, command: &ProjectCommand) -> AppResult<bool> {
        if let ProjectCommand::PlanPreference { enabled } = command {
            if !self.logged_in() || self.versions.blocked() {
                return Err("請先登入。".into());
            }
            let mut store = self.projects.store.clone();
            store.plan_mode = *enabled;
            store.save(&self.root)?;
            self.projects.store = store;
            return Ok(true);
        }
        let (conversation, run_id, message_index) = match command {
            ProjectCommand::PlanView {
                conversation,
                run_id,
                message_index,
            }
            | ProjectCommand::PlanReview {
                conversation,
                run_id,
                message_index,
                ..
            } => (conversation, run_id, *message_index),
            _ => return Ok(false),
        };
        if !self.logged_in()
            || self.versions.blocked()
            || self.active_id.as_ref() != Some(conversation)
            || !self
                .messages
                .get(message_index)
                .is_some_and(|m| m.request_id.as_ref() == Some(run_id))
        {
            return Err("請在原專案對話開啟計畫。".into());
        }
        let project = self
            .projects
            .store
            .project_for(conversation)
            .cloned()
            .ok_or("沒有此專案授權。")?;
        if matches!(command, ProjectCommand::PlanView { .. }) {
            let plan = projects::runner::pending_plan(&self.root, run_id).or_else(|| {
                self.messages[message_index]
                    .project_overview
                    .as_ref()
                    .and_then(|o| o.task_plan.clone())
            });
            self.view.post(&json!({"type":"project_plan","conversation":conversation,"run_id":run_id,"message_index":message_index,"plan":plan}))?;
            return Ok(true);
        }
        let ProjectCommand::PlanReview {
            revision,
            edited,
            feedback,
            ..
        } = command
        else {
            return Ok(false);
        };
        if self.projects.is_running()
            || self.busy != "none"
            || self.work.incoming.is_some()
            || !self.can_send()
            || message_index + 1 != self.messages.len()
            || !self.messages[message_index].project_paused
        {
            return Err("請先完成目前操作，並使用最新暫停計畫。".into());
        }
        let principal =
            crate::jobs::capabilities(&self.config, self.session.as_ref().ok_or("請先登入。")?)?
                .principal_id;
        let plan = projects::runner::review_plan(
            &self.root,
            run_id,
            &project,
            conversation,
            &principal,
            *revision,
            edited.as_deref().cloned(),
            feedback,
        )?;
        if let Some(overview) = self.messages[message_index].project_overview.as_mut() {
            overview.task_plan = Some(plan.clone());
        }
        let mut archive = self.archive.clone();
        let chat = archive
            .conversations
            .iter_mut()
            .find(|c| &c.id == conversation)
            .ok_or("對話已不存在。")?;
        chat.messages = self.messages.clone();
        history::save(&self.root, &archive)?;
        self.archive = archive;
        self.view.post(&json!({"type":"project_plan","conversation":conversation,"run_id":run_id,"message_index":message_index,"plan":plan}))?;
        self.begin_project_segment(self.messages.clone(), Some(run_id.clone()))?;
        Ok(true)
    }
}
