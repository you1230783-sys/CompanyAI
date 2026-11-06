//! Broker的execution操作；由父模組保留共用狀態與既有介面。
use super::*;

impl Broker {
    /// 每批結束先封存原文，再縮小快照。索引保留全部 ID，重送不會重做修改。
    pub(in crate::projects) fn archive_results(&mut self) -> AppResult<()> {
        for (id, (request, result)) in &self.results {
            let hash = self
                .memory()?
                .archive_operation(&json!([request, result]))?;
            self.archived_results.insert(
                id.clone(),
                json!({"hash":hash,"tool":request["tool"],"ok":result["ok"],"path":request["path"],"compare_path":request["compare_path"]}),
            );
        }
        self.results.clear();
        Ok(())
    }
    pub(super) fn recorded_operation(&self, id: &str) -> AppResult<Option<(Value, Value)>> {
        if let Some(pair) = self.results.get(id) {
            return Ok(Some(pair.clone()));
        }
        let Some(entry) = self.archived_results.get(id) else {
            return Ok(None);
        };
        let value = self
            .memory()?
            .archived_operation(entry["hash"].as_str().ok_or("操作索引無效。")?)?;
        serde_json::from_value(value)
            .map(Some)
            .map_err(|_| "操作原文格式無效。".into())
    }
    pub(in crate::projects) fn operation_history(&self) -> AppResult<Value> {
        let mut rows = Vec::new();
        for id in self
            .archived_results
            .keys()
            .chain(self.results.keys())
            .collect::<std::collections::BTreeSet<_>>()
        {
            let (request, result) = self.recorded_operation(id)?.ok_or("操作紀錄遺失。")?;
            rows.push(json!({"operation_id":id,"request":request,"result":result}));
        }
        Ok(json!(rows))
    }
    /// 非同步委派也共用操作去重表；不能與一般工具重複使用不同參數的 ID。
    pub(in crate::projects) fn cached_result(
        &self,
        id: &str,
        tool: &Tool,
    ) -> AppResult<Option<Value>> {
        if let Some(result) = self.task_plan.deny(tool) {
            return Ok(Some(result));
        }
        self.outlook.check_policy()?;
        crate::jobs::validate_id(id)?;
        let request = serde_json::to_value(tool).map_err(|e| e.to_string())?;
        if !crate::build_features::tool_available(request["tool"].as_str().unwrap_or_default()) {
            return Err("此建置未包含CAD功能，不能重播CAD操作或沿用其成功結果。".into());
        }
        if (self.cad_session.stopped() || self.cad_failed)
            && request["tool"]
                .as_str()
                .is_some_and(|name| name.starts_with("cad_") && name != "cad_inventory")
        {
            return Ok(Some(
                json!({"ok":false,"error":"本任務CAD操作已停止，不重讀、重畫或換CAD。請依cad_artifacts.last_execution或cad_session.last_edit回報原生失敗。"}),
            ));
        }
        if let Some((previous, result)) = self.recorded_operation(id)? {
            if previous != request {
                return Err("操作識別碼重複但參數不同；已停止。".into());
            }
            if matches!(
                tool,
                Tool::OutlookFolders { .. }
                    | Tool::OutlookHeaders { .. }
                    | Tool::OutlookCompare { .. }
                    | Tool::OutlookRead { .. }
                    | Tool::OutlookIndex { .. }
            ) && !self.outlook.is_allowed()
            {
                return Ok(None);
            }
            return Ok(Some(self.current_cad_result(tool, result.clone())));
        }
        Ok(None)
    }
    pub(in crate::projects) fn remember_result(
        &mut self,
        id: &str,
        tool: &Tool,
        result: &Value,
    ) -> AppResult<()> {
        self.cached_result(id, tool)?;
        self.observe_reading(id, tool, result)?;
        self.observe_source_versions(tool, result);
        if result["ok"] == true {
            self.retain_diagram_read(tool, &result["result"]);
        }
        if matches!(tool, Tool::ReadCodeSection { .. }) && result["ok"] == true {
            self.retain_code_section(&result["result"]);
        }
        // 寫入只記實際成功操作及當時選定草稿；這是可追溯關聯，不宣稱每條文字已核對。
        if result["ok"] == true
            && matches!(
                tool,
                Tool::EditOffice { .. }
                    | Tool::OfficeAction { .. }
                    | Tool::OfficeBatch { .. }
                    | Tool::EditText { .. }
                    | Tool::SaveCopy { .. }
            )
        {
            let request = serde_json::to_value(tool).map_err(|e| e.to_string())?;
            for note_id in &self.mail_notes.selected {
                if let Some(note) = self.mail_notes.notes.get_mut(note_id) {
                    if !note
                        .write_operations
                        .iter()
                        .any(|op| op["operation_id"] == id)
                    {
                        note.write_operations.push(json!({"operation_id":id,"tool":request["tool"],"copy_id":request["copy_id"],"revision":result["result"]["revision"],"path":result["result"]["path"],"meaning":"操作時選定的草稿；內容是否完整仍需核對"}));
                        if note.write_operations.len() > 8 {
                            note.write_operations.remove(0);
                        }
                    }
                }
            }
        }
        if matches!(tool, Tool::OutlookRead { .. })
            && result["ok"] == true
            && result["result"]["text"].is_string()
        {
            self.mail_notes.observe(id, result)?;
        }
        self.results.insert(
            id.into(),
            (
                serde_json::to_value(tool).map_err(|e| e.to_string())?,
                result.clone(),
            ),
        );
        Ok(())
    }
    pub fn execute(
        &mut self,
        id: &str,
        tool: &Tool,
        worker: &mut Worker,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        self.execute_progress(id, tool, worker, cancel, &mut |_| {})
    }
    pub fn execute_progress(
        &mut self,
        id: &str,
        tool: &Tool,
        worker: &mut Worker,
        cancel: &AtomicBool,
        notify: &mut dyn FnMut(&str),
    ) -> AppResult<Value> {
        crate::projects::runner::check_cancel(cancel)?;
        if let Some(result) = self.task_plan.deny(tool) {
            return Ok(result);
        }
        let request = serde_json::to_value(tool).map_err(|e| e.to_string())?;
        if !crate::build_features::tool_available(request["tool"].as_str().unwrap_or_default()) {
            return Err("此建置未包含CAD功能，尚未執行或重播CAD操作。".into());
        }
        self.reauthorize_outlook(cancel, self.chart_deadline)?;
        crate::jobs::validate_id(id)?;
        // 連重播舊工具結果都先經本次同意；授權不隨 checkpoint 恢復。
        if matches!(
            tool,
            Tool::OutlookFolders { .. }
                | Tool::OutlookHeaders { .. }
                | Tool::OutlookCompare { .. }
                | Tool::OutlookRead { .. }
                | Tool::OutlookIndex { .. }
        ) && !self.outlook.authorize(cancel, self.chart_deadline)?
        {
            return Ok(
                json!({"ok":true,"result":{"declined":true,"executed":false,"message":"使用者未確認 Outlook 使用範圍；資料夾名稱僅在本機預覽，未讀取郵件或交給 AI。"}}),
            );
        }
        if (self.cad_session.stopped() || self.cad_failed)
            && request["tool"]
                .as_str()
                .is_some_and(|name| name.starts_with("cad_") && name != "cad_inventory")
        {
            return Ok(
                json!({"ok":false,"error":"CAD操作已停止，不能以其他CAD工具重試；請依cad_artifacts.last_execution或cad_session.last_edit回報。"}),
            );
        }
        if let Some((old, result)) = self.recorded_operation(id)? {
            if old != request {
                return Err("操作識別碼重複但參數不同；已停止。".into());
            }
            return Ok(self.current_cad_result(tool, result.clone()));
        }
        // 容量守衛必須在真正改圖之前，不能改完才因報告容量不足回報失敗。
        if matches!(tool, Tool::CadEdit { .. } | Tool::CadActiveEdit { .. })
            && self.comparison_records.len() >= 20
        {
            return Ok(
                json!({"ok":false,"operation_id":id,"error":"CAD比較已達20批上限，本次尚未修改圖面；請先交付本任務成果。"}),
            );
        }
        if self.reading.enabled
            && !self.reading.pending.is_empty()
            && !matches!(
                tool,
                Tool::RecordReadingNotes { .. }
                    | Tool::ReadReadingNotes { .. }
                    | Tool::LoadSkill { .. }
                    | Tool::ReadSkillGuide { .. }
                    | Tool::ReadWorkLog { .. }
            )
        {
            return Ok(
                json!({"ok":false,"operation_id":id,"error":"上一批閱讀尚未保存成果。先record_reading_notes，逐項引用目前reading.pending的operation_id，保存實際流程／分區／關係／疑問；不可只重述已讀完。"}),
            );
        }
        // 必須早於 perform／計畫消耗／成果寫入；不能把執行中的未知錯誤標成未執行。
        let mut registration_contract = false;
        let outcome = loop {
            crate::projects::runner::check_cancel(cancel)?;
            let ownership = match self.operation_ownership(tool) {
                Ok(guard) => guard,
                Err(error) if error.contains(crate::projects::interaction::BUSY) => {
                    // 來源檔案可能在擁有權檢查時就被外部程式占用。
                    // 此時尚未執行任何操作，也沒有持有資源鎖；沿用關閉檔案提示，
                    // 確認後從頭核對來源與權限，不把可恢復狀態永久記成契約錯誤。
                    if self.wait_for_file(&error, cancel)? {
                        continue;
                    }
                    return Ok(
                        json!({"ok":true,"result":{"waiting_for_user":true,"wait_reason":"等待關閉占用檔案後繼續讀取","executed":false}}),
                    );
                }
                Err(error) => {
                    registration_contract = true;
                    break Err(error);
                }
            };
            let result = if let Tool::RunBatch { tasks } = tool {
                self.run_batch(id, tasks, worker.executable(), cancel)
            } else if matches!(
                tool,
                Tool::OutlookFolders { .. }
                    | Tool::OutlookHeaders { .. }
                    | Tool::OutlookCompare { .. }
                    | Tool::OutlookRead { .. }
            ) {
                self.outlook_tool(tool, worker.executable(), cancel, notify)
            } else if let Tool::RecordAnalysis { report } = tool {
                self.record_analysis(report).map_err(|error| {
                    registration_contract = error.contract;
                    error.message
                })
            } else if let Tool::RegisterReportValues { values } = tool {
                self.register_values(values).map_err(|error| {
                    registration_contract = error.contract;
                    error.message
                })
            } else {
                self.perform(tool, worker, cancel)
            };
            // 使用者尚未關閉外部文件時，不占著本機鎖等待確認卡片。
            // 重試會回到迴圈開頭重新排隊，再重新執行原有版本與權限檢查。
            drop(ownership);
            let Err(error) = &result else {
                break result;
            };
            let retryable = matches!(
                tool,
                Tool::ReadFile { .. }
                    | Tool::CreateWorkingCopy {
                        source: Some(_),
                        ..
                    }
                    | Tool::FindText { .. }
                    | Tool::SearchFiles { .. }
                    | Tool::ListDocumentSections { .. }
                    | Tool::ReadLog { .. }
                    | Tool::SearchLogs { .. }
                    | Tool::InspectExcel { .. }
                    | Tool::ReadExcelRange { .. }
                    | Tool::ChartExcelRange { .. }
                    | Tool::ExportExcelDataset { .. }
                    | Tool::PlanExcelAnalysis { .. }
                    | Tool::ExportPlannedExcel { .. }
                    | Tool::ExportLogDataset { .. }
                    | Tool::InspectDataset { .. }
                    | Tool::ChartDataset { .. }
                    | Tool::ChartFromExcel { .. }
            );
            if error.contains(crate::projects::interaction::DEFERRED)
                || (retryable && error.contains(crate::projects::interaction::BUSY))
            {
                if !error.contains(crate::projects::interaction::DEFERRED)
                    && self.wait_for_file(error, cancel)?
                {
                    continue;
                }
                return Ok(
                    json!({"ok":true,"result":{"waiting_for_user":true,"wait_reason":"等待關閉占用檔案後繼續讀取","executed":false}}),
                );
            }
            break result;
        };
        let result = match outcome {
            Ok(value) => json!({"ok":true,"operation_id":id,"result":value}),
            Err(error) => {
                json!({"ok":false,"operation_id":id,"error":error,"retry_same_operation":false})
            }
        };
        let mut result = cad_preflight::normalize_outcome(tool, result);
        // 以下入口只驗證記憶體計畫，全部通過才提交；Err 不會寫檔或部分修改。
        // 不將一般工具的 Err 當成未執行，因為 COM／Python 可能已產生部分成果。
        if result["ok"] == false
            && (registration_contract
                || matches!(
                    tool,
                    Tool::ProposePlan { .. }
                        | Tool::RevisePlan { .. }
                        | Tool::UpdateDeliverables { .. }
                        | Tool::RecordReadingNotes { .. }
                        | Tool::BindDeliverable { .. }
                        | Tool::SupersedeArtifact { .. }
                ))
        {
            result["error_kind"] = json!("contract");
            result["executed"] = json!(false);
        }
        if matches!(tool, Tool::ExecuteWorkflowStep { .. })
            && result["result"]["outcome"]["ok"] == false
        {
            result["ok"] = json!(false);
            result["error"] = json!("工作流程步驟失敗，已停止；詳outcome，不重播已成功操作。");
        }
        self.retain_cad_comparison(id, &mut result)?;
        self.observe_reading(id, tool, &result)?;
        self.observe_source_versions(tool, &result);
        if let Some(identity) = self.semantic_progress(tool, &result) {
            result["progress_identity"] = json!(identity);
        }
        if result["ok"] == true {
            self.retain_diagram_read(tool, &result["result"]);
        }
        if result["result"]["waiting_for_user"] != true {
            self.analysis.observe(tool, &result);
            self.analysis.completion = self.analysis.completion_summary();
            self.results.insert(id.into(), (request, result.clone()));
        }
        Ok(result)
    }
}
