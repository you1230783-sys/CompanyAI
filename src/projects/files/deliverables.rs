//! 將需求項目與真正的工作副本／輸出索引連接。每輪畫面與模型使用同一份狀態。
use super::*;

impl Broker {
    /// 設定由使用者保存的偏好快照；續接使用 SavedBroker 內原快照。
    pub(in crate::projects) fn set_work_context(
        &mut self,
        context: super::super::work_settings::Snapshot,
    ) {
        self.work_context = Some(context);
        self.sync_delivery();
    }
    /// 只有新的使用者條件版本才更新；回傳原文供 runner 走既有補充指示邊界。
    pub(in crate::projects) fn update_conditions(
        &mut self,
        conditions: super::super::work_settings::Conditions,
    ) -> Option<String> {
        let context = self.work_context.as_mut()?;
        if context.conditions == conditions {
            return None;
        }
        context.conditions = conditions.clone();
        self.analysis.review_required = true;
        self.sync_delivery();
        Some(format!("使用者已更新本對話條件（第{}版）：\n{}\n請重新核對受影響的計算與成果，已完成但無關的步驟不要重做。", conditions.revision,
            if conditions.text.is_empty() { "已清除先前條件卡；本次提問與其他明確補充仍有效。" } else { &conditions.text }))
    }
    pub(in crate::projects) fn work_context(
        &self,
    ) -> Option<&super::super::work_settings::Snapshot> {
        self.work_context.as_ref()
    }
    fn condition_revision(&self) -> u64 {
        self.work_context
            .as_ref()
            .map_or(0, |c| c.conditions.revision)
    }
    fn known_output(&self, path: &str) -> bool {
        self.published.iter().any(|p| p == path)
            || self.datasets.iter().any(|e| e.path == path)
            || self
                .python_artifacts
                .iter()
                .chain(&self.cad_artifacts)
                .chain(&self.comparison_artifacts)
                .any(|e| e.path == path)
            || self.chart_exports.iter().any(|e| e.path == path)
            || self.diagram_exports.iter().any(|e| e.path == path)
            || self.diagrams.iter().any(|e| e.path == path)
    }
    pub(super) fn update_deliverables(
        &mut self,
        items: &[super::super::deliverables::Item],
    ) -> AppResult<Value> {
        for item in items {
            if let Some(target) = &item.target {
                if !self.copies.contains_key(target) && !self.known_output(target) {
                    return Err(format!("交付目標尚不存在：{target}。先以target=null記待辦，建立副本或匯出後再綁定。"));
                }
            }
        }
        self.delivery_plan
            .update(items, self.condition_revision())?;
        self.sync_delivery();
        Ok(self.analysis.deliverables.clone())
    }
    /// saved 僅代表原生曾保存該版；finish 仍會逐檔讀回驗證，不能據此跳過交付檢查。
    pub(in crate::projects) fn delivery_snapshot(&self) -> Value {
        let changed = self.delivery_plan.conditions_revision != self.condition_revision();
        let items: Vec<_> = self.delivery_plan.items.iter().map(|item| {
            let copy = item.target.as_ref().and_then(|id| self.copies.get(id));
            let (status, path) = if item.blocked_reason.is_some() {
                ("blocked", None)
            } else if let Some(copy) = copy {
                if copy.saved_revision.as_deref() == Some(text::revision(&copy.text).as_str()) {
                    ("saved", copy.paths.last().cloned())
                } else { ("draft", copy.draft.as_ref().map(|d| d.path.clone())) }
            } else if let Some(path) = item.target.as_ref().filter(|p| self.known_output(p)) {
                ("saved", Some(path.clone()))
            } else if item.kind == "answer" { ("answer_pending", None) }
            else { ("pending", None) };
            json!({"id":item.id,"description":item.description,"kind":item.kind,"target":item.target,
                "status":status,"path":path,"blocked_reason":item.blocked_reason,"review_required":changed})
        }).collect();
        json!({"items":items,"conditions":self.work_context.as_ref().map(|c|&c.conditions),
            "review_required":changed,"notice":"已保存由原生工具紀錄判定；交付時再讀回核對。文字回答與內容正確性需另行確認。"})
    }
    pub(in crate::projects) fn sync_delivery(&mut self) {
        self.analysis.deliverables = self.delivery_snapshot();
    }
    pub(in crate::projects) fn has_blocked_deliverables(&self) -> bool {
        self.delivery_plan
            .items
            .iter()
            .any(|item| item.blocked_reason.is_some())
    }
    /// 僅在 finish 的檔案讀回核對通過後標記；文字回答不宣稱內容已驗證。
    pub(in crate::projects) fn mark_delivered(&mut self) {
        self.sync_delivery();
        if let Some(items) = self.analysis.deliverables["items"].as_array_mut() {
            for item in items {
                if item["status"] == "saved" {
                    item["status"] = json!("delivered");
                }
                if item["status"] == "answer_pending" {
                    item["status"] = json!("answered");
                }
            }
        }
    }
    pub(super) fn verify_deliverables(&self, paths: &[String]) -> AppResult<()> {
        if self.work_context.is_some() && !paths.is_empty() && self.delivery_plan.items.is_empty() {
            return Err(
                "本次有檔案成果，請先update_deliverables列出使用者要求並綁定成果，再交付。".into(),
            );
        }
        if !self.delivery_plan.items.is_empty()
            && self.delivery_plan.conditions_revision != self.condition_revision()
        {
            return Err("使用者條件已更新，請先核對全部交付項目，再update_deliverables。".into());
        }
        let snapshot = self.delivery_snapshot();
        for item in snapshot["items"].as_array().into_iter().flatten() {
            if item["blocked_reason"].is_string() || item["kind"] == "answer" {
                continue;
            }
            if item["status"] != "saved"
                || !item["path"]
                    .as_str()
                    .is_some_and(|p| paths.iter().any(|s| s == p))
            {
                return Err(format!("交付清單尚未完成：{}。請保存／匯出後綁定實際目標；確實無法完成時填寫blocked_reason，不能省略該項。",item["description"].as_str().unwrap_or("未命名項目")));
            }
        }
        Ok(())
    }
    /// 部分完成的限制由程式附加，避免模型最後一句「已完成」掩蓋未交項目。
    pub(in crate::projects) fn delivery_report(&self) -> String {
        let blocked: Vec<_> = self
            .delivery_plan
            .items
            .iter()
            .filter_map(|item| {
                item.blocked_reason
                    .as_ref()
                    .map(|reason| format!("- {}：{}", item.description, reason))
            })
            .collect();
        if blocked.is_empty() {
            String::new()
        } else {
            format!(
                "\n\n本次僅部分完成，以下需求未完成：\n{}",
                blocked.join("\n")
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::{
        deliverables::Item,
        work_settings::{Conditions, Snapshot},
    };

    #[test]
    fn pending_and_changed_conditions_cannot_finish_but_explicit_blockers_are_visible() {
        let mut broker = Broker::new(
            Project {
                id: "p".into(),
                name: "p".into(),
                root: std::env::current_dir().unwrap(),
                imports: BTreeMap::new(),
            },
            "r".into(),
        )
        .unwrap();
        broker.set_work_context(Snapshot::default());
        assert!(broker
            .verify_deliverables(&["untracked.txt".into()])
            .is_err());
        let mut item = Item {
            id: "report".into(),
            description: "Word報告".into(),
            kind: "file".into(),
            target: None,
            blocked_reason: None,
        };
        broker.update_deliverables(&[item.clone()]).unwrap();
        assert_eq!(broker.delivery_snapshot()["items"][0]["status"], "pending");
        assert!(broker.verify_deliverables(&[]).is_err());
        item.blocked_reason = Some("Office尚未開啟".into());
        broker.update_deliverables(&[item.clone()]).unwrap();
        broker.verify_deliverables(&[]).unwrap();
        assert!(broker.delivery_report().contains("本次僅部分完成"));
        broker.update_conditions(Conditions {
            revision: 1,
            text: "改交付Excel".into(),
        });
        assert!(broker.verify_deliverables(&[]).is_err());
        assert_eq!(broker.delivery_snapshot()["review_required"], true);
        item.description = "Excel報告".into();
        broker.update_deliverables(&[item]).unwrap();
        broker.verify_deliverables(&[]).unwrap();
        let mut saved = serde_json::to_value(broker.saved().unwrap()).unwrap();
        assert_eq!(saved["delivery_plan"]["conditions_revision"], 1);
        saved.as_object_mut().unwrap().remove("delivery_plan");
        saved.as_object_mut().unwrap().remove("work_context");
        let old: SavedBroker = serde_json::from_value(saved).unwrap();
        assert!(old.delivery_plan.items.is_empty());
        assert!(old.work_context.is_none());
    }
}
