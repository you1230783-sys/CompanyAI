//! 穩定錯誤代碼與可攜診斷。只記錯誤，不收集對話、文件、工具參數或登入資料。
use crate::{storage, AppResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{fs, path::Path, sync::Mutex};

const MAX_EVENTS: usize = 200;
const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;
static LOG_LOCK: Mutex<()> = Mutex::new(());

/// 代碼一旦發行便不換意義，文件與程式採同一組對照。
pub fn description(code: &str) -> (&'static str, &'static str) {
    match code {
        "01" => (
            "選擇或建立專案資料夾時權限受到限制。",
            "請選擇可讀寫的資料夾；網路磁碟請確認連線帳號與公司存取權限。",
        ),
        "02" => (
            "檔案或資料夾存取受到限制。",
            "請確認讀寫權限；若是公司加密文件，請確認目前帳號可在原應用程式開啟。",
        ),
        "03" => (
            "找不到指定檔案或路徑。",
            "檔案可能已移動、刪除，或網路磁碟尚未連線；請重新選取來源。",
        ),
        "04" => (
            "檔案正被其他程式使用。",
            "請儲存並關閉占用檔案的程式，再重試目前操作。",
        ),
        "05" => (
            "網路連線未完成或等候逾時。",
            "請檢查網路與服務狀態；結果未知的任務請先查詢原任務，不重複送出。",
        ),
        "06" => (
            "登入或服務授權需要確認。",
            "請確認登入狀態與帳號權限，必要時重新登入。",
        ),
        "07" => (
            "AI 服務暫時無法完成請求。",
            "請依畫面等待重試或稍後繼續；重試仍失敗時匯出錯誤資訊。",
        ),
        "08" => (
            "收到的資料格式不符合預期。",
            "請檢查檔案或服務回覆格式；若持續發生，提供錯誤檔給維護者。",
        ),
        "09" => (
            "本機紀錄無法讀取、解密或保存。",
            "請確認磁碟空間、帳號及資料夾權限；保留現有紀錄，不要直接刪除。",
        ),
        "10" => (
            "Office 或 Outlook 操作未完成。",
            "請確認桌面版應用程式可開啟、文件沒有提示視窗，並依原操作重試。",
        ),
        _ => (
            "操作未完成，原因尚無法確定。",
            "請保留畫面提示並匯出錯誤資訊，交由維護者確認。",
        ),
    }
}

/// 只依明確文字／原生錯誤碼分類；不能把所有 WinOSError 都猜成權限不足。
pub fn classify(stage: &str, raw: &str) -> &'static str {
    let text = raw.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|word| text.contains(word));
    if has(&[
        "access is denied",
        "access denied",
        "permission denied",
        "os error 5)",
        "winerror 5]",
        "0x80070005",
        "拒絕存取",
        "存取被拒",
        "存取遭拒",
        "權限不足",
    ]) {
        return if stage == "project_folder" {
            "01"
        } else {
            "02"
        };
    }
    if has(&[
        "sharing violation",
        "os error 32)",
        "0x80070020",
        "另一個程序正在使用",
        "被其他程式使用",
    ]) {
        return "04";
    }
    if has(&[
        "os error 2)",
        "os error 3)",
        "0x80070002",
        "0x80070003",
        "系統找不到",
        "no such file",
    ]) {
        return "03";
    }
    if has(&[
        "登入已到期",
        "重新登入",
        "unauthorized",
        "http 401",
        "http 403",
    ]) {
        return "06";
    }
    if has(&[
        "ai_backend_error",
        "service unavailable",
        "http 502",
        "http 503",
        "http 504",
        "上游暫時",
    ]) {
        return "07";
    }
    if has(&[
        "timeout",
        "timed out",
        "逾時",
        "無法連線",
        "連線失敗",
        "connection refused",
    ]) {
        return "05";
    }
    if has(&["dpapi", "解密", "無法保存設定", "本機紀錄"]) {
        return "09";
    }
    if has(&["invalid json", "格式錯誤", "結構不符", "無法解析"]) {
        return "08";
    }
    if has(&["com error", "hresult", "outlook", "excel", "word"]) {
        return "10";
    }
    "99"
}

