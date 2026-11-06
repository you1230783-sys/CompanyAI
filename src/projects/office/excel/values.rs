//! 供模型查看的小量樣本。CSV、原生快照和完整模式繼續使用未四捨五入的 Value2。
use super::{Cell, Page};
use crate::projects::excel_plan::temporal_codes;
use chrono::{Days, NaiveDate, NaiveTime};
use serde_json::{json, Map, Value};

pub const FORMAT: &str = "excel-values-v1";
pub const SAMPLE_NOTICE: &str = "四捨五入的 Excel 樣本值，最終數值請匯出 CSV 後以 Python 計算。";
const MILLIS_PER_DAY: u64 = 86_400_000;

/// Excel 1900 系統包含不存在的 1900-02-29，不能把該序號當作真實日期。
fn excel_date(day: u64, date_1904: bool) -> Option<NaiveDate> {
    let (base, offset) = match (date_1904, day) {
        (true, _) => (NaiveDate::from_ymd_opt(1904, 1, 1)?, day),
        (false, 0 | 60) => return None,
        (false, 1..=59) => (NaiveDate::from_ymd_opt(1900, 1, 1)?, day - 1),
        (false, _) => (NaiveDate::from_ymd_opt(1900, 1, 1)?, day - 2),
    };
    base.checked_add_days(Days::new(offset))
        .filter(|date| *date <= NaiveDate::from_ymd_opt(9999, 12, 31).expect("有效日期"))
}

/// 純時鐘只顯示一天內的時間；經過時間等有不同語意的格式保留 Excel 顯示文字。
fn iso_temporal(serial: f64, date_1904: bool, date: bool, time: bool) -> Option<String> {
    if !(0.0..=2_958_466.0).contains(&serial) || (!date && serial >= 1.0) {
        return None;
    }
    let millis = (serial * MILLIS_PER_DAY as f64).round() as u64;
    let day = if time {
        millis / MILLIS_PER_DAY
    } else {
        serial.floor() as u64
    };
    let millis_of_day = millis % MILLIS_PER_DAY;
    let clock = NaiveTime::from_num_seconds_from_midnight_opt(
        (millis_of_day / 1000) as u32,
        (millis_of_day % 1000 * 1_000_000) as u32,
    )?;
    match (date, time) {
        (true, true) => Some(format!("{} {clock}", excel_date(day, date_1904)?)),
        (true, false) => Some(excel_date(day, date_1904)?.to_string()),
        (false, true) if day == 0 => Some(clock.to_string()),
        _ => None,
    }
}

/// 回傳樣本值、欄型別及日期是否改用 Excel 原始顯示文字。
fn sample(cell: &Cell, date_1904: bool) -> (Value, &'static str, bool) {
    match &cell.value {
        Value::Null if cell.kind == "error" || (cell.kind == "merged" && !cell.text.is_empty()) => {
            (json!(cell.text), "text", false)
        }
        Value::Null => (Value::Null, "blank", false),
        Value::Bool(_) => (cell.value.clone(), "boolean", false),
        Value::Number(number) => {
            let value = number.as_f64().expect("JSON 有限數值");
            let (codes, elapsed) = temporal_codes(&cell.number_format);
            let codes = codes.replace("am/pm", "h").replace("a/p", "h");
            let time = codes.contains(['h', 's']);
            let date = codes.contains(['y', 'd']) || (!time && codes.contains("mm"));
            if date || time || elapsed {
                let iso = (!elapsed)
                    .then(|| iso_temporal(value, date_1904, date, time))
                    .flatten();
                return match iso {
                    Some(iso) => (json!(iso), "datetime", false),
                    None => (json!(cell.text), "datetime", true),
                };
            }
            // 科學記號的一位整數與五位小數正好是六位有效數字，極小數也不必先縮放。
            let rounded = if value.fract() == 0.0 {
                cell.value.clone()
            } else {
                serde_json::from_str(&format!("{value:.5e}")).unwrap_or_else(|_| cell.value.clone())
            };
            (rounded, "number", false)
        }
        _ => (cell.value.clone(), "text", false),
    }
}

/// 每欄只以非空白資料判定型別，公式與日期轉換例外以原始列號與欄名定位。
pub fn values(page: &Page, include_formulas: bool) -> Value {
    let mut kinds = vec!["blank"; page.columns.len()];
    let mut formulas = Vec::new();
    let mut formula_count = 0;
    let mut temporal_fallbacks = Vec::new();
    let mut rows = Vec::with_capacity(page.rows.len());
    // 若表頭也在資料列範圍內，該公式會由下面的迴圈列出，避免重複計數。
    if !page.rows.iter().any(|row| row.row == page.header_row) {
        for (column, cell) in page.columns.iter().zip(&page.headers) {
            if let Some(formula) = &cell.formula {
                formula_count += 1;
                if include_formulas {
                    formulas.push(json!({"row":page.header_row,"column":column,"formula":formula}));
                }
            }
        }
    }
    for row in &page.rows {
        let mut values = vec![json!(row.row)];
        for (index, (column, cell)) in page.columns.iter().zip(&row.cells).enumerate() {
            let (value, kind, fallback) = sample(cell, page.date_1904);
            values.push(value);
            if kind != "blank" && kinds[index] != kind {
                kinds[index] = if kinds[index] == "blank" {
                    kind
                } else {
                    "mixed"
                };
            }
            if fallback {
                temporal_fallbacks.push(json!({"row":row.row,"column":column}));
            }
            if let Some(formula) = &cell.formula {
                formula_count += 1;
                if include_formulas {
                    formulas.push(json!({"row":row.row,"column":column,"formula":formula}));
                }
            }
        }
        rows.push(values);
    }
    let column_kinds: Map<String, Value> = page
        .columns
        .iter()
        .cloned()
        .zip(kinds.into_iter().map(|kind| json!(kind)))
        .collect();
    let headers: Vec<_> = page.headers.iter().map(|cell| &cell.text).collect();
    let mut result = json!({
        "format":FORMAT,"sheet":page.sheet,"sheet_name":page.sheet_name,
        "columns":page.columns,"header_row":page.header_row,"headers":headers,
        "column_kinds":column_kinds,"rows":rows,"formula_cells":formula_count,
        "numeric_precision":"6 significant digits; exact values via export_excel_dataset",
        "start_row":page.start_row,"row_count":page.rows.len(),"next_row":page.next_row,
        "used_range":page.used_range,"date_1904":page.date_1904
    });
    if include_formulas {
        result["formulas"] = json!(formulas);
    }
    if !temporal_fallbacks.is_empty() {
        result["temporal_fallbacks"] =
            json!({"reason":"無法可靠轉換，保留 Excel 顯示文字。","cells":temporal_fallbacks});
    }
    result
}
