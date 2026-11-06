//! 計畫文字不授予權限。原生狀態控制唯讀階段，核准與退回只由桌面入口寫入。
use crate::AppResult;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub method: String,
    pub tools: Vec<String>,
    pub parameters: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Output {
    pub id: String,
    pub description: String,
    pub kind: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub goal: String,
    pub steps: Vec<Step>,
    pub sources: Vec<String>,
    pub outputs: Vec<Output>,
    pub assumptions: Vec<String>,
    pub questions: Vec<String>,
}
impl Plan {
    pub fn validate(&self) -> AppResult<()> {
        fn text(value: &str, max: usize) -> bool {
            !value.trim().is_empty() && value.chars().count() <= max
        }
        let bytes = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        if bytes.len() > 24_000
            || !text(&self.goal, 1000)
            || self.steps.is_empty()
            || self.steps.len() > 20
            || self.steps.iter().any(|s| {
                !text(&s.method, 600)
                    || s.parameters.chars().count() > 1000
                    || s.tools.is_empty()
                    || s.tools.len() > 10
                    || s.tools.iter().any(|t| !text(t, 80))
            })
            || self.sources.len() > 30
            || self.sources.iter().any(|s| !text(s, 500))
            || self.assumptions.len() > 20
            || self.assumptions.iter().any(|s| !text(s, 500))
            || self.questions.len() > 20
            || self.questions.iter().any(|s| !text(s, 500))
            || self.outputs.is_empty()
            || self.outputs.len() > 20
        {
            return Err(
                "計畫需有目標、1–20 步及1–20項產出，全文不可超過24KB；請縮短過長欄位。".into(),
            );
        }
        let mut ids = std::collections::BTreeSet::new();
        for output in &self.outputs {
            crate::jobs::validate_id(&output.id)?;
            if !ids.insert(&output.id)
                || !text(&output.description, 300)
                || !["file", "answer"].contains(&output.kind.as_str())
            {
                return Err("計畫產出 ID 不可重複，說明1–300字，類型只允許 file／answer。".into());
            }
        }
        Ok(())
    }
    /// 只有原步驟內的參數變動可直接記錄；方法、來源、問題、產出等改動一律重新核准。
    fn parameter_change_only(&self, next: &Self) -> bool {
        let mut same_parameters = next.clone();
        if self.steps.len() != next.steps.len() {
            return false;
        }
        for (old, new) in self.steps.iter().zip(&mut same_parameters.steps) {
            new.parameters.clone_from(&old.parameters);
        }
        *self == same_parameters
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Revision {
    pub revision: u32,
    pub plan: Plan,
    pub status: String,
    pub note: String,
    pub local_time: String,
}
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct State {
    pub enabled: bool,
    pub read_only: bool,
    pub approved: Option<Plan>,
    pub pending: Option<Revision>,
    pub history: Vec<Revision>,
    /// 核准／退回文字存於 checkpoint，恢復後才交給模型；不從模型輸出解析決策。
    pub notice: Option<String>,
}
impl State {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            read_only: enabled,
            ..Self::default()
        }
    }
    pub fn submit(&mut self, plan: &Plan, revise: bool, reason: &str) -> AppResult<()> {
        if !self.enabled || self.pending.is_some() {
            return Err("計畫模式未啟用或已有計畫等待核准，未接受新的計畫。".into());
        }
        if revise != self.approved.is_some() {
            return Err("首次使用 propose_plan；已有核准計畫後請使用 revise_plan。".into());
        }
        plan.validate()?;
        if self.history.len() >= 39 || reason.chars().count() > 1000 {
            return Err("計畫歷程最多40筆，修訂原因最多1000字。".into());
        }
        let minor = !self.read_only
            && self
                .approved
                .as_ref()
                .is_some_and(|p| p.parameter_change_only(plan));
        let entry = Revision {
            revision: self.history.last().map_or(1, |r| r.revision + 1),
            plan: plan.clone(),
            status: if minor { "parameter_update" } else { "pending" }.into(),
            note: reason.into(),
            local_time: crate::calendar::local_timestamp(),
        };
        self.history.push(entry.clone());
        if minor {
            self.approved = Some(plan.clone());
        } else {
            self.pending = Some(entry);
            self.read_only = true;
        }
        Ok(())
    }
    /// 呼叫者必須先核對對話、任務、登入身分與等待版本；此函式不提供給模型工具。
    pub fn review(&mut self, revision: u32, edited: Option<Plan>, feedback: &str) -> AppResult<()> {
        let pending = self.pending.as_ref().ok_or("目前沒有等待核准的計畫。")?;
        if pending.revision != revision
            || self.history.len() >= 40
            || feedback.chars().count() > 2000
        {
            return Err("計畫版本已變更、歷程已滿或補充超過2000字，請重新查看。".into());
        }
        if edited.is_none() && feedback.trim().is_empty() {
            return Err("退回計畫時請補充原因。".into());
        }
        if let Some(plan) = &edited {
            plan.validate()?;
        }
        let mut reviewed = pending.clone();
        reviewed.status = if edited.is_some() {
            "approved"
        } else {
            "rejected"
        }
        .into();
        reviewed.note = feedback.into();
        reviewed.local_time = crate::calendar::local_timestamp();
        if let Some(plan) = edited {
            reviewed.plan = plan.clone();
            self.approved = Some(plan);
            self.read_only = false;
        } else {
            self.read_only = true;
        }
        self.notice = Some(format!(
            "使用者計畫決策（不擴大既有權限）：{}",
            serde_json::to_string(&reviewed).map_err(|e| e.to_string())?
        ));
        self.history.push(reviewed);
        self.pending = None;
        Ok(())
    }
    /// 正面白名單：新增工具預設不會取得核准前的執行能力。內部閱讀筆記只保存閱讀進度。
    pub fn tool_allowed(&self, tool: &super::Tool) -> bool {
        if !self.enabled {
            return !matches!(
                tool,
                super::Tool::ProposePlan { .. } | super::Tool::RevisePlan { .. }
            );
        }
        if !self.read_only {
            return true;
        }
        if let super::Tool::RunBatch { tasks } = tool {
            return tasks
                .iter()
                .all(|t| t.request().is_ok_and(|t| self.tool_allowed(&t)));
        }
        let value = serde_json::to_value(tool).unwrap_or_default();
        self.name_allowed(value["tool"].as_str().unwrap_or_default())
    }
    pub fn name_allowed(&self, name: &str) -> bool {
        if !self.enabled {
            return !matches!(name, "propose_plan" | "revise_plan");
        }
        if !self.read_only {
            return true;
        }
        matches!(
            name,
            "propose_plan"
                | "revise_plan"
                | "list_files"
                | "read_file"
                | "find_text"
                | "read_code_section"
                | "search_files"
                | "inspect_dataset"
                | "load_skill"
                | "read_skill_guide"
                | "read_task_result"
                | "inspect_excel"
                | "read_excel_range"
                | "list_logs"
                | "record_reading_notes"
                | "read_reading_notes"
                | "read_work_log"
                | "list_notes"
                | "read_note"
                | "list_document_sections"
                | "read_document_section"
                | "read_mail_notes"
                | "list_workflows"
                | "read_workflow"
                | "compact_context"
                | "ask_preference"
                | "read_log"
                | "search_logs"
                | "cad_inventory"
                | "run_batch"
                | "finish"
                | "ask_user"
        )
    }
    pub fn deny(&self, tool: &super::Tool) -> Option<serde_json::Value> {
        (!self.tool_allowed(tool)).then(|| {
            serde_json::json!({"ok":false,"executed":false,"error_kind":"contract","error_code":14,
            "error":"計畫尚未核准或此模式未啟用；本次工具未執行。請先提出計畫並等待使用者確認。"})
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub fn plan() -> Plan {
        Plan {
            goal: "分析來源並交付報告".into(),
            steps: vec![Step {
                method: "核對資料後計算".into(),
                tools: vec!["run_python".into()],
                parameters: "lambda=0.1".into(),
            }],
            sources: vec!["input.csv".into()],
            outputs: vec![Output {
                id: "report".into(),
                description: "分析報告".into(),
                kind: "file".into(),
            }],
            assumptions: vec![],
            questions: vec![],
        }
    }
    #[test]
    fn unapproved_write_and_batch_return_contract14_without_execution() {
        let mode = State::new(true);
        let write = super::super::Tool::CreateWorkingCopy {
            source: None,
            name: "x.txt".into(),
        };
        let result = mode.deny(&write).unwrap();
        assert_eq!(
            crate::errors::tool_code("project_tool.create_working_copy", &result),
            "14"
        );
        assert_eq!(result["executed"], false);
        let batch = super::super::Tool::RunBatch {
            tasks: vec![super::super::files::batch::Task {
                task_id: "calc".into(),
                tool: "run_python".into(),
                arguments_json: r#"{"code":"result = {}","inputs":[],"purpose":"test"}"#.into(),
            }],
        };
        assert!(mode.deny(&batch).is_some());
        assert!(mode
            .deny(&super::super::Tool::ReadFile {
                path: "input.csv".into(),
                offset: 0
            })
            .is_none());
        for read in [
            "search_files",
            "search_logs",
            "list_notes",
            "read_note",
            "list_document_sections",
            "read_document_section",
            "read_mail_notes",
        ] {
            assert!(mode.name_allowed(read), "計畫需可使用既有唯讀入口 {read}");
        }
        assert!(!mode.name_allowed("future_write_tool"));
        assert!(State::default().deny(&write).is_none());
    }
    #[test]
    fn edited_approval_rejection_and_major_revision_preserve_history() {
        let mut state = State::new(true);
        state.submit(&plan(), false, "").unwrap();
        assert!(state.review(99, Some(plan()), "").is_err());
        state.review(1, None, "改用另一個方法").unwrap();
        assert!(state.read_only);
        assert_eq!(state.history[1].status, "rejected");
        state.submit(&plan(), false, "").unwrap();
        let mut edited = plan();
        edited.goal = "使用者修改過的版本".into();
        state.review(2, Some(edited.clone()), "核准").unwrap();
        assert_eq!(state.approved, Some(edited.clone()));
        assert!(state
            .notice
            .as_ref()
            .unwrap()
            .contains("使用者修改過的版本"));
        assert!(!state.read_only);
        edited.steps[0].parameters = "lambda=0.2".into();
        state.submit(&edited, true, "參數敏感度").unwrap();
        assert!(state.pending.is_none());
        assert_eq!(state.history.last().unwrap().status, "parameter_update");
        edited.steps[0].method = "更換主要方法".into();
        state.submit(&edited, true, "新方法").unwrap();
        assert!(state.read_only);
        assert!(state.pending.is_some());
        assert_eq!(state.history[0].plan.goal, "分析來源並交付報告");
        let restored: State = serde_json::from_slice(&serde_json::to_vec(&state).unwrap()).unwrap();
        assert!(restored.read_only);
        assert_eq!(restored.pending.unwrap().plan, edited);
    }
    #[test]
    fn output_changes_cannot_masquerade_as_parameter_tuning() {
        let original = plan();
        let mut edited = original.clone();
        edited.outputs.clear();
        assert!(!original.parameter_change_only(&edited));
        assert!(edited.validate().is_err());
        edited = original.clone();
        edited.outputs[0].description = "新增工作".into();
        assert!(!original.parameter_change_only(&edited));
        edited = original.clone();
        edited.sources.push("extra.csv".into());
        assert!(!original.parameter_change_only(&edited));
        edited = original.clone();
        edited.goal = "x".repeat(1001);
        assert!(edited.validate().is_err());
    }
}
