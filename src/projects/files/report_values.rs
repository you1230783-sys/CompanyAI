//! 報告綁定的Broker入口：來源讀取、Office編輯與成果保存均沿用原有邊界。
use super::*;
use crate::projects::report_values::{Assignment, BoundBlock, Metric, Spec};

impl Broker {
    pub(super) fn register_values(&mut self, specs: &[Spec]) -> AppResult<Value> {
        if specs.is_empty() || specs.len() > 40 {
            return Err("每次登記1–40個計算值。".into());
        }
        let mut next = self.report_values.clone();
        let mut seen = std::collections::BTreeSet::new();
        for spec in specs {
            if !seen.insert(&spec.id) {
                return Err("同批計算值ID不可重複。".into());
            }
            let (request, result) = self
                .recorded_operation(&spec.operation_id)?
                .ok_or("計算來源操作不存在。")?;
            let metric = Metric::from_result(spec, &request, &result)?;
            if let Some(old) = next.metrics.get(&spec.id) {
                if serde_json::to_value(old).map_err(|e| e.to_string())?
                    != serde_json::to_value(&metric).map_err(|e| e.to_string())?
                {
                    return Err(
                        "計算值ID已綁定另一結果；新計算請使用新ID，避免默默更改已交付數字。".into(),
                    );
                }
            }
            next.metrics.insert(spec.id.clone(), metric);
        }
        if next.metrics.len() > 200 {
            return Err("每個任務最多200個計算值。".into());
        }
        self.report_values = next;
        Ok(
            json!({"values":self.report_values.context(),"scope":"實際計算結果，非公式正確性驗證；以apply_report_values填入Office，用export_report_values取得圖表用CSV。"}),
        )
    }

    pub(super) fn apply_values(
        &mut self,
        id: &str,
        revision: &str,
        assignments: &[Assignment],
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        if assignments.is_empty() || assignments.len() > 20 {
            return Err("每次綁定1–20個報告欄位。".into());
        }
        let copy = self.copies.get(id).ok_or("不是本任務Office副本。")?;
        let snapshot = &copy
            .office
            .as_ref()
            .ok_or("數值欄位僅支援Office工作副本。")?
            .desired;
        let mut actions = Vec::new();
        let mut bounds = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for a in assignments {
            if !seen.insert(&a.block_id) {
                return Err("同批目標區塊不可重複。".into());
            }
            let metric = self
                .report_values
                .metrics
                .get(&a.metric_id)
                .ok_or("請先登記實際計算值。")?;
            let block = snapshot
                .blocks
                .iter()
                .find(|b| b.id == a.block_id)
                .ok_or("報告區塊不存在，請重新讀取副本。")?;
            if !matches!(block.kind.as_str(), "text" | "number") {
                return Err("公式與唯讀欄位不能綁定寫入。".into());
            }
            let numeric = block.kind == "number";
            let replacement = if numeric {
                metric.display.clone()
            } else {
                metric.text()
            };
            actions.push(office::Action::Edit {
                block_id: a.block_id.clone(),
                expected: block.text.clone(),
                replacement: replacement.clone(),
            });
            bounds.push(BoundBlock {
                copy_id: id.into(),
                block_id: a.block_id.clone(),
                metric_id: a.metric_id.clone(),
                expected: replacement,
                numeric,
            });
        }
        let mut result = self.office_batch(id, revision, actions, cancel)?;
        self.report_values
            .blocks
            .retain(|old| old.copy_id != id || !seen.contains(&old.block_id));
        self.report_values.blocks.extend(bounds);
        result["value_bindings"] = json!(self
            .report_values
            .blocks
            .iter()
            .filter(|b| b.copy_id == id)
            .rev()
            .take(20)
            .collect::<Vec<_>>());
        Ok(result)
    }

    /// 圖表與Excel可共用此CSV；數值直接由同一計算登記表產生，無模型轉抄。
    pub(super) fn export_values(
        &mut self,
        name: &str,
        ids: &[String],
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        if ids.is_empty() || ids.len() > 200 {
            return Err("需指定1–200個計算值ID。".into());
        }
        let mut table =
            super::super::datasets::Table::new(vec!["指標".into(), "數值".into(), "單位".into()])?;
        let mut seen = std::collections::BTreeSet::new();
        for (i, id) in ids.iter().enumerate() {
            if !seen.insert(id) {
                return Err("計算值ID不可重複。".into());
            }
            let metric = self
                .report_values
                .metrics
                .get(id)
                .ok_or("計算值ID不存在。")?;
            table.push(super::super::datasets::Row {
                path: format!("calculation:{}", metric.spec.operation_id),
                revision: metric.code_sha256.clone(),
                sheet: 0,
                row: i + 1,
                values: vec![id.clone(), metric.display.clone(), metric.spec.unit.clone()],
                kinds: vec!["text".into(), "number".into(), "text".into()],
                texts: vec![],
            })?;
        }
        let result = self.save_dataset(name, &table, cancel)?;
        self.report_values
            .datasets
            .push(json!({"dataset":result,"metric_ids":ids}));
        Ok(result)
    }

    pub(super) fn verify_report_values(&self, cancel: &AtomicBool) -> AppResult<()> {
        let mut checked = std::collections::BTreeSet::new();
        for metric in self.report_values.metrics.values() {
            if checked.insert(&metric.spec.operation_id) {
                let (request, result) = self
                    .recorded_operation(&metric.spec.operation_id)?
                    .ok_or("綁定計算紀錄遺失。")?;
                let inputs: Vec<super::super::python::Input> =
                    serde_json::from_value(request["inputs"].clone()).map_err(|e| e.to_string())?;
                let current = super::super::python::snapshot(&self.project, &inputs, cancel)?;
                let now = current.as_array().ok_or("計算來源快照無效。")?;
                let sources = result["result"]["sources"]
                    .as_array()
                    .ok_or("計算來源索引無效。")?;
                if now.len() != sources.len()
                    || now.iter().zip(sources).any(|(a, b)| {
                        a["path"] != b["path"]
                            || a["revision"] != b["revision"]
                            || a["scope"] != b["scope"]
                    })
                {
                    return Err("計算來源已變更，報告數值需要重新計算，不沿用舊結果交付。".into());
                }
            }
        }
        for bound in &self.report_values.blocks {
            let copy = self
                .copies
                .get(&bound.copy_id)
                .ok_or("綁定報告副本已移除。")?;
            let snapshot = &copy
                .office
                .as_ref()
                .ok_or("綁定報告不是Office副本。")?
                .desired;
            self.report_values.check_block(bound, snapshot)?;
        }
        Ok(())
    }
}
