//! 單次本機提醒。時間保存為Unix秒，由介面依Windows目前時區顯示。
//! 不依賴登入、AI或排程服務；退出程式期間到期的項目於下次啟動補送。
use crate::{storage, AppResult};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs, path::Path};

const MAX_ITEMS: usize = 200;
const FILE: &str = "reminders.dpapi";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    Pet,
    Windows,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Entry {
    pub id: u64,
    pub title: String,
    pub due_at: u64,
    pub channel: Channel,
    pub notified_at: Option<u64>,
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct Store {
    next_id: u64,
    pub entries: Vec<Entry>,
}

impl Store {
    /// 損毀或無法解密時回報錯誤，避免將使用者的既有提醒覆寫為空白。
    pub fn load(root: &Path) -> AppResult<Self> {
        let path = root.join(FILE);
        let metadata = match fs::metadata(&path) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default())
            }
            Err(error) => return Err(format!("無法讀取提醒事項：{error}")),
        };
        if metadata.len() > 262_144 {
            return Err("提醒事項檔案過大，請先保留檔案並聯絡維護者。".into());
        }
        let bytes = fs::read(path).map_err(|e| format!("無法讀取提醒事項：{e}"))?;
        let store: Self = serde_json::from_slice(&storage::protect(&bytes, false)?)
            .map_err(|_| "提醒事項格式錯誤，既有檔案已保留。".to_owned())?;
        store.validate()?;
        Ok(store)
    }

    pub fn save(&self, root: &Path) -> AppResult<()> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        storage::atomic_write(&root.join(FILE), &storage::protect(&bytes, true)?)
    }

    fn validate(&self) -> AppResult<()> {
        let mut ids = BTreeSet::new();
        if self.entries.len() > MAX_ITEMS
            || self.entries.iter().any(|entry| {
                entry.id == 0
                    || entry.id > self.next_id
                    || !ids.insert(entry.id)
                    || valid_title(&entry.title).is_err()
                    || entry.due_at == 0
                    || entry.due_at > 253_402_300_799
            })
        {
            return Err("提醒事項內容不完整，既有資料已保留。".into());
        }
        Ok(())
    }

    /// 編輯已提醒的項目會重新排程；只有使用者明確保存新時間才會再次提醒。
    pub fn upsert(
        &mut self,
        id: Option<u64>,
        title: &str,
        due_at: u64,
        channel: Channel,
        now: u64,
    ) -> AppResult<()> {
        let title = title.trim();
        valid_title(title)?;
        if due_at <= now || due_at.saturating_sub(now) > 10 * 366 * 24 * 3600 {
            return Err("請選擇未來十年內的提醒時間。".into());
        }
        let entry = if let Some(id) = id {
            self.entries
                .iter_mut()
                .find(|entry| entry.id == id)
                .ok_or("找不到此提醒事項，請重新開啟清單。")?
        } else {
            if self.entries.len() >= MAX_ITEMS {
                return Err("最多保存200項提醒，請先刪除不需要的紀錄。".into());
            }
            let next = self.next_id.checked_add(1).ok_or("提醒事項編號已用盡。")?;
            self.next_id = next;
            self.entries.push(Entry {
                id: next,
                title: String::new(),
                due_at,
                channel,
                notified_at: None,
            });
            self.entries.last_mut().ok_or("無法建立提醒事項。")?
        };
        entry.title = title.into();
        entry.due_at = due_at;
        entry.channel = channel;
        entry.notified_at = None;
        Ok(())
    }

    pub fn delete(&mut self, id: u64) -> AppResult<()> {
        let index = self
            .entries
            .iter()
            .position(|entry| entry.id == id)
            .ok_or("找不到此提醒事項。")?;
        self.entries.remove(index);
        Ok(())
    }

    /// 一次取最早的一筆，讓連續到期的泡泡各自有顯示時間。
    pub fn next_due(&self, now: u64) -> Option<Entry> {
        self.entries
            .iter()
            .filter(|entry| entry.notified_at.is_none() && entry.due_at <= now)
            .min_by_key(|entry| (entry.due_at, entry.id))
            .cloned()
    }

    pub fn mark_notified(&mut self, id: u64, now: u64) -> AppResult<()> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.id == id)
            .ok_or("找不到此提醒事項。")?;
        entry.notified_at = Some(now);
        Ok(())
    }
}

