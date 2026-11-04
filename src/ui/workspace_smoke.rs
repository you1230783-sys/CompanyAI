//! --self-check使用獨立資料夾驗證原生命令，從不建立真實使用者的預設工作區。
use super::*;

impl App {
    pub(in crate::ui) fn verify_workspace_flow(&mut self) -> AppResult<()> {
        if !self.smoke {
            return Err("工作區驗證僅供self-check。".into());
        }
        let original = self.root.clone();
        self.root = original.join(format!("workspace-{}", crate::jobs::new_id()?));
        let result = self.workspace_smoke_cases();
        self.workspace_import = None;
        self.document_export.cancel();
        self.document_export = Default::default();
        self.session = None;
        self.projects = Default::default();
        self.work = Default::default();
        self.archive = Default::default();
        self.active_id = None;
        self.messages.clear();
        self.draft.clear();
        self.root = original;
        self.reminders = reminders::ReminderRuntime::load(&self.root);
        let cases = result?;
        storage::atomic_write(&self.root.join("workspace-verification.json"),&serde_json::to_vec_pretty(&json!({"version":env!("CARGO_PKG_VERSION"),"result":"PASS","cases":cases,"company_service_tested":false})).map_err(|e|e.to_string())?)
    }

    fn workspace_smoke_cases(&mut self) -> AppResult<Vec<&'static str>> {
        let check = |ok: bool, message: &str| -> AppResult<()> {
            if ok {
                Ok(())
            } else {
                Err(format!("工作區原生驗證失敗：{message}"))
            }
        };
        self.session = None;
        self.busy = "none";
        self.projects = Default::default();
        self.work = Default::default();
        self.archive = Default::default();
        self.history_error = None;
        check(
            self.workspace_action(WorkspaceCommand::Open).is_err(),
            "login gate",
        )?;
        self.session = Some(Session {
            access_token: "workspace-fixture".into(),
            expires_at: crate::unix_now() + 3600,
            binding: self.config.binding()?,
        });
        self.versions = Default::default();
        self.work.store.principal_id = "fixture-principal".into();
        let root = self.root.join("project");
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let project_id = self.projects.store.add("測試工作區", root.clone())?;
        self.projects.store.default_workspace = Some(project_id.clone());
        self.projects.store.save(&self.root)?;
        self.messages = vec![Message::user("一般對話原文")];
        let source = self.archive.insert(self.messages.clone())?;
        self.active_id = Some(source.clone());
        self.set_draft("尚未送出的要求".into());
        let opened = self.workspace_action(WorkspaceCommand::Open)?;
        let conversation = opened["conversation"]
            .as_str()
            .ok_or("缺少工作區對話。")?
            .to_owned();
        check(
            conversation != source && self.messages.is_empty() && self.draft == "尚未送出的要求",
            "only creates draft",
        )?;
        check(
            self.archive
                .conversations
                .iter()
                .any(|c| c.id == source && c.messages.len() == 1 && c.draft == "尚未送出的要求"),
            "source retained",
        )?;
        check(
            self.archive
                .conversations
                .iter()
                .any(|c| c.id == conversation && c.handoff_source.as_deref() == Some(&source)),
            "source link",
        )?;
        check(
            self.workspace_action(WorkspaceCommand::FileBegin {
                conversation: conversation.clone(),
                name: "../escape.txt".into(),
                size: 4,
            })
            .is_err(),
            "path escape",
        )?;
        let begin = self.workspace_action(WorkspaceCommand::FileBegin {
            conversation: conversation.clone(),
            name: "test.txt".into(),
            size: 4,
        })?;
        let id = begin["id"].as_str().unwrap().to_owned();
        self.workspace_action(WorkspaceCommand::FileChunk {
            id: id.clone(),
            offset: 0,
            data: "dGVzdA==".into(),
        })?;
        self.generation += 1;
        check(
            self.workspace_command(WorkspaceCommand::FileFinish { id })
                .is_err()
                && self.workspace_import.is_none(),
            "stale account cancelled",
        )?;
        let begin = self.workspace_action(WorkspaceCommand::FileBegin {
            conversation: conversation.clone(),
            name: "test.txt".into(),
            size: 4,
        })?;
        let id = begin["id"].as_str().unwrap().to_owned();
        self.workspace_action(WorkspaceCommand::FileChunk {
            id: id.clone(),
            offset: 0,
            data: "dGVzdA==".into(),
        })?;
        let saved = self.workspace_action(WorkspaceCommand::FileFinish { id: id.clone() })?;
        check(
            std::fs::read(root.join(saved["path"].as_str().unwrap())).map_err(|e| e.to_string())?
                == b"test",
            "verified bytes",
        )?;
        check(
            self.workspace_action(WorkspaceCommand::FileFinish { id })
                .is_err(),
            "duplicate finish rejected",
        )?;
        check(
            !self.projects.is_running() && self.work.store.attachments.is_empty(),
            "no model or website upload",
        )?;
        self.messages = vec![Message::assistant("待匯出回答".into())];
        self.verify_export_guards(&conversation)?;
        let project = self
            .projects
            .store
            .project_for(&conversation)
            .ok_or("專案遺失")?
            .clone();
        let memory = projects::memory::Memory::open(project, &conversation)?;
        let mut template = projects::workflows::Template {
            id: "test-flow".into(),
            name: "測試流程".into(),
            created: crate::unix_now(),
            steps: vec![projects::workflows::Step {
                id: "read".into(),
                request: json!({"tool":"read_file","path":"test.txt","offset":0}),
                bindings: Default::default(),
            }],
        };
        memory.store_workflow(&template)?;
        self.reminders = reminders::ReminderRuntime::load(&self.root);
        let save=serde_json::from_value(json!({"action":"save","id":null,"title":"流程提醒","due_at":crate::unix_now()+3600,"channel":"windows","repeat_days":1,"workflow":{"conversation":conversation,"template":template.id}})).map_err(|e|e.to_string())?;
        self.reminder_command(save)?;
        check(!self.projects.is_running(), "schedule never executes")?;
        self.work.store.principal_id = "another-principal".into();
        check(
            self.reminder_command(reminders::ReminderCommand::LoadWorkflow { id: 1 })
                .is_err(),
            "wrong principal",
        )?;
        self.work.store.principal_id = "fixture-principal".into();
        self.reminder_command(reminders::ReminderCommand::LoadWorkflow { id: 1 })?;
        check(
            !self.projects.is_running(),
            "load only displays draft parameters",
        )?;
        template.name = "changed".into();
        memory.store_workflow(&template)?;
        check(
            self.reminder_command(reminders::ReminderCommand::LoadWorkflow { id: 1 })
                .is_err(),
            "changed template",
        )?;
        Ok(vec![
            "login gate",
            "existing default workspace reused",
            "original conversation and draft retained",
            "new project draft keeps source link",
            "path escape rejected",
            "account generation cancels import",
            "encrypted chunk import/readback",
            "duplicate finish rejected",
            "no model run or website attachment",
            "native export revision/root/cancel guards",
            "workflow reminder stores identity without execution",
            "wrong principal and changed template rejected",
            "manual load only opens parameter draft",
        ])
    }
}
