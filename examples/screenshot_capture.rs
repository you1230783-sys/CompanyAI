//! 人工框選驗證：只保存使用者框選的JPEG到明確指定的測試檔，不連線網站。
use std::{path::PathBuf, sync::atomic::AtomicBool};
fn main() -> Result<(), String> {
    let path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("請指定新的測試JPEG路徑。")?;
    if path.exists() {
        return Err("測試輸出已存在，不覆寫。".into());
    }
    let bytes = company_ai::screenshot::capture(&AtomicBool::new(false))?;
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    println!(
        "PASS: captured JPEG, {} bytes; no network request",
        bytes.len()
    );
    Ok(())
}
