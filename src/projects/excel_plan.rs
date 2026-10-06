//! Excel 分析先由模型解釋欄位用途，再由本機驗證、鎖定及篩選。
//! 規劃不是語意理解的保證；時間型別與來源表頭檢查能攔截常見的錯欄。
use super::office::excel::{self, Cell, Page};
use crate::AppResult;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub column: String,
    pub header: String,
}

/// purpose 是本次要求的原文；reason 保存模型將別名對應到表頭的解釋。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    pub path: String,
    pub revision: String,
    pub sheet: usize,
    pub header_row: usize,
    pub purpose: String,
    pub reason: String,
    pub x: Field,
    pub y: Vec<Field>,
    pub time: Option<Field>,
    /// none、time_of_day、elapsed；HH:MM 永遠是時:分，不猜成分:秒。
    pub time_mode: String,
    /// measurement 或 time；確實要求分析時間時，才允許時間作為 Y。
    pub y_kind: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan {
    pub id: String,
    pub proposal: Proposal,
    pub columns: Vec<String>,
    pub headers: Vec<String>,
    pub formats: Vec<String>,
    pub date_1904: bool,
}

/// 隨 CSV 保存欄位原意與已執行的規劃，不依賴模型在上下文中記住 A/B/D。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Metadata {
    #[serde(default)]
    pub sheet_name: String,
    pub headers: Vec<String>,
    pub formats: Vec<String>,
    pub date_1904: bool,
    pub plan: Option<Plan>,
    pub filter: Option<Window>,
    pub scanned_rows: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Window {
    pub start: String,
    pub end: String,
}

/// 忽略格式的引號文字、跳脫字元及色彩等區段，保留真正的時間格式碼。
pub fn temporal_format(format: &str) -> bool {
    let mut chars = format.chars().peekable();
    let mut codes = String::new();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                for q in chars.by_ref() {
                    if q == '"' {
                        break;
                    }
                }
            }
            '\\' | '_' | '*' => {
                chars.next();
            }
            '[' => {
                let mut part = String::new();
                for q in chars.by_ref() {
                    if q == ']' {
                        break;
                    }
                    part.push(q.to_ascii_lowercase());
                }
                if matches!(part.as_str(), "h" | "hh" | "m" | "mm" | "s" | "ss") {
                    return true;
                }
            }
            _ => codes.push(c.to_ascii_lowercase()),
        }
    }
    codes.contains(['h', 's', 'y', 'd']) || codes.contains("mm")
}

pub fn temporal(cell: &Cell) -> bool {
    temporal_format(&cell.number_format)
        || (cell.text.contains(':') && parse_clock(&cell.text, false).is_ok())
}

/// 以整數毫秒比較邊界，避免 Excel 浮點序號讓剛好 12:00 的列被排除。
pub fn parse_clock(value: &str, day: bool) -> AppResult<i64> {
    let parts: Vec<_> = value.trim().split(':').collect();
    if !(2..=3).contains(&parts.len()) || parts[0].is_empty() || parts[1].len() != 2 {
        return Err("時間請使用 HH:MM 或 HH:MM:SS（可含三位毫秒）；HH:MM 表示時:分。".into());
    }
    let digits = |s: &str| -> AppResult<i64> {
        if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
            return Err("時間含非數字。".into());
        }
        s.parse().map_err(|_| "時間數值過大。".into())
    };
    let hour = digits(parts[0])?;
    let minute = digits(parts[1])?;
    let (second, millis) = if parts.len() == 3 {
        let (seconds, fraction) = parts[2].split_once('.').unwrap_or((parts[2], ""));
        if seconds.len() != 2
            || fraction.len() > 3
            || (parts[2].contains('.') && fraction.is_empty())
        {
            return Err("秒數格式不正確。".into());
        }
        (
            digits(seconds)?,
            if fraction.is_empty() {
                0
            } else {
                digits(fraction)? * 10_i64.pow((3 - fraction.len()) as u32)
            },
        )
    } else {
        (0, 0)
    };
    if minute >= 60 || second >= 60 || hour > 1_000_000 || (day && hour >= 24) {
        return Err("時間超出範圍；一天中的時間需為 00:00:00–23:59:59.999。".into());
    }
    Ok(((hour * 60 + minute) * 60 + second) * 1000 + millis)
}

