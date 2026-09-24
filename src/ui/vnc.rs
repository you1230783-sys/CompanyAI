//! VNC 頁面命令。所有檔案位置由原生層決定，連線只接受已載入機台的分類與索引。
use super::*;
use crate::vnc::{self, sync, MachineKey, Manager, Options, Selection};
use windows::{
    core::w,
    Win32::{
        Foundation::{ERROR_CANCELLED, HWND as WinHwnd},
        System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER},
        UI::Shell::{
            Common::COMDLG_FILTERSPEC, FileOpenDialog, IFileOpenDialog, FOS_FILEMUSTEXIST,
            FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST, SIGDN_FILESYSPATH,
        },
    },
};

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub(super) enum VncCommand {
    SyncSettings {
        settings: sync::Settings,
        clear_password: bool,
        start: bool,
    },
    CancelSync,
    RefreshImport {
        preview_id: String,
    },
    DiscardImport {
        preview_id: String,
    },
    Import {
        revision: u64,
        preview_id: String,
        indices: Vec<usize>,
    },
    Batch {
        revision: u64,
        selection: Selection,
        operation: String,
    },
    MoveMachine {
        revision: u64,
        group: String,
        index: usize,
        direction: MoveDirection,
    },
    Open,
    Reload,
    ChooseViewer,
    SearchViewer,
    Options {
        fullscreen: bool,
        viewonly: bool,
        autoscaling: bool,
    },
    Connect {
        revision: u64,
        group: String,
        index: usize,
    },
    SaveMachine {
        revision: u64,
        original: Option<MachineKey>,
        group: String,
        name: String,
        ip: String,
        /// None 保留原密碼；Some("") 才是使用者明確清除。
        password: Option<String>,
    },
    DeleteMachine {
        revision: u64,
        group: String,
        index: usize,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum MoveDirection {
    Up,
    Down,
}

#[derive(Default)]
pub(super) struct VncRuntime {
    manager: Option<Manager>,
    revision: u64,
    search_id: Option<String>,
    status: String,
    settings: Option<sync::Settings>,
    settings_revision: u64,
    sync_id: Option<String>,
    cancel: Option<Arc<AtomicBool>>,
    preview: Option<(String, sync::Download)>,
    preview_revision: u64,
    busy: bool,
    closing: bool,
    commands: Option<std::sync::mpsc::Sender<sync::SessionCommand>>,
    workers: Vec<thread::JoinHandle<()>>,
}

impl VncRuntime {
    pub(super) fn cancel_sync(&self) {
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::Relaxed);
        }
        if let Some(commands) = &self.commands {
            let _ = commands.send(sync::SessionCommand::Close);
        }
    }

    /// 停用立即清除 UI 資料，但保留背景工作控制柄，真正退出時可等候登出完成。
    pub(super) fn disable(&mut self) {
        self.cancel_sync();
        let workers = std::mem::take(&mut self.workers);
        *self = Self {
            workers,
            ..Self::default()
        };
    }

    pub(super) fn shutdown(&mut self) {
        self.cancel_sync();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

/// 原生檔案選擇器僅允許選 Viewer；不接收網頁提供的任意執行檔路徑。
fn choose_viewer(window: HWND) -> AppResult<Option<PathBuf>> {
    // SAFETY: 主視窗執行緒已初始化 COM STA；所有字串與 owner 在同步對話框期間有效。
    unsafe {
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
            .map_err(|_| "無法建立檔案選擇器。")?;
        dialog
            .SetTitle(w!("選擇已安裝的 UltraVNC Viewer"))
            .map_err(|e| e.to_string())?;
        dialog
            .SetOptions(FOS_FILEMUSTEXIST | FOS_PATHMUSTEXIST | FOS_FORCEFILESYSTEM)
            .map_err(|e| e.to_string())?;
        dialog
            .SetFileTypes(&[COMDLG_FILTERSPEC {
                pszName: w!("UltraVNC Viewer"),
                pszSpec: w!("vncviewer.exe"),
            }])
            .map_err(|e| e.to_string())?;
        if let Err(error) = dialog.Show(Some(WinHwnd(window))) {
            if error.code() == windows::core::HRESULT::from_win32(ERROR_CANCELLED.0) {
                return Ok(None);
            }
            return Err(format!("無法選取 Viewer：{error}"));
        }
        let item = dialog.GetResult().map_err(|e| e.to_string())?;
        let name = item
            .GetDisplayName(SIGDN_FILESYSPATH)
            .map_err(|e| e.to_string())?;
        let path = name.to_string();
        CoTaskMemFree(Some(name.0.cast()));
        Ok(Some(PathBuf::from(
            path.map_err(|_| "Viewer 路徑無法辨識。")?,
        )))
    }
}

impl App {
    pub(super) fn vnc_state(&self) -> serde_json::Value {
        if !self.logged_in() || !self.config.vnc_enabled {
            return json!({"loaded":false});
        }
        let manager = self.vnc.manager.as_ref();
        json!({
            "loaded": manager.is_some(), "revision": self.vnc.revision,
            "searching": self.vnc.search_id.is_some(), "status": self.vnc.status,
            "syncing":self.vnc.busy, "session_open":self.vnc.sync_id.is_some(), "closing":self.vnc.closing,
            "sync_settings":self.vnc.settings.as_ref().map(sync::Settings::public),
            "settings_revision":self.vnc.settings_revision,
            "preview":self.vnc.preview.as_ref().map(|(id, result)| json!({"id":id,"machines":result.machines.iter().map(|m| {
                let mut value = json!(m);
                value["comparison"] = json!(manager.map(|manager| manager.comparison(m)).unwrap_or("new"));
                value
            }).collect::<Vec<_>>()})),
            "machines_path": manager.map(|m| m.path.to_string_lossy()),
            "viewer_path": manager.map(|m| &m.config.vnc_path),
            "options": manager.map(|m| json!({"fullscreen":m.config.options.fullscreen,
                "viewonly":m.config.options.viewonly,"autoscaling":m.config.options.autoscaling})),
            "groups": manager.map(Manager::public_groups).unwrap_or_else(|| json!([]))
        })
    }

    fn vnc_manager(&mut self, revision: u64) -> AppResult<&mut Manager> {
        if revision != self.vnc.revision {
            return Err("機台清單已更新，請重新選取機台。".into());
        }
        self.vnc
            .manager
            .as_mut()
            .ok_or_else(|| "尚未成功讀取機台設定，請先按「重新讀取」。".into())
    }

    pub(super) fn vnc_command(&mut self, command: VncCommand) -> AppResult<()> {
        if !self.logged_in() {
            return Err("請先完成瀏覽器登入，再使用 VNC。".into());
        }
        if !self.config.vnc_enabled {
            return Err("請先在設定中啟用 VNC 快速連線功能。".into());
        }
        if self.demo || self.smoke {
            return Err("示範與自我檢查不讀取真實 VNC 設定或啟動連線。".into());
        }
        let result = self.vnc_dispatch(command);
        if let Err(error) = &result {
            self.vnc.status = error.clone();
        }
        result
    }

    fn vnc_dispatch(&mut self, command: VncCommand) -> AppResult<()> {
        match command {
            VncCommand::SyncSettings {
                mut settings,
                clear_password,
                start,
            } => {
                if self.vnc.sync_id.is_some() {
                    return Err("請先匯入或捨棄本次清單，再修改連線設定或重新登入。".into());
                }
                let previous = self
                    .vnc
                    .settings
                    .as_ref()
                    .ok_or("請先開啟 VNC 頁面讀取設定。")?;
                if clear_password {
                    settings.password.clear();
                } else if settings.password.is_empty() {
                    if settings.base()?.origin() != previous.base()?.origin()
                        && !previous.password.is_empty()
                    {
                        return Err("網站主機已變更，請重新輸入該網站的密碼。".into());
                    }
                    settings.password.clone_from(&previous.password);
                }
                settings.save(&self.root)?;
                self.vnc.settings = Some(settings.clone());
                self.vnc.settings_revision += 1;
                self.vnc.status = "網站登入與連結設定已儲存；只在按更新時連線。".into();
                if start {
                    if settings.username.trim().is_empty() || settings.password.is_empty() {
                        return Err("請輸入機台網站的帳號與密碼。".into());
                    }
                    let id = crate::jobs::new_id()?;
                    let cancel = Arc::new(AtomicBool::new(false));
                    self.vnc.cancel = Some(cancel.clone());
                    self.vnc.sync_id = Some(id.clone());
                    self.vnc.busy = true;
                    self.vnc.closing = false;
                    self.vnc.preview = None;
                    self.vnc.status = "正在登入並取得機台清單…".into();
                    let tx = self.tx.clone();
                    let (commands, receiver) = std::sync::mpsc::channel();
                    self.vnc.commands = Some(commands);
                    self.vnc.workers.retain(|worker| !worker.is_finished());
                    self.vnc.workers.push(thread::spawn(move || {
                        sync::run_session(&settings, &cancel, receiver, |event| {
                            tx.send(Event::VncSync(id.clone(), event)).is_ok()
                        });
                    }));
                }
            }
            VncCommand::CancelSync => {
                self.vnc_end_sync("正在停止取得清單並嘗試登出…");
            }
            VncCommand::RefreshImport { preview_id } => {
                if self.vnc.busy
                    || self
                        .vnc
                        .preview
                        .as_ref()
                        .is_none_or(|(id, _)| id != &preview_id)
                {
                    return Err("清單忙碌或預覽已變更，請等待完成後重試。".into());
                }
                self.vnc
                    .commands
                    .as_ref()
                    .ok_or("網站登入已結束，請重新更新。")?
                    .send(sync::SessionCommand::Refresh)
                    .map_err(|_| "網站登入已結束，請重新更新。")?;
                self.vnc.preview = None;
                self.vnc.busy = true;
                self.vnc.status = "正在使用本次登入重新取得清單…".into();
            }
            VncCommand::DiscardImport { preview_id } => {
                if !self.vnc.busy
                    && self
                        .vnc
                        .preview
                        .as_ref()
                        .is_some_and(|(id, _)| id == &preview_id)
                {
                    self.vnc_end_sync("已捨棄暫存清單，本機機台未變更。正在登出…");
                }
            }
            VncCommand::Import {
                revision,
                preview_id,
                indices,
            } => {
                if self.vnc.busy || revision != self.vnc.revision {
                    return Err("機台清單已變更，請重新確認要匯入的項目。".into());
                }
                let (_, download) = self
                    .vnc
                    .preview
                    .as_ref()
                    .filter(|(id, _)| id == &preview_id)
                    .ok_or("匯入預覽已失效，請重新取得清單。")?;
                let count = self
                    .vnc
                    .manager
                    .as_mut()
                    .ok_or("請先讀取機台設定。")?
                    .import(download, &indices)?;
                self.vnc.revision += 1;
                self.vnc_end_sync(&format!(
                    "已匯入或更新 {count} 台機台，原有密碼與手動機台已保留。正在登出…"
                ));
                self.view.post(&json!({"type":"vnc_saved"}))?;
            }
            VncCommand::Batch {
                revision,
                selection,
                operation,
            } => {
                let manager = self.vnc_manager(revision)?;
                match operation.as_str() {
                    "groups_up" | "groups_down" => {
                        manager.move_groups(&selection.groups, operation == "groups_up")?
                    }
                    "up" | "down" | "delete" => manager.batch_machines(&selection, &operation)?,
                    _ => return Err("批次操作無效。".into()),
                }
                self.vnc.revision += 1;
                self.vnc.status = "批次機台操作已儲存。".into();
                self.view.post(&json!({"type":"vnc_saved"}))?;
            }
            VncCommand::MoveMachine {
                revision,
                group,
                index,
                direction,
            } => {
                let manager = self.vnc_manager(revision)?;
                manager.machine(&group, index)?;
                let mut machines = manager.machines.clone();
                let list = machines.get_mut(&group).ok_or("機台分類已不存在。")?;
                let target = match direction {
                    MoveDirection::Up => index.checked_sub(1),
                    MoveDirection::Down => index.checked_add(1).filter(|value| *value < list.len()),
                }
                .ok_or("機台已在此分類的最前或最後位置。")?;
                list.swap(index, target);
                manager.save_machines(machines)?;
                self.vnc.revision += 1;
                self.vnc.status = "機台順序已儲存。".into();
                self.view.post(&json!({"type":"vnc_saved"}))?;
            }
            VncCommand::Open | VncCommand::Reload => {
                if self.vnc.busy {
                    return Err("取得清單中，請先等候完成。".into());
                }
                // Reload 失敗時丟棄舊快照，避免仍可操作已損壞或已被替換的設定。
                self.vnc.manager = None;
                self.vnc.search_id = None;
                self.vnc.revision += 1;
                self.vnc.manager = Some(Manager::load(Manager::default_path()?)?);
                self.vnc.settings = Some(sync::Settings::load(&self.root)?);
                self.vnc.settings_revision += 1;
                self.vnc.status = "已讀取機台設定。連線只會在點擊機台後啟動。".into();
                let manager = self.vnc.manager.as_ref().ok_or("尚未載入 VNC 設定。")?;
                if vnc::validate_viewer(std::path::Path::new(&manager.config.vnc_path)).is_err() {
                    self.vnc_start_search()?;
                }
            }
            VncCommand::SearchViewer => self.vnc_start_search()?,
            VncCommand::ChooseViewer => {
                if let Some(path) = choose_viewer(self.window)? {
                    vnc::validate_viewer(&path)?;
                    self.vnc.search_id = None;
                    self.vnc_set_viewer(path)?;
                }
            }
            VncCommand::Options {
                fullscreen,
                viewonly,
                autoscaling,
            } => {
                let manager = self.vnc_manager(self.vnc.revision)?;
                let mut config = manager.config.clone();
                config.options = Options {
                    fullscreen,
                    viewonly,
                    autoscaling,
                    extra: config.options.extra,
                };
                manager.save_config(config)?;
                self.vnc.status = "連線選項已儲存。".into();
            }
            VncCommand::Connect {
                revision,
                group,
                index,
            } => {
                let manager = self.vnc_manager(revision)?;
                vnc::connect(manager, &group, index)?;
                self.vnc.status = format!(
                    "已啟動連線：{}（連線結果請查看 Viewer）",
                    manager.machine(&group, index)?.name
                );
            }
            VncCommand::SaveMachine {
                revision,
                original,
                group,
                name,
                ip,
                password,
            } => {
                let group = group.trim();
                let name = name.trim();
                let ip = ip.trim();
                let manager = self.vnc_manager(revision)?;
                let old = original
                    .as_ref()
                    .map(|key| manager.machine(&key.group, key.index))
                    .transpose()?;
                let password = password.unwrap_or_else(|| {
                    old.map(|machine| machine.password.clone())
                        .unwrap_or_default()
                });
                vnc::validate_machine(group, name, ip, &password)?;
                let machine = vnc::Machine {
                    name: name.into(),
                    ip: ip.into(),
                    password,
                    extra: old.map(|machine| machine.extra.clone()).unwrap_or_default(),
                };
                let mut machines = manager.machines.clone();
                if let Some(key) = &original {
                    let list = machines.get_mut(&key.group).ok_or("機台分類已不存在。")?;
                    if key.group == group {
                        list[key.index] = machine;
                    } else {
                        list.remove(key.index);
                        if list.is_empty() {
                            machines.remove(&key.group);
                        }
                        machines.entry(group.into()).or_default().push(machine);
                    }
                } else {
                    machines.entry(group.into()).or_default().push(machine);
                }
                manager.save_machines(machines)?;
                self.vnc.revision += 1;
                self.vnc.status = "機台設定已儲存。".into();
                self.view.post(&json!({"type":"vnc_saved"}))?;
            }
            VncCommand::DeleteMachine {
                revision,
                group,
                index,
            } => {
                let manager = self.vnc_manager(revision)?;
                manager.machine(&group, index)?;
                let mut machines = manager.machines.clone();
                let list = machines.get_mut(&group).ok_or("機台分類已不存在。")?;
                list.remove(index);
                if list.is_empty() {
                    machines.remove(&group);
                }
                manager.save_machines(machines)?;
                self.vnc.revision += 1;
                self.vnc.status = "機台已刪除。".into();
                self.view.post(&json!({"type":"vnc_saved"}))?;
            }
        }
        Ok(())
    }

    fn vnc_start_search(&mut self) -> AppResult<()> {
        if self.vnc.search_id.is_some() {
            return Ok(());
        }
        if self.vnc.manager.is_none() {
            return Err("請先成功讀取 VNC 設定。".into());
        }
        let id = crate::jobs::new_id()?;
        self.vnc.search_id = Some(id.clone());
        self.vnc.status = "正在標準安裝目錄搜尋 vncviewer.exe…".into();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let _ = tx.send(Event::VncSearch(id, vnc::find_viewer()));
        });
        Ok(())
    }

    fn vnc_set_viewer(&mut self, path: PathBuf) -> AppResult<()> {
        let manager = self.vnc_manager(self.vnc.revision)?;
        let mut config = manager.config.clone();
        config.vnc_path = path.to_str().ok_or("Viewer 路徑不是有效文字。")?.into();
        manager.save_config(config)?;
        self.vnc.status = "UltraVNC Viewer 路徑已儲存。".into();
        Ok(())
    }

    pub(super) fn vnc_search_result(
        &mut self,
        id: String,
        result: AppResult<PathBuf>,
    ) -> AppResult<()> {
        if !self.logged_in() || !self.config.vnc_enabled || self.vnc.search_id.as_ref() != Some(&id)
        {
            return Ok(());
        }
        self.vnc.search_id = None;
        match result.and_then(|path| self.vnc_set_viewer(path)) {
            Ok(()) => {}
            Err(error) => self.vnc.status = error,
        }
        Ok(())
    }

    fn vnc_end_sync(&mut self, status: &str) {
        self.vnc.cancel_sync();
        self.vnc.preview = None;
        self.vnc.busy = self.vnc.sync_id.is_some();
        self.vnc.closing = self.vnc.busy;
        self.vnc.status = status.into();
    }

    pub(super) fn vnc_sync_result(&mut self, id: String, event: sync::SessionEvent) {
        if !self.logged_in() || !self.config.vnc_enabled || self.vnc.sync_id.as_ref() != Some(&id) {
            return;
        }
        match event {
            sync::SessionEvent::Ready(download) => {
                // 使用者可能在背景完成與 UI 收到事件之間按停止，不能重新開啟預覽。
                if self.vnc.closing {
                    return;
                }
                self.vnc.busy = false;
                self.vnc.preview_revision += 1;
                self.vnc.status = format!(
                    "已取得清單，請勾選要匯入的分類或機台；匯入或捨棄後會登出。 {}",
                    download.warning
                );
                if download.machines.is_empty() {
                    self.vnc.status.push_str(" 本次沒有可匯入的機台。");
                }
                self.vnc.preview = Some((format!("{id}-{}", self.vnc.preview_revision), download));
            }
            sync::SessionEvent::Finished(result) => {
                self.vnc.sync_id = None;
                self.vnc.cancel = None;
                self.vnc.commands = None;
                self.vnc.preview = None;
                self.vnc.busy = false;
                match result {
                    Ok(()) => {
                        self.vnc.status = self
                            .vnc
                            .status
                            .replace("正在登出…", "已登出。")
                            .replace("正在停止取得清單並嘗試登出…", "已停止並登出。")
                    }
                    Err(error) if self.vnc.closing => {
                        self.vnc.status.push_str(&format!(" {error}"))
                    }
                    Err(error) => self.vnc.status = error,
                }
                self.vnc.closing = false;
            }
        }
    }
}
