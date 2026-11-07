//! 登入與聊天的工作流程，與介面分離，方便用本機測試伺服器驗證。
use crate::{
    config::{Config, CLIENT_ID},
    protocol::{self, DeviceGrant, OAuthError, TokenResponse},
    storage::Session,
    transport, AppResult,
};
use sha2::{Digest, Sha256};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    thread,
    time::{Duration, Instant},
};

pub(crate) const UPGRADE_MESSAGE: &str = "伺服器已要求使用新的安全連線，請重新登入。";
static UPGRADE_SESSIONS: Mutex<Vec<[u8; 32]>> = Mutex::new(Vec::new());

/// 只保留需要失效的憑證指紋。較晚到達的舊請求不可登出目前新 Session。
pub(crate) fn request_transport_upgrade(secret: &str) {
    if secret.is_empty() {
        return;
    }
    let digest: [u8; 32] = Sha256::digest(secret.as_bytes()).into();
    if let Ok(mut pending) = UPGRADE_SESSIONS.lock() {
        if pending.contains(&digest) {
            return;
        }
        if pending.len() == 8 {
            pending.remove(0);
        }
        pending.push(digest);
    }
}

pub(crate) fn take_transport_upgrade(session: &Session) -> bool {
    let digest: [u8; 32] = Sha256::digest(session.access_token.as_bytes()).into();
    UPGRADE_SESSIONS.lock().is_ok_and(|mut pending| {
        pending
            .iter()
            .position(|entry| *entry == digest)
            .is_some_and(|index| {
                pending.remove(index);
                true
            })
    })
}

/// 登入碼與產生它的來源一起傳遞，輪詢或重開瀏覽器時不能切到另一來源。
#[derive(Clone)]
pub(crate) struct LoginGrant {
    pub grant: DeviceGrant,
    pub config: Config,
    allow_v1: bool,
    /// 只有通過公鑰驗證的 v2 停用回應，才能讓 HTTP Session 在防降級紀錄下恢復。
    verified_v1_fallback: bool,
}

pub(crate) enum LoginResult {
    Session(Session, Config, bool),
    StartV1(LoginGrant),
}

enum DeviceFailure {
    /// TCP 無法連線／逾時，只有尚無 v2 紀錄時允許回退。
    Unavailable(String),
    /// 已通過公鑰驗證的伺服器明確停用 v2，包含 test_accounts 與 off。
    Disabled(String),
    Rejected(String),
}

impl From<String> for DeviceFailure {
    fn from(error: String) -> Self {
        Self::Rejected(error)
    }
}

fn request_device_for(config: &Config) -> Result<DeviceGrant, DeviceFailure> {
    config.validate()?;
    let mut fields = vec![("client_id", CLIENT_ID), ("scope", "chat:write")];
    if config.uses_transport_v2() {
        fields.push(("transport_version", "2"));
    }
    let response =
        transport::post_form(&config.endpoint(&config.device_path)?, &fields).map_err(|error| {
            let message = format!("申請登入碼失敗（尚未開啟瀏覽器）：{error}");
            // 只辨識傳輸層自己產生的逾時與無法建立 TCP。憑證或 TLS 錯誤永不降級。
            if config.uses_transport_v2()
                && [12002, 12029]
                    .iter()
                    .any(|code| error.starts_with(&format!("Windows 網路錯誤 {code}（")))
            {
                DeviceFailure::Unavailable(message)
            } else {
                DeviceFailure::Rejected(message)
            }
        })?;
    if response.status != 200 {
        let message = protocol::api_error(response.status, &response.body, "");
        if config.uses_transport_v2() && transport_v2_unavailable(&response) {
            return Err(DeviceFailure::Disabled(message));
        }
        return Err(DeviceFailure::Rejected(message));
    }
    let grant: DeviceGrant = serde_json::from_str(&response.body)
        .map_err(|_| "網站未回傳正確的登入碼 JSON，請對照 WEB_INTEGRATION.md。".to_string())?;
    grant.validate()?;
    config.verification_url(&grant.verification_uri)?;
    config.verification_url(grant.browser_url())?;
    Ok(grant)
}

