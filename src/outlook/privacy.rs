//! Outlook 資料夾授權只由本機選擇畫面建立；模型不能新增資料夾權限。
//! 規則使用 StoreID + EntryID 的雜湊，真實識別碼不交給 WebView 或 AI。
use super::*;
use crate::storage;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    path::Path,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

const FILE: &str = "outlook-folders.dpapi";

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Policy {
    pub configured: bool,
    pub allowed: BTreeSet<String>,
}

/// 此清單只送到本機選擇畫面，絕不作為模型工具結果。
#[derive(Clone, Serialize)]
pub struct Choice {
    pub id: String,
    pub parent: Option<String>,
    pub name: String,
    pub depth: usize,
    pub selected: bool,
}

pub fn key(store: &str, entry: &str) -> String {
    crate::projects::text::revision(&format!("{store}\n{entry}"))
}

impl Policy {
    #[cfg(not(test))]
    pub fn current() -> AppResult<Self> {
        Self::load(&storage::data_dir()?)
    }
    /// 單元測試不讀使用者正式設定；授權測試直接傳入 fixture Policy。
    #[cfg(test)]
    pub fn current() -> AppResult<Self> {
        Ok(Self::default())
    }

    pub fn load(root: &Path) -> AppResult<Self> {
        let path = root.join(FILE);
        match std::fs::metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(_) => return Err("無法讀取 Outlook 資料夾設定；未開放讀取。".into()),
            Ok(m) if m.len() > 2_000_000 => return Err("Outlook 資料夾設定過大。".into()),
            _ => (),
        }
        let bytes = std::fs::read(path).map_err(|_| "無法讀取 Outlook 資料夾設定。")?;
        serde_json::from_slice(&storage::protect(&bytes, false)?)
            .map_err(|_| "Outlook 資料夾設定損壞；未開放讀取。".into())
    }

    pub fn save(&self, root: &Path) -> AppResult<()> {
        let bytes = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        storage::atomic_write(&root.join(FILE), &storage::protect(&bytes, true)?)
    }

    pub fn revision(&self) -> String {
        // 排除規則改版時，舊快照必須重新核對，不能沿用舊四欄位可見清單。
        crate::projects::text::revision(&format!(
            "subject-senton-seconds-v2:{}:{:?}",
            self.configured, self.allowed
        ))
    }

    pub fn permits(&self, store: &str, entry: &str) -> bool {
        !self.configured || self.allowed.contains(&key(store, entry))
    }

    /// 只接受當次畫面列出的代號，且所有祖先均須勾選；偽造 UI 命令不能擴大範圍。
    pub fn from_selection(choices: &[Choice], selected: &[String]) -> AppResult<Self> {
        let allowed: BTreeSet<_> = selected.iter().cloned().collect();
        if selected.len() != allowed.len() || allowed.len() > 5000 {
            return Err("Outlook 資料夾選擇重複或過多。".into());
        }
        for id in &allowed {
            let item = choices
                .iter()
                .find(|c| &c.id == id)
                .ok_or("未知的資料夾選擇。")?;
            if item.parent.as_ref().is_some_and(|p| !allowed.contains(p)) {
                return Err("勾選子資料夾時也需保留上層資料夾。".into());
            }
        }
        Ok(Self {
            configured: true,
            allowed,
        })
    }
}

/// 檢查目前真正的祖先鏈，防止資料夾移到未授權分支後仍沿用舊代號。
pub(super) fn folder_allowed(policy: &Policy, folder: &IDispatch) -> AppResult<bool> {
    if !policy.configured {
        return Ok(true);
    }
    let mut current = folder.clone();
    for _ in 0..64 {
        let store = text(&current, "StoreID", 4096)?;
        let entry = text(&current, "EntryID", 4096)?;
        if !policy.permits(&store, &entry) {
            return Ok(false);
        }
        let parent = object(&get(&current, "Parent", &mut [])?)?;
        let class = i32::try_from(&get(&parent, "Class", &mut [])?)
            .map_err(|_| "無法核對 Outlook 資料夾上層。")?;
        if class == 1 {
            return Ok(true);
        } // olNamespace：已核對至資料檔根目錄。
        if class != 2 {
            return Err("無法核對 Outlook 資料夾範圍。".into());
        }
        current = parent;
    }
    Err("Outlook 資料夾層級過深，未開放讀取。".into())
}

pub(super) fn require_item(policy: &Policy, item: &IDispatch) -> AppResult<()> {
    if !policy.configured {
        return Ok(());
    }
    let parent = object(&get(item, "Parent", &mut [])?)?;
    if !folder_allowed(policy, &parent)? {
        return Err("此郵件不在已勾選的 Outlook 資料夾範圍，未讀取或匯出。".into());
    }
    Ok(())
}

