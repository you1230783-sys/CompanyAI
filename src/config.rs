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
    /// 保留舊版預設路由，讓既有 settings.json 不必手動補欄位。
    pub device_path: String,
    pub token_path: String,
    pub model: String,
    pub auth_header: AuthHeader,
    pub allow_http: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_url: String::new(),
            chat_path: "/v1/chat/completions".into(),
            device_path: DEVICE_PATH.into(),
            token_path: TOKEN_PATH.into(),
            model: String::new(),
            auth_header: AuthHeader::Bearer,
            allow_http: false,
        }
    }
}

impl Config {
    /// 網站可以部署在子目錄；將它視為目錄，避免 URL join 把最後一段當成檔名取代。
    /// 主機名稱交由 URL 函式庫解析，單段內網名稱、IP 與自訂埠號都可以使用。
    pub fn base_url(&self) -> AppResult<Url> {
        let input = self.server_url.trim();
        reject_ambiguous_characters(input)?;
        let mut url = Url::parse(input).map_err(|_| {
            "請輸入完整網站網址，例如 https://ai.company.example 或 http://intranet-host。"
                .to_string()
        })?;
        validate_url(&url, self.allow_http)?;
        if url.query().is_some() {
            return Err("網站網址不可含查詢字串；API Key 請由登入授權取得。".into());
        }
        if !url.path().ends_with('/') {
            // path_segments_mut 避免重新編碼既有的 %xx 路徑。
            url.path_segments_mut()
                .map_err(|_| "網站網址必須使用 HTTP 或 HTTPS。".to_string())?
                .push("");
        }
        Ok(url)
    }

    pub fn validate(&self) -> AppResult<()> {
        self.base_url()
            .map_err(|error| format!("網站網址：{error}"))?;
        for (label, path) in [
            ("聊天 API", &self.chat_path),
            ("登入碼路徑", &self.device_path),
            ("Token 路徑", &self.token_path),
        ] {
            self.endpoint(path)
                .map_err(|error| format!("{label}：{error}"))?;
        }
        if self.model.trim().is_empty() || self.model.len() > 200 {
            return Err("請填寫公司 API 支援的模型名稱（最多 200 bytes）。".into());
        }
        Ok(())
    }

    /// 支援完整 URL、以 / 開頭的主機根路徑，以及相對於網站子目錄的路徑。
    /// 最終 URL 必須同來源，避免把登入憑證送到不同主機、埠號或協定。
    pub fn endpoint(&self, path: &str) -> AppResult<Url> {
        let path = path.trim();
        if path.is_empty() {
            return Err("請填寫路徑或完整網址，例如 v1/chat/completions。".into());
        }
        reject_ambiguous_characters(path)?;
        if path.starts_with("//") || path.contains(['?', '#']) {
            return Err(
                "請填寫 API 路徑或完整 HTTP(S) 網址；不可使用 // 開頭、查詢字串或 # 片段。".into(),
            );
        }
        let base = self.base_url()?;
        let url = base
            .join(path)
            .map_err(|_| "無法解析 API 路徑或完整網址。".to_string())?;
        validate_url(&url, self.allow_http)?;
        if url.origin() != base.origin() {
            return Err("API 必須與網站網址使用相同的協定、主機名稱及埠號。".into());
        }
        Ok(url)
    }

    /// 憑證綁定解析後的實際路由及 Header；避免只改網站子目錄卻沿用舊 Key。
    /// 模型名稱不影響憑證綁定，因此可以在同一 API 下切換模型。
    pub fn binding(&self) -> AppResult<String> {
        Ok(format!(
            "v2|{}|{}|{}|{:?}",
            self.endpoint(&self.chat_path)?,
            self.endpoint(&self.device_path)?,
            self.endpoint(&self.token_path)?,
            self.auth_header
        ))
    }

    pub fn verification_url(&self, value: &str) -> AppResult<Url> {
        let url = Url::parse(value).map_err(|_| "網站回傳的登入網址無效。".to_string())?;
        validate_url(&url, self.allow_http)?;
        if url.origin() != self.base_url()?.origin() {
            return Err(
                "網站回傳了不同來源的登入網址，已停止。請檢查 verification_uri 設定。".into(),
            );
        }
        Ok(url)
    }
}

