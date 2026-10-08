//! 只更新既有圖表的參考線，不接受模型重傳或修改來源點陣。
use super::*;

impl Broker {
    pub(super) fn set_reference_lines(
        &mut self,
        index: usize,
        lines: &[super::super::charts::style::ReferenceLine],
    ) -> AppResult<Value> {
        let original = self
            .charts
            .get(index)
            .ok_or("找不到本次圖表，請先建立圖表。")?;
        let mut candidate = original.clone();
        candidate.reference_lines = lines.to_vec();
        candidate.validate()?;
        let size = |value: &super::super::charts::Chart| {
            serde_json::to_vec(value)
                .map(|v| v.len())
                .map_err(|e| e.to_string())
        };
        let total = serde_json::to_vec(&self.charts)
            .map_err(|e| e.to_string())?
            .len()
            - size(original)?
            + size(&candidate)?;
        if total > 16 * 1024 * 1024 {
            return Err("本次圖表資料合計超過16 MiB，未加入參考線。".into());
        }
        self.charts[index] = candidate;
        Ok(
            json!({"chart_index":index,"reference_lines":lines,"source_preserved":true,
            "previous_png_files_unchanged":true,"notice":"參考線已套用；需要PNG時請重新匯出。"}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reference_lines_keep_values_and_survive_serialization() {
        let mut chart: super::super::super::charts::Chart = serde_json::from_value(json!({
            "kind":"scatter","title":"參數比較","x_label":"秒","y_label":"量測",
            "x":[0,200,500],"series":[{"name":"Y","values":[1,2,3]}],"source":"fixture"
        }))
        .unwrap();
        let original = chart.clone();
        chart.reference_lines = serde_json::from_value(json!([
            {"axis":"x","value":200,"name":"參數1→2","color":"#d62728"},
            {"axis":"x","value":500,"name":"參數2→3","color":"#5470c6"}
        ]))
        .unwrap();
        chart.validate().unwrap();
        let restored: super::super::super::charts::Chart =
            serde_json::from_value(json!(chart)).unwrap();
        assert_eq!(restored.x, original.x);
        assert_eq!(restored.series[0].values, original.series[0].values);
        assert_eq!(restored.reference_lines.len(), 2);
        let mut invalid = chart.clone();
        invalid.reference_lines[0].axis = "z".into();
        assert!(invalid.validate().is_err());
        invalid = chart.clone();
        invalid.reference_lines = vec![chart.reference_lines[0].clone(); 11];
        assert!(invalid.validate().is_err());
        invalid = chart.clone();
        invalid.kind = "line".into();
        // 類別軸採位置，不能把秒數200當成第200筆資料。
        assert!(invalid.validate().is_err());
        invalid.reference_lines[0].value = 1.0;
        invalid.reference_lines[1].value = 2.0;
        invalid.validate().unwrap();
        chart.reference_lines[0].color = "url(script)".into();
        assert!(chart.validate().is_err());
    }
}
