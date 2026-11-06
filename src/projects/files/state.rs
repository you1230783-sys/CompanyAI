//! Broker的state操作；由父模組保留共用狀態與既有介面。
use super::*;

impl Broker {
    pub fn new(mut project: Project, task: String) -> AppResult<Self> {
        project.root = crate::projects::setup::resolve_project_path(&project.root)?;
        validate_root(&project.root)?;
        Ok(Self {
            task_id: task,
            task_plan: Default::default(),
            delivery_plan: Default::default(),
            work_context: None,
            analysis: Default::default(),
            mail_notes: Default::default(),
            mail_draft: None,
            excel_plans: BTreeMap::new(),
            file_waiter: None,
            cad_waiter: None,
            cad_progress: None,
            cad_active: None,
            cad_session: Default::default(),
            cad_snapshot: None,
            model: "fast".into(),
            cad_active_unsupported: None,
            outlook: crate::projects::mail::Session::default(),
            outlook_process: None,
            log_cursors: BTreeMap::new(),
            server_pdf: None,
            memory: None,
            project,
            output_conversation: None,
            output_folder: None,
            copies: BTreeMap::new(),
            translations: BTreeMap::new(),
            results: BTreeMap::new(),
            reading: crate::projects::reading::Notebook::default(),
            report_values: crate::projects::report_values::State::default(),
            workflow: None,
            source_versions: BTreeMap::new(),
            comparison_artifacts: Vec::new(),
            comparison_records: BTreeMap::new(),
            archived_results: BTreeMap::new(),
            work_log_index: None,
            references: references::References::default(),
            published: Vec::new(),
            txt_context: false,
            loaded_skills: vec![],
            charts: vec![],
            diagrams: vec![],
            diagram_sources: Vec::new(),
            diagram_renderer: None,
            diagram_exports: Vec::new(),
            previous_diagrams: BTreeMap::new(),
            chart_exports: vec![],
            datasets: vec![],
            python_artifacts: vec![],
            cad_artifacts: vec![],
            cad_failed: false,
            cad_last_result: None,
            cad_design: Default::default(),
            cad_attempts: 0,
            cad_partial_plans: vec![],
            cad_reads: BTreeMap::new(),
            cad_copy_version: None,
            cad_previews: BTreeMap::new(),
            png_renderer: None,
            chart_chooser: None,
            preferences: None,
            chart_deadline: std::time::Instant::now(),
            previous_charts: BTreeMap::new(),
            chart_palette: crate::config::default_chart_palette(),
        })
    }
    pub(in crate::projects) fn saved(&self) -> AppResult<SavedBroker> {
        // 經序列化建立不含程序資源的快照；授權與執行中的 COM 物件不保存。
        serde_json::from_value(
            json!({"task_plan":self.task_plan,"translations":self.translations,"delivery_plan":self.delivery_plan,"work_context":self.work_context,"comparison_records":self.comparison_records,"comparison_artifacts":self.comparison_artifacts,"source_versions":self.source_versions,"workflow":self.workflow,"report_values":self.report_values,"reading":self.reading,"analysis":self.analysis,"mail_notes":self.mail_notes,"mail_draft":self.mail_draft,"excel_plans":self.excel_plans,"outlook":self.outlook.saved,"log_cursors":self.log_cursors,"output_folder":self.output_folder,"copies":self.copies,
            "cad_design":self.cad_design,"cad_copy_version":self.cad_copy_version,
            "results":self.results,"archived_results":self.archived_results,"work_log_index":self.work_log_index,"references":self.references,"published":self.published,"txt_context":self.txt_context,"loaded_skills":self.loaded_skills,"charts":self.charts,"diagrams":self.diagrams,"diagram_sources":self.diagram_sources,"diagram_exports":self.diagram_exports,"chart_exports":self.chart_exports,"datasets":self.datasets,"python_artifacts":self.python_artifacts,"cad_artifacts":self.cad_artifacts,"cad_failed":self.cad_failed,"cad_last_result":self.cad_last_result,"cad_attempts":self.cad_attempts,"cad_partial_plans":self.cad_partial_plans,"cad_reads":self.cad_reads,"cad_previews":self.cad_previews}),
        )
        .map_err(|e| e.to_string())
    }
    pub(in crate::projects) fn restore(
        &mut self,
        state: SavedBroker,
        cancel: &AtomicBool,
    ) -> AppResult<()> {
        self.task_plan = state.task_plan;
        self.reading = state.reading;
        if state.translations.len() > 10 {
            return Err("保存的翻譯任務超過上限。".into());
        }
        for (id, translation) in &state.translations {
            crate::jobs::validate_id(id)?;
            relative(&translation.source)?;
            translation.original.serialize()?;
        }
        self.translations = state.translations;
        self.report_values = state.report_values;
        self.delivery_plan = state.delivery_plan;
        self.work_context = state.work_context;
        self.workflow = state.workflow;
        self.source_versions = state.source_versions;
        self.comparison_artifacts = state.comparison_artifacts;
        self.comparison_records = state.comparison_records;
        self.analysis = state.analysis;
        state.mail_notes.validate()?;
        self.mail_notes = state.mail_notes;
        self.mail_draft = state.mail_draft;
        if state.excel_plans.len() > 30 {
            return Err("保存的 Excel 規劃超過上限。".into());
        }
        for (id, plan) in &state.excel_plans {
            plan.validate()?;
            if id != &plan.id {
                return Err("Excel 規劃索引不符。".into());
            }
        }
        self.excel_plans = state.excel_plans;
        self.log_cursors = state.log_cursors;
        self.outlook.saved = state.outlook;
        if state.copies.len() > 20 {
            return Err("暫存工作副本數不合法。".into());
        }
        if let Some(folder) = &state.output_folder {
            if relative(folder)?.components().count() != 1 {
                return Err("暫存輸出目錄不合法。".into());
            }
            let _guards = pin(&self.project.root.join("_AI_Output").join(folder))?;
        }
        for (id, copy) in &state.copies {
            crate::jobs::validate_id(id)?;
            if relative(&copy.name)?.components().count() != 1 {
                return Err("暫存檔名不合法。".into());
            }
            if let Some(office) = &copy.office {
                if let Some(source) = &office.source {
                    if fingerprint(&self.project, source)? != office.fingerprint {
                        return Err(format!(
                            "來源 {source} 已變更，未恢復舊版修改；請重新提出任務。"
                        ));
                    }
                }
                if office.desired.serialize()? != copy.text {
                    return Err("暫存 Office 版本不一致。".into());
                }
            }
            if let Some(draft) = &copy.draft {
                self.verify_draft(draft, state.output_folder.as_deref())?;
                if read_cancel(&self.project, &draft.path, cancel, None, None)?.0 != copy.text {
                    return Err("草稿與保存的副本版本不一致，未繼續覆寫。".into());
                }
            }
            if copy.saved_revision.as_deref() == Some(text::revision(&copy.text).as_str()) {
                let path = copy.paths.last().ok_or("暫存成果缺少路徑。")?;
                if read_cancel(&self.project, path, cancel, None, None)?.0 != copy.text {
                    return Err(format!("成果 {path} 已變更，未繼續舊任務。"));
                }
            }
        }
        if let Some(draft) = &self.mail_draft {
            self.verify_draft(draft, state.output_folder.as_deref())?;
        }
        crate::projects::skills::context(&state.loaded_skills)?;
        if state.diagrams.len() > crate::projects::diagrams::MAX_DIAGRAMS {
            return Err("保存的 Mermaid 圖形超過上限。".into());
        }
        for diagram in &state.diagrams {
            diagram.validate()?;
            if read_cancel(&self.project, &diagram.path, cancel, None, None)?.0 != diagram.source {
                return Err("Mermaid 成果已變更，未恢復舊任務。".into());
            }
        }
        self.diagrams = state.diagrams;
        diagrams::validate_sources(&state.diagram_sources)?;
        self.diagram_sources = state.diagram_sources;
        for export in &state.diagram_exports {
            self.verify_diagram_export(export)?;
        }
        self.diagram_exports = state.diagram_exports;
        for chart in &state.charts {
            chart.validate()?;
        }
        for export in &state.chart_exports {
            self.verify_chart_export(export)?;
        }
        for dataset in &state.datasets {
            crate::projects::datasets::load(
                &self.project,
                &dataset.path,
                &dataset.revision,
                cancel,
            )?;
        }
        for artifact in &state.python_artifacts {
            self.verify_python_artifact(artifact)?;
        }
        for artifact in &state.cad_artifacts {
            self.verify_cad_artifact(artifact)?;
        }
        self.cad_artifacts = state.cad_artifacts;
        self.cad_failed = state.cad_failed;
        self.cad_last_result = state.cad_last_result;
        self.cad_design = state.cad_design;
        self.cad_attempts = state.cad_attempts;
        self.cad_partial_plans = state.cad_partial_plans;
        self.cad_reads = state.cad_reads;
        self.cad_copy_version = state.cad_copy_version;
        self.cad_previews = state.cad_previews;
        self.python_artifacts = state.python_artifacts;
        self.datasets = state.datasets;
        self.chart_exports = state.chart_exports;
        self.loaded_skills.clear();
        for id in state.loaded_skills {
            crate::projects::skills::activate(&mut self.loaded_skills, &id)?;
        }
        self.charts = state.charts;
        self.output_folder = state.output_folder;
        self.copies = state.copies;
        self.results = state.results;
        self.archived_results = state.archived_results;
        self.work_log_index = state.work_log_index;
        self.references = state.references;
        // 舊 checkpoint 首次恢復時補建；新 checkpoint 沿用已保存序號。
        for id in self.archived_results.keys().chain(self.results.keys()) {
            self.references.register(id);
        }
        self.published = state.published;
        self.txt_context = state.txt_context;
        Ok(())
    }
    /// 只有專案代理啟用跨任務記憶；一般聊天沒有此能力。
    pub fn enable_memory(&mut self, conversation: &str) -> AppResult<()> {
        crate::jobs::validate_id(conversation)?;
        self.memory = Some(crate::projects::memory::Memory::open(
            self.project.clone(),
            conversation,
        )?);
        self.txt_context |= self.memory()?.has_protected_documents()?;
        self.output_conversation = Some(conversation.to_owned());
        Ok(())
    }
    pub fn memory(&self) -> AppResult<&crate::projects::memory::Memory> {
        self.memory
            .as_ref()
            .ok_or("本次任務未啟用專案記憶。".into())
    }
}
