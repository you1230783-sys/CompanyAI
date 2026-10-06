//! 週報精靈只建立素材目錄；最後確認後才進入既有專案 runner。
use super::*;

pub(super) struct PendingWeekly {
    id: String,
    conversation: String,
    project_id: String,
    principal_id: String,
    folder: PathBuf,
    root: PathBuf,
    message_count: usize,
}

impl App {
    /// 檢查當前帳號與對話；不能將另一個專案開啟的舊精靈送到新對話。
    fn weekly_project(&self, conversation: &str) -> AppResult<projects::Project> {
        if self.active_id.as_deref() != Some(conversation)
            || !self.can_send()
            || self.projects.running.is_some()
            || self.work.incoming.is_some()
            || self.work.store.principal_id.is_empty()
        {
            return Err("請在原專案對話登入並等待目前操作結束。".into());
        }
        self.projects
            .store
            .project_for(conversation)
            .cloned()
            .ok_or("找不到目前專案。".into())
    }

    fn prepare_weekly(&mut self, conversation: &str, request_id: &str) -> AppResult<()> {
        crate::jobs::validate_id(request_id)?;
        let project = self.weekly_project(conversation)?;
        // 同一請求重播不重建素材目錄。
        if !self
            .projects
            .weekly
            .as_ref()
            .is_some_and(|p| p.id == request_id && p.conversation == conversation)
        {
            let folder = projects::setup::create_weekly(&project)?;
            self.projects.weekly = Some(PendingWeekly {
                id: request_id.into(),
                conversation: conversation.into(),
                project_id: project.id,
                principal_id: self.work.store.principal_id.clone(),
                root: project.root,
                folder,
                message_count: self.messages.len(),
            });
        }
        let pending = self.weekly_pending(conversation, request_id)?;
        let today = projects::setup::local_date()?;
        self.view
            .post(&json!({"type":"weekly_ready", "conversation":conversation,
            "request_id":request_id, "path":pending.folder.to_string_lossy(),
            "start":projects::setup::monday(today).to_string(), "end":today.to_string()}))
    }

    fn weekly_pending(&self, conversation: &str, request_id: &str) -> AppResult<&PendingWeekly> {
        let project = self.weekly_project(conversation)?;
        self.projects
            .weekly
            .as_ref()
            .filter(|p| {
                p.id == request_id
                    && p.conversation == conversation
                    && p.project_id == project.id
                    && p.root == project.root
                    && p.principal_id == self.work.store.principal_id
                    && p.message_count == self.messages.len()
            })
            .ok_or("週報準備視窗已失效，請重新按生成週報。".into())
    }

    fn submit_weekly(
        &mut self,
        conversation: &str,
        request_id: &str,
        start: &str,
        end: &str,
        notes: &str,
        confirmed: bool,
    ) -> AppResult<()> {
        if !confirmed {
            return Err("請先確認已放入週報參考資料。".into());
        }
        let pending = self.weekly_pending(conversation, request_id)?;
        projects::files::validate_root(&pending.folder)?;
        let relative = pending
            .folder
            .strip_prefix(&pending.root)
            .map_err(|_| "週報素材已不在專案內。")?;
        let prompt = projects::setup::weekly_prompt(
            &relative.to_string_lossy(),
            projects::setup::local_date()?,
            start,
            end,
            notes,
        )?;
        let mut messages = self.messages.clone();
        messages.push(Message::user(&prompt));
        self.begin_project_chat(messages)?;
        self.projects.weekly = None;
        Ok(())
    }

    /// 所有週報 UI 回覆皆带請求 ID；即使失敗也回覆，讓視窗恢復可操作。
    pub(super) fn weekly_command(&mut self, command: &ProjectCommand) -> AppResult<bool> {
        match command {
            ProjectCommand::WeeklyPrepare {
                conversation,
                request_id,
            } => {
                let result = self.prepare_weekly(conversation, request_id);
                if let Err(error) = &result {
                    self.view.post(&json!({"type":"weekly_error","conversation":conversation,"request_id":request_id,"error":error}))?;
                }
                result?;
            }
            ProjectCommand::WeeklySubmit {
                conversation,
                request_id,
                start,
                end,
                notes,
                confirmed,
            } => {
                let result =
                    self.submit_weekly(conversation, request_id, start, end, notes, *confirmed);
                self.view.post(&json!({"type":"weekly_submit_ack","conversation":conversation,"request_id":request_id,"ok":result.is_ok()}))?;
                result?;
            }
            ProjectCommand::WeeklyCancel {
                conversation,
                request_id,
            } => {
                if self
                    .projects
                    .weekly
                    .as_ref()
                    .is_some_and(|p| p.id == *request_id && p.conversation == *conversation)
                {
                    // 取消只移除精靈狀態，不刪除使用者已放入的資料。
                    self.projects.weekly = None;
                }
            }
            ProjectCommand::WeeklyOpen {
                conversation,
                request_id,
            } => {
                let pending = self.weekly_pending(conversation, request_id)?;
                projects::files::validate_root(&pending.folder)?;
                // 固定 Shell 的 open 動作；路徑只能來自原生保存的素材目錄。
                let folder = crate::wide(&pending.folder.to_string_lossy());
                let result = unsafe {
                    windows_sys::Win32::UI::Shell::ShellExecuteW(
                        self.window,
                        crate::wide("open").as_ptr(),
                        folder.as_ptr(),
                        std::ptr::null(),
                        std::ptr::null(),
                        1,
                    )
                };
                if result as usize <= 32 {
                    return Err("無法開啟素材資料夾，請確認網路連線。".into());
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}
