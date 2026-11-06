//! 私聊訊息索引以 DEK 加密；本文逐則使用獨立資料金鑰。
use super::{
    crypto::{self, Key, Secret},
    valid_device,
    vault::{self, Account, Vault},
};
use crate::AppResult;
use chrono::{DateTime, Days, Months, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::ErrorKind,
    path::PathBuf,
};

const INDEX: &str = "messages.gcm";
const MAX_INDEX: u64 = 128 * 1024 * 1024;
pub(crate) const MAX_IMAGE: usize = 10_000_000;
const RECEIPTS: &str = "receipts.dpapi";
const NOTICE_GUARD: &str = "chat-notices.dpapi";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Kind {
    Text,
    Image,
    ExpiredText,
    ExpiredImage,
    Removed,
    ImageRemoved,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum State {
    Pending,
    Rejected,
    Sent,
    Delivered,
    Expired,
    Received,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct Receipt {
    pub(crate) client_message_id: String,
    pub(crate) state: State,
}

impl Account {
    /// status 回傳後伺服器即可清掉回條。上鎖時先以 DPAPI 暫存去識別化的 UUID 與狀態，
    /// 不保存正文、好友或時間。解鎖後再合併到 DEK 加密索引。
    pub(crate) fn stage_receipts(&self, receipts: &[Receipt]) -> AppResult<()> {
        if receipts.is_empty() {
            return Ok(());
        }
        let mut staged = BTreeMap::new();
        for receipt in self
            .read_receipts()?
            .into_iter()
            .chain(receipts.iter().cloned())
        {
            staged
                .entry(receipt.client_message_id.clone())
                .or_insert(receipt);
        }
        let bytes = serde_json::to_vec(&staged.into_values().collect::<Vec<_>>())
            .map_err(|_| "無法編碼私聊回條。")?;
        vault::durable_write(
            &self.directory.join(RECEIPTS),
            &crate::storage::protect(&bytes, true)?,
        )
    }

    pub(crate) fn read_receipts(&self) -> AppResult<Vec<Receipt>> {
        let path = self.directory.join(RECEIPTS);
        if !path.is_file() {
            return Ok(Vec::new());
        }
        let protected = vault::read_limited(&path, 32 * 1024 * 1024)?;
        let raw = Secret(crate::storage::protect(&protected, false)?);
        serde_json::from_slice(&raw.0).map_err(|_| "私聊回條格式不正確。".into())
    }

    pub(crate) fn clear_receipts(&self) -> AppResult<()> {
        match fs::remove_file(self.directory.join(RECEIPTS)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("無法清除已保存的私聊回條：{error}")),
        }
    }

    /// 鎖定 status 沒有來源身分。有黑名單（或舊索引尚未核對）時，留待解鎖再分類。
    pub(crate) fn locked_notices_safe(&self) -> bool {
        let path = self.directory.join(NOTICE_GUARD);
        if !path.is_file() {
            return !self.directory.join("contacts.gcm").exists();
        }
        vault::read_limited(&path, 4096)
            .and_then(|raw| crate::storage::protect(&raw, false))
            .is_ok_and(|flag| flag == b"empty")
    }

    pub(crate) fn save_notice_guard(&self, has_blacklist: bool) -> AppResult<()> {
        let flag = if has_blacklist {
            b"blocked".as_slice()
        } else {
            b"empty".as_slice()
        };
        vault::durable_write(
            &self.directory.join(NOTICE_GUARD),
            &crate::storage::protect(flag, true)?,
        )
    }
}

pub(crate) enum Payload {
    Text(String),
    Image(Vec<u8>),
}

impl Payload {
    fn bytes(&self) -> &[u8] {
        match self {
            Self::Text(text) => text.as_bytes(),
            Self::Image(bytes) => bytes,
        }
    }

    pub(crate) fn kind(&self) -> Kind {
        match self {
            Self::Text(_) => Kind::Text,
            Self::Image(_) => Kind::Image,
        }
    }
}

impl Drop for Payload {
    fn drop(&mut self) {
        match self {
            Self::Text(text) => super::clear_text(text),
            Self::Image(bytes) => {
                for byte in bytes {
                    // SAFETY: 此緩衝區專供私聊影像，離開時覆寫明文。
                    unsafe { std::ptr::write_volatile(byte, 0) };
                }
            }
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
struct EntryKey([u8; 32]);

impl Drop for EntryKey {
    fn drop(&mut self) {
        for byte in &mut self.0 {
            // SAFETY: 本物件持有記錄金鑰陣列。
            unsafe { std::ptr::write_volatile(byte, 0) };
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct Record {
    pub(crate) id: String,
    pub(crate) peer: String,
    pub(crate) time: Option<String>,
    pub(crate) kind: Kind,
    pub(crate) state: State,
    pub(crate) reason: Option<String>,
    pub(crate) outgoing: bool,
    key: Option<EntryKey>,
    stored_bytes: u64,
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub(crate) struct History {
    records: Vec<Record>,
    /// 只留去重雜湊，批量刪除後也不會因重複取件而復原紀錄。
    received: BTreeSet<String>,
}

impl History {
    pub(crate) fn load(vault: &Vault) -> AppResult<Self> {
        vault.read(INDEX, MAX_INDEX)
    }

    fn save(&self, vault: &Vault) -> AppResult<()> {
        vault.write(INDEX, self, MAX_INDEX)
    }

    pub(crate) fn for_peer(&self, peer: &str) -> Vec<&Record> {
        let mut messages: Vec<_> = self
            .records
            .iter()
            .filter(|entry| entry.peer == peer)
            .collect();
        messages.sort_by(|left, right| match (&left.time, &right.time) {
            (Some(left), Some(right)) => left.cmp(right),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });
        messages
    }

    pub(crate) fn record(&self, id: &str) -> Option<&Record> {
        self.records.iter().find(|record| record.id == id)
    }

    pub(crate) fn occupied(&self, vault: &Vault) -> u64 {
        let payloads: u64 = self.records.iter().map(|record| record.stored_bytes).sum();
        let index = fs::metadata(vault.account.directory.join(INDEX)).map_or(0, |meta| meta.len());
        payloads.saturating_add(index)
    }

    pub(crate) fn append_outgoing(
        &mut self,
        vault: &Vault,
        peer: &str,
        body: &Payload,
    ) -> AppResult<String> {
        crate::jobs::validate_id(peer)?;
        let id = super::uuid()?;
        let mut entry = Record {
            id: id.clone(),
            peer: peer.into(),
            time: None,
            kind: body.kind(),
            state: State::Pending,
            reason: None,
            outgoing: true,
            key: None,
            stored_bytes: 0,
        };
        entry.store(vault, body)?;
        let mut next = self.clone();
        next.records.push(entry);
        if let Err(error) = next.save(vault) {
            let _ = fs::remove_file(payload_path(vault, &id)?);
            return Err(error);
        }
        *self = next;
        Ok(id)
    }

    /// 回傳 false 表示伺服器 ID 已保存，重抓時只需 ack，無須重複顯示。
    pub(crate) fn receive(&mut self, vault: &Vault, envelope: Incoming<'_>) -> AppResult<bool> {
        crate::jobs::validate_id(envelope.server_id)?;
        let fingerprint = format!("{:x}", Sha256::digest(envelope.server_id.as_bytes()));
        if self.received.contains(&fingerprint) {
            return Ok(false);
        }
        let mut next = self.clone();
        next.received.insert(fingerprint);
        let mut pending_payload = None;
        if !envelope.blocked {
            crate::jobs::validate_id(envelope.peer)?;
            let mut entry = Record {
                id: super::uuid()?,
                peer: envelope.peer.into(),
                time: Some(server_time(envelope.time)?),
                kind: envelope.kind,
                state: State::Received,
                reason: None,
                outgoing: false,
                key: None,
                stored_bytes: 0,
            };
            if let Some(body) = envelope.body {
                entry.store(vault, body)?;
                pending_payload = Some(entry.id.clone());
            }
            next.records.push(entry);
        }
        // 不論是否封鎖，去重資料與索引都確定寫入後，上層才能 ack。
        if let Err(error) = next.save(vault) {
            if let Some(id) = pending_payload {
                let _ = fs::remove_file(payload_path(vault, &id)?);
            }
            return Err(error);
        }
        *self = next;
        Ok(!envelope.blocked)
    }

    pub(crate) fn mark_sent(&mut self, vault: &Vault, id: &str, time: &str) -> AppResult<()> {
        let time = server_time(time)?;
        self.update_outgoing(vault, id, |entry| {
            if matches!(entry.state, State::Pending | State::Rejected) {
                entry.state = State::Sent;
                entry.reason = None;
            }
            entry.time = Some(time);
        })
    }

    pub(crate) fn reject(&mut self, vault: &Vault, id: &str, reason: String) -> AppResult<()> {
        self.update_outgoing(vault, id, |entry| {
            if matches!(entry.state, State::Pending | State::Rejected) {
                entry.state = State::Rejected;
                entry.reason = Some(reason);
            }
        })
    }

    fn update_outgoing(
        &mut self,
        vault: &Vault,
        id: &str,
        update: impl FnOnce(&mut Record),
    ) -> AppResult<()> {
        let mut next = self.clone();
        let entry = next
            .records
            .iter_mut()
            .find(|entry| entry.id == id && entry.outgoing)
            .ok_or("找不到這則送出的私聊訊息。")?;
        update(entry);
        next.save(vault)?;
        *self = next;
        Ok(())
    }

    pub(crate) fn apply_receipts(&mut self, vault: &Vault, receipts: &[Receipt]) -> AppResult<()> {
        let mut next = self.clone();
        let mut changed = false;
        for receipt in receipts {
            if let Some(record) = next
                .records
                .iter_mut()
                .find(|entry| entry.id == receipt.client_message_id && entry.outgoing)
            {
                if matches!(receipt.state, State::Delivered | State::Expired)
                    && !matches!(record.state, State::Delivered | State::Expired)
                {
                    record.state = receipt.state;
                    record.reason = None;
                    changed = true;
                }
            }
        }
        if changed {
            next.save(vault)?;
            *self = next;
        }
        Ok(())
    }

    /// 刪除與清理都先重寫索引、移除單則資料金鑰，再嘗試清理不再可讀的密文。
    pub(crate) fn remove(&mut self, vault: &Vault, id: &str) -> AppResult<()> {
        let mut next = self.clone();
        let entry = next
            .records
            .iter_mut()
            .find(|entry| entry.id == id)
            .ok_or("找不到要移除的私聊訊息。")?;
        entry.purge(Kind::Removed);
        self.commit_purge(vault, next, &[id.to_string()])
    }

    pub(crate) fn cleanup(
        &mut self,
        vault: &Vault,
        cutoff: Option<DateTime<Utc>>,
        only_images: bool,
    ) -> AppResult<usize> {
        let mut next = self.clone();
        let mut ids = Vec::new();
        next.records.retain_mut(|entry| {
            let in_range = cutoff.is_none_or(|cutoff| {
                entry
                    .time
                    .as_deref()
                    .and_then(|time| DateTime::parse_from_rfc3339(time).ok())
                    .is_some_and(|time| time < cutoff)
            });
            if !in_range || (only_images && entry.kind != Kind::Image) {
                return true;
            }
            ids.push(entry.id.clone());
            if only_images {
                entry.purge(Kind::ImageRemoved);
            }
            only_images
        });
        if !ids.is_empty() {
            self.commit_purge(vault, next, &ids)?;
        }
        Ok(ids.len())
    }

    fn commit_purge(&mut self, vault: &Vault, next: Self, ids: &[String]) -> AppResult<()> {
        next.save(vault)?;
        *self = next;
        for id in ids {
            match fs::remove_file(payload_path(vault, id)?) {
                Ok(()) => (),
                Err(error) if error.kind() == ErrorKind::NotFound => (),
                Err(error) => return Err(format!("內容已清除，但無法移除部分加密檔案：{error}")),
            }
        }
        Ok(())
    }
}

impl Record {
    fn store(&mut self, vault: &Vault, body: &Payload) -> AppResult<()> {
        let key: Key = Secret(crypto::random()?);
        let bytes = crypto::seal(&key, &vault.account.aad(self.id.as_bytes()), body.bytes())?;
        vault::durable_write(&payload_path(vault, &self.id)?, &bytes)?;
        self.key = Some(EntryKey(key.0));
        self.stored_bytes = bytes.len() as u64;
        Ok(())
    }

    pub(crate) fn payload(&self, vault: &Vault) -> AppResult<Payload> {
        let key = self.key.as_ref().ok_or("此訊息的內容已移除。")?;
        let bytes = vault::read_limited(&payload_path(vault, &self.id)?, MAX_IMAGE as u64 + 28)?;
        let mut plain = crypto::open(
            &Secret(key.0),
            &vault.account.aad(self.id.as_bytes()),
            &bytes,
        )?;
        match self.kind {
            Kind::Text => {
                let text = std::str::from_utf8(&plain.0).map_err(|_| "私聊文字格式不正確。")?;
                Ok(Payload::Text(text.into()))
            }
            Kind::Image => Ok(Payload::Image(std::mem::take(&mut plain.0))),
            _ => Err("此私聊訊息沒有本文。".into()),
        }
    }

    fn purge(&mut self, kind: Kind) {
        self.key = None;
        self.kind = kind;
        self.stored_bytes = 0;
        self.reason = None;
    }
}

pub(crate) struct Incoming<'a> {
    pub(crate) server_id: &'a str,
    pub(crate) peer: &'a str,
    pub(crate) time: &'a str,
    pub(crate) kind: Kind,
    pub(crate) body: Option<&'a Payload>,
    pub(crate) blocked: bool,
}

fn payload_path(vault: &Vault, id: &str) -> AppResult<PathBuf> {
    if !valid_device(id) {
        return Err("本機私聊訊息識別格式不正確。".into());
    }
    Ok(vault
        .account
        .directory
        .join("payloads")
        .join(format!("{id}.gcm")))
}

pub(crate) fn server_time(value: &str) -> AppResult<String> {
    DateTime::parse_from_rfc3339(value)
        .map(|time| {
            time.with_timezone(&Utc)
                .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
        })
        .map_err(|_| "私聊伺服器時間格式不正確。".into())
}

pub(crate) fn text_payload(text: String) -> AppResult<Payload> {
    if text.trim().is_empty() || text.chars().count() > 4_000 {
        return Err("私聊文字必須為 1 到 4,000 個字元。".into());
    }
    Ok(Payload::Text(text))
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Period {
    All,
    Year,
    HalfYear,
    Quarter,
    Month,
    BeforeYesterday,
}

impl Period {
    pub(crate) fn cutoff(self, now: u64) -> AppResult<Option<DateTime<Utc>>> {
        if matches!(self, Self::All) {
            return Ok(None);
        }
        let now = DateTime::from_timestamp(now as i64, 0).ok_or("無法取得清理期限。")?;
        let value = match self {
            Self::All => unreachable!(),
            Self::Year => now.checked_sub_months(Months::new(12)),
            Self::HalfYear => now.checked_sub_months(Months::new(6)),
            Self::Quarter => now.checked_sub_months(Months::new(3)),
            Self::Month => now.checked_sub_months(Months::new(1)),
            Self::BeforeYesterday => now
                .date_naive()
                .checked_sub_days(Days::new(1))
                .and_then(|day| day.and_hms_opt(0, 0, 0))
                .map(|time| time.and_utc()),
        };
        value.map(Some).ok_or_else(|| "清理期限超出範圍。".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::private_chat::vault::ServerSecret;

    #[test]
    fn delete_and_bulk_cleanup_drop_record_keys_and_preserve_dedup() {
        let root =
            std::env::temp_dir().join(format!("lmai-message-{}", crate::jobs::new_id().unwrap()));
        let account = Account::new(&root, "test-only", "cleanup").unwrap();
        let vault = account
            .create(
                "Abcd12",
                &ServerSecret {
                    value: Secret([5; 32]),
                    version: 1,
                },
            )
            .unwrap();
        let mut history = History::load(&vault).unwrap();
        let peer = "peer-one";
        let image = Payload::Image(b"test image private bytes".to_vec());
        let text = text_payload("test private text".into()).unwrap();
        let cases = [
            ("one", "2025-01-01T12:00:00Z", &image),
            ("two", "2025-01-02T12:00:00Z", &text),
            ("three", "2026-10-31T12:00:00Z", &image),
        ];
        for (id, time, body) in cases {
            assert!(history
                .receive(
                    &vault,
                    Incoming {
                        server_id: id,
                        peer,
                        time,
                        kind: body.kind(),
                        body: Some(body),
                        blocked: false,
                    }
                )
                .unwrap());
        }
        let first = history.for_peer(peer)[0].id.clone();
        let captured = fs::read(payload_path(&vault, &first).unwrap()).unwrap();
        let occupancy = history.occupied(&vault);
        let now = DateTime::parse_from_rfc3339("2026-11-07T02:00:00Z")
            .unwrap()
            .timestamp() as u64;
        assert_eq!(
            history
                .cleanup(&vault, Period::Year.cutoff(now).unwrap(), true)
                .unwrap(),
            1
        );
        assert_eq!(history.record(&first).unwrap().kind, Kind::ImageRemoved);
        assert!(history.occupied(&vault) < occupancy);
        // 即使把舊密文放回，最新加密索引已無資料金鑰。
        fs::write(payload_path(&vault, &first).unwrap(), captured).unwrap();
        let mut reloaded = History::load(&vault).unwrap();
        assert!(reloaded.record(&first).unwrap().payload(&vault).is_err());
        assert_eq!(
            reloaded
                .cleanup(&vault, Period::Year.cutoff(now).unwrap(), false)
                .unwrap(),
            2
        );
        assert_eq!(reloaded.for_peer(peer).len(), 1);
        assert!(!reloaded
            .receive(
                &vault,
                Incoming {
                    server_id: "two",
                    peer,
                    time: cases[1].1,
                    kind: Kind::Text,
                    body: Some(&text),
                    blocked: false,
                }
            )
            .unwrap());
        assert_eq!(reloaded.for_peer(peer).len(), 1);
        assert!(account.erase().is_ok());
        assert!(!account.directory.join(INDEX).exists());
    }

    #[test]
    fn unicode_length_and_calendar_periods_are_bounded() {
        assert!(text_payload("🖼".repeat(4000)).is_ok());
        assert!(text_payload("🖼".repeat(4001)).is_err());
        assert!(text_payload("   ".into()).is_err());
        let now = DateTime::parse_from_rfc3339("2026-11-07T13:00:00Z")
            .unwrap()
            .timestamp() as u64;
        for (period, expected) in [
            (Period::Year, "2025-11-07"),
            (Period::HalfYear, "2026-05-07"),
            (Period::Quarter, "2026-08-07"),
            (Period::Month, "2026-10-07"),
            (Period::BeforeYesterday, "2026-11-06"),
        ] {
            assert_eq!(
                period
                    .cutoff(now)
                    .unwrap()
                    .unwrap()
                    .date_naive()
                    .to_string(),
                expected
            );
        }
    }
}
