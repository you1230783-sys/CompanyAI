//! 以目前帳號的 DPAPI 佇列派送一般專案任務。所有命令均核對原生專案與任務身分。
use super::*;
use crate::projects::batch::{self, Entry, Queue, State};
use chrono::{Duration as LocalDuration, NaiveDate, NaiveDateTime};

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub(in crate::ui) enum Command {
    Enqueue {
        conversation: String,
        text: String,
    },
    ContinueTonight {
        conversation: String,
        run_id: String,
    },
    Edit {
        id: String,
        text: String,
    },
    Move {
        id: String,
        earlier: bool,
    },
    Cancel {
        id: String,
    },
    ReadSummary {
        window: NaiveDate,
    },
}

#[derive(Default)]
pub(super) struct Runtime {
    principal: String,
    queue: Queue,
    checked: Option<Instant>,
    awake: Option<crate::keep_awake::KeepAwake>,
}

fn draft_text(text: &str) -> AppResult<String> {
    let text = text.trim();
    if text.is_empty() || text.len() > 48_000 {
        return Err("批次要求不可空白或超過 48 KB。".into());
    }
    Ok(text.into())
}

impl App {
    fn batch_account(&self) -> Option<&str> {
        let id = self.work.store.principal_id.as_str();
        (self.logged_in()
            && !self.versions.blocked()
            && !self.smoke
            && !self.demo
            && self
                .work
                .caps
                .as_ref()
                .is_some_and(|caps| caps.principal_id == id && caps.supports("background"))
            && !id.is_empty())
        .then_some(id)
    }

    fn load_batch_account(&mut self) -> AppResult<bool> {
        let Some(principal) = self.batch_account().map(str::to_owned) else {
            self.projects.batch = Runtime::default();
            return Ok(false);
        };
        if self.projects.batch.principal != principal {
            let queue = Queue::load(&self.root, &principal)?;
            self.projects.batch = Runtime {
                principal,
                queue,
                ..Default::default()
            };
        }
        Ok(true)
    }

    fn batch_commit(&mut self, queue: Queue) -> AppResult<()> {
        let principal = &self.projects.batch.principal;
        if self.batch_account() != Some(principal) {
            return Err("晚間批次的登入帳號已變更。".into());
        }
        queue.save(&self.root, principal)?;
        self.projects.batch.queue = queue;
        Ok(())
    }

    pub(super) fn batch_state(&self) -> serde_json::Value {
        let runtime = &self.projects.batch;
        if self.batch_account() != Some(runtime.principal.as_str()) {
            return serde_json::Value::Null;
        }
        let window = batch::local_now()
            .ok()
            .and_then(|now| recent_window(&runtime.queue, now));
        let unread = window.filter(|window| runtime.queue.seen.is_none_or(|seen| seen < *window));
        let entries = runtime.queue.entries.iter().map(|entry| {
            let name = self.projects.store.projects.iter().find(|project| project.id == entry.project)
                .map_or("原專案", |project| project.name.as_str());
            json!({"id":entry.id,"conversation":entry.conversation,"project":name,"title":entry.title,
                "draft":entry.draft.as_ref().filter(|_| runtime.queue.editable(&entry.id).is_ok()),
                "state":entry.state,"detail":entry.detail,"ready":entry.ready,"window":entry.window,
                "editable":runtime.queue.editable(&entry.id).is_ok()})
        }).collect::<Vec<_>>();
        json!({"entries":entries,"unread":unread,"window":window})
    }

