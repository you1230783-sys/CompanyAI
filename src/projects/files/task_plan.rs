//! 計畫核准沿用交付與 checkpoint；不新增任何文件、Office 或刪除授權。
use super::*;

impl Broker {
    pub(super) fn propose_task_plan(
        &mut self,
        plan: &crate::projects::task_plan::Plan,
        revise: bool,
        reason: &str,
    ) -> AppResult<Value> {
        self.task_plan.submit(plan, revise, reason)?;
        Ok(
            json!({"plan_pending":self.task_plan.pending.is_some(),"plan":self.task_plan,
            "notice":"重大計畫等待使用者確認；未取得任何額外檔案或工具權限。"}),
        )
    }
}

impl SavedBroker {
    /// 只由原生使用者確認呼叫。先驗證完整新狀態，避免交付清單與核准版本部分更新。
    pub(in crate::projects) fn review_task_plan(
        &mut self,
        revision: u32,
        edited: Option<crate::projects::task_plan::Plan>,
        feedback: &str,
    ) -> AppResult<crate::projects::task_plan::State> {
        let mut next = self.task_plan.clone();
        let approved = edited.is_some();
        next.review(revision, edited, feedback)?;
        if approved {
            let plan = next.approved.as_ref().ok_or("核准計畫遺失。")?;
            // 產出與使用者核准的版本完全一致；保留仍屬同一需求的原生綁定。
            // 已移除需求仍保留在計畫歷程，已發布的成果與操作紀錄不刪除。
            self.delivery_plan.items = plan
                .outputs
                .iter()
                .map(|output| {
                    self.delivery_plan
                        .items
                        .iter()
                        .find(|old| {
                            old.id == output.id
                                && old.description == output.description
                                && old.kind == output.kind
                        })
                        .cloned()
                        .unwrap_or_else(|| crate::projects::deliverables::Item {
                            id: output.id.clone(),
                            description: output.description.clone(),
                            kind: output.kind.clone(),
                            target: None,
                            blocked_reason: None,
                        })
                })
                .collect();
            self.delivery_plan.conditions_revision = self
                .work_context
                .as_ref()
                .map_or(0, |c| c.conditions.revision);
        }
        self.task_plan = next;
        Ok(self.task_plan.clone())
    }
}