fn valid_title(title: &str) -> AppResult<()> {
    if title.trim().is_empty() || title.chars().count() > 180 || title.chars().any(char::is_control)
    {
        return Err("提醒內容請填寫1～180字，勿包含換行或控制字元。".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overdue_reminders_are_ordered_and_not_replayed() {
        let mut store = Store::default();
        store.upsert(None, "較晚", 200, Channel::Pet, 100).unwrap();
        store
            .upsert(None, "較早", 150, Channel::Windows, 100)
            .unwrap();
        assert!(store.next_due(149).is_none());
        let due = store.next_due(300).unwrap();
        assert_eq!(due.title, "較早");
        store.mark_notified(due.id, 300).unwrap();
        let restored: Store = serde_json::from_slice(&serde_json::to_vec(&store).unwrap()).unwrap();
        assert_eq!(restored.next_due(300).unwrap().title, "較晚");
        store
            .upsert(Some(due.id), "改時間", 400, Channel::Pet, 300)
            .unwrap();
        assert_eq!(store.entries[1].notified_at, None);
        store.delete(due.id).unwrap();
        assert!(store.entries.iter().all(|e| e.id != due.id));
    }

    #[test]
    fn rejects_invalid_inputs_and_corrupt_identity() {
        let mut store = Store::default();
        for title in ["", " ", "第一行\n第二行", &"字".repeat(181)] {
            assert!(store.upsert(None, title, 200, Channel::Pet, 100).is_err());
        }
        assert!(store.upsert(None, "過期", 100, Channel::Pet, 100).is_err());
        assert!(store
            .upsert(None, "太遠", u64::MAX, Channel::Pet, 100)
            .is_err());
        assert!(store
            .upsert(Some(88), "不存在", 200, Channel::Pet, 100)
            .is_err());
        store.upsert(None, "正常", 200, Channel::Pet, 100).unwrap();
        store.entries.push(store.entries[0].clone());
        assert!(store.validate().is_err());
    }

    #[test]
    fn capacity_is_bounded_but_existing_entries_can_be_edited() {
        let mut store = Store::default();
        for _ in 0..MAX_ITEMS {
            store.upsert(None, "提醒", 200, Channel::Pet, 100).unwrap();
        }
        assert!(store.upsert(None, "超額", 200, Channel::Pet, 100).is_err());
        store
            .upsert(Some(1), "更新", 300, Channel::Windows, 100)
            .unwrap();
        assert_eq!(store.entries.len(), MAX_ITEMS);
        assert_eq!(store.entries[0].channel, Channel::Windows);
    }

    #[test]
    fn encrypted_persistence_round_trip_and_corruption_is_preserved() {
        let root = std::env::temp_dir().join(format!(
            "lmai-reminders-{}-{}",
            std::process::id(),
            crate::jobs::new_id().unwrap()
        ));
        fs::create_dir(&root).unwrap();
        let mut store = Store::load(&root).unwrap();
        store
            .upsert(None, "本機私有提醒", 200, Channel::Pet, 100)
            .unwrap();
        store.save(&root).unwrap();
        let bytes = fs::read(root.join(FILE)).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("本機私有提醒"));
        assert_eq!(Store::load(&root).unwrap().entries[0].title, "本機私有提醒");
        fs::write(root.join(FILE), b"broken").unwrap();
        assert!(Store::load(&root).is_err());
        assert_eq!(fs::read(root.join(FILE)).unwrap(), b"broken");
        fs::remove_file(root.join(FILE)).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
