//! 範本只擷取本任務成功操作；執行時仍走真正Broker，不新增任意命令入口。
use super::*;
use crate::projects::workflows::{Active, Capture, Step, Template};

impl Broker {
    pub(super) fn save_workflow(&self, name: &str, captures: &[Capture]) -> AppResult<Value> {
        let mut steps = Vec::new();
        for capture in captures {
            let (request, result) = self
                .recorded_operation(&capture.operation_id)?
                .ok_or("範本來源操作不存在。")?;
            if result["ok"] != true
                || result["result"]["executed"] == false
                || result["result"]["status"] == "FAILED"
            {
                return Err("只能擷取實際成功的操作，不能保存未知或失敗步驟。".into());
            }
            let bindings = serde_json::from_str(&capture.bindings_json)
                .map_err(|e| format!("插槽JSON無效：{e}"))?;
            steps.push(Step {
                id: capture.step_id.clone(),
                request,
                bindings,
            });
        }
        let mut template = Template {
            id: String::new(),
            name: name.into(),
            created: crate::unix_now(),
            steps,
        };
        template.validate()?;
        template.id = text::revision(&serde_json::to_string(&template).map_err(|e| e.to_string())?);
        self.memory()?.store_workflow(&template)?;
        Ok(
            json!({"saved":true,"workflow_id":template.id,"name":template.name,"steps":template.steps.len(),"notice":"已保存可執行步驟；完整內容用read_workflow分頁查看。下次先prepare_workflow替換來源／日期與必要版本，檢視計畫再逐步執行。此範本不保存權限。"}),
        )
    }
    pub(super) fn prepare_workflow(&mut self, id: &str, parameters: &str) -> AppResult<Value> {
        if self
            .workflow
            .as_ref()
            .is_some_and(|a| a.blocked || a.next < a.template.steps.len())
        {
            return Err(
                "目前流程尚未完成或已有失敗，不能重建流程來重播；請先查看結果與回報。".into(),
            );
        }
        if parameters.len() > 32_000 {
            return Err("流程參數超過32KB。".into());
        }
        let template = self.memory()?.load_workflow(id)?;
        let active = Active {
            execution_id: crate::jobs::new_id()?,
            template,
            parameters: serde_json::from_str(parameters).map_err(|e| e.to_string())?,
            next: 0,
            results: BTreeMap::new(),
            blocked: false,
        };
        active.template.validate()?;
        let required: std::collections::BTreeSet<_> = active
            .template
            .steps
            .iter()
            .flat_map(|s| s.bindings.values())
            .filter_map(|v| v.strip_prefix("param:"))
            .collect();
        if required
            .iter()
            .any(|name| !active.parameters.contains_key(*name))
            || active
                .parameters
                .keys()
                .any(|name| !required.contains(name.as_str()))
        {
            return Err("流程參數缺少必要名稱或含未使用名稱，請依範本插槽確認後再準備。".into());
        }
        let context = active.context();
        let steps: Vec<_> = active
            .template
            .steps
            .iter()
            .map(|s| json!({"id":s.id,"tool":s.request["tool"],"parameter_slots":s.bindings.len()}))
            .collect();
        let parameters = active.parameters.clone();
        self.workflow = Some(active);
        Ok(
            json!({"plan":context,"steps":steps,"parameters":parameters,"executed":false,"notice":"完整步驟與程式可用read_workflow分頁檢視；prepare不執行任何步驟。"}),
        )
    }
    pub(super) fn execute_workflow_step(
        &mut self,
        execution: &str,
        index: usize,
        worker: &mut Worker,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        let active = self
            .workflow
            .as_ref()
            .ok_or("請先prepare_workflow並檢視步驟。")?;
        if active.execution_id != execution || active.next != index {
            return Err("流程執行ID或步驟不符，未執行。".into());
        }
        let request = active.request()?;
        let value = serde_json::to_value(&request).map_err(|e| e.to_string())?;
        let tool = value["tool"].as_str().ok_or("流程工具無效。")?;
        if !crate::projects::workflows::allowed(tool)
            || !crate::projects::skills::enabled(tool, &self.loaded_skills)
        {
            return Err("此步驟工具不允許或技能尚未載入，請先載入所需技能。".into());
        }
        let id = text::revision(&format!("workflow:{execution}:{index}"));
        let step_id = active.template.steps[index].id.clone();
        self.workflow.as_mut().unwrap().blocked = true;
        let result = self.execute(&id, &request, worker, cancel)?;
        let active = self.workflow.as_mut().unwrap();
        active.results.insert(step_id, result.clone());
        if result["ok"] == true && result["result"]["executed"] != false {
            active.next += 1;
            active.blocked = false;
        }
        Ok(json!({"workflow":active.context(),"outcome":result}))
    }
}
