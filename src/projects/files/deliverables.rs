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
        if let Some(plan) = &self.task_plan.approved {
            if items.iter().any(|i| {
                !plan
                    .outputs
                    .iter()
                    .any(|o| o.id == i.id && o.description == i.description && o.kind == i.kind)
            }) {
                return Err("交付項目與核准計畫不同；請先 revise_plan 並取得核准。".into());
            }
        }
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

    pub(super) fn bind_deliverable(
        &mut self,
        bindings: &[super::super::deliverables::Binding],
    ) -> AppResult<Value> {
        for binding in bindings {
            if !self.copies.contains_key(&binding.target) && !self.known_output(&binding.target) {
                return Err(format!("交付目標尚不存在：{}。", binding.target));
            }
        }
        self.delivery_plan
            .bind(bindings, self.condition_revision())?;
        self.sync_delivery();
        Ok(self.analysis.deliverables.clone())
    }

    pub(super) fn supersede_artifact(
        &mut self,
        path: &str,
        replaced_by: &str,
        reason: &str,
    ) -> AppResult<Value> {
        if !self.known_output(path) || !self.known_output(replaced_by) {
            return Err("取代雙方必須都是本任務原生登記的輸出路徑，不接受原件或未知路徑。".into());
        }
        self.delivery_plan
            .supersede(super::super::deliverables::Superseded {
                path: path.into(),
                replaced_by: replaced_by.into(),
                reason: reason.into(),
            })?;
        self.sync_delivery();
        Ok(json!({"superseded":self.delivery_plan.superseded,"deleted":false}))
    }

    /// 所有舊成果仍先讀回核對；只在呈現清單時排除取代者，綁定成果排在前面。
    pub(super) fn primary_delivery_paths(&self, paths: Vec<String>) -> Vec<String> {
        let snapshot = self.delivery_snapshot();
        let preferred: Vec<_> = snapshot["items"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|i| i["path"].as_str())
            .collect();
        let mut active: Vec<_> = paths
            .into_iter()
            .filter(|p| !self.delivery_plan.superseded.iter().any(|s| &s.path == p))
            .collect();
        active.sort_by_key(|p| preferred.iter().position(|v| *v == p).unwrap_or(usize::MAX));
        let mut seen = std::collections::BTreeSet::new();
        active.retain(|p| seen.insert(p.clone()));
        active
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
        json!({"items":items,"superseded":self.delivery_plan.superseded,"conditions":self.work_context.as_ref().map(|c|&c.conditions),
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
    fn superseded_files_stay_on_disk_and_still_require_valid_hashes() {
        use sha2::{Digest, Sha256};
        let root = std::env::current_dir()
            .unwrap()
            .join(".build")
            .join(format!("delivery-{}", crate::jobs::new_id().unwrap()));
        let folder = root.join("_AI_Output").join("fixture");
        fs::create_dir_all(&folder).unwrap();
        let mut broker = Broker::new(
            Project {
                id: "p".into(),
                name: "p".into(),
                root: root.clone(),
                imports: BTreeMap::new(),
            },
            "r".into(),
        )
        .unwrap();
        let old = "_AI_Output/fixture/v1.png";
        let new = "_AI_Output/fixture/v2.png";
        // 模擬原生已驗證的輸出索引；此測試針對交付雜湊與清單，PNG解碼另有測試。
        for path in [old, new] {
            fs::write(root.join(path), b"registered bytes").unwrap();
            broker
                .python_artifacts
                .push(crate::projects::python::Artifact {
                    path: path.into(),
                    sha256: format!("{:x}", Sha256::digest(b"registered bytes")),
                });
        }
        broker
            .update_deliverables(&[Item {
                id: "figure".into(),
                description: "最終圖".into(),
                kind: "file".into(),
                target: Some(old.into()),
                blocked_reason: None,
            }])
            .unwrap();
        broker.supersede_artifact(old, new, "重算").unwrap();
        assert!(
            broker.finish(&[]).is_err(),
            "不能用取代標記掩蓋仍綁定舊成果的需求"
        );
        broker
            .bind_deliverable(&[super::super::super::deliverables::Binding {
                id: "figure".into(),
                target: new.into(),
            }])
            .unwrap();
        assert_eq!(broker.finish(&[]).unwrap(), vec![new.to_string()]);
        assert!(root.join(old).exists());
        assert!(broker
            .supersede_artifact("original.csv", new, "未知來源")
            .is_err());
        fs::write(root.join(old), b"tampered").unwrap();
        assert!(broker.finish(&[]).is_err(), "已取代成果仍保留既有雜湊檢查");
        drop(broker);
        // 只刪除本測試建立的已知檔案，不遞迴刪除任何目錄。
        for path in [old, new] {
            fs::remove_file(root.join(path)).unwrap();
        }
    }

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
