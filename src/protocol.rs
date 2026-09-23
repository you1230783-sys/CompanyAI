//! 對外 JSON 契約集中在這裡，網站實作者可對照 docs/WEB_INTEGRATION.md。
use crate::{config::MAX_SESSION_SECONDS, AppResult};
use serde::{Deserialize, Serialize};

mod legacy_reply;
mod reply;
pub use reply::{assistant_message, assistant_text, preserve_received_reply, ReplyPayload};

/// 保留不能安全併入完成結果的原始文字，讓使用者仍能展開查看。
#[derive(Clone, Serialize, Deserialize)]
pub struct ReceivedReply {
    pub text: String,
    pub from_stream: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<String>,
    /// 本機串流中斷標記；不加入對外 Chat Completions 的訊息欄位。
    #[serde(default)]
    pub incomplete: bool,
    /// 正規化的結構化回答；本機保存及畫面使用，不當成 API 工具或指令。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_payload: Option<ReplyPayload>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub received_replies: Vec<ReceivedReply>,
}

impl Message {
    pub fn user(content: &str) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
            request_id: None,
            attachments: Vec::new(),
            incomplete: false,
            response_payload: None,
            received_replies: Vec::new(),
        }
    }
    pub fn assistant(content: String) -> Self {
        Self {
            role: "assistant".into(),
            content,
            request_id: None,
            attachments: Vec::new(),
            incomplete: false,
            response_payload: None,
            received_replies: Vec::new(),
        }
    }
}

/// 第一版使用 Chat Completions 的文字訊息與非串流回應。
#[derive(Serialize)]
pub struct ChatRequest<'a> {
    pub model: &'a str,
    pub messages: Vec<serde_json::Value>,
    pub stream: bool,
}

pub fn chat_json(model: &str, messages: &[Message]) -> AppResult<String> {
    if messages.is_empty() || messages.len() > 40 {
        return Err("每次最多保留 20 輪對話；請清除對話後再試。".into());
    }
    if messages
        .iter()
        .any(|message| message.content.len() > 64_000)
    {
        return Err("單則訊息最多 64 KB，請縮短內容。".into());
    }
    serde_json::to_string_pretty(&ChatRequest {
        model,
        messages: messages
            .iter()
            .map(|m| serde_json::json!({"role":m.role,"content":m.content}))
            .collect(),
        stream: false,
    })
    .map_err(|error| error.to_string())
}

#[derive(Clone, Deserialize)]
pub struct DeviceGrant {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    pub expires_in: u64,
    #[serde(default = "default_interval")]
    pub interval: u64,
}
fn default_interval() -> u64 {
    5
}

impl DeviceGrant {
    pub fn validate(&self) -> AppResult<()> {
        if self.device_code.is_empty()
            || self.device_code.len() > 2048
            || self.user_code.is_empty()
            || self.user_code.len() > 64
            || !(1..=900).contains(&self.expires_in)
            || !(1..=60).contains(&self.interval)
        {
            return Err("登入碼回應格式不符：有效期需為 1–900 秒，輪詢間隔需為 1–60 秒。".into());
        }
        Ok(())
    }
    pub fn browser_url(&self) -> &str {
        self.verification_uri_complete
            .as_deref()
            .unwrap_or(&self.verification_uri)
    }
}

/// 不實作 Debug，避免把登入憑證意外印到 log。
#[derive(Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
}
impl TokenResponse {
    pub fn validate(&self) -> AppResult<()> {
        if !self.token_type.eq_ignore_ascii_case("Bearer")
            || self.access_token.is_empty()
            || self.access_token.len() > 8192
            || !self
                .access_token
                .bytes()
                .all(|byte| byte.is_ascii_graphic())
            || self.expires_in == 0
            || self.expires_in > MAX_SESSION_SECONDS
        {
            return Err("授權回應無效：需提供 Bearer access_token，期限為 1 秒至 30 天。".into());
        }
        Ok(())
    }
}

#[derive(Deserialize)]
pub struct OAuthError {
    pub error: String,
}

/// API 錯誤只顯示必要訊息，並遮蔽當次憑證，避免錯誤回應意外洩漏 Header。
pub fn api_error(status: u32, body: &str, secret: &str) -> String {
    let hint = match status {
        401 => "登入已到期或 API Key 無效，請重新登入。",
        403 => "此帳號沒有呼叫此 API 或模型的權限。",
        404 => "找不到服務，請聯絡管理者確認部署。",
        426 => "此版本已停止支援，請按「下載更新」取得新版。",
        429 => "請求過多，請稍後再試。",
        _ => "API 請求失敗，請檢查網站服務。",
    };
    let parsed = serde_json::from_str::<serde_json::Value>(body).ok();
    let detail = parsed
        .as_ref()
        .and_then(|value| value.get("error")?.get("message")?.as_str())
        .unwrap_or("");
    let redacted = if secret.is_empty() {
        detail.to_string()
    } else {
        detail.replace(secret, "[已遮蔽]")
    };
    format!(
        "HTTP {status}：{hint}\n{}",
        redacted.chars().take(300).collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chat_contract_preserves_unicode_and_escapes() {
        let messages = vec![Message::user("你好 \"測試\"\n第二行")];
        let value: serde_json::Value =
            serde_json::from_str(&chat_json("my-model", &messages).unwrap()).unwrap();
        assert_eq!(value["messages"][0]["content"], messages[0].content);
        assert_eq!(value["stream"], false);
        assert_eq!(
            assistant_text(r#"{"choices":[{"message":{"content":"成功"}}]}"#).unwrap(),
            "成功"
        );
        assert!(assistant_text(r#"{"choices":[]}"#).is_err());
    }
    #[test]
    fn refuses_header_injection_and_unbounded_tokens() {
        let mut token = TokenResponse {
            access_token: "key\r\nx-header: injected".into(),
            token_type: "Bearer".into(),
            expires_in: 100,
        };
        assert!(token.validate().is_err());
        token.access_token = "normal-key".into();
        token.expires_in = MAX_SESSION_SECONDS + 1;
        assert!(token.validate().is_err());
        assert!(!api_error(
            500,
            r#"{"error":{"message":"bad normal-key"}}"#,
            "normal-key"
        )
        .contains("normal-key"));
    }
}
