//! 有限且可重現的圖表座標轉換。保留來源點陣，先篩列、再依保留列的共同序號轉換。
//! X／Y 指圖上的實體軸；水平長條圖的原始類別在 Y，量測值在 X。
use super::{Chart, Series};
use crate::AppResult;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Original,
    Offset,
    Index,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Axis {
    pub mode: Mode,
    pub offset: f64,
    pub start: f64,
    pub step: f64,
}
impl Default for Axis {
    fn default() -> Self {
        Self {
            mode: Mode::Original,
            offset: 0.0,
            start: 1.0,
            step: 1.0,
        }
    }
}
impl Axis {
    fn validate(&self) -> AppResult<()> {
        if ![self.offset, self.start, self.step]
            .iter()
            .all(|v| v.is_finite())
            || (self.mode == Mode::Index && self.step == 0.0)
        {
            return Err("座標轉換需使用有限數字，重新編號的間距不可為0。".into());
        }
        Ok(())
    }
    /// 缺值由呼叫者保留；重新編號不會把空白製造成量測值。
    fn number(&self, value: f64, row: usize) -> AppResult<f64> {
        let value = match self.mode {
            Mode::Original => value,
            Mode::Offset => value + self.offset,
            Mode::Index => self.start + self.step * row as f64,
        };
        if !value.is_finite() {
            return Err("座標轉換結果超出有限數值範圍，請減小位移、起點或間距。".into());
        }
        Ok(value)
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Transform {
    pub x: Axis,
    pub y: Axis,
    pub drop_empty: bool,
}

/// 呈現用副本；source_indices 保留每列在原圖的零起算位置，不把不同系列各自壓縮。
pub struct View {
    pub x: Vec<Value>,
    pub series: Vec<Series>,
    pub source_indices: Vec<usize>,
}
impl Transform {
    pub fn category(&self, kind: &str, axis: &str) -> bool {
        let base = if kind == "horizontal_bar" { "y" } else { "x" };
        if axis != base || kind == "scatter" {
            return false;
        }
        // 長條圖的類別位置仍等距；折線的衍生數字改用數值軸，平移才有實際座標意義。
        kind == "bar"
            || kind == "horizontal_bar"
            || (if axis == "x" { &self.x } else { &self.y }).mode == Mode::Original
    }

    pub fn view(&self, chart: &Chart, kind: &str) -> AppResult<View> {
        self.x.validate()?;
        self.y.validate()?;
        let source_indices: Vec<_> = (0..chart.x.len())
            .filter(|&i| !self.drop_empty || chart.series.iter().any(|s| s.values[i].is_some()))
            .collect();
        if source_indices.is_empty() {
            return Err("移除無值位置後沒有資料，請保留缺值或重新選取來源。".into());
        }
        let (base, measure) = if kind == "horizontal_bar" {
            (&self.y, &self.x)
        } else {
            (&self.x, &self.y)
        };
        let x = source_indices
            .iter()
            .enumerate()
            .map(|(row, &i)| {
                if base.mode == Mode::Original {
                    return Ok(chart.x[i].clone());
                }
                let original = if base.mode == Mode::Index {
                    0.0
                } else {
                    chart.x[i]
                        .as_f64()
                        .ok_or("文字／日期類別不能直接數值平移；請選重新編號或保留原值。")?
                };
                Ok(Value::from(base.number(original, row)?))
            })
            .collect::<AppResult<Vec<_>>>()?;
        if kind == "scatter" && x.iter().any(|v| v.as_f64().is_none()) {
            return Err("散佈圖需要數值座標；文字類別可先明確選擇重新編號。".into());
        }
        let series = chart
            .series
            .iter()
            .map(|series| {
                let values = source_indices
                    .iter()
                    .enumerate()
                    .map(|(row, &i)| {
                        series.values[i]
                            .map(|value| measure.number(value, row))
                            .transpose()
                    })
                    .collect::<AppResult<Vec<_>>>()?;
                let skip_indices = source_indices
                    .iter()
                    .enumerate()
                    .filter_map(|(row, i)| {
                        series.skip_indices.binary_search(i).is_ok().then_some(row)
                    })
                    .collect();
                Ok(Series {
                    name: series.name.clone(),
                    values,
                    skip_indices,
                })
            })
            .collect::<AppResult<Vec<_>>>()?;
        Ok(View {
            x,
            series,
            source_indices,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> Chart {
        serde_json::from_value(json!({"kind":"line","title":"t","source":"CSV","x_label":"Index","y_label":"Y",
            "x":[7001,7002,7003,7004,7005],"series":[{"name":"A","values":[10,null,20,null,0],"skip_indices":[1]},
            {"name":"B","values":[null,null,30,40,null]}]})).unwrap()
    }
    #[test]
    fn drop_then_index_keeps_other_series_alignment_zero_and_source() {
        let chart = fixture();
        let mut transform = Transform {
            drop_empty: true,
            ..Default::default()
        };
        transform.x.mode = Mode::Index;
        transform.y.mode = Mode::Offset;
        transform.y.offset = -10.0;
        let view = transform.view(&chart, "line").unwrap();
        assert_eq!(view.source_indices, vec![0, 2, 3, 4]);
        assert_eq!(view.x, vec![json!(1.0), json!(2.0), json!(3.0), json!(4.0)]);
        assert_eq!(
            view.series[0].values,
            vec![Some(0.0), Some(10.0), None, Some(-10.0)]
        );
        assert_eq!(
            view.series[1].values,
            vec![None, Some(20.0), Some(30.0), None]
        );
        assert!(view.series[0].skip_indices.is_empty());
        assert_eq!(chart.x[0], 7001);
        assert_eq!(chart.series[0].values[0], Some(10.0));
    }
    #[test]
    fn both_axes_reindex_by_shared_row_and_horizontal_roles_are_explicit() {
        let chart = fixture();
        let mut transform = Transform::default();
        transform.y.mode = Mode::Index;
        transform.y.start = 5.0;
        transform.y.step = -2.0;
        let vertical = transform.view(&chart, "line").unwrap();
        assert_eq!(
            vertical.series[0].values,
            vec![Some(5.0), None, Some(1.0), None, Some(-3.0)]
        );
        assert_eq!(vertical.series[0].skip_indices, vec![1]);
        transform.x.mode = Mode::Offset;
        transform.x.offset = 100.0;
        let horizontal = transform.view(&chart, "horizontal_bar").unwrap();
        assert_eq!(horizontal.x[0], 5.0);
        assert_eq!(horizontal.series[0].values[0], Some(110.0));
        assert!(transform.category("horizontal_bar", "y"));
    }
    #[test]
    fn offset_preserves_numeric_spacing_and_rejects_text_overflow_empty() {
        let mut chart = fixture();
        let mut transform = Transform::default();
        transform.x.mode = Mode::Offset;
        transform.x.offset = -7000.0;
        assert_eq!(transform.view(&chart, "line").unwrap().x[4], 5.0);
        assert!(!transform.category("line", "x"));
        chart.x[0] = json!("007001");
        assert!(transform.view(&chart, "line").is_err());
        transform.x.mode = Mode::Index;
        assert!(transform.view(&chart, "scatter").is_ok());
        transform.x.start = f64::MAX;
        transform.x.step = f64::MAX;
        assert!(transform.view(&chart, "line").is_err());
        transform.x = Axis::default();
        transform.drop_empty = true;
        for series in &mut chart.series {
            series.values.fill(None);
        }
        assert!(transform.view(&chart, "line").is_err());
    }
    #[test]
    fn locked_excel_csv_keeps_source_columns_and_allows_derived_index() {
        use crate::projects::{
            datasets::Table,
            excel_plan::{self, Plan},
        };
        let (mut proposal, mut page) = excel_plan::tests::fixture();
        proposal.x.column = "A".into();
        proposal.x.header = "Index".into();
        proposal.time = None;
        proposal.time_mode = "none".into();
        page.columns[0] = "A".into();
        page.headers[0].text = "Index".into();
        page.headers[0].value = json!("Index");
        page.rows[0].cells[0].value = json!(7001);
        page.rows[0].cells[0].text = "7001".into();
        page.rows[0].cells[0].number_format = "0".into();
        let plan = Plan::create(proposal, &page).unwrap();
        let mut table = Table::from_excel(&page, "a.xlsx", "excel:r").unwrap();
        table.excel.as_mut().unwrap().plan = Some(plan);
        assert!(table
            .page("序列", &["D".into()], 1, 1)
            .err()
            .unwrap()
            .contains("transform_chart"));
        let page = table.page("A", &["D".into()], 1, 1).unwrap();
        let chart = super::super::from_page(
            &page,
            "scatter",
            "圖樣Mean值",
            "Index",
            "Mean",
            "量測.csv",
            "r",
        )
        .unwrap();
        let mut transform = Transform::default();
        transform.x.mode = Mode::Index;
        let view = transform.view(&chart, "scatter").unwrap();
        assert_eq!(view.x[0], 1.0);
        assert_eq!(chart.x[0], 7001.0);
        assert_eq!(view.series[0].values[0], Some(168.4));
    }
}
