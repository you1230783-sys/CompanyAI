//! 全站鈴鐺 adapter：共用網站資料、獨立游標；REST 成功前不永久修改本機快取。
use crate::{
    config::Config,
    notifications,
    storage::{self, Session},
    transport, AppResult,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};
use url::Url;

pub const PATH: &str = "/lm_server/api/desktop/notifications";
pub const SITE_ROOT: &str = "/lm_server/";
#[derive(Clone, Serialize, Deserialize)]
pub struct Notice {
    pub id: String,
    pub source: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub body: String,
    pub url: Option<String>,
    pub resource_id: Option<String>,
    pub created_at: String,
    pub is_read: bool,
    pub read_at: Option<String>,
    #[serde(default)]
    pub received_at: String,
}
#[derive(Deserialize)]
pub struct Page {
    pub notifications: Vec<Notice>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
    pub unread_count: usize,
    #[serde(default)]
    pub deleted_ids: Vec<String>,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Cache {
    pub binding: String,
    pub cursor: Option<String>,
    pub items: Vec<Notice>,
    pub unread_count: usize,
    pub initialized: bool,
    #[serde(skip)]
    pub suppress_popup: bool,
}
#[derive(Debug)]
pub struct Failure {
    pub status: u32,
    pub message: String,
}
impl From<String> for Failure {
    fn from(_: String) -> Self {
        Self {
            status: 0,
            message: "全站通知連線失敗或逾時，將稍後重試。".into(),
        }
    }
}
fn failure(status: u32) -> Failure {
    let message = match status {
        401 => "全站通知授權已失效，請重新登入。",
        403 => "沒有全站通知權限，請聯絡網站管理者。",
        404 => "網站尚未提供全站通知 API。",
        410 => "全站通知游標已到期，正在重新同步。",
        429 => "全站通知請求過於頻繁，稍後重試。",
        _ => "全站通知服務暫時無法使用。",
    };
    Failure {
        status,
        message: message.into(),
    }
}
fn id_valid(id: &str) -> bool {
    !id.is_empty()
        && !matches!(id, "." | "..")
        && id.len() <= 512
        && !id.chars().any(char::is_control)
}
impl Notice {
    fn validate(&self) -> AppResult<()> {
        if !id_valid(&self.id)
            || self.source.len() > 64
            || self.kind.len() > 100
            || self.title.len() > 1000
            || self.body.len() > 8000
            || self.url.as_ref().is_some_and(|u| u.len() > 4096)
            || self.resource_id.as_ref().is_some_and(|r| r.len() > 512)
            || chrono::DateTime::parse_from_rfc3339(&self.created_at).is_err()
            || self
                .read_at
                .as_ref()
                .is_some_and(|s| chrono::DateTime::parse_from_rfc3339(s).is_err())
        {
            return Err("全站通知資料不正確。".into());
        }
        Ok(())
    }
}
impl Cache {
    pub fn for_session(config: &Config, session: &Session) -> AppResult<Self> {
        Ok(Self {
            binding: format!(
                "{:x}",
                Sha256::digest(format!("{}|{}", config.binding()?, session.access_token))
            ),
            ..Default::default()
        })
    }
    /// 已讀與刪除完全採 server 值；不把本機舊已讀覆蓋到網站新狀態。
    pub fn merge(&mut self, page: Page) -> AppResult<()> {
        if page.notifications.len() > 100
            || page.deleted_ids.len() > 100
            || page.next_cursor.as_ref().is_some_and(|c| c.len() > 2048)
            || (page.has_more && (page.next_cursor.is_none() || page.next_cursor == self.cursor))
        {
            return Err("全站通知分頁資料不正確。".into());
        }
        for notice in &page.notifications {
            notice.validate()?;
        }
        if page.deleted_ids.iter().any(|id| !id_valid(id)) {
            return Err("通知刪除識別碼不正確。".into());
        }
        for mut notice in page.notifications {
            if let Some(old) = self.items.iter_mut().find(|n| n.id == notice.id) {
                notice.received_at = old.received_at.clone();
                *old = notice;
            } else {
                notice.received_at = notifications::now_text();
                self.items.push(notice);
            }
        }
        self.items.retain(|n| !page.deleted_ids.contains(&n.id));
        self.items.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        self.items.truncate(500);
        if page.next_cursor.is_some() {
            self.cursor = page.next_cursor;
        }
        self.unread_count = page.unread_count;
        Ok(())
    }
    pub fn new_unread_count(&self, old: &Self) -> usize {
        if !old.initialized || self.suppress_popup {
            return 0;
        }
        self.items
            .iter()
            .filter(|n| !n.is_read && !old.items.iter().any(|o| o.id == n.id))
            .count()
    }
}
fn page(config: &Config, session: &Session, cursor: Option<&str>) -> Result<Page, Failure> {
    if !session.valid_for(config) {
        return Err(failure(401));
    }
    let mut url = config.endpoint(PATH)?;
    url.query_pairs_mut().append_pair("limit", "100");
    if let Some(cursor) = cursor {
        url.query_pairs_mut().append_pair("after", cursor);
    }
    let response = transport::get(
        &url,
        Some(("Authorization", &format!("Bearer {}", session.access_token))),
    )?;
    if response.status != 200 {
        return Err(failure(response.status));
    }
    serde_json::from_str(&response.body).map_err(|_| Failure {
        status: 0,
        message: "全站通知 JSON 格式不正確。".into(),
    })
}
/// 分頁先存於副本；任一頁失敗保留原快取及游標，完整成功後才一次落盤。
pub fn sync(config: &Config, session: &Session, old: &Cache) -> Result<Cache, Failure> {
    let mut next = old.clone();
    next.suppress_popup = false;
    let mut reset = false;
    if !next.initialized || next.cursor.is_none() {
        next.items.clear();
        next.suppress_popup = true;
    }
    for _ in 0..100 {
        let page = match page(config, session, next.cursor.as_deref()) {
            Err(e) if e.status == 410 && !reset => {
                next = Cache::for_session(config, session)?;
                next.suppress_popup = true;
                reset = true;
                continue;
            }
            result => result?,
        };
        let more = page.has_more;
        next.merge(page)
            .map_err(|message| Failure { status: 0, message })?;
        if !more {
            next.initialized = true;
            return Ok(next);
        }
    }
    Err(Failure {
        status: 0,
        message: "全站通知分頁超出安全上限，保留既有快取。".into(),
    })
}
#[derive(Clone, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    Read { id: String, open: bool },
    ReadAll,
    DeleteAll,
}
pub fn action(config: &Config, session: &Session, action: &Action) -> Result<(), Failure> {
    if !session.valid_for(config) {
        return Err(failure(401));
    }
    let mut url = config.endpoint(PATH)?;
    let method = match action {
        Action::Read { id, .. } => {
            if !id_valid(id) {
                return Err(Failure {
                    status: 0,
                    message: "通知識別碼不正確。".into(),
                });
            }
            url.path_segments_mut()
                .map_err(|_| failure(400))?
                .push(id)
                .push("read");
            "POST"
        }
        Action::ReadAll => {
            url.path_segments_mut()
                .map_err(|_| failure(400))?
                .push("read-all");
            "POST"
        }
        Action::DeleteAll => "DELETE",
    };
    let response = transport::request_method(
        &url,
        method,
        "application/json",
        "{}",
        Some(("Authorization", &format!("Bearer {}", session.access_token))),
        15_000,
    )?;
    if matches!(response.status, 200 | 204) {
        Ok(())
    } else {
        Err(failure(response.status))
    }
}
/// root-relative URL 忠實使用 server 路徑；相對 URL 才以目前網站掛載點解析。
pub fn open_url(config: &Config, value: &str) -> AppResult<Url> {
    if value.is_empty()
        || value.len() > 4096
        || value.chars().any(char::is_control)
        || value.contains('\\')
        || value.starts_with("//")
    {
        return Err("通知連結不正確。".into());
    }
    let base = config.endpoint(SITE_ROOT)?;
    let url = base.join(value).map_err(|_| "通知連結無法解析。")?;
    if !matches!(url.scheme(), "http" | "https")
        || url.origin() != base.origin()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("通知只可開啟同一公司網站的 HTTP(S) 連結。".into());
    }
    Ok(url)
}
pub fn load(root: &Path, config: &Config, session: &Session) -> AppResult<Cache> {
    let empty = Cache::for_session(config, session)?;
    let path = root.join("site-notifications.dpapi");
    match fs::metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(empty),
        Err(_) => return Err("無法讀取全站通知快取。".into()),
        Ok(m) if m.len() > 8_000_000 => return Err("通知快取過大。".into()),
        _ => {}
    }
    let bytes = storage::protect(&fs::read(path).map_err(|_| "無法讀取通知快取。")?, false)?;
    let cache: Cache = serde_json::from_slice(&bytes).map_err(|_| "全站通知快取損毀。")?;
    if cache.binding != empty.binding {
        return Ok(empty);
    }
    if cache.items.len() > 500 || cache.cursor.as_ref().is_some_and(|c| c.len() > 2048) {
        return Err("通知快取不正確。".into());
    }
    for notice in &cache.items {
        notice.validate()?;
    }
    Ok(cache)
}
pub fn save(root: &Path, cache: &Cache) -> AppResult<()> {
    let bytes = serde_json::to_vec(cache).map_err(|_| "無法保存全站通知。")?;
    storage::atomic_write(
        &root.join("site-notifications.dpapi"),
        &storage::protect(&bytes, true)?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(read: bool) -> Page {
        serde_json::from_value(serde_json::json!({"notifications":[{"id":"kanban:123","source":"kanban","type":"mention","title":"test","body":"body","url":"kanban/1","created_at":"2026-09-17T00:00:00Z","is_read":read,"read_at":null}],"has_more":false,"next_cursor":"opaque+cursor","unread_count":if read{0}else{1}})).unwrap()
    }
    #[test]
    fn authoritative_read_delete_replay_and_first_sync() {
        let empty = Cache::default();
        let mut cache = empty.clone();
        cache.merge(page(false)).unwrap();
        assert_eq!(cache.new_unread_count(&empty), 0);
        cache.initialized = true;
        let old = cache.clone();
        cache.merge(page(false)).unwrap();
        assert_eq!(cache.items.len(), 1);
        assert_eq!(cache.new_unread_count(&old), 0);
        cache.merge(page(true)).unwrap();
        assert!(cache.items[0].is_read);
        cache.merge(page(false)).unwrap();
        assert!(!cache.items[0].is_read);
        let mut next = page(false);
        next.notifications[0].id = "briefing:1".into();
        cache.merge(next).unwrap();
        assert_eq!(cache.new_unread_count(&old), 1);
        let mut deleted = page(true);
        deleted.notifications.clear();
        deleted.deleted_ids = vec!["kanban:123".into()];
        cache.merge(deleted).unwrap();
        assert_eq!(cache.items.len(), 1);
        let mut invalid = page(false);
        invalid.has_more = true;
        assert!(cache.merge(invalid).is_err());
    }
    #[test]
    fn urls_and_errors_do_not_expose_secrets() {
        let config = Config::default();
        assert_eq!(
            open_url(&config, "kanban/123").unwrap().path(),
            "/lm_server/kanban/123"
        );
        assert_eq!(
            open_url(&config, "/lm_server/briefing/1").unwrap().path(),
            "/lm_server/briefing/1"
        );
        for value in [
            "javascript:alert(1)",
            "file:///C:/x",
            "//evil.test",
            "https://evil.test",
            "http://user@lp2-en-server/x",
        ] {
            assert!(open_url(&config, value).is_err());
        }
        for status in [401, 403, 429, 500] {
            assert!(!failure(status).message.contains("token"));
        }
    }
}

#[cfg(test)]
#[path = "site_notifications/http_tests.rs"]
mod http_tests;
