//! Outlook Table 每次複製100列到本機，避免每封信逐屬性往返 COM。
//! 日期以 Outlook 內建屬性名稱取得本機時間；不使用地址簿、Message-ID 或正文。
use super::*;
use chrono::{NaiveDate, NaiveDateTime};
use std::sync::atomic::AtomicBool;
use windows::Win32::System::Ole::{
    SafeArrayGetDim, SafeArrayGetElement, SafeArrayGetLBound, SafeArrayGetUBound,
};

const COMMON: &[&str] = &[
    "EntryID",
    "MessageClass",
    "Subject",
    "SentOn",
    "LastModificationTime",
];
const VISIBLE: &[&str] = &["ReceivedTime", "SenderName", "To", "CC", "ConversationID"];

#[derive(Clone)]
pub(super) struct Row {
    pub entry: String,
    pub subject: String,
    pub sent: NaiveDateTime,
    pub modified: String,
    pub received: String,
    pub sender: String,
    pub recipients: Vec<String>,
    pub conversation: String,
}
impl Row {
    pub fn key(&self) -> String {
        key(&self.subject, self.sent)
    }
}

/// JSON二元組避免主旨中的分隔字元造成歧義；秒以下差異不影響副本判定。
pub(super) fn key(subject: &str, sent: NaiveDateTime) -> String {
    crate::projects::text::revision(
        &serde_json::json!([subject, sent.format("%Y-%m-%d %H:%M:%S").to_string()]).to_string(),
    )
}

fn string(value: &VARIANT) -> AppResult<String> {
    if matches!(
        unsafe { value.Anonymous.Anonymous.vt },
        VT_EMPTY | VT_NULL | VT_ERROR
    ) {
        return Err("Outlook 隱藏郵件識別欄位不存在或無法載入。".into());
    }
    let mut result = VARIANT::default();
    unsafe { VariantChangeType(&mut result, value, VAR_CHANGE_FLAGS(0), VT_BSTR) }
        .map_err(|_| "Outlook 隱藏副本索引的文字欄位不可讀。")?;
    Ok(unsafe { result.Anonymous.Anonymous.Anonymous.bstrVal.to_string() })
}

/// GetArray的第一維是欄，第二維是列。VARIANT擁有SAFEARRAY，借用期间不自行釋放。
fn cells(value: &VARIANT, width: usize) -> AppResult<Vec<Vec<VARIANT>>> {
    let vt = unsafe { value.Anonymous.Anonymous.vt };
    if vt == VT_EMPTY {
        return Ok(vec![]);
    }
    if vt.0 != VT_ARRAY.0 | VT_VARIANT.0 {
        return Err("Outlook Table陣列型別不符。".into());
    }
    let array = unsafe { value.Anonymous.Anonymous.Anonymous.parray };
    if array.is_null() {
        return Err("Outlook Table陣列維度不符。".into());
    }
    let bounds = |dim| unsafe {
        Ok::<_, String>((
            SafeArrayGetLBound(array, dim).map_err(|e| e.to_string())?,
            SafeArrayGetUBound(array, dim).map_err(|e| e.to_string())?,
        ))
    };
    let dimensions = unsafe { SafeArrayGetDim(array) };
    if dimensions == 1 {
        let (first, last) = bounds(1)?;
        if last < first {
            return Ok(vec![]);
        }
    }
    if dimensions != 2 {
        return Err("Outlook Table陣列維度不符。".into());
    }
    let (col, last_col) = bounds(1)?;
    let (row, last_row) = bounds(2)?;
    if last_row < row {
        return Ok(vec![]);
    }
    if last_col - col + 1 != width as i32 || last_row - row + 1 > 100 {
        return Err("Outlook Table批次大小不符。".into());
    }
    let mut rows = Vec::new();
    for j in row..=last_row {
        let mut values = Vec::new();
        for i in col..=last_col {
            let mut cell = VARIANT::default();
            unsafe {
                SafeArrayGetElement(array, [i, j].as_ptr(), (&mut cell as *mut VARIANT).cast())
            }
            .map_err(|e| e.to_string())?;
            values.push(cell);
        }
        rows.push(values);
    }
    Ok(rows)
}

