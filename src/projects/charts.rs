//! 固定圖表資料契約；不接受 ECharts option、JavaScript 或任意 HTML。
use crate::AppResult;
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
            || self.x.len() > 1000
            || self.series.is_empty()
            || self.series.len() > 8
        {
            return Err("圖表限 line/bar/scatter，1–1000 筆與 1–8 個系列。".into());
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
    if c2 <= c1 || c2 - c1 > 8 || r2 <= r1 || r2 - r1 > 1000 {
        return Err("範圍需為 2–9 欄、2–1001 列。".into());
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
#[cfg(test)]
mod tests {
    use super::*;
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
