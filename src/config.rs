//! 可調整的網站與模型設定。網址先驗證，再交給 WinHTTP，避免錯送憑證。
use crate::AppResult;
use serde::{Deserialize, Serialize};
use url::Url;

pub const CLIENT_ID: &str = "company-ai-desktop";
pub const DEVICE_PATH: &str = "/api/desktop/oauth/device";
pub const TOKEN_PATH: &str = "/api/desktop/oauth/token";
pub const MAX_SESSION_SECONDS: u64 = 30 * 24 * 60 * 60;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthHeader {
    #[default]
    Bearer,
    XApiKey,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub server_url: String,
    pub chat_path: String,
    pub model: String,
    pub auth_header: AuthHeader,
    pub allow_http: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_url: String::new(),
            chat_path: "/v1/chat/completions".into(),
            model: String::new(),
            auth_header: AuthHeader::Bearer,
            allow_http: false,
        }
    }
}

impl Config {
    /// 網站欄位只接受來源（scheme + host + port），各 API 路徑分開定義。
    pub fn origin(&self) -> AppResult<Url> {
        let url = Url::parse(self.server_url.trim())
            .map_err(|_| "請輸入完整網站網址，例如 https://ai.company.example".to_string())?;
        validate_url(&url, self.allow_http)?;
        if url.path() != "/" || url.query().is_some() {
            return Err("網站欄位只填 https://主機名稱[:埠號]；API 路徑請填在下方。".into());
        }
        Ok(url)
    }

    pub fn validate(&self) -> AppResult<()> {
        self.origin()?;
        self.endpoint(&self.chat_path)?;
        if self.model.trim().is_empty() || self.model.len() > 200 {
            return Err("請填寫公司 API 支援的模型名稱（最多 200 bytes）。".into());
        }
        Ok(())
    }

    /// 不接受跨站絕對網址、查詢參數或反斜線，確保登入與 API 留在設定的網站。
    pub fn endpoint(&self, path: &str) -> AppResult<Url> {
        if !path.starts_with('/') || path.starts_with("//") || path.contains(['\\', '?', '#']) {
            return Err(
                "API 路徑必須以單一 / 開頭，例如 /v1/chat/completions；不可含查詢字串。".into(),
            );
        }
        let origin = self.origin()?;
        let url = origin
            .join(path)
            .map_err(|_| "API 路徑無效。".to_string())?;
        if url.origin() != origin.origin() {
            return Err("API 必須位於同一網站。".into());
        }
        Ok(url)
    }

    /// 憑證綁定網站、路由及 Header 類型；修改這些欄位時必須重新登入。
    /// 模型名稱不影響憑證綁定，因此可以在同一 API 下切換模型。
    pub fn binding(&self) -> AppResult<String> {
        Ok(format!(
            "{}|{}|{:?}",
            self.origin()?.origin().ascii_serialization(),
            self.chat_path,
            self.auth_header
        ))
    }

    pub fn verification_url(&self, value: &str) -> AppResult<Url> {
        let url = Url::parse(value).map_err(|_| "網站回傳的登入網址無效。".to_string())?;
        validate_url(&url, self.allow_http)?;
        if url.origin() != self.origin()?.origin() {
            return Err(
                "網站回傳了不同來源的登入網址，已停止。請檢查 verification_uri 設定。".into(),
            );
        }
        Ok(url)
    }
}

/// HTTPS 使用 Windows 憑證信任庫。HTTP 僅允許本機測試或使用者明確勾選。
pub fn validate_url(url: &Url, allow_http: bool) -> AppResult<()> {
    let loopback = matches!(url.host_str(), Some("127.0.0.1" | "[::1]" | "localhost"));
    if url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err("網址不可含帳號、密碼或 # 片段。".into());
    }
    match url.scheme() {
        "https" => Ok(()),
        "http" if allow_http || loopback => Ok(()),
        _ => Err("請使用 HTTPS；僅內網 HTTP 測試時，明確勾選允許 HTTP。".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_cross_origin_and_insecure_endpoints() {
        let config = Config {
            server_url: "https://company.example".into(),
            model: "test".into(),
            ..Config::default()
        };
        assert!(config.validate().is_ok());
        for path in [
            "//other.example/api",
            "/\\other.example",
            "https://other.example",
            "/chat?key=secret",
        ] {
            assert!(config.endpoint(path).is_err());
        }
        assert!(config
            .verification_url("https://other.example/login")
            .is_err());
        assert!(Config {
            server_url: "http://company.example".into(),
            ..config
        }
        .validate()
        .is_err());
    }

    #[test]
    fn model_change_keeps_binding_but_route_change_does_not() {
        let mut config = Config {
            server_url: "https://company.example".into(),
            ..Config::default()
        };
        let before = config.binding().unwrap();
        config.model = "another-model".into();
        assert_eq!(before, config.binding().unwrap());
        config.chat_path = "/other/chat".into();
        assert_ne!(before, config.binding().unwrap());
    }
}
