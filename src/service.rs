//! 版本門檻與模型選單。只接受明確、可驗證的伺服器資料，不從顯示名稱猜模型。
use crate::{
    config::{Config, MODELS_PATH, VERSION_PATH},
    storage::Session,
    transport, AppResult,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// 第一版的版本契約限定三段非負整數；避免用字串排序造成 0.10 小於 0.9。
pub fn version_number(value: &str) -> AppResult<[u32; 3]> {
    let parts: Vec<_> = value.split('.').collect();
    if parts.len() != 3 {
        return Err("版本資訊格式不正確。".into());
    }
    let mut numbers = [0; 3];
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() || !part.bytes().all(|c| c.is_ascii_digit()) {
            return Err("版本資訊格式不正確。".into());
        }
        numbers[index] = part.parse().map_err(|_| "版本資訊超出範圍。")?;
    }
    Ok(numbers)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VersionInfo {
    pub latest_version: String,
    pub minimum_version: String,
    #[serde(default)]
    pub message: String,
}
impl VersionInfo {
    pub fn validate(&self) -> AppResult<()> {
        if version_number(&self.minimum_version)? > version_number(&self.latest_version)?
            || self.message.len() > 2000
        {
            return Err("版本資訊不一致，請聯絡管理者。".into());
        }
        Ok(())
    }
    pub fn required(&self) -> bool {
        // 只有 validate 成功的資料能放入 UI 的已知版本狀態。
        version_number(CURRENT_VERSION).ok() < version_number(&self.minimum_version).ok()
    }
    pub fn available(&self) -> bool {
        version_number(CURRENT_VERSION).ok() < version_number(&self.latest_version).ok()
    }
}

/// 失敗不覆蓋已知門檻：第一次查詢失敗可暫用，但不能以斷線解除已確認的強制更新。
#[derive(Default)]
pub struct VersionState {
    pub known: Option<VersionInfo>,
}
impl VersionState {
    pub fn apply(&mut self, result: AppResult<VersionInfo>) -> AppResult<()> {
        let mut info = result?;
        info.validate()?;
        // 已確認必須更新後，後續回應不可降低門檻；只有安裝足夠新的程式才能解除。
        if let Some(previous) = self.known.as_ref().filter(|v| v.required()) {
            if version_number(&info.minimum_version)? < version_number(&previous.minimum_version)? {
                info.minimum_version = previous.minimum_version.clone();
            }
            if version_number(&info.latest_version)? < version_number(&info.minimum_version)? {
                info.latest_version = previous.latest_version.clone();
            }
        }
        self.known = Some(info);
        Ok(())
    }
    /// 此檔沒有個人資訊；原子寫入保留已確認的最低版本，重新開啟及離線都不能解除。
    pub fn save(&self, root: &std::path::Path) -> AppResult<()> {
        if let Some(info) = &self.known {
            crate::storage::atomic_write(
                &root.join("version-policy.json"),
                &serde_json::to_vec(info).map_err(|e| e.to_string())?,
            )?;
        }
        Ok(())
    }
    pub fn load(root: &std::path::Path) -> AppResult<Self> {
        let path = root.join("version-policy.json");
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(format!("無法讀取已保存的版本限制：{e}")),
        };
        if bytes.len() > 65_536 {
            return Err("版本限制檔案過大，請聯絡 IT。".into());
        }
        let mut state = Self::default();
        state.apply(
            serde_json::from_slice(&bytes)
                .map_err(|_| "版本限制檔案損毀，請重新安裝新版。".to_string()),
        )?;
        Ok(state)
    }
    pub fn blocked(&self) -> bool {
        self.known.as_ref().is_some_and(VersionInfo::required)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct ModelOption {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: String,
}
#[derive(Clone, Debug, Deserialize)]
pub struct ModelCatalog {
    pub models: Vec<ModelOption>,
    #[serde(default)]
    pub default_model: Option<String>,
}
impl ModelCatalog {
    pub fn validate(&self) -> AppResult<()> {
        if self.models.is_empty() || self.models.len() > 40 {
            return Err("目前沒有可用的模型選項。".into());
        }
        let mut ids = HashSet::new();
        for model in &self.models {
            if model.id.is_empty()
                || model.id.len() > 64
                || !model
                    .id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
                || model.label.trim().is_empty()
                || model.label.chars().count() > 40
                || model.label.chars().any(char::is_control)
                || model.description.len() > 1000
                || !ids.insert(&model.id)
            {
                return Err("模型選項格式不正確，請聯絡管理者。".into());
            }
        }
        if self
            .default_model
            .as_ref()
            .is_some_and(|id| !self.models.iter().any(|model| &model.id == id))
        {
            return Err("預設模型不在可用清單內。".into());
        }
        Ok(())
    }
    pub fn selected_index(&self, preferred: &str) -> usize {
        self.models
            .iter()
            .position(|m| m.id == preferred)
            .or_else(|| {
                self.default_model
                    .as_ref()
                    .and_then(|id| self.models.iter().position(|m| &m.id == id))
            })
            .unwrap_or(0)
    }
}

pub fn fetch_version(config: &Config) -> AppResult<VersionInfo> {
    let response = transport::get(&config.endpoint(VERSION_PATH)?, None)?;
    if response.status != 200 {
        return Err("暫時無法檢查版本，稍後會再嘗試。".into());
    }
    let info: VersionInfo =
        serde_json::from_str(&response.body).map_err(|_| "版本資訊格式不正確。")?;
    info.validate()?;
    Ok(info)
}

/// 可匿名取得基本清單；若後端需要使用者權限，401 後會等登入完成再重查。
pub fn fetch_models(config: &Config, session: Option<&Session>) -> AppResult<ModelCatalog> {
    let token = session
        .filter(|s| s.valid_for(config))
        .map(|s| format!("Bearer {}", s.access_token));
    let response = transport::get(
        &config.endpoint(MODELS_PATH)?,
        token.as_ref().map(|v| ("Authorization", v.as_str())),
    )?;
    if response.status == 401 {
        return Err("請先登入，再重新整理模型選單。".into());
    }
    if response.status != 200 {
        return Err("無法取得模型選項，請稍後重新整理服務。".into());
    }
    let catalog: ModelCatalog =
        serde_json::from_str(&response.body).map_err(|_| "模型清單格式不正確。")?;
    catalog.validate()?;
    Ok(catalog)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mandatory_floor_survives_restart_and_server_downgrade() {
        let root = std::env::temp_dir().join(format!(
            "LM_AI-policy-test-{}",
            crate::jobs::new_id().unwrap()
        ));
        let mut state = VersionState::default();
        state
            .apply(Ok(VersionInfo {
                latest_version: "99.0.0".into(),
                minimum_version: "98.0.0".into(),
                message: String::new(),
            }))
            .unwrap();
        state.save(&root).unwrap();
        let mut reloaded = VersionState::load(&root).unwrap();
        assert!(reloaded.blocked());
        reloaded
            .apply(Ok(VersionInfo {
                latest_version: CURRENT_VERSION.into(),
                minimum_version: "0.1.0".into(),
                message: String::new(),
            }))
            .unwrap();
        assert!(reloaded.blocked());
        assert_eq!(reloaded.known.unwrap().minimum_version, "98.0.0");
        std::fs::remove_file(root.join("version-policy.json")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
    #[test]
    fn version_order_and_failure_policy() {
        assert!(version_number("0.10.0").unwrap() > version_number("0.9.9").unwrap());
        for value in ["1.0", "1.0.0.0", "v1.0.0", "1.0.-1", "1.0.0-beta"] {
            assert!(version_number(value).is_err());
        }
        let mut state = VersionState::default();
        assert!(state.apply(Err("offline".into())).is_err());
        assert!(!state.blocked());
        state
            .apply(Ok(VersionInfo {
                latest_version: "99.0.0".into(),
                minimum_version: "99.0.0".into(),
                message: String::new(),
            }))
            .unwrap();
        assert!(state.blocked());
        assert!(state.apply(Err("offline".into())).is_err());
        assert!(state.blocked());
        assert!(state
            .apply(Ok(VersionInfo {
                latest_version: "0.1.0".into(),
                minimum_version: "99.0.0".into(),
                message: String::new()
            }))
            .is_err());
        assert!(state.blocked());
    }
    #[test]
    fn model_aliases_are_dynamic_and_validated() {
        let catalog: ModelCatalog = serde_json::from_str(r#"{"models":[{"id":"fast","label":"快速"},{"id":"quality","label":"品質"},{"id":"ultra","label":"Ultra"}],"default_model":"quality"}"#).unwrap();
        catalog.validate().unwrap();
        assert_eq!(catalog.selected_index("ultra"), 2);
        assert_eq!(catalog.selected_index("old-model"), 1);
        let mut bad = catalog.clone();
        bad.models[1].id = "fast".into();
        assert!(bad.validate().is_err());
        bad = catalog;
        bad.default_model = Some("missing".into());
        assert!(bad.validate().is_err());
    }
}