fn transport_v2_unavailable(response: &transport::HttpResponse) -> bool {
    response.status == 400
        && serde_json::from_str::<OAuthError>(&response.body)
            .is_ok_and(|error| error.error == "transport_v2_unavailable")
}

pub fn request_device(config: &Config) -> AppResult<DeviceGrant> {
    request_device_for(config).map_err(|error| match error {
        DeviceFailure::Unavailable(message)
        | DeviceFailure::Disabled(message)
        | DeviceFailure::Rejected(message) => message,
    })
}

pub(crate) fn begin_login(config: &Config) -> AppResult<LoginGrant> {
    begin_login_with(config, config.v2_candidate()?, request_device_for)
}

fn begin_login_with(
    config: &Config,
    secure: Option<Config>,
    mut request: impl FnMut(&Config) -> Result<DeviceGrant, DeviceFailure>,
) -> AppResult<LoginGrant> {
    let allow_unreachable_fallback = !config.uses_transport_v2();
    let mut verified_v1_fallback = false;
    if let Some(secure) = secure {
        match request(&secure) {
            Ok(grant) => {
                return Ok(LoginGrant {
                    grant,
                    config: secure,
                    allow_v1: true,
                    verified_v1_fallback: false,
                })
            }
            Err(DeviceFailure::Disabled(_)) => verified_v1_fallback = true,
            Err(DeviceFailure::Unavailable(_)) if allow_unreachable_fallback => (),
            Err(DeviceFailure::Unavailable(message) | DeviceFailure::Rejected(message)) => {
                return Err(message)
            }
        }
    } else if !allow_unreachable_fallback {
        return Err("此登入已使用 HTTPS v2，但正式公鑰未設定。".into());
    }
    let mut fallback = config.clone();
    if fallback.uses_transport_v2() {
        fallback.server_url = crate::config::SERVER_URL.into();
    }
    let grant = request(&fallback).map_err(|error| match error {
        DeviceFailure::Unavailable(message)
        | DeviceFailure::Disabled(message)
        | DeviceFailure::Rejected(message) => message,
    })?;
    Ok(LoginGrant {
        grant,
        config: fallback,
        allow_v1: false,
        verified_v1_fallback,
    })
}

/// 登入輪詢結果明確區分「尚未完成」與「應增加間隔」，遵守 RFC 8628 的節奏。
pub enum PollResult {
    Pending,
    SlowDown,
    Granted(Session),
    TransportUnavailable,
}

pub fn poll_once(config: &Config, grant: &DeviceGrant) -> AppResult<PollResult> {
    let mut fields = vec![
        ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ("device_code", grant.device_code.as_str()),
        ("client_id", CLIENT_ID),
    ];
    if config.uses_transport_v2() {
        fields.push(("transport_version", "2"));
    }
    let response = transport::post_form(&config.endpoint(&config.token_path)?, &fields)
        .map_err(|error| format!("查詢登入授權結果失敗：{error}"))?;
    parse_token_response(config, grant, response)
}

