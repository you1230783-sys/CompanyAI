//! VNC 頁面命令。所有檔案位置由原生層決定，連線只接受已載入機台的分類與索引。
use super::*;
use crate::vnc::{self, Manager, Options};
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
pub(super) struct MachineKey {
    group: String,
    index: usize,
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
        if !self.config.vnc_enabled {
            return json!({"loaded":false});
        }
        let manager = self.vnc.manager.as_ref();
        json!({
            "loaded": manager.is_some(), "revision": self.vnc.revision,
            "searching": self.vnc.search_id.is_some(), "status": self.vnc.status,
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
                // Reload 失敗時丟棄舊快照，避免仍可操作已損壞或已被替換的設定。
                self.vnc.manager = None;
                self.vnc.search_id = None;
                self.vnc.revision += 1;
                self.vnc.manager = Some(Manager::load(Manager::default_path()?)?);
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
        if !self.config.vnc_enabled || self.vnc.search_id.as_ref() != Some(&id) {
            return Ok(());
        }
        self.vnc.search_id = None;
        match result.and_then(|path| self.vnc_set_viewer(path)) {
            Ok(()) => {}
            Err(error) => self.vnc.status = error,
        }
        Ok(())
    }
}
