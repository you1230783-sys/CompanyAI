//! 私聊 API 由原生端建立固定同源路徑；命令只帶經驗證的聊天 ID 或好友／邀請代號。
use super::{
    crypto::Secret,
    vault::{ServerSecret, PASSWORD_MISMATCH},
};
use crate::{
    config::Config,
    storage::Session,
    transport::{self, pins::Pins},
    AppResult,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeSet, ptr};
use url::Url;

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct Profile {
    #[serde(default)]
    pub(crate) chat_id: String,
    pub(crate) nickname: String,
    pub(crate) employee_no: String,
}

impl Drop for Profile {
    fn drop(&mut self) {
        super::clear_text(&mut self.chat_id);
        super::clear_text(&mut self.nickname);
        super::clear_text(&mut self.employee_no);
    }
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct Friend {
    pub(crate) friend_id: String,
    pub(crate) nickname: String,
    pub(crate) employee_no: String,
    pub(crate) status: String,
}

impl Drop for Friend {
    fn drop(&mut self) {
        super::clear_text(&mut self.friend_id);
        super::clear_text(&mut self.nickname);
        super::clear_text(&mut self.employee_no);
    }
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct Invitation {
    #[serde(alias = "id")]
    pub(crate) invite_id: String,
    pub(crate) nickname: String,
    pub(crate) employee_no: String,
    #[serde(default)]
    pub(crate) status: String,
    #[serde(default, alias = "server_time")]
    pub(crate) created_at: String,
}

impl Drop for Invitation {
    fn drop(&mut self) {
        super::clear_text(&mut self.invite_id);
        super::clear_text(&mut self.nickname);
        super::clear_text(&mut self.employee_no);
        super::clear_text(&mut self.created_at);
    }
}

#[derive(Clone, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct Invitations {
    #[serde(default, alias = "received")]
    pub(crate) incoming: Vec<Invitation>,
    #[serde(default, alias = "sent")]
    pub(crate) outgoing: Vec<Invitation>,
}

#[derive(Clone, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct Contacts {
    pub(crate) profile: Option<Profile>,
    pub(crate) friends: Vec<Friend>,
    pub(crate) invites: Invitations,
    pub(crate) blocked: Vec<Profile>,
}

impl Contacts {
    pub(crate) fn is_blocked(&self, employee: &str) -> bool {
        self.blocked
            .iter()
            .any(|entry| entry.employee_no == employee)
    }

    pub(crate) fn merge_friends(&mut self, mut friends: Vec<Friend>) {
        let seen: BTreeSet<_> = friends
            .iter()
            .map(|friend| friend.friend_id.clone())
            .collect();
        friends.extend(self.friends.drain(..).filter_map(|mut friend| {
            if seen.contains(&friend.friend_id) {
                return None;
            }
            friend.status = "removed".into();
            Some(friend)
        }));
        self.friends = friends;
    }
}

pub(crate) fn chat_id(text: &str) -> AppResult<String> {
    const ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let value = text.trim().to_ascii_uppercase();
    let valid = value.len() == 9
        && value.bytes().enumerate().all(|(index, byte)| {
            if index == 4 {
                byte == b'-'
            } else {
                ALPHABET.contains(&byte)
            }
        });
    if !valid {
        return Err("聊天 ID 必須為 8 碼，格式為 XXXX-XXXX。".into());
    }
    Ok(value)
}

/// 僅回傳固定、可公開顯示的錯誤；伺服器錯誤本文不進診斷。
#[derive(Debug)]
pub(crate) struct Error {
    pub(crate) message: String,
    pub(crate) unavailable: bool,
    pub(crate) replaced: bool,
    pub(crate) password_failed: bool,
}

impl From<String> for Error {
    fn from(message: String) -> Self {
        Self {
            password_failed: message == PASSWORD_MISMATCH,
            message,
            unavailable: false,
            replaced: false,
        }
    }
}

#[derive(Clone)]
pub(crate) struct Api {
    base: Url,
    session: Session,
    device: String,
    test_pins: Option<Pins>,
}

impl Api {
    pub(crate) fn new(config: &Config, session: &Session, device: &str) -> AppResult<Self> {
        if !config.uses_transport_v2()
            || transport::pins::configured()?.is_none()
            || !session.valid_for(config)
            || !super::valid_device(device)
        {
            return Err("私聊需要目前帳號的安全連線及啟用裝置。".into());
        }
        Ok(Self {
            base: config.endpoint("/lm_server/api/desktop/chat/")?,
            session: session.clone(),
            device: device.into(),
            test_pins: None,
        })
    }

    #[cfg(test)]
    fn fixture(base: Url, test_pins: Pins) -> Self {
        Self {
            base,
            session: Session {
                access_token: "fixture-private-chat".into(),
                expires_at: crate::unix_now() + 600,
                binding: String::new(),
            },
            device: "d9b0160f-dedb-4a44-8c31-5c6a6f9a3af2".into(),
            test_pins: Some(test_pins),
        }
    }