fn parse_token_response(
    config: &Config,
    grant: &DeviceGrant,
    response: transport::HttpResponse,
) -> AppResult<PollResult> {
    if response.status == 200 {
        let token: TokenResponse = serde_json::from_str(&response.body)
            .map_err(|_| "授權端點的 Token JSON 格式錯誤。".to_string())?;
        return Session::from_token(token, config).map(PollResult::Granted);
    }
    if config.uses_transport_v2() && transport_v2_unavailable(&response) {
        return Ok(PollResult::TransportUnavailable);
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
    wait_for_token(config, grant, cancelled)?.ok_or_else(|| "此帳號未提供 HTTPS v2 登入。".into())
}

pub(crate) fn finish_login(flow: LoginGrant, cancelled: &AtomicBool) -> AppResult<LoginResult> {
    let session = wait_for_token(&flow.config, &flow.grant, cancelled)?;
    finish_login_with(flow, session, cancelled, request_device)
}

fn finish_login_with(
    flow: LoginGrant,
    session: Option<Session>,
    cancelled: &AtomicBool,
    request: impl FnOnce(&Config) -> AppResult<DeviceGrant>,
) -> AppResult<LoginResult> {
    if let Some(session) = session {
        return Ok(LoginResult::Session(
            session,
            flow.config,
            flow.verified_v1_fallback,
        ));
    }
    if !flow.allow_v1 || cancelled.load(Ordering::Relaxed) {
        return Err("此登入無法使用 HTTPS v2，已停止連線。".into());
    }
    // 驗證頁已授權的 v2 裝置碼不可送給 HTTP。另取全新的 v1 授權。
    let mut config = flow.config;
    config.server_url = crate::config::SERVER_URL.into();
    let grant = request(&config)?;
    Ok(LoginResult::StartV1(LoginGrant {
        grant,
        config,
        allow_v1: false,
        verified_v1_fallback: true,
    }))
}

fn wait_for_token(
    config: &Config,
    grant: &DeviceGrant,
    cancelled: &AtomicBool,
) -> AppResult<Option<Session>> {
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
            PollResult::Granted(session) => return Ok(Some(session)),
            PollResult::TransportUnavailable => return Ok(None),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn grant(id: &str) -> DeviceGrant {
        serde_json::from_value(serde_json::json!({"device_code":id,"user_code":"ABC123",
            "verification_uri":"http://lp2-en-server/verify","expires_in":60,"interval":1}))
        .unwrap()
    }

    fn secure() -> Config {
        Config {
            server_url: crate::config::SERVER_URL_V2.into(),
            ..Default::default()
        }
    }

    #[test]
    fn verified_disabled_response_can_fallback_even_with_persistent_v2_lock() {
        let mut routes = Vec::new();
        let flow = begin_login_with(&secure(), Some(secure()), |config| {
            routes.push(config.uses_transport_v2());
            if config.uses_transport_v2() {
                Err(DeviceFailure::Disabled("transport_v2_unavailable".into()))
            } else {
                Ok(grant("fresh-v1"))
            }
        })
        .unwrap();
        assert_eq!(routes, [true, false]);
        assert_eq!(flow.grant.device_code, "fresh-v1");
        assert!(flow.verified_v1_fallback);
        assert_verified_fallback_survives_restart(flow);

        // test_accounts 通常在 token 輪詢後才得知帳號；同樣不可被 Windows 共用記錄擋住。
        let flow = begin_login_with(&secure(), Some(secure()), |_| Ok(grant("v2"))).unwrap();
        let result = finish_login_with(flow, None, &AtomicBool::new(false), |config| {
            assert!(!config.uses_transport_v2());
            Ok(grant("other-account-v1"))
        })
        .unwrap();
        let LoginResult::StartV1(flow) = result else {
            panic!("必須重新取得 HTTP 授權")
        };
        assert!(flow.verified_v1_fallback);
        assert_verified_fallback_survives_restart(flow);
    }

    /// 模擬同一 Windows 使用者先登入 v2、登出，再登入非測試帳號並重開。
    /// 使用真實 DPAPI 與正式保存／啟動路徑；伺服器回應由上方兩種協商流程注入。
    fn assert_verified_fallback_survives_restart(flow: LoginGrant) {
        use crate::storage;
        let root = std::env::temp_dir().join(format!(
            "lmai-login-restart-{}",
            crate::jobs::new_id().unwrap()
        ));
        let token = |config: &Config| {
            Session::from_token(
                TokenResponse {
                    access_token: "restart-fixture".into(),
                    token_type: "Bearer".into(),
                    expires_in: 3600,
                },
                config,
            )
            .unwrap()
        };
        storage::save_login_session(&root, &token(&secure()), &secure(), false).unwrap();
        storage::clear_session(&root).unwrap();
        let session = token(&flow.config);
        let result = finish_login_with(flow, Some(session), &AtomicBool::new(false), |_| {
            panic!("不應再次授權")
        })
        .unwrap();
        let LoginResult::Session(session, route, verified) = result else {
            panic!("缺少 HTTP Session")
        };
        storage::save_login_session(&root, &session, &route, verified).unwrap();
        for saved_config in [Config::default(), secure()] {
            let (restored, warning) =
                storage::transport_for_saved_session(&root, saved_config).unwrap();
            assert!(!restored.uses_transport_v2());
            assert!(warning.is_none());
            assert_eq!(
                storage::load_session(&root, &restored)
                    .unwrap()
                    .unwrap()
                    .access_token,
                session.access_token
            );
        }
        // 恢復 Session 不解除新登入的防降級限制，連不上仍不可回退。
        assert!(storage::transport_for_startup(&root, Config::default())
            .0
            .uses_transport_v2());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn negotiation_fallback_only_for_unavailable_v2_before_persistent_lock() {
        let normal = Config::default();
        let mut routes = Vec::new();
        let flow = begin_login_with(&normal, Some(secure()), |route| {
            routes.push(route.uses_transport_v2());
            if route.uses_transport_v2() {
                Err(DeviceFailure::Unavailable("逾時".into()))
            } else {
                Ok(grant("v1"))
            }
        })
        .unwrap();
        assert_eq!(routes, [true, false]);
        assert_eq!(flow.grant.device_code, "v1");
        assert!(!flow.verified_v1_fallback);
        for (route, unavailable) in [(&normal, false), (&secure(), true)] {
            let mut calls = 0;
            let result = begin_login_with(route, Some(secure()), |config| {
                calls += 1;
                assert!(config.uses_transport_v2());
                if unavailable {
                    Err(DeviceFailure::Unavailable("逾時".into()))
                } else {
                    Err(DeviceFailure::Rejected("TLS_PIN_MISMATCH".into()))
                }
            });
            assert!(result.is_err());
            assert_eq!(calls, 1);
        }
        let flow = begin_login_with(&normal, None, |route| {
            assert!(!route.uses_transport_v2());
            Ok(grant("placeholder"))
        })
        .unwrap();
        assert_eq!(flow.grant.device_code, "placeholder");
        assert!(!flow.verified_v1_fallback);
    }

    #[test]
    fn v2_token_success_stays_bound_and_explicit_rejection_uses_a_fresh_grant() {
        let response = |status, body: &str| transport::HttpResponse {
            status,
            body: body.into(),
        };
        let secure = secure();
        let original = grant("original-v2");
        let granted = parse_token_response(
            &secure,
            &original,
            response(
                200,
                r#"{"access_token":"fixture","token_type":"Bearer","expires_in":60}"#,
            ),
        )
        .unwrap();
        let PollResult::Granted(session) = granted else {
            panic!("缺少 v2 Session")
        };
        assert!(session.valid_for(&secure));
        assert!(!session.valid_for(&Config::default()));
        assert!(matches!(
            parse_token_response(
                &secure,
                &original,
                response(400, r#"{"error":"transport_v2_unavailable"}"#)
            )
            .unwrap(),
            PollResult::TransportUnavailable
        ));
        let cancel = AtomicBool::new(false);
        let mut flow = LoginGrant {
            grant: original,
            config: secure,
            allow_v1: true,
            verified_v1_fallback: false,
        };
        let result = finish_login_with(flow.clone(), None, &cancel, |route| {
            assert!(!route.uses_transport_v2());
            Ok(grant("fresh-v1"))
        })
        .unwrap();
        assert!(
            matches!(result, LoginResult::StartV1(next) if next.grant.device_code == "fresh-v1")
        );
        flow.allow_v1 = false;
        assert!(finish_login_with(flow, None, &cancel, |_| panic!("不應建立 v1 授權")).is_err());
    }

    #[test]
    fn upgrade_request_matches_only_the_rejected_session() {
        let config = Config::default();
        let token = |name: &str| {
            Session::from_token(
                TokenResponse {
                    access_token: name.into(),
                    token_type: "Bearer".into(),
                    expires_in: 60,
                },
                &config,
            )
            .unwrap()
        };
        let old = token("fixture-old");
        let new = token("fixture-new");
        let error = protocol::api_error(
            426,
            r#"{"error":"transport_upgrade_required"}"#,
            &old.access_token,
        );
        assert!(error.contains(UPGRADE_MESSAGE));
        assert!(!take_transport_upgrade(&new));
        assert!(take_transport_upgrade(&old));
        assert!(!take_transport_upgrade(&old));
    }
}
