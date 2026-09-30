//! 使用者主動重試最新提問；保留舊回覆，並使用首次提問時的上下文。
use super::*;
use crate::jobs::{self, Task};

/// 重試訊息指向較早的使用者訊息，不把失敗回覆再次塞入同一請求。
pub(super) fn context(messages: &[Message]) -> AppResult<Vec<Message>> {
    let last = messages.last().ok_or("找不到要重試的提問。")?;
    let index = last.retry_context_index.unwrap_or(messages.len() - 1);
    if last.role != "user" || index >= messages.len() || messages[index].role != "user" {
        return Err("重試上下文已變更，請重新開啟對話。".into());
    }
    let mut result = messages[..=index].to_vec();
    *result.last_mut().ok_or("缺少使用者訊息。")? = last.clone();
    Ok(result)
}

impl App {
    fn retry_allowed(&self, index: usize) -> bool {
        if !self.logged_in()
            || self.versions.blocked()
            || self.busy != "none"
            || self.history_error.is_some()
            || self.work.storage_error
            || self.work.incoming.is_some()
            || self.projects.active(self.active_id.as_deref())
            || (self.mail_flow.phase != "idle" && self.mail_flow.conversation == self.active_id)
            || !self.work.store.drafts(self.active_id.as_deref()).is_empty()
        {
            return false;
        }
        let Some(message) = self.messages.get(index) else {
            return false;
        };
        if let Some(task) = self.work.store.tasks.iter().find(|task| {
            Some(task.request_id.as_str()) == message.request_id.as_deref()
                && Some(&task.conversation_id) == self.active_id.as_ref()
                && !task.title_generation
        }) {
            if self.work.streams.contains(&task.request_id) {
                return false;
            }
            if !task.remote.as_ref().is_some_and(jobs::TaskStatus::terminal) {
                // 結果不明時只允許用同一 ID 重送；已停止追蹤且請求被清除者不可猜測。
                return task.active()
                    && task.remote.is_none()
                    && task.request.get("messages").is_some();
            }
            if task.active() {
                return false;
            } // 終態尚未寫入對話，先等原回覆保存。
        } else if self
            .messages
            .iter()
            .skip(index + 1)
            .any(|message| message.incomplete)
        {
            return false;
        }
        self.can_send()
            && self.messages.len()
                < if self
                    .active_id
                    .as_deref()
                    .and_then(|id| self.projects.store.project_for(id))
                    .is_some()
                {
                    999
                } else {
                    39
                }
    }

    pub(super) fn retry_state(&self) -> serde_json::Value {
        let Some(index) = self
            .messages
            .iter()
            .rposition(|message| message.role == "user")
        else {
            return serde_json::Value::Null;
        };
        json!({"user_index":index,"message_count":self.messages.len(),
            "request_id":self.messages[index].request_id,"enabled":self.retry_allowed(index)})
    }

    pub(super) fn retry_chat(
        &mut self,
        conversation: &str,
        index: usize,
        count: usize,
        request_id: Option<&str>,
    ) -> AppResult<()> {
        if self.active_id.as_deref() != Some(conversation)
            || self.messages.len() != count
            || self
                .messages
                .iter()
                .rposition(|message| message.role == "user")
                != Some(index)
            || self
                .messages
                .get(index)
                .and_then(|message| message.request_id.as_deref())
                != request_id
        {
            return Err("對話已變更，請使用目前顯示的重試按鈕。".into());
        }
        if !self.retry_allowed(index) {
            return Err("目前尚有工作、附件或結果待確認，請先完成或確認後再重試。".into());
        }
        let previous = self
            .work
            .store
            .tasks
            .iter()
            .find(|task| {
                Some(task.request_id.as_str()) == request_id
                    && task.conversation_id == conversation
                    && !task.title_generation
            })
            .cloned();
        if let Some(task) = &previous {
            if task.active() {
                // 原請求可能已被伺服器接受；保留識別碼及完整內容，禁止另建重複工作。
                self.submit_work(task.clone())?;
                self.toast("正在重新確認原本的請求");
                return Ok(());
            }
        }
        self.preserve_draft()?;
        let draft = self.draft.clone();
        let mut messages = self.messages.clone();
        let mut question = messages[index].clone();
        question.retry_context_index = Some(question.retry_context_index.unwrap_or(index));
        question.project_activity.clear();
        messages.push(question);
        if self.projects.store.project_for(conversation).is_some() {
            self.begin_project_chat(messages)?;
        } else {
            self.retry_work_chat(conversation, messages, previous.as_ref())?;
        }
        // 重試針對既有提問，不吃掉使用者尚未送出的草稿。
        self.set_draft(draft);
        self.preserve_draft()?;
        Ok(())
    }

