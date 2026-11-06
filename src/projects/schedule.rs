//! 已核准流程的本機排程資料與持久執行狀態；不經模型重寫參數。
//! 只由開啟中的桌面計時器啟動，未知結果不自動重播。
use super::{files::SavedBroker, text, workflows::Template};
use crate::{jobs, storage, AppResult};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::Path};
pub mod execution;
#[cfg(test)]
mod tests;

const STORE_FILE: &str = "workflow-schedules.dpapi";
pub const GRACE_SECONDS: u64 = 300;

#[derive(Clone, Serialize, Deserialize)]
pub struct Grant {
    pub target: crate::reminders::WorkflowTarget,
    pub template: Template,
    pub parameters: BTreeMap<String, Value>,
    pub due_at: u64,
    pub repeat_days: u32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub continuous_retry: bool,
}
impl Grant {
    pub fn validate(&self) -> AppResult<()> {
        jobs::validate_id(&self.target.conversation)?;
        jobs::validate_id(&self.target.project)?;
        self.template.validate()?;
        self.template.validate_parameters(&self.parameters)?;
        if self.template.id != self.target.template
            || text::revision(&serde_json::to_string(&self.template).map_err(|e| e.to_string())?)
                != self.target.revision
            || self.target.principal.is_empty()
            || self.target.principal.len() > 200
            || !matches!(self.repeat_days, 0 | 1 | 7)
            || self.due_at == 0
            || self.due_at > 253_402_300_799
            || serde_json::to_vec(&self.parameters)
                .map_err(|e| e.to_string())?
                .len()
                > 32_000
        {
            return Err("排程授權、時間、範本或參數不完整，請重新預覽。".into());
        }
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub grant: Grant,
    pub next_due: Option<u64>,
    pub enabled: bool,
    pub execution: Option<String>,
    pub status: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Audit {
    pub execution: String,
    pub schedule: String,
    pub due_at: u64,
    pub started_at: u64,
    pub finished_at: Option<u64>,
    pub status: String,
    pub next_step: usize,
    pub outputs: Vec<String>,
    pub detail: String,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Store {
    pub entries: Vec<Entry>,
    pub audit: Vec<Audit>,
}
impl Store {
    pub fn load(root: &Path) -> AppResult<Self> {
        let store: Self = match read(&root.join(STORE_FILE)) {
            Ok(store) => store,
            Err(error) if !root.join(STORE_FILE).exists() => {
                // 僅真正不存在時採空白；讀取／解密失敗不可覆寫原資料。
                if error.starts_with("not_found:") {
                    Self::default()
                } else {
                    return Err(error);
                }
            }
            Err(error) => return Err(error),
        };
        store.validate()?;
        Ok(store)
    }
    fn validate(&self) -> AppResult<()> {
        if self.entries.len() > 50 || self.audit.len() > 100 {
            return Err("排程或審計已達容量上限。".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for entry in &self.entries {
            jobs::validate_id(&entry.id)?;
            entry.grant.validate()?;
            if !ids.insert(&entry.id) {
                return Err("排程識別碼重複。".into());
            }
        }
        let mut executions = std::collections::BTreeSet::new();
        for audit in &self.audit {
            jobs::validate_id(&audit.execution)?;
            if !executions.insert(&audit.execution) || !ids.contains(&audit.schedule) {
                return Err("審計識別碼或所屬排程不符。".into());
            }
        }
        Ok(())
    }
    pub fn save(&self, root: &Path) -> AppResult<()> {
        self.validate()?;
        write(&root.join(STORE_FILE), self)
    }
    pub fn add(&mut self, grant: Grant, now: u64) -> AppResult<String> {
        grant.validate()?;
        if self.entries.len() >= 50 || grant.due_at <= now || grant.due_at - now > 10 * 366 * 86400
        {
            return Err("最多50項排程；首次時間須在未來十年內。".into());
        }
        let id = jobs::new_id()?;
        self.entries.push(Entry {
            id: id.clone(),
            next_due: Some(grant.due_at),
            grant,
            enabled: true,
            execution: None,
            status: "等待排程".into(),
        });
        Ok(id)
    }
    /// 到期超過五分鐘不補跑寫入。重複排程直接前進至未來一次，不逐次補跑。
    pub fn skip_missed(&mut self, now: u64) -> AppResult<bool> {
        let mut changed = false;
        for entry in &mut self.entries {
            if entry.enabled
                && entry.execution.is_none()
                && entry
                    .next_due
                    .is_some_and(|due| now.saturating_sub(due) > GRACE_SECONDS)
            {
                // 未派送工具也要留下可由通知查閱的紀錄；不偽裝成一次成功執行。
                // 容量滿時保留全部舊紀錄並明確停用，不能默默刪除審計。
                if self.audit.len() >= 100 {
                    entry.enabled = false;
                    entry.status = "審計已達100筆，未執行並停用；請由維護者封存".into();
                    changed = true;
                    continue;
                }
                self.audit.push(Audit {
                    execution: jobs::new_id()?,
                    schedule: entry.id.clone(),
                    due_at: entry.next_due.unwrap_or(now),
                    started_at: now,
                    finished_at: Some(now),
                    status: "錯過時間，已略過本次".into(),
                    next_step: 0,
                    outputs: vec![],
                    detail: "超過五分鐘等待期限；可能未登入、程式忙碌或程式未開啟。本次未派送任何工具，不補跑寫入。".into(),
                });
                entry.next_due =
                    next_due(entry.next_due.unwrap_or(now), entry.grant.repeat_days, now);
                entry.enabled = entry.next_due.is_some();
                entry.status = "已錯過時間，未執行；不補跑寫入".into();
                changed = true;
            }
        }
        Ok(changed)
    }
    /// 必須將回傳的新 Store 保存成功，才可啟動工作執行緒。
    pub fn claim(&mut self, id: &str, now: u64) -> AppResult<(Entry, String)> {
        if self.audit.len() >= 100 {
            return Err("已保存100次執行審計，請先由維護者封存；未啟動更多工作。".into());
        }
        if self.entries.iter().any(|entry| entry.status == "執行中") {
            return Err("已有排程執行中。".into());
        }
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.id == id)
            .ok_or("排程不存在。")?;
        let due = entry.next_due.ok_or("排程沒有下次時間。")?;
        if !entry.enabled || entry.execution.is_some() || due > now || now - due > GRACE_SECONDS {
            return Err("排程未到期、已停用或已認領。".into());
        }
        let execution = jobs::new_id()?;
        entry.execution = Some(execution.clone());
        entry.status = "執行中".into();
        self.audit.push(Audit {
            execution: execution.clone(),
            schedule: id.into(),
            due_at: due,
            started_at: now,
            finished_at: None,
            status: "已認領".into(),
            next_step: 0,
            outputs: vec![],
            detail: String::new(),
        });
        Ok((entry.clone(), execution))
    }
    pub fn reconcile(&mut self, execution: &str, journal: &Journal, now: u64) -> AppResult<()> {
        let audit = self
            .audit
            .iter_mut()
            .find(|audit| audit.execution == execution)
            .ok_or("執行審計不存在。")?;
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.id == audit.schedule)
            .ok_or("排程不存在。")?;
        if entry.execution.as_deref() != Some(execution)
            || journal.execution != execution
            || journal.grant_revision != grant_revision(&entry.grant)?
        {
            return Err("執行身分或授權版本不符。".into());
        }
        audit.status = journal.phase.label().into();
        audit.next_step = journal.next;
        audit.outputs = journal.outputs.clone();
        audit.detail = journal.detail.clone();
        audit.finished_at = Some(now);
        entry.status = audit.status.clone();
        // Skipped只能表示任何步驟派送前的檢查；拒絕矛盾的持久紀錄。
        let safe_skip = journal.phase == Phase::Skipped
            && journal.next == 0
            && journal.outputs.is_empty()
            && journal.broker.is_none();
        if journal.phase == Phase::Completed || safe_skip {
            entry.execution = None;
            entry.next_due = next_due(audit.due_at, entry.grant.repeat_days, now);
            entry.enabled = entry.enabled && entry.next_due.is_some();
        } else {
            entry.enabled = false;
        }
        Ok(())
    }
}
fn next_due(due: u64, repeat: u32, now: u64) -> Option<u64> {
    if repeat == 0 {
        return None;
    }
    let interval = u64::from(repeat) * 86400;
    due.checked_add((now.saturating_sub(due) / interval + 1) * interval)
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Preparing,
    Ready,
    InFlight,
    Blocked,
    Completed,
    Skipped,
}
impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Preparing => "準備中斷，需檢查",
            Self::Ready => "已暫停，可從未執行步驟續接",
            Self::InFlight => "執行結果未知，禁止重播",
            Self::Blocked => "步驟失敗，禁止自動重播",
            Self::Completed => "完成",
            Self::Skipped => "執行前條件不符，已略過本次",
        }
    }
}
#[derive(Serialize, Deserialize)]
pub struct Journal {
    pub execution: String,
    pub grant_revision: String,
    pub phase: Phase,
    pub next: usize,
    pub outputs: Vec<String>,
    pub detail: String,
    pub(super) broker: Option<SavedBroker>,
}
impl Journal {
    /// 第一次執行使用 create_new；同一 execution 即使已完成也不可覆寫或重跑。
    pub fn create(&self, root: &Path) -> AppResult<()> {
        use std::io::Write;
        jobs::validate_id(&self.execution)?;
        let directory = root.join("workflow-executions");
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let bytes = storage::protect(&serde_json::to_vec(self).map_err(|e| e.to_string())?, true)?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join(format!("{}.dpapi", self.execution)))
            .map_err(|e| format!("執行紀錄已存在或無法建立，未重播：{e}"))?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|e| e.to_string())
    }
    pub fn new(execution: String, grant: &Grant) -> AppResult<Self> {
        Ok(Self {
            execution,
            grant_revision: grant_revision(grant)?,
            phase: Phase::Preparing,
            next: 0,
            outputs: vec![],
            detail: String::new(),
            broker: None,
        })
    }
    pub fn load(root: &Path, execution: &str) -> AppResult<Self> {
        jobs::validate_id(execution)?;
        let result: Self = read(
            &root
                .join("workflow-executions")
                .join(format!("{execution}.dpapi")),
        )?;
        if result.execution != execution {
            return Err("執行紀錄身分不符。".into());
        }
        Ok(result)
    }
    pub fn save(&self, root: &Path) -> AppResult<()> {
        jobs::validate_id(&self.execution)?;
        let directory = root.join("workflow-executions");
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        write(&directory.join(format!("{}.dpapi", self.execution)), self)
    }
    pub fn can_resume(&self, grant: &Grant) -> AppResult<()> {
        if self.phase != Phase::Ready
            || self.broker.is_none()
            || self.grant_revision != grant_revision(grant)?
        {
            return Err("只可續接已持久保存、尚未執行的下一步；未知／失敗步驟不得重播。".into());
        }
        Ok(())
    }
}
pub fn grant_revision(grant: &Grant) -> AppResult<String> {
    Ok(text::revision(
        &serde_json::to_string(grant).map_err(|e| e.to_string())?,
    ))
}
fn read<T: DeserializeOwned>(path: &Path) -> AppResult<T> {
    let meta = fs::metadata(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            format!("not_found:{e}")
        } else {
            e.to_string()
        }
    })?;
    if meta.len() > 64 * 1024 * 1024 {
        return Err("排程持久資料超過64MB，原檔已保留。".into());
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    serde_json::from_slice(&storage::protect(&bytes, false)?)
        .map_err(|_| "排程資料損毀或版本不符，原檔已保留。".into())
}
fn write(path: &Path, value: &impl Serialize) -> AppResult<()> {
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    if bytes.len() > 60 * 1024 * 1024 {
        return Err("排程持久資料超過60MB，尚未執行下一步。".into());
    }
    storage::atomic_write(path, &storage::protect(&bytes, true)?)
}
