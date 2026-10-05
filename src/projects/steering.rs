//! 使用者在執行途中追加的指示。只接受 UI 的明確提交，文件文字不能寫入此佇列。
//! 短暫互斥鎖定義操作開始／任務結束的界線，不在模型或工具等待期間鎖住 UI。
use crate::{jobs, storage, AppResult};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Instruction {
    pub id: String,
    pub text: String,
    /// pending 可修改／撤回；staged 已接收，sent 已帶入模型請求。
    pub status: String,
}
#[derive(Default, Clone, Serialize, Deserialize)]
struct State {
    entries: Vec<Instruction>,
    #[serde(skip)]
    closed: bool,
}
#[derive(Clone)]
pub struct Inbox {
    path: PathBuf,
    state: Arc<Mutex<State>>,
}
impl Inbox {
    pub fn open(root: &Path, id: &str) -> AppResult<Self> {
        jobs::validate_id(id)?;
        let path = root
            .join("project-runs")
            .join(format!("{id}.instructions.dpapi"));
        let state = if path.exists() {
            if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 150_000 {
                return Err("補充指示紀錄過大。".into());
            }
            let bytes = storage::protect(&std::fs::read(&path).map_err(|e| e.to_string())?, false)?;
            serde_json::from_slice(&bytes).map_err(|_| "補充指示紀錄無法解析。")?
        } else {
            State::default()
        };
        Ok(Self {
            path,
            state: Arc::new(Mutex::new(state)),
        })
    }
    /// 寫入成功才更新記憶體狀態；磁碟錯誤不宣稱指示已接收。
    fn change<T>(&self, edit: impl FnOnce(&mut State) -> AppResult<T>) -> AppResult<T> {
        let mut guard = self.state.lock().map_err(|_| "補充指示狀態無法存取。")?;
        let mut next = guard.clone();
        let value = edit(&mut next)?;
        let bytes = serde_json::to_vec(&next).map_err(|e| e.to_string())?;
        storage::atomic_write(&self.path, &storage::protect(&bytes, true)?)?;
        *guard = next;
        Ok(value)
    }
    pub fn entries(&self) -> AppResult<Vec<Instruction>> {
        Ok(self
            .state
            .lock()
            .map_err(|_| "補充指示狀態無法存取。")?
            .entries
            .clone())
    }
    pub fn submit(&self, id: Option<&str>, text: &str) -> AppResult<String> {
        let text = text.trim();
        if text.is_empty() || text.chars().count() > 4000 {
            return Err("補充指示需為 1–4000 字。".into());
        }
        self.change(|state| {
            if state.closed {
                return Err("任務正在結束，補充尚未送出；請在下一則對話提交。".into());
            }
            if let Some(id) = id {
                let total = state
                    .entries
                    .iter()
                    .filter(|e| e.id != id)
                    .map(|e| e.text.len())
                    .sum::<usize>()
                    + text.len();
                if total > 48_000 {
                    return Err("本次補充指示合計不可超過 48 KB。".into());
                }
                let entry = state
                    .entries
                    .iter_mut()
                    .find(|e| e.id == id && e.status == "pending")
                    .ok_or("這則指示已被接收，請另送一則修正。")?;
                entry.text = text.into();
                return Ok(id.into());
            }
            if state.entries.len() >= 30
                || state.entries.iter().map(|e| e.text.len()).sum::<usize>() + text.len() > 48_000
            {
                return Err("本次任務補充指示已達 30 則或 48 KB，請完成後另開任務。".into());
            }
            let id = jobs::new_id()?;
            state.entries.push(Instruction {
                id: id.clone(),
                text: text.into(),
                status: "pending".into(),
            });
            Ok(id)
        })
    }
    pub fn withdraw(&self, id: &str) -> AppResult<()> {
        self.change(|state| {
            let entry = state
                .entries
                .iter_mut()
                .find(|e| e.id == id && e.status == "pending")
                .ok_or("指示已被接收，無法撤回；請另外補充修正。")?;
            entry.status = "withdrawn".into();
            Ok(())
        })
    }
    /// 有新指示便停止尚未執行的舊決策。沒有新指示時，terminal 原子關閉入口，
    /// 防止最後一次檢查與完成通知之間接受一則永遠無法處理的訊息。
    pub fn boundary(&self, terminal: bool) -> AppResult<Vec<Instruction>> {
        self.change(|state| {
            let mut entries = Vec::new();
            for entry in &mut state.entries {
                if entry.status == "pending" {
                    entry.status = "staged".into();
                    entries.push(entry.clone());
                }
            }
            if terminal && entries.is_empty() {
                state.closed = true;
            }
            Ok(entries)
        })
    }
    pub fn mark_sent(&self) -> AppResult<()> {
        self.change(|state| {
            for entry in &mut state.entries {
                if entry.status == "staged" {
                    entry.status = "sent".into();
                }
            }
            Ok(())
        })
    }
    pub fn close(&self, closed: bool) -> AppResult<()> {
        self.change(|state| {
            state.closed = closed;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn late_messages_are_not_accepted_after_terminal_boundary() {
        let root = std::env::temp_dir().join(format!("lmai-steering-{}", jobs::new_id().unwrap()));
        let inbox = Inbox::open(&root, "test").unwrap();
        let id = inbox.submit(None, "只看第二台設備").unwrap();
        inbox.submit(Some(&id), "只看 Device02").unwrap();
        assert_eq!(inbox.boundary(true).unwrap()[0].text, "只看 Device02");
        assert!(inbox.withdraw(&id).is_err());
        inbox.mark_sent().unwrap();
        assert!(inbox.boundary(true).unwrap().is_empty());
        assert!(inbox.submit(None, "太晚了").is_err());
        let restored = Inbox::open(&root, "test").unwrap();
        assert_eq!(restored.entries().unwrap()[0].status, "sent");
        let new = restored.submit(None, "新補充").unwrap();
        restored.withdraw(&new).unwrap();
        assert!(restored.boundary(false).unwrap().is_empty());
    }
}