pub(super) fn scan(
    ns: &IDispatch,
    folder: &IDispatch,
    field: &str,
    range: Option<(NaiveDate, NaiveDate)>,
    visible: bool,
    cancel: &AtomicBool,
) -> AppResult<Vec<Row>> {
    batch::check_cancel(cancel)?;
    let filter = range
        .map(|(start, end)| -> AppResult<String> {
            Ok(format!(
                "[{field}] >= '{}' AND [{field}] < '{}'",
                project::jet_day(start)?,
                project::jet_day(end.succ_opt().ok_or("日期範圍超出上限。")?)?
            ))
        })
        .transpose()?;
    let (table, filtered) = if let Some(filter) = filter {
        match get(folder, "GetTable", &mut [filter.as_str().into()]).and_then(|v| object(&v)) {
            Ok(table) => (table, true),
            // 部分資料提供者不支援Restrict，但仍可由排序後的批次資料本機篩選。
            Err(_) => (object(&get(folder, "GetTable", &mut [])?)?, false),
        }
    } else {
        (object(&get(folder, "GetTable", &mut [])?)?, false)
    };
    let columns = object(&get(&table, "Columns", &mut [])?)?;
    get(&columns, "RemoveAll", &mut [])?;
    for name in COMMON
        .iter()
        .chain(if visible { VISIBLE.iter() } else { [].iter() })
    {
        get(&columns, "Add", &mut [(*name).into()])?;
    }
    // 隱藏範圍只依寄送時間；可見收件資料依ReceivedTime排序。
    get(&table, "Sort", &mut [true.into(), field.into()])?;
    let count = i32::try_from(&get(&table, "GetRowCount", &mut [])?)
        .map_err(|_| "Outlook Table數量不可讀。")?;
    let mut result = Vec::new();
    let mut scanned = 0usize;
    let mut previous = None;
    let store = text(folder, "StoreID", 4096)?;
    loop {
        batch::check_cancel(cancel)?;
        let block = cells(
            &get(&table, "GetArray", &mut [100i32.into()])?,
            if visible { 10 } else { 5 },
        )?;
        if block.is_empty() {
            break;
        }
        let mut before_start = false;
        for row in block {
            scanned += 1;
            if scanned > 200_000 {
                return Err(
                    "Outlook 隱藏副本索引超過20萬列，未開放此批資料；請縮小日期範圍。".into(),
                );
            }
            let class = string(&row[1])?;
            if class != "IPM.Note" && !class.starts_with("IPM.Note.") {
                continue;
            }
            let sent = project::variant_time(&row[3])?;
            let date = if field == "ReceivedTime" {
                project::variant_time(&row[5])?
            } else {
                sent
            };
            if !filtered && previous.is_some_and(|previous| date > previous) {
                return Err("Outlook 隱藏副本索引時間排序不穩定，請重新查詢。".into());
            }
            previous = Some(date);
            if let Some((start, end)) = range {
                if date.date() < start {
                    before_start = true;
                    continue;
                }
                if date.date() > end {
                    continue;
                }
            }
            let entry = string(&row[0])?;
            let mut subject = string(&row[2])?;
            // Table字串可能截短；長主旨僅定向取原屬性，絕不拿截短內容作指紋。
            if subject.encode_utf16().count() >= 120 {
                let item = object(&get(
                    ns,
                    "GetItemFromID",
                    &mut [store.as_str().into(), entry.as_str().into()],
                )?)?;
                subject = text(&item, "Subject", 32768)?;
            }
            let modified = project::variant_time(&row[4])?
                .format("%Y-%m-%d %H:%M:%S%.3f")
                .to_string();
            result.push(Row {
                entry,
                subject,
                sent,
                modified,
                received: if visible {
                    project::variant_time(&row[5])?
                        .format("%Y-%m-%d %H:%M:%S%.3f")
                        .to_string()
                } else {
                    String::new()
                },
                sender: if visible {
                    string(&row[6]).unwrap_or_default()
                } else {
                    String::new()
                },
                recipients: if visible {
                    ["to", "cc"]
                        .iter()
                        .enumerate()
                        .filter_map(|(i, role)| {
                            string(&row[7 + i])
                                .ok()
                                .filter(|v| !v.is_empty())
                                .map(|v| format!("{role}:{v}"))
                        })
                        .collect()
                } else {
                    vec![]
                },
                conversation: if visible {
                    string(&row[9]).unwrap_or_default()
                } else {
                    String::new()
                },
            });
        }
        super::process::notify(&format!(
            "正在處理Outlook信件… 本資料夾已掃描{scanned}個項目{}",
            if filtered && count > 0 {
                format!("（{}%）", (scanned * 100 / count as usize).min(100))
            } else {
                String::new()
            }
        ));
        if before_start {
            break;
        }
    }
    if i32::try_from(&get(&table, "GetRowCount", &mut [])?).ok() != Some(count) {
        return Err("Outlook 隱藏副本索引期間資料夾已變更，未開放部分結果。".into());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_preserves_subject_and_compares_whole_seconds() {
        let date = NaiveDate::from_ymd_opt(2026, 9, 30)
            .unwrap()
            .and_hms_milli_opt(23, 59, 59, 100)
            .unwrap();
        assert_eq!(
            key("主題", date),
            key("主題", date + chrono::Duration::milliseconds(700))
        );
        assert_ne!(key("主題", date), key("RE: 主題", date));
        assert_ne!(
            key("主題", date),
            key("主題", date + chrono::Duration::seconds(1))
        );
    }
}