    fn retry_work_chat(
        &mut self,
        conversation: &str,
        mut messages: Vec<Message>,
        previous: Option<&Task>,
    ) -> AppResult<()> {
        if self
            .work
            .store
            .tasks
            .iter()
            .filter(|task| task.active())
            .count()
            >= 8
        {
            return Err("請先等待部分任務完成後再重試。".into());
        }
        let id = jobs::new_id()?;
        let question = messages.last_mut().ok_or("缺少使用者訊息。")?;
        question.request_id = Some(id.clone());
        let title: String = question.content.chars().take(50).collect();
        let mut request = if let Some(task) =
            previous.filter(|task| task.request.get("messages").is_some())
        {
            // 保留原模型、用途與附件；僅換這次使用者明確要求的新請求 ID。
            task.request.clone()
        } else {
            let settings =
                question
                    .retry_settings
                    .clone()
                    .unwrap_or(crate::protocol::RetrySettings {
                        model: self.config.model.clone(),
                        mode: self.work.mode.clone(),
                        skills: true,
                        attachment_ids: vec![],
                    });
            if !question.attachments.is_empty() && settings.attachment_ids.is_empty() {
                return Err("原附件紀錄已不可用，請重新加入附件後送出。".into());
            }
            let tokens = settings
                .attachment_ids
                .iter()
                .map(|id| {
                    self.work
                        .store
                        .attachments
                        .iter()
                        .find(|file| {
                            &file.id == id && file.conversation_id == conversation && !file.removed
                        })
                        .and_then(|file| file.token())
                        .map(str::to_owned)
                        .ok_or_else(|| "原附件已到期或移除，請重新加入附件。".to_string())
                })
                .collect::<AppResult<Vec<_>>>()?;
            let mut request = jobs::chat_request(
                &settings.model,
                &context(&messages)?,
                conversation,
                &id,
                &settings.mode,
                tokens,
            )?;
            jobs::set_skills(&mut request, settings.skills);
            request
        };
        let model = request["model"].as_str().ok_or("原請求缺少模型。")?;
        if !self
            .models
            .as_ref()
            .is_some_and(|catalog| catalog.models.iter().any(|entry| entry.id == model))
        {
            return Err("原請求使用的模型目前不可用，請重新送出並選擇可用模型。".into());
        }
        // 即使原任務尚在，也不傳送已失效的附件 Token。
        if request["attachment_tokens"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|token| {
                !self.work.store.attachments.iter().any(|file| {
                    file.conversation_id == conversation
                        && !file.removed
                        && file
                            .token()
                            .is_some_and(|value| token.as_str() == Some(value))
                })
            })
        {
            return Err("原附件已到期或移除，請重新加入附件。".into());
        }
        let skills = request["skills"]
            .as_bool()
            .unwrap_or_else(|| previous.is_none_or(|task| !task.mail_analysis));
        jobs::set_skills(&mut request, skills);
        request["client_request_id"] = json!(id);
        request["conversation_id"] = json!(conversation);
        let mode = request["execution_mode"]
            .as_str()
            .ok_or("原請求缺少模式。")?
            .to_owned();
        let task = Task {
            request_id: id,
            conversation_id: conversation.into(),
            mode,
            title,
            mail_analysis: request["skills"] == false,
            request,
            created_at: crate::unix_now(),
            remote: None,
            applied: false,
            message: "正在重新嘗試".into(),
            title_generation: false,
            tool_events: Vec::new(),
            partial: String::new(),
        };
        self.messages = messages;
        self.save_history()?;
        self.work.store.tasks.push(task.clone());
        self.work_save()?;
        self.submit_work(task)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_retry_keeps_original_context_and_does_not_include_failed_answers() {
        let original = Message::user("修訂文字");
        let mut retry = original.clone();
        retry.retry_context_index = Some(0);
        let messages = vec![
            original,
            Message::assistant("失敗".into()),
            retry.clone(),
            Message::assistant("再次失敗".into()),
            retry,
        ];
        let context = context(&messages).unwrap();
        assert_eq!(context.len(), 1);
        assert_eq!(context[0].content, "修訂文字");
    }
}