impl Window {
    pub fn bounds(&self, mode: &str) -> AppResult<(i64, i64)> {
        if !matches!(mode, "time_of_day" | "elapsed") {
            return Err("時間區間需要 time_of_day 或 elapsed 規劃。".into());
        }
        let start = parse_clock(&self.start, mode == "time_of_day")?;
        // 24:00 僅可作當日區間終點，不將來源日期取餘數。
        let end = if mode == "time_of_day" && matches!(self.end.trim(), "24:00" | "24:00:00") {
            86_400_000
        } else {
            parse_clock(&self.end, mode == "time_of_day")?
        };
        if start == end || (mode == "elapsed" && start > end) {
            return Err("時間區間不可相同；經過時間的終點需大於起點。".into());
        }
        Ok((start, end))
    }
    pub fn contains(&self, value: &Cell, mode: &str) -> AppResult<bool> {
        let (start, end) = self.bounds(mode)?;
        let millis = if value.kind == "number" {
            let n = value
                .value
                .as_f64()
                .filter(|v| v.is_finite() && *v >= 0.0)
                .ok_or("時間數值無效。")?;
            if mode == "time_of_day" && n >= 1.0 {
                return Err(
                    "時間欄含日期或超過一天，不能默默去掉日期；請分離日期或明確使用經過時間。"
                        .into(),
                );
            }
            if n > 1_000_000.0 / 24.0 {
                return Err("時間數值超出支援範圍。".into());
            }
            (n * 86_400_000.0).round() as i64
        } else if value.kind == "text" {
            parse_clock(
                value.value.as_str().ok_or("時間文字無效。")?,
                mode == "time_of_day",
            )?
        } else {
            return Err("時間欄含空白、錯誤或不支援的型別；未跳過資料。".into());
        };
        Ok(if start < end {
            millis >= start && millis < end
        } else {
            millis >= start || millis < end
        })
    }
}

impl Proposal {
    /// 保留實際 Excel 欄號；選 B/D 時不能重新編號為 A/B。
    pub fn columns(&self) -> AppResult<Vec<String>> {
        if self.purpose.trim().is_empty()
            || self.reason.trim().is_empty()
            || self.purpose.chars().count() > 2000
            || self.reason.chars().count() > 1000
        {
            return Err("欄位規劃需附使用者要求（最多2000字）與對應理由（最多1000字）。".into());
        }
        if self.y.is_empty()
            || self.y.len() > 8
            || !matches!(self.y_kind.as_str(), "measurement" | "time")
        {
            return Err("需指定1–8個Y欄及 measurement／time 用途。".into());
        }
        if (self.time.is_none() && self.time_mode != "none")
            || (self.time.is_some()
                && !matches!(self.time_mode.as_str(), "time_of_day" | "elapsed"))
        {
            return Err("篩選時間欄與 time_mode 不一致。".into());
        }
        let mut columns = Vec::new();
        for field in std::iter::once(&self.x)
            .chain(self.y.iter())
            .chain(self.time.iter())
        {
            let canonical = excel::column_name(excel::column_index(&field.column)?);
            if canonical != field.column
                || field.header.trim().is_empty()
                || field.header.chars().count() > 100
            {
                return Err("欄位需使用大寫 Excel 欄字母與實際非空白表頭（最多100字）。".into());
            }
            if !columns.contains(&canonical) {
                columns.push(canonical);
            }
        }
        if self.y.iter().any(|f| f.column == self.x.column)
            || self
                .y
                .iter()
                .map(|f| &f.column)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != self.y.len()
            || columns.len() > 9
        {
            return Err("X/Y 不可重複，含篩選時間欄最多9欄。".into());
        }
        if self.y_kind == "measurement"
            && self
                .time
                .as_ref()
                .is_some_and(|t| self.y.iter().any(|y| y.column == t.column))
        {
            return Err("Y 選到了時間篩選欄，與量測值用途矛盾。請核對表頭，重新 plan_excel_analysis；不要把時間小數當作量測值。".into());
        }
        Ok(columns)
    }
}

