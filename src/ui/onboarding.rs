//! 教學完成紀錄與跨版本摘要。資料放在 EXE 旁，方便測試期間手動重設。
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
    last_version: Option<String>,
    pending_from: Option<String>,
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
        // 首次支援此功能時，舊版沒有旗標；由安裝包在替換 EXE 前記錄真實舊版本。
        let transition = folder.join("update-transition.ini");
        let installed_from = fs::metadata(&transition)
            .ok()
            .filter(|m| m.len() <= 4096)
            .and_then(|_| fs::read_to_string(&transition).ok())
            .and_then(|text| {
                let value = |key: &str| {
                    text.lines()
                        .find_map(|line| line.trim().strip_prefix(key).map(str::to_owned))
                };
                (value("to=").as_deref() == Some(service::CURRENT_VERSION))
                    .then(|| value("from="))
                    .flatten()
            });
        let previous = self.saved.last_version.clone().or(installed_from);
        if let Some(previous) = previous {
            if service::version_number(&previous)?
                < service::version_number(service::CURRENT_VERSION)?
                && self.saved.pending_from.is_none()
            {
                self.saved.pending_from = Some(previous);
            }
        }
        self.saved.last_version = Some(service::CURRENT_VERSION.into());
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
            "updates" => saved.pending_from = None,
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

    /// 更新啟動前保存目前版本。沒有確認安裝成功前不先改成目標版本。
    pub(super) fn before_update(&self) -> AppResult<()> {
        if self.disabled {
            return Ok(());
        }
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        let mut saved = self.saved.clone();
        saved.last_version = Some(service::CURRENT_VERSION.into());
        self.persist(&saved)
    }

    pub(super) fn state(&self) -> Value {
        let from = self.saved.pending_from.as_deref();
        let lower = from.and_then(|v| service::version_number(v).ok());
        let current = service::version_number(service::CURRENT_VERSION).ok();
        // 只加入已明確決定交付的簡短公告，從 0.8.65 開始；不回填歷史版本。
        let history: Vec<Value> =
            serde_json::from_str::<Vec<Value>>(include_str!("../../assets/release-notes.json"))
                .unwrap_or_default()
                .into_iter()
                .filter(|entry| {
                    let version = entry["version"]
                        .as_str()
                        .and_then(|v| service::version_number(v).ok());
                    matches!((current,version), (Some(b),Some(v)) if v <= b && v >= [0,8,65])
                })
                .collect();
        let entries: Vec<_> = history
            .iter()
            .filter(|entry| {
                let version = entry["version"]
                    .as_str()
                    .and_then(|v| service::version_number(v).ok());
                matches!((lower,version), (Some(a),Some(v)) if a < v)
            })
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
            "updates":entries,"release_history":history,"tutorials":tutorials,
            "from_version":from,"to_version":service::CURRENT_VERSION,
            "error":self.error,"state_file":self.path})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tutorial_revision_and_upgrade_range_are_independent() {
        let mut state = Onboarding {
            path: None,
            error: None,
            disabled: false,
            saved: Saved {
                last_version: Some(service::CURRENT_VERSION.into()),
                pending_from: Some("0.8.50".into()),
                tutorials: BTreeMap::new(),
            },
        };
        assert_eq!(state.state()["chat_pending"], true);
        let info = state.state();
        let updates = info["updates"].as_array().unwrap();
        assert!(!updates.is_empty());
        assert!(updates.iter().any(|entry| entry["version"] == "0.8.65"));
        assert!(updates.iter().all(|entry| {
            let version = service::version_number(entry["version"].as_str().unwrap()).unwrap();
            version >= [0, 8, 65]
                && version <= service::version_number(service::CURRENT_VERSION).unwrap()
        }));
        state.saved.tutorials.insert(CHAT_ID.into(), CHAT_REVISION);
        assert_eq!(state.state()["chat_pending"], false);
        state.saved.pending_from = Some(service::CURRENT_VERSION.into());
        assert!(state.state()["updates"].as_array().unwrap().is_empty());
        assert_eq!(Onboarding::load(true).state()["enabled"], false);
    }

    #[test]
    fn completing_one_topic_preserves_the_other_and_persists() {
        let path = std::env::current_dir()
            .unwrap()
            .join(".build")
            .join(format!(
                "onboarding-test-{}.json",
                crate::jobs::new_id().unwrap()
            ));
        let mut state = Onboarding {
            path: Some(path.clone()),
            error: None,
            disabled: false,
            saved: Saved {
                pending_from: Some("0.8.64".into()),
                ..Saved::default()
            },
        };
        state.acknowledge("chat").unwrap();
        let saved: Saved = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved.tutorials.get(CHAT_ID), Some(&CHAT_REVISION));
        assert_eq!(saved.pending_from.as_deref(), Some("0.8.64"));
        state.acknowledge("updates").unwrap();
        assert!(state.saved.pending_from.is_none());
        assert!(state.acknowledge("unknown").is_err());
        fs::remove_file(path).unwrap();
    }
}
