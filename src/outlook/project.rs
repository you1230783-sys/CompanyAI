//! 專案郵件讀取端。僅連接使用者已開啟的 Classic Outlook，不新增資料檔或同步設定。
//! COM 物件全在單次呼叫的 STA 執行緒內建立及釋放；代號由桌面保存，不交給模型。
use super::*;
use crate::projects::mail::{self, Folder, Header, Scan, Source};
use chrono::{Duration, NaiveDate, NaiveDateTime};
use std::{
    path::Path,
    sync::atomic::AtomicBool,
    time::{Duration as StdDuration, Instant},
};

pub struct Reader;

fn count(items: &IDispatch) -> AppResult<i32> {
    i32::try_from(&get(items, "Count", &mut [])?).map_err(|_| "無法取得 Outlook 項目數量。".into())
}
fn item(items: &IDispatch, index: i32) -> AppResult<IDispatch> {
    object(&get(items, "Item", &mut [VARIANT::from(index)])?)
}
fn namespace(app: &IDispatch) -> AppResult<IDispatch> {
    object(&get(app, "GetNamespace", &mut [VARIANT::from("MAPI")])?)
}
fn time(item: &IDispatch, name: &str) -> AppResult<NaiveDateTime> {
    let raw = get(item, name, &mut [])?;
    let mut converted = VARIANT::default();
    unsafe { VariantChangeType(&mut converted, &raw, VAR_CHANGE_FLAGS(0), VT_DATE) }
        .map_err(|_| "無法讀取郵件時間。")?;
    let days = unsafe { converted.Anonymous.Anonymous.Anonymous.date };
    if !days.is_finite() || !(1.0..2_000_000.0).contains(&days) {
        return Err("郵件時間超出範圍。".into());
    }
    NaiveDate::from_ymd_opt(1899, 12, 30)
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .and_then(|base| {
            base.checked_add_signed(Duration::milliseconds((days * 86_400_000.0).round() as i64))
        })
        .ok_or("郵件時間無法換算。".into())
}
pub(super) fn timestamp(item: &IDispatch, name: &str) -> AppResult<String> {
    Ok(time(item, name)?
        .format("%Y-%m-%d %H:%M:%S%.3f")
        .to_string())
}
/// Jet 使用本機日期格式；OLE 的 DATE→BSTR 轉換等同 General Date，午夜只產生日期。
/// 不硬寫美式日期，也不在查詢字串加入 Outlook 不支援的秒數。
fn jet_day(day: NaiveDate) -> AppResult<String> {
    let base = NaiveDate::from_ymd_opt(1899, 12, 30).ok_or("日期基準不正確。")?;
    let raw = VARIANT::from((day - base).num_days() as f64);
    let mut date = VARIANT::default();
    let mut formatted = VARIANT::default();
    unsafe {
        VariantChangeType(&mut date, &raw, VAR_CHANGE_FLAGS(0), VT_DATE)
            .and_then(|()| VariantChangeType(&mut formatted, &date, VAR_CHANGE_FLAGS(0), VT_BSTR))
    }
    .map_err(|_| "無法依 Windows 地區格式建立 Outlook 日期條件。")?;
    let value = unsafe { formatted.Anonymous.Anonymous.Anonymous.bstrVal.to_string() };
    Ok(value.replace('\'', "''"))
}
pub(super) fn property(item: &IDispatch, tag: &str) -> AppResult<String> {
    let accessor = object(&get(item, "PropertyAccessor", &mut [])?)?;
    let value = get(&accessor, "GetProperty", &mut [VARIANT::from(tag)])?;
    let mut converted = VARIANT::default();
    unsafe { VariantChangeType(&mut converted, &value, VAR_CHANGE_FLAGS(0), VT_BSTR) }
        .map_err(|_| "郵件地址屬性無法轉成文字。")?;
    let value = unsafe { converted.Anonymous.Anonymous.Anonymous.bstrVal.to_string() };
    if value.len() > 4096 {
        return Err("郵件地址過長。".into());
    }
    Ok(value)
}
/// 優先 SMTP 地址；無法解析時保留 Outlook 原地址，但去重層不猜測相同身分。
pub(super) fn sender(item: &IDispatch) -> AppResult<String> {
    property(item, "http://schemas.microsoft.com/mapi/proptag/0x5D01001F")
        .or_else(|_| text(item, "SenderEmailAddress", 4096))
}
pub(super) fn recipients(item: &IDispatch) -> AppResult<Vec<String>> {
    let recipients = object(&get(item, "Recipients", &mut [])?)?;
    let total = count(&recipients)?;
    if total > 200 {
        return Ok(vec!["收件者超過 200 個，未展開；此封不作地址去重。".into()]);
    }
    let mut result = Vec::new();
    for index in 1..=total {
        let recipient = self::item(&recipients, index)?;
        let address = property(
            &recipient,
            "http://schemas.microsoft.com/mapi/proptag/0x39FE001E",
        )
        .or_else(|_| text(&recipient, "Address", 4096))?;
        let kind =
            i32::try_from(&get(&recipient, "Type", &mut [])?).map_err(|_| "收件者類型不正確。")?;
        let role = match kind {
            1 => "to",
            2 => "cc",
            3 => "bcc",
            _ => "other",
        };
        result.push(format!("{role}:{address}"));
        if result.iter().map(String::len).sum::<usize>() > 12_000 {
            return Ok(vec!["收件地址超過 12 KB，未展開；此封不作地址去重。".into()]);
        }
    }
    Ok(result)
}
fn folder(ns: &IDispatch, reference: &Folder) -> AppResult<IDispatch> {
    let value = object(&get(
        ns,
        "GetFolderFromID",
        &mut [
            VARIANT::from(reference.store.as_str()),
            VARIANT::from(reference.entry.as_str()),
        ],
    )?)?;
    if text(&value, "EntryID", 4096)? != reference.entry
        || text(&value, "StoreID", 4096)? != reference.store
    {
        return Err("Outlook 資料夾身分已改變。".into());
    }
    if !privacy::folder_allowed(&privacy::Policy::current()?, &value)? {
        return Err("資料夾不在已確認的 Outlook 範圍。".into());
    }
    Ok(value)
}
fn snapshot_folder(
    value: &IDispatch,
    scope: &str,
    excluded: &[String],
    readable: bool,
) -> AppResult<Folder> {
    let store = text(value, "StoreID", 4096)?;
    let entry = text(value, "EntryID", 4096)?;
    let id = crate::projects::text::revision(&format!("{scope}\n{store}\n{entry}"));
    let folders = object(&get(value, "Folders", &mut [])?)?;
    let mail_folder = i32::try_from(&get(value, "DefaultItemType", &mut [])?).ok() == Some(0);
    Ok(Folder {
        id,
        name: text(value, "Name", 1000)?,
        path: text(value, "FolderPath", 4096)?,
        scope: scope.into(),
        store: store.clone(),
        entry,
        children: {
            let policy = privacy::Policy::current()?;
            let mut visible = 0;
            for i in 1..=count(&folders)? {
                let child = item(&folders, i)?;
                if policy.permits(&store, &text(&child, "EntryID", 4096)?) {
                    visible += 1;
                }
            }
            visible
        },
        readable: readable && mail_folder,
        excluded: excluded.to_vec(),
    })
}
fn header(item: &IDispatch, folder: &Folder) -> AppResult<Header> {
    let sent_at = timestamp(item, "SentOn")?;
    let sender = sender(item)?;
    let recipients = recipients(item)?;
    let duplicate_key = mail::duplicate_key(&sent_at, &sender, &recipients);
    let conversation = text(item, "ConversationID", 4096).unwrap_or_default();
    Ok(Header {
        thread_id: if conversation.is_empty() {
            String::new()
        } else {
            crate::projects::text::revision(&format!("{}\n{conversation}", folder.store))
        },
        id: crate::jobs::new_id()?,
        folder_id: folder.id.clone(),
        subject: text(item, "Subject", 3000)?,
        sender,
        recipients,
        sent_at,
        received_at: timestamp(item, "ReceivedTime")?,
        entry: text(item, "EntryID", 4096)?,
        modified: timestamp(item, "LastModificationTime")?,
        duplicate_key,
    })
}
fn check(cancel: &AtomicBool, started: Instant) -> AppResult<()> {
    batch::check_cancel(cancel)?;
    if started.elapsed() >= StdDuration::from_secs(30) {
        return Err("Outlook 單次查詢已達 30 秒，結果不完整；請縮小資料夾或日期範圍。".into());
    }
    Ok(())
}
/// 回傳經資料夾權限、身分與版本核對的郵件；呼叫期間 COM Apartment 必須仍有效。
fn checked_item(
    ns: &IDispatch,
    reference: &Folder,
    snapshot: &Header,
    cancel: &AtomicBool,
) -> AppResult<IDispatch> {
    let item = object(&get(
        ns,
        "GetItemFromID",
        &mut [
            VARIANT::from(reference.store.as_str()),
            VARIANT::from(snapshot.entry.as_str()),
        ],
    )?)?;
    let parent = object(&get(&item, "Parent", &mut [])?)?;
    if text(&parent, "EntryID", 4096)? != reference.entry
        || text(&parent, "StoreID", 4096)? != reference.store
        || i32::try_from(&get(&item, "Class", &mut [])?).ok() != Some(43)
    {
        return Err("郵件已移動或身分改變，請重新列出標題。".into());
    }
    privacy::require_item(&privacy::Policy::current()?, &item)?;
    exclusions::Exclusions::from_namespace(ns, &privacy::Policy::current()?, cancel)?
        .require(&item)?;
    let current = header(&item, reference)?;
    if current.entry != snapshot.entry
        || current.modified != snapshot.modified
        || current.subject != snapshot.subject
        || current.sender != snapshot.sender
        || current.recipients != snapshot.recipients
        || current.sent_at != snapshot.sent_at
    {
        return Err("郵件在標題預覽後已變更，未讀取內文；請重新列出。".into());
    }
    Ok(item)
}
impl Source for Reader {
    fn verify(&mut self, folder: &Folder, header: &Header, cancel: &AtomicBool) -> AppResult<()> {
        batch::check_cancel(cancel)?;
        let (_apartment, app) = batch::connect()?;
        checked_item(&namespace(&app)?, folder, header, cancel)?;
        Ok(())
    }

