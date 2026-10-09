//! 主輸入框的專案送出方式。排程先落盤，舊 worker 結束後才啟動新 worker。
use super::*;
use crate::history::QueuedProjectMessage;

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::ui) enum ComposeMode {
    NextTurn,
    Interrupt,
    AfterTask,
}

impl App {
    /// 原生命令逐次核對對話與執行 ID，不將過期送出轉成另一個任務的指示。
    pub(super) fn compose_project_message(
        &mut self,
        conversation: &str,
        run_id: &str,
        mode: ComposeMode,
        instruction_id: Option<&str>,
        text: &str,
    ) -> AppResult<String> {
        let inbox = self.supplement_inbox(conversation, run_id)?;
        if matches!(mode, ComposeMode::NextTurn) {
            return inbox.submit(instruction_id, text);
        }
        if instruction_id.is_some() {
            return Err("修改補充指示時，請使用「下一輪加入提示」。".into());
        }
        if text.trim().is_empty() || text.encode_utf16().count() > 16_000 {
            return Err("下一則訊息需為 1–16000 個字元。".into());
        }
        let project = self
            .projects
            .store
            .project_for(conversation)
            .ok_or("專案已不存在。")?;
        if self.work.store.principal_id.is_empty() || self.history_error.is_some() {
            return Err("無法確認帳號或保存排程，文字尚未送出。".into());
        }
        let pending = QueuedProjectMessage {
            local_time: Some(crate::calendar::local_timestamp()),
            id: crate::jobs::new_id()?,
            after_run: run_id.into(),
            project_id: project.id.clone(),
            principal_id: self.work.store.principal_id.clone(),
            model: self.config.model.clone(),
            text: text.into(),
            interrupt: matches!(mode, ComposeMode::Interrupt),
            auto_start: true,
        };
        let mut archive = self.archive.clone();
        let chat = archive
            .conversations
            .iter_mut()
            .find(|c| c.id == conversation)
            .ok_or("對話已不存在。")?;
        if chat.project_queued.is_some() {
            return Err("已有一則待送訊息，請先取消原排程；文字仍留在輸入框。".into());
        }
        chat.project_queued = Some(pending.clone());
        history::save(&self.root, &archive)?;
        self.archive = archive;
        // 保存成功才取消；新任務由 Finished 事件接手，不與舊檔案操作重疊。
        if pending.interrupt {
            self.projects.cancel();
        }
        Ok(pending.id)
    }

    pub(super) fn cancel_queued_project(&mut self, conversation: &str, id: &str) -> AppResult<()> {
        if self.active_id.as_deref() != Some(conversation) {
            return Err("請在原對話取消排程。".into());
        }
        let mut archive = self.archive.clone();
        let chat = archive
            .conversations
            .iter_mut()
            .find(|c| c.id == conversation)
            .ok_or("對話已不存在。")?;
        if !chat.project_queued.as_ref().is_some_and(|q| q.id == id) {
            return Err("待送訊息已變更或已送出。".into());
        }
        chat.project_queued = None;
        history::save(&self.root, &archive)?;
        self.archive = archive;
        Ok(())
    }

    /// 手動停止取消的是本次自動接續意圖；排程文字仍保留供使用者查看或送出。
    pub(super) fn hold_queued_project(&mut self, conversation: &str) -> AppResult<()> {
        // 先在記憶體關閉自動接續；即使落盤失敗，也不能違反使用者剛按的停止。
        if let Some(queued) = self
            .archive
            .conversations
            .iter_mut()
            .find(|c| c.id == conversation)
            .and_then(|c| c.project_queued.as_mut())
        {
            queued.auto_start = false;
            history::save(&self.root, &self.archive)?;
        }
        Ok(())
    }

    /// 可在另一個對話開啟時接續，仍綁定原專案、帳號與模型，不切換使用者畫面。
    pub(super) fn start_queued_project(&mut self, conversation: &str, id: &str) -> AppResult<()> {
        let chat = self
            .archive
            .conversations
            .iter()
            .find(|c| c.id == conversation)
            .ok_or("對話已不存在。")?;
        let queued = chat
            .project_queued
            .as_ref()
            .filter(|q| q.id == id)
            .ok_or("待送訊息已變更或已送出。")?
            .clone();
        if !self.logged_in()
            || self.versions.blocked()
            || self.busy != "none"
            || self.projects.running.is_some()
            || self.projects.error.is_some()
            || self.work.incoming.is_some()
            || self.work.capability_loading
            || self.work.storage_error
            || self.history_error.is_some()
            || self.work.store.pending(Some(conversation))
            || self.work.store.principal_id != queued.principal_id
            || self.config.model != queued.model
            || !self.work.caps.as_ref().is_some_and(|c| {
                c.principal_id == queued.principal_id && c.supports(&self.work.mode)
            })
            || !self
                .models
                .as_ref()
                .is_some_and(|c| c.models.iter().any(|m| m.id == queued.model))
        {
            return Err(
                "待送訊息已保留；請確認原帳號、模型、登入與版本狀態，再按「現在傳送」。".into(),
            );
        }
        if !self
            .projects
            .store
            .project_for(conversation)
            .is_some_and(|p| p.id == queued.project_id)
        {
            return Err("原專案授權已變更，待送訊息不會自動執行。".into());
        }
        let mut messages = chat.messages.clone();
        if messages.len() >= 998 {
            return Err("此專案對話已達長度上限，請將待送文字帶到新對話。".into());
        }
        let mut message = Message::user(&queued.text);
        message.local_time = queued.local_time;
        messages.push(message);
        // 排程 ID 同時作為新 run ID；清除排程與加入使用者訊息在同一次歷史保存完成。
        self.begin_project_for(conversation.into(), messages, None, Some(&queued.id))
    }

    pub(super) fn finish_queued_project(
        &mut self,
        conversation: &str,
        run_id: &str,
        completed: bool,
        cancelled: bool,
    ) {
        let queued = self
            .archive
            .conversations
            .iter()
            .find(|c| c.id == conversation)
            .and_then(|c| c.project_queued.clone())
            .filter(|q| q.after_run == run_id && q.auto_start);
        let Some(queued) = queued else {
            return;
        };
        // 暫停、缺資料或失敗不能冒充完成；其餘終態只保留文字供手動傳送。
        if queued.ready_after(run_id, completed, cancelled) {
            if let Err(error) = self.start_queued_project(conversation, &queued.id) {
                self.fail(error);
                let _ = self.hold_queued_project(conversation);
            }
        } else if let Err(error) = self.hold_queued_project(conversation) {
            self.fail(error);
        }
    }
}
