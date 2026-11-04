//! 匯出預覽綁定原生歷史版本；確認後背景產生副本，不從前端接收回答正文。
use super::*;
use crate::projects::{self, files::export};

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum ExportCommand {
    Preview {
        conversation: String,
        index: Option<usize>,
        pdf: bool,
    },
    Save {
        id: String,
        name: String,
    },
    Cancel,
}
struct Pending {
    id: String,
    conversation: String,
    generation: u64,
    revision: String,
    project: projects::Project,
    identity: [u32; 3],
    pdf: bool,
    plan: export::Plan,
}
#[derive(Default)]
pub(super) struct Runtime {
    pending: Option<Pending>,
    running: Option<(String, Arc<AtomicBool>)>,
}
impl Runtime {
    pub(super) fn cancel(&mut self) {
        self.pending = None;
        if let Some((_, cancel)) = &self.running {
            cancel.store(true, Ordering::Relaxed);
        }
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.cancel();
    }
}

impl App {
    pub(super) fn verify_export_guards(&mut self, conversation: &str) -> AppResult<()> {
        if !self.smoke {
            return Err("匯出驗證僅供self-check。".into());
        }
        self.export_command(ExportCommand::Preview {
            conversation: conversation.into(),
            index: Some(0),
            pdf: false,
        })?;
        let id = self
            .document_export
            .pending
            .as_ref()
            .ok_or("預覽遺失。")?
            .id
            .clone();
        self.messages[0].content.push_str(" changed");
        if self
            .export_command(ExportCommand::Save {
                id: id.clone(),
                name: "rejected.docx".into(),
            })
            .is_ok()
        {
            return Err("匯出未拒絕過期歷史。".into());
        }
        self.export_command(ExportCommand::Cancel)?;
        if self
            .export_command(ExportCommand::Save {
                id,
                name: "rejected.docx".into(),
            })
            .is_ok()
        {
            return Err("取消後仍可匯出。".into());
        }
        self.export_command(ExportCommand::Preview {
            conversation: conversation.into(),
            index: Some(0),
            pdf: false,
        })?;
        let pending = self.document_export.pending.as_mut().ok_or("預覽遺失。")?;
        let id = pending.id.clone();
        pending.identity = [0, 0, 0];
        if self
            .export_command(ExportCommand::Save {
                id,
                name: "rejected.docx".into(),
            })
            .is_ok()
        {
            return Err("匯出未拒絕已替換根目錄。".into());
        }
        self.export_command(ExportCommand::Cancel)
    }
    fn export_revision(&self, conversation: &str) -> AppResult<String> {
        if !self.logged_in()
            || self.versions.blocked()
            || self.active_id.as_deref() != Some(conversation)
            || self.projects.is_running()
            || self.workspace_import.is_some()
            || self.work.incoming.is_some()
        {
            return Err("請在原對話等待目前工作完成後匯出。".into());
        }
        Ok(projects::text::revision(
            &serde_json::to_string(&self.messages).map_err(|e| e.to_string())?,
        ))
    }
    pub(super) fn export_command(&mut self, command: ExportCommand) -> AppResult<()> {
        let result = (|| -> AppResult<()> {
            match command {
                ExportCommand::Cancel => {
                    self.document_export.cancel();
                    Ok(())
                }
                ExportCommand::Preview {
                    conversation,
                    index,
                    pdf,
                } => {
                    if self.busy != "none" || self.document_export.running.is_some() {
                        return Err("請等待目前工作完成。".into());
                    }
                    let revision = self.export_revision(&conversation)?;
                    let messages = if let Some(index) = index {
                        let m = self
                            .messages
                            .get(index)
                            .filter(|m| m.role == "assistant")
                            .ok_or("回答已不存在。")?;
                        vec![m.clone()]
                    } else {
                        self.messages
                            .iter()
                            .filter(|m| m.role == "user" || m.role == "assistant")
                            .cloned()
                            .collect()
                    };
                    let project = match self.projects.store.project_for(&conversation) {
                        Some(p) => p.clone(),
                        None => self.default_workspace()?,
                    };
                    let identity = projects::files::directory_identity(&project.root)?;
                    let plan = export::prepare(&project, &messages)?;
                    let id = crate::jobs::new_id()?;
                    self.view.post(&json!({"type":"document_export_preview","id":id,"conversation":conversation,"messages":plan.messages,"blocks":plan.actions.len(),"warnings":plan.warnings,"folder":project.root.join("_AI_Output").join(projects::files::conversation_folder_name(&conversation)?),"pdf":pdf}))?;
                    self.document_export.pending = Some(Pending {
                        id,
                        conversation,
                        generation: self.generation,
                        revision,
                        project,
                        identity,
                        pdf,
                        plan,
                    });
                    self.publish();
                    Ok(())
                }
                ExportCommand::Save { id, name } => {
                    let pending = self
                        .document_export
                        .pending
                        .as_ref()
                        .filter(|p| p.id == id)
                        .ok_or("預覽已取消／過期，請重新匯出。")?;
                    if self.busy != "none"
                        || pending.generation != self.generation
                        || pending.revision != self.export_revision(&pending.conversation)?
                        || projects::files::directory_identity(&pending.project.root)?
                            != pending.identity
                    {
                        return Err("對話、帳號或工作區已改變，請重新預覽。".into());
                    }
                    let pending = self.document_export.pending.take().ok_or("預覽已取消。")?;
                    let cancel = Arc::new(AtomicBool::new(false));
                    self.document_export.running = Some((id.clone(), cancel.clone()));
                    self.busy = "export";
                    let tx = self.tx.clone();
                    let generation = self.generation;
                    let worker_exe = std::env::current_exe().map_err(|e| e.to_string())?;
                    crate::reliability::spawn(move || {
                        let result = export::save(
                            &pending.project,
                            &worker_exe,
                            &pending.conversation,
                            &name,
                            pending.pdf,
                            &pending.plan,
                            &cancel,
                        );
                        let _ = tx.send(Event::DocumentExport(id, generation, result));
                    });
                    self.publish();
                    Ok(())
                }
            }
        })();
        if let Err(error) = &result {
            self.view
                .post(&json!({"type":"document_export_error","message":error}))?;
        }
        result
    }
    pub(super) fn export_event(
        &mut self,
        id: String,
        generation: u64,
        result: AppResult<serde_json::Value>,
    ) -> AppResult<()> {
        if self
            .document_export
            .running
            .as_ref()
            .is_none_or(|(active, _)| active != &id)
        {
            return Ok(());
        }
        self.document_export.running = None;
        if self.busy == "export" {
            self.busy = "none";
        }
        if generation == self.generation && self.logged_in() {
            match result {
                Ok(value) => self
                    .view
                    .post(&json!({"type":"document_export_done","result":value}))?,
                Err(error) => self
                    .view
                    .post(&json!({"type":"document_export_error","message":error}))?,
            }
        }
        self.publish();
        Ok(())
    }
}
