//! 工作流程保存已成功的工具參數與明確參數插槽，不保存登入或操作授權。
//! 每次只執行一步；步驟結果以JSON Pointer傳給後續步驟，不猜新副本ID或版本。
use crate::AppResult;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capture {
    pub step_id: String,
    pub operation_id: String,
    /// 目標JSON Pointer -> param:名稱 或 step:先前步驟ID:/result/...
    pub bindings_json: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Step {
    pub id: String,
    pub request: Value,
    pub bindings: BTreeMap<String, String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Template {
    pub id: String,
    pub name: String,
    pub created: u64,
    pub steps: Vec<Step>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Active {
    pub execution_id: String,
    pub template: Template,
    pub parameters: BTreeMap<String, Value>,
    pub next: usize,
    pub results: BTreeMap<String, Value>,
    pub blocked: bool,
}

pub fn allowed(tool: &str) -> bool {
    matches!(
        tool,
        "list_files"
            | "read_file"
            | "read_code_section"
            | "find_text"
            | "inspect_excel"
            | "read_excel_range"
            | "run_python"
            | "inspect_dataset"
            | "chart_dataset"
            | "create_working_copy"
            | "office_action"
            | "office_batch"
            | "edit_office"
            | "save_copy"
            | "register_report_values"
            | "apply_report_values"
            | "export_report_values"
            | "create_mermaid"
            | "export_mermaid"
            | "load_skill"
    )
}
impl Template {
    pub fn validate(&self) -> AppResult<()> {
        if self.name.trim().is_empty()
            || self.name.chars().count() > 80
            || self.steps.is_empty()
            || self.steps.len() > 40
        {
            return Err("範本需名稱1–80字、1–40個成功步驟。".into());
        }
        let mut known = BTreeSet::new();
        for step in &self.steps {
            crate::jobs::validate_id(&step.id)?;
            let tool = step.request["tool"].as_str().ok_or("範本步驟缺少工具。")?;
            if !allowed(tool) || !known.insert(step.id.clone()) || step.bindings.len() > 30 {
                return Err("範本含不支援工具、重複步驟或過多插槽。".into());
            }
            // 副本身份及版本屬於一次執行；不能在同一任務重跑時偷偷改到舊副本。
            if step.request.get("copy_id").is_some() {
                for pointer in ["/copy_id", "/revision"] {
                    if step.request.pointer(pointer).is_some()
                        && !step
                            .bindings
                            .get(pointer)
                            .is_some_and(|r| r.starts_with("step:"))
                    {
                        return Err(
                            "工作副本ID及版本必須引用前面新執行步驟的結果，不能保存舊任務值。"
                                .into(),
                        );
                    }
                }
            }
            if tool == "register_report_values" {
                for index in 0..step.request["values"].as_array().map_or(0, Vec::len) {
                    let pointer = format!("/values/{index}/operation_id");
                    if !step
                        .bindings
                        .get(&pointer)
                        .is_some_and(|r| r.starts_with("step:") && r.ends_with(":/operation_id"))
                    {
                        return Err(
                            "流程的計算值需引用前面新計算步驟的operation_id，不沿用上次數字。"
                                .into(),
                        );
                    }
                }
            }
            for (pointer, reference) in &step.bindings {
                if pointer == "/tool"
                    || pointer.is_empty()
                    || pointer.len() > 400
                    || step.request.pointer(pointer).is_none()
                {
                    return Err("參數插槽必須指向既存工具參數，不能更換工具。".into());
                }
                if let Some(name) = reference.strip_prefix("param:") {
                    crate::jobs::validate_id(name)?;
                } else if let Some(other) = reference.strip_prefix("step:") {
                    let (id, path) = other
                        .split_once(':')
                        .ok_or("步驟引用格式為step:ID:/result/欄位。")?;
                    if id == step.id
                        || !known.contains(id)
                        || !(path.starts_with("/result/") || path == "/operation_id")
                    {
                        return Err("只能引用前面成功步驟的result欄位。".into());
                    }
                } else {
                    return Err("插槽只接受param:名稱或step:ID:/result/欄位。".into());
                }
            }
        }
        if serde_json::to_vec(self).map_err(|e| e.to_string())?.len() > 200_000 {
            return Err("範本超過200KB。".into());
        }
        Ok(())
    }
}
impl Active {
    pub fn request(&self) -> AppResult<super::Tool> {
        if self.blocked {
            return Err(
                "此流程已在執行中或失敗，未重播。請檢視結果；未知寫入不能自動重試。".into(),
            );
        }
        let step = self.template.steps.get(self.next).ok_or("流程已完成。")?;
        let mut request = step.request.clone();
        for (pointer, reference) in &step.bindings {
            let value = if let Some(name) = reference.strip_prefix("param:") {
                self.parameters
                    .get(name)
                    .ok_or_else(|| format!("缺少流程參數：{name}"))?
                    .clone()
            } else {
                let (id, path) = reference
                    .strip_prefix("step:")
                    .ok_or("引用無效。")?
                    .split_once(':')
                    .ok_or("步驟引用無效。")?;
                self.results
                    .get(id)
                    .filter(|r| r["ok"] == true)
                    .and_then(|r| r.pointer(path))
                    .ok_or("前一步沒有此成功結果，未猜值。")?
                    .clone()
            };
            *request.pointer_mut(pointer).ok_or("參數目標已失效。")? = value;
        }
        serde_json::from_value(request).map_err(|e| format!("套用參數後工具格式無效：{e}"))
    }
    pub fn context(&self) -> Value {
        json!({"workflow_id":self.template.id,"name":self.template.name,"execution_id":self.execution_id,
            "next_step":self.next,"step_count":self.template.steps.len(),"blocked":self.blocked,
            "next":self.template.steps.get(self.next).map(|s|json!({"id":s.id,"tool":s.request["tool"]})),
            "completed":self.results.keys().collect::<Vec<_>>(),"notice":"每次execute_workflow_step執行一個已檢視步驟，重新核對資料與授權；失敗停止，不重播未知寫入。"})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_copy_ids_and_old_calculation_operations_are_not_runnable_templates() {
        for request in [
            json!({"tool":"save_copy","copy_id":"old","revision":"old-r"}),
            json!({"tool":"register_report_values","values":[{"operation_id":"old-calc"}]}),
        ] {
            let template = Template {
                id: String::new(),
                name: "不能沿用舊值".into(),
                created: 0,
                steps: vec![Step {
                    id: "step1".into(),
                    request,
                    bindings: BTreeMap::new(),
                }],
            };
            assert!(template.validate().is_err());
        }
    }
    #[test]
    fn rerun_resolves_new_source_and_prior_result_without_reusing_old_copy_id() {
        let template = Template {
            id: "template".into(),
            name: "副本".into(),
            created: 0,
            steps: vec![
                Step {
                    id: "create".into(),
                    request: json!({"tool":"create_working_copy","source":"old.txt","name":"報告.txt"}),
                    bindings: BTreeMap::from([("/source".into(), "param:source".into())]),
                },
                Step {
                    id: "save".into(),
                    request: json!({"tool":"save_copy","copy_id":"old-copy","revision":"old"}),
                    bindings: BTreeMap::from([
                        ("/copy_id".into(), "step:create:/result/copy_id".into()),
                        ("/revision".into(), "step:create:/result/revision".into()),
                    ]),
                },
            ],
        };
        template.validate().unwrap();
        let mut active = Active {
            execution_id: "run".into(),
            template,
            parameters: BTreeMap::from([("source".into(), json!("new.txt"))]),
            next: 0,
            results: BTreeMap::new(),
            blocked: false,
        };
        assert_eq!(
            serde_json::to_value(active.request().unwrap()).unwrap()["source"],
            "new.txt"
        );
        active.next = 1;
        assert!(active.request().is_err());
        active.results.insert(
            "create".into(),
            json!({"ok":true,"result":{"copy_id":"new-copy","revision":"new"}}),
        );
        let request = serde_json::to_value(active.request().unwrap()).unwrap();
        assert_eq!(request["copy_id"], "new-copy");
        assert_eq!(request["revision"], "new");
        active.blocked = true;
        assert!(active.request().is_err());
    }
    #[test]
    fn rejects_tool_switch_and_forward_dependency() {
        let mut t = Template {
            id: "t".into(),
            name: "t".into(),
            created: 0,
            steps: vec![Step {
                id: "first".into(),
                request: json!({"tool":"read_file","path":"a.txt","offset":0}),
                bindings: BTreeMap::from([("/tool".into(), "param:tool".into())]),
            }],
        };
        assert!(t.validate().is_err());
        t.steps[0].bindings = BTreeMap::from([("/path".into(), "step:later:/result/path".into())]);
        assert!(t.validate().is_err());
    }
}