    fn call(
        &self,
        method: &str,
        route: &str,
        query: Option<(&str, &str)>,
        body: Option<Value>,
    ) -> Result<Secret<Vec<u8>>, Error> {
        if self.session.expires_at <= crate::unix_now() {
            return Err("登入已到期，請重新登入。".to_string().into());
        }
        let mut url = self
            .base
            .join(route)
            .map_err(|_| "私聊 API 路徑不正確。".to_string())?;
        if url.origin() != self.base.origin() || !url.path().starts_with(self.base.path()) {
            return Err("私聊 API 來源不符。".to_string().into());
        }
        if let Some((key, value)) = query {
            url.query_pairs_mut().append_pair(key, value);
        }
        let body = Secret(body.map_or_else(Vec::new, |value| value.to_string().into_bytes()));
        let authorization = format!("Bearer {}", self.session.access_token);
        let response = transport::chat_request(
            &url,
            method,
            &body.0,
            &authorization,
            &self.device,
            self.test_pins.as_ref(),
        )?;
        let raw = Secret(response.body.into_bytes());
        if (200..300).contains(&response.status) {
            return Ok(raw);
        }
        #[derive(Deserialize)]
        struct Rejection {
            error: String,
        }
        let code = serde_json::from_slice::<Rejection>(&raw.0)
            .ok()
            .map(|value| value.error);
        let (message, unavailable, replaced) = match (response.status, code.as_deref()) {
            (409, Some("device_replaced")) => ("私聊已在另一台電腦啟用。".into(), true, true),
            (403, Some("chat_disabled")) => ("伺服器目前未開放此帳號私聊。".into(), true, false),
            (401 | 426, Some("transport_upgrade_required")) => {
                crate::auth::request_transport_upgrade(&self.session.access_token);
                (crate::auth::UPGRADE_MESSAGE.into(), true, false)
            }
            (404, _) => (
                "找不到此聊天 ID 或此對象目前無法使用私聊。".into(),
                false,
                false,
            ),
            (429, _) => ("私聊操作過於頻繁，請稍後再試。".into(), false, false),
            (409, Some("recipient_paused")) => (
                "對方超過 72 小時未上線，暫停收取新訊息。".into(),
                false,
                false,
            ),
            (409, Some("already_friends")) => ("你們已經是好友。".into(), false, false),
            (status, _) => (
                format!("私聊服務拒絕此操作（HTTP {status}）。"),
                false,
                false,
            ),
        };
        Err(Error {
            message,
            unavailable,
            replaced,
            password_failed: false,
        })
    }

    fn read<T: DeserializeOwned>(
        &self,
        method: &str,
        route: &str,
        query: Option<(&str, &str)>,
    ) -> Result<T, Error> {
        let body = self.call(method, route, query, None)?;
        serde_json::from_slice(&body.0).map_err(|_| "私聊服務回應格式不正確。".to_string().into())
    }

    pub(crate) fn activate(&self) -> Result<(), Error> {
        self.call(
            "POST",
            "devices/activate",
            None,
            Some(json!({"device_id": self.device})),
        )?;
        Ok(())
    }

    pub(crate) fn profile(&self) -> Result<Profile, Error> {
        self.read("GET", "me", None)
    }

    pub(crate) fn regenerate(&self) -> Result<(), Error> {
        self.call("POST", "me/chat-id/regenerate", None, None)?;
        Ok(())
    }

    pub(crate) fn lookup(&self, code: &str) -> Result<Profile, Error> {
        let code = chat_id(code)?;
        let mut profile: Profile = self.read("GET", "lookup", Some(("chat_id", &code)))?;
        profile.chat_id = code;
        Ok(profile)
    }

    pub(crate) fn invite(&self, code: &str) -> Result<(), Error> {
        self.call(
            "POST",
            "invites",
            None,
            Some(json!({"chat_id": chat_id(code)?})),
        )?;
        Ok(())
    }

    pub(crate) fn invites(&self) -> Result<Invitations, Error> {
        self.read("GET", "invites", None)
    }

    pub(crate) fn answer(&self, id: &str, accept: bool) -> Result<(), Error> {
        crate::jobs::validate_id(id)?;
        let action = if accept { "accept" } else { "reject" };
        self.call("POST", &format!("invites/{id}/{action}"), None, None)?;
        Ok(())
    }

    pub(crate) fn friends(&self) -> Result<Vec<Friend>, Error> {
        self.read("GET", "friends", None)
    }

    pub(crate) fn refresh_contacts(
        &self,
        blocked: &[Profile],
    ) -> Result<(Profile, Vec<Friend>, Invitations), Error> {
        let profile = self.profile()?;
        let friends = self.friends()?;
        let mut invitations = self.invites()?;
        let mut auto_reject = Vec::new();
        invitations.incoming.retain(|invite| {
            let blocked = blocked
                .iter()
                .any(|entry| entry.employee_no == invite.employee_no);
            if blocked {
                auto_reject.push(invite.invite_id.clone());
            }
            !blocked
        });
        for id in auto_reject {
            if let Err(error) = self.answer(&id, false) {
                if error.unavailable {
                    return Err(error);
                }
                // 一般網路失敗時仍隱藏該邀請；下次同步再重試拒絕。
            }
        }
        Ok((profile, friends, invitations))
    }