/// URL 解析器會忽略部分換行或將反斜線當成斜線，先拒絕這些容易誤導的輸入。
fn reject_ambiguous_characters(value: &str) -> AppResult<()> {
    if value.contains('\\') || value.chars().any(char::is_control) {
        return Err("網址或路徑不可含反斜線、換行或控制字元。".into());
    }
    Ok(())
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

    #[test]
    fn intranet_routes_accept_root_relative_and_complete_urls() {
        // 使用與公司部署相同的單段主機和多層路徑形式，不在公開原始碼記錄真實主機。
        let expected = "http://intranet-host/gateway/api/v1/desktop/v1/chat/completions";
        for (server_url, chat_path) in [
            (
                "http://intranet-host",
                "/gateway/api/v1/desktop/v1/chat/completions",
            ),
            (
                "http://intranet-host",
                "gateway/api/v1/desktop/v1/chat/completions",
            ),
            ("http://intranet-host", expected),
            (
                "http://intranet-host/gateway/api/v1/desktop",
                "v1/chat/completions",
            ),
            (
                "http://intranet-host/gateway/api/v1/desktop/",
                "v1/chat/completions",
            ),
            // 單一 / 表示從主機根目錄開始，不重複附加網站的子目錄。
            (
                "http://intranet-host/gateway",
                "/gateway/api/v1/desktop/v1/chat/completions",
            ),
        ] {
            let config = Config {
                server_url: server_url.into(),
                chat_path: chat_path.into(),
                model: "test".into(),
                allow_http: true,
                ..Config::default()
            };
            config.validate().unwrap();
            assert_eq!(
                config.endpoint(&config.chat_path).unwrap().as_str(),
                expected
            );
        }
    }

    #[test]
    fn ip_port_and_encoded_base_paths_are_preserved() {
        for host in ["https://192.0.2.1:8443", "https://[::1]:8443"] {
            let config = Config {
                server_url: format!("{host}/a%20b"),
                ..Config::default()
            };
            assert_eq!(
                config.endpoint("v1/chat/completions").unwrap().as_str(),
                format!("{host}/a%20b/v1/chat/completions")
            );
        }
    }

    #[test]
    fn flexible_routes_still_reject_unsafe_or_cross_origin_urls() {
        let config = Config {
            server_url: "http://intranet-host/gateway".into(),
            allow_http: true,
            ..Config::default()
        };
        for route in [
            "",
            "//other-host/chat",
            "/\\other-host/chat",
            "v1/cha\nt",
            "v1/chat?key=secret",
            "http://other-host/chat",
            "http://intranet-host:8080/chat",
            "https://intranet-host/chat",
            "http://user:password@intranet-host/chat",
            "http://intranet-host/chat#fragment",
            "file:///chat",
        ] {
            assert!(
                config.endpoint(route).is_err(),
                "unexpected accepted route: {route:?}"
            );
        }
        for website in [
            "http://intranet-host?key=secret",
            "http://intranet-host/#fragment",
            "http://user:password@intranet-host",
            "http://intranet-host/a\nb",
            "http://intranet-host\\gateway",
        ] {
            assert!(Config {
                server_url: website.into(),
                ..config.clone()
            }
            .base_url()
            .is_err());
        }
        let error = Config {
            allow_http: false,
            ..config
        }
        .base_url()
        .unwrap_err();
        assert!(error.contains("HTTP"));
    }

    #[test]
    fn resolved_routes_define_session_binding() {
        let mut config = Config {
            server_url: "https://intranet-host/gateway".into(),
            chat_path: "v1/chat/completions".into(),
            device_path: "oauth/device".into(),
            token_path: "oauth/token".into(),
            ..Config::default()
        };
        let before = config.binding().unwrap();
        config.chat_path = "https://intranet-host/gateway/v1/chat/completions".into();
        assert_eq!(before, config.binding().unwrap());
        config.token_path = "oauth/another-token".into();
        assert_ne!(before, config.binding().unwrap());
        config.token_path = "oauth/token".into();
        config.server_url = "https://intranet-host/another-gateway".into();
        assert_ne!(before, config.binding().unwrap());
    }

    #[test]
    fn previous_settings_keep_default_auth_routes() {
        let config: Config = serde_json::from_str(
            r#"{"server_url":"https://company.example","chat_path":"/v1/chat/completions","model":"test"}"#,
        ).unwrap();
        assert_eq!(config.device_path, DEVICE_PATH);
        assert_eq!(config.token_path, TOKEN_PATH);
        config.validate().unwrap();
    }
}
