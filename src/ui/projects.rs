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
        delete_chats: bool,
    },
    Rename {
        id: String,
        name: String,
    },
    ChooseImportFile {
        id: String,
        request_id: u64,
    },
    Import {
        id: String,
        path: String,
        text: String,
    },
    ClearImports {
        id: String,
    },
    Reveal {
        conversation: String,
        path: String,
    },
    Resume {
        conversation: String,
        run_id: String,
        message_count: usize,
    },
    ChartChoice {
        run_id: String,
        request_id: String,
        choices: Option<Vec<projects::charts::quality::Choice>>,
    },
    Stop,
    Diagnostics {
        conversation: String,
        run_id: String,
    },
}
pub(super) enum ProjectEvent {
    ReviewChart(
        String,
        String,
        projects::charts::quality::Review,
        mpsc::Sender<Option<Vec<projects::charts::quality::Choice>>>,
    ),
    Progress(String, String),
    Charts(String, Vec<projects::charts::Chart>),
    ExportPng(
        String,
        projects::charts::Chart,
        mpsc::Sender<AppResult<String>>,
    ),
    Diagnostics(String, String, AppResult<String>),
    Finished(String, String, AppResult<String>),
}
struct PendingChart {
    id: String,
    review: projects::charts::quality::Review,
    reply: mpsc::Sender<Option<Vec<projects::charts::quality::Choice>>>,
}
pub(super) struct Running {
    pending_chart: Option<PendingChart>,
    id: String,
    conversation: String,
    activity: Vec<String>,
    charts: Vec<projects::charts::Chart>,
    started: u64,
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

/// 共用原生選擇器；選文件時從專案根目錄開始，回傳後仍驗證授權邊界。
fn choose_path(owner: HWND, project_root: Option<&std::path::Path>) -> AppResult<Option<PathBuf>> {
    use windows::{
        core::{w, HSTRING},
        Win32::{
            Foundation::{ERROR_CANCELLED, HWND as WinHwnd},
            System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER},
            UI::Shell::{
                Common::COMDLG_FILTERSPEC, FileOpenDialog, IFileOpenDialog, IShellItem,
                SHCreateItemFromParsingName, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM,
                FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, SIGDN_FILESYSPATH,
            },
        },
    };
    // 主 UI 執行緒已有 COM STA；只使用 Windows 原生選擇器授權根目錄。
    unsafe {
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| e.to_string())?;
        let options = if let Some(root) = project_root {
            dialog
                .SetTitle(w!("選擇專案文件"))
                .map_err(|e| e.to_string())?;
            dialog
                .SetFileTypes(&[COMDLG_FILTERSPEC {
                    pszName: w!("文字匯入來源（TXT、MD、PDF、MSG）"),
                    pszSpec: w!("*.txt;*.md;*.pdf;*.msg"),
                }])
                .map_err(|e| e.to_string())?;
            let folder: IShellItem =
                SHCreateItemFromParsingName(&HSTRING::from(root.as_os_str()), None)
                    .map_err(|e| e.to_string())?;
            dialog.SetFolder(&folder).map_err(|e| e.to_string())?;
            FOS_FILEMUSTEXIST | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST
        } else {
            dialog
                .SetTitle(w!("選擇專案資料夾"))
                .map_err(|e| e.to_string())?;
            FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST
        };
        dialog.SetOptions(options).map_err(|e| e.to_string())?;
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
            message.project_paused = projects::runner::paused_available(&self.root, &id);
            message.project_activity =
                projects::runner::recover_activity(&self.root, &id).unwrap_or_default();
            message.project_charts = projects::runner::recover_charts(&self.root, &id);
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
    pub(super) fn project_task(&self) -> Option<serde_json::Value> {
        self.projects.running.as_ref().map(|run| json!({"id":run.id,"conversation_id":run.conversation,"title":"專案工作","mode":"background","state":"running","active":true,"project":true,"created_at":run.started,"message":self.projects.status,"can_retry":false}))
    }
    pub(super) fn project_state(&self) -> serde_json::Value {
        json!({"items":self.projects.store.projects.iter().map(|p| json!({"id":p.id,"name":p.name,"root":p.root,"import_count":p.imports.len()})).collect::<Vec<_>>(),
            "chart_review":self.projects.running.as_ref().and_then(|r|r.pending_chart.as_ref().map(|p|json!({"request_id":p.id,"review":p.review}))),
            "running":self.projects.running.is_some(),"running_id":self.projects.running.as_ref().map(|r|&r.id),"activity":self.projects.running.as_ref().map(|r|&r.activity),"charts":self.projects.running.as_ref().map(|r|&r.charts),"running_conversation":self.projects.running.as_ref().map(|r|&r.conversation),"status":self.projects.status,"error":self.projects.error})
    }
    pub(super) fn project_command(&mut self, command: ProjectCommand) -> AppResult<()> {
        if let ProjectCommand::ChartChoice {
            run_id,
            request_id,
            choices,
        } = &command
        {
            if !self.logged_in() {
                return Err("請先登入。".into());
            }
            let run = self
                .projects
                .running
                .as_mut()
                .filter(|r| r.id == *run_id && !r.cancel.load(Ordering::Relaxed))
                .ok_or("任務已結束。")?;
            let pending = run
                .pending_chart
                .as_ref()
                .filter(|p| p.id == *request_id)
                .ok_or("圖表選擇已失效。")?;
            if let Some(choices) = choices {
                pending.review.validate(choices)?;
            }
            let pending = run.pending_chart.take().ok_or("找不到圖表選擇。")?;
            pending
                .reply
                .send(choices.clone())
                .map_err(|_| "圖表等待已結束。")?;
            self.projects.status = "正在套用圖表選擇…".into();
            return Ok(());
        }
        if let ProjectCommand::Diagnostics {
            conversation,
            run_id,
        } = &command
        {
            if self.active_id.as_deref() != Some(conversation.as_str())
                || !self.archive.conversations.iter().any(|c| {
                    c.id == *conversation
                        && c.messages
                            .iter()
                            .any(|m| m.request_id.as_deref() == Some(run_id.as_str()))
                })
            {
                return Err("只能查看目前對話所屬的執行紀錄。".into());
            }
            let (root, conversation, id, tx) = (
                self.root.clone(),
                conversation.clone(),
                run_id.clone(),
                self.tx.clone(),
            );
            let token = self
                .session
                .as_ref()
                .map(|s| s.access_token.clone())
                .unwrap_or_default();
            thread::spawn(move || {
                let result = projects::diagnostics::read(&root, &id, &conversation).map(|text| {
                    if token.is_empty() {
                        text
                    } else {
                        text.replace(&token, "[已隱藏]")
                    }
                });
                let _ = tx.send(Event::Project(ProjectEvent::Diagnostics(
                    conversation,
                    id,
                    result,
                )));
            });
            return Ok(());
        }
        if let Some(error) = &self.projects.error {
            return Err(error.clone());
        }
        if let ProjectCommand::Reveal { conversation, path } = &command {
            let project = self
                .projects
                .store
                .project_for(conversation)
                .ok_or("此對話已不屬於專案。")?;
            return projects::files::reveal(project, path);
        }
        if matches!(command, ProjectCommand::Stop) {
            self.projects.cancel();
            return Ok(());
        }
        if self.projects.running.is_some() {
            return Err("請先完成或停止專案任務，再修改專案設定。".into());
        }
        if self.busy != "none" || self.work.incoming.is_some() {
            return Err("請先完成目前操作。".into());
        }
        match command {
            ProjectCommand::Resume {
                conversation,
                run_id,
                message_count,
            } => {
                if !self.logged_in()
                    || self.versions.blocked()
                    || !self.can_send()
                    || self.active_id.as_deref() != Some(&conversation)
                    || self.messages.len() != message_count
                {
                    return Err("目前無法續接，請確認登入與最新對話狀態。".into());
                }
                let message = self.messages.last().ok_or("找不到暫停訊息。")?;
                if !message.project_paused
                    || message.request_id.as_deref() != Some(&run_id)
                    || !projects::runner::paused_available(&self.root, &run_id)
                {
                    return Err("此暫停點已失效，請使用最新的繼續按鈕。".into());
                }
                self.begin_project_segment(self.messages.clone(), Some(run_id))?;
            }
            ProjectCommand::Create { name } => {
                if let Some(root) = choose_path(self.window, None)? {
                    let mut store = self.projects.store.clone();
                    let id = store.add(&name, root)?;
                    store.save(&self.root)?;
                    self.projects.store = store;
                    self.new_project_chat(&id)?;
                }
            }
            ProjectCommand::NewChat { id } => self.new_project_chat(&id)?,
            ProjectCommand::Rename { id, name } => {
                let name = name.trim();
                if name.is_empty() || name.chars().count() > 60 {
                    return Err("專案名稱需為 1–60 字。".into());
                }
                let mut store = self.projects.store.clone();
                store
                    .projects
                    .iter_mut()
                    .find(|p| p.id == id)
                    .ok_or("找不到專案。")?
                    .name = name.into();
                store.save(&self.root)?;
                self.projects.store = store;
                self.toast("已更新專案名稱");
            }
            ProjectCommand::ChooseImportFile { id, request_id } => {
                let mut project = self
                    .projects
                    .store
                    .projects
                    .iter()
                    .find(|p| p.id == id)
                    .cloned()
                    .ok_or("找不到專案。")?;
                projects::files::validate_root(&project.root)?;
                if let Some(path) = choose_path(self.window, Some(&project.root))? {
                    // 選擇器可以瀏覽其他目錄，但只有專案內檔案可被接受。
                    let relative = path
                        .strip_prefix(&project.root)
                        .map_err(|_| "請選擇此專案資料夾內的 TXT、MD、PDF 或 MSG 檔案。")?;
                    let key = relative.to_string_lossy().replace('\\', "/");
                    projects::files::relative(&key)?;
                    // 不嘗試解碼加密內容，只沿用 broker 驗證格式、連結與檔案身分。
                    project.imports.insert(key.clone(), String::new());
                    projects::files::read(&project, &key)?;
                    self.view
                        .post(&json!({"type":"project_import_file","id":id,
                        "request_id":request_id,"path":key}))?;
                }
            }
            ProjectCommand::Remove { id, delete_chats } => {
                self.remove_project(&id, delete_chats)?;
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
                self.toast("已匯入文字");
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
                self.toast("已清除匯入文字");
            }
            ProjectCommand::Reveal { .. } => unreachable!("已於上方處理成果定位"),
            ProjectCommand::Diagnostics { .. } => unreachable!("已於上方處理診斷讀取"),
            ProjectCommand::ChartChoice { .. } => unreachable!("handled above"),
            ProjectCommand::Stop => (),
        }
        Ok(())
    }
    /// 只刪除本機對話紀錄，絕不對專案根目錄或成果路徑呼叫檔案刪除。
    fn remove_project(&mut self, id: &str, delete_chats: bool) -> AppResult<()> {
        if self.history_error.is_some() {
            return Err("本機對話紀錄目前無法保存，請稍後再試。".into());
        }
        let mut store = self.projects.store.clone();
        if !store.projects.iter().any(|project| project.id == id) {
            return Err("找不到專案。".into());
        }
        let chats: std::collections::HashSet<String> = store
            .conversations
            .iter()
            .filter(|(_, project)| project.as_str() == id)
            .map(|(chat, _)| chat.clone())
            .collect();
        if delete_chats {
            if chats.iter().any(|chat| self.work.store.pending(Some(chat)))
                || self
                    .work
                    .store
                    .attachments
                    .iter()
                    .any(|a| chats.contains(&a.conversation_id) && !a.sent && !a.removed)
                || (self.mail_flow.phase != "idle"
                    && self
                        .mail_flow
                        .conversation
                        .as_ref()
                        .is_some_and(|chat| chats.contains(chat)))
            {
                return Err("專案對話仍有工作或附件，請先完成或取消後再刪除。".into());
            }
            let mut archive = self.archive.clone();
            archive
                .conversations
                .retain(|chat| !chats.contains(&chat.id));
            history::save(&self.root, &archive)?;
            self.archive = archive;
            // 即使後續另一份管理紀錄儲存失敗，也不可用舊畫面把已刪對話存回去。
            if self
                .active_id
                .as_ref()
                .is_some_and(|chat| chats.contains(chat))
            {
                self.active_id = None;
                self.messages.clear();
                self.set_draft(String::new());
                self.focus_draft = true;
            }
            self.work
                .store
                .tasks
                .retain(|task| !chats.contains(&task.conversation_id));
            self.work
                .store
                .attachments
                .retain(|file| !chats.contains(&file.conversation_id));
            self.work
                .store
                .conversations
                .retain(|chat, _| !chats.contains(chat));
            if !self.work.store.principal_id.is_empty() {
                self.work.store.save(&self.root, &self.config)?;
            }
        } else {
            self.preserve_draft()?;
        }
        store.projects.retain(|project| project.id != id);
        store.conversations.retain(|_, project| project != id);
        store.save(&self.root)?;
        self.projects.store = store;
        self.toast(if delete_chats {
            "已移除專案與對話"
        } else {
            "已移除專案，對話已移到一般對話"
        });
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
    pub(super) fn begin_project_chat(&mut self, messages: Vec<Message>) -> AppResult<()> {
        self.begin_project_segment(messages, None)
    }
    fn begin_project_segment(
        &mut self,
        mut messages: Vec<Message>,
        resume_id: Option<String>,
    ) -> AppResult<()> {
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
        let resume = resume_id.is_some();
        if resume {
            messages.push(Message::user("繼續先前未完成的任務。"));
        }
        let id = match resume_id {
            Some(id) => id,
            None => crate::jobs::new_id()?,
        };
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(message) = messages.last_mut() {
            message.request_id = Some(id.clone());
        }
        self.messages = messages.clone();
        self.save_history()?;
        let run = projects::runner::Run {
            resume,
            id: id.clone(),
            project,
            conversation: conversation.clone(),
            messages: if resume {
                vec![Message::user("繼續未完成任務")]
            } else {
                super::retry::context(&messages)?
            },
            config: self.config.clone(),
            session: self.session.clone().ok_or("請先登入。")?,
            root: self.root.clone(),
            cancel: cancel.clone(),
        };
        self.projects.running = Some(Running {
            pending_chart: None,
            id: id.clone(),
            conversation: conversation.clone(),
            activity: vec!["準備專案任務…".into()],
            charts: vec![],
            started: crate::unix_now(),
            cancel,
        });
        self.set_draft(String::new());
        self.projects.status = "正在建立受限制執行器…".into();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let export_tx = tx.clone();
            let export_id = id.clone();
            let renderer: projects::charts::png::Renderer = Box::new(move |chart, cancel| {
                let (reply, response) = mpsc::channel();
                export_tx
                    .send(Event::Project(ProjectEvent::ExportPng(
                        export_id.clone(),
                        chart.clone(),
                        reply,
                    )))
                    .map_err(|_| "桌面介面已關閉，PNG 未匯出。")?;
                let deadline = Instant::now() + Duration::from_secs(45);
                loop {
                    if cancel.load(Ordering::Relaxed) {
                        return Err("PNG 匯出已取消。".into());
                    }
                    if Instant::now() >= deadline {
                        return Err("PNG 繪製逾時，未寫入檔案。".into());
                    }
                    match response.recv_timeout(Duration::from_millis(100)) {
                        Ok(result) => return projects::charts::png::decode_url(&result?),
                        Err(mpsc::RecvTimeoutError::Timeout) => (),
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            return Err("PNG 繪製中斷。".into())
                        }
                    }
                }
            });
            let review_tx = tx.clone();
            let review_id = id.clone();
            let chooser: projects::charts::quality::Chooser =
                Box::new(move |review, cancel, deadline| {
                    let (reply, response) = mpsc::channel();
                    review_tx
                        .send(Event::Project(ProjectEvent::ReviewChart(
                            review_id.clone(),
                            crate::jobs::new_id()?,
                            review.clone(),
                            reply,
                        )))
                        .map_err(|_| "桌面介面已關閉。")?;
                    loop {
                        if cancel.load(Ordering::Relaxed) || Instant::now() >= deadline {
                            return Ok(None);
                        }
                        match response.recv_timeout(Duration::from_millis(100)) {
                            Ok(choice) => return Ok(choice),
                            Err(mpsc::RecvTimeoutError::Timeout) => (),
                            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(None),
                        }
                    }
                });
            let result = projects::runner::run_with_chart_export(
                run,
                |text| {
                    let _ = tx.send(Event::Project(ProjectEvent::Progress(id.clone(), text)));
                },
                |charts| {
                    let _ = tx.send(Event::Project(ProjectEvent::Charts(id.clone(), charts)));
                },
                renderer,
                chooser,
            );
            let _ = tx.send(Event::Project(ProjectEvent::Finished(
                id,
                conversation,
                result,
            )));
        });
        if !resume && self.messages.iter().filter(|m| m.role == "user").count() == 1 {
            if let Err(error) = self.queue_title(&self.active_id.clone().ok_or("找不到對話。")?)
            {
                self.toast(&format!("專案任務已開始；標題暫未產生：{error}"));
            }
        }
        Ok(())
    }
    pub(super) fn project_event(&mut self, event: ProjectEvent) -> AppResult<()> {
        match event {
            ProjectEvent::ReviewChart(id, request_id, review, reply) => {
                if let Some(run) = self
                    .projects
                    .running
                    .as_mut()
                    .filter(|r| r.id == id && !r.cancel.load(Ordering::Relaxed))
                {
                    run.pending_chart = Some(PendingChart {
                        id: request_id,
                        review,
                        reply,
                    });
                    self.projects.status = "等待選擇圖表資料處理方式…".into();
                    run.activity.push(self.projects.status.clone());
                }
            }
            ProjectEvent::Diagnostics(conversation, id, result) => {
                if self.logged_in() && self.active_id.as_deref() == Some(&conversation) {
                    self.view.post(&json!({"type":"project_diagnostics","conversation":conversation,"run_id":id,"text":result.unwrap_or_else(|e|format!("無法讀取執行紀錄：{e}"))}))?;
                }
            }
            ProjectEvent::ExportPng(id, chart, reply) => {
                if !self
                    .projects
                    .running
                    .as_ref()
                    .is_some_and(|run| run.id == id && !run.cancel.load(Ordering::Relaxed))
                {
                    let _ = reply.send(Err("任務已結束或取消，PNG 未匯出。".into()));
                } else if let Err(error) = self.view.export_chart_png(&chart, reply.clone()) {
                    let _ = reply.send(Err(error));
                }
            }
            ProjectEvent::Charts(id, charts) => {
                if let Some(run) = self.projects.running.as_mut().filter(|r| r.id == id) {
                    run.charts = charts;
                }
            }
            ProjectEvent::Progress(id, text) => {
                if let Some(run) = self.projects.running.as_mut().filter(|r| r.id == id) {
                    if run.activity.last() != Some(&text) {
                        if run.activity.len() >= 120 {
                            run.activity.remove(0);
                        }
                        run.activity.push(text.clone());
                    }
                    self.projects.status = text;
                }
            }
            ProjectEvent::Finished(id, conversation, result) => {
                if !self.projects.running.as_ref().is_some_and(|r| r.id == id) {
                    return Ok(());
                }
                let cancelled = self
                    .projects
                    .running
                    .as_ref()
                    .is_some_and(|r| r.cancel.load(Ordering::Relaxed));
                let activity = self
                    .projects
                    .running
                    .take()
                    .map(|run| run.activity)
                    .unwrap_or_default();
                let paused = projects::runner::paused_available(&self.root, &id);
                self.projects.status = if paused {
                    "專案已暫停，可按繼續。"
                } else if cancelled {
                    "專案任務已停止。"
                } else if result
                    .as_ref()
                    .is_ok_and(|text| text.starts_with("需要你的補充："))
                {
                    "等待你的補充。"
                } else if result.is_ok() {
                    "專案任務已完成。"
                } else {
                    "專案任務未完成，請查看對話。"
                }
                .into();
                let succeeded = result.is_ok();
                let text = result.unwrap_or_else(|error| format!("本次任務未完成：{error}"));
                let mut archive = self.archive.clone();
                let c = archive
                    .conversations
                    .iter_mut()
                    .find(|c| c.id == conversation)
                    .ok_or("專案對話已不存在。")?;
                let mut message = Message::assistant(text);
                message.project_paused = paused;
                message.project_activity = activity;
                message.project_charts = projects::runner::recover_charts(&self.root, &id);
                message.request_id = Some(id.clone());
                c.messages.push(message);
                c.updated_at = crate::unix_now();
                let final_text = c
                    .messages
                    .last()
                    .map(|m| m.content.clone())
                    .unwrap_or_default();
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
                // 暫停不建立「完成」卡；續接仍屬同一個任務。
                self.work.store.tasks.retain(|t| t.request_id != id);
                if !paused {
                    // 一個使用者任務只保存一張已套用的卡片，內部模型回合不進工作清單。
                    let state = if cancelled {
                        "cancelled"
                    } else if succeeded {
                        "completed"
                    } else {
                        "failed"
                    };
                    let remote = crate::jobs::TaskStatus {
                        agent_envelope: Default::default(),
                        task_id: id.clone(),
                        client_request_id: id.clone(),
                        state: state.into(),
                        progress: None,
                        queue_position: None,
                        result: if succeeded {
                            Some(json!({"choices":[{"message":{"content":final_text}}]}))
                        } else {
                            None
                        },
                        response_payload_json: None,
                        error_message: if succeeded {
                            String::new()
                        } else {
                            final_text.chars().take(500).collect()
                        },
                    };
                    self.work.store.tasks.push(crate::jobs::Task {
                        request_id: id.clone(),
                        conversation_id: conversation.clone(),
                        request: json!({}),
                        mode: "background".into(),
                        title: "專案工作".into(),
                        created_at: crate::unix_now(),
                        remote: Some(remote),
                        applied: true,
                        message: self.projects.status.clone(),
                        mail_analysis: false,
                        title_generation: false,
                        tool_events: vec![],
                        partial: String::new(),
                    });
                    self.work_save()?;
                }
                if self.logged_in() && !cancelled {
                    let waiting = self.projects.status == "等待你的補充。";
                    let notice = crate::notifications::Notification {
                        id: format!("project_{id}"),
                        kind: if paused {
                            "project.paused"
                        } else if waiting {
                            "project.waiting_user"
                        } else if succeeded {
                            "project.completed"
                        } else {
                            "project.failed"
                        }
                        .into(),
                        title: self.projects.status.clone(),
                        summary: final_text.chars().take(50).collect(),
                        created_at: crate::notifications::now_text(),
                        expires_at: None,
                        resource_id: Some(id.clone()),
                        read_at: if self.is_foreground() {
                            Some(crate::notifications::now_text())
                        } else {
                            None
                        },
                        dismissed: false,
                    };
                    self.inbox.merge(crate::notifications::EventPage {
                        events: vec![notice],
                        next_cursor: None,
                        has_more: false,
                    })?;
                    crate::notifications::save(&self.root, &self.inbox)?;
                    self.toast(&self.projects.status.clone());
                    if self.config.notification_popups && !self.is_foreground() {
                        self.work.task_balloon = true;
                        tray(self.window, NIM_MODIFY, Some(&self.projects.status));
                    }
                }
            }
        }
        Ok(())
    }
}
