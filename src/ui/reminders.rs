//! 本機提醒的介面命令與計時器。保存成功後才更新記憶體或送出通知。
use super::*;
use crate::reminders::{Channel, Store};

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub(super) enum ReminderCommand {
    State,
    Save {
        id: Option<u64>,
        title: String,
        due_at: u64,
        channel: Channel,
        #[serde(default)]
        repeat_days: u32,
        #[serde(default)]
        workflow: Option<WorkflowRequest>,
    },
    LoadWorkflow {
        id: u64,
    },
    Delete {
        id: u64,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WorkflowRequest {
    conversation: String,
    template: String,
}

pub(super) struct ReminderRuntime {
    store: AppResult<Store>,
    next_check: Instant,
}
impl ReminderRuntime {
    pub(super) fn load(root: &Path) -> Self {
        Self {
            store: Store::load(root),
            next_check: Instant::now(),
        }
    }
}

impl App {
    pub(super) fn reminder_command(&mut self, command: ReminderCommand) -> AppResult<()> {
        let result = (|| {
            if let ReminderCommand::LoadWorkflow { id } = command {
                return self.load_reminder_workflow(id);
            }
            let store = self.reminders.store.as_ref().map_err(Clone::clone)?;
            let mut next = store.clone();
            let changed = match command {
                ReminderCommand::State => false,
                ReminderCommand::Save {
                    id,
                    title,
                    due_at,
                    channel,
                    repeat_days,
                    workflow,
                } => {
                    let target = workflow
                        .as_ref()
                        .map(|request| self.reminder_target(request))
                        .transpose()?;
                    next.upsert(id, &title, due_at, channel, crate::unix_now())?;
                    next.configure(id, repeat_days, target)?;
                    true
                }
                ReminderCommand::Delete { id } => {
                    next.delete(id)?;
                    true
                }
                ReminderCommand::LoadWorkflow { .. } => unreachable!(),
            };
            if changed {
                next.save(&self.root)?;
                self.reminders.store = Ok(next);
            }
            self.publish_reminders(true)
        })();
        if let Err(error) = &result {
            let _ = self
                .view
                .post(&json!({"type":"reminder_error", "message":error}));
        }
        result
    }

    fn reminder_target(
        &self,
        request: &WorkflowRequest,
    ) -> AppResult<crate::reminders::WorkflowTarget> {
        if !self.logged_in() || self.versions.blocked() || self.work.store.principal_id.is_empty() {
            return Err("請先登入並完成能力同步後設定流程提醒。".into());
        }
        let project = self
            .projects
            .store
            .project_for(&request.conversation)
            .ok_or("原專案對話已不存在。")?;
        let memory = crate::projects::memory::Memory::open(project.clone(), &request.conversation)?;
        let flow = memory.load_workflow(&request.template)?;
        Ok(crate::reminders::WorkflowTarget {
            conversation: request.conversation.clone(),
            project: project.id.clone(),
            principal: self.work.store.principal_id.clone(),
            root_identity: crate::projects::files::directory_identity(&project.root)?,
            template: flow.id.clone(),
            revision: crate::projects::text::revision(
                &serde_json::to_string(&flow).map_err(|e| e.to_string())?,
            ),
        })
    }

    fn load_reminder_workflow(&mut self, id: u64) -> AppResult<()> {
        if self.busy != "none" || self.work.incoming.is_some() || self.projects.is_running() {
            return Err("請先等待目前操作結束，再載入流程。".into());
        }
        let target = self
            .reminders
            .store
            .as_ref()
            .map_err(Clone::clone)?
            .entries
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| e.workflow.clone())
            .ok_or("此提醒沒有流程。")?;
        let current = self.reminder_target(&WorkflowRequest {
            conversation: target.conversation.clone(),
            template: target.template.clone(),
        })?;
        if current.project != target.project
            || current.principal != target.principal
            || current.root_identity != target.root_identity
            || current.revision != target.revision
        {
            return Err("帳號、專案位置或流程範本已變更，請從成果中心重新設定提醒。".into());
        }
        let project = self
            .projects
            .store
            .project_for(&target.conversation)
            .ok_or("專案已不存在。")?
            .clone();
        let data = crate::projects::memory::Memory::open(project, &target.conversation)?
            .list_workflows()?;
        let flow = data["workflows"]
            .as_array()
            .and_then(|flows| flows.iter().find(|f| f["id"] == target.template))
            .ok_or("流程已不存在。")?;
        self.command(Command::SelectChat {
            id: target.conversation.clone(),
        })?;
        if self.active_id.as_deref() != Some(&target.conversation) {
            return Err("無法切換原對話，未載入流程。".into());
        }
        self.publish();
        self.view
            .post(&json!({"type":"reminder_flow","conversation":target.conversation,"flow":flow}))
    }

    fn publish_reminders(&self, acknowledgement: bool) -> AppResult<()> {
        let store = self.reminders.store.as_ref().map_err(Clone::clone)?;
        self.view.post(&json!({"type":"reminder_state", "entries":store.entries, "acknowledgement":acknowledgement}))
    }

    pub(super) fn tick_reminders(&mut self) {
        // 自檢／模型測試不觸發真實通知；正式程式即使未登入也可提醒。
        if self.smoke || self.demo || !self.ready || Instant::now() < self.reminders.next_check {
            return;
        }
        self.reminders.next_check = Instant::now() + Duration::from_secs(1);
        let Ok(store) = &self.reminders.store else {
            return;
        };
        let now = crate::unix_now();
        let Some(entry) = store.next_due(now) else {
            return;
        };
        let previous = store.clone();
        let mut next = previous.clone();
        let result = next
            .mark_notified(entry.id, now)
            .and_then(|()| next.save(&self.root));
        if let Err(error) = result {
            self.reminders.next_check = Instant::now() + Duration::from_secs(30);
            self.fail(error);
            self.publish();
            return;
        }
        // 先保存送出記錄，避免重開後每次重播。Windows是否顯示仍由通知／勿擾設定決定。
        self.reminders.store = Ok(next);
        self.work.tray_restore_only = true;
        self.schedules.notice = false;
        #[cfg(feature = "pet")]
        let pet_available = {
            let pet = self.pet.settings();
            pet.notice_seconds > 0 && pet.bubble_opacity > 0
        };
        // 精簡版保留舊提醒資料，原桌寵通知改由Windows呈現，不遺失提醒。
        #[cfg(not(feature = "pet"))]
        let pet_available = false;
        let delivered = if entry.channel == Channel::Pet && pet_available {
            self.desktop_notice(&format!("提醒：{}", entry.title));
            true
        } else {
            windows_notice(self.window, &entry.title)
        };
        if !delivered {
            // Shell拒絕接收時保留未送出狀態，稍後再試；不可一直每50ms重試。
            if previous.save(&self.root).is_ok() {
                self.reminders.store = Ok(previous);
            }
            self.fail("Windows未接受提醒通知，請確認系統通知設定；提醒清單仍可查看。".into());
            self.publish();
        }
        self.reminders.next_check =
            Instant::now() + Duration::from_secs(if delivered { 6 } else { 30 });
        let _ = self.publish_reminders(false);
    }
}

/// 沿用應用程式托盤圖示，文字只來自使用者明確設定的提醒。
pub(super) fn windows_notice(window: HWND, title: &str) -> bool {
    let mut data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: window,
        uID: 1,
        uFlags: NIF_INFO,
        dwInfoFlags: NIIF_INFO,
        ..Default::default()
    };
    copy_notice_text(&mut data.szInfoTitle, "LM_AI 提醒事項");
    copy_notice_text(&mut data.szInfo, title);
    unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) != 0 }
}

