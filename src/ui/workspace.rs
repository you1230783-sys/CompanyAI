//! 預設工作區只在使用者選擇本機處理時建立；檔案逐塊加密暫存、驗證後複製。
use super::*;
use crate::{attachments, projects};
use std::io::{Read, Write};
#[path = "workspace_smoke.rs"]
mod smoke;

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum WorkspaceCommand {
    Open,
    OpenDefault,
    FileBegin {
        conversation: String,
        name: String,
        size: u64,
    },
    FileChunk {
        id: String,
        offset: u64,
        data: String,
    },
    FileFinish {
        id: String,
    },
    FileAbort {
        id: String,
    },
}

pub(super) struct Pending {
    incoming: attachments::Incoming,
    conversation: String,
    generation: u64,
    project: projects::Project,
    identity: [u32; 3],
    name: String,
}

impl App {
    pub(super) fn default_workspace(&mut self) -> AppResult<projects::Project> {
        if let Some(id) = &self.projects.store.default_workspace {
            let project = self
                .projects
                .store
                .projects
                .iter()
                .find(|p| &p.id == id)
                .ok_or("預設工作區已移除，請重新建立。")?
                .clone();
            projects::files::validate_root(&project.root)?;
            return Ok(project);
        }
        let mut store = self.projects.store.clone();
        let root = projects::setup::create_default("profile")?;
        let id = store.add("我的工作區", root)?;
        store.default_workspace = Some(id.clone());
        let project = store
            .projects
            .iter()
            .find(|p| p.id == id)
            .ok_or("未建立工作區。")?
            .clone();
        store.save(&self.root)?;
        self.projects.store = store;
        Ok(project)
    }
    pub(super) fn workspace_command(&mut self, command: WorkspaceCommand) -> AppResult<()> {
        let result = self.workspace_action(command);
        match result {
            Ok(data) => self.view.post(&data),
            Err(error) => {
                self.workspace_import = None;
                self.view
                    .post(&json!({"type":"file_error","message":error}))?;
                Err(error)
            }
        }
    }

    fn workspace_ready(&self) -> AppResult<()> {
        if !self.logged_in()
            || self.versions.blocked()
            || self.busy != "none"
            || self.projects.is_running()
            || self.work.incoming.is_some()
            || self.history_error.is_some()
            || self.mail_flow.phase != "idle"
        {
            return Err("請先登入並完成目前操作，再將檔案放到工作區。".into());
        }
        Ok(())
    }

