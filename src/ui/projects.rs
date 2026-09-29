//! 專案 UI 控制器。只有使用者原生命令可新增授權，模型不能建立專案或擴大根目錄。
use super::*;
use crate::projects::{self, Store};

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub(super) enum ProjectCommand {
    Create {
        name: String,
    },
    NewChat {
        id: String,
    },
    Remove {
        id: String,
    },
    Import {
        id: String,
        path: String,
        text: String,
    },
    ClearImports {
        id: String,
    },
    Stop,
}
pub(super) enum ProjectEvent {
    Progress(String, String),
    Finished(String, String, AppResult<String>),
}
pub(super) struct Running {
    id: String,
    conversation: String,
    cancel: Arc<AtomicBool>,
}
#[derive(Default)]
pub(super) struct ProjectRuntime {
    pub store: Store,
    error: Option<String>,
    running: Option<Running>,
    status: String,
}
impl ProjectRuntime {
    pub fn load(root: &std::path::Path) -> Self {
        let mut runtime = Self::default();
        match Store::load(root) {
            Ok(store) => runtime.store = store,
            Err(error) => runtime.error = Some(error),
        }
        runtime
    }
    pub fn cancel(&mut self) {
        if let Some(run) = &self.running {
            run.cancel.store(true, Ordering::Relaxed);
            self.status = "正在停止本機操作…".into();
        }
    }
    pub fn active(&self, conversation: Option<&str>) -> bool {
        self.running
            .as_ref()
            .is_some_and(|r| Some(r.conversation.as_str()) == conversation)
    }
}
impl Drop for ProjectRuntime {
    fn drop(&mut self) {
        self.cancel();
    }
}

fn choose_folder(owner: HWND) -> AppResult<Option<PathBuf>> {
    use windows::{
        core::w,
        Win32::{
            Foundation::{ERROR_CANCELLED, HWND as WinHwnd},
            System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER},
            UI::Shell::{
                FileOpenDialog, IFileOpenDialog, FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST,
                FOS_PICKFOLDERS, SIGDN_FILESYSPATH,
            },
        },
    };
    // 主 UI 執行緒已有 COM STA；只使用 Windows 原生選擇器授權根目錄。
    unsafe {
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| e.to_string())?;
        dialog
            .SetTitle(w!("選擇專案資料夾（原檔唯讀，成果另存副本）"))
            .map_err(|e| e.to_string())?;
        dialog
            .SetOptions(FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST)
            .map_err(|e| e.to_string())?;
        if let Err(error) = dialog.Show(Some(WinHwnd(owner))) {
            if error.code() == windows::core::HRESULT::from_win32(ERROR_CANCELLED.0) {
                return Ok(None);
            }
            return Err(error.to_string());
        }
        let value = dialog
            .GetResult()
            .and_then(|item| item.GetDisplayName(SIGDN_FILESYSPATH))
            .map_err(|e| e.to_string())?;
        let path = value.to_string();
        CoTaskMemFree(Some(value.0.cast()));
        Ok(Some(PathBuf::from(path.map_err(|e| e.to_string())?)))
    }
}

