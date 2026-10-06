//! 專案 Outlook 的受控入口：先取得本次執行區段的明確同意，再讀取任何 Outlook 資料。
//! checkpoint 只保存快照與不透明代號；同意及 COM 連線絕不持久化。
use crate::{jobs, AppResult};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};
#[cfg(test)]
mod tests;

pub type Consent = Box<
    dyn FnMut(&AtomicBool, Instant) -> AppResult<Option<crate::outlook::privacy::Policy>> + Send,
>;

mod comparison;

/// 所有內部定位資料只寫入 DPAPI checkpoint，不直接序列化成工具回覆。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub path: String,
    pub scope: String,
    pub store: String,
    pub entry: String,
    pub children: usize,
    pub readable: bool,
    pub excluded: Vec<String>,
}
impl Folder {
    fn public(&self) -> Value {
        json!({"folder_id":self.id,"name":self.name,"path":self.path,"scope":self.scope,
            "child_count":self.children,"can_read_headers":self.readable})
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Header {
    #[serde(default)]
    pub thread_id: String,
    pub id: String,
    pub folder_id: String,
    pub subject: String,
    pub sender: String,
    pub recipients: Vec<String>,
    pub sent_at: String,
    pub received_at: String,
    pub entry: String,
    pub modified: String,
    /// 無法取得完整地址時不猜測去重；保留該封信並在結果說明。
    pub duplicate_key: Option<String>,
}
impl Header {
    fn public(&self) -> Value {
        let recipients: Vec<_> = self
            .recipients
            .iter()
            .take(8)
            .map(|s| s.chars().take(500).collect::<String>())
            .collect();
        json!({"mail_id":self.id,"thread_id":self.thread_id,"folder_id":self.folder_id,"subject":self.subject,
            "sender":self.sender,"recipients":recipients,"recipient_count":self.recipients.len(),"recipients_preview_truncated":self.recipients.len()>8 || self.recipients.iter().any(|s|s.chars().count()>500),"sent_at":self.sent_at,
            "received_at":self.received_at,"dedup_available":self.duplicate_key.is_some()})
    }
}
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Scan {
    pub headers: Vec<Header>,
    pub complete: bool,
    pub notices: Vec<String>,
}
/// 介面只提供唯讀操作；測試來源可驗證未同意時完全沒有呼叫讀取端。
pub trait Source {
    /// 快取回傳前仍核對原件與目前資料夾授權；測試來源可明確模擬移動／變更。
    fn verify(
        &mut self,
        _folder: &Folder,
        _header: &Header,
        _cancel: &AtomicBool,
    ) -> AppResult<()> {
        Ok(())
    }
    fn folders(
        &mut self,
        scope: &str,
        parent: Option<&Folder>,
        cancel: &AtomicBool,
    ) -> AppResult<(Vec<Folder>, Vec<String>)>;
    fn headers(
        &mut self,
        folder: &Folder,
        start: NaiveDate,
        end: NaiveDate,
        cancel: &AtomicBool,
    ) -> AppResult<Scan>;
    fn body(&mut self, folder: &Folder, header: &Header, cancel: &AtomicBool) -> AppResult<String>;
}
#[derive(Clone, Serialize, Deserialize)]
struct PageSet {
    folder: String,
    start: String,
    end: String,
    ids: Vec<String>,
    complete: bool,
    duplicates: usize,
    notices: Vec<String>,
}
#[derive(Default, Serialize, Deserialize)]
pub(super) struct Saved {
    #[serde(default)]
    privacy_revision: String,
    #[serde(default)]
    comparisons: BTreeMap<String, comparison::Report>,
    #[serde(default)]
    compared: std::collections::BTreeSet<String>,
    folders: BTreeMap<String, Folder>,
    mails: BTreeMap<String, Header>,
    pages: BTreeMap<String, PageSet>,
    bodies: BTreeMap<String, String>,
    seen: BTreeMap<String, String>,
}
#[derive(Default)]
pub(super) struct Session {
    pub saved: Saved,
    consent: Option<Consent>,
    allowed: Option<bool>,
    policy: crate::outlook::privacy::Policy,
    pub policy_root: Option<std::path::PathBuf>,
}
impl Session {
    pub fn has_snapshot(&self) -> bool {
        !self.saved.folders.is_empty()
    }
    pub fn check_policy(&self) -> AppResult<()> {
        let Some(root) = &self.policy_root else {
            return Ok(());
        };
        if self.policy.configured
            && self.policy.revision() != crate::outlook::privacy::Policy::load(root)?.revision()
        {
            return Err("Outlook 資料夾設定已改變，停止使用舊結果；請開新對話。".into());
        }
        Ok(())
    }
    pub fn is_allowed(&self) -> bool {
        self.allowed == Some(true)
    }
    pub fn set_consent(&mut self, consent: Option<Consent>) {
        self.consent = consent;
    }
    /// 對話文字、模型參數及舊 checkpoint 均不能取代 UI 的同意。
    pub fn authorize(&mut self, cancel: &AtomicBool, deadline: Instant) -> AppResult<bool> {
        if cancel.load(Ordering::Relaxed) {
            return Err("Outlook 讀取已取消。".into());
        }
        if let Some(allowed) = self.allowed {
            return Ok(allowed);
        }
        let selected = match self.consent.as_mut() {
            Some(confirm) => confirm(cancel, deadline)?,
            None => None,
        };
        self.allowed = Some(selected.is_some());
        if let Some(policy) = selected {
            let revision = policy.revision();
            if !self.saved.folders.is_empty() && self.saved.privacy_revision != revision {
                self.allowed = Some(false);
                return Err("Outlook 資料夾範圍已變更；舊快照不再使用，請開啟新對話執行。".into());
            }
            self.saved.privacy_revision = revision;
            self.policy = policy;
        }
        Ok(self.allowed == Some(true))
    }
    fn require_consent(&self) -> AppResult<()> {
        if self.allowed != Some(true) {
            return Err("尚未取得本次 Outlook 讀取同意。".into());
        }
        self.check_policy()
    }
    pub fn folders(
        &mut self,
        source: &mut dyn Source,
        scope: &str,
        parent_id: Option<&str>,
        offset: usize,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        self.require_consent()?;
        if !matches!(scope, "local_inbox" | "online_sent") {
            return Err("Outlook 範圍只能是 local_inbox 或 online_sent。".into());
        }
        let parent = parent_id
            .map(|id| {
                self.saved
                    .folders
                    .get(id)
                    .filter(|f| f.scope == scope)
                    .ok_or("資料夾代號不屬於本次已列出的範圍。".to_owned())
            })
            .transpose()?;
        let (mut folders, notices) = source.folders(scope, parent, cancel)?;
        folders.retain(|f| self.policy.permits(&f.store, &f.entry));
        folders.sort_by(|a, b| a.path.cmp(&b.path));
        if offset > folders.len() {
            return Err("資料夾清單已改變，請從第一頁重新列出。".into());
        }
        if self.saved.folders.len()
            + folders
                .iter()
                .filter(|f| !self.saved.folders.contains_key(&f.id))
                .count()
            > 5000
        {
            return Err("本次最多保存 5000 個 Outlook 資料夾代號，請縮小任務範圍。".into());
        }
        let total = folders.len();
        let mut page = Vec::new();
        let mut characters = 0;
        for folder in folders.into_iter().skip(offset).take(100) {
            let length = folder.public().to_string().chars().count();
            if !page.is_empty() && characters + length > 12_000 {
                break;
            }
            characters += length;
            page.push(folder);
        }
        let public: Vec<_> = page.iter().map(Folder::public).collect();
        for folder in page {
            self.saved.folders.insert(folder.id.clone(), folder);
        }
        let next = offset + public.len();
        Ok(
            json!({"folders":public,"next_offset":next,"has_more":next<total,"total":total,"notices":notices,
            "scope_note":"僅列名稱、不讀郵件。子資料夾需明確指定 parent_id；不自動展開或讀取全部信件。線上寄件備份使用 Outlook 信箱及其同步快取，不保證即時同步。"}),
        )
    }
    pub fn headers(
        &mut self,
        source: &mut dyn Source,
        folder_id: &str,
        start: &str,
        end: &str,
        cursor: Option<&str>,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        self.require_consent()?;
        let (page_id, offset) = if let Some(cursor) = cursor {
            let (id, offset) = cursor.rsplit_once(':').ok_or("郵件游標格式不正確。")?;
            (
                id.to_owned(),
                offset
                    .parse::<usize>()
                    .map_err(|_| "郵件游標位置不正確。")?,
            )
        } else {
            let first = NaiveDate::parse_from_str(start, "%Y-%m-%d")
                .map_err(|_| "起始日期需為 YYYY-MM-DD。")?;
            let last = NaiveDate::parse_from_str(end, "%Y-%m-%d")
                .map_err(|_| "結束日期需為 YYYY-MM-DD。")?;
            if last < first || (last - first).num_days() > 92 {
                return Err("每次郵件查詢需為 1 至 93 個日曆日。".into());
            }
            let folder = self
                .saved
                .folders
                .get(folder_id)
                .filter(|f| f.readable)
                .ok_or("請先列出並選擇可讀取郵件的資料夾。")?;
            if self.saved.pages.len() >= 100 || self.saved.mails.len() >= 4000 {
                return Err("本次郵件快照已達上限，請另開較小範圍的任務。".into());
            }
            let scan = source.headers(folder, first, last, cancel)?;
            if self.saved.mails.len() + scan.headers.len() > 4000 {
                return Err("郵件快照超過本次 4000 封上限，請縮小日期範圍。".into());
            }
            let mut ids = Vec::new();
            let mut duplicates = 0;
            let current_bytes = serde_json::to_vec(&self.saved.mails)
                .map_err(|e| e.to_string())?
                .len();
            if current_bytes
                + serde_json::to_vec(&scan.headers)
                    .map_err(|e| e.to_string())?
                    .len()
                > 4 * 1024 * 1024
            {
                return Err("本次標題快照超過 4 MiB，請縮小日期範圍。".into());
            }
            for mut header in scan.headers {
                // 再查同一封原件仍可看到它；副本去重不能讓重新查詢變成空清單。
                let existing = self
                    .saved
                    .mails
                    .values()
                    .find(|m| m.folder_id == header.folder_id && m.entry == header.entry)
                    .cloned();
                if let Some(existing) = existing {
                    header.id = existing.id;
                    if header.modified != existing.modified {
                        self.saved.bodies.remove(&header.id);
                    }
                    if !ids.contains(&header.id) {
                        ids.push(header.id.clone());
                    }
                    if let Some(key) = &header.duplicate_key {
                        self.saved.seen.insert(key.clone(), header.id.clone());
                    }
                    self.saved.mails.insert(header.id.clone(), header);
                    continue;
                }
                if let Some(key) = &header.duplicate_key {
                    if self.saved.seen.contains_key(key) {
                        duplicates += 1;
                        continue;
                    }
                    self.saved.seen.insert(key.clone(), header.id.clone());
                }
                ids.push(header.id.clone());
                self.saved.mails.insert(header.id.clone(), header);
            }
            let id = jobs::new_id()?;
            self.saved.pages.insert(
                id.clone(),
                PageSet {
                    folder: folder_id.into(),
                    start: start.into(),
                    end: end.into(),
                    ids,
                    complete: scan.complete,
                    duplicates,
                    notices: scan.notices,
                },
            );
            (id, 0)
        };
        let page = self
            .saved
            .pages
            .get(&page_id)
            .ok_or("郵件游標不屬於本次快照。")?;
        if page.folder != folder_id
            || page.start != start
            || page.end != end
            || offset > page.ids.len()
        {
            return Err("續頁條件與原郵件查詢不一致。".into());
        }
        let mut headers = Vec::new();
        let mut characters = 0;
        for id in page.ids.iter().skip(offset).take(40) {
            let saved = self.saved.mails.get(id).ok_or("郵件快照不完整。")?;
            let folder = self
                .saved
                .folders
                .get(&saved.folder_id)
                .ok_or("郵件資料夾快照不完整。")?;
            if !self.policy.permits(&folder.store, &folder.entry) {
                return Err("郵件不在已勾選範圍。".into());
            }
            source.verify(folder, saved, cancel)?;
            let header = saved.public();
            let length = header.to_string().chars().count();
            if !headers.is_empty() && characters + length > 12_000 {
                break;
            }
            characters += length;
            headers.push(header);
        }
        let next = offset + headers.len();
        Ok(
            json!({"headers":headers,"total_unique":page.ids.len(),"duplicates_omitted":page.duplicates,"has_more":next<page.ids.len(),
            "next_cursor":(next<page.ids.len()).then(||format!("{page_id}:{next}")),"complete":page.complete&&next==page.ids.len(),"scan_complete":page.complete,"notices":page.notices,
            "dedup_rule":"本次任務內，相同寄出時間、寄件地址及完整收件地址集合只保留一份；主旨不參與。無法取得完整地址時保留並標示。"}),
        )
    }
    pub fn body(
        &mut self,
        source: &mut dyn Source,
        mail_id: &str,
        offset: usize,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        self.require_consent()?;
        let header = self
            .saved
            .mails
            .get(mail_id)
            .ok_or("請先從標題清單挑選本次郵件代號。")?;
        let folder = self
            .saved
            .folders
            .get(&header.folder_id)
            .ok_or("缺少原郵件資料夾。")?;
        if !self.policy.permits(&folder.store, &folder.entry) {
            return Err("郵件不在已勾選範圍。".into());
        }
        source.verify(folder, header, cancel)?;
        if !self.saved.bodies.contains_key(mail_id) {
            if self.saved.bodies.len() >= 50 {
                return Err("本次 AI 最多閱讀 50 封不同郵件內文；請依已讀證據整理交付，不反覆查回相同內容。".into());
            }
            let folder = self
                .saved
                .folders
                .get(&header.folder_id)
                .ok_or("缺少原郵件資料夾。")?;
            let body = source.body(folder, header, cancel)?;
            if body.len() > 256_000 {
                return Err("單封郵件內文超過 256 KB，請由 Outlook 複製所需段落匯入專案。".into());
            }
            self.saved.bodies.insert(mail_id.into(), body);
        }
        let body = self
            .saved
            .bodies
            .get(mail_id)
            .ok_or("郵件內文快照不存在。")?;
        let total = body.chars().count();
        if offset > total {
            return Err("內文位置超過尾端。".into());
        }
        let text: String = body.chars().skip(offset).take(12_000).collect();
        let next = offset + text.chars().count();
        Ok(
            json!({"mail":header.public(),"text":text,"offset":offset,"next_offset":next,"has_more":next<total,"total_chars":total,"ai_read_count":self.saved.bodies.len(),"ai_read_remaining":50-self.saved.bodies.len(),"attachments_included":false,
            "scope":"本次選定郵件的唯讀純文字快照；附件未讀取，未更改未讀狀態。"}),
        )
    }
}

/// 同一郵件移動到不同本地資料夾後，SentOn 與 SMTP 地址仍可比對。
pub fn duplicate_key(sent: &str, sender: &str, recipients: &[String]) -> Option<String> {
    if sent.is_empty()
        || !sender.contains('@')
        || recipients.is_empty()
        || recipients.iter().any(|r| !r.contains('@'))
    {
        return None;
    }
    let mut addresses: Vec<_> = recipients
        .iter()
        .map(|r| r.trim().to_ascii_lowercase())
        .collect();
    addresses.sort();
    addresses.dedup();
    Some(super::text::revision(
        &serde_json::to_string(&(sent, sender.trim().to_ascii_lowercase(), addresses)).ok()?,
    ))
}