impl Plan {
    pub fn create(proposal: Proposal, page: &Page) -> AppResult<Self> {
        let columns = proposal.columns()?;
        if page.columns != columns || page.rows.is_empty() {
            return Err("規劃樣本缺少資料或欄位不符。".into());
        }
        for field in std::iter::once(&proposal.x)
            .chain(proposal.y.iter())
            .chain(proposal.time.iter())
        {
            let index = columns
                .iter()
                .position(|c| c == &field.column)
                .ok_or("缺少規劃欄位。")?;
            if page
                .headers
                .get(index)
                .is_none_or(|h| h.text != field.header)
            {
                return Err(format!(
                    "{} 欄實際表頭不等於 {:?}，請重新檢視表頭。",
                    field.column, field.header
                ));
            }
            let is_y = proposal.y.iter().any(|y| y.column == field.column);
            if is_y
                && proposal.y_kind == "measurement"
                && page
                    .rows
                    .iter()
                    .any(|r| r.cells.get(index).is_some_and(temporal))
            {
                return Err(format!("{}／{} 的樣本是日期或時間，不能作為本次量測值 Y。請重新判斷欄位；只有使用者確實要求分析時間時才使用 y_kind=time。",field.column,field.header));
            }
        }
        if let Some(time) = &proposal.time {
            let index = columns
                .iter()
                .position(|c| c == &time.column)
                .ok_or("缺少時間欄。")?;
            if !page
                .rows
                .iter()
                .any(|r| r.cells.get(index).is_some_and(temporal))
            {
                return Err(format!("{}／{} 的樣本無法辨識為時間；請核對格式與內容，不把一般數值猜成 Excel 時間序號。",time.column,time.header));
            }
        }
        let formats = (0..columns.len())
            .map(|i| {
                page.rows
                    .iter()
                    .filter_map(|r| r.cells.get(i))
                    .find(|c| c.kind != "blank")
                    .map(|c| c.number_format.clone())
                    .unwrap_or_default()
            })
            .collect();
        let id =
            super::text::revision(&serde_json::to_string(&proposal).map_err(|e| e.to_string())?);
        Ok(Self {
            id,
            proposal,
            columns,
            headers: page.headers.iter().map(|h| h.text.clone()).collect(),
            formats,
            date_1904: page.date_1904,
        })
    }
    pub fn check_axes(&self, x: &str, ys: &[String]) -> AppResult<()> {
        let expected: Vec<_> = self.proposal.y.iter().map(|f| f.column.clone()).collect();
        if x != self.proposal.x.column || ys != expected {
            return Err(format!("作圖欄位與已鎖定規劃不符：X={}／{}；Y={}。請使用這些欄位，不可因省略中間欄而重新編號；若使用者更正目標，重新規劃及匯出。",self.proposal.x.column,self.proposal.x.header,expected.join(",")));
        }
        Ok(())
    }
    pub fn summary(&self) -> serde_json::Value {
        json!({"plan_id":self.id,"mapping":self.proposal,"columns":self.columns,"headers":self.headers,"formats":self.formats,"date_1904":self.date_1904,
            "next":"使用 export_planned_excel，時間區間為起點含、終點不含；相同plan可用於多個時間段，不能套用到其他檔案。"})
    }
    /// 從 CSV／續接紀錄讀回時，核對結構及識別碼，避免欄位重新編號或資料損壞。
    pub fn validate(&self) -> AppResult<()> {
        if self.columns != self.proposal.columns()?
            || self.headers.len() != self.columns.len()
            || self.formats.len() != self.columns.len()
            || self.id
                != super::text::revision(
                    &serde_json::to_string(&self.proposal).map_err(|e| e.to_string())?,
                )
        {
            return Err("保存的 Excel 欄位規劃不一致，請重新核對及規劃。".into());
        }
        for field in std::iter::once(&self.proposal.x)
            .chain(self.proposal.y.iter())
            .chain(self.proposal.time.iter())
        {
            let index = self
                .columns
                .iter()
                .position(|c| c == &field.column)
                .ok_or("規劃缺少欄位。")?;
            if self.headers[index] != field.header {
                return Err("保存的 Excel 表頭與規劃不一致。".into());
            }
        }
        Ok(())
    }
}