/// 依字元截斷，保留結尾NUL且不切斷emoji的UTF-16代理對。
fn copy_notice_text(buffer: &mut [u16], text: &str) {
    let mut at = 0;
    for character in text.chars() {
        let mut units = [0; 2];
        let encoded = character.encode_utf16(&mut units);
        if at + encoded.len() >= buffer.len() {
            break;
        }
        buffer[at..at + encoded.len()].copy_from_slice(encoded);
        at += encoded.len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_shell_accepts_native_reminder_notification() {
        unsafe {
            let window = CreateWindowExW(
                0,
                wide("STATIC").as_ptr(),
                wide("LM_AI notification test").as_ptr(),
                WS_POPUP,
                0,
                0,
                1,
                1,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null(),
            );
            assert!(!window.is_null());
            let data = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: window,
                uID: 1,
                uFlags: NIF_ICON,
                hIcon: LoadIconW(ptr::null_mut(), IDI_INFORMATION),
                ..Default::default()
            };
            let added = Shell_NotifyIconW(NIM_ADD, &data);
            let delivered = added != 0 && windows_notice(window, "LM_AI 本機提醒功能測試");
            Shell_NotifyIconW(NIM_DELETE, &data);
            DestroyWindow(window);
            assert_ne!(added, 0, "Windows未接受測試托盤圖示");
            assert!(delivered, "Windows未接受提醒通知");
        }
    }

    #[test]
    fn notification_truncation_preserves_utf16_and_termination() {
        let mut buffer = [0; 4];
        copy_notice_text(&mut buffer, "中😀文");
        assert_eq!(String::from_utf16(&buffer[..3]).unwrap(), "中😀");
        assert_eq!(buffer[3], 0);
    }
}
