//! 原生預覽／確認與開啟中計時器；UI 不能直接提交工具或持久授權。
use super::*;
use crate::projects::schedule::{self, Entry, Grant, Journal, Store};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum ScheduleCommand {
    State,
    Preview {
        request_id: String,
        conversation: String,
        template: String,
        parameters: BTreeMap<String, Value>,
        due_at: u64,
        repeat_days: u32,
        #[serde(default)]
        continuous_retry: bool,
    },
    Confirm {
        preview_id: String,
    },
    Disable {
        id: String,
    },
    Resume {
        id: String,
    },
    Stop,
    StopExecution {
        execution: String,
    },
}
struct Preview {
    id: String,
    grant: Grant,
    expires: Instant,
}
struct Running {
    conversation: String,
    wait_events: mpsc::Receiver<Option<String>>,
    waiting: Option<String>,
    execution: String,
    cancel: Arc<AtomicBool>,
    result: mpsc::Receiver<AppResult<()>>,
}
pub(super) struct Runtime {
    store: AppResult<Store>,
    preview: Option<Preview>,
    running: Option<Running>,
    next_check: Instant,
    recovered: bool,
    pub(super) notice: bool,
    last_error_notice: Option<String>,
}
impl Runtime {
    pub(super) fn load(root: &Path) -> Self {
        Self {
            store: Store::load(root),
            preview: None,
            running: None,
            next_check: Instant::now(),
            recovered: false,
            notice: false,
            last_error_notice: None,
        }
    }
    pub(super) fn active(&self) -> bool {
        self.running.is_some()
    }
    pub(super) fn conversation(&self) -> Option<&str> {
        self.running.as_ref().map(|run| run.conversation.as_str())
    }
    pub(super) fn active_conversation(&self, conversation: &str) -> bool {
        self.running
            .as_ref()
            .is_some_and(|run| run.conversation == conversation)
    }
    pub(super) fn cancel(&self) {
        if let Some(run) = &self.running {
            run.cancel.store(true, Ordering::Relaxed);
        }
    }
}
impl App {
    /// 隱藏視窗自檢只注入受控排程，不派送 Office 或模型。
    pub(super) fn verify_schedule_slot_accounting(&mut self) -> AppResult<()> {
        if !self.smoke || self.projects.count() != 1 || self.schedules.active() {
            return Err("排程名額自檢前置條件不符。".into());
        }
        let (_tx, result) = mpsc::channel();
        let (_wait_tx, wait_events) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        self.schedules.running = Some(Running {
            conversation: "schedule-chat".into(),
            execution: "schedule-run".into(),
            cancel: cancel.clone(),
            result,
            wait_events,
            waiting: None,
        });
        let result = (|| {
            if !self.project_slots_full() || self.schedule_task().is_none() {
                return Err("排程未占用第二名額。".into());
            }
            if self
                .schedule_command(ScheduleCommand::StopExecution {
                    execution: "stale".into(),
                })
                .is_ok()
                || cancel.load(Ordering::Relaxed)
            {
                return Err("過期排程卡片停止了新任務。".into());
            }
            self.schedule_command(ScheduleCommand::StopExecution {
                execution: "schedule-run".into(),
            })?;
            if !cancel.load(Ordering::Relaxed) {
                return Err("排程停止旗標未設置。".into());
            }
            Ok(())
        })();
        self.schedules.running = None;
        result
    }
    pub(super) fn schedule_task(&self) -> Option<Value> {
        self.schedules.running.as_ref().map(|run| {
            json!({"id":run.execution,"conversation_id":run.conversation,
            "title":"流程排程","mode":"background","state":"running","active":true,"schedule":true,
            "message":run.waiting.as_deref().unwrap_or("正在執行固定流程"),"can_retry":false})
        })
    }
    fn schedule_available(&self) -> AppResult<()> {
        if !self.logged_in() || self.versions.blocked() || self.work.store.principal_id.is_empty() {
            return Err("請登入並完成能力同步後使用流程排程。".into());
        }
        if self.smoke || self.demo {
            return Err("示範／自檢模式不能建立實際排程。".into());
        }
        Ok(())
    }
    fn schedule_idle(&self) -> bool {
        self.busy == "none"
            && !self.project_slots_full()
            && self.work.incoming.is_none()
            && self.workspace_import.is_none()
            && !self.mail_busy
            && !self.update_busy
            && !self.schedules.active()
    }
    fn schedule_project(&self, grant: &Grant) -> AppResult<crate::projects::Project> {
        self.schedule_available()?;
        let project = self
            .projects
            .store
            .project_for(&grant.target.conversation)
            .ok_or("排程原對話或專案已不存在。")?
            .clone();
        schedule::execution::check_target(&project, grant, &self.work.store.principal_id)?;
        Ok(project)
    }
    pub(super) fn schedule_command(&mut self, command: ScheduleCommand) -> AppResult<()> {
        let result = (|| {
            match command {
                ScheduleCommand::State => {}
                ScheduleCommand::Stop => self.schedules.cancel(),
                ScheduleCommand::StopExecution { execution } => {
                    if self
                        .schedules
                        .running
                        .as_ref()
                        .is_some_and(|r| r.execution == execution)
                    {
                        self.schedules.cancel();
                    } else {
                        return Err("排程已變更，請重新查看任務卡片。".into());
                    }
                }
                ScheduleCommand::Preview {
                    request_id,
                    conversation,
                    template,
                    parameters,
                    due_at,
                    repeat_days,
                    continuous_retry,
                } => {
                    crate::jobs::validate_id(&request_id)?;
                    self.schedule_available()?;
                    if !self.schedule_idle() {
                        return Err("請等目前工作完成後再預覽排程。".into());
                    }
                    if self.active_id.as_deref() != Some(&conversation) {
                        return Err("請在原專案對話重新開啟流程。".into());
                    }
                    let project = self
                        .projects
                        .store
                        .project_for(&conversation)
                        .ok_or("專案對話不存在。")?;
                    let flow =
                        crate::projects::memory::Memory::open(project.clone(), &conversation)?
                            .load_workflow(&template)?;
                    let grant = Grant {
                        target: crate::reminders::WorkflowTarget {
                            conversation,
                            project: project.id.clone(),
                            principal: self.work.store.principal_id.clone(),
                            root_identity: crate::projects::files::directory_identity(
                                &project.root,
                            )?,
                            template: flow.id.clone(),
                            revision: crate::projects::text::revision(
                                &serde_json::to_string(&flow).map_err(|e| e.to_string())?,
                            ),
                        },
                        template: flow,
                        parameters,
                        due_at,
                        repeat_days,
                        continuous_retry,
                    };
                    grant.validate()?;
                    if due_at <= crate::unix_now() {
                        return Err("請選未來的執行時間。".into());
                    }
                    let id = crate::jobs::new_id()?;
                    self.view.post(&json!({"type":"schedule_preview","id":id,"request_id":request_id,"grant":grant,"root":project.root,
                        "notice":"確認即授權此固定流程依時間在本機執行；不經AI改參數。只在程式開啟／托盤且登入有效時執行。忙碌延後最多5分鐘，逾時不補跑；來源仍逐步核對，成果寫受控_AI_Output副本。失敗／未知結果不自動重播。"}))?;
                    self.schedules.preview = Some(Preview {
                        id,
                        grant,
                        expires: Instant::now() + Duration::from_secs(120),
                    });
                    return Ok(());
                }
                ScheduleCommand::Confirm { preview_id } => {
                    if !self.schedule_idle() {
                        return Err("目前忙碌，請重新預覽。".into());
                    }
                    let preview = self.schedules.preview.take().ok_or("請先預覽排程。")?;
                    if preview.id != preview_id
                        || Instant::now() > preview.expires
                        || self.active_id.as_deref() != Some(&preview.grant.target.conversation)
                    {
                        return Err("預覽已過期或對話已變更，請重新預覽。".into());
                    }
                    self.schedule_project(&preview.grant)?;
                    let mut next = self.schedules.store.as_ref().map_err(Clone::clone)?.clone();
                    next.add(preview.grant, crate::unix_now())?;
                    next.save(&self.root)?;
                    self.schedules.store = Ok(next);
                }
                ScheduleCommand::Disable { id } => {
                    self.schedule_available()?;
                    let mut next = self.schedules.store.as_ref().map_err(Clone::clone)?.clone();
                    let entry = next
                        .entries
                        .iter_mut()
                        .find(|entry| {
                            entry.id == id
                                && entry.grant.target.principal == self.work.store.principal_id
                        })
                        .ok_or("此帳號沒有這項排程。")?;
                    entry.enabled = false;
                    entry.status = "使用者停用".into();
                    if entry.execution.as_ref().is_some_and(|id| {
                        self.schedules
                            .running
                            .as_ref()
                            .is_some_and(|run| &run.execution == id)
                    }) {
                        self.schedules.cancel();
                    }
                    next.save(&self.root)?;
                    self.schedules.store = Ok(next);
                    self.schedule_notice(
                        "流程排程已依您的操作停用；可開啟排程清單查看狀態與審計。",
                    );
                }
                ScheduleCommand::Resume { id } => {
                    if !self.schedule_idle() {
                        return Err("請等目前工作結束再續接。".into());
                    }
                    let mut next = self.schedules.store.as_ref().map_err(Clone::clone)?.clone();
                    let entry = next
                        .entries
                        .iter_mut()
                        .find(|entry| entry.id == id)
                        .ok_or("排程不存在。")?;
                    self.schedule_project(&entry.grant)?;
                    if self.projects.active(Some(&entry.grant.target.conversation)) {
                        return Err("請等待原對話工作完成再續接排程。".into());
                    }
                    let execution = entry.execution.clone().ok_or("沒有可續接的執行。")?;
                    Journal::load(&self.root, &execution)?.can_resume(&entry.grant)?;
                    entry.enabled = true;
                    entry.status = "執行中".into();
                    let entry = entry.clone();
                    next.save(&self.root)?;
                    self.schedules.store = Ok(next);
                    self.launch_schedule(entry, execution, true)?;
                }
            }
            self.publish_schedules()
        })();
        if let Err(error) = &result {
            let _ = self
                .view
                .post(&json!({"type":"schedule_error","message":error}));
        }
        result
    }
    fn publish_schedules(&self) -> AppResult<()> {
        let store = self.schedules.store.as_ref().map_err(Clone::clone)?;
        // 跨帳號不公開別人的流程參數、路徑與審計；登入失效只顯示執行中狀態供停止。
        let entries: Vec<_> = store
            .entries
            .iter()
            .filter(|entry| {
                self.logged_in() && entry.grant.target.principal == self.work.store.principal_id
            })
            .collect();
        let audit: Vec<_> = store
            .audit
            .iter()
            .filter(|audit| entries.iter().any(|entry| entry.id == audit.schedule))
            .collect();
        self.view.post(&json!({"type":"schedule_state","entries":entries,"audit":audit,"running":self.schedules.active(),"waiting":self.schedules.running.as_ref().and_then(|run|run.waiting.as_ref())}))
    }
    fn launch_schedule(&mut self, entry: Entry, execution: String, resume: bool) -> AppResult<()> {
        if self.project_slots_full() || self.projects.active(Some(&entry.grant.target.conversation))
        {
            return Err("排程等待可用任務名額或原對話完成。".into());
        }
        let conversation = entry.grant.target.conversation.clone();
        let project = self.schedule_project(&entry.grant)?;
        let root = self.root.clone();
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let (wait_tx, wait_events) = mpsc::channel();
        let (tx, result) = mpsc::channel();
        let run_id = execution.clone();
        crate::reliability::spawn(move || {
            let _resources =
                crate::resource_locks::Context::enter(Some(crate::resource_locks::Context {
                    label: "流程排程".into(),
                    cancel: worker_cancel.clone(),
                    notify: Arc::new(move |message| {
                        let _ = wait_tx.send(message);
                    }),
                }));
            let outcome = schedule::execution::run(
                &root,
                &exe,
                project,
                &entry.grant,
                &run_id,
                &worker_cancel,
                resume,
            );
            let _ = tx.send(outcome);
        });
        self.schedules.running = Some(Running {
            conversation,
            wait_events,
            waiting: None,
            execution,
            cancel,
            result,
        });
        self.status = "正在執行已核准排程；可在流程排程清單停止。草稿已保留。".into();
        self.publish();
        Ok(())
    }
    pub(super) fn tick_schedules(&mut self) {
        if self.smoke || self.demo || !self.ready || Instant::now() < self.schedules.next_check {
            return;
        }
        self.schedules.next_check = Instant::now() + Duration::from_secs(1);
        if !self.logged_in() || self.versions.blocked() {
            self.schedules.cancel();
        }
        let result = self.advance_schedules();
        if let Err(error) = result {
            self.schedules.next_check = Instant::now() + Duration::from_secs(30);
            // 保存／啟動失敗也要通知；同一持續錯誤不每30秒重複彈出。
            if self.schedules.last_error_notice.as_deref() != Some(&error) {
                self.schedules.last_error_notice = Some(error.clone());
                self.schedule_notice(
                    "流程排程未能完成，請開啟排程狀態與審計核對；未自動重播未確認的操作。",
                );
            }
            self.fail(error);
            self.publish();
        } else {
            self.schedules.last_error_notice = None;
        }
    }
    fn advance_schedules(&mut self) -> AppResult<()> {
        let mut changed = false;
        if let Some(run) = self.schedules.running.as_mut() {
            while let Ok(message) = run.wait_events.try_recv() {
                run.waiting = message;
                changed = true;
            }
        }
        if changed {
            self.publish();
            self.publish_schedules()?;
        }
        let mut next = self.schedules.store.as_ref().map_err(Clone::clone)?.clone();
        let now = crate::unix_now();
        if let Some(run) = &self.schedules.running {
            let result = match run.result.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => return Ok(()),
                Err(mpsc::TryRecvError::Disconnected) => Err("執行緒中斷，請核對持久紀錄。".into()),
            };
            let execution = run.execution.clone();
            let entry = next
                .entries
                .iter()
                .find(|entry| entry.execution.as_deref() == Some(&execution))
                .ok_or("執行所屬排程遺失。")?;
            let mut journal = Journal::load(&self.root, &execution)
                .unwrap_or(Journal::new(execution.clone(), &entry.grant)?);
            if let Err(error) = result {
                journal.detail = error;
            }
            next.reconcile(&execution, &journal, now)?;
            next.save(&self.root)?;
            self.schedules.store = Ok(next);
            self.schedules.running = None;
            self.status = format!("流程排程：{}", journal.phase.label());
            self.publish();
            self.schedule_notice(&self.status.clone());
            return self.publish_schedules();
        }
        if !self.schedules.recovered || next.entries.iter().any(|entry| entry.status == "執行中")
        {
            // 重啟僅核對上次紀錄，絕不自動重播未完成的 execution。
            for entry in next
                .entries
                .clone()
                .into_iter()
                .filter(|entry| entry.execution.is_some())
            {
                let execution = entry.execution.as_deref().ok_or("缺少執行ID。")?;
                let journal = Journal::load(&self.root, execution)
                    .unwrap_or(Journal::new(execution.into(), &entry.grant)?);
                next.reconcile(execution, &journal, now)?;
            }
            next.save(&self.root)?;
            self.schedules.store = Ok(next.clone());
            self.schedules.recovered = true;
        }
        if next.skip_missed(now)? {
            next.save(&self.root)?;
            self.schedules.store = Ok(next.clone());
            let _ = self.publish_schedules();
            self.schedule_notice("有流程排程錯過時間而未執行，請查看排程狀態與審計。重複排程保留下次；審計容量已滿則停用並保留舊紀錄。");
        }
        if !self.schedule_idle() || self.schedule_available().is_err() {
            return Ok(());
        }
        let candidate = next
            .entries
            .iter()
            .find(|entry| {
                entry.enabled
                    && !self.projects.active(Some(&entry.grant.target.conversation))
                    && entry.execution.is_none()
                    && entry.grant.target.principal == self.work.store.principal_id
                    && entry.next_due.is_some_and(|due| due <= now)
            })
            .cloned();
        if let Some(entry) = candidate {
            if let Err(error) = self.schedule_project(&entry.grant) {
                let item = next
                    .entries
                    .iter_mut()
                    .find(|item| item.id == entry.id)
                    .ok_or("排程遺失。")?;
                item.enabled = false;
                item.status = error;
                next.save(&self.root)?;
                self.schedules.store = Ok(next);
                self.schedule_notice("流程排程因授權或來源設定改變而停用，請開啟排程清單核對。");
                return self.publish_schedules();
            }
            let (entry, execution) = next.claim(&entry.id, now)?;
            next.save(&self.root)?;
            self.schedules.store = Ok(next);
            self.launch_schedule(entry, execution, false)?;
            self.publish_schedules()?;
        }
        Ok(())
    }

    fn schedule_notice(&mut self, message: &str) {
        use crate::notification_preferences::{Category, Delivery};
        self.popup(
            Category::Schedule,
            message,
            Delivery::Both,
            super::popup::Target::Schedule,
        );
    }

    /// 通知僅導覽到目前帳號可看的審計；舊帳號內容不放進通知點擊參數。
    pub(super) fn open_schedule_notice(&mut self) -> bool {
        if !std::mem::take(&mut self.schedules.notice) {
            return false;
        }
        let _ = self.view.post(&json!({"type":"show_schedule_audit"}));
        true
    }
}
