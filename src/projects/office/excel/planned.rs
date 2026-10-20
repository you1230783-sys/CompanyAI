//! 已鎖定欄位的本機篩選；只掃時間欄，符合的列才讀 X/Y，不假定時間已排序。
use super::*;
use crate::projects::excel_plan::{Plan, Window};
use std::time::{Duration, Instant};

pub fn read(
    path: &Path,
    plan: &Plan,
    start: usize,
    scan_rows: usize,
    window: Option<&Window>,
    cancel: &AtomicBool,
) -> AppResult<(Page, usize)> {
    check_row(start)?;
    let proposal = &plan.proposal;
    if start <= proposal.header_row
        || !(1..=250_000).contains(&scan_rows)
        || start
            .checked_add(scan_rows - 1)
            .is_none_or(|end| end > 1_048_576)
    {
        return Err("規劃讀取需從表頭後開始，每次最多掃描250000列。".into());
    }
    if let Some(window) = window {
        window.bounds(&proposal.time_mode)?;
    }
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() }.map_err(|e| e.to_string())?;
    let _apartment = Apartment;
    let session = session(path)?;
    let sheet = worksheet(&session, proposal.sheet)?;
    let used_range = bounds(&sheet)?;
    let mut headers = Vec::new();
    for (index, column) in plan.columns.iter().enumerate() {
        let header = cell(&sheet, column, proposal.header_row)?;
        if plan.headers.get(index) != Some(&header.text) {
            return Err("Excel 表頭與規劃不同，請重新規劃。".into());
        }
        headers.push(header);
    }
    let time_index = proposal
        .time
        .as_ref()
        .and_then(|f| plan.columns.iter().position(|c| c == &f.column));
    if window.is_some() && time_index.is_none() {
        return Err("此規劃沒有時間篩選欄。".into());
    }
    let end = (start + scan_rows - 1).min(used_range.last_row);
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut rows = Vec::new();
    let mut scanned = 0;
    let mut bytes = 0;
    for row in start..=end {
        check_cancel(cancel)?;
        if Instant::now() >= deadline {
            return Err(
                "Excel 本機時間掃描已達120秒，未發布部分資料；請縮小掃描列範圍後分批。".into(),
            );
        }
        scanned += 1;
        if let (Some(window), Some(index)) = (window, time_index) {
            // 比較只需要 Value2，不讀取不相關欄位的文字、格式或公式。
            let address = format!("{}{row}", plan.columns[index]);
            let target = obj(invoke(
                &sheet,
                "Range",
                vec![address.as_str().into()],
                false,
            )?)?;
            let value = get(&target, "Value2")?;
            let (value, kind) = match unsafe { value.Anonymous.Anonymous.vt } {
                VT_R8 | VT_I4 | VT_I2 => (
                    json!(f64::try_from(&value).map_err(|_| "時間數值讀取失敗。")?),
                    "number",
                ),
                VT_BSTR => (json!(string(&value)?), "text"),
                _ => return Err(format!("時間欄 {address} 含空白或錯誤；未自動忽略資料。")),
            };
            let time = Cell {
                value,
                kind: kind.into(),
                text: String::new(),
                formula: None,
                number_format: String::new(),
            };
            if !window
                .contains(&time, &proposal.time_mode)
                .map_err(|e| format!("{address}：{e}"))?
            {
                continue;
            }
        }
        if rows.len() >= crate::projects::charts::MAX_POINTS {
            return Err("時間篩選符合超過10000筆，請縮小時間段；未抽樣或發布部分CSV。".into());
        }
        let mut cells = Vec::new();
        for column in &plan.columns {
            check_cancel(cancel)?;
            let value = cell(&sheet, column, row)?;
            if proposal.y_kind == "measurement"
                && proposal.y.iter().any(|y| &y.column == column)
                && crate::projects::excel_plan::temporal(&value)
            {
                return Err(format!(
                    "{column}{row} 是日期或時間，與已鎖定的量測值Y用途不符；未發布。"
                ));
            }
            account_cell_limit(&value, &mut bytes, 32 * 1024 * 1024)?;
            cells.push(value);
        }
        rows.push(Row { row, cells });
    }
    if rows.is_empty() {
        return Err("指定掃描範圍與時間區間沒有符合資料；未建立空CSV。".into());
    }
    let date_1904 = bool::try_from(&get(session.document()?, "Date1904")?)
        .map_err(|_| "Excel 日期系統無效。")?;
    if date_1904 != plan.date_1904 {
        return Err("Excel 日期系統與規劃不同。".into());
    }
    Ok((
        Page {
            sheet: proposal.sheet,
            sheet_name: string(&get(&sheet, "Name")?)?,
            columns: plan.columns.clone(),
            header_row: proposal.header_row,
            headers,
            start_row: start,
            rows,
            next_row: (end < used_range.last_row).then_some(end + 1),
            used_range,
            date_1904,
        },
        scanned,
    ))
}
