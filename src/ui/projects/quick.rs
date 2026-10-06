//! 專案 Outlook 與圖片試驗入口；共用現有 runner，不另建立郵件權限或聊天引擎。
use super::*;

#[derive(Clone, Copy, PartialEq, serde::Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::ui) enum Kind {
    Outlook,
    Image,
}

pub(super) struct Pending {
    id: String,
    conversation: String,
    project: String,
    principal: String,
    model: String,
    root: PathBuf,
    messages: usize,
    kind: Kind,
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
        if kind == Kind::Image && !projects::vision::input::model_supported(&self.config.model) {
            return Err(projects::vision::input::UNSUPPORTED_MODEL.into());
        }
        self.projects.quick = Some(Pending {
            id: id.into(),
            conversation: conversation.into(),
            project: project.id,
            principal: self.work.store.principal_id.clone(),
            model: self.config.model.clone(),
            root: project.root,
            messages: self.messages.len(),
            kind,
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
        path: &str,
    ) -> AppResult<()> {
        let (pending, project) = self.quick_pending(conversation, id)?;
        if notes.chars().count() > 1000 {
            return Err("補充內容最多 1000 字。".into());
        }
        let prompt = match pending.kind {
            Kind::Outlook => {
                projects::setup::outlook_prompt(projects::setup::local_date()?, start, end, notes)?
            }
            Kind::Image => {
                // 送出前先驗證來源；提示保留此版本，辨識結果會附實際讀取版本。
                let image = projects::vision::input::load(&project, path)?;
                format!("請使用 image-read 技能的 analyze_image，辨識專案相對圖片 {:?}。本次指定來源 SHA256：{}；若實際版本不同，先告知並確認。辨識要求：{}。這是圖片傳送試驗，請明確回報工具辨識結果及來源；工具失敗就告知，不以檔名猜圖中內容。完成後正常呼叫 finish 回覆，不必建立文件。", image.path,image.sha256,
                    if notes.trim().is_empty() { "請描述這張圖片裡看到的東西，不需要辨識文字" } else { notes.trim() })
            }
        };
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
                path,
            } => {
                let result = self.submit_quick(conversation, request_id, start, end, notes, path);
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
            ProjectCommand::QuickChooseImage {
                conversation,
                request_id,
            } => {
                let (pending, project) = self.quick_pending(conversation, request_id)?;
                if pending.kind != Kind::Image {
                    return Err("此視窗不接受圖片選擇。".into());
                }
                if let Some(path) = choose_path(self.window, Some(&project.root), true)? {
                    self.quick_pending(conversation, request_id)?;
                    let relative = path
                        .strip_prefix(&project.root)
                        .map_err(|_| "請先把圖片放到目前專案資料夾，再選擇圖片。")?;
                    let image =
                        projects::vision::input::load(&project, &relative.to_string_lossy())?;
                    self.view.post(&json!({"type":"project_quick_image","conversation":conversation,"request_id":request_id,"path":image.path}))?;
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}
