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
    /// 與完成旗標分開；略過選單不代表已學會任一教學。
    practice_welcome_dismissed: bool,
    /// 0.8.79可能沒有此欄位；缺少時保持未知，不能推測上一個使用版本。
    last_version: Option<String>,
    release_pending: bool,
    release_from: Option<String>,
}

/// 只接受三段數字版本，不能用字典排序比較0.8.100與0.8.99。
fn version(value: &str) -> Option<[u32; 3]> {
    let parts: Vec<_> = value
        .split('.')
        .map(str::parse::<u32>)
        .collect::<Result<_, _>>()
        .ok()?;
    parts.try_into().ok()
}

impl Saved {
    fn started(&mut self, current: &str) {
        if self.last_version.as_deref() == Some(current) {
            return;
        }
        let previous = self.last_version.as_deref().and_then(version);
        let now = version(current);
        // 降版只記錄實際執行版本，不把較新公告套到較舊程式。
        self.release_pending = now.is_some() && (previous.is_none() || previous < now);
        self.release_from = self.last_version.clone().filter(|s| version(s).is_some());
        self.last_version = Some(current.into());
    }
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
        self.saved.started(service::CURRENT_VERSION);
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
            "release" => saved.release_pending = false,
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

    /// 首次選擇或跳過練習後不再自動開啟；設定中的手動入口始終保留。
    pub(super) fn dismiss_practice_welcome(&mut self) -> AppResult<()> {
        if self.disabled || self.saved.practice_welcome_dismissed {
            return Ok(());
        }
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        let mut saved = self.saved.clone();
        saved.practice_welcome_dismissed = true;
        self.persist(&saved)?;
        self.saved = saved;
        Ok(())
    }

    pub(super) fn state(&self) -> Value {
        // 歷史公告隨EXE提供，離線可查看；只顯示不晚於當前版本的已知紀錄。
        let release_notes: Vec<Value> =
            serde_json::from_str::<Vec<Value>>(include_str!("../../assets/release-notes.json"))
                .unwrap_or_default()
                .into_iter()
                .filter(|entry| {
                    entry["version"]
                        .as_str()
                        .and_then(version)
                        .is_some_and(|v| Some(v) <= version(service::CURRENT_VERSION))
                })
                .collect();
        let from = self.saved.release_from.as_deref().and_then(version);
        let updates: Vec<_> = release_notes
            .iter()
            .filter(|entry| {
                from.is_none_or(|v| {
                    entry["version"]
                        .as_str()
                        .and_then(version)
                        .is_some_and(|n| n > v)
                })
            })
            .cloned()
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
        json!({"enabled":!self.disabled,"practice_welcome_pending":!self.saved.practice_welcome_dismissed,"chat_pending":!self.disabled && self.error.is_none()
            && self.saved.tutorials.get(CHAT_ID).copied().unwrap_or(0) < CHAT_REVISION,
            "release_notes":release_notes,"tutorials":tutorials,
            "release_pending":!self.disabled && self.error.is_none() && self.saved.release_pending,
            "from_version":self.saved.release_from,"updates":updates,
            "version":service::CURRENT_VERSION,
            "error":self.error,"state_file":self.path})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_state_preserves_tutorials_and_announces_actual_range() {
        let mut saved: Saved = serde_json::from_value(json!({
            "last_version":"0.8.65", "pending_from":"0.8.64",
            "tutorials":{"chat.basics":1}
        }))
        .unwrap();
        saved.started(service::CURRENT_VERSION);
        let state = Onboarding {
            path: None,
            saved,
            error: None,
            disabled: false,
        };
        assert_eq!(state.state()["chat_pending"], false);
        assert_eq!(state.state()["from_version"], "0.8.65");
        assert_eq!(state.state()["release_pending"], true);
        let info = state.state();
        let notes = info["release_notes"].as_array().unwrap();
        assert!(!notes.is_empty());
        assert!(notes
            .iter()
            .all(|entry| entry["version"].as_str().and_then(version)
                <= version(service::CURRENT_VERSION)));
        let persisted = serde_json::to_value(&state.saved).unwrap();
        assert!(persisted.get("pending_from").is_none());
        assert_eq!(persisted["tutorials"][CHAT_ID], CHAT_REVISION);
    }

    #[test]
    fn unknown_version_restart_ack_and_downgrade_are_distinct() {
        let mut saved = Saved::default();
        saved.started("0.9.0");
        assert!(saved.release_pending);
        assert!(saved.release_from.is_none());
        saved.started("0.9.0");
        assert!(saved.release_pending); // 未閱讀不能因重開而消失。
        saved.release_pending = false;
        saved.started("0.9.0");
        assert!(!saved.release_pending);
        saved.started("0.8.99");
        assert!(!saved.release_pending); // 降版不宣告新版功能。
        saved.started("0.8.100");
        assert!(saved.release_pending);
        assert_eq!(saved.release_from.as_deref(), Some("0.8.99"));
        assert!(version("0.8.100") > version("0.8.99"));
        assert!(version("0.8.bad").is_none());
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
        assert_eq!(state.saved.tutorials.get("chat.attachments"), Some(&2));
        // 選單已提供與個別主題完成分開保存；略過不能把其他主題標成完成。
        let completed = state.saved.tutorials.clone();
        assert_eq!(state.state()["practice_welcome_pending"], true);
        state.dismiss_practice_welcome().unwrap();
        assert_eq!(state.state()["practice_welcome_pending"], false);
        assert_eq!(state.saved.tutorials, completed);
        let persisted: Saved = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert!(persisted.practice_welcome_dismissed);
        assert_eq!(persisted.tutorials, completed);
        assert!(state.acknowledge("unknown").is_err());
        fs::remove_file(path).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