/// 已知憑證先替換，再整行移除可能的認證欄位。診斷不是原始 HTTP 封包擷取器。
fn redact(raw: &str, secrets: &[&str]) -> String {
    let mut text = raw.to_string();
    for secret in secrets.iter().filter(|value| !value.is_empty()) {
        text = text.replace(secret, "[憑證已隱藏]");
    }
    text.lines()
        .map(|line| {
            let lower = line.to_lowercase();
            if [
                "bearer ",
                "authorization",
                "access_token",
                "refresh_token",
                "api_key",
                "cookie",
                "password",
                "client_secret",
            ]
            .iter()
            .any(|key| lower.contains(key))
            {
                "[認證欄位已隱藏]"
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        .chars()
        .take(4000)
        .collect()
}

/// 只擷取標準數字代碼；不把代碼附近的檔名／服務正文帶入預設報告。
fn technical_codes(raw: &str) -> Vec<String> {
    let lower = raw.to_lowercase();
    let mut codes = Vec::new();
    for (prefix, width, hex) in [
        ("0x", 8, true),
        ("os error ", 5, false),
        ("winerror ", 5, false),
        ("http ", 3, false),
    ] {
        for (offset, _) in lower.match_indices(prefix) {
            let digits: String = lower[offset + prefix.len()..]
                .chars()
                .take_while(|c| {
                    if hex {
                        c.is_ascii_hexdigit()
                    } else {
                        c.is_ascii_digit()
                    }
                })
                .take(width + 1)
                .collect();
            if digits.is_empty() || digits.len() > width || (hex && digits.len() != width) {
                continue;
            }
            let code = format!("{prefix}{digits}");
            if !codes.contains(&code) {
                codes.push(code);
            }
            if codes.len() == 8 {
                return codes;
            }
        }
    }
    codes
}

#[derive(Serialize, Deserialize)]
struct Entry {
    at_unix: u64,
    code: String,
    stage: String,
    detail: String,
}
fn load(root: &Path) -> AppResult<Vec<Entry>> {
    let path = root.join("errors.dpapi");
    match fs::metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(error) => return Err(error.to_string()),
        Ok(info) if info.len() > MAX_FILE_BYTES => return Err("錯誤紀錄超過允許大小。".into()),
        _ => (),
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    serde_json::from_slice(&storage::protect(&bytes, false)?)
        .map_err(|_| "錯誤紀錄格式無效。".into())
}

/// 紀錄失敗時仍回傳原問題的友善說明，附註診斷未保存；不遞迴呼叫錯誤處理器。
pub fn report(root: &Path, stage: &str, raw: &str, secrets: &[&str]) -> String {
    let code = classify(stage, raw);
    let detail = redact(raw, secrets);
    let result = (|| -> AppResult<()> {
        let _guard = LOG_LOCK.lock().map_err(|_| "錯誤紀錄鎖定失敗。")?;
        let mut entries = load(root)?;
        entries.push(Entry {
            at_unix: crate::unix_now(),
            code: code.into(),
            stage: stage
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '.')
                .take(64)
                .collect(),
            detail: detail.clone(),
        });
        if entries.len() > MAX_EVENTS {
            entries.drain(..entries.len() - MAX_EVENTS);
        }
        let bytes = serde_json::to_vec(&entries).map_err(|e| e.to_string())?;
        storage::atomic_write(&root.join("errors.dpapi"), &storage::protect(&bytes, true)?)
    })();
    let (message, action) = description(code);
    let mut display = format!("Error Code: {code}\n{message}\n{action}");
    // 已有中文操作提示通常包含游標／續接等必要指引，保留它；底層例外不直接堆在畫面上。
    if detail
        .chars()
        .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
        && !["WinOSError", "WinError", "HRESULT", "os error", "Traceback"]
            .iter()
            .any(|word| detail.contains(word))
    {
        display.push_str(&format!(
            "\n{}",
            detail.chars().take(800).collect::<String>()
        ));
    }
    if result.is_err() {
        display.push_str("\n本次錯誤紀錄未能保存，請一併保留此畫面。")
    }
    display
}

/// 預設白名單輸出固定欄位，沒有對話、帳號、檔名與原始例外；詳細資訊必須由使用者勾選。
pub fn export(root: &Path, include_details: bool) -> AppResult<Vec<u8>> {
    let _guard = LOG_LOCK.lock().map_err(|_| "錯誤紀錄鎖定失敗。")?;
    let (entries, log_status) = match load(root) {
        Ok(entries) => (entries, "ok"),
        Err(_) => (vec![], "unreadable_or_invalid"),
    };
    let events: Vec<Value> = entries.iter().map(|entry| {
        let (message, action) = description(&entry.code);
        let mut value = json!({"at_unix":entry.at_unix,"code":entry.code,"stage":entry.stage,"message":message,"suggestion":action,"technical_codes":technical_codes(&entry.detail)});
        if include_details { value["detail"] = json!(entry.detail); }
        value
    }).collect();
    serde_json::to_vec_pretty(
        &json!({"format":"lmai-errors-v1","includes_conversation_log":false,"contains_content":include_details,
            "includes_error_details":include_details,"app_version":crate::service::CURRENT_VERSION,
            "platform":"windows-x64","exported_at_unix":crate::unix_now(),
            "local_date":crate::calendar::today().ok().map(|date| date.to_string()),
            "log_status":log_status,"retained_event_limit":MAX_EVENTS,"events":events
        }),
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn known_codes_do_not_guess_unknown_windows_errors() {
        assert_eq!(
            classify("project_folder", "Access is denied. (os error 5)"),
            "01"
        );
        assert_eq!(classify("interface", "0x80070005"), "02");
        assert_eq!(classify("project_folder", "WinOSError unexpected"), "99");
        assert_eq!(classify("tool", "AI_BACKEND_ERROR"), "07");
        assert_eq!(classify("tool", "os error 32)"), "04");
        assert!(!redact(
            "Authorization: Bearer sensitive\nSECRET in C:/private/file",
            &["SECRET"]
        )
        .contains("SECRET"));
        assert!(!redact("Authorization: Bearer sensitive", &[]).contains("sensitive"));
    }
    #[test]
    fn export_is_separate_and_private_by_default() {
        let root =
            std::env::temp_dir().join(format!("lmai-errors-{}", crate::jobs::new_id().unwrap()));
        let shown = report(
            &root,
            "project_folder",
            "Access denied: C:/private/project token-secret",
            &["token-secret"],
        );
        assert!(shown.starts_with("Error Code: 01"));
        let public = String::from_utf8(export(&root, false).unwrap()).unwrap();
        assert!(!public.contains("private/project"));
        assert!(!public.contains("token-secret"));
        assert!(public.contains("project_folder"));
        let detailed = String::from_utf8(export(&root, true).unwrap()).unwrap();
        assert!(detailed.contains("private/project"));
        assert!(!detailed.contains("token-secret"));
        fs::write(root.join("errors.dpapi"), b"damaged").unwrap();
        assert!(String::from_utf8(export(&root, false).unwrap())
            .unwrap()
            .contains("unreadable_or_invalid"));
        fs::remove_dir_all(root).unwrap();
    }
}
