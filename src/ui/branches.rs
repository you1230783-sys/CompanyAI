//! 分支控制器：先核對畫面身分及附件，再保存新版本；切換永遠不送出工作。
use super::*;
mod smoke;

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum BranchCommand {
    Edit {
        conversation: String,
        branch_id: String,
        index: usize,
        message_count: usize,
        request_id: Option<String>,
        text: String,
    },
    Select {
        conversation: String,
        current_branch: String,
        target_branch: String,
    },
}
impl App {
    pub(super) fn is_branch_snapshot(&self, conversation: &str) -> bool {
        self.archive
            .conversations
            .iter()
            .find(|c| c.id == conversation)
            .is_some_and(|c| c.branch.snapshot)
    }
    /// 僅解析保留訊息明確持有的附件，不從整個對話附件池推測授權。
    pub(super) fn branch_attachment_tokens(
        &self,
        conversation: &str,
        messages: &[Message],
    ) -> AppResult<Vec<String>> {
        let mut tokens = Vec::new();
        for message in messages.iter().filter(|m| m.role == "user") {
            if !message.attachments.is_empty()
                && message
                    .retry_settings
                    .as_ref()
                    .is_none_or(|s| s.attachment_ids.is_empty())
            {
                return Err(
                    "原附件紀錄已不可用，請另開對話重新附加；不會忽略附件建立分支。".into(),
                );
            }
            for id in message
                .retry_settings
                .iter()
                .flat_map(|s| &s.attachment_ids)
            {
                let file = self
                    .work
                    .store
                    .attachments
                    .iter()
                    .find(|file| {
                        file.id == *id && file.conversation_id == conversation && !file.removed
                    })
                    .ok_or("原附件已移除，請另開對話重新附加。")?;
                let token = file
                    .token()
                    .ok_or("原附件已到期或尚未就緒，請另開對話重新附加。")?
                    .to_owned();
                if !tokens.contains(&token) {
                    tokens.push(token);
                }
            }
        }
        Ok(tokens)
    }
    fn branch_idle(&self) -> bool {
        self.logged_in()
            && !self.versions.blocked()
            && !self.panic_fault
            && self.busy == "none"
            && self.history_error.is_none()
            && !self.work.storage_error
            && self.work.incoming.is_none()
            && !self.projects.active(self.active_id.as_deref())
            && (self.mail_flow.phase == "idle" || self.mail_flow.conversation != self.active_id)
            && self.work.store.drafts(self.active_id.as_deref()).is_empty()
            && !self.work.store.tasks.iter().any(|t| {
                Some(&t.conversation_id) == self.active_id.as_ref()
                    && !t.title_generation
                    && (t.active() || self.work.streams.contains(&t.request_id))
            })
    }
    pub(super) fn branch_state(&self) -> serde_json::Value {
        let Some(chat) = self
            .archive
            .conversations
            .iter()
            .find(|c| Some(&c.id) == self.active_id.as_ref())
        else {
            return serde_json::Value::Null;
        };
        let project = self.projects.store.project_for(&chat.id).is_some();
        let mut versions: Vec<_> = chat
            .alternate_branches
            .iter()
            .map(|b| &b.info)
            .chain([&chat.branch])
            .collect();
        versions.sort_by_key(|b| b.number);
        json!({"id":chat.branch.id,"number":chat.branch.number,"versions":versions,
            "idle":self.branch_idle(),"can_send":self.can_send(),"project":project,
            "draft_index":self.messages.len().checked_sub(1).filter(|i| history::editable_project_draft(&self.messages,*i)),
            "notice":"請先完成目前工作，並移除或送出尚未送出的附件。"})
    }
    pub(super) fn branch_command(&mut self, command: BranchCommand) -> AppResult<()> {
        let (conversation, branch_id) = match &command {
            BranchCommand::Edit {
                conversation,
                branch_id,
                ..
            } => (conversation, branch_id),
            BranchCommand::Select {
                conversation,
                current_branch,
                ..
            } => (conversation, current_branch),
        };
        if !self.branch_idle() || self.active_id.as_ref() != Some(conversation) {
            return Err("請在原對話完成目前工作，並移除或送出草稿附件後再編輯或切換版本。".into());
        }
        let index = self
            .archive
            .conversations
            .iter()
            .position(|c| &c.id == conversation && &c.branch.id == branch_id)
            .ok_or("對話版本已變更，請重新操作。")?;
        let project = self.projects.store.project_for(conversation).is_some();
        let mut archive = self.archive.clone();
        // 草稿可能只在畫面上，交換前也必須保存至原版本。
        archive.conversations[index].draft = self.draft.clone();
        let mut submit = false;
        match command {
            BranchCommand::Select { target_branch, .. } => {
                if project {
                    return Err("專案不提供原地切換分支，請使用從這裡另開新對話。".into());
                }
                archive.conversations[index].select_branch(&target_branch)?;
            }
            BranchCommand::Edit {
                index: message_index,
                message_count,
                request_id,
                text,
                ..
            } => {
                if self.messages.len() != message_count
                    || self
                        .messages
                        .get(message_index)
                        .is_none_or(|m| m.role != "user" || m.request_id != request_id)
                {
                    return Err("訊息已變更，請重新開啟編輯。".into());
                }
                if text.trim().is_empty() || text.encode_utf16().count() > 16_000 {
                    return Err("提問需為1至16,000字元。".into());
                }
                if project {
                    if !history::editable_project_draft(&self.messages, message_index) {
                        return Err("專案訊息已執行，不能原地編輯；請從這裡另開新對話。".into());
                    }
                    archive.conversations[index].messages.pop();
                    archive.conversations[index].draft = text;
                } else {
                    if !self.can_send()
                        || self.work.store.tasks.iter().filter(|t| t.active()).count() >= 8
                    {
                        return Err("目前不能送出新請求，請等待工作完成並確認模型可用。".into());
                    }
                    self.branch_attachment_tokens(
                        &archive.conversations[index].id,
                        &self.messages[..=message_index],
                    )?;
                    archive.conversations[index].fork_message(message_index, &text)?;
                    // 在修改目前版本前驗證序列化大小等限制；真正送出仍產生另一個新ID。
                    crate::jobs::snapshot_chat_request(
                        &self.config.model,
                        &archive.conversations[index].messages,
                        &archive.conversations[index].id,
                        "preflight",
                        &self.work.mode,
                        vec![],
                    )?;
                    submit = true;
                }
            }
        }
        history::save(&self.root, &archive)?;
        self.messages = archive.conversations[index].messages.clone();
        let draft = archive.conversations[index].draft.clone();
        self.archive = archive;
        self.handoff = None;
        self.set_draft(draft);
        self.focus_draft = project;
        if submit {
            self.begin_work_chat(self.messages.clone(), "chat")?;
        }
        self.status = if submit {
            "已建立並送出新版本，舊版本完整保留。"
        } else if project {
            "已放回草稿，尚未送出。"
        } else {
            "已切換版本，沒有送出或執行任何工作。"
        }
        .into();
        self.view.post(&json!({"type":"branch_done"}))?;
        Ok(())
    }
}
