//! 專案內部模型請求只作為同一工作步驟。以識別碼過濾通知，不猜測標題或隱藏其他工作。
use crate::{jobs, storage, AppResult};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};
static IDS: OnceLock<Mutex<BTreeMap<PathBuf, Vec<String>>>> = OnceLock::new();
fn loaded<'a>(
    all: &'a mut BTreeMap<PathBuf, Vec<String>>,
    root: &Path,
) -> AppResult<&'a mut Vec<String>> {
    if !all.contains_key(root) {
        let path = root.join("project-child-requests.dpapi");
        let values = match std::fs::read(&path) {
            Ok(bytes) if bytes.len() <= 4_000_000 => {
                serde_json::from_slice(&storage::protect(&bytes, false)?)
                    .map_err(|_| "專案請求索引無法解析。")?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            _ => return Err("專案請求索引無法讀取。".into()),
        };
        all.insert(root.to_owned(), values);
    }
    all.get_mut(root).ok_or("缺少專案請求索引。".into())
}
pub fn register(root: &Path, id: &str) -> AppResult<()> {
    jobs::validate_id(id)?;
    let mut all = IDS
        .get_or_init(Default::default)
        .lock()
        .map_err(|_| "專案請求索引被中斷。")?;
    let ids = loaded(&mut all, root)?;
    if !ids.iter().any(|old| old == id) {
        ids.push(id.into());
        if ids.len() > 32_000 {
            ids.drain(..1000);
        }
        storage::atomic_write(
            &root.join("project-child-requests.dpapi"),
            &storage::protect(&serde_json::to_vec(ids).map_err(|e| e.to_string())?, true)?,
        )?;
    }
    Ok(())
}
pub fn internal(root: &Path, id: Option<&str>) -> bool {
    let Some(id) = id else {
        return false;
    };
    let Ok(mut all) = IDS.get_or_init(Default::default).lock() else {
        return false;
    };
    loaded(&mut all, root).is_ok_and(|ids| ids.iter().any(|old| old == id))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn request_and_task_ids_survive_restart_without_hiding_unrelated_notices() {
        let root = std::env::current_dir()
            .unwrap()
            .join(".build/event-tests")
            .join(jobs::new_id().unwrap());
        register(&root, "request_1").unwrap();
        register(&root, "task_1").unwrap();
        assert!(internal(&root, Some("task_1")));
        assert!(!internal(&root, Some("normal_task")));
        assert!(!internal(&root, None));
        IDS.get().unwrap().lock().unwrap().remove(&root);
        assert!(internal(&root, Some("request_1")));
    }
}
