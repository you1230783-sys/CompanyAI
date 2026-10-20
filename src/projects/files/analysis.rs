//! 將分析結論綁定既有操作；不允許模型自行提供「原文」或通過旗標。
use super::*;
use crate::projects::analysis;

impl Broker {
    pub(super) fn record_log_events(
        &mut self,
        events: &[super::super::timeline::Event],
    ) -> AppResult<Value> {
        if events.is_empty() || events.len() > 50 {
            return Err("每次需提交1–50筆事件。".into());
        }
        let mut timeline = self.analysis.timeline.clone();
        for event in events {
            let (request, result) = self
                .recorded_operation(&event.evidence.operation_id)?
                .ok_or("事件證據不存在於本次任務。")?;
            let row = super::super::timeline::from_evidence(event, &request, &result)?;
            if let Some(old) = timeline.iter_mut().find(|old| old["id"] == row["id"]) {
                *old = row;
            } else {
                timeline.push(row);
            }
        }
        if timeline.len() > 500
            || serde_json::to_vec(&timeline)
                .map_err(|e| e.to_string())?
                .len()
                > 1_500_000
        {
            return Err("事件時間表超過500筆或1.5MB，請縮小調查範圍。".into());
        }
        super::super::timeline::reconcile(&mut timeline);
        let attention = timeline
            .iter()
            .filter(|e| !matches!(e["pair_status"].as_str(), Some("paired" | "not_applicable")))
            .count();
        // 僅回傳有界的待查索引，完整原文留在本機面板及既有操作紀錄。
        let unresolved: Vec<_> = timeline
            .iter()
            .filter(|e| !matches!(e["pair_status"].as_str(), Some("paired" | "not_applicable")))
            .take(20)
            .map(|e| {
                json!({"label":e["label"],"group":e["group"],"path":e["path"],
                "revision":e["revision"],"line":e["line"],"status":e["pair_status"]})
            })
            .collect();
        self.analysis.timeline = timeline;
        Ok(
            json!({"events":self.analysis.timeline.len(),"needs_review":attention,"attention":unresolved,"attention_truncated":attention>20,"notice":"已保存原文事件；AI分類與時間先後均不代表已證明原因。"}),
        )
    }

    pub(super) fn record_analysis(&mut self, report: &analysis::Report) -> AppResult<Value> {
        report.validate()?;
        let mut findings = Vec::new();
        let mut excerpt_bytes = 0usize;
        for finding in &report.findings {
            let mut evidence = Vec::new();
            for reference in &finding.evidence {
                let (request, result) = self
                    .recorded_operation(&reference.operation_id)?
                    .ok_or("證據操作不存在於本次任務，請先讀取原文。")?;
                let tool = request["tool"].as_str().unwrap_or("");
                if !matches!(
                    tool,
                    "read_file"
                        | "read_document_section"
                        | "read_log"
                        | "search_logs"
                        | "read_excel_range"
                        | "inspect_dataset"
                        | "run_python"
                        | "outlook_read"
                ) {
                    return Err(
                        "請引用原文閱讀、資料集或 Python 計算結果；清單與筆記不能當作原文證據。"
                            .into(),
                    );
                }
                let excerpt = analysis::select(&result, &reference.pointer)?;
                excerpt_bytes += excerpt.to_string().len();
                if excerpt_bytes > 32_000 {
                    return Err("分析證據總量超過 32000 bytes，請縮小引用區間。".into());
                }
                let data = &result["result"];
                if data["content_read"] == false {
                    return Err("此操作沒有讀取內容，不能用作證據。".into());
                }
                let snapshot_only = tool == "inspect_dataset"
                    || data["working_copy"] == true
                    || data["imported_snapshot"] == true
                    || request["path"].as_str().is_some_and(|path| {
                        self.copies.contains_key(path)
                            || path.replace('\\', "/").starts_with("_AI_Output/")
                    });
                let selection = if tool == "run_python" {
                    json!({"purpose":request["purpose"],"inputs":request["inputs"]})
                } else {
                    request.clone()
                };
                evidence.push(json!({"operation_id":reference.operation_id,"pointer":reference.pointer,"tool":tool,
                    "kind":if tool=="run_python" {"calculation"} else if snapshot_only {"snapshot"} else {"source"},
                    "path":data.get("path").or_else(||request.get("path")),"revision":data["revision"],
                    "selection":selection,"sources":data["sources"],"excerpt":excerpt,
                    "notice":"成功工具回傳的當時快照；不代表現在檔案未變更，也不代表程式已驗證此結論。"}));
            }
            findings
                .push(json!({"claim":finding.claim,"status":finding.status,"evidence":evidence}));
        }
        let mut checks = Vec::new();
        for spec in &report.checks {
            let (_, result) = self
                .recorded_operation(&spec.operation_id)?
                .ok_or("核對操作不存在於本次任務。")?;
            checks.push(analysis::check(spec, &result)?);
        }
        // 先完成全部核對才寫記憶。方法僅作可再嘗試的經驗，不自動執行舊程式。
        let method_note = if let Some(method) = &report.method {
            Some(
                self.memory()?
                    .save_analysis_method(method, &checks, &self.task_id)?,
            )
        } else {
            self.analysis
                .report
                .as_ref()
                .and_then(|r| r.get("method_note"))
                .cloned()
        };
        self.analysis.report = Some(
            json!({"goal":report.goal,"current_step":report.current_step,
            "open_questions":report.open_questions,"superseded":report.superseded,"findings":findings,"checks":checks,"method_note":method_note}),
        );
        self.analysis.review_required = false;
        self.analysis.completion = self.analysis.completion_summary();
        Ok(self.analysis.summary())
    }
}