    fn folders(
        &mut self,
        scope: &str,
        parent: Option<&Folder>,
        cancel: &AtomicBool,
    ) -> AppResult<(Vec<Folder>, Vec<String>)> {
        batch::check_cancel(cancel)?;
        let (_apartment, app) = batch::connect()?;
        folders_from_app(&app, scope, parent, cancel)
    }

    fn headers(
        &mut self,
        reference: &Folder,
        start: NaiveDate,
        end: NaiveDate,
        cancel: &AtomicBool,
    ) -> AppResult<Scan> {
        batch::check_cancel(cancel)?;
        let (_apartment, app) = batch::connect()?;
        let ns = namespace(&app)?;
        let folder = folder(&ns, reference)?;
        let hidden =
            exclusions::Exclusions::from_namespace(&ns, &privacy::Policy::current()?, cancel)?;
        let all_items = object(&get(&folder, "Items", &mut [])?)?;
        let field = if reference.scope == "online_sent" {
            "SentOn"
        } else {
            "ReceivedTime"
        };
        let after = end.succ_opt().ok_or("日期範圍超出上限。")?;
        let filter = format!(
            "[{field}] >= '{}' AND [{field}] < '{}'",
            jet_day(start)?,
            jet_day(after)?
        );
        let items = object(&get(
            &all_items,
            "Restrict",
            &mut [VARIANT::from(filter.as_str())],
        )?)?;
        get(
            &items,
            "Sort",
            &mut [
                VARIANT::from(true),
                VARIANT::from(format!("[{field}]").as_str()),
            ],
        )?;
        let started = Instant::now();
        let total = count(&items)?;
        let mut scan = Scan {
            notices: hidden.notices(),
            complete: true,
            ..Default::default()
        };
        for index in 1..=total {
            batch::check_cancel(cancel)?;
            if index > 10_000
                || scan.headers.len() >= 2000
                || started.elapsed() >= StdDuration::from_secs(30)
            {
                scan.complete = false;
                scan.notices.push(
                    "達到 10000 個項目、2000 封郵件或 30 秒上限，未掃完；請縮小日期範圍。".into(),
                );
                break;
            }
            let candidate = (|| -> AppResult<(bool, Option<Header>)> {
                let item = item(&items, index)?;
                if i32::try_from(&get(&item, "Class", &mut [])?).ok() != Some(43) {
                    return Ok((false, None));
                }
                let day = time(&item, field)?.date();
                // Restrict 後仍以原始 OLE DATE 核對日期，避免地區格式造成範圍誤判。
                if day < start {
                    return Ok((true, None));
                }
                if day > end {
                    return Ok((false, None));
                }
                privacy::require_item(&privacy::Policy::current()?, &item)?;
                if hidden.contains(&item)? {
                    return Ok((false, None));
                }
                Ok((false, Some(header(&item, reference)?)))
            })();
            match candidate {
                Ok((true, _)) => break,
                Ok((false, Some(mail))) => scan.headers.push(mail),
                Ok((false, None)) => (),
                Err(error) => {
                    scan.complete = false;
                    if scan.notices.len() < 5 {
                        scan.notices.push(error);
                    }
                }
            }
        }
        if count(&items)? != total {
            scan.complete = false;
            scan.notices
                .push("查詢期間郵件數量改變，請重新查詢以確認完整性。".into());
        }
        Ok(scan)
    }
    fn body(
        &mut self,
        reference: &Folder,
        snapshot: &Header,
        cancel: &AtomicBool,
    ) -> AppResult<String> {
        batch::check_cancel(cancel)?;
        let (_apartment, app) = batch::connect()?;
        let ns = namespace(&app)?;
        let item = checked_item(&ns, reference, snapshot, cancel)?;
        let body = text(&item, "Body", 256_000)?;
        if timestamp(&item, "LastModificationTime")? != snapshot.modified {
            return Err("郵件在讀取內文期間改變，此次內容未使用。".into());
        }
        batch::check_cancel(cancel)?;
        Ok(body)
    }
}

