//! 隱藏資料夾副本排除：只比較完整主旨及寄送時間（秒），不讀地址簿或正文。
use super::*;
use chrono::NaiveDate;
use std::{
    cell::RefCell,
    collections::BTreeSet,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

#[derive(Default, Clone)]
pub(super) struct Exclusions {
    keys: BTreeSet<String>,
}

// 僅同一輔助程序記憶體內重用。每次先批次核對日期範圍內的實際列，
// 指紋含政策、資料夾、EntryID、版本和識別值；不以Count或TTL假裝內容未變。
thread_local! { static CACHE: RefCell<Option<(String, Exclusions)>> = const { RefCell::new(None) }; }

impl Exclusions {
    #[cfg(test)]
    pub fn from_app(
        app: &IDispatch,
        policy: &privacy::Policy,
        cancel: &AtomicBool,
    ) -> AppResult<Self> {
        let ns = object(&get(app, "GetNamespace", &mut ["MAPI".into()])?)?;
        Self::from_namespace(&ns, policy, cancel)
    }
    #[cfg(test)]
    pub fn from_namespace(
        ns: &IDispatch,
        policy: &privacy::Policy,
        cancel: &AtomicBool,
    ) -> AppResult<Self> {
        Self::from_range(ns, policy, None, cancel)
    }
    pub fn from_range(
        ns: &IDispatch,
        policy: &privacy::Policy,
        range: Option<(NaiveDate, NaiveDate)>,
        cancel: &AtomicBool,
    ) -> AppResult<Self> {
        batch::check_cancel(cancel)?;
        if !policy.configured {
            return Ok(Self::default());
        }
        let started = Instant::now();
        let count = |items: &IDispatch| -> AppResult<i32> {
            i32::try_from(&get(items, "Count", &mut [])?)
                .map_err(|_| "Outlook 隱藏副本索引無法確認集合數量。".into())
        };
        let stores = object(&get(ns, "Stores", &mut [])?)?;
        let total = count(&stores)?;
        if !(0..=100).contains(&total) {
            return Err("Outlook 隱藏副本索引資料檔過多。".into());
        }
        let mut stack = Vec::new();
        for i in 1..=total {
            let store = object(&get(&stores, "Item", &mut [i.into()])?)?;
            stack.push((object(&get(&store, "GetRootFolder", &mut [])?)?, false, 0));
        }
        let mut folders = BTreeSet::new();
        let mut records = Vec::new();
        let mut result = Self::default();
        while let Some((folder, parent_hidden, depth)) = stack.pop() {
            batch::check_cancel(cancel)?;
            if started.elapsed() >= Duration::from_secs(600) || folders.len() >= 5000 || depth >= 64
            {
                return Err(
                    "Outlook 隱藏副本索引未完成（10分鐘或資料夾界線），未開放此批資料。".into(),
                );
            }
            let store = text(&folder, "StoreID", 4096)?;
            let entry = text(&folder, "EntryID", 4096)?;
            let identity = privacy::key(&store, &entry);
            if !folders.insert(identity.clone()) {
                return Err("Outlook 隱藏副本索引資料夾結構重複。".into());
            }
            let hidden = parent_hidden || !policy.permits(&store, &entry);
            records.push(format!("{identity}:{hidden}"));
            let children = object(&get(&folder, "Folders", &mut [])?)?;
            let children_count = count(&children)?;
            if children_count < 0 || children_count as usize + folders.len() + stack.len() > 5000 {
                return Err("Outlook 隱藏副本索引超過資料夾界線。".into());
            }
            for i in 1..=children_count {
                stack.push((
                    object(&get(&children, "Item", &mut [i.into()])?)?,
                    hidden,
                    depth + 1,
                ));
            }
            if !hidden {
                continue;
            }
            if i32::try_from(&get(&folder, "DefaultItemType", &mut [])?)
                .map_err(|_| "Outlook 隱藏副本索引無法確認資料夾類型。")?
                != 0
            {
                continue;
            }
            super::process::notify(&format!(
                "正在比對Outlook隱藏副本… 已檢查{}個資料夾",
                folders.len()
            ));
            let rows = table::scan(ns, &folder, "SentOn", range, false, cancel)
                .map_err(|e| format!("Outlook 隱藏副本索引未完成，未開放此批資料。{e}"))?;
            for row in rows {
                let key = row.key();
                records.push(format!("{identity}:{}:{}:{key}", row.entry, row.modified));
                result.keys.insert(key);
                if records.len() > 200_000 {
                    return Err("Outlook 隱藏副本索引資料量超出界線；請縮小日期範圍。".into());
                }
            }
        }
        records.sort();
        let stamp = crate::projects::text::revision(
            &serde_json::json!([
                policy.revision(),
                range.map(|(s, e)| (s.to_string(), e.to_string())),
                records
            ])
            .to_string(),
        );
        CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            if let Some((old, value)) = cache.as_ref().filter(|(old, _)| old == &stamp) {
                let _ = old;
                return Ok(value.clone());
            }
            *cache = Some((stamp, result.clone()));
            Ok(result)
        })
    }
    pub fn contains_key(&self, key: &str) -> bool {
        self.keys.contains(key)
    }
    pub fn contains(&self, item: &IDispatch) -> AppResult<bool> {
        if self.keys.is_empty() {
            return Ok(false);
        }
        let subject = text(item, "Subject", 32768)?;
        let sent = project::variant_time(&get(item, "SentOn", &mut [])?)?;
        Ok(self.contains_key(&table::key(&subject, sent)))
    }
    pub fn require(&self, item: &IDispatch) -> AppResult<()> {
        if self.contains(item)? {
            return Err("此郵件的完整主旨與寄送時間（秒）符合隱藏郵件，未開放內文。".into());
        }
        Ok(())
    }
}

