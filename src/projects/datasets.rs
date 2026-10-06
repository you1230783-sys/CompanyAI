//! 將大量資料留在本地 CSV；模型只接收欄位、統計與首尾預覽。
//! 不執行正規表示式、公式或腳本。CSV 保留來源行號，畫圖沿用既有異常值決策。
use super::{charts, files, office::excel, Project};
use crate::AppResult;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    io::Read,
    sync::atomic::{AtomicBool, Ordering},
};

pub const MAX_ROWS: usize = 250_000;
pub const MAX_BYTES: usize = 64 * 1024 * 1024;
const PREFIX: [&str; 6] = [
    "__source_path",
    "__source_revision",
    "__source_sheet",
    "__source_row",
    "__kinds",
    "__display",
];

/// LOG 欄位採固定字串分隔或前後標記；索引從 1 開始，缺少標記保留空白。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub name: String,
    pub mode: String,
    pub delimiter: Option<String>,
    pub index: Option<usize>,
    pub start: Option<String>,
    pub end: Option<String>,
}
impl Field {
    pub fn validate(&self) -> AppResult<()> {
        if self.name.trim().is_empty()
            || self.name.chars().count() > 60
            || self.name.starts_with("__")
        {
            return Err("資料欄名需為 1–60 字，不能以 __ 開頭。".into());
        }
        let short = |s: &Option<String>| s.as_ref().is_none_or(|s| s.chars().count() <= 100);
        if !short(&self.delimiter) || !short(&self.start) || !short(&self.end) {
            return Err("分隔或定位標記最多 100 字。".into());
        }
        let valid = match self.mode.as_str() {
            "delimited" => {
                self.delimiter.as_ref().is_some_and(|s| !s.is_empty())
                    && self.index.is_some_and(|n| (1..=256).contains(&n))
                    && self.start.is_none()
                    && self.end.is_none()
            }
            "between" => {
                self.start.as_ref().is_some_and(|s| !s.is_empty())
                    && self.end.as_ref().is_none_or(|s| !s.is_empty())
                    && self.delimiter.is_none()
                    && self.index.is_none()
            }
            "timestamp_seconds" => {
                self.delimiter.is_none()
                    && self.index.is_none()
                    && self.start.is_none()
                    && self.end.is_none()
            }
            _ => false,
        };
        if !valid {
            return Err("LOG 欄位設定不合法：delimited 需分隔字串與 index；between 需 start、可選 end；timestamp_seconds 不帶其他設定。".into());
        }
        Ok(())
    }
    pub fn extract(&self, line: &str, millis: Option<u32>) -> AppResult<String> {
        let value = match self.mode.as_str() {
            "timestamp_seconds" => millis
                .map(|v| format!("{}.{:03}", v / 1000, v % 1000))
                .unwrap_or_default(),
            "delimited" => line
                .split(self.delimiter.as_deref().ok_or("缺少分隔字串。")?)
                .nth(self.index.ok_or("缺少欄位序號。")?.saturating_sub(1))
                .unwrap_or("")
                .trim()
                .to_owned(),
            "between" => {
                let value = line
                    .split_once(self.start.as_deref().ok_or("缺少起始標記。")?)
                    .map(|(_, rest)| rest);
                match (value, self.end.as_deref()) {
                    (Some(rest), Some(end)) => rest
                        .split_once(end)
                        .map(|(v, _)| v)
                        .unwrap_or("")
                        .trim()
                        .to_owned(),
                    (Some(rest), None) => rest.trim().to_owned(),
                    _ => String::new(),
                }
            }
            _ => return Err("不支援的欄位擷取方式。".into()),
        };
        if value.len() > 4096 {
            return Err("擷取的單格超過 4096 bytes，請縮小定位範圍；未截斷資料。".into());
        }
        Ok(value)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Reference {
    pub path: String,
    pub revision: String,
    pub rows: usize,
    pub columns: Vec<String>,
}
pub struct Row {
    pub path: String,
    pub revision: String,
    pub sheet: usize,
    pub row: usize,
    pub values: Vec<String>,
    pub kinds: Vec<String>,
    pub texts: Vec<String>,
}
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Row>,
    pub excel: Option<super::excel_plan::Metadata>,
    size: usize,
}
impl Table {
    pub fn new(columns: Vec<String>) -> AppResult<Self> {
        let unique: BTreeSet<_> = columns.iter().collect();
        if columns.is_empty()
            || columns.len() > 16
            || unique.len() != columns.len()
            || columns
                .iter()
                .any(|s| s.trim().is_empty() || s.chars().count() > 100 || s.starts_with("__"))
        {
            return Err("CSV 需 1–16 個不重複的短欄名，不能以 __ 開頭。".into());
        }
        Ok(Self {
            columns,
            rows: vec![],
            excel: None,
            size: 0,
        })
    }
    pub fn push(&mut self, mut row: Row) -> AppResult<()> {
        if self.rows.len() >= MAX_ROWS
            || row.values.len() != self.columns.len()
            || row.values.iter().any(|v| v.len() > 4096)
        {
            return Err(
                "CSV 超過 250000 筆、單格 4096 bytes，或欄數不一致；未產生部分成果。".into(),
            );
        }
        if row.kinds.is_empty() {
            row.kinds = row
                .values
                .iter()
                // LOG 本來就是文字；在本地辨識明確有限數字，保留原字串作顯示／追查。
                // 這樣一萬筆數值不會每次作圖都重複產生九萬格「文字轉換」明細。
                // Excel 會提供原始 kinds，不走此推斷，錯誤／合併／文字型別保持不變。
                .map(|s| {
                    if s.is_empty() {
                        "blank"
                    } else if charts::quality::numeric_text(s).is_some() {
                        "number"
                    } else {
                        "text"
                    }
                    .into()
                })
                .collect();
        }
        if row.texts.is_empty() {
            row.texts = row.values.clone();
        }
        if row.kinds.len() != self.columns.len()
            || row.texts.len() != self.columns.len()
            || row.texts.iter().any(|s| s.len() > 4096)
        {
            return Err("CSV 型別與顯示文字欄數不一致或過長。".into());
        }
        self.size += row.texts.iter().map(String::len).sum::<usize>();
        self.size += row.path.len()
            + row.revision.len()
            + 100
            + row.values.iter().map(String::len).sum::<usize>();
        if self.size > MAX_BYTES {
            return Err("資料集超過 64 MiB，請按時間或機台分批。".into());
        }
        self.rows.push(row);
        Ok(())
    }
    /// Excel Value2 數值保留完整精度；日期仍為 Excel 序號，不猜測單位或時間軸。
    pub fn from_excel(page: &excel::Page, path: &str, revision: &str) -> AppResult<Self> {
        let mut table = Self::new(page.columns.clone())?;
        table.excel = Some(super::excel_plan::Metadata {
            sheet_name: page.sheet_name.clone(),
            headers: page.headers.iter().map(|h| h.text.clone()).collect(),
            formats: (0..page.columns.len())
                .map(|i| {
                    page.rows
                        .iter()
                        .filter_map(|r| r.cells.get(i))
                        .find(|c| c.kind != "blank")
                        .map(|c| c.number_format.clone())
                        .unwrap_or_default()
                })
                .collect(),
            date_1904: page.date_1904,
            plan: None,
            filter: None,
            scanned_rows: page.rows.len(),
        });
        for row in &page.rows {
            table.push(Row {
                path: path.into(),
                revision: revision.into(),
                sheet: page.sheet,
                row: row.row,
                values: row
                    .cells
                    .iter()
                    .map(|c| match &c.value {
                        Value::Null => String::new(),
                        Value::String(s) => s.clone(),
                        value => value.to_string(),
                    })
                    .collect(),
                kinds: row.cells.iter().map(|c| c.kind.clone()).collect(),
                texts: row.cells.iter().map(|c| c.text.clone()).collect(),
            })?;
        }
        Ok(table)
    }
    pub fn csv(&self) -> AppResult<String> {
        if self.rows.is_empty() {
            return Err("篩選後沒有資料，未建立空的 CSV。".into());
        }
        let mut out = String::from("\u{feff}");
        record(
            &mut out,
            PREFIX
                .iter()
                .map(|s| s.to_string())
                .chain(self.columns.iter().cloned()),
        );
        for (index, row) in self.rows.iter().enumerate() {
            record(
                &mut out,
                [
                    row.path.clone(),
                    row.revision.clone(),
                    row.sheet.to_string(),
                    row.row.to_string(),
                    serde_json::to_string(&row.kinds).map_err(|e| e.to_string())?,
                    // v2 的第一列攜帶欄位結構；其餘列與舊 CSV 保持顯示文字陣列。
                    // 使用既有 JSON 追蹤欄，不插入假資料列或重新編號 A/B/D。
                    if index == 0 && self.excel.is_some() {
                        json!({"version":2,"texts":row.texts,"excel":self.excel}).to_string()
                    } else {
                        serde_json::to_string(&row.texts).map_err(|e| e.to_string())?
                    },
                ]
                .into_iter()
                .chain(row.values.iter().cloned()),
            );
            if out.len() > MAX_BYTES {
                return Err("CSV 編碼後超過 64 MiB，請縮小範圍。".into());
            }
        }
        Ok(out)
    }
    /// 只有首尾各十筆可送模型；逐格長文字明示截短，完整字串仍在 CSV。
    pub fn summary(&self, reference: &Reference) -> Value {
        let preview = |(i, row): (usize, &Row)| {
            json!({"data_row":i+1,"source_row":row.row,
            "values":row.values.iter().map(|v| { let mut s:String=v.chars().take(80).collect(); if v.chars().count()>80 {s.push_str("…（預覽截短）");} s }).collect::<Vec<_>>()})
        };
        let statistics: Vec<_> = self.columns.iter().enumerate().map(|(i, name)| {
            let mut numeric = 0; let mut blank = 0; let mut min: Option<f64> = None; let mut max: Option<f64> = None;
            for row in &self.rows {
                let value = &row.values[i];
                if row.kinds[i]=="blank" {blank += 1;}
                else if let Some(n) = matches!(row.kinds[i].as_str(),"number"|"text").then(|| charts::quality::numeric_text(value)).flatten() {
                    numeric += 1; min = Some(min.map_or(n, |m| m.min(n))); max = Some(max.map_or(n, |m| m.max(n)));
                }
            }
            json!({"column":name,"numeric":numeric,"blank":blank,"non_numeric":self.rows.len()-numeric-blank,"min":min,"max":max})
        }).collect();
        json!({"dataset":reference,"statistics":statistics,"excel_schema":self.excel,
            "head":self.rows.iter().enumerate().take(10).map(preview).collect::<Vec<_>>(),
            "tail":self.rows.iter().enumerate().skip(self.rows.len().saturating_sub(10)).map(preview).collect::<Vec<_>>(),
            "scope":"僅已選取的欄位與範圍；首尾預覽不代表資料分布。完整數值只在本地 CSV，畫圖請呼叫 chart_dataset，不要抄寫點陣或重新逐頁讀取。CSV 前六欄為來源追蹤及原始型別／顯示文字；LOG sheet=0、row=原行號。"})
    }
    pub fn page(
        &self,
        x: &str,
        ys: &[String],
        start: usize,
        count: usize,
    ) -> AppResult<excel::Page> {
        if let Some(plan) = self.excel.as_ref().and_then(|m| m.plan.as_ref()) {
            plan.check_axes(x, ys)?;
        }
        if start == 0 || !(1..=charts::MAX_POINTS).contains(&count) || ys.is_empty() || ys.len() > 8
        {
            return Err(
                "畫圖需 1–8 個 Y 欄、從 1 開始的資料列，每張 1–10000 筆；不自動抽樣。".into(),
            );
        }
        let names: Vec<String> = std::iter::once(x.to_owned())
            .chain(ys.iter().cloned())
            .collect();
        if names.iter().collect::<BTreeSet<_>>().len() != names.len() {
            return Err("X／Y 欄位不可重複。".into());
        }
        let indices = names
            .iter()
            .map(|n| {
                self.columns
                    .iter()
                    .position(|c| c == n)
                    .ok_or_else(|| format!("CSV 沒有欄位 {n}。"))
            })
            .collect::<AppResult<Vec<_>>>()?;
        let end = start
            .checked_sub(1)
            .and_then(|s| s.checked_add(count))
            .ok_or("資料範圍無效。")?;
        if end > self.rows.len() {
            return Err("資料範圍超過 CSV 筆數，未靜默縮短。".into());
        }
        let cell = |s: &str| excel::Cell {
            value: if s.is_empty() { Value::Null } else { json!(s) },
            text: s.into(),
            kind: if s.is_empty() { "blank" } else { "text" }.into(),
            formula: None,
            number_format: String::new(),
        };
        Ok(excel::Page {
            sheet: 0,
            sheet_name: "CSV".into(),
            columns: names.clone(),
            header_row: 1,
            headers: indices
                .iter()
                .map(|&i| {
                    cell(
                        self.excel
                            .as_ref()
                            .and_then(|m| m.headers.get(i))
                            .unwrap_or(&self.columns[i]),
                    )
                })
                .collect(),
            start_row: start,
            rows: self.rows[start - 1..end]
                .iter()
                .enumerate()
                .map(|(i, r)| excel::Row {
                    row: start + i,
                    cells: indices
                        .iter()
                        .map(|&c| {
                            let mut result = cell(&r.values[c]);
                            result.kind = r.kinds[c].clone();
                            result.text = r.texts[c].clone();
                            result.number_format = self
                                .excel
                                .as_ref()
                                .and_then(|m| m.formats.get(c))
                                .cloned()
                                .unwrap_or_default();
                            if result.kind == "number" {
                                result.value = charts::quality::numeric_text(&r.values[c])
                                    .map_or(Value::Null, |v| json!(v));
                            }
                            result
                        })
                        .collect(),
                })
                .collect(),
            next_row: (end < self.rows.len()).then_some(end + 1),
            used_range: excel::Bounds {
                first_row: 1,
                last_row: self.rows.len(),
                first_column: x.into(),
                last_column: ys.last().cloned().unwrap_or_default(),
            },
            date_1904: self.excel.as_ref().is_some_and(|m| m.date_1904),
        })
    }
    pub fn annotate(&self, chart: &mut charts::Chart) {
        for issue in &mut chart.data_issues {
            if let Some(source) = self.rows.get(issue.row.saturating_sub(1)) {
                let data_row = issue.row;
                issue.sheet = source.sheet;
                issue.row = source.row;
                issue.cell = format!(
                    "{} | 工作表 {} | 原列/行 {} | CSV 資料列 {} | {}",
                    source.path, source.sheet, source.row, data_row, issue.cell
                );
            }
        }
    }
}

/// 以可逆的單引號前綴防止試算表將文字當成公式；讀回時還原，原數值不改。
fn escaped(value: &str) -> String {
    if value.starts_with('\'')
        || (value.starts_with(['=', '+', '-', '@', '\t', '\r'])
            && charts::quality::numeric_text(value).is_none())
    {
        format!("'{value}")
    } else {
        value.into()
    }
}
fn record(out: &mut String, values: impl Iterator<Item = String>) {
    for (i, value) in values.enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('"');
        out.push_str(&escaped(&value).replace('"', "\"\""));
        out.push('"');
    }
    out.push_str("\r\n");
}
/// 僅接受本工具生成的有引號 CSV 方言；不把任意一般 CSV 猜成帶有來源的資料集。
fn parse(text: &str) -> AppResult<Table> {
    let mut chars = text.trim_start_matches('\u{feff}').chars().peekable();
    let mut rows = Vec::new();
    let mut row = Vec::new();
    while chars.peek().is_some() {
        if chars.next() != Some('"') {
            return Err("CSV 格式不合法：需要本工具匯出的資料集。".into());
        }
        let mut value = String::new();
        loop {
            match chars.next() {
                Some('"') if chars.peek() == Some(&'"') => {
                    chars.next();
                    value.push('"');
                }
                Some('"') => break,
                Some(c) => value.push(c),
                None => return Err("CSV 引號未結束。".into()),
            }
            if value.len() > 150_000 {
                return Err("CSV 單格過大。".into());
            }
        }
        row.push(value.strip_prefix('\'').unwrap_or(&value).to_owned());
        if row.len() > 22 {
            return Err("CSV 欄數過大。".into());
        }
        match chars.next() {
            Some(',') => (),
            Some('\r') if chars.next() == Some('\n') => {
                rows.push(std::mem::take(&mut row));
            }
            _ => return Err("CSV 列分隔不合法。".into()),
        }
        if rows.len() > MAX_ROWS + 1 {
            return Err("CSV 資料筆數超過上限。".into());
        }
    }
    if !row.is_empty() || rows.is_empty() || rows[0].len() < 7 || rows[0][..6] != PREFIX {
        return Err("CSV 缺少來源欄位。".into());
    }
    let mut table = Table::new(rows[0][6..].to_vec())?;
    for (index, row) in rows.into_iter().skip(1).enumerate() {
        if row.len() != table.columns.len() + 6 {
            return Err("CSV 欄數不一致。".into());
        }
        let display: Value = serde_json::from_str(&row[5]).map_err(|_| "CSV 顯示欄無效。")?;
        let texts = if display.is_object() {
            if index != 0 || display["version"] != 2 {
                return Err("CSV 欄位結構版本或位置無效。".into());
            }
            let metadata: super::excel_plan::Metadata =
                serde_json::from_value(display["excel"].clone())
                    .map_err(|_| "CSV 欄位結構無效。")?;
            if metadata.headers.len() != table.columns.len()
                || metadata.formats.len() != table.columns.len()
            {
                return Err("CSV 欄位結構長度不符。".into());
            }
            if let Some(plan) = &metadata.plan {
                plan.validate()?;
                if plan.columns != table.columns
                    || plan.headers != metadata.headers
                    || plan.date_1904 != metadata.date_1904
                {
                    return Err("CSV 欄位與鎖定規劃不符。".into());
                }
            }
            table.excel = Some(metadata);
            serde_json::from_value(display["texts"].clone()).map_err(|_| "CSV 顯示文字無效。")?
        } else {
            serde_json::from_value(display).map_err(|_| "CSV 顯示欄無效。")?
        };
        if let Some(plan) = table.excel.as_ref().and_then(|m| m.plan.as_ref()) {
            if row[0] != plan.proposal.path
                || row[1] != plan.proposal.revision
                || row[2] != plan.proposal.sheet.to_string()
            {
                return Err("CSV 來源與欄位規劃不一致。".into());
            }
        }
        table.push(Row {
            path: row[0].clone(),
            revision: row[1].clone(),
            sheet: row[2].parse().map_err(|_| "來源工作表無效。")?,
            row: row[3].parse().map_err(|_| "來源行號無效。")?,
            values: row[6..].to_vec(),
            kinds: serde_json::from_str(&row[4]).map_err(|_| "CSV 型別欄無效。")?,
            texts,
        })?;
    }
    Ok(table)
}
pub fn load(
    project: &Project,
    path: &str,
    expected: &str,
    cancel: &AtomicBool,
) -> AppResult<Table> {
    inspect(project, path, Some(expected), cancel).map(|(table, _)| table)
}
/// 跨任務可只指定路徑取得目前版本；實際作圖仍必須鎖定已檢視的版本。
pub fn inspect(
    project: &Project,
    path: &str,
    expected: Option<&str>,
    cancel: &AtomicBool,
) -> AppResult<(Table, String)> {
    let relative = files::relative(path)?;
    if !relative.starts_with("_AI_Output")
        || relative
            .extension()
            .is_none_or(|s| !s.eq_ignore_ascii_case("csv"))
    {
        return Err("資料集需為專案 _AI_Output 中的 CSV。".into());
    }
    let target = project.root.join(relative);
    let _guards = files::pin(target.parent().ok_or("CSV 缺少目錄。")?)?;
    let mut file = files::checked_file(&target)?;
    files::reject_internal(&file)?;
    if file.metadata().map_err(|e| e.to_string())?.len() > MAX_BYTES as u64 {
        return Err("CSV 超過 64 MiB。".into());
    }
    let mut bytes = Vec::new();
    let mut buffer = [0; 65536];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("CSV 讀取已取消。".into());
        }
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..n]);
        if bytes.len() > MAX_BYTES {
            return Err("CSV 超過 64 MiB。".into());
        }
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| "CSV 必須是 UTF-8。")?;
    let revision = super::text::revision(text);
    if expected.is_some_and(|r| r != revision) {
        return Err("CSV 版本已改變，請確認資料集後重新匯出；未使用舊欄位或舊圖表。".into());
    }
    Ok((parse(text)?, revision))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn large_numeric_log_dataset_keeps_every_point_without_per_number_anomalies() {
        let columns = (0..9).map(|i| format!("v{i}")).collect::<Vec<_>>();
        let mut table = Table::new(columns.clone()).unwrap();
        for row in 1..=10_000 {
            table
                .push(Row {
                    path: "20260623_message_Z01-CY.log".into(),
                    revision: "source-version".into(),
                    sheet: 0,
                    row,
                    values: (0..9)
                        .map(|i| {
                            if i == 0 {
                                format!("{row:05}")
                            } else {
                                (row * i).to_string()
                            }
                        })
                        .collect(),
                    kinds: vec![],
                    texts: vec![],
                })
                .unwrap();
        }
        let restored = parse(&table.csv().unwrap()).unwrap();
        let page = restored.page("v0", &columns[1..], 1, 10_000).unwrap();
        let mut chart =
            charts::from_page(&page, "scatter", "full log", "x", "y", "data.csv", "r").unwrap();
        restored.annotate(&mut chart);
        chart.validate().unwrap();
        assert_eq!(chart.x.len(), 10_000);
        assert_eq!(chart.series.len(), 8);
        assert_eq!(chart.series[7].values[9999], Some(80_000.0));
        assert!(chart.data_issues.is_empty());
        let categorical =
            charts::from_page(&page, "line", "categories", "x", "y", "data.csv", "r").unwrap();
        assert_eq!(categorical.x[0], "00001", "類別 X 的前導零不可丟失");
    }
    #[test]
    fn csv_roundtrip_preserves_quotes_unicode_formulas_and_provenance() {
        let mut table = Table::new(vec!["A".into(), "F".into()]).unwrap();
        for (i, s) in [
            "001",
            "中文,\"值\"\n下一行",
            "=HYPERLINK(1)",
            "'原始",
            "-1.234e-8",
            "",
        ]
        .iter()
        .enumerate()
        {
            table
                .push(Row {
                    path: "來源.log".into(),
                    revision: "版本".into(),
                    sheet: 0,
                    row: 100 + i,
                    values: vec![i.to_string(), s.to_string()],
                    kinds: vec![],
                    texts: vec![],
                })
                .unwrap();
        }
        let text = table.csv().unwrap();
        assert!(text.contains("'=HYPERLINK"));
        let restored = parse(&text).unwrap();
        for (a, b) in table.rows.iter().zip(&restored.rows) {
            assert_eq!(a.values, b.values);
            assert_eq!(a.row, b.row);
        }
        assert!(parse(&text[..text.len() - 2]).is_err());
    }
    #[test]
    fn excel_csv_retains_plan_headers_formats_and_correct_y() {
        let (proposal, page) = super::super::excel_plan::tests::fixture();
        let plan = super::super::excel_plan::Plan::create(proposal, &page).unwrap();
        let mut table = Table::from_excel(&page, "a.xlsx", "excel:r").unwrap();
        table.excel.as_mut().unwrap().plan = Some(plan);
        let restored = parse(&table.csv().unwrap()).unwrap();
        assert!(restored.page("B", &["B".into()], 1, 1).is_err());
        let page = restored.page("B", &["D".into()], 1, 1).unwrap();
        assert_eq!(page.headers[1].text, "圖樣Mean值");
        assert_eq!(page.rows[0].cells[0].number_format, "hh:mm:ss");
        let chart = charts::from_page(
            &page,
            "line",
            "透光值",
            "紀錄時間",
            "圖樣Mean值",
            "data.csv",
            "r",
        )
        .unwrap();
        assert_eq!(chart.series[0].values, vec![Some(168.4)]);
        assert_eq!(chart.x, vec!["12:00:00"]);
        assert!(parse(&table.csv().unwrap().replace("excel:r", "excel:changed")).is_err());
    }
    #[test]
    fn preview_is_bounded_and_chart_uses_the_middle_rows_and_original_location() {
        let mut table = Table::new(vec!["seconds".into(), "pressure".into()]).unwrap();
        for i in 1..=100 {
            table
                .push(Row {
                    path: "machine.log".into(),
                    revision: "r".into(),
                    sheet: 0,
                    row: i + 400,
                    values: vec![
                        i.to_string(),
                        if i == 50 {
                            "bad".into()
                        } else {
                            (i * 2).to_string()
                        },
                    ],
                    kinds: vec![],
                    texts: vec![],
                })
                .unwrap();
        }
        let r = Reference {
            path: "data.csv".into(),
            revision: "r".into(),
            rows: 100,
            columns: table.columns.clone(),
        };
        let summary = table.summary(&r);
        assert_eq!(summary["head"].as_array().unwrap().len(), 10);
        assert_eq!(summary["tail"].as_array().unwrap().len(), 10);
        assert!(!summary.to_string().contains("bad"));
        let page = table.page("seconds", &["pressure".into()], 1, 100).unwrap();
        let prepared =
            charts::prepare_page(&page, "scatter", "test", "x", "y", "data.csv", "r").unwrap();
        let mut chart = prepared.apply(&[charts::quality::Choice::Skip]).unwrap();
        table.annotate(&mut chart);
        assert_eq!(chart.series[0].values[48], Some(98.0));
        assert_eq!(chart.series[0].skip_indices, vec![49]);
        assert!(chart
            .data_issues
            .iter()
            .any(|i| i.original_text == "bad" && i.row == 450 && i.cell.contains("machine.log")));
        assert!(table.page("seconds", &["pressure".into()], 1, 101).is_err());
    }
}
