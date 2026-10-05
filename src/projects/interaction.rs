//! 可恢復的檔案占用狀態；只有確定尚未執行的讀取可以在使用者確認後重試。
use crate::AppResult;
use std::{path::Path, sync::atomic::AtomicBool, time::Instant};
pub type FileWaiter = Box<dyn FnMut(&str, &AtomicBool, Instant) -> AppResult<bool> + Send>;
pub const BUSY: &str = "[FILE_IN_USE]";
pub const DEFERRED: &str = "[FILE_READ_DEFERRED]";
pub fn open_error(path: &Path, error: std::io::Error) -> String {
    if matches!(error.raw_os_error(), Some(32 | 33)) {
        format!("{BUSY}「{}」目前正被其他程式使用。請先儲存並關閉該檔案，再試一次；若是持續寫入的 LOG，請使用已輪替或另存的紀錄。",path.file_name().unwrap_or_default().to_string_lossy())
    } else {
        format!(
            "無法讀取「{}」：{error}",
            path.file_name().unwrap_or_default().to_string_lossy()
        )
    }
}
