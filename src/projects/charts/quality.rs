//! 作圖資料預檢及使用者決策；不改來源活頁簿，不讓模型傳入處理政策。
use super::{Chart, Series};
use crate::{projects::office::excel::Page, AppResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::AtomicBool,
    time::Instant,
};

pub type Chooser = Box<dyn FnMut(&Review, &AtomicBool, Instant) -> AppResult<Option<Vec<Choice>>>>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Choice {
    Gap,
    Skip,
    Zero,
}

#[derive(Clone, Debug, Serialize)]
pub struct Group {
    pub column: String,
    pub category: String,
    pub x_axis: bool,
    pub count: usize,
    pub samples: Vec<String>,
    pub choices: Vec<Choice>,
    #[serde(skip)]
    cells: Vec<(usize, usize)>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Review {
    pub title: String,
    pub source: String,
    pub converted: usize,
    pub blanks: usize,
    pub groups: Vec<Group>,
}
impl Review {
    pub fn validate(&self, choices: &[Choice]) -> AppResult<()> {
        if choices.len() != self.groups.len()
            || self
                .groups
                .iter()
                .zip(choices)
                .any(|(g, c)| !g.choices.contains(c))
        {
            return Err("請為每一組資料選擇有效的處理方式。".into());
        }
        Ok(())
    }
}
pub struct Prepared {
    pub review: Review,
    chart: Chart,
    issues: Vec<(usize, usize, super::DataIssue)>,
}

/// 僅接受不含千分位、單位或百分號的有限十進位數字；不猜地區格式。
pub fn numeric_text(text: &str) -> Option<f64> {
    let text = text.trim();
    if text.is_empty()
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || b"+-.eE".contains(&b))
    {
        return None;
    }
    text.parse::<f64>().ok().filter(|v| v.is_finite())
}
fn category(cell: &crate::projects::office::excel::Cell) -> &'static str {
    match cell.kind.as_str() {
        "error" => "Excel 錯誤",
        "merged" => "合併儲存格",
        "blank" => "空白",
        "text" if cell.text.chars().any(|c| c.is_ascii_digit()) => "格式不明的文字",
        "text" => "非數字文字",
        _ => "不支援的值",
    }
}
impl Prepared {
    pub fn apply(mut self, choices: &[Choice]) -> AppResult<Chart> {
        self.review.validate(choices)?;
        let mut removed = BTreeSet::new();
        let mut counts = [0usize; 3];
        let mut policies = BTreeMap::new();
        for (group, choice) in self.review.groups.iter().zip(choices) {
            for &(row, col) in &group.cells {
                policies.insert((row, col), choice);
                if group.x_axis {
                    removed.insert(row);
                    continue;
                }
                match choice {
                    Choice::Gap => counts[0] += 1,
                    Choice::Skip => {
                        self.chart.series[col - 1].skip_indices.push(row);
                        counts[1] += 1;
                    }
                    Choice::Zero => {
                        self.chart.series[col - 1].values[row] = Some(0.0);
                        counts[2] += 1;
                    }
                }
            }
        }
        self.chart.data_issues = self
            .issues
            .into_iter()
            .map(|(row, col, mut issue)| {
                if removed.contains(&row) {
                    issue.handling = "排除整列（X 軸無效，所有系列同步排除）".into();
                } else if let Some(choice) = policies.get(&(row, col)) {
                    issue.handling = match choice {
                        Choice::Gap => "保留缺值（折線中斷）",
                        Choice::Skip => "略過此點（保留 X 位置，折線接續）",
                        Choice::Zero => "設為 0",
                    }
                    .into();
                }
                issue
            })
            .collect();
        // 只有無法建立 X 座標時才刪整列；其他系列的有效 Y 不受某系列缺值影響。
        self.chart.x = self
            .chart
            .x
            .into_iter()
            .enumerate()
            .filter_map(|(i, x)| (!removed.contains(&i)).then_some(x))
            .collect();
        for series in &mut self.chart.series {
            let skipped: BTreeSet<_> = series.skip_indices.iter().copied().collect();
            series.skip_indices.clear();
            let mut values = Vec::new();
            for (old, value) in series.values.iter().copied().enumerate() {
                if removed.contains(&old) {
                    continue;
                }
                if skipped.contains(&old) {
                    series.skip_indices.push(values.len());
                }
                values.push(value);
            }
            series.values = values;
        }
        if self
            .chart
            .series
            .iter()
            .all(|s| s.values.iter().all(Option::is_none))
        {
            return Err("選取範圍在處理後沒有可繪製的數值，請重新選擇資料。".into());
        }
        self.chart.data_note = format!("數字文字轉換 {} 格；原空白 {} 格；異常值：缺值 {}、略過 {}、設零 {}；無效 X 排除 {} 列。", self.review.converted,self.review.blanks,counts[0],counts[1],counts[2],removed.len());
        self.chart.validate()?;
        Ok(self.chart)
    }
}

