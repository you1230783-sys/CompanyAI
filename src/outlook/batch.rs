//! 多封郵件的唯讀快照與受限 MSG 匯出。COM 物件只留在建立它的 STA 執行緒。
use super::*;
use crate::{
    attachments::{self, Attachment, AttachmentRules, Incoming},
    jobs,
};
use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
mod query;
pub use query::SearchScope;

pub const MAX_MAILS: usize = 50;
pub const SKILL: &str = include_str!("skill.md");

#[derive(Clone, Serialize)]
pub struct Mail {
    pub id: String,
    /// Outlook 資料夾顯示路徑；不是 PST／OST 的磁碟路徑。
    pub folder: String,
    #[serde(flatten)]
    pub preview: MailPreview,
    #[serde(skip)]
    pub store_id: String,
}
#[derive(Clone, Default, Serialize)]
pub struct MailList {
    pub mails: Vec<Mail>,
    pub scope: String,
    pub truncated: bool,
    pub notice: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub tool: String,
    pub mail_id: String,
    pub reason: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    pub schema_version: u32,
    pub summary: String,
    pub requests: Vec<Request>,
}

pub fn check_cancel(cancel: &AtomicBool) -> AppResult<()> {
    if cancel.load(Ordering::Relaxed) {
        Err("已停止郵件分析，不再匯出或自動送出。".into())
    } else {
        Ok(())
    }
}
/// 嚴格解析整份回覆；任何越界 ID／未知工具都拒絕，不能部分執行。
pub fn decision(reply: &str, mails: &[Mail], allow: bool, maximum: usize) -> AppResult<Decision> {
    let trimmed = reply.trim();
    let raw = trimmed
        .strip_prefix("```json")
        .and_then(|s| s.strip_suffix("```"))
        .unwrap_or(trimmed)
        .trim();
    let result: Decision = serde_json::from_str(raw)
        .map_err(|_| "AI 未依郵件工具契約回覆；未匯出任何郵件，請重試或改用其他模型。")?;
    if result.schema_version != 1
        || result.summary.len() > 16_000
        || result.requests.len() > maximum.min(20)
        || (!allow && !result.requests.is_empty())
    {
        return Err("AI 郵件請求超出本次授權範圍。".into());
    }
    let mut seen = std::collections::HashSet::new();
    for request in &result.requests {
        if request.tool != "outlook.export_msg"
            || request.reason.len() > 2000
            || !mails.iter().any(|m| m.id == request.mail_id)
            || !seen.insert(&request.mail_id)
        {
            return Err("AI 指定未知工具、重複郵件或批次外的郵件；未執行匯出。".into());
        }
    }
    Ok(result)
}
pub fn prompt(mails: &[Mail], allow: bool, maximum: usize) -> AppResult<String> {
    let data =
        serde_json::json!({"allow_export":allow,"max_exports":maximum.min(20),"mails":mails});
    let prompt = format!("{SKILL}\n\n本次資料：\n{data}");
    if prompt.len() > 60_000 {
        return Err("郵件基本資訊過長，請減少勾選數量。".into());
    }
    Ok(prompt)
}
pub fn cutoff(today: NaiveDate, period: &str) -> AppResult<NaiveDate> {
    let days = match period {
        "today" => 0,
        "three_days" => 2,
        "week" => today.weekday().num_days_from_monday() as i64,
        _ => return Err("不支援的郵件日期範圍。".into()),
    };
    today
        .checked_sub_signed(Duration::days(days))
        .ok_or("郵件日期範圍不正確。".into())
}
fn today() -> AppResult<NaiveDate> {
    let mut time = windows_sys::Win32::Foundation::SYSTEMTIME::default();
    unsafe {
        windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut time);
    }
    NaiveDate::from_ymd_opt(time.wYear as i32, time.wMonth as u32, time.wDay as u32)
        .ok_or("無法讀取本機日期。".into())
}
/// 保留 OLE DATE 的小數時間，跨資料夾合併時才能依真正的收件時間排序。
fn received_time(mail: &IDispatch) -> AppResult<f64> {
    let value = get(mail, "ReceivedTime", &mut [])?;
    let mut date = VARIANT::default();
    unsafe { VariantChangeType(&mut date, &value, VAR_CHANGE_FLAGS(0), VT_DATE) }
        .map_err(|_| "無法讀取郵件日期。")?;
    let days = unsafe { date.Anonymous.Anonymous.Anonymous.date };
    if !days.is_finite() || !(0.0..3_000_000.0).contains(&days) {
        return Err("郵件日期不正確。".into());
    }
    Ok(days)
}
fn received_date(days: f64) -> AppResult<NaiveDate> {
    NaiveDate::from_ymd_opt(1899, 12, 30)
        .ok_or("日期基準錯誤。")?
        .checked_add_signed(Duration::days(days.floor() as i64))
        .ok_or("郵件日期超出範圍。".into())
}
fn connect() -> AppResult<(ComApartment, IDispatch)> {
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() }
        .map_err(|_| "無法初始化 Outlook COM。")?;
    let apartment = ComApartment;
    let class = unsafe { CLSIDFromProgID(PCWSTR(wide("Outlook.Application").as_ptr())) }
        .map_err(|_| "找不到 Classic Outlook。")?;
    let mut active = None;
    unsafe { GetActiveObject(&class, None, &mut active) }
        .map_err(|_| "請先開啟 Classic Outlook，並使用相同 Windows 帳號與權限。")?;
    let app = active
        .ok_or("找不到 Outlook。")?
        .cast()
        .map_err(|_| "無法連接 Outlook。")?;
    Ok((apartment, app))
}
fn snapshot(item: &IDispatch) -> AppResult<Mail> {
    let parent = object(&get(item, "Parent", &mut [])?)?;
    Ok(Mail {
        id: jobs::new_id()?,
        folder: text(&parent, "FolderPath", 4096)?,
        store_id: text(&parent, "StoreID", 4096)?,
        preview: MailPreview {
            subject: text(item, "Subject", 3000)?,
            sender: text(item, "SenderName", 3000)?,
            to: text(item, "To", 4096)?,
            cc: text(item, "CC", 4096)?,
            received_at: text(item, "ReceivedTime", 200)?,
            unread: bool::try_from(&get(item, "UnRead", &mut [])?)
                .map_err(|_| "無法讀取未讀狀態。")?,
            body: None,
            entry_id: text(item, "EntryID", 4096)?,
        },
    })
}
/// 日期使用本機日曆日；跨信箱／資料檔查詢交由 query，選取郵件維持直接快照。
pub fn list(
    period: &str,
    unread: bool,
    scope: SearchScope,
    cancel: &AtomicBool,
) -> AppResult<MailList> {
    let (_apartment, app) = connect()?;
    if period != "selected" {
        return query::list(&app, period, unread, scope, cancel);
    }
    let mut list = MailList {
        scope: "Outlook 目前選取".into(),
        ..Default::default()
    };
    check_cancel(cancel)?;
    let window = object(&get(&app, "ActiveWindow", &mut [])?)?;
    if i32::try_from(&get(&window, "Class", &mut [])?).ok() == Some(35) {
        let item = object(&get(&window, "CurrentItem", &mut [])?)?;
        if i32::try_from(&get(&item, "Class", &mut [])?).ok() != Some(43) {
            return Err("目前項目不是郵件。".into());
        }
        list.mails.push(snapshot(&item)?);
        return Ok(list);
    }
    let collection = object(&get(&window, "Selection", &mut [])?)?;
    let count =
        i32::try_from(&get(&collection, "Count", &mut [])?).map_err(|_| "無法取得郵件數量。")?;
    for index in 1..=count.min(10_000) {
        check_cancel(cancel)?;
        let item = object(&get(&collection, "Item", &mut [VARIANT::from(index)])?)?;
        if i32::try_from(&get(&item, "Class", &mut [])?).ok() != Some(43) {
            continue;
        }
        if list.mails.len() == MAX_MAILS {
            list.truncated = true;
            break;
        }
        list.mails.push(snapshot(&item)?);
    }
    if count > 10_000 {
        list.truncated = true;
    }
    Ok(list)
}

