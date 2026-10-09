//! 使用者呈現設定；有限座標轉換與 AI 共用規則，不更改原始點陣。
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

/// 圖框內左上角的比例座標；不綁定螢幕像素，也不代表資料座標。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

/// 字型採固定清單，避免把任意CSS或腳本當成文字樣式；缺少字型時由系統備援。
pub const FONTS: &[&str] = &[
    "sans-serif",
    "Microsoft JhengHei",
    "PMingLiU",
    "DFKai-SB",
    "Arial",
    "Times New Roman",
    "Consolas",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Annotation {
    pub text: String,
    pub position: Position,
    pub font_family: String,
    pub font_size: u16,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub color: String,
    pub background: Option<String>,
}

/// 只有使用者操作的排版會保存；缺省項目每次依圖框自動定位。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Layout {
    pub title: Option<Position>,
    pub legend: Option<Position>,
    pub reference_labels: Vec<Option<Position>>,
    pub annotations: Vec<Annotation>,
}
impl Layout {
    fn validate(&self, line_count: usize) -> AppResult<()> {
        if self.reference_labels.len() > line_count || self.reference_labels.len() > 10 {
            return Err("參考線文字位置與參考線數量不符。".into());
        }
        if self.annotations.len() > 20 {
            return Err("圖中文字最多20則。".into());
        }
        for annotation in &self.annotations {
            if annotation.text.trim().is_empty()
                || annotation.text.chars().count() > 500
                || !FONTS.contains(&annotation.font_family.as_str())
                || !(8..=72).contains(&annotation.font_size)
                || !color(&annotation.color)
                || annotation
                    .background
                    .as_ref()
                    .is_some_and(|value| !color(value))
            {
                return Err(
                    "圖中文字需為1–500字、字級8–72、清單內字型，字色與底色為#RRGGBB。".into(),
                );
            }
        }
        for position in self
            .title
            .iter()
            .chain(self.legend.iter())
            .chain(self.reference_labels.iter().flatten())
            .chain(
                self.annotations
                    .iter()
                    .map(|annotation| &annotation.position),
            )
        {
            if !position.x.is_finite()
                || !position.y.is_finite()
                || !(0.0..=1.0).contains(&position.x)
                || !(0.0..=1.0).contains(&position.y)
            {
                return Err("圖表文字位置必須在圖框內。".into());
            }
        }
        Ok(())
    }
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<super::transform::Transform>,
    #[serde(default)]
    pub quality_policy: super::quality::Policy,
    #[serde(default)]
    pub layout: Layout,
}
fn color(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].bytes().all(|b| b.is_ascii_hexdigit())
}
/// 共用 AI 與編輯器的參考線界線；數值使用目前圖上的實體 X／Y，類別軸使用位置。
pub(super) fn validate_lines(
    lines: &[ReferenceLine],
    kind: &str,
    transform: &super::transform::Transform,
    positions: usize,
) -> AppResult<()> {
    if lines.len() > 10 {
        return Err("參考線最多10條，請選擇重要切換點或分圖；未自動省略。".into());
    }
    for line in lines {
        let category = transform.category(kind, &line.axis);
        if !["x", "y"].contains(&line.axis.as_str())
            || !line.value.is_finite()
            || (category
                && (line.value.fract() != 0.0
                    || line.value < 0.0
                    || line.value >= positions as f64))
            || line.name.chars().count() > 100
            || !color(&line.color)
        {
            return Err("參考線座標、名稱或顏色無效；類別軸使用從0起算的有效位置。".into());
        }
    }
    Ok(())
}
impl Style {
    /// 與前端初始樣式一致；AI改圖先讀取這份設定，避免重建原始數據。
    pub fn defaults(chart: &Chart, palette: &[String]) -> Self {
        let horizontal = chart.kind == "horizontal_bar";
        Self {
            title: chart.title.clone(),
            kind: chart.kind.clone(),
            legend: "right".into(),
            x_label: if horizontal {
                &chart.y_label
            } else {
                &chart.x_label
            }
            .clone(),
            y_label: if horizontal {
                &chart.x_label
            } else {
                &chart.y_label
            }
            .clone(),
            x_min: None,
            x_max: None,
            y_min: None,
            y_max: None,
            series: chart
                .series
                .iter()
                .enumerate()
                .map(|(i, s)| SeriesStyle {
                    name: s.name.clone(),
                    color: palette.get(i).cloned().unwrap_or_else(|| "#5470c6".into()),
                })
                .collect(),
            lines: chart.reference_lines.clone(),
            layout: Layout::default(),
            transform: Some(chart.transform.as_deref().cloned().unwrap_or_default()),
            quality_policy: Default::default(),
        }
    }
    pub fn validate(&self, chart: &Chart) -> AppResult<()> {
        chart.validate()?;
        self.validate_view(chart)
    }