    pub(super) fn batch_command(&mut self, command: Command) -> AppResult<()> {
        if !self.load_batch_account()? {
            return Err("請登入並完成帳號同步後使用晚間批次。".into());
        }
        let mut queue = self.projects.batch.queue.clone();
        match command {
            Command::Enqueue { conversation, text } => {
                self.enqueue_batch(&mut queue, &conversation, Some(draft_text(&text)?), None)?;
            }
            Command::ContinueTonight {
                conversation,
                run_id,
            } => {
                self.enqueue_batch(&mut queue, &conversation, None, Some(run_id))?;
            }
            Command::Edit { id, text } => {
                let index = queue.editable(&id)?;
                let entry = &mut queue.entries[index];
                if entry.draft.is_none() {
                    return Err("暫停任務以原 checkpoint 續接。".into());
                }
                let text = draft_text(&text)?;
                entry.title = text.chars().take(80).collect();
                entry.draft = Some(text);
            }
            Command::Move { id, earlier } => {
                let index = queue.editable(&id)?;
                let next = |entry: &Entry| entry.state == State::Queued && entry.window.is_none();
                let adjacent = if earlier {
                    queue.entries[..index].iter().rposition(next)
                } else {
                    queue.entries[index + 1..]
                        .iter()
                        .position(next)
                        .map(|offset| index + 1 + offset)
                };
                if let Some(adjacent) = adjacent {
                    queue.entries.swap(index, adjacent);
                }
            }
            Command::Cancel { id } => {
                let index = queue.editable(&id)?;
                queue.entries.remove(index);
            }
            Command::ReadSummary { window } => {
                if recent_window(&queue, batch::local_now()?) != Some(window) {
                    return Err("批次總覽已變更，請重新開啟。".into());
                }
                queue.seen = Some(window);
            }
        }
        self.batch_commit(queue)
    }

    fn enqueue_batch(
        &self,
        queue: &mut Queue,
        conversation: &str,
        draft: Option<String>,
        resume: Option<String>,
    ) -> AppResult<()> {
        if self.active_id.as_deref() != Some(conversation) {
            return Err("請在原專案對話排入晚間批次。".into());
        }
        let project = self
            .projects
            .store
            .project_for(conversation)
            .ok_or("此對話沒有專案授權。")?;
        let ready = batch::ready_at(batch::local_now()?, crate::jobs::random_u32()?);
        let expires_at = self.session.as_ref().ok_or("請先登入。")?.expires_at;
        if expires_at <= batch::unix_window_end(ready)? {
            return Err("目前登入有效期不足以涵蓋批次窗口（至早上 07:00），請先重新登入。".into());
        }
        if let Some(id) = &resume {
            if self.projects.active(Some(conversation))
                || !self.messages.last().is_some_and(|message| {
                    message.project_paused
                        && message.request_id.as_ref() == Some(id)
                        && message.content.contains("可改為今晚批次處理")
                })
                || !projects::runner::paused_for(
                    &self.root,
                    id,
                    conversation,
                    &self.projects.batch.principal,
                )
            {
                return Err("原暫停任務已變更，請在原對話重新確認。".into());
            }
        }
        let id = crate::jobs::new_id()?;
        let title = draft.as_ref().map_or_else(
            || "續接未完成的專案任務".into(),
            |text| text.chars().take(80).collect(),
        );
        queue.enqueue(Entry {
            run_id: resume.clone().unwrap_or_else(|| id.clone()),
            id,
            conversation: conversation.into(),
            project: project.id.clone(),
            root_identity: projects::files::directory_identity(&project.root)?,
            title,
            draft,
            plan_mode: self.projects.store.plan_mode,
            resume: resume.is_some(),
            state: State::Queued,
            ready,
            window: None,
            detail: String::new(),
        })
    }

    pub(in crate::ui) fn batch_tick(&mut self) -> AppResult<bool> {
        if self
            .projects
            .batch
            .checked
            .is_some_and(|last| last.elapsed() < Duration::from_secs(1))
        {
            return Ok(false);
        }
        if !self.load_batch_account()? {
            return Ok(false);
        }
        let account_changed = self.projects.batch.checked.is_none();
        self.projects.batch.checked = Some(Instant::now());
        let now = batch::local_now()?;
        let window = batch::active_window(now);
        let awake = window.is_some() && self.projects.batch.queue.pending_count() > 0;
        match (awake, self.projects.batch.awake.is_some()) {
            (true, false) => {
                self.projects.batch.awake = Some(crate::keep_awake::KeepAwake::watch(|| true)?)
            }
            (false, true) => self.projects.batch.awake = None,
            _ => (),
        }
        let mut changed = account_changed | self.recover_batch_run(now, window)?;
        if let Some(window) = window.filter(|_| !self.project_slots_full()) {
            changed |= self.dispatch_batch(now, window)?;
        }
        if let Some(window) = recent_window(&self.projects.batch.queue, now).filter(|window| {
            self.projects
                .batch
                .queue
                .notified
                .is_none_or(|old| old < *window)
        }) {
            let mut queue = self.projects.batch.queue.clone();
            queue.notified = Some(window);
            self.batch_commit(queue)?;
            if self.config.notification_popups && !self.is_foreground() {
                self.desktop_notice("昨晚批次結果已整理，可在晚間批次佇列查看。");
            }
            changed = true;
        }
        Ok(changed)
    }

