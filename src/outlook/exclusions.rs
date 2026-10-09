//! 未勾選資料夾的跨資料夾排除索引。只在本機讀取郵件識別屬性，不讀 Body／附件。
//! 每次查詢重新建立，避免舊快取漏掉剛移入隱藏資料夾的副本；掃描不完整時拒絕放行。
use super::*;
use std::{
    collections::BTreeSet,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

#[derive(Default)]
pub(super) struct Exclusions {
    message_ids: BTreeSet<String>,
    envelopes: BTreeSet<String>,
    missing_id_envelopes: BTreeSet<String>,
}

/// Internet Message-ID 可跨 Store／EntryID 識別副本。額外保存完整信封指紋，
/// 支援匯入後遺失 Message-ID 的郵件；不使用主旨單獨判定，也不作模糊比對。
struct Identity {
    message_id: Option<String>,
    envelope: Option<String>,
}
fn identities(item: &IDispatch) -> AppResult<Identity> {
    let mut message_id = None;
    for tag in ["0x1035001F", "0x1035001E"] {
        if let Ok(id) = project::property(
            item,
            &format!("http://schemas.microsoft.com/mapi/proptag/{tag}"),
        ) {
            if !id.trim().is_empty() {
                message_id = Some(crate::projects::text::revision(&format!(
                    "message-id:{}",
                    id.trim()
                )));
                break;
            }
        }
    }
    let envelope = (|| -> AppResult<String> {
        let sent = project::timestamp(item, "SentOn")?;
        let sender = project::sender(item)?;
        let recipients = project::recipients(item)?;
        let key = crate::projects::mail::duplicate_key(&sent, &sender, &recipients)
            .ok_or("郵件缺少可比對的完整寄收件地址。")?;
        Ok(crate::projects::text::revision(&format!(
            "envelope:{key}:{}",
            text(item, "Subject", 3000)?
        )))
    })();
    let envelope = envelope.ok();
    if message_id.is_none() && envelope.is_none() {
        return Err("無法核對隱藏資料夾的郵件副本；未開放此批 Outlook 資料。請確認 Outlook 已完整載入郵件。".into());
    }
    Ok(Identity {
        message_id,
        envelope,
    })
}

impl Exclusions {
    pub fn from_app(
        app: &IDispatch,
        policy: &privacy::Policy,
        cancel: &AtomicBool,
    ) -> AppResult<Self> {
        if !policy.configured {
            return Ok(Self::default());
        }
        let ns = object(&get(app, "GetNamespace", &mut ["MAPI".into()])?)?;
        Self::from_namespace(&ns, policy, cancel)
    }

    pub fn from_namespace(
        ns: &IDispatch,
        policy: &privacy::Policy,
        cancel: &AtomicBool,
    ) -> AppResult<Self> {
        if !policy.configured {
            return Ok(Self::default());
        }
        let started = Instant::now();
        let count = |items: &IDispatch| -> AppResult<i32> {
            i32::try_from(&get(items, "Count", &mut [])?)
                .map_err(|_| "無法核對 Outlook 隱藏範圍。".into())
        };
        let stores = object(&get(ns, "Stores", &mut [])?)?;
        let total = count(&stores)?;
        if !(0..=100).contains(&total) {
            return Err("Outlook 資料檔數超過可核對範圍。".into());
        }
        let mut stack = Vec::new();
        for index in 1..=total {
            let store = object(&get(&stores, "Item", &mut [index.into()])?)?;
            stack.push((object(&get(&store, "GetRootFolder", &mut [])?)?, false, 0));
        }
        let mut result = Self::default();
        let mut folders = BTreeSet::new();
        let mut scanned = 0usize;
        while let Some((folder, parent_hidden, depth)) = stack.pop() {
            batch::check_cancel(cancel)?;
            if started.elapsed() > Duration::from_secs(120) || folders.len() >= 5000 || depth >= 64
            {
                return Err("Outlook 隱藏副本比對未完成（120秒／5000資料夾上限），未開放此批資料。請減少 Outlook 開啟的資料檔後再試。".into());
            }
            let store = text(&folder, "StoreID", 4096)?;
            let entry = text(&folder, "EntryID", 4096)?;
            if !folders.insert(privacy::key(&store, &entry)) {
                return Err("Outlook 資料夾結構重複，無法完整比對隱藏副本。".into());
            }
            let hidden = parent_hidden || !policy.permits(&store, &entry);
            let children = object(&get(&folder, "Folders", &mut [])?)?;
            let child_count = count(&children)?;
            if child_count < 0 || child_count as usize + stack.len() + folders.len() > 5000 {
                return Err("Outlook 隱藏副本比對超過資料夾範圍。".into());
            }
            for index in 1..=child_count {
                stack.push((
                    object(&get(&children, "Item", &mut [index.into()])?)?,
                    hidden,
                    depth + 1,
                ));
            }
            if !hidden
                || i32::try_from(&get(&folder, "DefaultItemType", &mut [])?)
                    .map_err(|_| "無法核對隱藏資料夾類型。")?
                    != 0
            {
                continue;
            }
            let items = object(&get(&folder, "Items", &mut [])?)?;
            let total = count(&items)?;
            if total < 0 || scanned + total as usize > 100_000 {
                return Err(
                    "隱藏資料夾超過十萬個項目，無法完成副本排除；未開放此批 Outlook 資料。".into(),
                );
            }
            for index in 1..=total {
                batch::check_cancel(cancel)?;
                if started.elapsed() > Duration::from_secs(120) {
                    return Err("Outlook 隱藏副本比對逾時，未開放此批資料。".into());
                }
                scanned += 1;
                let item = object(&get(&items, "Item", &mut [index.into()])?)?;
                if i32::try_from(&get(&item, "Class", &mut [])?)
                    .map_err(|_| "無法核對隱藏項目類型。")?
                    == 43
                {
                    result.insert(identities(&item)?);
                }
            }
            if count(&items)? != total {
                return Err("比對期間隱藏資料夾已改變，請重新查詢。".into());
            }
        }
        Ok(result)
    }

    fn insert(&mut self, identity: Identity) {
        if let Some(envelope) = identity.envelope {
            if identity.message_id.is_none() {
                self.missing_id_envelopes.insert(envelope.clone());
            }
            self.envelopes.insert(envelope);
        }
        if let Some(id) = identity.message_id {
            self.message_ids.insert(id);
        }
    }
    fn matches(&self, identity: &Identity) -> bool {
        if let Some(id) = &identity.message_id {
            // 兩邊都有不同 Message-ID 時，不因相同主旨／寄收時間而誤判副本。
            self.message_ids.contains(id)
                || identity
                    .envelope
                    .as_ref()
                    .is_some_and(|key| self.missing_id_envelopes.contains(key))
        } else {
            identity
                .envelope
                .as_ref()
                .is_some_and(|key| self.envelopes.contains(key))
        }
    }
    pub fn contains(&self, item: &IDispatch) -> AppResult<bool> {
        if self.message_ids.is_empty() && self.envelopes.is_empty() {
            return Ok(false);
        }
        Ok(self.matches(&identities(item)?))
    }

    pub fn require(&self, item: &IDispatch) -> AppResult<()> {
        if self.contains(item)? {
            return Err("此郵件在隱藏資料夾中另有副本，已一併隱藏，未讀取或匯出。".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distinct_message_ids_win_over_same_envelope_and_missing_ids_have_fallback() {
        let identity = |id: Option<&str>| Identity {
            message_id: id.map(String::from),
            envelope: Some("same-envelope".into()),
        };
        let mut index = Exclusions::default();
        index.insert(identity(Some("original")));
        assert!(index.matches(&identity(Some("original"))));
        assert!(!index.matches(&identity(Some("different"))));
        assert!(index.matches(&identity(None)));
        index.insert(identity(None));
        assert!(index.matches(&identity(Some("lost-in-copy"))));
    }
}
