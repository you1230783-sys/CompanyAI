//! 教學完成紀錄與目前版本公告。資料放在 EXE 旁，方便測試期間手動重設。
//! 教學 revision 與應用版本獨立；升版不會自動要求重看同一張卡。
use crate::{service, storage, AppResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::PathBuf};

const CHAT_ID: &str = "chat.basics";
const CHAT_REVISION: u32 = 1;

/// 教學內容與 revision 一起發行；只接受目錄內的項目，不能由前端任意寫入旗標。
fn tutorial_catalog() -> Vec<Value> {
    serde_json::from_str(include_str!("../../assets/tutorials.json")).unwrap_or_default()
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Saved {
    tutorials: BTreeMap<String, u32>,
}

pub(super) struct Onboarding {
    path: Option<PathBuf>,
    saved: Saved,
    error: Option<String>,
    disabled: bool,
}

impl Onboarding {
    pub(super) fn load(disabled: bool) -> Self {
        let mut this = Self {
            path: None,
            saved: Saved::default(),
            error: None,
            disabled,
        };
        if disabled {
            return this;
        }
        if let Err(error) = this.initialize() {
            this.error = Some(error);
        }
        this
    }

    fn initialize(&mut self) -> AppResult<()> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let folder = exe.parent().ok_or("找不到教學紀錄目錄")?;
        let path = folder.join("onboarding-state.json");
        self.path = Some(path.clone());
        match fs::metadata(&path) {
            Ok(info) => {
                if info.len() > 65536 {
                    return Err("教學紀錄超過大小限制，請檢查 onboarding-state.json".into());
                }
                self.saved = serde_json::from_slice(&fs::read(&path).map_err(|e| e.to_string())?)
                    .map_err(|_| "onboarding-state.json 格式錯誤；未覆寫原紀錄")?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.to_string()),
        }
        // 舊版版本欄位由 serde 忽略；保留每張教學卡的完成紀錄。
        self.persist(&self.saved)
    }

    fn persist(&self, saved: &Saved) -> AppResult<()> {
        let path = self.path.as_ref().ok_or("找不到教學紀錄路徑")?;
        storage::atomic_write(
            path,
            &serde_json::to_vec_pretty(saved).map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("無法儲存教學紀錄：{e}"))
    }

    pub(super) fn acknowledge(&mut self, topic: &str) -> AppResult<()> {
        if self.disabled {
            return Ok(());
        }
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        let mut saved = self.saved.clone();
        match topic {
            "chat" => {
                saved.tutorials.insert(CHAT_ID.into(), CHAT_REVISION);
            }
            id => {
                let revision = tutorial_catalog()
                    .iter()
                    .find(|entry| entry["id"] == id)
                    .and_then(|entry| entry["revision"].as_u64())
                    .and_then(|revision| u32::try_from(revision).ok())
                    .ok_or("未知的教學項目")?;
                saved.tutorials.insert(id.into(), revision);
            }
        }
        self.persist(&saved)?;
        self.saved = saved;
        Ok(())
    }

    pub(super) fn state(&self) -> Value {
        // 公告檔只保存本版內容；升版時直接替換，不累積舊版公告。
        let release_notes: Vec<Value> =
            serde_json::from_str::<Vec<Value>>(include_str!("../../assets/release-notes.json"))
                .unwrap_or_default()
                .into_iter()
                .filter(|entry| entry["version"].as_str() == Some(service::CURRENT_VERSION))
                .collect();
        let tutorials: Vec<_> = tutorial_catalog()
            .into_iter()
            .map(|mut entry| {
                let id = entry["id"].as_str().unwrap_or("");
                let revision = entry["revision"].as_u64().unwrap_or(1);
                entry["completed"] = json!(
                    u64::from(self.saved.tutorials.get(id).copied().unwrap_or(0)) >= revision
                );
                entry
            })
            .collect();
        json!({"enabled":!self.disabled,"chat_pending":!self.disabled && self.error.is_none()
            && self.saved.tutorials.get(CHAT_ID).copied().unwrap_or(0) < CHAT_REVISION,
            "release_notes":release_notes,"tutorials":tutorials,
            "version":service::CURRENT_VERSION,
            "error":self.error,"state_file":self.path})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_state_preserves_tutorials_without_upgrade_announcements() {
        let saved: Saved = serde_json::from_value(json!({
            "last_version":"0.8.65", "pending_from":"0.8.64",
            "tutorials":{"chat.basics":1}
        }))
        .unwrap();
        let state = Onboarding {
            path: None,
            saved,
            error: None,
            disabled: false,
        };
        assert_eq!(state.state()["chat_pending"], false);
        assert!(state.state().get("from_version").is_none());
        assert!(state.state().get("updates").is_none());
        let info = state.state();
        let notes = info["release_notes"].as_array().unwrap();
        assert!(!notes.is_empty());
        assert!(notes
            .iter()
            .all(|entry| entry["version"] == service::CURRENT_VERSION));
        let persisted = serde_json::to_value(&state.saved).unwrap();
        assert!(persisted.get("pending_from").is_none());
        assert_eq!(persisted["tutorials"][CHAT_ID], CHAT_REVISION);
    }

    #[test]
    fn completing_one_topic_preserves_the_other_and_persists() {
        // 不與專案 Broker 的路徑鎖測試共用 .build；Windows 在目錄被固定時
        // 可能拒絕原子重新命名。獨立暫存目錄讓本測試只驗證教學保存契約。
        let root = std::env::temp_dir().join(format!(
            "company-ai-onboarding-{}",
            crate::jobs::new_id().unwrap()
        ));
        let path = root.join("onboarding-state.json");
        let mut state = Onboarding {
            path: Some(path.clone()),
            error: None,
            disabled: false,
            saved: Saved::default(),
        };
        state.acknowledge("chat").unwrap();
        let saved: Saved = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved.tutorials.get(CHAT_ID), Some(&CHAT_REVISION));
        state.acknowledge("chat.attachments").unwrap();
        assert_eq!(state.saved.tutorials.get(CHAT_ID), Some(&CHAT_REVISION));
        assert_eq!(state.saved.tutorials.get("chat.attachments"), Some(&1));
        assert!(state.acknowledge("unknown").is_err());
        fs::remove_file(path).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