    pub(crate) fn remove_friend(&self, id: &str) -> Result<(), Error> {
        crate::jobs::validate_id(id)?;
        self.call("DELETE", &format!("friends/{id}"), None, None)?;
        Ok(())
    }

    pub(crate) fn key_secret(&self, reset: bool) -> Result<ServerSecret, Error> {
        use windows_sys::Win32::Security::Cryptography::{
            CryptStringToBinaryA, CRYPT_STRING_BASE64, CRYPT_STRING_STRICT,
        };
        #[derive(Deserialize)]
        struct Wire {
            secret: String,
            version: u64,
        }
        let (method, route) = if reset {
            ("POST", "key-secret/reset")
        } else {
            ("GET", "key-secret")
        };
        let raw = self.call(method, route, None, None)?;
        let wire: Wire =
            serde_json::from_slice(&raw.0).map_err(|_| "聊天秘密回應格式錯誤。".to_string())?;
        let encoded = Secret(wire.secret.into_bytes());
        if encoded.0.len() != 44 || encoded.0.last() != Some(&b'=') || wire.version == 0 {
            return Err("聊天秘密格式錯誤。".to_string().into());
        }
        let mut value = Secret([0; 32]);
        let mut size = 32;
        // SAFETY: 來源及輸出緩衝區皆在同步呼叫期間有效。
        let decoded = unsafe {
            CryptStringToBinaryA(
                encoded.0.as_ptr(),
                44,
                CRYPT_STRING_BASE64 | CRYPT_STRING_STRICT,
                value.0.as_mut_ptr(),
                &mut size,
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };
        if decoded == 0 || size != 32 {
            return Err("聊天秘密格式錯誤。".to_string().into());
        }
        Ok(ServerSecret {
            version: wire.version,
            value,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_needs_authenticated_v2_and_valid_ids() {
        let config = Config::default();
        let session = Session {
            access_token: "fixture".into(),
            expires_at: crate::unix_now() + 300,
            binding: config.binding().unwrap(),
        };
        assert!(Api::new(&config, &session, &super::super::uuid().unwrap()).is_err());
        assert_eq!(chat_id(" 0123-abcd ").unwrap(), "0123-ABCD");
        for invalid in ["0123-IJKL", "0123/ABCD", "0123-ABCD?q", "Ｏ123-ABCD"] {
            assert!(chat_id(invalid).is_err());
        }
    }

    #[test]
    #[ignore = "需要 scripts/Test-PrivateChat.py 啟動本機 TLS"]
    fn tls_private_chat_fixture() {
        let get = |name| std::env::var(name).expect("本機私聊 TLS fixture 參數");
        let base = Url::parse(&get("LMAI_CHAT_FIXTURE_URL")).unwrap();
        let pins = Pins::parse(
            &get("LMAI_CHAT_FIXTURE_PRIMARY"),
            &get("LMAI_CHAT_FIXTURE_BACKUP"),
        )
        .unwrap();
        let api = Api::fixture(base, pins);
        api.activate().unwrap();
        assert_eq!(api.profile().unwrap().chat_id, "0123-ABCD");
        let profile = api.lookup("4567-EFGH").unwrap();
        assert_eq!(profile.nickname, "邀請對象");
        assert_eq!(profile.employee_no, "200");
        api.invite(&profile.chat_id).unwrap();
        let blocked = Profile {
            chat_id: String::new(),
            nickname: "封鎖測試".into(),
            employee_no: "500".into(),
        };
        let (_, _, invitations) = api.refresh_contacts(&[blocked]).unwrap();
        assert_eq!(invitations.incoming.len(), 2);
        assert!(invitations
            .incoming
            .iter()
            .all(|item| item.employee_no != "500"));
        api.answer("accept-fixture", true).unwrap();
        api.answer("reject-fixture", false).unwrap();
        let friends = api.friends().unwrap();
        assert_eq!(friends[0].nickname, "接受測試");
        api.regenerate().unwrap();
        assert_eq!(api.profile().unwrap().chat_id, "9876-WXYZ");
        assert_eq!(api.friends().unwrap()[0].friend_id, "friend-fixture");
        api.remove_friend("friend-fixture").unwrap();
        assert!(api.friends().unwrap().is_empty());
        let secret = api.key_secret(false).unwrap();
        let reset = api.key_secret(true).unwrap();
        assert_eq!(reset.version, secret.version + 1);
        assert_ne!(reset.value.0, secret.value.0);
        let replaced = api.profile().err().expect("裝置已被取代");
        assert!(replaced.unavailable && replaced.replaced);
    }
}
