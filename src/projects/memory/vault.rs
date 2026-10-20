//! 專案私有儲存：父目錄固定、拒絕連結、獨占交易及隨機暫存原子替換。
//! 不使用一般文件工具，也不把 DPAPI 明文寫到磁碟。
use crate::{jobs, projects::files, storage, AppResult};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
    path::{Path, PathBuf},
};
use windows_sys::Win32::Storage::FileSystem::*;

#[derive(Clone)]
pub(super) struct Vault {
    root: PathBuf,
}
pub(super) struct Transaction {
    root: PathBuf,
    _pins: Vec<File>,
    _lock: File,
}
impl Vault {
    pub fn new(root: &Path) -> AppResult<Self> {
        files::validate_root(root)?;
        let _pins = files::pin(root)?;
        let directory = root.join(".lmai");
        match fs::create_dir(&directory) {
            Ok(()) => {
                // 點開頭在 Windows 並不代表隱藏；仍以工具邊界拒絕存取，而非依賴此屬性。
                unsafe {
                    SetFileAttributesW(
                        crate::wide(&directory.to_string_lossy()).as_ptr(),
                        FILE_ATTRIBUTE_HIDDEN,
                    );
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(format!("無法建立專案記憶：{e}")),
        }
        let vault = Self { root: directory };
        let _transaction = vault.transaction()?;
        Ok(vault)
    }
    pub fn transaction(&self) -> AppResult<Transaction> {
        let pins = files::pin(&self.root)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .share_mode(0)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(self.root.join("store.lock"))
            .map_err(|_| "專案記憶正在使用或無法存取，請稍後再試。")?;
        let mut info = BY_HANDLE_FILE_INFORMATION::default();
        if unsafe { GetFileInformationByHandle(lock.as_raw_handle(), &mut info) } == 0
            || info.dwFileAttributes & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY)
                != 0
            || info.nNumberOfLinks != 1
        {
            return Err("專案記憶鎖不是一般檔案。".into());
        }
        Ok(Transaction {
            root: self.root.clone(),
            _pins: pins,
            _lock: lock,
        })
    }
}
impl Transaction {
    fn path(&self, bucket: &str, key: &str) -> AppResult<(PathBuf, files::StageGuard)> {
        // 目錄與鍵都由程式產生；即使未來誤接模型參數，也不得形成相對路徑。
        jobs::validate_id(bucket)?;
        jobs::validate_id(key)?;
        let folder = self.root.join(bucket);
        match fs::create_dir(&folder) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e.to_string()),
        }
        let pins = files::pin_memory_directory(&folder)?;
        Ok((folder.join(format!("{key}.dpapi")), pins))
    }
    pub fn read<T: DeserializeOwned>(&self, bucket: &str, key: &str) -> AppResult<Option<T>> {
        let (path, _pins) = self.path(bucket, key)?;
        match fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.to_string()),
            Ok(_) => (),
        }
        let mut file = files::checked_file(&path)?;
        if file.metadata().map_err(|e| e.to_string())?.len() > 8_500_000 {
            return Err("專案記憶檔案過大。".into());
        }
        let mut encrypted = Vec::new();
        file.read_to_end(&mut encrypted)
            .map_err(|e| e.to_string())?;
        let plain = storage::protect(&encrypted, false)?;
        serde_json::from_slice(&plain)
            .map(Some)
            .map_err(|_| "專案記憶格式無效；保留原檔。".into())
    }
    /// 只清除程式管理的快取鍵；父目錄固定，刪除不跟隨重新解析點。
    pub fn remove_cache(&self, key: &str) -> AppResult<()> {
        let (path, _pins) = self.path("cache", key)?;
        if fs::symlink_metadata(&path).is_ok() {
            files::checked_file(&path)?;
            fs::remove_file(path).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    pub fn write<T: Serialize>(&self, bucket: &str, key: &str, value: &T) -> AppResult<()> {
        let (path, _pins) = self.path(bucket, key)?;
        if fs::symlink_metadata(&path).is_ok() {
            files::checked_file(&path)?;
        }
        let plain = serde_json::to_vec(value).map_err(|e| e.to_string())?;
        if plain.len() > 8_000_000 {
            return Err("專案記憶已達單檔 8 MB 上限。".into());
        }
        let encrypted = storage::protect(&plain, true)?;
        let temporary = path.with_extension(format!("{}.tmp", jobs::new_id()?));
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .share_mode(0)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        let result = (|| {
            output.write_all(&encrypted).map_err(|e| e.to_string())?;
            output.sync_all().map_err(|e| e.to_string())?;
            drop(output);
            fs::rename(&temporary, &path).map_err(|e| format!("專案記憶原子替換失敗：{e}"))
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}