/// 列夾核心只依已連線 Outlook 物件工作；測試可用 IDispatch fixture 驗證。
pub(crate) fn folders_from_app(
    app: &IDispatch,
    scope: &str,
    parent: Option<&Folder>,
    cancel: &AtomicBool,
) -> AppResult<(Vec<Folder>, Vec<String>)> {
    let ns = namespace(app)?;
    let started = Instant::now();
    let mut result = Vec::new();
    let mut notices = Vec::new();
    if let Some(parent) = parent {
        let current = folder(&ns, parent)?;
        let children = object(&get(&current, "Folders", &mut [])?)?;
        let total = count(&children)?;
        if total > 10_000 {
            return Err("單層 Outlook 資料夾超過 10000 個，請整理後再讀取。".into());
        }
        for index in 1..=total {
            check(cancel, started)?;
            let child = item(&children, index)?;
            if parent.excluded.contains(&text(&child, "EntryID", 4096)?) {
                continue;
            }
            if privacy::folder_allowed(&privacy::Policy::current()?, &child)? {
                result.push(snapshot_folder(&child, scope, &parent.excluded, true)?);
            }
        }
    } else {
        let stores = object(&get(&ns, "Stores", &mut [])?)?;
        let total = count(&stores)?;
        if total > 100 {
            return Err("已載入 Outlook 資料檔超過 100 個，請先縮小使用中的範圍。".into());
        }
        for index in 1..=total {
            check(cancel, started)?;
            let store = item(&stores, index)?;
            let kind = i32::try_from(&get(&store, "ExchangeStoreType", &mut [])?)
                .map_err(|_| "無法確認 Outlook 信箱類型。")?;
            if scope == "local_inbox" {
                // FilePath 僅判斷已載入 Store 的 PST 類型；不掃描或開啟磁碟上的其他檔案。
                if kind != 3 {
                    continue;
                }
                let path = text(&store, "FilePath", 32768)?;
                if Path::new(&path)
                    .extension()
                    .and_then(|s| s.to_str())
                    .is_none_or(|s| !s.eq_ignore_ascii_case("pst"))
                {
                    continue;
                }
                let root = object(&get(&store, "GetRootFolder", &mut [])?)?;
                let mut excluded = Vec::new();
                for kind in [3i32, 4, 5, 16, 23, 20] {
                    if let Ok(value) = get(&store, "GetDefaultFolder", &mut [VARIANT::from(kind)]) {
                        if let Ok(value) = object(&value) {
                            excluded.push(text(&value, "EntryID", 4096)?);
                        }
                    }
                }
                // 規則可能把郵件放在 PST 根目錄下的平行資料夾，先列資料檔再讓 AI 選擇。
                if privacy::folder_allowed(&privacy::Policy::current()?, &root)? {
                    result.push(snapshot_folder(&root, scope, &excluded, false)?);
                }
            } else if matches!(kind, 0 | 1 | 4)
                || (kind == 3
                    && Path::new(&text(&store, "FilePath", 32768)?)
                        .extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|e| e.eq_ignore_ascii_case("ost")))
            {
                // olFolderInbox=6、olFolderSentMail=5；收件與寄件日期規則仍分開。
                let folder_kind = if scope == "online_inbox" { 6i32 } else { 5i32 };
                match get(
                    &store,
                    "GetDefaultFolder",
                    &mut [VARIANT::from(folder_kind)],
                )
                .and_then(|v| object(&v))
                {
                    Ok(sent) => {
                        if privacy::folder_allowed(&privacy::Policy::current()?, &sent)? {
                            result.push(snapshot_folder(&sent, scope, &[], true)?);
                        }
                    }
                    Err(_) => notices.push("部分資料檔沒有可讀取的指定預設郵件資料夾。".into()),
                }
            }
        }
        if result.is_empty() {
            notices.push(if scope=="local_inbox" {"沒有找到已載入的本地 PST；請在 Classic Outlook 開啟正確的資料檔，不會自動改讀線上收件匣。"}else if scope=="online_inbox" {"沒有找到 Exchange／OST 信箱的收件匣。"}else{"沒有找到 Exchange／OST 信箱的寄件備份；不會改讀本地寄件資料夾。"}.into());
        }
    }
    Ok((result, notices))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn jet_midnight_uses_windows_date_format_and_round_trips() {
        let day = NaiveDate::from_ymd_opt(2026, 6, 23).unwrap();
        let formatted = jet_day(day).unwrap();
        let source = VARIANT::from(formatted.as_str());
        let mut parsed = VARIANT::default();
        unsafe { VariantChangeType(&mut parsed, &source, VAR_CHANGE_FLAGS(0), VT_DATE) }.unwrap();
        let days = unsafe { parsed.Anonymous.Anonymous.Anonymous.date };
        assert_eq!(
            days,
            (day - NaiveDate::from_ymd_opt(1899, 12, 30).unwrap()).num_days() as f64
        );
    }
}
