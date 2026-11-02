//! 使用者明確保存的工作偏好與條件。只由桌面設定入口寫入，模型工具沒有寫入權限。
//! 任務開始保留偏好快照；條件版本可在工具邊界更新，不改寫已提交的模型請求。
use crate::{jobs, storage, AppResult};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    pub language: String,
    pub decimals: Option<u8>,
    pub date_format: String,
    pub chart_background: String,
    pub notes: String,
}
impl Preferences {
    pub fn validate(&self) -> AppResult<()> {
        if self.language.chars().count() > 40
            || self.decimals.is_some_and(|n| n > 8)
            || !["", "YYYY-MM-DD", "YYYY/MM/DD", "DD/MM/YYYY"].contains(&self.date_format.as_str())
            || !["", "white", "transparent"].contains(&self.chart_background.as_str())
            || self.notes.chars().count() > 1000
        {
            return Err("偏好格式無效：語言最多40字、小數0–8位、補充最多1000字。".into());
        }
        Ok(())
    }
    /// 空欄位代表沿用個人偏好；專案覆寫不會修改個人設定。
    fn overlay(&self, project: &Self) -> Self {
        let choose = |local: &String, base: &String| {
            if local.is_empty() {
                base.clone()
            } else {
                local.clone()
            }
        };
        Self {
            language: choose(&project.language, &self.language),
            decimals: project.decimals.or(self.decimals),
            date_format: choose(&project.date_format, &self.date_format),
            chart_background: choose(&project.chart_background, &self.chart_background),
            notes: choose(&project.notes, &self.notes),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Conditions {
    pub revision: u64,
    pub text: String,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub personal: Preferences,
    pub projects: BTreeMap<String, Preferences>,
    pub conditions: BTreeMap<String, Conditions>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Snapshot {
    pub preferences: Preferences,
    pub conditions: Conditions,
}

impl Settings {
    pub fn load(root: &Path) -> AppResult<Self> {
        let path = root.join("work-settings.dpapi");
        if !path.exists() {
            return Ok(Self::default());
        }
        if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 2_100_000 {
            return Err("工作偏好檔過大，原檔保留。".into());
        }
        let bytes = storage::protect(&std::fs::read(path).map_err(|e| e.to_string())?, false)?;
        let value: Self =
            serde_json::from_slice(&bytes).map_err(|_| "工作偏好無法解析，原檔保留。")?;
        value.validate()?;
        Ok(value)
    }
    fn validate(&self) -> AppResult<()> {
        self.personal.validate()?;
        if self.projects.len() > 50 || self.conditions.len() > 500 {
            return Err("工作偏好超過50個專案或500份對話條件。".into());
        }
        for (id, preference) in &self.projects {
            jobs::validate_id(id)?;
            preference.validate()?;
        }
        for (id, condition) in &self.conditions {
            jobs::validate_id(id)?;
            if condition.text.chars().count() > 4000 {
                return Err("對話條件最多4000字。".into());
            }
        }
        Ok(())
    }
    pub fn save(&self, root: &Path) -> AppResult<()> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        if bytes.len() > 2_000_000 {
            return Err("工作偏好超過2MB，未儲存。".into());
        }
        storage::atomic_write(
            &root.join("work-settings.dpapi"),
            &storage::protect(&bytes, true)?,
        )
    }
    pub fn snapshot(&self, project: &str, conversation: &str) -> Snapshot {
        Snapshot {
            preferences: self.personal.overlay(
                self.projects
                    .get(project)
                    .unwrap_or(&Preferences::default()),
            ),
            conditions: self
                .conditions
                .get(conversation)
                .cloned()
                .unwrap_or_default(),
        }
    }
    /// 使用讀取時的版本避免兩個視窗互相覆蓋；清空仍增加版本，讓執行中任務知道條件已移除。
    pub fn set_conditions(
        &mut self,
        conversation: &str,
        expected: u64,
        text: &str,
    ) -> AppResult<()> {
        jobs::validate_id(conversation)?;
        if text.chars().count() > 4000 {
            return Err("對話條件最多4000字。".into());
        }
        let old = self
            .conditions
            .get(conversation)
            .cloned()
            .unwrap_or_default();
        if old.revision != expected {
            return Err("條件已更新，請重新開啟後再修改。".into());
        }
        if old.text != text.trim() {
            self.conditions.insert(
                conversation.into(),
                Conditions {
                    revision: old.revision.checked_add(1).ok_or("條件版本超出範圍。")?,
                    text: text.trim().into(),
                },
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encrypted_settings_roundtrip_and_invalid_updates_preserve_saved_choices() {
        let root = std::env::temp_dir().join(format!(
            "companyai-work-settings-{}",
            jobs::new_id().unwrap()
        ));
        std::fs::create_dir(&root).unwrap();
        let mut settings = Settings::default();
        settings.personal.language = "繁體中文".into();
        settings
            .set_conditions("chat", 0, "只處理合成測試資料")
            .unwrap();
        settings.save(&root).unwrap();
        let bytes = std::fs::read(root.join("work-settings.dpapi")).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("只處理合成測試資料"));
        let restored = Settings::load(&root).unwrap();
        assert_eq!(
            restored.snapshot("p", "chat").conditions.text,
            "只處理合成測試資料"
        );
        settings.personal.decimals = Some(99);
        assert!(settings.save(&root).is_err());
        assert_eq!(
            std::fs::read(root.join("work-settings.dpapi")).unwrap(),
            bytes
        );
        std::fs::remove_file(root.join("work-settings.dpapi")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
    #[test]
    fn project_overrides_and_condition_revisions_preserve_user_choices() {
        let mut settings = Settings::default();
        settings.personal.language = "繁體中文".into();
        settings.personal.decimals = Some(2);
        settings.projects.insert(
            "project".into(),
            Preferences {
                decimals: Some(0),
                ..Default::default()
            },
        );
        let snapshot = settings.snapshot("project", "chat");
        assert_eq!(snapshot.preferences.decimals, Some(0));
        assert_eq!(snapshot.preferences.language, "繁體中文");
        settings
            .set_conditions("chat", 0, "排除異常值並列出筆數")
            .unwrap();
        assert!(settings.set_conditions("chat", 0, "舊視窗覆寫").is_err());
        settings.set_conditions("chat", 1, "").unwrap();
        assert_eq!(settings.snapshot("project", "chat").conditions.revision, 2);
        assert_eq!(snapshot.conditions.revision, 0);
    }
}
