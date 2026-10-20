//! 登入與聊天的工作流程，與介面分離，方便用本機測試伺服器驗證。
use crate::{
    config::{Config, CLIENT_ID},
    protocol::{self, DeviceGrant, OAuthError, TokenResponse},
    storage::Session,
    transport, AppResult,
};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

pub fn request_device(config: &Config) -> AppResult<DeviceGrant> {
    config.validate()?;
    let response = transport::post_form(
        &config.endpoint(&config.device_path)?,
        &[("client_id", CLIENT_ID), ("scope", "chat:write")],
    )
    .map_err(|error| format!("申請登入碼失敗（尚未開啟瀏覽器）：{error}"))?;
    if response.status != 200 {
        return Err(protocol::api_error(response.status, &response.body, ""));
    }
    let grant: DeviceGrant = serde_json::from_str(&response.body)
        .map_err(|_| "網站未回傳正確的登入碼 JSON，請對照 WEB_INTEGRATION.md。".to_string())?;
    grant.validate()?;
    config.verification_url(&grant.verification_uri)?;
    config.verification_url(grant.browser_url())?;
    Ok(grant)
}

/// 登入輪詢結果明確區分「尚未完成」與「應增加間隔」，遵守 RFC 8628 的節奏。
pub enum PollResult {
    Pending,
    SlowDown,
    Granted(Session),
}

pub fn poll_once(config: &Config, grant: &DeviceGrant) -> AppResult<PollResult> {
    let response = transport::post_form(
        &config.endpoint(&config.token_path)?,
        &[
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("device_code", &grant.device_code),
            ("client_id", CLIENT_ID),
        ],
    )
    .map_err(|error| format!("查詢登入授權結果失敗：{error}"))?;
    if response.status == 200 {
        let token: TokenResponse = serde_json::from_str(&response.body)
            .map_err(|_| "授權端點的 Token JSON 格式錯誤。".to_string())?;
        return Session::from_token(token, config).map(PollResult::Granted);
    }
    if matches!(response.status, 400 | 429) {
        let error: OAuthError = serde_json::from_str(&response.body)
            .map_err(|_| "授權端點必須回傳 JSON error。".to_string())?;
        return match error.error.as_str() {
            "authorization_pending" => Ok(PollResult::Pending),
            "slow_down" => Ok(PollResult::SlowDown),
            "access_denied" => Err("你已在網頁拒絕這次登入。".into()),
            "expired_token" => Err("一次性登入碼已過期或已使用，請重新登入。".into()),
            _ => Err("授權端點拒絕登入，請確認網站 client_id 與 grant_type 設定。".into()),
        };
    }
    Err(protocol::api_error(
        response.status,
        &response.body,
        &grant.device_code,
    ))
}

pub fn wait_for_login(
    config: &Config,
    grant: &DeviceGrant,
    cancelled: &AtomicBool,
) -> AppResult<Session> {
    let deadline = Instant::now() + Duration::from_secs(grant.expires_in);
    let mut interval = grant.interval;
    loop {
        // 以短暫睡眠等待，讓取消按鈕不用等到下一個輪詢間隔才生效。
        let next = Instant::now() + Duration::from_secs(interval);
        while Instant::now() < next {
            if cancelled.load(Ordering::Relaxed) {
                return Err("已取消登入。".into());
            }
            if Instant::now() >= deadline {
                return Err("登入碼已過期，請重新登入。".into());
            }
            thread::sleep(Duration::from_millis(100));
        }
        let result = poll_once(config, grant)?;
        if cancelled.load(Ordering::Relaxed) {
            return Err("已取消登入。".into());
        }
        if Instant::now() >= deadline {
            return Err("登入碼已過期，請重新登入。".into());
        }
        match result {
            PollResult::Granted(session) => return Ok(session),
            PollResult::Pending => {}
            PollResult::SlowDown => interval += 5,
        }
    }
}

// 僅保留舊協定的測試工具；正式桌面聊天全部走 jobs 的持久任務。
#[cfg(test)]
pub struct ChatOutcome {
    pub reply: AppResult<String>,
    pub unauthorized: bool,
}

/// 憑證放在 Header，JSON 本文只含模型與訊息，不把共用的上游 API Key 存入程式。
#[cfg(test)]
pub fn send_chat(config: &Config, session: &Session, body: &str) -> ChatOutcome {
    use crate::config::AuthHeader;
    let mut unauthorized = false;
    let reply = (|| {
        config.validate()?;
        if !session.valid_for(config) {
            unauthorized = true;
            return Err("登入已到期或設定已改變，請重新登入。".into());
        }
        let (name, value) = match config.auth_header {
            AuthHeader::Bearer => ("Authorization", format!("Bearer {}", session.access_token)),
            AuthHeader::XApiKey => ("X-API-Key", session.access_token.clone()),
        };
        let response = transport::request(
            &config.endpoint(&config.chat_path)?,
            "application/json; charset=utf-8",
            body,
            Some((name, &value)),
            120_000,
        )?;
        unauthorized = response.status == 401;
        if response.status != 200 {
            return Err(protocol::api_error(
                response.status,
                &response.body,
                &session.access_token,
            ));
        }
        protocol::assistant_text(&response.body)
    })();
    ChatOutcome {
        reply,
        unauthorized,
    }
}