/// 一次掃完整個範圍，按欄位與類型彙整，避免逐格打斷使用者。
pub fn prepare(page: &Page, mut chart: Chart) -> AppResult<Prepared> {
    if page.columns.len() < 2 || page.columns.len() > 9 || page.headers.len() != page.columns.len()
    {
        return Err("Excel 圖表欄位結構不一致。".into());
    }
    chart.x.clear();
    let mut issues = Vec::new();
    chart.series = page
        .headers
        .iter()
        .enumerate()
        .skip(1)
        .map(|(i, h)| Series {
            name: if h.text.trim().is_empty() {
                page.columns[i].clone()
            } else {
                h.text.clone()
            },
            values: vec![],
            skip_indices: vec![],
        })
        .collect();
    let mut review = Review {
        title: chart.title.clone(),
        source: chart.source.clone(),
        converted: 0,
        blanks: 0,
        groups: vec![],
    };
    for (row_index, row) in page.rows.iter().enumerate() {
        if row.cells.len() != page.columns.len() {
            return Err("Excel 資料列與欄位數不一致。".into());
        }
        for (col, cell) in row.cells.iter().enumerate() {
            let number = if cell.kind == "number" {
                cell.value.as_f64().filter(|v| v.is_finite())
            } else if cell.kind == "text" {
                cell.value.as_str().and_then(numeric_text)
            } else {
                None
            };
            let numeric_axis = col > 0 || chart.kind == "scatter";
            let valid = if numeric_axis {
                number.is_some() || (col > 0 && cell.kind == "blank")
            } else {
                matches!(cell.kind.as_str(), "number" | "text")
                    && !cell.text.trim().is_empty()
                    && !cell.text.chars().all(|c| c == '#')
            };
            if numeric_axis && number.is_some() && cell.kind == "text" {
                review.converted += 1;
            }
            if col > 0 && cell.kind == "blank" {
                review.blanks += 1;
            }
            if col == 0 {
                chart.x.push(if chart.kind == "scatter" {
                    json!(number.unwrap_or(0.0))
                } else {
                    Value::String(cell.text.clone())
                });
            } else {
                chart.series[col - 1].values.push(number);
            }
            let converted = numeric_axis && number.is_some() && cell.kind == "text";
            let blank = col > 0 && cell.kind == "blank";
            if !valid || converted || blank {
                let issue = super::DataIssue {
                    sheet: page.sheet,
                    row: row.row,
                    cell: format!("{}{}", page.columns[col], row.row),
                    series: if col == 0 {
                        "X 軸".into()
                    } else {
                        chart.series[col - 1].name.clone()
                    },
                    x_value: row.cells[0].text.clone(),
                    y_value: if col == 0 {
                        row.cells
                            .iter()
                            .skip(1)
                            .map(|c| c.text.as_str())
                            .collect::<Vec<_>>()
                            .join(" / ")
                    } else {
                        cell.text.clone()
                    },
                    original_value: cell.value.clone(),
                    original_text: cell.text.clone(),
                    category: if converted {
                        "數字文字".into()
                    } else {
                        category(cell).into()
                    },
                    handling: if converted {
                        format!("轉為數值 {}", number.unwrap_or_default())
                    } else if blank {
                        "原空白保留缺值（折線中斷）".into()
                    } else {
                        String::new()
                    },
                };
                issues.push((row_index, col, issue));
            }
            if valid {
                continue;
            }
            let category = category(cell);
            let existing = review
                .groups
                .iter()
                .position(|g| g.column == page.columns[col] && g.category == category);
            let index = existing.unwrap_or_else(|| {
                review.groups.push(Group {
                    column: page.columns[col].clone(),
                    category: category.into(),
                    x_axis: col == 0,
                    count: 0,
                    samples: vec![],
                    cells: vec![],
                    choices: if col == 0 {
                        vec![Choice::Skip]
                    } else {
                        vec![Choice::Gap, Choice::Skip, Choice::Zero]
                    },
                });
                review.groups.len() - 1
            });
            let group = &mut review.groups[index];
            group.count += 1;
            group.cells.push((row_index, col));
            if group.samples.len() < 3 {
                group.samples.push(format!(
                    "{}{}：{}",
                    page.columns[col],
                    row.row,
                    cell.text.chars().take(60).collect::<String>()
                ));
            }
        }
    }
    Ok(Prepared {
        review,
        chart,
        issues,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decimal_text_never_guesses_units_or_locale() {
        for value in ["339", " 12.5 ", "-1.2e3", ".5"] {
            assert!(numeric_text(value).is_some());
        }
        for value in ["NG", "X", "1,234", "12 mm", "5%", "NaN", "inf", "1e999", ""] {
            assert!(numeric_text(value).is_none());
        }
    }
    pub fn fixture() -> Page {
        serde_json::from_value(json!({"sheet":1,"sheet_name":"s","columns":["A","F","G"],"header_row":1,"headers":[{"value":"ID","text":"ID","kind":"text"},{"value":"Y","text":"Y","kind":"text"},{"value":"Y2","text":"Y2","kind":"text"}],"start_row":2,"rows":[
            {"row":2,"cells":[{"value":"00123","text":"00123","kind":"text"},{"value":"339","text":"339","kind":"text"},{"value":2,"text":"2","kind":"number"}]},
            {"row":3,"cells":[{"value":"00124","text":"00124","kind":"text"},{"value":"NG","text":"NG","kind":"text"},{"value":3,"text":"3","kind":"number"}]},
            {"row":4,"cells":[{"value":"00125","text":"00125","kind":"text"},{"value":null,"text":"","kind":"blank"},{"value":4,"text":"4","kind":"number"}]}
        ],"next_row":null,"used_range":{"first_row":1,"last_row":4,"first_column":"A","last_column":"G"},"date_1904":false})).unwrap()
    }
    fn chart() -> Chart {
        serde_json::from_value(json!({"kind":"line","title":"t","x_label":"x","y_label":"y","x":[],"series":[],"source":"test"})).unwrap()
    }
    #[test]
    fn choices_preserve_blanks_coordinates_and_other_series() {
        for choice in [Choice::Gap, Choice::Skip, Choice::Zero] {
            let prepared = prepare(&fixture(), chart()).unwrap();
            assert_eq!(prepared.review.converted, 1);
            let result = prepared.apply(std::slice::from_ref(&choice)).unwrap();
            assert_eq!(result.x[0], "00123");
            assert_eq!(result.series[0].values[0], Some(339.0));
            assert_eq!(result.series[0].values[2], None);
            assert_eq!(result.series[1].values[1], Some(3.0));
            let issue = result.data_issues.iter().find(|i| i.cell == "F3").unwrap();
            assert_eq!(issue.row, 3);
            assert_eq!(issue.x_value, "00124");
            assert_eq!(issue.original_value, json!("NG"));
            assert!(issue.handling.contains(match choice {
                Choice::Gap => "保留缺值",
                Choice::Skip => "略過此點",
                Choice::Zero => "設為 0",
            }));
            assert!(result
                .data_issues
                .iter()
                .any(|i| i.cell == "F2" && i.handling == "轉為數值 339"));
            assert_eq!(
                result.series[0].values[1],
                if choice == Choice::Zero {
                    Some(0.0)
                } else {
                    None
                }
            );
            assert_eq!(
                result.series[0].skip_indices,
                if choice == Choice::Skip {
                    vec![1]
                } else {
                    vec![]
                }
            );
        }
    }
    #[test]
    fn invalid_x_requires_explicit_row_removal_and_alignment() {
        let mut page = fixture();
        page.rows[0].cells[0].kind = "error".into();
        let p = prepare(&page, chart()).unwrap();
        assert!(p.review.validate(&[Choice::Zero, Choice::Gap]).is_err());
        let c = p.apply(&[Choice::Skip, Choice::Gap]).unwrap();
        assert_eq!(c.x, vec![json!("00124"), json!("00125")]);
        assert_eq!(c.series[1].values, vec![Some(3.0), Some(4.0)]);
        assert!(c
            .data_issues
            .iter()
            .any(|i| i.row == 2 && i.cell == "A2" && i.handling.contains("排除整列")));
    }
}
