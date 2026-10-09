//! 固定圖表資料契約；不接受 ECharts option、JavaScript 或任意 HTML。
use crate::AppResult;
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub mod png;
pub mod quality;
pub mod style;
pub mod transform;
pub const KINDS: &[&str] = &["line", "bar", "scatter", "step", "area", "horizontal_bar"];

pub const MAX_POINTS: usize = 10_000;
pub const MAX_SERIES: usize = 8;
pub const MAX_CHART_BYTES: usize = 8 * 1024 * 1024;
/// 桌面由原始 Excel 快照產生的處理明細，保留被排除列的來源位置。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataIssue {
    pub sheet: usize,
    pub row: usize,
    pub cell: String,
    pub series: String,
    pub x_value: String,
    pub y_value: String,
    pub original_value: Value,
    pub original_text: String,
    pub category: String,
    pub handling: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Series {
    pub name: String,
    pub values: Vec<Option<f64>>,
    /// 僅由使用者確認的資料處理產生；保留 X 位置，略過指定缺值點。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skip_indices: Vec<usize>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chart {
    pub kind: String,
    pub title: String,
    pub x_label: String,
    pub y_label: String,
    pub x: Vec<Value>,
    pub series: Vec<Series>,
    pub source: String,
    /// AI 的參考線與原始資料一併保存；使用者呈現設定可覆寫，PNG沿用同一份資料。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_lines: Vec<style::ReferenceLine>,
    /// AI與使用者共用的受控呈現設定；來源點陣保持不變。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<Box<style::Style>>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub data_note: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data_issues: Vec<DataIssue>,
    /// AI 透過受控工具設定的呈現轉換；原始 X／Y 與來源仍完整保存。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<Box<transform::Transform>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<quality::Source>,
}
impl Chart {
    pub fn validate(&self) -> AppResult<()> {
        if !KINDS.contains(&self.kind.as_str())
            || self.x.is_empty()
            || self.x.len() > MAX_POINTS
            || self.series.is_empty()
            || self.series.len() > MAX_SERIES
        {
            return Err(
                "圖表限 line/bar/scatter/step/area/horizontal_bar，1–10000 筆與 1–8 個系列。"
                    .into(),
            );
        }
        for label in [
            &self.title,
            &self.x_label,
            &self.y_label,
            &self.source,
            &self.data_note,
        ] {
            if label.chars().count() > 500 {
                return Err("圖表標籤超過 500 字。".into());
            }
        }
        for x in &self.x {
            if !(x.is_number() || x.as_str().is_some_and(|s| s.chars().count() <= 100)) {
                return Err("橫軸只接受短文字或數字；散佈圖必須使用數字。".into());
            }
        }
        for series in &self.series {
            if series.skip_indices.windows(2).any(|w| w[0] >= w[1])
                || series
                    .skip_indices
                    .iter()
                    .any(|i| series.values.get(*i) != Some(&None))
            {
                return Err("略過點必須是有效且不重複的缺值位置。".into());
            }
            if series.name.is_empty()
                || series.name.chars().count() > 100
                || series.values.len() != self.x.len()
                || series.values.iter().flatten().any(|v| !v.is_finite())
            {
                return Err("系列長度需等於橫軸筆數，數值需有限，缺值使用 null。".into());
            }
        }
        if serde_json::to_vec(self).map_err(|e| e.to_string())?.len() > MAX_CHART_BYTES {
            return Err("圖表資料超過 8 MiB，請縮短標籤或分圖；未自動抽樣。".into());
        }
        if let Some(source) = &self.quality {
            source.validate(self)?;
        }
        let transform = self.transform.clone().unwrap_or_default();
        let view = transform.view(self, &self.kind)?;
        style::validate_lines(&self.reference_lines, &self.kind, &transform, view.x.len())?;
        if let Some(style) = &self.style {
            style.validate_view(self)?;
        }
        Ok(())
    }
}
fn cell(value: &str) -> AppResult<(usize, usize)> {
    let value = value.replace('$', "").to_uppercase();
    let mut column = 0usize;
    let mut row = String::new();
    for c in value.chars() {
        if c.is_ascii_uppercase() && row.is_empty() {
            column = column
                .checked_mul(26)
                .and_then(|v| v.checked_add(c as usize - 'A' as usize + 1))
                .ok_or("欄位超出範圍。")?;
        } else if c.is_ascii_digit() {
            row.push(c);
        } else {
            return Err("範圍需為 A1:C20。".into());
        }
    }
    let row = row.parse::<usize>().map_err(|_| "列號無效。")?;
    if row == 0 || row > 1_048_576 || column == 0 || column > 16_384 {
        return Err("儲存格超出範圍。".into());
    }
    Ok((column, row))
}
/// 從已由原生 Office 讀取的快照取值，避免模型再次抄寫數字。第一列為系列標題。
#[allow(clippy::too_many_arguments)]
pub fn prepare_excel(
    content: &str,
    sheet: usize,
    range: &str,
    kind: &str,
    title: &str,
    x_label: &str,
    y_label: &str,
    path: &str,
    revision: &str,
) -> AppResult<quality::Prepared> {
    let snapshot: super::office::Snapshot =
        serde_json::from_str(content).map_err(|_| "需要 Excel 文件快照。")?;
    if !snapshot.scope.starts_with("Excel") || sheet == 0 {
        return Err("需要 Excel 文件與從 1 開始的工作表序號。".into());
    }
    let (first, last) = range
        .split_once(':')
        .ok_or("範圍需包含標題列，例如 A1:C20。")?;
    let (c1, r1) = cell(first)?;
    let (c2, r2) = cell(last)?;
    if c2 <= c1 || c2 - c1 > MAX_SERIES || r2 <= r1 || r2 - r1 > MAX_POINTS {
        return Err(
            "範圍需為 2–9 欄、2–10001 列（含標題）；大型 Excel 請使用 chart_excel_range。".into(),
        );
    }
    let prefix = format!("s{sheet}:");
    let mut cells = std::collections::BTreeMap::new();
    for block in &snapshot.blocks {
        if let Some(address) = block.id.strip_prefix(&prefix) {
            cells.insert(cell(address)?, block);
        }
    }
    use super::office::excel::{column_name, Bounds, Cell, Page, Row};
    let get = |c: usize, r: usize| {
        let block = cells.get(&(c, r));
        let text = block.map(|b| b.text.clone()).unwrap_or_default();
        let kind = block.map(|b| b.kind.as_str()).unwrap_or("blank");
        // 舊 Office 快照將 VT_EMPTY 表示成空白 text；保持既有缺值行為。
        let kind = if kind == "text" && text.trim().is_empty() {
            "blank"
        } else {
            kind
        };
        Cell {
            value: if kind == "number" {
                text.parse::<f64>()
                    .ok()
                    .map_or(Value::Null, |n| serde_json::json!(n))
            } else {
                Value::String(text.clone())
            },
            text,
            kind: kind.into(),
            formula: None,
            number_format: String::new(),
        }
    };
    let page = Page {
        sheet,
        sheet_name: String::new(),
        columns: (c1..=c2).map(column_name).collect(),
        header_row: r1,
        headers: (c1..=c2).map(|c| get(c, r1)).collect(),
        start_row: r1 + 1,
        rows: (r1 + 1..=r2)
            .map(|r| Row {
                row: r,
                cells: (c1..=c2).map(|c| get(c, r)).collect(),
            })
            .collect(),
        next_row: None,
        used_range: Bounds {
            first_row: r1,
            last_row: r2,
            first_column: column_name(c1),
            last_column: column_name(c2),
        },
        date_1904: false,
    };
    prepare_page(&page, kind, title, x_label, y_label, path, revision)
}
/// 直接使用選欄取得的 Value2；預檢完整範圍後才詢問使用者。
pub fn prepare_page(
    page: &super::office::excel::Page,
    kind: &str,
    title: &str,
    x_label: &str,
    y_label: &str,
    path: &str,
    revision: &str,
) -> AppResult<quality::Prepared> {
    quality::prepare(
        page,
        Chart {
            transform: None,
            quality: None,
            reference_lines: Vec::new(),
            style: None,
            kind: kind.into(),
            title: title.into(),
            x_label: x_label.into(),
            y_label: y_label.into(),
            x: vec![],
            series: vec![],
            data_note: String::new(),
            data_issues: vec![],
            source: format!(
                "{path} | {revision} | 工作表 {} | 欄 {} | 列 {}–{}",
                page.sheet,
                page.columns.join(","),
                page.start_row,
                page.rows.last().map_or(page.start_row, |r| r.row)
            ),
        },
    )
}
/// 無需使用者決策時供既有檢查呼叫；有異常值必須經正式 UI 確認。
pub fn from_page(
    page: &super::office::excel::Page,
    kind: &str,
    title: &str,
    x_label: &str,
    y_label: &str,
    path: &str,
    revision: &str,
) -> AppResult<Chart> {
    prepare_page(page, kind, title, x_label, y_label, path, revision)?.apply(&[])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_snapshot_empty_text_remains_a_gap() {
        let blocks=[("A1","X"),("B1","Y"),("A2","1"),("B2","339"),("A3","2"),("B3","")].iter().map(|(id,text)|serde_json::json!({"id":format!("s1:{id}"),"label":id,"kind":"text","text":text})).collect::<Vec<_>>();
        let content = serde_json::json!({"scope":"Excel","blocks":blocks}).to_string();
        let prepared = prepare_excel(
            &content,
            1,
            "A1:B3",
            "line",
            "t",
            "x",
            "y",
            "test.xlsx",
            "r",
        )
        .unwrap();
        assert!(prepared.review.groups.is_empty());
        let chart = prepared.apply(&[]).unwrap();
        assert_eq!(chart.series[0].values, vec![Some(339.0), None]);
    }
    #[test]
    fn ten_thousand_points_are_preserved_without_sampling() {
        let mut chart = Chart {
            transform: None,
            quality: None,
            reference_lines: Vec::new(),
            style: None,
            kind: "line".into(),
            title: "T".into(),
            x_label: "x".into(),
            y_label: "y".into(),
            x: (0..MAX_POINTS).map(|i| serde_json::json!(i)).collect(),
            series: (0..MAX_SERIES)
                .map(|i| Series {
                    name: format!("s{i}"),
                    values: vec![Some(1.0); MAX_POINTS],
                    skip_indices: vec![],
                })
                .collect(),
            source: "測試".into(),
            data_note: String::new(),
            data_issues: vec![],
        };
        chart.series[0].values[3000] = None;
        chart.validate().unwrap();
        assert_eq!(chart.x.len(), 10000);
        assert_eq!(chart.series[0].values[3000], None);
        chart.x.push(serde_json::json!(10000));
        for series in &mut chart.series {
            series.values.push(Some(1.0));
        }
        assert!(chart.validate().is_err());
    }
    #[test]
    fn chart_rejects_mismatched_and_executable_options() {
        let mut chart:Chart=serde_json::from_value(serde_json::json!({"kind":"line","title":"T","x_label":"s","y_label":"V","x":[1,2],"series":[{"name":"a","values":[2,null]}],"source":"測試"})).unwrap();
        chart.validate().unwrap();
        chart.series[0].values.pop();
        assert!(chart.validate().is_err());
        let mut value = serde_json::to_value(chart).unwrap();
        value["formatter"] = serde_json::json!("alert(1)");
        assert!(serde_json::from_value::<Chart>(value).is_err());
        assert!(cell("../A1").is_err());
        assert!(cell("XFE1").is_err());
    }
}
