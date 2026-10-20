//! 呈現政策只作用於原生預檢標記的缺值位置。有效的 0、其他系列及原始明細不變。
use super::{Chart, Choice};
use crate::AppResult;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    /// None 表示沿用建立圖表時的決策，供舊圖與分組政策相容。
    pub blank: Option<Choice>,
    pub invalid: Option<Choice>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cell {
    pub row: usize,
    pub series: usize,
    pub issue: usize,
    pub blank: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub cells: Vec<Cell>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_answer: Option<String>,
}
impl Source {
    pub fn validate(&self, chart: &Chart) -> AppResult<()> {
        let mut seen = std::collections::BTreeSet::new();
        for cell in &self.cells {
            if cell.row >= chart.x.len()
                || cell.series >= chart.series.len()
                || cell.issue >= chart.data_issues.len()
                || !seen.insert((cell.row, cell.series))
            {
                return Err("圖表缺值來源索引不一致。".into());
            }
        }
        Ok(())
    }
}
pub fn label(choice: &Choice) -> &'static str {
    match choice {
        Choice::Gap => "保留缺值（折線中斷）",
        Choice::Skip => "略過此點（保留 X 位置，折線接續）",
        Choice::Zero => "設為 0",
    }
}
impl Policy {
    pub fn view(&self, original: &Chart) -> AppResult<Chart> {
        let mut chart = original.clone();
        let Some(source) = &original.quality else {
            return Ok(chart);
        };
        source.validate(original)?;
        for cell in &source.cells {
            let choice = if cell.blank {
                &self.blank
            } else {
                &self.invalid
            };
            let Some(choice) = choice else {
                continue;
            };
            let series = &mut chart.series[cell.series];
            // 這些位置在原始資料中都不是數值，因此可從先前的設零還原缺值。
            series.values[cell.row] = if *choice == Choice::Zero {
                Some(0.0)
            } else {
                None
            };
            series.skip_indices.retain(|i| *i != cell.row);
            if *choice == Choice::Skip {
                series.skip_indices.push(cell.row);
            }
            chart.data_issues[cell.issue].handling = label(choice).into();
        }
        for series in &mut chart.series {
            series.skip_indices.sort_unstable();
        }
        if self.blank.is_some() || self.invalid.is_some() {
            let describe =
                |choice: &Option<Choice>| choice.as_ref().map(label).unwrap_or("沿用建立時處理");
            chart.data_note = format!(
                "Y 空白：{}；Y 異常：{}。原值與來源明細保留。",
                describe(&self.blank),
                describe(&self.invalid)
            );
        }
        Ok(chart)
    }
}
