//! 僅供使用者調整呈現的設定；不混入模型 Chart 工具，也不更改原始點陣。
use super::Chart;
use crate::AppResult;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeriesStyle {
    pub name: String,
    pub color: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceLine {
    pub axis: String,
    pub value: f64,
    pub name: String,
    pub color: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Style {
    pub title: String,
    pub x_label: String,
    pub y_label: String,
    pub kind: String,
    pub legend: String,
    pub x_min: Option<f64>,
    pub x_max: Option<f64>,
    pub y_min: Option<f64>,
    pub y_max: Option<f64>,
    pub series: Vec<SeriesStyle>,
    pub lines: Vec<ReferenceLine>,
}
fn color(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].bytes().all(|b| b.is_ascii_hexdigit())
}
impl Style {
    pub fn validate(&self, chart: &Chart) -> AppResult<()> {
        chart.validate()?;
        if !super::KINDS.contains(&self.kind.as_str())
            || !["right", "bottom_right", "bottom", "top", "hidden"].contains(&self.legend.as_str())
        {
            return Err("圖表種類或圖例位置不支援。".into());
        }
        if self.kind == "scatter" && chart.x.iter().any(|v| v.as_f64().is_none()) {
            return Err("散佈圖需要原始數值X；不能把時間標籤當作數字。".into());
        }
        if [&self.title, &self.x_label, &self.y_label]
            .iter()
            .any(|s| s.chars().count() > 200)
            || self.title.trim().is_empty()
            || self.series.len() != chart.series.len()
            || self.lines.len() > 10
        {
            return Err("標題最多200字，系列需相符，參考線最多10條。".into());
        }
        let category = |axis: &str| {
            self.kind != "scatter" && ((self.kind == "horizontal_bar") == (axis == "y"))
        };
        let position = |axis: &str, v: f64| -> bool {
            v.is_finite()
                && (!category(axis) || (v.fract() == 0.0 && v >= 0.0 && v < chart.x.len() as f64))
        };
        for (axis, min, max) in [("x", self.x_min, self.x_max), ("y", self.y_min, self.y_max)] {
            if min.is_some_and(|v| !position(axis, v))
                || max.is_some_and(|v| !position(axis, v))
                || min.zip(max).is_some_and(|(a, b)| a >= b)
            {
                return Err("座標範圍無效：下限需小於上限；類別軸需使用有效資料位置。".into());
            }
        }
        for s in &self.series {
            if s.name.trim().is_empty() || s.name.chars().count() > 100 || !color(&s.color) {
                return Err("系列名稱最多100字，顏色需為#RRGGBB。".into());
            }
        }
        for line in &self.lines {
            if !["x", "y"].contains(&line.axis.as_str())
                || !position(&line.axis, line.value)
                || line.name.chars().count() > 100
                || !color(&line.color)
            {
                return Err("參考線座標、名稱或顏色無效。".into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn style_cannot_change_values_or_add_scripts_and_checks_axis_roles() {
        let (proposal, page) = crate::projects::excel_plan::tests::fixture();
        let chart = super::super::from_page(
            &page,
            "line",
            "量測",
            "時間",
            "透光值",
            &proposal.path,
            &proposal.revision,
        )
        .unwrap();
        let mut style:Style=serde_json::from_value(json!({"title":"量測","x_label":"時間","y_label":"透光值","kind":"line","legend":"right","x_min":null,"x_max":null,"y_min":140,"y_max":180,"series":[{"name":"透光值","color":"#008800"}],"lines":[{"axis":"y","value":170,"name":"上限","color":"#ff0000"}]})).unwrap();
        style.validate(&chart).unwrap();
        style.y_min = Some(200.0);
        assert!(style.validate(&chart).is_err());
        style.y_min = Some(140.0);
        style.lines[0].axis = "x".into();
        assert!(style.validate(&chart).is_err());
        style.lines[0].value = 0.0;
        style.validate(&chart).unwrap();
        style.kind = "scatter".into();
        assert!(style.validate(&chart).is_err());
        style.kind = "line".into();
        style.series[0].color = "url(script)".into();
        assert!(style.validate(&chart).is_err());
        let mut injected = serde_json::to_value(&style).unwrap();
        injected["values"] = json!([99]);
        assert!(serde_json::from_value::<Style>(injected).is_err());
        assert_eq!(chart.series[0].values, vec![Some(168.4)]);
    }
}
