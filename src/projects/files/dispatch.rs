//! Broker的dispatch操作；由父模組保留共用狀態與既有介面。
use super::*;

impl Broker {
    pub(super) fn perform(
        &mut self,
        tool: &Tool,
        worker: &mut Worker,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        match tool {
            Tool::ProposePlan { plan } => self.propose_task_plan(plan, false, "首次計畫"),
            Tool::RevisePlan { plan, reason } => self.propose_task_plan(plan, true, reason),
            Tool::DocumentTask { command } => self.document_task(command, worker, cancel),
            Tool::OfficeAction { operation, .. }
                if matches!(
                    operation.as_ref(),
                    office::Action::TranslateRuns { .. }
                        | office::Action::WordRichParagraph { .. }
                        | office::Action::WordRichTable { .. }
                ) =>
            {
                Err("內部翻譯／匯出格式只能由原生文件流程套用。".into())
            }
            Tool::OfficeBatch { operations, .. }
                if operations.iter().any(|a| {
                    matches!(
                        a,
                        office::Action::TranslateRuns { .. }
                            | office::Action::WordRichParagraph { .. }
                            | office::Action::WordRichTable { .. }
                    )
                }) =>
            {
                Err("批次操作不接受內部翻譯區段。".into())
            }
            Tool::RegisterReportValues { values } => {
                self.register_values(values).map_err(|error| error.message)
            }
            Tool::LinkDiagramSources {
                diagram_index,
                revision,
                nodes,
            } => self.link_diagram_sources(*diagram_index, revision, nodes, worker, cancel),
            Tool::ExportCadComparison {
                source_operation,
                name,
            } => self.export_cad_comparison(source_operation, name),
            Tool::SaveWorkflow { name, steps } => self.save_workflow(name, steps),
            Tool::UpdateDeliverables { items } => self.update_deliverables(items),
            Tool::BindDeliverable { bindings } => self.bind_deliverable(bindings),
            Tool::SupersedeArtifact {
                path,
                replaced_by,
                reason,
            } => self.supersede_artifact(path, replaced_by, reason),
            Tool::ListWorkflows {} => self.memory()?.list_workflows(),
            Tool::ReadWorkflow {
                workflow_id,
                offset,
            } => {
                let template = self.memory()?.load_workflow(workflow_id)?;
                let content = serde_json::to_string_pretty(&template).map_err(|e| e.to_string())?;
                let total = content.chars().count();
                if *offset > total {
                    return Err("範本檢視位置超出範圍。".into());
                }
                let text: String = content.chars().skip(*offset).take(6000).collect();
                Ok(
                    json!({"workflow_id":workflow_id,"text":text,"offset":offset,"next_offset":offset+text.chars().count(),"total":total,"executed":false}),
                )
            }
            Tool::PrepareWorkflow {
                workflow_id,
                parameters_json,
            } => self.prepare_workflow(workflow_id, parameters_json),
            Tool::ExecuteWorkflowStep {
                execution_id,
                step_index,
            } => self.execute_workflow_step(execution_id, *step_index, worker, cancel),
            Tool::ApplyReportValues {
                copy_id,
                revision,
                assignments,
            } => self.apply_values(copy_id, revision, assignments, cancel),
            Tool::ExportReportValues { name, metric_ids } => {
                self.export_values(name, metric_ids, cancel)
            }
            Tool::RecordReadingNotes { notes } => self.record_reading_notes(notes),
            Tool::ReadReadingNotes {
                path,
                offset,
                source_operation,
                reason,
            } => {
                if let Some(reference) = source_operation {
                    let resolved = self.references.resolve(reference)?;
                    let id = &resolved;
                    if reason
                        .as_deref()
                        .is_none_or(|s| s.trim().is_empty() || s.chars().count() > 300)
                    {
                        return Err(
                            "定向查原文需提供1–300字具體缺項理由，不以『上下文沒有』要求重讀全文。"
                                .into(),
                        );
                    }
                    if !self.reading.notes.contains_key(id)
                        && !self.reading.pending.contains_key(id)
                    {
                        return Err("找不到這份筆記的來源操作。".into());
                    }
                    let recalled_key = format!("{id}:{offset}");
                    if self.reading.recalled_pages.contains(&recalled_key) {
                        let mut note = json!(self.reading.notes.get(id));
                        self.references.project(&mut note);
                        return Ok(json!({"source_operation":reference,"reading_note":note,
                            "already_recalled":true,"notice":"此封存頁已補查。請把剛取得的細節更新至record_reading_notes，再完成答覆；不要重複查同頁。"}));
                    }
                    let (_, result) = self.recorded_operation(id)?.ok_or("閱讀來源封存遺失。")?;
                    let text = result["result"]["text"]
                        .as_str()
                        .or_else(|| result["result"]["analysis"].as_str())
                        .ok_or("來源沒有可讀原文。")?;
                    let total = text.chars().count();
                    if *offset > total {
                        return Err("原文查閱位置超出範圍。".into());
                    }
                    let part: String = text.chars().skip(*offset).take(6000).collect();
                    self.reading.recalled_pages.insert(recalled_key);
                    let followup = self
                        .reading
                        .allow_image_followup(id, reason.as_deref().unwrap());
                    Ok(
                        json!({"source_operation":reference,"source":result["result"]["path"],
                        "images":result["result"]["images"],"revision":result["result"]["revision"],
                        "text":part,"offset":offset,"next_offset":offset+part.chars().count(),"total":total,
                        "reason":reason,"snapshot_only":true,"image_followup":followup}),
                    )
                } else {
                    let mut value = self.reading.read(path.as_deref(), *offset)?;
                    self.references.project(&mut value);
                    Ok(value)
                }
            }
            Tool::ReadMailNotes {
                mode,
                note_ids,
                offset,
                focus,
            } => self.read_mail_notes(mode, note_ids, *offset, focus),
            Tool::SetWorkStage {
                stage,
                note_ids,
                reason,
            } => self.set_work_stage(*stage, note_ids, reason),
            Tool::RecordLogEvents { events } => self.record_log_events(events),
            Tool::RecordAnalysis { report } => {
                self.record_analysis(report).map_err(|error| error.message)
            }
            Tool::PlanExcelAnalysis { proposal } => {
                let columns = proposal.columns()?;
                let selection = office::excel::Selection {
                    sheet: proposal.sheet,
                    columns,
                    header_row: proposal.header_row,
                    start_row: proposal.header_row.checked_add(1).ok_or("表頭列號無效。")?,
                    row_count: 5,
                };
                let (page, _) =
                    self.with_excel(&proposal.path, Some(&proposal.revision), |path| {
                        office::excel::read(path, &selection, cancel)
                    })?;
                let plan =
                    crate::projects::excel_plan::Plan::create(proposal.as_ref().clone(), &page)?;
                if self.excel_plans.len() >= 30 && !self.excel_plans.contains_key(&plan.id) {
                    return Err("每次任務最多30份Excel欄位規劃。".into());
                }
                let summary = plan.summary();
                self.excel_plans.insert(plan.id.clone(), plan);
                Ok(summary)
            }
            Tool::ExportPlannedExcel {
                plan_id,
                start_row,
                scan_rows,
                window,
                name,
            } => {
                let plan = self
                    .excel_plans
                    .get(plan_id)
                    .ok_or("找不到本任務的plan_id；請先plan_excel_analysis核對欄位。")?
                    .clone();
                plan.validate()?;
                let ((page, scanned), _) =
                    self.with_excel(&plan.proposal.path, Some(&plan.proposal.revision), |path| {
                        office::excel::planned::read(
                            path,
                            &plan,
                            *start_row,
                            *scan_rows,
                            window.as_ref(),
                            cancel,
                        )
                    })?;
                let mut table = crate::projects::datasets::Table::from_excel(
                    &page,
                    &plan.proposal.path,
                    &plan.proposal.revision,
                )?;
                let source_path = plan.proposal.path.clone();
                let source_revision = plan.proposal.revision.clone();
                if let Some(metadata) = table.excel.as_mut() {
                    metadata.plan = Some(plan);
                    metadata.filter = window.clone();
                    metadata.scanned_rows = scanned;
                }
                let mut result = self.save_dataset(name, &table, cancel)?;
                result["source_path"] = json!(source_path);
                result["source_revision"] = json!(source_revision);
                result["selected_rows"] = json!(table.rows.len());
                result["scanned_rows"] = json!(scanned);
                result["next_source_row"] = json!(page.next_row);
                result["scan_complete"] = json!(page.next_row.is_none());
                Ok(result)
            }
            Tool::OutlookFolders {
                scope,
                parent_id,
                offset,
            } => self.outlook.folders(
                &mut crate::outlook::project::Reader,
                scope,
                parent_id.as_deref(),
                *offset,
                cancel,
            ),
            Tool::OutlookHeaders {
                folder_id,
                start_date,
                end_date,
                cursor,
            } => self.outlook.headers(
                &mut crate::outlook::project::Reader,
                folder_id,
                start_date,
                end_date,
                cursor.as_deref(),
                cancel,
            ),
            Tool::OutlookCompare { mail_ids, offset } => self.outlook.compare(
                &mut crate::outlook::project::Reader,
                mail_ids,
                *offset,
                cancel,
            ),
            Tool::OutlookIndex { query, offset } => self.outlook.index(query, *offset),
            Tool::OutlookRead { mail_id, offset } => {
                self.txt_context = true;
                if let Some(note) = self.reuse_mail_note(mail_id, *offset)? {
                    return Ok(note);
                }
                self.outlook.body(
                    &mut crate::outlook::project::Reader,
                    mail_id,
                    *offset,
                    cancel,
                )
            }
            Tool::ListLogs {
                path,
                date,
                category,
                station,
                offset,
            } => crate::projects::logs::list(
                &self.project,
                path,
                date.as_deref(),
                category.as_deref(),
                station.as_deref(),
                *offset,
                cancel,
            ),
            Tool::ReadLog {
                path,
                revision,
                start_line,
                start_column,
                line_count,
            } => {
                self.txt_context = true;
                crate::projects::logs::read(
                    &self.project,
                    path,
                    revision.as_deref(),
                    *start_line,
                    *start_column,
                    *line_count,
                    cancel,
                )
            }
            Tool::SearchLogs { query, cursor } => {
                self.txt_context = true;
                let previous = cursor
                    .as_ref()
                    .map(|id| {
                        self.log_cursors
                            .get(id)
                            .ok_or("LOG 游標已失效，請從第一頁重新搜尋。")
                    })
                    .transpose()?;
                let (mut result, next) =
                    crate::projects::logs::search(&self.project, query, previous, cancel)?;
                if let Some(next) = next {
                    if self.log_cursors.len() >= 240 {
                        return Err("LOG 續頁已達本次任務上限，請縮小查詢範圍。".into());
                    }
                    // 同一查詢／來源／位置取得同一個游標，避免重查製造假進度。
                    let id =
                        text::revision(&serde_json::to_string(&next).map_err(|e| e.to_string())?);
                    self.log_cursors.insert(id.clone(), next);
                    result["next_cursor"] = json!(id);
                } else {
                    result["next_cursor"] = Value::Null;
                }
                Ok(result)
            }
            Tool::InspectExcel {
                path,
                sheet,
                header_row,
                start_column,
                column_count,
            } => {
                self.txt_context = true;
                let (mut value, revision) = self.with_excel(path, None, |target| {
                    office::excel::inspect(
                        target,
                        *sheet,
                        *header_row,
                        start_column,
                        *column_count,
                        cancel,
                    )
                })?;
                value["path"] = json!(path);
                value["revision"] = json!(revision);
                Ok(value)
            }
            Tool::ReadExcelRange {
                path,
                revision,
                sheet,
                columns,
                header_row,
                start_row,
                row_count,
            } => {
                self.txt_context = true;
                let selection = office::excel::Selection {
                    sheet: *sheet,
                    columns: columns.clone(),
                    header_row: *header_row,
                    start_row: *start_row,
                    row_count: *row_count,
                };
                selection.validate()?;
                let (page, _) = self.with_excel(path, Some(revision), |target| {
                    office::excel::read(target, &selection, cancel)
                })?;
                let mut value = serde_json::to_value(&page).map_err(|e| e.to_string())?;
                value["path"] = json!(path);
                value["revision"] = json!(revision);
                value["row_count"] = json!(page.rows.len());
                value["data_cells"] = json!(page.rows.len() * page.columns.len());
                value["scope"] = json!("僅所列欄位與列號；next_row 為所選欄位下一批位置，不代表整份文件已讀完。value 為 Excel Value2；日期是序號，text 為顯示文字。公式值為 Excel 本次開啟時提供，未主動更新外部連結。");
                Ok(value)
            }
            Tool::ChartExcelRange {
                path,
                revision,
                sheet,
                x_column,
                y_columns,
                header_row,
                start_row,
                row_count,
                kind,
                title,
                x_label,
                y_label,
            } => {
                self.require_planned_export(path, *sheet)?;
                self.txt_context = true;
                if y_columns.is_empty() || y_columns.len() > 8 {
                    return Err("圖表需選擇 1–8 個縱軸欄位。".into());
                }
                let columns = std::iter::once(x_column.clone())
                    .chain(y_columns.iter().cloned())
                    .collect();
                let selection = office::excel::Selection {
                    sheet: *sheet,
                    columns,
                    header_row: *header_row,
                    start_row: *start_row,
                    row_count: *row_count,
                };
                selection.validate_chart()?;
                let (page, _) = self.with_excel(path, Some(revision), |target| {
                    office::excel::read_chart(target, &selection, cancel)
                })?;
                crate::projects::excel_plan::check_unplanned_y(&page)?;
                let prepared = crate::projects::charts::prepare_page(
                    &page, kind, title, x_label, y_label, path, revision,
                )?;
                let Some(chart) = self.review_chart(prepared, cancel)? else {
                    return Ok(self.deferred_chart());
                };
                // 決策期間不佔用 Excel；接受前重新核對來源版本，舊決策不套到新資料。
                self.with_excel(path, Some(revision), |_| Ok(()))?;
                self.add_chart(chart)
            }
            Tool::CompactContext {
                working_note,
                superseded,
                next_step,
            } => {
                crate::projects::progress::validate_handoff(working_note, superseded, next_step).map_err(|error| {
                    if self.reading.notes.is_empty() { error } else {
                        format!("{error} 本任務{}份來源閱讀筆記已獨立保存，compact_context會保留它們；交接只需引用這些筆記並說明下一步，不要重抄全部函式／檔案摘要。", self.reading.notes.len())
                    }
                })?;
                // runner 在本結果封存成功後才套用；此處不可自行丟棄正在執行的對話。
                self.archive_results()?;
                if self.loaded_skills.iter().any(|id| id == "cad-new") {
                    self.cad_design.record(working_note, next_step)?;
                } else {
                    self.cad_design
                        .remember_before_loading(working_note, next_step)?;
                }
                Ok(
                    json!({"handoff_validated":true,"apply_at":"本工具結果封存後、下一次模型請求前","raw_history":"read_work_log"}),
                )
            }
            Tool::ExportLogDataset {
                query,
                revisions,
                fields,
                name,
            } => {
                self.txt_context = true;
                let (table, extraction) = crate::projects::logs::dataset(
                    &self.project,
                    query,
                    revisions,
                    fields,
                    cancel,
                )?;
                let mut result = self.save_dataset(name, &table, cancel)?;
                result["extraction"] = extraction;
                result["field_rules"] = json!(fields);
                Ok(result)
            }
            Tool::ExportExcelDataset {
                path,
                revision,
                sheet,
                columns,
                header_row,
                start_row,
                row_count,
                name,
            } => {
                self.require_planned_export(path, *sheet)?;
                self.txt_context = true;
                let selection = office::excel::Selection {
                    sheet: *sheet,
                    columns: columns.clone(),
                    header_row: *header_row,
                    start_row: *start_row,
                    row_count: *row_count,
                };
                selection.validate_chart()?;
                let (page, _) = self.with_excel(path, Some(revision), |target| {
                    office::excel::read_chart(target, &selection, cancel)
                })?;
                let table = crate::projects::datasets::Table::from_excel(&page, path, revision)?;
                let mut result = self.save_dataset(name, &table, cancel)?;
                result["headers"] = json!(page
                    .headers
                    .iter()
                    .map(|h| h.text.chars().take(100).collect::<String>())
                    .collect::<Vec<_>>());
                result["next_source_row"] = json!(page.next_row);
                result["date_1904"] = json!(page.date_1904);
                Ok(result)
            }
            Tool::CadActiveRead { version, offset } => {
                self.read_active_cad(version, *offset, cancel)
            }
            Tool::CadSnapshot {
                version,
                path,
                refresh,
            } => self.snapshot_cad(version, path.as_deref(), *refresh, cancel),
            Tool::CadQuery {
                snapshot_id,
                filter,
                offset,
                limit,
                details,
            } => self.query_cad(snapshot_id, filter, *offset, *limit, *details, cancel),
            Tool::CadPreview {
                snapshot_id,
                candidates,
                region,
                show_user,
            } => self.preview_cad(snapshot_id, candidates, *region, *show_user, cancel),
            Tool::CadActiveEdit {
                revision,
                edits,
                operations,
            } => self.edit_active_cad(revision, edits, operations, cancel),
            Tool::CadExcelPlan {
                path,
                revision,
                settings,
            } => self.plan_cad_excel(path, revision, settings, cancel),
            Tool::CadExcelApply { .. } => self.apply_cad_excel(tool, cancel),
            Tool::CadInventory {} => crate::cad::inventory(cancel),
            Tool::CadRead {
                path,
                version,
                offset,
                revision,
            } => self.read_cad(path, version, *offset, revision.as_deref(), cancel),
            Tool::CadEdit { .. } => self.edit_cad(tool, cancel),
            Tool::CadDraw {
                name,
                version,
                operations,
            } => self.draw_cad(name, version, operations, cancel),
            Tool::CadPlanStart { name, version } => self.start_cad_plan(name, version, cancel),
            Tool::CadPlanPart {
                plan_id,
                plan_revision,
                part_id,
                summary,
                operations,
            } => self.put_cad_plan_part(plan_id, *plan_revision, part_id, summary, operations),
            Tool::CadPlanInspect { plan_id, part_id } => {
                self.inspect_cad_plan(plan_id, part_id.as_deref())
            }
            Tool::CadPlanCommit {
                plan_id,
                plan_revision,
            } => self.commit_cad_plan(plan_id, *plan_revision, cancel),
            Tool::RunPython {
                purpose,
                code,
                inputs,
            } => self.run_python(purpose, code, inputs, cancel),
            Tool::InspectDataset { path, revision } => {
                let (table, actual_revision) = crate::projects::datasets::inspect(
                    &self.project,
                    path,
                    revision.as_deref(),
                    cancel,
                )?;
                let reference = crate::projects::datasets::Reference {
                    path: path.clone(),
                    revision: actual_revision,
                    rows: table.rows.len(),
                    columns: table.columns.clone(),
                };
                Ok(table.summary(&reference))
            }
            Tool::ChartDataset {
                path,
                revision,
                x_column,
                y_columns,
                start_row,
                row_count,
                kind,
                title,
                x_label,
                y_label,
            } => {
                let table = crate::projects::datasets::load(&self.project, path, revision, cancel)?;
                let page = table.page(x_column, y_columns, *start_row, *row_count)?;
                if table.excel.as_ref().is_none_or(|m| m.plan.is_none()) {
                    crate::projects::excel_plan::check_unplanned_y(&page)?;
                }
                let prepared = crate::projects::charts::prepare_page(
                    &page, kind, title, x_label, y_label, path, revision,
                )?;
                let Some(mut chart) = self.review_chart(prepared, cancel)? else {
                    return Ok(self.deferred_chart());
                };
                // 使用者選擇異常值處理期間，CSV 也可能被外部程式改動。
                crate::projects::datasets::load(&self.project, path, revision, cancel)?;
                table.annotate(&mut chart);
                if let Some(meta) = &table.excel {
                    let mapping = page
                        .columns
                        .iter()
                        .zip(&page.headers)
                        .map(|(c, h)| {
                            format!("{c}/{}", h.text.chars().take(40).collect::<String>())
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    let selected = &table.rows[*start_row - 1..*start_row - 1 + *row_count];
                    let first = &selected[0];
                    let min = selected.iter().map(|r| r.row).min().unwrap_or(first.row);
                    let max = selected.iter().map(|r| r.row).max().unwrap_or(first.row);
                    chart.source = format!(
                        "{} | {} | 工作表 {} ({}) | X/Y: {} | 原始列 {}–{}（選取{}筆，可能不連續）",
                        first.path,
                        first.revision,
                        first.sheet,
                        meta.sheet_name,
                        mapping,
                        min,
                        max,
                        selected.len()
                    )
                    .chars()
                    .take(500)
                    .collect();
                    if let Some(window) = &meta.filter {
                        chart.data_note = format!(
                            "時間 [{} 至 {})；掃描{}列／符合{}筆。{}",
                            window.start,
                            window.end,
                            meta.scanned_rows,
                            table.rows.len(),
                            chart.data_note
                        )
                        .chars()
                        .take(500)
                        .collect();
                    }
                }
                self.add_chart(chart)
            }
            Tool::AskPreference { .. } => Err("偏好問題需由任務協調器保存。".into()),
            Tool::RunBatch { .. } => Err("批次工作只能由頂層 broker 執行，不允許巢狀批次。".into()),
            Tool::ReadSkillGuide { id, offset } => {
                let guide = crate::projects::skills::read_guide_for(id, &self.model)?;
                let total = guide.chars().count();
                if *offset > total {
                    return Err("技能說明位置超出範圍。".into());
                }
                let text: String = guide.chars().skip(*offset).take(3000).collect();
                Ok(
                    json!({"id":id,"text":text,"next_offset":offset+text.chars().count(),"total":total}),
                )
            }
            Tool::LoadSkill { id } => {
                if id.starts_with("outlook-")
                    && !self.mail_notes.notes.is_empty()
                    && self.mail_notes.stage != crate::projects::mail_notes::Stage::Read
                {
                    return Err("已有郵件成果；先查 read_mail_notes。確需回讀請用 set_work_stage(read,note_ids,reason)。".into());
                }
                self.activate_skill(id)?;
                let mut result = json!({"id":id,"loaded":self.loaded_skills,"instructions_location":"下一輪 system，工具定義位於 tools；無需重複載入。"});
                if id == "cad-new" {
                    result["planning_note_saved"] = json!(!self.cad_design.pending());
                    result["next_action"] = json!(if self.cad_design.pending() {
                        "本任務尚未保存短方案；必要條件未齊先ask_user，足夠就compact_context。不要靠重讀指南尋找尚未公告的cad_draw。"
                    } else {
                        "已沿用短方案；下一輪cad_plan_start建立計畫，每輪cad_plan_part只送1–20項。全部區段齊備才commit，不重讀指南或重送整圖。"
                    });
                }
                Ok(result)
            }
            Tool::SearchFiles { paths, query } => self.search_files(paths, query, worker, cancel),
            Tool::WriteExcelFormulas {
                copy_id,
                revision,
                sheet,
                cell,
                rows,
                dynamic,
            } => {
                if !self.loaded_skills.iter().any(|s| s == "excel-formulas") {
                    return Err("請先載入excel-formulas。".into());
                }
                self.office_action(
                    copy_id,
                    revision,
                    office::Action::ExcelFormulas {
                        sheet: *sheet,
                        cell: cell.clone(),
                        rows: rows.clone(),
                        dynamic: *dynamic,
                    },
                    cancel,
                )
            }
            Tool::OfficeBatch {
                copy_id,
                revision,
                operations,
            } => self.office_batch(copy_id, revision, operations.clone(), cancel),
            Tool::CreateMermaid {
                name,
                source,
                based_on,
            } => self.create_mermaid(name, source, based_on.as_deref(), cancel),
            Tool::InspectMermaid {
                message_index,
                diagram_index,
            } => self.inspect_mermaid(*message_index, *diagram_index),
            Tool::ExportMermaid {
                message_index,
                diagram_index,
                format,
            } => self.export_mermaid(*message_index, *diagram_index, *format, cancel),
            Tool::CreateChart { chart } => {
                if !self.excel_plans.is_empty() {
                    return Err("本任務已鎖定Excel分析欄位，請使用export_planned_excel及chart_dataset；不可手抄數列繞過來源核對。".into());
                }
                if !chart.data_note.is_empty()
                    || !chart.data_issues.is_empty()
                    || chart.transform.is_some()
                    || chart.quality.is_some()
                    || chart.style.is_some()
                    || chart.series.iter().any(|s| !s.skip_indices.is_empty())
                {
                    return Err("圖表的使用者處理紀錄只能由桌面預檢建立。".into());
                }
                self.add_chart((**chart).clone())
            }
            Tool::ExportChartPng { chart_index, name } => {
                self.export_chart_png(*chart_index, name, cancel)
            }
            Tool::InspectChart {
                message_index,
                chart_index,
            } => self.inspect_chart(*message_index, *chart_index),
            Tool::EditChart {
                message_index,
                chart_index,
                style,
            } => self.edit_chart(*message_index, *chart_index, style),
            Tool::SetChartReferenceLines { chart_index, lines } => {
                self.set_reference_lines(*chart_index, lines)
            }
            Tool::SetChartPolicy {
                chart_index,
                policy,
            } => {
                let original = self.charts.get(*chart_index).ok_or("找不到本次圖表。")?;
                let mut chart = policy.view(original)?;
                if let Some(style) = chart.style.as_mut() {
                    style.quality_policy = policy.clone();
                }
                chart.validate()?;
                let note = chart.data_note.clone();
                self.charts[*chart_index] = chart;
                Ok(
                    json!({"chart_index":chart_index,"data_note":note,"previous_png_files_unchanged":true,"notice":"如需PNG，請再次匯出以包含新政策。"}),
                )
            }
            Tool::TransformChart {
                chart_index,
                transform,
            } => {
                // 在本機從既有圖表衍生座標，不讓模型重抄資料或繞過 CSV 來源核對。
                let original = self
                    .charts
                    .get(*chart_index)
                    .ok_or("找不到本次任務的圖表編號，請先建立圖表。")?;
                let mut candidate = original.clone();
                if let Some(style) = candidate.style.as_mut() {
                    style.transform = Some(transform.clone());
                } else {
                    candidate.transform = Some(Box::new(transform.clone()));
                }
                candidate.validate()?;
                let serialized_size = |value: &crate::projects::charts::Chart| {
                    serde_json::to_vec(value)
                        .map(|bytes| bytes.len())
                        .map_err(|e| e.to_string())
                };
                let total = serde_json::to_vec(&self.charts)
                    .map_err(|e| e.to_string())?
                    .len()
                    - serialized_size(original)?
                    + serialized_size(&candidate)?;
                if total > 16 * 1024 * 1024 {
                    return Err("本次任務的圖表資料合計超過16 MiB，未套用轉換。".into());
                }
                // 呈現樣式可換圖型或缺值政策；工具預覽需與實際畫面使用相同資料。
                let source = if let Some(style) = &candidate.style {
                    style.quality_policy.view(&candidate)?
                } else {
                    candidate.clone()
                };
                let kind = candidate
                    .style
                    .as_ref()
                    .map_or(&candidate.kind, |s| &s.kind);
                let view = transform.view(&source, kind)?;
                let result = json!({"chart_index":chart_index,"transform":transform,
                    "original_rows":candidate.x.len(),"retained_rows":view.source_indices.len(),
                    "first_x":view.x.first(),"last_x":view.x.last(),
                    "series_preview":view.series.iter().map(|s| json!({"name":s.name,"first":s.values.first(),"last":s.values.last()})).collect::<Vec<_>>(),
                    "source_preserved":true,"previous_png_files_unchanged":true});
                self.charts[*chart_index] = candidate;
                Ok(result)
            }
            Tool::ChartFromExcel {
                path,
                revision,
                sheet,
                range,
                kind,
                title,
                x_label,
                y_label,
            } => {
                self.require_planned_export(path, *sheet)?;
                let content = self.content(path, cancel, worker)?;
                if text::revision(&content) != *revision {
                    return Err("文件版本已變更，請重新讀取。".into());
                }
                let prepared = crate::projects::charts::prepare_excel(
                    &content, *sheet, range, kind, title, x_label, y_label, path, revision,
                )?;
                let Some(chart) = self.review_chart(prepared, cancel)? else {
                    return Ok(self.deferred_chart());
                };
                if text::revision(&self.content(path, cancel, worker)?) != *revision {
                    return Err("文件版本已變更，請重新讀取。".into());
                }
                self.add_chart(chart)
            }
            Tool::AnalyzeImage { .. } => Err("圖片辨識需由專案任務協調器執行。".into()),
            Tool::SummarizeDocument { .. } => Err("摘要委派需由專案任務協調器執行。".into()),
            Tool::ReadWorkLog {
                operation_id,
                offset,
            } => {
                if let Some(reference) = operation_id {
                    let resolved = self.references.resolve(reference)?;
                    let id = &resolved;
                    if let Some(note) = self.reading.notes.get(id) {
                        let mut note = json!(note);
                        self.references.project(&mut note);
                        return Ok(json!({"operation_id":reference,"reading_note":note,
                            "notice":"此來源已整理。需要原始辨識／文件原文的具體缺項，改用read_reading_notes(source_operation,reason,offset)；不必重新尋找ID或重讀原檔。"}));
                    }
                    let (request, result) = self
                        .recorded_operation(id)?
                        .ok_or("找不到此操作的已保存結果。")?;
                    let log =
                        json!({"operation_id":id,"request":request,"result":result}).to_string();
                    let total = log.chars().count();
                    if *offset > total {
                        return Err("操作紀錄位置超過尾端。".into());
                    }
                    let part: String = log.chars().skip(*offset).take(6000).collect();
                    return Ok(
                        json!({"text":part,"offset":offset,"next_offset":offset+part.chars().count(),"total":total,"operation_id":id,
                            "outcome":work_log_outcome(&result),"notice":"outcome只摘錄結果狀態；完整請求及結果仍按next_offset查回。"}),
                    );
                }
                // 一次索引分頁期間固定內容，避免新操作插到字典中間使游標漂移。
                // 不把封存雜湊或工具正文混入目錄；細節仍以 operation_id 精確查回。
                if *offset == 0 || self.work_log_index.is_none() {
                    let mut index = self.archived_results.clone();
                    for (id, (request, result)) in &self.results {
                        index.insert(
                            id.clone(),
                            json!({"tool":request["tool"],"ok":result["ok"],"path":request["path"],"compare_path":request["compare_path"]}),
                        );
                    }
                    self.work_log_index = Some(
                        index
                            .into_iter()
                            .filter(|(_, v)| v["tool"] != "read_work_log")
                            .map(|(id, v)| json!({"operation_id":self.references.alias(&id),"tool":v["tool"],"ok":v["ok"],"path":v["path"],"compare_path":v["compare_path"]}))
                            .collect(),
                    );
                }
                let index = self.work_log_index.as_ref().ok_or("紀錄索引尚未建立。")?;
                if *offset > index.len() {
                    return Err("索引位置超出範圍；新版索引按筆數分頁，請從 offset=0 重取。".into());
                }
                let end = offset.saturating_add(20).min(index.len());
                Ok(
                    json!({"operations":index[*offset..end],"offset":offset,"next_offset":end,
                    "total":index.len(),"has_more":end<index.len(),"order":"固定快照；非時間順序",
                    "notice":"索引offset是筆數；指定operation_id後offset是原文字元。需要更新索引時從0重取。"}),
                )
            }
            Tool::ListNotes { query } => self.memory()?.list_notes(query),
            Tool::ReadNote { id } => self.memory()?.read_note(id),
            Tool::CreateNote { scope, title, body } => {
                self.memory()?.create_note(scope, title, body)
            }
            Tool::UpdateNote {
                id,
                revision,
                title,
                body,
            } => self
                .memory()?
                .change_note(id, revision, Some((title, body)), false),
            Tool::DeleteNote { id, revision } => {
                self.memory()?.change_note(id, revision, None, false)
            }
            Tool::RestoreNote { id, revision } => {
                self.memory()?.change_note(id, revision, None, true)
            }
            Tool::ListDocumentSections { path, offset } => {
                self.content(path, cancel, worker)?;
                self.memory()?.document_info(path, *offset)
            }
            Tool::ReadDocumentSection {
                path,
                revision,
                section_id,
            } => {
                self.txt_context = true;
                let mut result = self
                    .memory
                    .as_mut()
                    .ok_or("未啟用專案記憶。")?
                    .read_section(path, revision, section_id)?;
                self.reading.project_saved_read(&mut result);
                Ok(result)
            }
            Tool::UpdateDocumentNote {
                path,
                revision,
                note_revision,
                section_id,
                summary,
            } => self.memory()?.update_document_note(
                path,
                revision,
                note_revision,
                section_id.as_deref(),
                summary,
            ),
            Tool::ReadTaskResult {
                task_id,
                field,
                offset,
            } => self.memory()?.read_task_result(task_id, field, *offset),
            Tool::ListFiles { path, offset } => {
                if source_code::excluded(Path::new(path)) {
                    return Err("此資料夾屬於預設排除的路徑。".into());
                }
                let target = self.project.root.join(relative(path)?);
                let _guards = pin(&target)?;
                if let Some(directory) = _guards.last() {
                    reject_internal(directory)?;
                }
                let mut candidates = Vec::new();
                for (scanned, entry) in fs::read_dir(&target)
                    .map_err(|e| e.to_string())?
                    .enumerate()
                {
                    let entry = entry.map_err(|e| e.to_string())?;
                    if cancel.load(Ordering::Relaxed) {
                        return Err("已停止列出檔案。".into());
                    }
                    if scanned >= 100_000 {
                        return Err("單一資料夾超過100000項，請整理子資料夾後再列出；未宣稱已取得完整清單。".into());
                    }
                    if source_code::excluded(&entry.path()) {
                        continue;
                    }
                    if entry
                        .file_name()
                        .to_string_lossy()
                        .eq_ignore_ascii_case(".lmai")
                    {
                        continue;
                    }
                    let metadata = fs::symlink_metadata(entry.path()).map_err(|e| e.to_string())?;
                    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                        continue;
                    }
                    if metadata.is_dir() || metadata.is_file() {
                        candidates.push((entry, metadata));
                    }
                }
                // 固定排序後再分頁，不讓未知副檔名或位於舊500項掃描界線後的檔案消失。
                candidates.sort_by_cached_key(|(entry, _)| {
                    (
                        entry.file_name().to_string_lossy().to_lowercase(),
                        entry.file_name(),
                    )
                });
                let total = candidates.len();
                if *offset > total {
                    return Err(
                        "清單位置超過項目數；資料夾可能已變更，請從offset=0重新列出。".into(),
                    );
                }
                let mut entries = Vec::new();
                for (entry, metadata) in candidates.into_iter().skip(*offset).take(200) {
                    let mut info = json!({"name":entry.file_name().to_string_lossy(),"directory":metadata.is_dir()});
                    if metadata.is_dir() {
                        info["kind"] = json!("directory");
                        info["read_tool"] = json!("list_files");
                    }
                    if !metadata.is_dir() {
                        info["bytes"] = json!(metadata.len());
                        info["kind"] = json!("unclassified");
                        info["read_tool"] = json!("read_file");
                        info["text_support"] = json!("requires_content_check");
                        info["original_read_only"] = json!(true);
                    }
                    if !metadata.is_dir()
                        && entry
                            .path()
                            .extension()
                            .is_some_and(|e| e.eq_ignore_ascii_case("dwg"))
                    {
                        info["kind"] = json!("cad_dwg");
                        info["read_tool"] = json!("cad_read");
                        info["bytes"] = json!(metadata.len());
                    }
                    if !metadata.is_dir() && source_code::supported(&entry.path()) {
                        info["kind"] = json!("source_text");
                        info["bytes"] = json!(metadata.len());
                        info["read_tool"] = json!("read_code_section");
                        info["copy_editing"] = json!("after_text_validation_max_200KB");
                    }
                    // 圖片與文件一起列出，但只公告類型及讀取工具；列目錄不會送圖。
                    if !metadata.is_dir()
                        && crate::projects::vision::input::supported(&entry.path())
                    {
                        info["kind"] = json!("image");
                        info["bytes"] = json!(metadata.len());
                        if crate::projects::vision::input::model_supported(&self.model) {
                            info["read_tool"] = json!("analyze_image");
                        } else {
                            info["image_input_available"] = json!(false);
                        }
                    }
                    entries.push(info);
                }
                let next = offset + entries.len();
                Ok(
                    json!({"entries":entries,"offset":offset,"next_offset":(next<total).then_some(next),"total":total,"truncated":next<total,"default_exclusions":"不依副檔名隱藏檔案；版本庫、依賴、編譯產物、連結與常見機密設定仍排除。每頁200項，依next_offset續頁；資料夾變更時重新列出。本層清單不代表已讀取子目錄或全文。"}),
                )
            }
            Tool::ReadFile { path, offset } => {
                let canonical = self.code_copy_id(path).unwrap_or_else(|| path.clone());
                let path = &canonical;
                if crate::projects::vision::input::supported(Path::new(path)) {
                    // 與 LOG 的專用閱讀工具相同，回傳明確導引，不把二進位當文字。
                    // 仍驗證專案邊界、檔案與格式；此處不發出任何圖片辨識請求。
                    let image = crate::projects::vision::input::load(&self.project, path)?;
                    return Ok(
                        json!({"kind":"image","image":image.metadata(),"content_read":false,
                        "guidance":if crate::projects::vision::input::model_supported(&self.model) {
                            "這是專案圖片，尚未辨識內容。依任務需要直接呼叫 analyze_image(path,focus)，不必載入技能；不需要圖片資訊即可略過。"
                        } else {crate::projects::vision::input::UNSUPPORTED_MODEL}}),
                    );
                }
                if crate::projects::logs::supported(Path::new(path)) {
                    if *offset != 0 {
                        return Err(
                            "LOG 請載入 log-analysis，使用 read_log 的行號與 revision 續讀。"
                                .into(),
                        );
                    }
                    self.txt_context = true;
                    let mut result =
                        crate::projects::logs::read(&self.project, path, None, 1, 0, 30, cancel)?;
                    result["guidance"] = json!("這是 LOG 首頁。載入 log-analysis，使用 read_log 續讀或 search_logs 搜尋；不可使用全文摘要工具。");
                    return Ok(result);
                }
                let (content, source_revision) = self.content_snapshot(path, cancel, worker)?;
                let total = content.chars().count();
                if *offset > total {
                    return Err("讀取位置超過全文。".into());
                }
                let text: String = content.chars().skip(*offset).take(6000).collect();
                let next = offset + text.chars().count();
                let document =
                    if self.copies.contains_key(path) || !uses_document_memory(Path::new(path)) {
                        Value::Null
                    } else if let Some(memory) = self.memory.as_mut() {
                        memory.record_read(path, *offset, next);
                        memory.read_info(path, *offset, next)?
                    } else {
                        Value::Null
                    };
                let mut result = json!({"path":path,"working_copy":self.copies.contains_key(path),"text":text,"offset":offset,"next_offset":next,"total":total,"truncated":next<total,"revision":text::revision(&content),"source_revision":source_revision,"document":document,"imported_snapshot":self.project.imports.contains_key(&path.replace('\\', "/"))});
                self.reading.project_saved_read(&mut result);
                Ok(result)
            }
            Tool::ReadCodeSection {
                path,
                first_line,
                last_line,
            } => self.read_code_section(path, *first_line, *last_line, cancel, worker),
            Tool::EditCodeSection {
                copy_id,
                section_id,
                revision,
                first_line,
                last_line,
                section_hash,
                replacement,
            } => {
                if let Some(section_id) = section_id {
                    if !revision.is_empty()
                        || *first_line != 0
                        || *last_line != 0
                        || !section_hash.is_empty()
                    {
                        return Err("程式區段代號模式只需copy_id、section_id、replacement，請省略另填的行號與雜湊。".into());
                    }
                    self.replace_code_section(copy_id, section_id, replacement)
                } else {
                    self.edit_code_section(
                        copy_id,
                        revision,
                        *first_line,
                        *last_line,
                        section_hash,
                        replacement,
                    )
                }
            }
            Tool::FindText { path, text: needle } => {
                let canonical = self.code_copy_id(path).unwrap_or_else(|| path.clone());
                let path = &canonical;
                if needle.is_empty() {
                    return Err("搜尋文字不可空白。".into());
                }
                // 不將資料夾／glob 誤報為格式不支援，避免模型換工具反覆讀同一路徑。
                if path.contains('*') || path.contains('?') || path.ends_with(['/', '\\']) {
                    return Err("find_text 只接受單一明確檔案，支援 CPP/H 等唯讀原始碼；資料夾請先 list_files，取得檔名後用 search_files（全程式碼每批最多100檔、混合文件最多20檔）。不要重送相同資料夾或萬用字元。".into());
                }
                let content = self.content(path, cancel, worker)?;
                let matches: Vec<_> = content.match_indices(needle).take(100).map(|(index,_)| json!({"offset":content[..index].chars().count(),"line":content[..index].bytes().filter(|b| *b == b'\n').count()+1})).collect();
                let positions: Vec<_> = content
                    .match_indices(needle)
                    .take(101)
                    .map(|(index, _)| content[..index].chars().count())
                    .collect();
                Ok(
                    json!({"path":path,"copy_id":self.code_copy_id(path),"source_kind":if self.copies.contains_key(path){"working_copy"}else{"original_read_only"},"matches":matches,"total_lines":content.split_inclusive('\n').count(),"positions":positions.iter().take(100).collect::<Vec<_>>(),"truncated":positions.len()>100,"revision":text::revision(&content)}),
                )
            }
            Tool::CreateWorkingCopy { source, name } => {
                if self.copies.len() >= 20 {
                    return Err("本次工作副本已達 20 份。".into());
                }
                let rel = relative(name)?;
                if rel.components().count() != 1 {
                    return Err("工作副本只填檔名，不含資料夾。".into());
                }
                let ext = extension(&rel)?;
                if !plain_text_candidate(&rel)
                    && !office::supported(&rel)
                    && !matches!(ext.as_str(), "pdf" | "msg")
                {
                    return Err("CAD 與圖片請使用專用工具，不建立純文字封裝副本。".into());
                }
                if matches!(ext.as_str(), "pdf" | "msg") {
                    return Err("PDF／MSG 只支援 TXT 文字副本。".into());
                }
                // 前後檔案指紋必須一致，不能把讀取中途變更的來源綁到錯誤基準。
                let source_fingerprint = source
                    .as_ref()
                    .map(|path| fingerprint(&self.project, path))
                    .transpose()?;
                let (content, encoding) = if let Some(source) = source {
                    if ext != "md" {
                        self.txt_context = true;
                    }
                    let source_ext = extension(Path::new(source))?;
                    let expected_ext = if matches!(source_ext.as_str(), "pdf" | "msg") {
                        "txt"
                    } else {
                        &source_ext
                    };
                    if ext != expected_ext {
                        return Err("副本需保留來源格式；PDF／MSG 只建立 TXT 文字副本。".into());
                    }
                    read_cancel(
                        &self.project,
                        source,
                        cancel,
                        Some(worker),
                        self.server_pdf.as_mut(),
                    )?
                } else {
                    if matches!(ext.as_str(), "docx" | "xlsx" | "pptx") {
                        self.txt_context = true;
                        (
                            office::render(None, &rel, None, &[], None, cancel)?.serialize()?,
                            Encoding::Utf8(true),
                        )
                    } else if office::supported(&rel) {
                        return Err(
                            "舊版 Office 請從來源建立副本；新文件可用 Markdown、TXT、一般文字格式或 DOCX／XLSX／PPTX。"
                                .into(),
                        );
                    } else {
                        (String::new(), Encoding::Utf8(true))
                    }
                };
                if !office::supported(&rel) {
                    // 原文閱讀可達4MiB；所有一般文字副本維持200KB的既有編輯／編碼上限。
                    text::validate(&content)?;
                }
                if let (Some(path), Some(expected)) = (source, &source_fingerprint) {
                    if fingerprint(&self.project, path)? != *expected {
                        return Err("建立副本期間原件已變更，請重新讀取後再建立。".into());
                    }
                }
                let origin = Some(match source {
                    Some(path) => super::super::review::Origin::Source {
                        path: path.clone(),
                        fingerprint: source_fingerprint.clone().unwrap_or_default(),
                    },
                    None => super::super::review::Origin::New,
                });
                let office = if office::supported(&rel) {
                    let original: office::Snapshot =
                        serde_json::from_str(&content).map_err(|e| e.to_string())?;
                    Some(OfficeCopy {
                        source: source.clone(),
                        fingerprint: source_fingerprint.unwrap_or_default(),
                        desired: original.clone(),
                        original: source.as_ref().map(|_| original),
                        actions: Vec::new(),
                    })
                } else {
                    None
                };
                let id = crate::jobs::new_id()?;
                let revision = text::revision(&content);
                self.copies.insert(
                    id.clone(),
                    Copy {
                        origin,
                        baseline: Some(content.clone()),
                        reviews: BTreeMap::new(),
                        office,
                        name: name.clone(),
                        code_review: crate::projects::code_review::State {
                            baseline: (ext == "py").then(|| content.clone()),
                            ..Default::default()
                        },
                        text: content,
                        encoding,
                        saved_revision: None,
                        python_checked_revision: None,
                        draft: None,
                        code_sections: vec![],
                        paths: Vec::new(),
                    },
                );
                Ok(json!({"copy_id":id,"revision":revision}))
            }
            Tool::EditText {
                copy_id,
                revision,
                start,
                expected,
                replacement,
            } => {
                let copy = self
                    .copies
                    .get_mut(copy_id)
                    .ok_or("不是本次任務的工作副本。")?;
                if copy.office.is_some() {
                    return Err(
                        "Office 副本請使用 edit_office 修改區塊，不能用純文字索引修改封裝。".into(),
                    );
                }
                let next = worker.edit(
                    &Edit {
                        content: copy.text.clone(),
                        revision: revision.clone(),
                        start: *start,
                        expected: expected.clone(),
                        replacement: replacement.clone(),
                    },
                    cancel,
                )?;
                // 即使子程序出錯，broker 也只接受相同固定操作的結果。
                if next != text::edit(&copy.text, revision, *start, expected, replacement)? {
                    return Err("子程序修改結果不一致。".into());
                }
                if extension(Path::new(&copy.name))? == "py" {
                    return self.save_python_draft(copy_id, &next);
                }
                copy.text = next;
                Ok(json!({"copy_id":copy_id,"revision":text::revision(&copy.text)}))
            }
            Tool::EditOffice {
                copy_id,
                revision,
                block_id,
                expected,
                replacement,
            } => self.office_action(
                copy_id,
                revision,
                office::Action::Edit {
                    block_id: block_id.clone(),
                    expected: expected.clone(),
                    replacement: replacement.clone(),
                },
                cancel,
            ),
            Tool::OfficeAction {
                copy_id,
                revision,
                operation,
            } => self.office_action(copy_id, revision, *operation.clone(), cancel),
            Tool::PlanCodeChange {
                copy_id,
                requirements,
            } => self.plan_code_change(copy_id, requirements),
            Tool::TestPython {
                copy_id,
                revision,
                tests,
            } => self.test_python(copy_id, revision, tests, cancel),
            Tool::ReviewCodeChange {
                copy_id,
                revision,
                checks,
            } => self.review_code_change(copy_id, revision, checks),
            Tool::CheckPython { path, revision } => {
                self.check_python(path, revision, cancel, worker)
            }
            Tool::SaveCopy { copy_id, revision } => {
                let copy = self
                    .copies
                    .get_mut(copy_id)
                    .ok_or("不是本次任務的工作副本。")?;
                if text::revision(&copy.text) != *revision {
                    return Err("版本已改變，請重新讀取。".into());
                }
                if extension(Path::new(&copy.name))? == "py"
                    && copy.python_checked_revision.as_ref() != Some(revision)
                {
                    return Err(
                        "PY副本需先用check_python核對目前revision並通過語法檢查，再save_copy。"
                            .into(),
                    );
                }
                if extension(Path::new(&copy.name))? == "py" {
                    copy.code_review.ready(revision)?;
                }
                if copy.saved_revision.as_ref() == Some(revision) {
                    return Ok(
                        json!({"copy_id":copy_id,"path":copy.paths.last(),"revision":revision}),
                    );
                }
                // PY 每段已落盤；檢查通過只升級同一路徑為完成成果，不產生 _1、_2。
                if let Some(draft) = &copy.draft {
                    let path = draft.path.clone();
                    if read_cancel(&self.project, &path, cancel, Some(worker), None)?.0 != copy.text
                    {
                        return Err("草稿內容已變更，未發布。".into());
                    }
                    if !copy.paths.contains(&path) {
                        copy.paths.push(path.clone());
                    }
                    if !self.published.contains(&path) {
                        self.published.push(path.clone());
                    }
                    copy.saved_revision = Some(revision.clone());
                    return Ok(
                        json!({"copy_id":copy_id,"path":path,"revision":revision,"verified":true,"syntax_valid":true}),
                    );
                }
                if self.txt_context && extension(Path::new(&copy.name))? == "md" {
                    return Err(
                        "本次任務讀取過 TXT，禁止輸出至未加密的 MD。請建立 TXT 成果。".into(),
                    );
                }
                // 原檔版本與內容都核對，格式或其他非文字部分變動也不能沿用舊副本。
                if let Some(office) = &copy.office {
                    if office
                        .source
                        .as_ref()
                        .map(|s| fingerprint(&self.project, s))
                        .transpose()?
                        .is_some_and(|stamp| stamp != office.fingerprint)
                    {
                        return Err("Office 原檔已變動，請重新建立副本。".into());
                    }
                }
                let _root = pin(&self.project.root)?;
                let base = self.project.root.join("_AI_Output");
                match fs::create_dir(&base) {
                    Ok(()) => (),
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
                    Err(e) => return Err(e.to_string()),
                }
                let _base = pin(&base)?;
                let folder_name = shared_output_folder(
                    &base,
                    &mut self.output_folder,
                    self.output_conversation.as_deref(),
                )?;
                let folder = base.join(&folder_name);
                let _folder = pin(&folder)?;
                let (name, mut output) = reserve_output(&folder, &copy.name)?;
                // 路徑先記錄，失敗時也能告知使用者可能已建立的檔案。
                let relative = format!("_AI_Output/{folder_name}/{name}");
                self.published.push(relative.clone());
                if let Some(office) = &copy.office {
                    // Office 先寫入本次獨立暫存目錄；發布仍以已保留的 create_new handle 寫入，
                    // 不讓 Office 的 SaveAs 覆寫使用者檔案。公司加密是否允許此複製，最後由 Office 讀回確認。
                    let stage = folder.join(format!(".office_{}", crate::jobs::new_id()?));
                    fs::create_dir(&stage).map_err(|e| e.to_string())?;
                    let stage_guard = pin_stage(&stage)?;
                    let staged = stage.join(&copy.name);
                    let saved = (|| {
                        let snapshot = render_office(
                            &self.project,
                            &copy.name,
                            office,
                            Some(&staged),
                            cancel,
                        )?;
                        if snapshot != office.desired {
                            return Err("Office 重建結果與工作版本不一致。".into());
                        }
                        let mut input = checked_file(&staged)?;
                        if input.metadata().map_err(|e| e.to_string())?.len() > 50_000_000 {
                            return Err("Office 成果超過 50 MB。".into());
                        }
                        std::io::copy(&mut input, &mut output).map_err(|e| e.to_string())?;
                        output.sync_all().map_err(|e| e.to_string())
                    })();
                    // 暫存位置只由程式產生，不遞迴刪除、不跟隨模型提供的路徑。
                    let _ = fs::remove_file(&staged);
                    drop(stage_guard);
                    let _ = fs::remove_dir(&stage);
                    saved.map_err(|e: String| format!("可能已建立 {relative}，尚未交付：{e}"))?;
                } else {
                    let bytes = text::encode(&copy.text, copy.encoding)?;
                    output
                        .write_all(&bytes)
                        .and_then(|_| output.sync_all())
                        .map_err(|e| format!("可能有部分輸出 {relative}，未交付：{e}"))?;
                }
                drop(output);
                let verified = read_cancel(&self.project, &relative, cancel, Some(worker), None).map_err(|e| format!("已建立 {relative}，但無法驗證加密後內容，尚未交付。請用對應應用程式檢查：{e}"))?.0;
                if verified != copy.text {
                    return Err(format!("已建立 {relative}，讀回內容不一致，尚未交付。"));
                }
                copy.saved_revision = Some(revision.clone());
                copy.paths.push(relative.clone());
                copy.reviews
                    .insert(relative.clone(), copy.comparison(&relative, false));
                Ok(json!({"copy_id":copy_id,"path":relative,"revision":revision,"verified":true}))
            }
            Tool::DeleteCopy { copy_id } => {
                let copy = self.copies.get(copy_id).ok_or("不是本次任務的工作副本。")?;
                if !copy.paths.is_empty() || copy.draft.is_some() {
                    return Err("已保存的草稿或成果不可刪除；請由使用者管理。".into());
                }
                self.copies.remove(copy_id);
                Ok(json!({"discarded":copy_id}))
            }
        }
    }
    /// 驗證通過才替換記憶體版本；失敗的格式／結構操作不會污染先前成功的工作。
    pub(super) fn office_action(
        &mut self,
        id: &str,
        revision: &str,
        action: office::Action,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        self.office_batch(id, revision, vec![action], cancel)
    }
    /// 整批在候選文件重建成功後才提交；失敗不修改目前副本，且只啟動一次 Office。
    pub(super) fn office_batch(
        &mut self,
        id: &str,
        revision: &str,
        mut actions: Vec<office::Action>,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        if actions.is_empty() || actions.len() > 20 {
            return Err("每批需為 1–20 個 Office 操作。".into());
        }
        for action in &mut actions {
            if matches!(action, office::Action::ExcelFormulas { .. })
                && !self.loaded_skills.iter().any(|s| s == "excel-formulas")
            {
                return Err("寫入公式需獨立excel-formulas技能。".into());
            }
            if let office::Action::InsertImage { path, sha256, .. } = action {
                if sha256.is_some() {
                    return Err("圖片雜湊由程式管理，不接受模型提供的 sha256。".into());
                }
                *sha256 = Some(locked_image(&self.project, path)?.2);
            }
        }
        let copy = self.copies.get_mut(id).ok_or("不是本次任務的工作副本。")?;
        if text::revision(&copy.text) != revision {
            return Err("版本已改變，請重新讀取。".into());
        }
        let office = copy.office.as_mut().ok_or("此工具只適用 Office 副本。")?;
        let mut candidate = office.clone();
        candidate.actions.extend(actions);
        if candidate.actions.len() > 200 {
            return Err("單一文件最多 200 次修改；請發布後建立新副本。".into());
        }
        let snapshot = render_office(&self.project, &copy.name, &candidate, None, cancel)?;
        let content = snapshot.serialize()?;
        let result = json!({"copy_id":id,"revision":text::revision(&content),"structure":snapshot.structure,"calculation":snapshot.calculation,
            "blocks_tail":snapshot.blocks.iter().rev().take(12).map(|b| json!({"id":b.id,"label":b.label,"kind":b.kind,"format":b.format,"preview":b.text.chars().take(120).collect::<String>()})).collect::<Vec<_>>(),
            "hint":"結構改變後使用新版本與區塊 ID；完整內容可用 read_file(copy_id) 讀取。"});
        candidate.desired = snapshot;
        *office = candidate;
        copy.text = content;
        Ok(result)
    }
}