    /// 結束在啟動及完成落盤之間的程序時，只有原生 checkpoint 能授權續接。
    fn recover_batch_run(
        &mut self,
        now: NaiveDateTime,
        window: Option<NaiveDate>,
    ) -> AppResult<bool> {
        let mut queue = self.projects.batch.queue.clone();
        let Some(entry) = queue.entries.iter_mut().find(|entry| {
            entry.state == State::Running && self.projects.run(&entry.run_id).is_none()
        }) else {
            return Ok(false);
        };
        let principal = &self.projects.batch.principal;
        let safe =
            projects::runner::paused_for(&self.root, &entry.run_id, &entry.conversation, principal);
        let history = self
            .archive
            .conversations
            .iter()
            .find(|chat| chat.id == entry.conversation);
        let last = history
            .and_then(|chat| chat.messages.last())
            .filter(|msg| msg.request_id.as_deref() == Some(&entry.run_id));
        if let Some(message) = last.filter(|msg| msg.role == "assistant" && !msg.project_paused) {
            let status = message
                .project_analysis
                .as_ref()
                .and_then(|analysis| analysis.recovery["state"].as_str());
            entry.state = match status {
                Some("completed") => State::Completed,
                _ if message.content.starts_with("需要你的補充：") => State::WaitingUser,
                _ => State::Failed,
            };
            entry.detail = "依原生任務紀錄還原結果。".into();
        } else if safe
            || (!entry.resume
                && history.is_some_and(|chat| {
                    !chat
                        .messages
                        .iter()
                        .any(|msg| msg.request_id.as_deref() == Some(&entry.run_id))
                }))
        {
            entry.resume = safe;
            entry.state = State::Queued;
            entry.detail = "因程式關閉中斷，已保留續接位置。".into();
            if window.is_some() && window == entry.window {
                entry.ready = now;
            } else {
                entry.ready = batch::ready_at(now, crate::jobs::random_u32()?);
            }
        } else {
            entry.state = State::Failed;
            entry.detail = "因程式關閉中斷，找不到可安全續接的任務紀錄。".into();
        }
        self.batch_commit(queue)?;
        Ok(true)
    }

    fn dispatch_batch(&mut self, now: NaiveDateTime, window: NaiveDate) -> AppResult<bool> {
        let mut queue = self.projects.batch.queue.clone();
        let Some(index) = queue.next_ready(now) else {
            return Ok(false);
        };
        let entry = &mut queue.entries[index];
        if self.projects.active(Some(&entry.conversation))
            || self.schedules.active_conversation(&entry.conversation)
        {
            return Ok(false);
        }
        entry.state = State::Running;
        entry.window = Some(window);
        entry.detail.clear();
        let claimed = entry.clone();
        self.batch_commit(queue)?;
        if let Err(error) = self.launch_batch(&claimed) {
            self.batch_finished(&claimed.run_id, &error, false, false, false)?;
        }
        Ok(true)
    }

    fn launch_batch(&mut self, entry: &Entry) -> AppResult<()> {
        let project = self
            .projects
            .store
            .project_for(&entry.conversation)
            .ok_or("原專案授權已移除。")?;
        if project.id != entry.project
            || projects::files::directory_identity(&project.root)? != entry.root_identity
        {
            return Err("原專案根目錄已變更，請重新排入。".into());
        }
        let chat = self
            .archive
            .conversations
            .iter()
            .find(|chat| chat.id == entry.conversation)
            .ok_or("原對話已移除。")?;
        let mut messages = chat.messages.clone();
        let resume = if entry.resume {
            if !messages.last().is_some_and(|message| {
                message.project_paused && message.request_id.as_deref() == Some(&entry.run_id)
            }) || !projects::runner::paused_for(
                &self.root,
                &entry.run_id,
                &entry.conversation,
                &self.projects.batch.principal,
            ) {
                return Err("原暫停任務已變更，請在對話確認。".into());
            }
            Some(entry.run_id.clone())
        } else {
            messages.push(Message::user(
                entry.draft.as_deref().ok_or("缺少批次草稿。")?,
            ));
            None
        };
        self.launch_project(
            entry.conversation.clone(),
            messages,
            resume,
            None,
            Some(entry),
        )
    }