/// 舊工具沒有量測用途宣告時，不自動把時間小數當成 Y；仍可重新規劃合法時間分析。
pub fn check_unplanned_y(page: &Page) -> AppResult<()> {
    for (index, column) in page.columns.iter().enumerate().skip(1) {
        if page
            .rows
            .iter()
            .any(|r| r.cells.get(index).is_some_and(temporal))
        {
            return Err(format!("Y欄 {column} 的資料是日期／時間。請先plan_excel_analysis分開指定篩選時間與量測值；若使用者確實要分析時間，需明確規劃y_kind=time。"));
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub fn fixture() -> (Proposal, Page) {
        let proposal:Proposal=serde_json::from_value(json!({"path":"a.xlsx","revision":"excel:r","sheet":1,"header_row":1,
            "purpose":"12:00到13:00的透光值趨勢","reason":"圖樣Mean值是此表唯一的量測欄，時間僅作X與篩選",
            "x":{"column":"B","header":"紀錄時間"},"y":[{"column":"D","header":"圖樣Mean值"}],
            "time":{"column":"B","header":"紀錄時間"},"time_mode":"time_of_day","y_kind":"measurement"})).unwrap();
        let page:Page=serde_json::from_value(json!({"sheet":1,"sheet_name":"工作表1","columns":["B","D"],"header_row":1,
            "headers":[{"value":"紀錄時間","text":"紀錄時間","kind":"text","formula":null},{"value":"圖樣Mean值","text":"圖樣Mean值","kind":"text","formula":null}],
            "start_row":2,"rows":[{"row":2,"cells":[{"value":0.5,"text":"12:00:00","kind":"number","formula":null,"number_format":"hh:mm:ss"},{"value":168.4,"text":"168.4","kind":"number","formula":null,"number_format":"0.0"}]}],
            "next_row":null,"used_range":{"first_row":1,"last_row":2,"first_column":"A","last_column":"D"},"date_1904":false})).unwrap();
        (proposal, page)
    }
    #[test]
    fn measurement_binding_rejects_time_and_survives_serialization() {
        let (proposal, page) = fixture();
        let plan = Plan::create(proposal.clone(), &page).unwrap();
        assert_eq!(plan.columns, vec!["B", "D"]);
        let restored: Plan = serde_json::from_str(&serde_json::to_string(&plan).unwrap()).unwrap();
        restored.validate().unwrap();
        restored.check_axes("B", &["D".into()]).unwrap();
        assert!(restored.check_axes("A", &["B".into()]).is_err());
        let mut wrong = proposal.clone();
        wrong.x = Field {
            column: "A".into(),
            header: "Index".into(),
        };
        wrong.y = vec![wrong.time.clone().unwrap()];
        assert!(wrong.columns().is_err());
        let mut wrong = proposal.clone();
        wrong.y[0].header = "透光值".into();
        assert!(
            Plan::create(wrong, &page).is_err(),
            "實際表頭不可用別名冒充"
        );
        let mut wrong_page = page.clone();
        wrong_page.rows[0].cells[1].number_format = "hh:mm:ss".into();
        assert!(Plan::create(proposal.clone(), &wrong_page).is_err());
        let mut legitimate = proposal;
        legitimate.y_kind = "time".into();
        legitimate.purpose = "比較兩個時間欄".into();
        assert!(
            Plan::create(legitimate, &wrong_page).is_ok(),
            "合法時間Y仍支援"
        );
    }
    #[test]
    fn time_windows_are_half_open_and_preserve_dates_and_elapsed_days() {
        let (_, page) = fixture();
        let mut cell = page.rows[0].cells[0].clone();
        let window = Window {
            start: "12:00".into(),
            end: "13:00".into(),
        };
        assert!(window.contains(&cell, "time_of_day").unwrap());
        cell.value = json!(13.0 / 24.0);
        assert!(!window.contains(&cell, "time_of_day").unwrap());
        cell.value = json!(46000.5);
        assert!(window.contains(&cell, "time_of_day").is_err());
        let overnight = Window {
            start: "23:00".into(),
            end: "01:00".into(),
        };
        cell.value = json!(0.5 / 24.0);
        assert!(overnight.contains(&cell, "time_of_day").unwrap());
        let elapsed = Window {
            start: "25:00".into(),
            end: "26:00".into(),
        };
        cell.value = json!(25.5 / 24.0);
        assert!(elapsed.contains(&cell, "elapsed").unwrap());
        assert!(elapsed.bounds("time_of_day").is_err());
        assert_eq!(parse_clock("12:00", true).unwrap(), 43_200_000);
        assert_eq!(parse_clock("00:00:01.125", true).unwrap(), 1125);
        assert!(parse_clock("12:60", false).is_err());
        assert!(Window {
            start: "00:00".into(),
            end: "24:00".into()
        }
        .bounds("time_of_day")
        .is_ok());
        cell.kind = "text".into();
        cell.value = json!("12:30:00");
        assert!(window.contains(&cell, "time_of_day").unwrap());
        cell.value = json!("不明");
        assert!(window.contains(&cell, "time_of_day").is_err());
    }
    #[test]
    fn number_format_distinguishes_temporal_codes_from_literal_units() {
        for value in ["hh:mm:ss", "[h]:mm:ss", "yyyy/m/d", "[$-409]h:mm AM/PM"] {
            assert!(temporal_format(value), "{value}");
        }
        for value in [
            "General",
            "0.00",
            "[Red]0.00",
            "0.00\"ms\"",
            "0.0\\s",
            "0.00E+00",
        ] {
            assert!(!temporal_format(value), "{value}");
        }
    }
}