/// 舊版「目前選取／最近幾日」介面沒有專案日期參數，依候選的寄送日
/// 延後建立索引，同一次操作同日只掃一次，仍不必遍歷整個隱藏歷史信箱。
pub(super) struct Deferred<'a> {
    namespace: IDispatch,
    policy: privacy::Policy,
    cancel: &'a AtomicBool,
    days: RefCell<std::collections::BTreeMap<NaiveDate, Exclusions>>,
}
impl<'a> Deferred<'a> {
    pub fn from_app(
        app: &IDispatch,
        policy: &privacy::Policy,
        cancel: &'a AtomicBool,
    ) -> AppResult<Self> {
        Self::from_namespace(
            &object(&get(app, "GetNamespace", &mut ["MAPI".into()])?)?,
            policy,
            cancel,
        )
    }
    pub fn from_namespace(
        namespace: &IDispatch,
        policy: &privacy::Policy,
        cancel: &'a AtomicBool,
    ) -> AppResult<Self> {
        batch::check_cancel(cancel)?;
        Ok(Self {
            namespace: namespace.clone(),
            policy: policy.clone(),
            cancel,
            days: RefCell::new(Default::default()),
        })
    }
    pub fn notices(&self) -> Vec<String> {
        vec![]
    }
    pub fn contains(&self, item: &IDispatch) -> AppResult<bool> {
        batch::check_cancel(self.cancel)?;
        if !self.policy.configured {
            return Ok(false);
        }
        let day = project::variant_time(&get(item, "SentOn", &mut [])?)?.date();
        if !self.days.borrow().contains_key(&day) {
            if self.days.borrow().len() >= 128 {
                return Err("Outlook 隱藏副本索引的候選寄送日期過於分散；請縮小範圍。".into());
            }
            let index = Exclusions::from_range(
                &self.namespace,
                &self.policy,
                Some((day, day)),
                self.cancel,
            )?;
            self.days.borrow_mut().insert(day, index);
        }
        self.days
            .borrow()
            .get(&day)
            .ok_or("Outlook 隱藏副本索引不存在。")?
            .contains(item)
    }
    pub fn require(&self, item: &IDispatch) -> AppResult<()> {
        if self.contains(item)? {
            return Err("此郵件的完整主旨與寄送時間（秒）符合隱藏郵件，未開放。".into());
        }
        Ok(())
    }
}
