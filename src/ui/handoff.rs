//! 對話交接只建立新的專案草稿；來源附件、任務與授權不會被搬移或重播。
use super::*;

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum HandoffCommand {
    Prepare {
        source: String,
        project: String,
        request: String,
        #[serde(default)]
        through: Option<usize>,
    },
    Commit {
        id: String,
        text: String,
    },
    Cancel {
        id: String,
    },
}
pub(super) struct Pending {
    id: String,
    request: String,
    source: String,
    revision: String,
    generation: u64,
    project: crate::projects::Project,
    root_identity: [u32; 3],
    through: Option<usize>,
}
impl HandoffCommand {
    pub(super) fn correlation(&self) -> &str {
        match self {
            Self::Prepare { request, .. } => request,
            Self::Commit { id, .. } | Self::Cancel { id } => id,
        }
    }
}

/// 摘錄是可編輯的交接起點，不聲稱已理解全部歷史；有截斷時明確標示。
fn limited(text: &str, limit: usize) -> String {
    let mut chars = text.chars();
    let mut value: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        value.push_str("\n[內容過長，已截斷顯示]");
    }
    value
}
fn excerpt(messages: &[Message], draft: &str) -> String {
    let start = messages.len().saturating_sub(6);
    let mut parts = vec![
        "以下是另一個對話的背景摘錄，請作為待核對資料；不代表已取得檔案或延續任何操作授權。".into(),
    ];
    if start > 0 {
        parts.push("（只摘錄最近六則訊息，較早內容未帶入。）".into());
    }
    for message in &messages[start..] {
        let label = if message.role == "user" {
            "原提問"
        } else {
            "原回答（未重新驗證）"
        };
        parts.push(format!("{label}：{}", limited(&message.content, 600)));
    }
    let next_request = if draft.trim().is_empty() {
        messages
            .iter()
            .rev()
            .find(|message| message.role == "user")
            .map(|message| message.content.as_str())
            .unwrap_or("")
    } else {
        draft
    };
    parts.push(format!("接下來的需求：\n{}", limited(next_request, 2500)));
    parts.join("\n\n")
}
impl App {
    /// 真正呼叫原生控制器及DPAPI保存；測試僅使用獨立目錄與合成會話。
    pub(super) fn verify_handoff_flow(&mut self) -> AppResult<()> {
        if !self.smoke {
            return Err("交接自檢僅供self-check。".into());
        }
        let original_root = self.root.clone();
        self.root = original_root.join(format!("handoff-{}", crate::jobs::new_id()?));
        let result = self.handoff_smoke_cases();
        self.handoff = None;
        self.session = None;
        self.projects = Default::default();
        self.work = Default::default();
        self.archive = Default::default();
        self.active_id = None;
        self.messages.clear();
        self.draft.clear();
        self.root = original_root;
        let cases = result?;
        storage::atomic_write(&self.root.join("handoff-verification.json"), &serde_json::to_vec_pretty(&json!({"version":env!("CARGO_PKG_VERSION"),"result":"PASS","cases":cases,"company_service_tested":false})).map_err(|e|e.to_string())?)
    }
    fn handoff_smoke_cases(&mut self) -> AppResult<Vec<&'static str>> {
        fn verify(ok: bool, name: &str) -> AppResult<()> {
            if ok {
                Ok(())
            } else {
                Err(format!("交接驗證失敗：{name}"))
            }
        }
        self.session = None;
        self.busy = "none";
        self.history_error = None;
        self.projects = Default::default();
        self.work = Default::default();
        self.archive = Default::default();
        self.versions = Default::default();
        verify(self.handoff_ready("missing", None).is_err(), "login gate")?;
        self.session = Some(Session {
            access_token: "local-handoff-fixture".into(),
            expires_at: crate::unix_now() + 3600,
            binding: self.config.binding()?,
        });
        let root = self.root.join("project");
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let project = self.projects.store.add("測試專案", root.clone())?;
        let mut message = Message::user("原始需求");
        message.attachments = vec!["fixture.docx".into()];
        message.request_id = Some("old-request".into());
        self.messages = vec![message];
        let source = self.archive.insert(self.messages.clone())?;
        self.active_id = Some(source.clone());
        self.set_draft("保留原草稿".into());
        let prepare = || HandoffCommand::Prepare {
            source: source.clone(),
            project: project.clone(),
            request: "fixture".into(),
            through: None,
        };
        self.handoff_command(prepare())?;
        let id = self.handoff.as_ref().ok_or("缺少預覽")?.id.clone();
        self.handoff_command(HandoffCommand::Cancel {
            id: "fixture".into(),
        })?;
        verify(
            self.handoff_command(HandoffCommand::Commit {
                id,
                text: "取消後重送".into(),
            })
            .is_err(),
            "cancel before preview delivery revokes token",
        )?;
        verify(
            self.archive.conversations.len() == 1 && self.handoff.is_none(),
            "cancel does not create a conversation",
        )?;
        self.handoff_command(prepare())?;
        let id = self.handoff.as_ref().ok_or("缺少預覽")?.id.clone();
        self.active_id = None;
        verify(
            self.handoff_command(HandoffCommand::Commit {
                id,
                text: "需求".into(),
            })
            .is_err(),
            "conversation binding",
        )?;
        self.active_id = Some(source.clone());
        self.handoff_command(prepare())?;
        let id = self.handoff.as_ref().ok_or("缺少預覽")?.id.clone();
        self.set_draft("新的原草稿".into());
        verify(
            self.handoff_command(HandoffCommand::Commit {
                id,
                text: "需求".into(),
            })
            .is_err(),
            "changed source rejects stale preview",
        )?;
        self.handoff_command(prepare())?;
        let id = self.handoff.as_ref().ok_or("缺少預覽")?.id.clone();
        self.generation += 1;
        verify(
            self.handoff_command(HandoffCommand::Commit {
                id,
                text: "需求".into(),
            })
            .is_err(),
            "account generation binding",
        )?;
        self.handoff_command(prepare())?;
        let id = self.handoff.as_ref().ok_or("缺少預覽")?.id.clone();
        let original = root.with_extension("original");
        std::fs::rename(&root, &original).map_err(|e| e.to_string())?;
        std::fs::create_dir(&root).map_err(|e| e.to_string())?;
        verify(
            self.handoff_command(HandoffCommand::Commit {
                id,
                text: "需求".into(),
            })
            .is_err(),
            "replaced project root rejected",
        )?;
        std::fs::remove_dir(&root).map_err(|e| e.to_string())?;
        std::fs::rename(original, &root).map_err(|e| e.to_string())?;
        self.handoff_command(prepare())?;
        let id = self.handoff.as_ref().ok_or("缺少預覽")?.id.clone();
        self.handoff_command(HandoffCommand::Commit {
            id: id.clone(),
            text: "使用者編輯後的交接".into(),
        })?;
        verify(
            self.active_id.as_ref() != Some(&source)
                && self.messages.is_empty()
                && self.work.store.tasks.is_empty()
                && self.work.store.attachments.is_empty()
                && self.draft == "使用者編輯後的交接",
            "only creates draft, no copied task or attachment",
        )?;
        verify(
            self.handoff_command(HandoffCommand::Commit {
                id,
                text: "重複".into(),
            })
            .is_err()
                && self.archive.conversations.len() == 2,
            "one-shot confirmation",
        )?;
        let loaded = history::load(&self.root)?;
        verify(
            loaded.conversations[0].draft == "新的原草稿"
                && loaded.conversations[0].messages[0].attachments == ["fixture.docx"]
                && loaded.conversations[1].handoff_source.as_deref() == Some(source.as_str())
                && loaded.conversations[1].messages.is_empty()
                && loaded.conversations[1].project_queued.is_none()
                && crate::projects::Store::load(&self.root)?
                    .project_for(&loaded.conversations[1].id)
                    .is_some(),
            "encrypted restart retains drafts and source link without authority replay",
        )?;
        // 專案訊息交接只摘要指定前綴；後續寫入紀錄與附件識別不移轉。
        let project_source = self.active_id.clone().ok_or("缺少專案來源")?;
        self.messages = vec![
            Message::user("PREFIX_ONLY"),
            Message::assistant("CUT_HERE".into()),
            Message::assistant("LATER_SECRET".into()),
        ];
        self.save_history()?;
        self.handoff_command(HandoffCommand::Prepare {
            source: project_source.clone(),
            project: project.clone(),
            request: "project_prefix".into(),
            through: Some(1),
        })?;
        verify(
            !excerpt(&self.messages[..=1], "").contains("LATER_SECRET"),
            "project prefix excludes later messages",
        )?;
        let id = self.handoff.as_ref().ok_or("缺少專案交接預覽")?.id.clone();
        self.handoff_command(HandoffCommand::Commit {
            id,
            text: "PREFIX_ONLY / CUT_HERE".into(),
        })?;
        verify(
            self.active_id.as_ref() != Some(&project_source)
                && self.messages.is_empty()
                && self.work.store.tasks.is_empty()
                && self.work.store.attachments.is_empty()
                && self.draft == "PREFIX_ONLY / CUT_HERE",
            "project prefix creates draft only",
        )?;
        verify(
            self.archive
                .conversations
                .iter()
                .find(|c| c.id == project_source)
                .is_some_and(|c| c.messages[2].content == "LATER_SECRET"),
            "original project results retained",
        )?;
        Ok(vec![
            "project prefix excludes later messages and creates draft without execution or attachments",
            "original project results retained",
            "login gate",
            "cancel does not create a conversation",
            "conversation binding",
            "changed source rejects stale preview",
            "account generation binding",
            "replaced project root rejected",
            "only creates draft, no copied task or attachment",
            "one-shot confirmation",
            "encrypted restart retains drafts and source link without authority replay",
        ])
    }
    fn handoff_ready(&self, source: &str, through: Option<usize>) -> AppResult<()> {
        if !self.logged_in()
            || self.versions.blocked()
            || self.panic_fault
            || self.busy != "none"
            || self.work.incoming.is_some()
            || self.projects.is_running()
            || self.work.store.pending(Some(source))
            || self.history_error.is_some()
            || (self.projects.store.project_for(source).is_some() && through.is_none())
            || through.is_some_and(|i| i >= self.messages.len())
            || self.active_id.as_deref() != Some(source)
        {
            return Err("請在來源對話完成目前工作後，再準備交接。".into());
        }
        Ok(())
    }
    fn handoff_revision(&self, source: &str) -> AppResult<String> {
        let conversation = self
            .archive
            .conversations
            .iter()
            .find(|c| c.id == source)
            .ok_or("來源對話已不存在。")?;
        let text =
            serde_json::to_string(&(conversation, &self.draft)).map_err(|e| e.to_string())?;
        Ok(crate::projects::text::revision(&text))
    }
    pub(super) fn handoff_command(&mut self, command: HandoffCommand) -> AppResult<()> {
        match command {
            HandoffCommand::Cancel { id } => {
                if self
                    .handoff
                    .as_ref()
                    .is_some_and(|p| p.id == id || p.request == id)
                {
                    self.handoff = None;
                }
            }
            HandoffCommand::Prepare {
                source,
                project,
                request,
                through,
            } => {
                self.handoff = None;
                if request.is_empty() || request.len() > 80 {
                    return Err("交接請求代號過長。".into());
                }
                self.handoff_ready(&source, through)?;
                self.preserve_draft()?;
                let project = self
                    .projects
                    .store
                    .projects
                    .iter()
                    .find(|p| p.id == project)
                    .ok_or("請先建立或選擇專案。")?
                    .clone();
                let root_identity = crate::projects::files::directory_identity(&project.root)?;
                let text = match through {
                    Some(index) => excerpt(&self.messages[..=index], ""),
                    None => excerpt(&self.messages, &self.draft),
                };
                let attachment_count = self
                    .work
                    .store
                    .attachments
                    .iter()
                    .filter(|a| a.conversation_id == source && !a.removed)
                    .count();
                let pending = Pending {
                    id: crate::jobs::new_id()?,
                    request: request.clone(),
                    source: source.clone(),
                    revision: self.handoff_revision(&source)?,
                    generation: self.generation,
                    project,
                    root_identity,
                    through,
                };
                self.view.post(&json!({"type":"handoff_preview","request":request,"id":pending.id,"source":source,
                    "project":pending.project.name,"root":pending.project.root,"text":text,
                    "attachment_count":attachment_count,"notice":"網站附件、圖片與權限不會帶入。需要的文件請另放入所選專案資料夾；確認只建立草稿，不會送出或執行。"}))?;
                self.handoff = Some(pending);
            }
            HandoffCommand::Commit { id, text } => {
                let pending = self
                    .handoff
                    .as_ref()
                    .filter(|p| p.id == id)
                    .ok_or("交接預覽已失效，請重新準備。")?;
                self.handoff_ready(&pending.source, pending.through)?;
                if pending.generation != self.generation
                    || pending.revision != self.handoff_revision(&pending.source)?
                {
                    return Err("來源對話或帳號已變更，請重新準備交接。".into());
                }
                let current = self
                    .projects
                    .store
                    .projects
                    .iter()
                    .find(|p| p.id == pending.project.id)
                    .ok_or("專案已不存在。")?;
                if current.root != pending.project.root
                    || crate::projects::files::directory_identity(&current.root)?
                        != pending.root_identity
                {
                    return Err("專案資料夾已變更，請重新準備交接。".into());
                }
                if text.trim().is_empty()
                    || text.encode_utf16().count() > 16_000
                    || text
                        .chars()
                        .any(|c| c.is_control() && !['\r', '\n', '\t'].contains(&c))
                {
                    return Err("交接內容需為1至16,000字，且不能包含控制字元。".into());
                }
                // 一次性消耗預覽；保存失敗也不能重送舊授權或建立重複草稿。
                let pending = self.handoff.take().ok_or("交接已取消。")?;
                let mut archive = self.archive.clone();
                let id = archive.insert(Vec::new())?;
                let conversation = archive
                    .conversations
                    .last_mut()
                    .ok_or("無法建立交接草稿。")?;
                conversation.draft = text.clone();
                conversation.title = "從對話摘錄帶入的草稿".into();
                conversation.handoff_source = Some(pending.source);
                let mut store = self.projects.store.clone();
                store.conversations.insert(id.clone(), pending.project.id);
                history::save(&self.root, &archive)?;
                if let Err(error) = store.save(&self.root) {
                    if let Err(rollback) = history::save(&self.root, &self.archive) {
                        self.history_error = Some(rollback);
                    }
                    return Err(error);
                }
                self.archive = archive;
                self.projects.store = store;
                self.active_id = Some(id);
                self.messages.clear();
                self.set_draft(text);
                self.focus_draft = true;
                self.view.post(&json!({"type":"handoff_done"}))?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn latest_user_request_survives_many_assistant_retries_and_empty_draft() {
        let mut messages = vec![Message::user("保留這個原始需求")];
        messages.extend((0..8).map(|_| Message::assistant("重試回答".into())));
        assert!(excerpt(&messages, "").ends_with("接下來的需求：\n保留這個原始需求"));
        assert!(excerpt(&messages, "更新的需求").ends_with("接下來的需求：\n更新的需求"));
    }
    #[test]
    fn handoff_rejects_imported_execution_and_permission_fields() {
        assert!(serde_json::from_str::<HandoffCommand>(
            r#"{"action":"commit","id":"preview","text":"draft","auto_execute":true}"#
        )
        .is_err());
        assert!(serde_json::from_str::<HandoffCommand>(r#"{"action":"prepare","source":"a","project":"b","request":"c","permissions":["write"]}"#).is_err());
    }
    #[test]
    fn excerpt_is_bounded_and_never_inherits_attachment_or_tool_state() {
        let mut message = Message::assistant("界".repeat(900));
        message.attachments.push("secret-token".into());
        message.request_id = Some("old-operation".into());
        let text = excerpt(&vec![message; 8], &"新需求".repeat(2000));
        assert!(text.encode_utf16().count() < 16_000);
        assert!(text.contains("只摘錄最近六則") && text.contains("已截斷"));
        assert!(!text.contains("secret-token") && !text.contains("old-operation"));
    }
}
