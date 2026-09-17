//! Company AI 測試版：將通訊、登入、儲存與 Windows 介面分開，方便逐步擴充。
mod appearance;
pub mod auth;
pub mod config;
pub mod demo;
pub mod protocol;
pub mod selection;
pub mod service;
pub mod storage;
pub mod transport;
pub mod ui;

/// 統一使用可直接顯示給使用者的錯誤訊息，不把 Token 或 HTTP Header 印到記錄。
pub type AppResult<T> = Result<T, String>;

/// Windows API 使用以零結尾的 UTF-16 字串；只在平台邊界進行轉換。
pub fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 使用 Unix 秒数儲存授權到期時間，避免時區與日期字串解析差異。
pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
