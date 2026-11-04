//! 非預期 panic 的最小紀錄與 Rust／原生邊界。
//!
//! Hook 只同步保存固定欄位，不讀取 payload、Token、文件或任務參數。
//! 捕捉 unwind 後停止使用可能部分修改的狀態；不能將它當成一般 Result 重試。
use crate::AppResult;
use serde::{Deserialize, Serialize};
use std::{
    cell::Cell,
    fs::{self, OpenOptions},
    hash::{Hash, Hasher},
    io::Write,
    panic::{self, AssertUnwindSafe},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        OnceLock,
    },
    thread::{self, JoinHandle},
};

static DIRECTORY: OnceLock<PathBuf> = OnceLock::new();
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
static FAULTED: AtomicBool = AtomicBool::new(false);
static LOG_FAILED: AtomicBool = AtomicBool::new(false);
thread_local! { static BOUNDARY: Cell<u8> = const { Cell::new(0) }; }
const EVENT_LIMIT: u64 = 32;

/// 類別只有固定數字：0=未捕捉、1=背景、2=Win32、3=COM。來源只保存雜湊。
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PanicEntry {
    at_unix: u64,
    process: u32,
    event: u64,
    boundary: u8,
    source_hash: u64,
    line: u32,
    column: u32,
    version: String,
}

/// 程序入口最先安裝；測試使用獨立資料夾，不能覆寫使用者的錯誤資料。
pub fn install(root: &Path) -> AppResult<()> {
    if DIRECTORY.get().is_some() {
        return Ok(());
    }
    let directory = root.join("panic-events");
    fs::create_dir_all(&directory).map_err(|_| "無法建立異常紀錄目錄。")?;
    use std::os::windows::fs::MetadataExt;
    if fs::symlink_metadata(&directory)
        .map_err(|_| "無法驗證異常紀錄目錄。")?
        .file_attributes()
        & 0x400
        != 0
    {
        return Err("異常紀錄目錄不能是連結。".into());
    }
    // 啟動前輪替，只處理本模組命名的普通檔案，不循連結或遞迴刪除。
    let mut files: Vec<_> = fs::read_dir(&directory)
        .map_err(|_| "無法讀取異常紀錄目錄。")?
        .filter_map(Result::ok)
        .filter(|e| event_file(&e.path()))
        .collect();
    files.sort_by_key(|e| e.file_name());
    let remove = files.len().saturating_sub(64);
    for file in files.into_iter().take(remove) {
        let _ = fs::remove_file(file.path());
    }
    if DIRECTORY.set(directory).is_err() {
        return Ok(());
    }
    panic::set_hook(Box::new(|info| {
        FAULTED.store(true, Ordering::Release);
        let event = SEQUENCE.fetch_add(1, Ordering::Relaxed) + 1;
        if event > EVENT_LIMIT {
            LOG_FAILED.store(true, Ordering::Release);
            return;
        }
        let Some(directory) = DIRECTORY.get() else {
            return;
        };
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let (line, column) = if let Some(site) = info.location() {
            site.file().hash(&mut hasher);
            (site.line(), site.column())
        } else {
            (0, 0)
        };
        let at = crate::unix_now();
        let process = std::process::id();
        let boundary = BOUNDARY.try_with(Cell::get).unwrap_or(0);
        let path = directory.join(format!("{at:020}-{process:010}-{event:03}.json"));
        // 不呼叫通用錯誤處理器（它可能正持有鎖），也不呼叫會輸出payload的預設hook。
        // 每事件獨立檔案，create_new避免覆寫；sync_all在返回panic runtime前完成。
        let saved = (|| -> std::io::Result<()> {
            let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
            writeln!(file, "{{\"at_unix\":{at},\"process\":{process},\"event\":{event},\"boundary\":{boundary},\"source_hash\":{},\"line\":{line},\"column\":{column},\"version\":\"{}\"}}", hasher.finish(), env!("CARGO_PKG_VERSION"))?;
            file.sync_all()
        })();
        if saved.is_err() {
            LOG_FAILED.store(true, Ordering::Release);
        }
    }));
    Ok(())
}

fn event_file(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    name.strip_suffix(".json")
        .is_some_and(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit() || c == b'-'))
        && fs::symlink_metadata(path)
            .is_ok_and(|m| m.is_file() && m.file_attributes() & 0x400 == 0 && m.len() < 2048)
}

/// 診斷只投影型別固定的事件；不回傳檔名、資料夾或任意額外JSON欄位。
pub fn events(root: &Path) -> Vec<PanicEntry> {
    let Ok(entries) = fs::read_dir(root.join("panic-events")) else {
        return vec![];
    };
    let mut paths: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| event_file(p))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .rev()
        .take(64)
        .filter_map(|path| {
            let bytes = fs::read(path).ok()?;
            let event: PanicEntry = serde_json::from_slice(&bytes).ok()?;
            (event.boundary <= 3
                && event.version.len() < 32
                && event
                    .version
                    .bytes()
                    .all(|b| b.is_ascii_digit() || b == b'.'))
            .then_some(event)
        })
        .collect()
}

/// 捕捉到的非預期錯誤會鎖住本次執行的寫入功能，直到使用者重新啟動。
pub fn faulted() -> bool {
    FAULTED.load(Ordering::Acquire)
}

/// Hook寫檔失敗時仍要告知使用者，不能宣稱已保存診斷。
pub fn log_failed() -> bool {
    LOG_FAILED.load(Ordering::Acquire)
}

fn guarded<T>(kind: u8, work: impl FnOnce() -> T) -> Result<T, ()> {
    let previous = BOUNDARY.with(|scope| scope.replace(kind));
    // SAFETY（例外安全）：閉包失敗後不恢復其中的業務操作；全域fault鎖定後續寫入。
    let result = panic::catch_unwind(AssertUnwindSafe(work));
    BOUNDARY.with(|scope| scope.set(previous));
    result.map_err(|payload| {
        FAULTED.store(true, Ordering::Release);
        // 不執行未知payload的Drop，避免在回呼邊界再次panic；只在異常路徑洩漏此物件。
        std::mem::forget(payload);
    })
}

/// UI背景工作只透過訊息回傳；panic會進入可匯出診斷的保護狀態，不重跑工作。
pub fn spawn(work: impl FnOnce() + Send + 'static) -> JoinHandle<()> {
    thread::spawn(move || {
        let _ = guarded(1, work);
    })
}

/// 批次panic仍是全域故障，不能轉成一般單項錯誤後繼續提交成果。
/// 保留hook的同步診斷，並通知同組工作取消；guarded也避免再次解構未知panic payload。
pub(crate) fn batch<T>(cancel: &AtomicBool, work: impl FnOnce() -> AppResult<T>) -> AppResult<T> {
    guarded(1, work).unwrap_or_else(|_| {
        cancel.store(true, Ordering::Release);
        Err("Error Code: 13\n批次工作發生內部異常，程式已進入保護狀態；停止後續操作，請核對既有成果後重新啟動，不重播成功寫入。".into())
    })
}

/// 必須在extern system函式內呼叫，不能只在DispatchMessage外層捕捉。
pub fn win32<T>(fallback: T, work: impl FnOnce() -> T) -> T {
    guarded(2, work).unwrap_or(fallback)
}

/// 傳給WebView2的Rust閉包在COM ABI返回前轉換成HRESULT失敗。
pub fn com<T>(work: impl FnOnce() -> windows::core::Result<T>) -> windows::core::Result<T> {
    guarded(3, work).unwrap_or_else(|_| {
        Err(windows::core::Error::from(
            windows::Win32::Foundation::E_UNEXPECTED,
        ))
    })
}