/// 列舉已開啟 Outlook 的資料夾名稱供人選擇；不取得 Items、Body 或磁碟資料檔。
pub fn catalog(policy: &Policy, cancel: &AtomicBool) -> AppResult<Vec<Choice>> {
    let (_apartment, app) = batch::connect()?;
    catalog_from_app(policy, cancel, &app)
}

pub(super) fn catalog_from_app(
    policy: &Policy,
    cancel: &AtomicBool,
    app: &IDispatch,
) -> AppResult<Vec<Choice>> {
    let ns = object(&get(app, "GetNamespace", &mut ["MAPI".into()])?)?;
    let stores = object(&get(&ns, "Stores", &mut [])?)?;
    let count =
        i32::try_from(&get(&stores, "Count", &mut [])?).map_err(|_| "無法列出 Outlook 資料檔。")?;
    if count > 100 {
        return Err("Outlook 資料檔超過 100 個，請縮小開啟範圍。".into());
    }
    let started = Instant::now();
    let mut stack = Vec::new();
    for i in (1..=count).rev() {
        let store = object(&get(&stores, "Item", &mut [i.into()])?)?;
        stack.push((
            object(&get(&store, "GetRootFolder", &mut [])?)?,
            None,
            0usize,
        ));
    }
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    while let Some((folder, parent, depth)) = stack.pop() {
        batch::check_cancel(cancel)?;
        if started.elapsed() > Duration::from_secs(60) || result.len() >= 5000 || depth >= 64 {
            return Err("資料夾清單未完整取得（60 秒／5000 個／64 層上限）；未新增授權，請縮小 Outlook 開啟範圍。".into());
        }
        let store = text(&folder, "StoreID", 4096)?;
        let entry = text(&folder, "EntryID", 4096)?;
        let id = key(&store, &entry);
        if !seen.insert(id.clone()) {
            return Err("Outlook 資料夾結構重複，未新增授權。".into());
        }
        result.push(Choice {
            id: id.clone(),
            parent,
            depth,
            name: text(&folder, "Name", 1000)?,
            selected: policy.permits(&store, &entry),
        });
        let children = object(&get(&folder, "Folders", &mut [])?)?;
        let total =
            i32::try_from(&get(&children, "Count", &mut [])?).map_err(|_| "無法列出子資料夾。")?;
        if total < 0 || stack.len() + result.len() + total as usize > 5000 {
            return Err("資料夾清單超過 5000 個，未新增授權。".into());
        }
        for i in (1..=total).rev() {
            stack.push((
                object(&get(&children, "Item", &mut [i.into()])?)?,
                Some(id.clone()),
                depth + 1,
            ));
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_is_explicit_and_requires_parent() {
        let choices = vec![
            Choice {
                id: "a".into(),
                parent: None,
                name: "私人".into(),
                depth: 0,
                selected: true,
            },
            Choice {
                id: "b".into(),
                parent: Some("a".into()),
                name: "子層".into(),
                depth: 1,
                selected: true,
            },
        ];
        assert!(Policy::from_selection(&choices, &["b".into()]).is_err());
        assert!(Policy::from_selection(&choices, &["unknown".into()]).is_err());
        let policy = Policy::from_selection(&choices, &[]).unwrap();
        assert!(!policy.permits("store", "folder"));
        assert!(Policy::from_selection(&choices, &["a".into(), "b".into()]).is_ok());
    }
    #[test]
    fn policy_is_encrypted_persistent_and_corruption_fails_closed() {
        let root = std::env::temp_dir().join(format!(
            "lmai-mail-privacy-{}",
            crate::jobs::new_id().unwrap()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let policy = Policy {
            configured: true,
            allowed: [key("PRIVATE_STORE", "PRIVATE_FOLDER")]
                .into_iter()
                .collect(),
        };
        policy.save(&root).unwrap();
        let bytes = std::fs::read(root.join(FILE)).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("allowed"));
        let restored = Policy::load(&root).unwrap();
        assert!(restored.permits("PRIVATE_STORE", "PRIVATE_FOLDER"));
        assert!(!restored.permits("OTHER_STORE", "PRIVATE_FOLDER"));
        assert!(!restored.permits("PRIVATE_STORE", "NEW_FOLDER"));
        assert_eq!(restored.revision(), policy.revision());
        std::fs::write(root.join(FILE), b"corrupt").unwrap();
        assert!(Policy::load(&root).is_err());
        std::fs::remove_file(root.join(FILE)).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