/// 明文 MSG 僅存於 App 新建的私有子目錄，完成加密後立即清理。
struct ExportDirectory(PathBuf);
impl Drop for ExportDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.0.join("message.msg"));
        let _ = fs::remove_dir(&self.0);
    }
}
pub fn export(
    root: &Path,
    conversation: &str,
    mail: &Mail,
    rules: &AttachmentRules,
    usage: (usize, u64),
    cancel: &AtomicBool,
) -> AppResult<Attachment> {
    check_cancel(cancel)?;
    let (_apartment, app) = connect()?;
    let namespace = object(&get(&app, "GetNamespace", &mut [VARIANT::from("MAPI")])?)?;
    // COM 參數逆序；同時指定 StoreID，避免誤讀其他信箱的同名郵件。
    let item = object(&get(
        &namespace,
        "GetItemFromID",
        &mut [
            VARIANT::from(mail.store_id.as_str()),
            VARIANT::from(mail.preview.entry_id.as_str()),
        ],
    )?)?;
    if i32::try_from(&get(&item, "Class", &mut [])?).ok() != Some(43)
        || text(&item, "EntryID", 4096)? != mail.preview.entry_id
    {
        return Err("原郵件已移動或不存在，請重新取得清單。".into());
    }
    // 快照關鍵欄位改變時停止，不能把新內容當成先前授權的郵件。
    if text(&item, "Subject", 3000)? != mail.preview.subject
        || text(&item, "SenderName", 3000)? != mail.preview.sender
    {
        return Err("郵件資訊已改變，請重新取得清單。".into());
    }
    let id = jobs::new_id()?;
    let directory = root.join("outlook-exports").join(&id);
    fs::create_dir_all(&directory).map_err(|_| "無法建立郵件暫存目錄。")?;
    let directory = ExportDirectory(directory);
    let path = directory.0.join("message.msg");
    check_cancel(cancel)?;
    get(
        &item,
        "SaveAs",
        &mut [
            VARIANT::from(9i32),
            VARIANT::from(path.to_string_lossy().as_ref()),
        ],
    )?;
    check_cancel(cancel)?;
    let size = fs::metadata(&path)
        .map_err(|_| "Outlook 未產生 MSG 檔。")?
        .len();
    let name = format!("mail_{}.msg", mail.id);
    rules.check(&name, size, usage.0, usage.1)?;
    let mut source = fs::File::open(&path).map_err(|_| "無法讀取 MSG 暫存檔。")?;
    let mut incoming = Incoming::new(root, id.clone(), size)?;
    let mut bytes = vec![0; attachments::CHUNK_BYTES];
    loop {
        check_cancel(cancel)?;
        let n = source
            .read(&mut bytes)
            .map_err(|_| "無法讀取郵件匯出內容。")?;
        if n == 0 {
            break;
        }
        incoming.append_bytes(&bytes[..n])?;
    }
    incoming.finish()?;
    Ok(Attachment {
        id,
        conversation_id: conversation.into(),
        name,
        size,
        mime_type: "application/vnd.ms-outlook".into(),
        remote: None,
        state: "upload_pending".into(),
        message: String::new(),
        sent: false,
        removed: false,
        uploaded_bytes: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn date_ranges_and_tool_allowlist() {
        let today = NaiveDate::from_ymd_opt(2026, 9, 17).unwrap();
        assert_eq!(cutoff(today, "three_days").unwrap().day(), 15);
        assert_eq!(cutoff(today, "week").unwrap().day(), 14);
        assert_eq!(cutoff(today, "today").unwrap(), today);
        let mails = vec![Mail {
            id: "m1".into(),
            folder: "測試收件匣".into(),
            preview: demo_mail(false),
            store_id: "private-store".into(),
        }];
        let request = r#"{"schema_version":1,"summary":"測試","requests":[{"tool":"outlook.export_msg","mail_id":"m1","reason":"需正文"}]}"#;
        assert!(decision(request, &mails, true, 20).is_ok());
        assert!(decision(request, &mails, false, 20).is_err());
        assert!(decision(&request.replace("m1", "other"), &mails, true, 20).is_err());
        assert!(decision(
            &request.replace("outlook.export_msg", "run_command"),
            &mails,
            true,
            20
        )
        .is_err());
        let prompt = prompt(&mails, true, 20).unwrap();
        assert!(!prompt.contains("private-store"));
        assert!(!prompt.contains("demo-mail"));
    }
}
