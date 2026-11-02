//! 成果的進度指紋只使用原生內容與來源版本。輸出名稱／隨機操作ID不代表新成果。
//! 此資訊只影響停滯判定，不回傳舊工具結果、不攔截必要寫入或取消原本的核對。
use super::*;

/// 只忽略別名與輸出檔名；來源版本、區間、具名結果與輸出資料均參與比較。
pub(super) fn calculation_identity(sources: &[Value], response: &Value) -> Option<String> {
    let sources: Vec<_> = sources
        .iter()
        .map(|source| {
            let mut source = source.clone();
            if let Some(object) = source.as_object_mut() {
                object.remove("name");
            }
            source
        })
        .collect();
    let mut outputs = Vec::new();
    for output in response["outputs"].as_array()? {
        outputs.push(match output["kind"].as_str()? {
            "table" => json!(["table", output["columns"], output["rows"]]),
            "xlsx" => json!(["xlsx", output["content_sha256"].as_str()?]),
            _ => return None,
        });
    }
    // stdout是顯示用紀錄；計算結果必須寫入summary，時間戳不能使相同資料變成新進度。
    Some(text::revision(
        &json!(["calculation", sources, response["summary"], outputs]).to_string(),
    ))
}
impl Broker {
    fn metric_identity(&self, id: &str) -> Option<Value> {
        let metric = self.report_values.metrics.get(id)?;
        Some(json!([
            metric.spec.pointer,
            metric.spec.decimals,
            metric.spec.unit,
            metric.value,
            metric.sources
        ]))
    }
    pub(super) fn semantic_progress(&self, tool: &Tool, result: &Value) -> Option<String> {
        if result["ok"] != true
            || result["result"]["executed"] == false
            || result["result"]["status"] == "FAILED"
        {
            return None;
        }
        let data = &result["result"];
        let identity = match tool {
            Tool::RunPython { .. } => {
                if let Some(identity) = data["progress_identity"].as_str() {
                    return Some(identity.into());
                }
                // 原始檔路徑與區間必須保留；只移除輸出成果的命名，保留其位元組／資料版本。
                let outputs: Vec<_> = data["artifacts"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|a| json!([a["revision"], a["sha256"], a["rows"], a["columns"]]))
                    .collect();
                if data["sources"].is_null() || data["summary"].is_null() {
                    return None;
                }
                json!([
                    "calculation",
                    data["sources"],
                    data["summary"],
                    data["stdout"],
                    outputs
                ])
            }
            Tool::CreateMermaid { source, .. } => json!(["mermaid-source", source]),
            Tool::ExportMermaid {
                message_index,
                diagram_index,
                format,
            } => {
                let diagram = match message_index {
                    Some(index) => self.previous_diagrams.get(index)?.get(*diagram_index)?,
                    None => self.diagrams.get(*diagram_index)?,
                };
                json!(["mermaid-export", diagram.revision, format])
            }
            Tool::ExportChartPng { chart_index, .. } => {
                json!(["chart-png", self.charts.get(*chart_index)?])
            }
            Tool::CreateChart { .. } | Tool::ChartDataset { .. } => {
                // 只對本次確實回報的圖表建立指紋，不能誤用上一次圖表。
                let index = data["chart_index"].as_u64()? as usize;
                json!(["chart", self.charts.get(index)?])
            }
            Tool::RegisterReportValues { values } => {
                let values = values
                    .iter()
                    .map(|spec| self.metric_identity(&spec.id))
                    .collect::<Option<Vec<_>>>()?;
                json!(["registered-values", values])
            }
            Tool::ExportReportValues { metric_ids, .. } => {
                let values = metric_ids
                    .iter()
                    .map(|id| self.metric_identity(id))
                    .collect::<Option<Vec<_>>>()?;
                json!(["exported-values", values])
            }
            Tool::ApplyReportValues { copy_id, .. } => {
                let copy = self.copies.get(copy_id)?;
                let bindings = self
                    .report_values
                    .blocks
                    .iter()
                    .filter(|bound| &bound.copy_id == copy_id)
                    .map(|bound| {
                        Some(json!([
                            bound.block_id,
                            bound.expected,
                            bound.numeric,
                            self.metric_identity(&bound.metric_id)?
                        ]))
                    })
                    .collect::<Option<Vec<_>>>()?;
                json!([
                    "bound-report",
                    copy_id,
                    text::revision(&copy.text),
                    bindings
                ])
            }
            _ => return None,
        };
        Some(text::revision(&identity.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calculation_names_and_logs_do_not_hide_identical_data() {
        let sources =
            vec![json!({"name":"input","path":"data.csv","revision":"one","scope":"all"})];
        let first = json!({"summary":{"mean":2},"stdout":"12:00", "outputs":[
            {"kind":"table","name":"a.csv","columns":["mean"],"rows":[[2]]},
            {"kind":"xlsx","name":"a.xlsx","hex":"zip timestamp one","content_sha256":"cells-one"}]});
        let mut second = first.clone();
        second["outputs"][0]["name"] = json!("b.csv");
        second["outputs"][1]["hex"] = json!("zip timestamp two");
        second["stdout"] = json!("13:00");
        let identity = calculation_identity(&sources, &first);
        assert_eq!(identity, calculation_identity(&sources, &second));
        second["outputs"][0]["rows"][0][0] = json!(3);
        assert_ne!(identity, calculation_identity(&sources, &second));
        let mut changed = sources.clone();
        changed[0]["revision"] = json!("two");
        assert_ne!(identity, calculation_identity(&changed, &first));
        second = first.clone();
        second["outputs"][1]
            .as_object_mut()
            .unwrap()
            .remove("content_sha256");
        assert!(calculation_identity(&sources, &second).is_none());
    }
}
