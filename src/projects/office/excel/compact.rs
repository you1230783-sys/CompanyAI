//! 僅用於交付模型的 Excel 精簡格式；不改變 COM／資料集使用的 Page 序列化契約。
use super::Page;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

pub const FORMAT: &str = "excel-compact-v1";

/// 省略文字時唯一允許的還原規則。數值不能只比較 parse 後相等，否則會丟失 1.00 等顯示格式。
fn default_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        Value::Bool(value) => if *value { "TRUE" } else { "FALSE" }.into(),
        Value::Number(number) => {
            if number.is_f64() {
                number.as_f64().expect("JSON 有限數值").to_string()
            } else {
                number.to_string()
            }
        }
        _ => value.to_string(),
    }
}

/// 欄預設由本頁資料格選出；次數相同取字典序最小者，確保相同頁產生相同結果。
fn most_common<'a>(values: impl Iterator<Item = &'a str>, fallback: &str) -> String {
    let mut counts = BTreeMap::new();
    for value in values {
        *counts.entry(value).or_insert(0usize) += 1;
    }
    let mut best = (fallback, 0);
    for (value, count) in counts {
        if count > best.1 {
            best = (value, count);
        }
    }
    best.0.to_owned()
}

/// rows 為 [原列號, 各欄 Value2]；欄共用 kind／number_format，差異與顯示文字、公式另列。
/// 表頭通常是一般文字；特殊表頭以完整 Cell 保存並標記 header，避免與同列資料格混淆。
pub fn compact(page: &Page) -> Value {
    let mut kinds = Map::new();
    let mut formats = Map::new();
    for (index, column) in page.columns.iter().enumerate() {
        let cells = || page.rows.iter().filter_map(|row| row.cells.get(index));
        kinds.insert(
            column.clone(),
            json!(most_common(cells().map(|c| c.kind.as_str()), "blank")),
        );
        formats.insert(
            column.clone(),
            json!(most_common(
                cells().map(|c| c.number_format.as_str()),
                "General"
            )),
        );
    }
    let mut exceptions = Vec::new();
    let mut cell_texts = Vec::new();
    let mut formulas = Vec::new();
    let mut rows = Vec::new();
    for row in &page.rows {
        let mut values = vec![json!(row.row)];
        for (column, cell) in page.columns.iter().zip(&row.cells) {
            values.push(cell.value.clone());
            let mut exception = json!({"row":row.row,"column":column});
            if kinds[column] != cell.kind {
                exception["kind"] = json!(cell.kind);
            }
            if formats[column] != cell.number_format {
                exception["number_format"] = json!(cell.number_format);
            }
            if exception.as_object().expect("固定物件").len() > 2 {
                exceptions.push(exception);
            }
            if cell.text != default_text(&cell.value) {
                cell_texts.push(json!({"row":row.row,"column":column,"text":cell.text}));
            }
            if let Some(formula) = &cell.formula {
                formulas.push(json!({"row":row.row,"column":column,"formula":formula}));
            }
        }
        rows.push(values);
    }
    let mut headers = Vec::new();
    for (column, cell) in page.columns.iter().zip(&page.headers) {
        headers.push(cell.text.clone());
        if cell.value != Value::String(cell.text.clone())
            || cell.kind != "text"
            || cell.number_format != "General"
            || cell.formula.is_some()
        {
            exceptions
                .push(json!({"row":page.header_row,"column":column,"header":true,"cell":cell}));
        }
    }
    let row_fields: Vec<_> = std::iter::once("row")
        .chain(page.columns.iter().map(String::as_str))
        .collect();
    json!({
        "format":FORMAT,"sheet":page.sheet,"sheet_name":page.sheet_name,
        "columns":page.columns,"header_row":page.header_row,"headers":headers,
        "column_kinds":kinds,"column_formats":formats,"row_fields":row_fields,"rows":rows,
        "cell_texts":cell_texts,"exceptions":exceptions,"formulas":formulas,
        "text_rule":"未列 cell_texts 的資料格：null 為空字串、字串照原值、布林為 TRUE/FALSE、數字為最短十進位表示（不補尾端0）。headers 為顯示文字；未列 header:true 例外的表頭 Cell 為同值文字／text／General／無公式。header:true 的 cell 保存完整表頭，其他 exceptions 覆寫欄預設。",
        "start_row":page.start_row,"row_count":page.rows.len(),"next_row":page.next_row,
        "used_range":page.used_range,"date_1904":page.date_1904
    })
}

#[cfg(test)]
mod tests;
