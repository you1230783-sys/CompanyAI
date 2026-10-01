//! 固定圖表資料契約；不接受 ECharts option、JavaScript 或任意 HTML。
use crate::AppResult;
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub mod png;

pub const MAX_POINTS: usize = 10_000;
pub const MAX_SERIES: usize = 8;
pub const MAX_CHART_BYTES: usize = 8 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Series {
    pub name: String,
    pub values: Vec<Option<f64>>,
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
}
impl Chart {
    pub fn validate(&self) -> AppResult<()> {
        if !["line", "bar", "scatter"].contains(&self.kind.as_str())
            || self.x.is_empty()
            || self.x.len() > MAX_POINTS
            || self.series.is_empty()
            || self.series.len() > MAX_SERIES
        {
            return Err("圖表限 line/bar/scatter，1–10000 筆與 1–8 個系列。".into());
        }
        for label in [&self.title, &self.x_label, &self.y_label, &self.source] {
            if label.chars().count() > 500 {
                return Err("圖表標籤超過 500 字。".into());
            }
        }
        for x in &self.x {
            if self.kind == "scatter" && x.as_f64().is_none()
                || !(x.is_number() || x.as_str().is_some_and(|s| s.chars().count() <= 100))
            {
                return Err("橫軸只接受短文字或數字；散佈圖必須使用數字。".into());
            }
        }
        for series in &self.series {
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
pub fn from_excel(
    content: &str,
    sheet: usize,
    range: &str,
    kind: &str,
    title: &str,
    x_label: &str,
    y_label: &str,
    path: &str,
    revision: &str,
) -> AppResult<Chart> {
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
    let mut chart = Chart {
        kind: kind.into(),
        title: title.into(),
        x_label: x_label.into(),
        y_label: y_label.into(),
        x: vec![],
        series: vec![],
        source: format!("{path} | {revision} | 工作表 {sheet} {range}"),
    };
    for c in c1 + 1..=c2 {
        let name = cells
            .get(&(c, r1))
            .map(|b| b.text.clone())
            .filter(|s| !s.trim().is_empty())
            .ok_or("系列標題不可空白。")?;
        chart.series.push(Series {
            name,
            values: vec![],
        });
    }
    for r in r1 + 1..=r2 {
        let x = cells.get(&(c1, r)).ok_or("橫軸儲存格不存在。")?;
        if x.kind == "formula" || x.kind == "readonly" || x.text.trim().is_empty() {
            return Err("橫軸不支援空白、公式或特殊儲存格。".into());
        }
        chart.x.push(if kind == "scatter" {
            serde_json::json!(x.text.parse::<f64>().map_err(|_| "散佈圖橫軸需要數字。")?)
        } else {
            Value::String(x.text.clone())
        });
        for c in c1 + 1..=c2 {
            let value = match cells.get(&(c, r)) {
                None => None,
                Some(b) if b.text.trim().is_empty() => None,
                Some(b) if b.kind == "number" => {
                    Some(b.text.parse::<f64>().map_err(|_| "儲存格無法轉為數字。")?)
                }
                _ => return Err("縱軸只能使用原生數值或空白；公式與文字不自行轉換。".into()),
            };
            chart.series[c - c1 - 1].values.push(value);
        }
    }
    chart.validate()?;
    Ok(chart)
}
/// 直接使用 Excel 選欄工具的原生數值，不經模型抄寫，不因中間欄或空值而位移。
/// 第一個欄位是 X，後續為 Y；同一版本的公式數值可讀，錯誤／合併格不可冒充空白。
pub fn from_page(
    page: &super::office::excel::Page,
    kind: &str,
    title: &str,
    x_label: &str,
    y_label: &str,
    path: &str,
    revision: &str,
) -> AppResult<Chart> {
    if page.columns.len() < 2 || page.columns.len() > 9 || page.headers.len() != page.columns.len()
    {
        return Err("Excel 圖表欄位結構不一致。".into());
    }
    let mut chart = Chart {
        kind: kind.into(),
        title: title.into(),
        x_label: x_label.into(),
        y_label: y_label.into(),
        x: vec![],
        series: page
            .headers
            .iter()
            .enumerate()
            .skip(1)
            .map(|(i, header)| Series {
                name: if header.text.trim().is_empty() {
                    page.columns[i].clone()
                } else {
                    header.text.clone()
                },
                values: vec![],
            })
            .collect(),
        source: format!(
            "{path} | {revision} | 工作表 {} | 欄 {} | 列 {}–{}",
            page.sheet,
            page.columns.join(","),
            page.start_row,
            page.rows.last().map_or(page.start_row, |r| r.row)
        ),
    };
    for row in &page.rows {
        if row.cells.len() != page.columns.len() {
            return Err("Excel 資料列與欄位數不一致。".into());
        }
        let x = &row.cells[0];
        if !matches!(x.kind.as_str(), "number" | "text")
            || x.value.as_str().is_some_and(|s| s.trim().is_empty())
        {
            return Err(format!(
                "橫軸 {}{} 為空白、錯誤或特殊值，請明確縮小範圍；不自動刪列。",
                page.columns[0], row.row
            ));
        }
        chart.x.push(if kind == "scatter" {
            serde_json::json!(x.value.as_f64().ok_or("散佈圖橫軸需為 Excel 數值。")?)
        } else if !x.text.is_empty() {
            if x.text.chars().all(|c| c == '#') {
                return Err("橫軸顯示為 ####，請先在 Excel 調整顯示或改選有效標籤欄。".into());
            }
            Value::String(x.text.clone())
        } else {
            x.value.clone()
        });
        for (i, cell) in row.cells.iter().enumerate().skip(1) {
            let number = match cell.kind.as_str() {
                "blank" => None,
                "number" => Some(cell.value.as_f64().ok_or("Excel 數值格式錯誤。")?),
                _ => {
                    return Err(format!(
                        "縱軸 {}{} 不是数值或空白；錯誤、文字與合併格不轉為零。",
                        page.columns[i], row.row
                    ))
                }
            };
            chart.series[i - 1].values.push(number);
        }
    }
    chart.validate()?;
    Ok(chart)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ten_thousand_points_are_preserved_without_sampling() {
        let mut chart = Chart {
            kind: "line".into(),
            title: "T".into(),
            x_label: "x".into(),
            y_label: "y".into(),
            x: (0..MAX_POINTS).map(|i| serde_json::json!(i)).collect(),
            series: (0..MAX_SERIES)
                .map(|i| Series {
                    name: format!("s{i}"),
                    values: vec![Some(1.0); MAX_POINTS],
                })
                .collect(),
            source: "測試".into(),
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