    /// Chart本身驗證完成後使用，避免AI樣式與圖表之間遞迴驗證。
    pub(super) fn validate_view(&self, chart: &Chart) -> AppResult<()> {
        if !super::KINDS.contains(&self.kind.as_str())
            || !["right", "bottom_right", "bottom", "top", "hidden"].contains(&self.legend.as_str())
        {
            return Err("圖表種類或圖例位置不支援。".into());
        }
        let transform = self
            .transform
            .as_ref()
            .or(chart.transform.as_deref())
            .cloned()
            .unwrap_or_default();
        let source = self.quality_policy.view(chart)?;
        let view = transform.view(&source, &self.kind)?;
        if [&self.title, &self.x_label, &self.y_label]
            .iter()
            .any(|s| s.chars().count() > 200)
            || self.title.trim().is_empty()
            || self.series.len() != chart.series.len()
            || self.lines.len() > 10
        {
            return Err("標題最多200字，系列需相符，參考線最多10條。".into());
        }
        let category = |axis: &str| transform.category(&self.kind, axis);
        let position = |axis: &str, v: f64| -> bool {
            v.is_finite()
                && (!category(axis) || (v.fract() == 0.0 && v >= 0.0 && v < view.x.len() as f64))
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
        validate_lines(&self.lines, &self.kind, &transform, view.x.len())?;
        self.layout.validate(self.lines.len())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn annotations_validate_without_accepting_html_options_or_losing_legacy_layout() {
        let mut layout:Layout=serde_json::from_value(json!({"annotations":[{
            "text":"區間1\n第二行", "position":{"x":0.4,"y":0.3}, "font_family":"Microsoft JhengHei",
            "font_size":18,"bold":true,"italic":true,"underline":true,"color":"#123456","background":"#ffffff"
        }]})).unwrap();
        layout.validate(0).unwrap();
        let restored: Layout = serde_json::from_value(json!(layout)).unwrap();
        assert!(restored.annotations[0].underline);
        layout.annotations[0].font_family = "url(script)".into();
        assert!(layout.validate(0).is_err());
        layout.annotations[0].font_family = "Arial".into();
        layout.annotations[0].position.x = -0.1;
        assert!(layout.validate(0).is_err());
        layout.annotations[0].position.x = 0.5;
        layout.annotations[0].font_size = 73;
        assert!(layout.validate(0).is_err());
        layout.annotations[0].font_size = 18;
        layout.annotations = vec![layout.annotations[0].clone(); 21];
        assert!(layout.validate(0).is_err());
        assert!(serde_json::from_value::<Layout>(json!({"formatter":"run()"})).is_err());
    }
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
        assert!(style.layout.title.is_none()); // 舊圖表沒有layout仍可載入。
        style.layout.title = Some(Position { x: 0.2, y: 0.1 });
        style.layout.reference_labels = vec![Some(Position { x: 0.4, y: 0.3 })];
        let saved = serde_json::to_string(&style).unwrap();
        serde_json::from_str::<Style>(&saved)
            .unwrap()
            .validate(&chart)
            .unwrap();
        style.layout.title.as_mut().unwrap().x = 1.1;
        assert!(style.validate(&chart).is_err());
        style.layout.title.as_mut().unwrap().x = f64::NAN;
        assert!(style.validate(&chart).is_err());
        style.layout = Layout::default();
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
