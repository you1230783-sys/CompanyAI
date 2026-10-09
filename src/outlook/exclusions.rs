//! 未勾選資料夾的跨資料夾排除索引。只在本機讀取郵件識別屬性，不讀 Body／附件。
//! 每次查詢重新建立，避免舊快取漏掉剛移入隱藏資料夾的副本；掃描不完整時拒絕放行。
use super::*;
use std::{
    collections::BTreeSet,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

/// 全欄位指紋走集合查詢；缺欄位項目只在本機保留部分指紋。
#[derive(Default)]
pub(super) struct Exclusions {
    complete: BTreeSet<String>,
    identities: Vec<Identity>,
    partial: Vec<Identity>,
    issues: BTreeSet<String>,
}

#[derive(Clone)]
struct Identity {
    // 固定順序：主旨、寄送時間、寄件地址、To/CC 地址集合。不存在不等於空字串。
    fields: [Option<String>; 4],
}
impl Identity {
    fn fingerprint(&self) -> Option<String> {
        self.fields.iter().all(Option::is_some).then(|| {
            crate::projects::text::revision(
                &serde_json::to_string(&self.fields).unwrap_or_default(),
            )
        })
    }
    /// 只有共同存在且不同的欄位，才足以證明與隱藏郵件無關。
    fn conflicts(&self, other: &Self) -> bool {
        self.fields
            .iter()
            .zip(&other.fields)
            .any(|(a, b)| matches!((a, b), (Some(a), Some(b)) if a != b))
    }
}

/// 不讀 Message-ID、Body 或附件，也不為取得 SMTP 額外向地址簿查詢。
/// 同型別的 Exchange 原始地址可以直接比較，不能因沒有 @ 而視為失敗。
fn identities(item: &IDispatch) -> (Identity, Vec<String>) {
    let recipients = || -> AppResult<String> {
        let collection = object(&get(item, "Recipients", &mut [])?)?;
        let total = i32::try_from(&get(&collection, "Count", &mut [])?)
            .map_err(|_| "收件者數量無法讀取。")?;
        if !(0..=200).contains(&total) {
            return Err("收件者數量超出200個上限。".into());
        }
        let mut addresses = BTreeSet::new();
        for index in 1..=total {
            let recipient = object(&get(&collection, "Item", &mut [index.into()])?)?;
            let kind = i32::try_from(&get(&recipient, "Type", &mut [])?)
                .map_err(|_| "收件者類型無法讀取。")?;
            // BCC 不在收件端完整呈現；共同指紋只包含 To/CC。
            if kind == 3 {
                continue;
            }
            if !matches!(kind, 1 | 2) {
                return Err("未知收件者類型。".into());
            }
            let address = text(&recipient, "Address", 4096)?
                .trim()
                .to_ascii_lowercase();
            if address.is_empty() {
                return Err("收件地址為空。".into());
            }
            addresses.insert(format!("{kind}:{address}"));
        }
        serde_json::to_string(&addresses).map_err(|e| e.to_string())
    };
    let sender = || -> AppResult<String> {
        let address = text(item, "SenderEmailAddress", 4096)?
            .trim()
            .to_ascii_lowercase();
        if address.is_empty() {
            return Err("寄件地址為空。".into());
        }
        Ok(address)
    };
    let values = [
        text(item, "Subject", 3000),
        project::timestamp(item, "SentOn"),
        sender(),
        recipients(),
    ];
    let mut fields = [None, None, None, None];
    let mut issues = Vec::new();
    for (index, value) in values.into_iter().enumerate() {
        match value {
            Ok(value) => fields[index] = Some(crate::projects::text::revision(&value)),
            Err(error) => issues.push(format!(
                "{}：{error}",
                ["主旨", "寄送時間", "寄件地址", "收件地址"][index]
            )),
        }
    }
    (Identity { fields }, issues)
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
                    let (identity, issues) = identities(&item);
                    if identity.fields.iter().all(Option::is_none) {
                        return Err(format!("隱藏郵件的四個識別欄位皆無法讀取，無法界定排除範圍；查詢失敗，郵件數量未知。{}", issues.join("；")));
                    }
                    result.issues.extend(issues);
                    result.insert(identity);
                }
            }
            if count(&items)? != total {
                return Err("比對期間隱藏資料夾已改變，請重新查詢。".into());
            }
        }
        Ok(result)
    }

    fn insert(&mut self, identity: Identity) {
        if let Some(key) = identity.fingerprint() {
            self.complete.insert(key);
        } else {
            self.partial.push(identity.clone());
        }
        self.identities.push(identity);
    }

    fn matches(&self, identity: &Identity) -> AppResult<bool> {
        if let Some(key) = identity.fingerprint() {
            if self.complete.contains(&key) {
                return Ok(true);
            }
            if self.partial.iter().all(|hidden| hidden.conflicts(identity)) {
                return Ok(false);
            }
        } else if self
            .identities
            .iter()
            .all(|hidden| hidden.conflicts(identity))
        {
            return Ok(false);
        }
        Err("此候選郵件與隱藏郵件有識別欄位不足且無法排除關聯，已暫時排除此候選；其他可確認無關的郵件仍可讀取。".into())
    }

    pub fn notices(&self) -> Vec<String> {
        if self.partial.is_empty() {
            return vec![];
        }
        vec![format!("隱藏副本索引含{}封欄位不足的郵件；以可用欄位排除無關候選，無法判定的候選不開放。欄位診斷：{}", self.partial.len(), self.issues.iter().take(6).cloned().collect::<Vec<_>>().join("；"))]
    }
    pub fn contains(&self, item: &IDispatch) -> AppResult<bool> {
        if self.identities.is_empty() {
            return Ok(false);
        }
        let (identity, issues) = identities(item);
        self.matches(&identity).map_err(|error| {
            if issues.is_empty() {
                error
            } else {
                format!("{error} 候選欄位診斷：{}", issues.join("；"))
            }
        })
    }
    pub fn require(&self, item: &IDispatch) -> AppResult<()> {
        if self.contains(item)? {
            return Err("此郵件的主旨、寄送時間、寄件地址與To/CC指紋符合隱藏郵件，已一併隱藏，未讀取或匯出。".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_and_partial_fingerprints_do_not_block_unrelated_mail() {
        let mail = |subject: &str| Identity {
            fields: [
                Some(subject.into()),
                Some("time".into()),
                Some("/o=exchange".into()),
                Some("recipients".into()),
            ],
        };
        let mut index = Exclusions::default();
        index.insert(mail("hidden"));
        assert_eq!(index.matches(&mail("hidden")), Ok(true));
        assert_eq!(index.matches(&mail("other")), Ok(false));
        let mut partial = mail("partial");
        partial.fields[2] = None;
        index.insert(partial.clone());
        assert!(index.matches(&mail("partial")).is_err());
        assert_eq!(index.matches(&mail("other")), Ok(false));
        assert!(index.matches(&partial).is_err());
        partial.fields[0] = Some("unrelated".into());
        assert_eq!(index.matches(&partial), Ok(false));
    }
}