    fn workspace_action(&mut self, command: WorkspaceCommand) -> AppResult<serde_json::Value> {
        self.workspace_ready()?;
        if let Some(pending) = &self.workspace_import {
            if pending.generation != self.generation
                || self.active_id.as_deref() != Some(&pending.conversation)
                || projects::files::directory_identity(&pending.project.root)? != pending.identity
            {
                return Err("對話、登入或工作區位置已改變，匯入已停止。".into());
            }
        }
        let force_default = matches!(&command, WorkspaceCommand::OpenDefault);
        match command {
            WorkspaceCommand::Open | WorkspaceCommand::OpenDefault => {
                if self.workspace_import.is_some() {
                    return Err("上一個檔案仍在接收。".into());
                }
                // 已在專案時使用目前授權根目錄，不擅自搬移到另一個專案。
                if force_default
                    || self
                        .active_id
                        .as_deref()
                        .and_then(|id| self.projects.store.project_for(id))
                        .is_none()
                {
                    let id = self.default_workspace()?.id;
                    let source = self.active_id.clone();
                    let draft = self.draft.clone();
                    self.new_project_chat(&id)?;
                    self.set_draft(draft);
                    if let Some(conversation) = self
                        .archive
                        .conversations
                        .iter_mut()
                        .find(|c| Some(&c.id) == self.active_id.as_ref())
                    {
                        conversation.handoff_source = source;
                    }
                    self.preserve_draft()?;
                }
                self.publish();
                Ok(json!({"type":"file_ack","conversation":self.active_id,"local_workspace":true}))
            }
            WorkspaceCommand::FileBegin {
                conversation,
                name,
                size,
            } => {
                if self.workspace_import.is_some()
                    || self.active_id.as_deref() != Some(&conversation)
                    || size == 0
                    || size > 50_000_000
                {
                    return Err("請在目前工作區匯入1–50MB的檔案，並等待上一個檔案完成。".into());
                }
                let relative = projects::files::relative(&name)?;
                if relative.components().count() != 1 || name.len() > 180 {
                    return Err("匯入只接受單一檔名，最長180 bytes。".into());
                }
                let extension = relative
                    .extension()
                    .and_then(|v| v.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if ![
                    "doc", "docx", "docm", "xls", "xlsx", "xlsm", "xlsb", "ppt", "pptx", "pptm",
                    "pdf", "txt", "md", "csv", "json", "log", "png", "jpg", "jpeg", "bmp", "tif",
                    "tiff", "gif", "py",
                ]
                .contains(&extension.as_str())
                {
                    return Err(
                        "本機匯入目前只接受Office、PDF、文字、資料、Python與常用圖片格式。".into(),
                    );
                }
                let project = self
                    .projects
                    .store
                    .project_for(&conversation)
                    .ok_or("目前不是工作區對話。")?
                    .clone();
                let identity = projects::files::directory_identity(&project.root)?;
                let id = crate::jobs::new_id()?;
                let incoming = attachments::Incoming::new(&self.root, id.clone(), size)?;
                self.workspace_import = Some(Pending {
                    incoming,
                    conversation,
                    generation: self.generation,
                    project,
                    identity,
                    name,
                });
                Ok(json!({"type":"file_ack","id":id,"offset":0}))
            }
            WorkspaceCommand::FileChunk { id, offset, data } => {
                let pending = self
                    .workspace_import
                    .as_mut()
                    .filter(|p| p.incoming.id == id)
                    .ok_or("匯入已結束。")?;
                pending.incoming.append(offset, &data)?;
                Ok(json!({"type":"file_ack","id":id,"offset":pending.incoming.received}))
            }
            WorkspaceCommand::FileAbort { id } => {
                if self
                    .workspace_import
                    .as_ref()
                    .is_some_and(|p| p.incoming.id == id)
                {
                    self.workspace_import = None;
                }
                Ok(json!({"type":"file_ack","id":id,"cancelled":true}))
            }
            WorkspaceCommand::FileFinish { id } => {
                if self
                    .workspace_import
                    .as_ref()
                    .is_none_or(|p| p.incoming.id != id)
                {
                    return Err("匯入已結束。".into());
                }
                let pending = self.workspace_import.take().ok_or("匯入已結束。")?;
                let expected = pending.incoming.expected;
                pending.incoming.finish()?;
                let copied = copy_spool(
                    &self.root,
                    &id,
                    expected,
                    &pending.project,
                    &pending.name,
                    pending.identity,
                );
                let _ = std::fs::remove_file(attachments::spool_path(&self.root, &id)?);
                let path = copied?;
                self.set_draft(format!(
                    "{}{}已匯入本機檔案：{}\n請依我的需求處理，原檔保持不變。",
                    self.draft,
                    if self.draft.is_empty() { "" } else { "\n" },
                    serde_json::to_string(&path).map_err(|e| e.to_string())?
                ));
                self.preserve_draft()?;
                self.publish();
                Ok(
                    json!({"type":"file_ack","id":id,"finished":true,"path":path,"local_workspace":true}),
                )
            }
        }
    }
}

/// 輸出目錄由原生ID產生且create_new；內容逐塊雜湊並重新讀回，不執行匯入檔。
fn copy_spool(
    root: &Path,
    id: &str,
    expected: u64,
    project: &projects::Project,
    name: &str,
    identity: [u32; 3],
) -> AppResult<String> {
    use sha2::{Digest, Sha256};
    crate::jobs::validate_id(id)?;
    if projects::files::relative(name)?.components().count() != 1
        || expected == 0
        || expected > 50_000_000
    {
        return Err("匯入檔名或大小不合法。".into());
    }
    let _root = projects::files::pin(&project.root)?;
    if projects::files::directory_identity(&project.root)? != identity {
        return Err("工作區已被替換，未匯入。".into());
    }
    let folder = project.root.join(format!("匯入_{id}"));
    std::fs::create_dir(&folder).map_err(|e| e.to_string())?;
    let _pins = projects::files::pin(&folder)?;
    let target = folder.join(name);
    let result = (|| -> AppResult<()> {
        let mut output = std::fs::File::options()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|e| e.to_string())?;
        let mut input = attachments::SpoolReader::open(root, id)?;
        let mut hash = Sha256::new();
        let mut count = 0u64;
        let mut buffer = [0u8; 65536];
        loop {
            let n = input.read(&mut buffer).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            count += n as u64;
            if count > expected {
                return Err("匯入檔案大小超出預期。".into());
            }
            hash.update(&buffer[..n]);
            output.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
        }
        if count != expected {
            return Err("匯入檔案不完整。".into());
        }
        output.sync_all().map_err(|e| e.to_string())?;
        drop(output);
        let relative = format!("匯入_{id}/{name}");
        if projects::files::fingerprint(project, &relative)? != format!("{:x}", hash.finalize()) {
            return Err("本機副本讀回不符，未完成匯入。".into());
        }
        Ok(())
    })();
    if let Err(error) = result {
        let _ = std::fs::remove_file(&target);
        return Err(error);
    }
    Ok(format!("匯入_{id}/{name}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn import_verifies_bytes_root_identity_and_does_not_overwrite() {
        let root =
            std::env::temp_dir().join(format!("workspace-{}", crate::jobs::new_id().unwrap()));
        std::fs::create_dir(&root).unwrap();
        let project = projects::Project {
            id: "project".into(),
            name: "test".into(),
            root: root.clone(),
            imports: Default::default(),
        };
        let identity = projects::files::directory_identity(&root).unwrap();
        let bytes = "原始😀文件".as_bytes();
        let id = crate::jobs::new_id().unwrap();
        let mut incoming =
            attachments::Incoming::new(&root, id.clone(), bytes.len() as u64).unwrap();
        incoming.append_bytes(bytes).unwrap();
        incoming.finish().unwrap();
        assert!(copy_spool(
            &root,
            &id,
            bytes.len() as u64,
            &project,
            "../escape.txt",
            identity
        )
        .is_err());
        assert!(copy_spool(
            &root,
            &id,
            bytes.len() as u64,
            &project,
            "file.txt",
            [0, 0, 0]
        )
        .is_err());
        let path = copy_spool(
            &root,
            &id,
            bytes.len() as u64,
            &project,
            "file.txt",
            identity,
        )
        .unwrap();
        assert_eq!(std::fs::read(root.join(&path)).unwrap(), bytes);
        assert!(copy_spool(
            &root,
            &id,
            bytes.len() as u64,
            &project,
            "file.txt",
            identity
        )
        .is_err());
        assert_eq!(std::fs::read(root.join(&path)).unwrap(), bytes);
    }
}
