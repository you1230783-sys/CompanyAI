//! 通知以 REST 事件紀錄為準；WebSocket 只喚醒補查，避免重連或分頁時漏事件。
use crate::{
    config::Config,
    storage::{self, Session},
    transport, unix_now, AppResult,
};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

pub const EVENTS_PATH: &str = "/lm_server/api/desktop/events";
pub const SOCKET_PATH: &str = "/lm_server/api/desktop/events/ws";
const MAX_SAVED_EVENTS: usize = 500;

#[derive(Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub summary: String,
    pub created_at: String,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub resource_id: Option<String>,
    #[serde(default)]
    pub read_at: Option<String>,
}
fn timestamp(value: &str) -> AppResult<i64> {
    DateTime::parse_from_rfc3339(value)
        .map(|d| d.timestamp())
        .map_err(|_| "通知時間格式不正確。".into())
}
impl Notification {
    fn validate(&self) -> AppResult<()> {
        if self.id.is_empty()
            || self.id.len() > 128
            || !self
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
            || self.title.trim().is_empty()
            || self.title.chars().count() > 150
            || self.summary.chars().count() > 2000
            || self.kind.len() > 100
        {
            return Err("通知資料格式不正確。".into());
        }
        timestamp(&self.created_at)?;
        if let Some(date) = &self.expires_at {
            timestamp(date)?;
        }
        if let Some(date) = &self.read_at {
            timestamp(date)?;
        }
        Ok(())
    }
    pub fn expired(&self) -> bool {
        self.expires_at
            .as_deref()
            .and_then(|s| timestamp(s).ok())
            .is_some_and(|t| t <= unix_now() as i64)
    }
}
#[derive(Deserialize)]
pub struct EventPage {
    pub events: Vec<Notification>,
    pub next_cursor: Option<String>,
    #[serde(default)]
    pub has_more: bool,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Inbox {
    /// 指紋只用來隔離不同 Token 的本機通知游標，不代替後端的帳號授權。
    pub session_fingerprint: String,
    pub cursor: Option<String>,
    pub events: Vec<Notification>,
}
impl Inbox {
    pub fn for_session(session: &Session) -> Self {
        Self {
            session_fingerprint: format!("{:x}", Sha256::digest(session.access_token.as_bytes())),
            ..Self::default()
        }
    }
    pub fn unread_count(&self) -> usize {
        self.events
            .iter()
            .filter(|e| e.read_at.is_none() && !e.expired())
            .count()
    }
    /// 合併完成並成功落盤後，呼叫端才採用游標；重複補查不會產生第二份通知。
    pub fn merge(&mut self, page: EventPage) -> AppResult<usize> {
        if page.events.len() > 100
            || page.next_cursor.as_ref().is_some_and(|c| c.len() > 2048)
            || (page.has_more && (page.next_cursor.is_none() || page.next_cursor == self.cursor))
        {
            return Err("通知分頁資料不正確。".into());
        }
        for event in &page.events {
            event.validate()?;
        }
        let mut added = 0;
        for mut event in page.events {
            if let Some(existing) = self.events.iter_mut().find(|e| e.id == event.id) {
                // 已讀操作可能先於落後的補查回應；不要把已讀回退成未讀。
                if event.read_at.is_none() {
                    event.read_at = existing.read_at.clone();
                }
                *existing = event;
            } else {
                if !event.expired() && event.read_at.is_none() {
                    added += 1;
                }
                self.events.push(event);
            }
        }
        self.events.retain(|e| !e.expired());
        self.events
            .sort_by_key(|e| std::cmp::Reverse(timestamp(&e.created_at).unwrap_or(0)));
        self.events.truncate(MAX_SAVED_EVENTS);
        if page.next_cursor.is_some() {
            self.cursor = page.next_cursor;
        }
        Ok(added)
    }
}
pub fn fetch_page(
    config: &Config,
    session: &Session,
    cursor: Option<&str>,
) -> AppResult<EventPage> {
    if !session.valid_for(config) {
        return Err("請先登入才能接收通知。".into());
    }
    let mut url = config.endpoint(EVENTS_PATH)?;
    if let Some(cursor) = cursor {
        url.query_pairs_mut().append_pair("after", cursor);
    }
    let token = format!("Bearer {}", session.access_token);
    let response = transport::get(&url, Some(("Authorization", &token)))?;
    if response.status != 200 {
        return Err("暫時無法同步通知；登入有效時會自動重試。".into());
    }
    serde_json::from_str(&response.body).map_err(|_| "網站通知 JSON 格式不正確。".into())
}
pub fn mark_read(config: &Config, session: &Session, id: &str) -> AppResult<()> {
    if !session.valid_for(config) {
        return Err("請重新登入後標記已讀。".into());
    }
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
    {
        return Err("通知識別碼不正確。".into());
    }
    let token = format!("Bearer {}", session.access_token);
    let response = transport::request(
        &config.endpoint(&format!("{EVENTS_PATH}/{id}/read"))?,
        "application/json",
        "{}",
        Some(("Authorization", &token)),
        15_000,
    )?;
    if matches!(response.status, 200 | 204) {
        Ok(())
    } else {
        Err("標記已讀失敗，請稍後再試。".into())
    }
}
pub fn now_text() -> String {
    DateTime::<Utc>::from_timestamp(unix_now() as i64, 0)
        .unwrap_or_default()
        .to_rfc3339_opts(SecondsFormat::Secs, true)
}
pub fn load(root: &Path, session: &Session) -> AppResult<Inbox> {
    let fresh = Inbox::for_session(session);
    let path = root.join("notifications.dpapi");
    let meta = match fs::metadata(&path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(fresh),
        Err(_) => return Err("無法讀取通知快取。".into()),
    };
    if meta.len() > 8_000_000 {
        return Err("通知快取過大。".into());
    }
    let bytes = storage::protect(&fs::read(path).map_err(|_| "無法讀取通知快取。")?, false)?;
    let inbox: Inbox = serde_json::from_slice(&bytes).map_err(|_| "通知快取格式不正確。")?;
    if inbox.session_fingerprint != fresh.session_fingerprint {
        return Ok(fresh);
    }
    if inbox.events.len() > MAX_SAVED_EVENTS {
        return Err("通知快取數量不正確。".into());
    }
    for event in &inbox.events {
        event.validate()?;
    }
    Ok(inbox)
}
pub fn save(root: &Path, inbox: &Inbox) -> AppResult<()> {
    let bytes = serde_json::to_vec(inbox).map_err(|e| e.to_string())?;
    storage::atomic_write(
        &root.join("notifications.dpapi"),
        &storage::protect(&bytes, true)?,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    fn event() -> Notification {
        Notification {
            id: "evt_test".into(),
            kind: "notice".into(),
            title: "測試通知".into(),
            summary: "".into(),
            created_at: now_text(),
            expires_at: None,
            resource_id: None,
            read_at: None,
        }
    }
    #[test]
    fn event_replay_deduplicates_and_preserves_read_state() {
        let mut inbox = Inbox::default();
        let e = event();
        assert_eq!(
            inbox
                .merge(EventPage {
                    events: vec![e.clone()],
                    next_cursor: Some("c1".into()),
                    has_more: false
                })
                .unwrap(),
            1
        );
        inbox.events[0].read_at = Some(now_text());
        assert_eq!(
            inbox
                .merge(EventPage {
                    events: vec![e],
                    next_cursor: Some("c2".into()),
                    has_more: false
                })
                .unwrap(),
            0
        );
        assert_eq!(inbox.events.len(), 1);
        assert_eq!(inbox.unread_count(), 0);
        assert!(inbox
            .merge(EventPage {
                events: vec![],
                next_cursor: Some("c2".into()),
                has_more: true
            })
            .is_err());
        let mut expired = event();
        expired.expires_at = Some("2000-01-01T00:00:00Z".into());
        inbox
            .merge(EventPage {
                events: vec![expired],
                next_cursor: None,
                has_more: false,
            })
            .unwrap();
        assert!(inbox.events.is_empty());
    }
}