    pub(super) fn batch_finished(
        &mut self,
        id: &str,
        text: &str,
        paused: bool,
        cancelled: bool,
        succeeded: bool,
    ) -> AppResult<()> {
        let mut queue = self.projects.batch.queue.clone();
        let Some(entry) = queue
            .entries
            .iter_mut()
            .find(|entry| entry.run_id == id && entry.state == State::Running)
        else {
            return Ok(());
        };
        entry.detail = if cancelled {
            "任務已手動停止。".into()
        } else {
            text.chars().take(220).collect()
        };
        entry.state = if paused && text.contains(batch::WAIT_FILE) {
            let now = batch::local_now()?;
            entry.resume = true;
            let check = now + LocalDuration::minutes(batch::OFFICE_RECHECK_MINUTES);
            entry.ready = if batch::active_window(check) == entry.window {
                check
            } else {
                batch::next_evening(check, crate::jobs::random_u32()?)
            };
            State::OfficeWait
        } else if cancelled || text.contains("已自動重試 50 次") {
            State::Failed
        } else if paused || text.starts_with("需要你的補充：") {
            State::WaitingUser
        } else if succeeded {
            State::Completed
        } else {
            State::Failed
        };
        self.batch_commit(queue)
    }

    /// 到期先保存原因，再由既有登出流程取消執行。其他未派送項目延至下一晚。
    pub(in crate::ui) fn batch_login_expired(&mut self) -> AppResult<()> {
        let principal = &self.projects.batch.principal;
        if principal.is_empty() || self.work.store.principal_id != *principal {
            return Ok(());
        }
        let now = batch::local_now()?;
        let Some(window) = batch::active_window(now) else {
            return Ok(());
        };
        let mut queue = self.projects.batch.queue.clone();
        for entry in queue
            .entries
            .iter_mut()
            .filter(|entry| entry.state.pending() && entry.ready < batch::window_end(window))
        {
            entry.window = Some(window);
            entry.detail = "登入已到期，晚間批次已停止。".into();
            if entry.state == State::Running {
                entry.state = State::Failed;
            } else {
                entry.ready = batch::next_evening(now, crate::jobs::random_u32()?);
            }
        }
        queue.save(&self.root, principal)?;
        self.projects.batch.queue = queue;
        Ok(())
    }

    pub(in crate::ui) fn confirm_batch_exit(&self) -> bool {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            MessageBoxW, IDYES, MB_DEFBUTTON2, MB_ICONQUESTION, MB_YESNO,
        };
        let count = self.projects.batch.queue.pending_count();
        if count == 0 || self.batch_account() != Some(self.projects.batch.principal.as_str()) {
            return true;
        }
        let message: Vec<u16> =
            format!("有 {count} 個晚間批次任務尚未完成，離開後不會執行。要離開 LM_AI 嗎？\0")
                .encode_utf16()
                .collect();
        let title: Vec<u16> = "晚間批次\0".encode_utf16().collect();
        (unsafe {
            MessageBoxW(
                self.window,
                message.as_ptr(),
                title.as_ptr(),
                MB_ICONQUESTION | MB_YESNO | MB_DEFBUTTON2,
            )
        }) == IDYES
    }
}

fn recent_window(queue: &Queue, now: NaiveDateTime) -> Option<NaiveDate> {
    queue
        .entries
        .iter()
        .filter_map(|entry| entry.window)
        .filter(|window| batch::window_end(*window) <= now)
        .filter(|window| {
            !queue
                .entries
                .iter()
                .any(|entry| entry.window == Some(*window) && entry.state == State::Running)
        })
        .max()
}