impl App {
    pub(super) fn recover_project_history(&mut self) -> AppResult<()> {
        if self.history_error.is_some() || self.projects.error.is_some() {
            return Ok(());
        }
        let mut archive = self.archive.clone();
        let mut changed = false;
        for conversation in &mut archive.conversations {
            if !self
                .projects
                .store
                .conversations
                .contains_key(&conversation.id)
            {
                continue;
            }
            let Some(last) = conversation.messages.last() else {
                continue;
            };
            if last.role != "user" {
                continue;
            }
            let Some(id) = last.request_id.clone() else {
                continue;
            };
            let mut message = Message::assistant(projects::runner::recover(&self.root, &id)?);
            message.request_id = Some(id);
            conversation.messages.push(message);
            changed = true;
        }
        if changed {
            history::save(&self.root, &archive)?;
            self.archive = archive;
        }
        Ok(())
    }
    pub(super) fn project_state(&self) -> serde_json::Value {
        json!({"items":self.projects.store.projects.iter().map(|p| json!({"id":p.id,"name":p.name,"root":p.root,"import_count":p.imports.len()})).collect::<Vec<_>>(),
            "running":self.projects.running.is_some(),"running_conversation":self.projects.running.as_ref().map(|r|&r.conversation),"status":self.projects.status,"error":self.projects.error})
    }
    pub(super) fn project_command(&mut self, command: ProjectCommand) -> AppResult<()> {
        if let Some(error) = &self.projects.error {
            return Err(error.clone());
        }
        if matches!(command, ProjectCommand::Stop) {
            self.projects.cancel();
            return Ok(());
        }
        if self.projects.running.is_some() {
            return Err("請先完成或停止專案任務，再修改專案設定。".into());
        }
        match command {
            ProjectCommand::Create { name } => {
                if let Some(root) = choose_folder(self.window)? {
                    let mut store = self.projects.store.clone();
                    let id = store.add(&name, root)?;
                    store.save(&self.root)?;
                    self.projects.store = store;
                    self.new_project_chat(&id)?;
                }
            }
            ProjectCommand::NewChat { id } => self.new_project_chat(&id)?,
            ProjectCommand::Remove { id } => {
                let mut store = self.projects.store.clone();
                store.projects.retain(|p| p.id != id);
                store.conversations.retain(|_, p| p != &id);
                store.save(&self.root)?;
                self.projects.store = store;
                self.projects.status =
                    "已移除專案授權；原始文件、成果及對話保留，對話改列於最近對話。".into();
            }
            ProjectCommand::Import { id, path, text } => {
                projects::text::validate(&text)?;
                let relative = projects::files::relative(&path)?;
                let mut store = self.projects.store.clone();
                let project = store
                    .projects
                    .iter_mut()
                    .find(|p| p.id == id)
                    .ok_or("找不到專案。")?;
                // 先以無內容的暫存匯入通過檔案身分／邊界驗證，再保存真正的使用者文字。
                let key = relative.to_string_lossy().replace('\\', "/");
                project.imports.insert(key.clone(), String::new());
                projects::files::read(project, &key)?;
                project.imports.insert(key, text);
                store.save(&self.root)?;
                self.projects.store = store;
                self.projects.status =
                    "已保存使用者匯入的明文快照（DPAPI）；後續讀取此檔使用快照，原檔未修改。"
                        .into();
            }
            ProjectCommand::ClearImports { id } => {
                let mut store = self.projects.store.clone();
                store
                    .projects
                    .iter_mut()
                    .find(|p| p.id == id)
                    .ok_or("找不到專案。")?
                    .imports
                    .clear();
                store.save(&self.root)?;
                self.projects.store = store;
                self.projects.status = "已清除匯入快照，下一次重新讀取原始文件。".into();
            }
            ProjectCommand::Stop => (),
        }
        Ok(())
    }
    fn new_project_chat(&mut self, project_id: &str) -> AppResult<()> {
        if self.busy != "none" || self.work.incoming.is_some() {
            return Err("請先完成目前操作。".into());
        }
        let project = self
            .projects
            .store
            .projects
            .iter()
            .find(|p| p.id == project_id)
            .ok_or("找不到專案。")?;
        projects::files::validate_root(&project.root)?;
        self.preserve_draft()?;
        let mut archive = self.archive.clone();
        let id = archive.insert(Vec::new())?;
        let mut store = self.projects.store.clone();
        store.conversations.insert(id.clone(), project_id.into());
        // 中途寫入失敗最多留下無權限的空對話，不會給一般對話增加授權。
        history::save(&self.root, &archive)?;
        store.save(&self.root)?;
        self.archive = archive;
        self.projects.store = store;
        self.active_id = Some(id);
        self.messages.clear();
        self.set_draft(String::new());
        self.focus_draft = true;
        Ok(())
    }
    pub(super) fn begin_project_chat(&mut self, mut messages: Vec<Message>) -> AppResult<()> {
        if self.projects.running.is_some() {
            return Err("第一版一次只執行一個專案任務，請先等待或停止。".into());
        }
        let conversation = self.active_id.clone().ok_or("請先建立專案對話。")?;
        let project = self
            .projects
            .store
            .project_for(&conversation)
            .cloned()
            .ok_or("此對話沒有專案授權。")?;
        if !self.work.store.drafts(Some(&conversation)).is_empty() {
            return Err("專案文件請放在授權資料夾內；第一版不混用網站附件。".into());
        }
        let id = crate::jobs::new_id()?;
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(message) = messages.last_mut() {
            message.request_id = Some(id.clone());
        }
        self.messages = messages.clone();
        self.save_history()?;
        let run = projects::runner::Run {
            id: id.clone(),
            project,
            conversation: conversation.clone(),
            messages,
            config: self.config.clone(),
            session: self.session.clone().ok_or("請先登入。")?,
            root: self.root.clone(),
            cancel: cancel.clone(),
        };
        self.projects.running = Some(Running {
            id: id.clone(),
            conversation: conversation.clone(),
            cancel,
        });
        self.set_draft(String::new());
        self.projects.status = "正在建立受限制執行器…".into();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let result = projects::runner::run(run, |text| {
                let _ = tx.send(Event::Project(ProjectEvent::Progress(id.clone(), text)));
            });
            let _ = tx.send(Event::Project(ProjectEvent::Finished(
                id,
                conversation,
                result,
            )));
        });
        if self.messages.iter().filter(|m| m.role == "user").count() == 1 {
            if let Err(error) = self.queue_title(&self.active_id.clone().ok_or("找不到對話。")?)
            {
                self.toast(&format!("專案任務已開始；標題暫未產生：{error}"));
            }
        }
        Ok(())
    }
    pub(super) fn project_event(&mut self, event: ProjectEvent) -> AppResult<()> {
        match event {
            ProjectEvent::Progress(id, text) => {
                if self.projects.running.as_ref().is_some_and(|r| r.id == id) {
                    self.projects.status = text;
                }
            }
            ProjectEvent::Finished(id, conversation, result) => {
                if !self.projects.running.as_ref().is_some_and(|r| r.id == id) {
                    return Ok(());
                }
                self.projects.running = None;
                self.projects.status = if result.is_ok() {
                    "本次執行已結束，子程序已釋放。"
                } else {
                    "任務未完成，子程序已釋放；請查看對話說明。"
                }
                .into();
                let text = result.unwrap_or_else(|error| format!("本次任務未完成：{error}"));
                let mut archive = self.archive.clone();
                let c = archive
                    .conversations
                    .iter_mut()
                    .find(|c| c.id == conversation)
                    .ok_or("專案對話已不存在。")?;
                let mut message = Message::assistant(text);
                message.request_id = Some(id);
                c.messages.push(message);
                c.updated_at = crate::unix_now();
                history::save(&self.root, &archive)?;
                self.archive = archive;
                if self.logged_in() && self.active_id.as_deref() == Some(&conversation) {
                    self.messages = self
                        .archive
                        .conversations
                        .iter()
                        .find(|c| c.id == conversation)
                        .ok_or("找不到對話。")?
                        .messages
                        .clone();
                }
            }
        }
        Ok(())
    }
}
