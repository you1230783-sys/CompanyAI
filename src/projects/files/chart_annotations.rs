//! AI與使用者共用圖表樣式；歷史圖另建版本，不修改來源點陣或舊訊息。
use super::*;

impl Broker {
    pub(in crate::projects) fn set_chart_history(
        &mut self,
        messages: &[crate::protocol::Message],
        palette: &[String],
    ) {
        self.chart_palette = palette.to_vec();
        self.previous_charts = messages
            .iter()
            .enumerate()
            .filter_map(|(index, message)| {
                if message.project_charts.is_empty() {
                    return None;
                }
                let mut charts = message.project_charts.clone();
                for (chart_index, style) in &message.project_chart_styles {
                    if let Some(chart) = charts.get_mut(*chart_index) {
                        chart.style = Some(Box::new(style.clone()));
                    }
                }
                Some((index, charts))
            })
            .collect();
    }

    fn chart_at(
        &self,
        message: Option<usize>,
        index: usize,
    ) -> AppResult<&super::super::charts::Chart> {
        let charts = match message {
            Some(message) => self
                .previous_charts
                .get(&message)
                .ok_or("找不到此對話中的圖表訊息，請先inspect_chart查索引。")?,
            None => &self.charts,
        };
        charts
            .get(index)
            .ok_or("找不到圖表編號，請先inspect_chart查索引。".into())
    }

    pub(super) fn inspect_chart(
        &self,
        message: Option<usize>,
        index: Option<usize>,
    ) -> AppResult<Value> {
        if let Some(index) = index {
            let chart = self.chart_at(message, index)?;
            chart.validate()?;
            let style = chart.style.as_deref().cloned().unwrap_or_else(|| {
                super::super::charts::style::Style::defaults(chart, &self.chart_palette)
            });
            return Ok(
                json!({"message_index":message,"chart_index":index,"style":style,
                "rows":chart.x.len(),"source":chart.source,"source_preserved":true}),
            );
        }
        let describe = |message: Option<usize>,
                        index: usize,
                        chart: &super::super::charts::Chart| {
            json!({"message_index":message,"chart_index":index,"title":chart.style.as_ref().map_or(&chart.title, |s|&s.title),"rows":chart.x.len()})
        };
        Ok(
            json!({"current":self.charts.iter().enumerate().map(|(i,c)|describe(None,i,c)).collect::<Vec<_>>(),
            "previous":self.previous_charts.iter().filter(|(m,_)|message.is_none_or(|n|n==**m)).flat_map(|(m,charts)|charts.iter().enumerate().map(move |(i,c)|describe(Some(*m),i,c))).collect::<Vec<_>>(),
            "notice":"歷史圖以message_index定位；編輯後另建本輪圖表，舊訊息保留。指定chart_index查完整樣式。"}),
        )
    }

    pub(super) fn edit_chart(
        &mut self,
        message: Option<usize>,
        index: usize,
        style: &super::super::charts::style::Style,
    ) -> AppResult<Value> {
        let original = self.chart_at(message, index)?;
        style.validate(original)?;
        let mut candidate = original.clone();
        candidate.style = Some(Box::new(style.clone()));
        let index = if message.is_some() {
            self.add_chart(candidate)?;
            self.charts.len() - 1
        } else {
            self.replace_chart(index, candidate)?;
            index
        };
        Ok(
            json!({"chart_index":index,"message_index":null,"style_revision":text::revision(&serde_json::to_string(style).map_err(|e|e.to_string())?),"source_preserved":true,
            "previous_png_files_unchanged":true,"notice":"圖表設定已套用；需要新版PNG時請再次匯出。"}),
        )
    }

    /// 完整驗證候選版本與總大小，成功才替換；失敗不能留下半套樣式。
    fn replace_chart(
        &mut self,
        index: usize,
        candidate: super::super::charts::Chart,
    ) -> AppResult<()> {
        candidate.validate()?;
        let original = self.charts.get(index).ok_or("找不到本輪圖表。")?;
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
            return Err("本次圖表資料合計超過16 MiB，未套用設定。".into());
        }
        self.charts[index] = candidate;
        Ok(())
    }
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
        if let Some(style) = candidate.style.as_mut() {
            // 改過圖型的呈現座標可能與來源不同，只驗證目前樣式的參考線。
            style.lines = lines.to_vec();
            style.layout.reference_labels.clear();
        } else {
            candidate.reference_lines = lines.to_vec();
        }
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
    fn ai_edits_historical_user_style_as_a_new_chart_and_rejects_invalid_updates() {
        let root = std::env::current_dir()
            .unwrap()
            .join(".build")
            .join(format!("chart-style-{}", crate::jobs::new_id().unwrap()));
        fs::create_dir_all(&root).unwrap();
        let project = Project {
            id: "style-test".into(),
            name: "style-test".into(),
            root: root.clone(),
            imports: BTreeMap::new(),
        };
        let mut broker = Broker::new(project, "style-test".into()).unwrap();
        let chart:super::super::super::charts::Chart=serde_json::from_value(json!({"kind":"line","title":"原圖","x_label":"秒","y_label":"量測","x":[0,200,500],"series":[{"name":"Y","values":[90,100,110]}],"source":"fixture"})).unwrap();
        let mut message = crate::protocol::Message::assistant("原回答".into());
        let mut style = super::super::super::charts::style::Style::defaults(
            &chart,
            &crate::config::default_chart_palette(),
        );
        style.title = "使用者修改的標題".into();
        style.series[0].color = "#112233".into();
        message.project_charts.push(chart.clone());
        message.project_chart_styles.insert(0, style.clone());
        broker.set_chart_history(&[message], &crate::config::default_chart_palette());
        let read = broker.inspect_chart(Some(0), Some(0)).unwrap();
        assert_eq!(read["style"]["title"], style.title);
        assert!(read.get("x").is_none());
        style.title = "AI新版本".into();
        assert_eq!(
            broker.edit_chart(Some(0), 0, &style).unwrap()["chart_index"],
            0
        );
        assert_eq!(broker.charts[0].series[0].values, chart.series[0].values);
        assert_eq!(
            broker.inspect_chart(Some(0), Some(0)).unwrap()["style"]["title"],
            "使用者修改的標題"
        );
        let saved = broker.saved().unwrap();
        broker.restore(saved, &AtomicBool::new(false)).unwrap();
        assert_eq!(
            broker.inspect_chart(None, Some(0)).unwrap()["style"]["series"][0]["color"],
            "#112233"
        );
        style.series[0].color = "url(script)".into();
        assert!(broker.edit_chart(None, 0, &style).is_err());
        assert_eq!(
            broker.charts[0].style.as_ref().unwrap().series[0].color,
            "#112233"
        );
        // 折線圖轉為水平長條後，X參考線改用量測值；不能再套用原折線圖的類別位置。
        style.series[0].color = "#112233".into();
        style.kind = "horizontal_bar".into();
        broker.edit_chart(None, 0, &style).unwrap();
        let lines =
            serde_json::from_value::<Vec<super::super::super::charts::style::ReferenceLine>>(
                json!([{"axis":"x","value":100,"name":"基準","color":"#112233"}]),
            )
            .unwrap();
        broker.set_reference_lines(0, &lines).unwrap();
        assert!(broker.charts[0].reference_lines.is_empty());
        assert_eq!(
            broker.charts[0].style.as_ref().unwrap().lines[0].value,
            100.0
        );
        fs::remove_dir(root).unwrap();
    }
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
