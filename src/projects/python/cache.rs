//! 持有驗證過的唯讀 handle 到程序結束，避免每次分析重新雜湊整個 runtime。
//! Windows 的目錄 handle 不阻止新增子項目，因此快取命中仍核對完整路徑集合。
use super::*;
use std::{collections::BTreeSet, sync::Mutex};

struct VerifiedRuntime {
    root: PathBuf,
    _guards: Vec<File>,
    paths: BTreeSet<PathBuf>,
}

#[derive(Default)]
struct RuntimeCache(Mutex<Option<VerifiedRuntime>>);

static CACHE: RuntimeCache = RuntimeCache(Mutex::new(None));

impl RuntimeCache {
    fn verify(
        &self,
        root: &Path,
        cancel: &AtomicBool,
        verify: impl FnOnce() -> AppResult<Vec<File>>,
    ) -> AppResult<()> {
        // 只在驗證期間持鎖，兩個 worker 不在此鎖內執行；第一次失敗不保存狀態。
        let mut cached = self
            .0
            .lock()
            .map_err(|_| "Python 驗證狀態異常，請重啟程式。")?;
        if cancel.load(Ordering::Relaxed) {
            return Err("Python 驗證已取消。".into());
        }
        if let Some(runtime) = cached.as_ref() {
            if runtime.root != root {
                return Err("Python 環境位置已變更，請重啟程式後再試。".into());
            }
            return check_paths(root, &runtime.paths, cancel);
        }
        let guards = verify()?;
        // 此清單已在 verify 中驗證雜湊，且清單的 handle 仍由 guards 鎖住。
        let raw = std::fs::read(root.join("runtime-manifest.json")).map_err(|e| e.to_string())?;
        let entries: Vec<RuntimeFile> = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
        let mut paths = BTreeSet::from([PathBuf::from("runtime-manifest.json")]);
        for entry in entries {
            let path = files::relative(&entry.path)?;
            paths.insert(path.clone());
            let mut parent = path.parent();
            while let Some(directory) = parent.filter(|p| !p.as_os_str().is_empty()) {
                paths.insert(directory.to_path_buf());
                parent = directory.parent();
            }
        }
        check_paths(root, &paths, cancel)?;
        *cached = Some(VerifiedRuntime {
            root: root.to_path_buf(),
            _guards: guards,
            paths,
        });
        Ok(())
    }
}

/// 不重新雜湊已鎖定的檔案，但仍拒絕後加的模組／bytecode 或空目錄。
fn check_paths(root: &Path, expected: &BTreeSet<PathBuf>, cancel: &AtomicBool) -> AppResult<()> {
    use std::os::windows::fs::MetadataExt;
    let mut actual = BTreeSet::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory).map_err(|e| e.to_string())? {
            if cancel.load(Ordering::Relaxed) {
                return Err("Python 驗證已取消。".into());
            }
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_path_buf();
            let metadata = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            if metadata.file_attributes() & 0x400 != 0 || !expected.contains(&relative) {
                return Err("Python 環境新增了清單外項目或連結；請使用完整安裝包修復。".into());
            }
            if metadata.is_dir() {
                pending.push(path);
            }
            actual.insert(relative);
        }
    }
    if &actual != expected {
        return Err("Python 環境檔案集合已變更；請重新安裝。".into());
    }
    Ok(())
}

/// 回傳成功時，程序全域狀態已保留所有檔案鎖，不由工作程序生命週期釋放。
pub(super) fn verified_runtime(root: &Path, cancel: &AtomicBool) -> AppResult<()> {
    CACHE.verify(root, cancel, || super::verify_runtime_files(root, cancel))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "lmai-python-cache-{}",
                crate::jobs::new_id().unwrap()
            ));
            std::fs::create_dir(&path).unwrap();
            std::fs::write(path.join("module.py"), b"verified").unwrap();
            let manifest = json!([{"path":"module.py","sha256":format!("{:x}",Sha256::digest(b"verified")),"size":8}]);
            std::fs::write(
                path.join("runtime-manifest.json"),
                serde_json::to_vec(&manifest).unwrap(),
            )
            .unwrap();
            Self(path)
        }
        fn verify_files(&self, calls: &AtomicUsize) -> AppResult<Vec<File>> {
            calls.fetch_add(1, Ordering::Relaxed);
            let mut guards = files::pin(&self.0)?;
            read_pinned(&self.0.join("runtime-manifest.json"), 4096, &mut guards)?;
            let data = read_pinned(&self.0.join("module.py"), 4096, &mut guards)?;
            if data != b"verified" {
                return Err("fixture hash differs".into());
            }
            Ok(guards)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            assert!(self.0.starts_with(std::env::temp_dir()));
            assert!(self
                .0
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("lmai-python-cache-"));
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn cache_retains_locks_and_rejects_new_modules_without_rehashing() {
        let fixture = Fixture::new();
        let cache = RuntimeCache::default();
        let calls = AtomicUsize::new(0);
        let cancel = AtomicBool::new(false);
        for _ in 0..2 {
            cache
                .verify(&fixture.0, &cancel, || fixture.verify_files(&calls))
                .unwrap();
        }
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert!(std::fs::write(fixture.0.join("module.py"), b"modified").is_err());
        assert!(std::fs::remove_file(fixture.0.join("module.py")).is_err());
        assert!(std::fs::rename(&fixture.0, fixture.0.with_extension("moved")).is_err());
        // 目錄的讀取鎖不保證阻止新增；每次沿用必須重新核對集合。
        let extra = fixture.0.join("extra.py");
        std::fs::write(&extra, b"untrusted").unwrap();
        assert!(cache
            .verify(&fixture.0, &cancel, || fixture.verify_files(&calls))
            .is_err());
        std::fs::remove_file(extra).unwrap();
        cache
            .verify(&fixture.0, &cancel, || fixture.verify_files(&calls))
            .unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        let other = Fixture::new();
        assert!(cache
            .verify(&other.0, &cancel, || other.verify_files(&calls))
            .is_err());
        drop(cache);
        std::fs::write(fixture.0.join("module.py"), b"after process exit").unwrap();
    }

    #[test]
    fn failed_or_cancelled_verification_is_not_cached() {
        let fixture = Fixture::new();
        let cache = RuntimeCache::default();
        let calls = AtomicUsize::new(0);
        let cancel = AtomicBool::new(false);
        std::fs::write(fixture.0.join("module.py"), b"corrupt").unwrap();
        assert!(cache
            .verify(&fixture.0, &cancel, || fixture.verify_files(&calls))
            .is_err());
        std::fs::write(fixture.0.join("module.py"), b"verified").unwrap();
        cancel.store(true, Ordering::Relaxed);
        assert!(cache
            .verify(&fixture.0, &cancel, || fixture.verify_files(&calls))
            .is_err());
        cancel.store(false, Ordering::Relaxed);
        cache
            .verify(&fixture.0, &cancel, || fixture.verify_files(&calls))
            .unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn concurrent_first_use_verifies_once() {
        let fixture = Fixture::new();
        let cache = RuntimeCache::default();
        let calls = AtomicUsize::new(0);
        let barrier = std::sync::Barrier::new(2);
        std::thread::scope(|scope| {
            let workers: Vec<_> = (0..2)
                .map(|_| {
                    scope.spawn(|| {
                        barrier.wait();
                        cache
                            .verify(&fixture.0, &AtomicBool::new(false), || {
                                fixture.verify_files(&calls)
                            })
                            .unwrap();
                    })
                })
                .collect();
            for worker in workers {
                worker.join().unwrap();
            }
        });
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }
}
