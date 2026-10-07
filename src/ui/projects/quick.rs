//! 專案 Outlook 快速入口；圖片直接沿用一般檔案閱讀，不提供獨立入口。
use super::*;

#[derive(Clone, Copy, PartialEq, serde::Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::ui) enum Kind {
    Outlook,
}

pub(super) struct Pending {
    id: String,
    conversation: String,
    project: String,
    principal: String,
    model: String,
    root: PathBuf,
    messages: usize,
}

impl App {
    fn quick_pending(
        &self,
        conversation: &str,
        id: &str,
    ) -> AppResult<(&Pending, projects::Project)> {
        let project = self.weekly_project(conversation)?;
        let pending = self
            .projects
            .quick
            .as_ref()
            .filter(|p| {
                p.id == id
                    && p.conversation == conversation
                    && p.project == project.id
                    && p.root == project.root
                    && p.principal == self.work.store.principal_id
                    && p.model == self.config.model
                    && p.messages == self.messages.len()
            })
            .ok_or("此操作視窗已失效，請在原專案重新開啟。")?;
        Ok((pending, project))
    }

    fn prepare_quick(&mut self, conversation: &str, id: &str, kind: Kind) -> AppResult<()> {
        crate::jobs::validate_id(id)?;
        let project = self.weekly_project(conversation)?;
        self.projects.quick = Some(Pending {
            id: id.into(),
            conversation: conversation.into(),
            project: project.id,
            principal: self.work.store.principal_id.clone(),
            model: self.config.model.clone(),
            root: project.root,
            messages: self.messages.len(),
        });
        let today = projects::setup::local_date()?;
        self.view.post(&json!({"type":"project_quick_ready","conversation":conversation,"request_id":id,
            "kind":kind,"start":projects::setup::monday(today).to_string(),"end":today.to_string()}))
    }

    fn submit_quick(
        &mut self,
        conversation: &str,
        id: &str,
        start: &str,
        end: &str,
        notes: &str,
    ) -> AppResult<()> {
        self.quick_pending(conversation, id)?;
        if notes.chars().count() > 1000 {
            return Err("補充內容最多 1000 字。".into());
        }
        let prompt =
            projects::setup::outlook_prompt(projects::setup::local_date()?, start, end, notes)?;
        let mut messages = self.messages.clone();
        messages.push(Message::user(&prompt));
        self.begin_project_chat(messages)?;
        self.projects.quick = None;
        Ok(())
    }

    /// 回覆一律附對話與請求代號；畫面不能接收別的專案較晚返回的結果。
    pub(super) fn quick_command(&mut self, command: &ProjectCommand) -> AppResult<bool> {
        match command {
            ProjectCommand::QuickPrepare {
                conversation,
                request_id,
                kind,
            } => {
                let result = self.prepare_quick(conversation, request_id, *kind);
                if result.is_err() {
                    self.view.post(&json!({"type":"project_quick_error","conversation":conversation,"request_id":request_id}))?;
                }
                result?;
            }
            ProjectCommand::QuickSubmit {
                conversation,
                request_id,
                start,
                end,
                notes,
            } => {
                let result = self.submit_quick(conversation, request_id, start, end, notes);
                self.view.post(&json!({"type":"project_quick_ack","conversation":conversation,"request_id":request_id,"ok":result.is_ok()}))?;
                result?;
            }
            ProjectCommand::QuickCancel {
                conversation,
                request_id,
            } => {
                if self
                    .projects
                    .quick
                    .as_ref()
                    .is_some_and(|p| p.conversation == *conversation && p.id == *request_id)
                {
                    self.projects.quick = None;
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}
