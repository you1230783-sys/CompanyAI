//! 有界並行工作：唯讀快照／計算最多兩個同時進行，成果只由主 broker 依序提交。
//! 不允許依賴本批新成果、同檔編輯、Outlook 授權或巢狀批次，避免未知副作用被重播。
use super::*;
use crate::projects::{charts, datasets, python};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub task_id: String,
    pub tool: String,
    pub arguments_json: String,
}
impl Task {
    pub fn request(&self) -> AppResult<Tool> {
        crate::jobs::validate_id(&self.task_id)?;
        if self.arguments_json.len() > 64_000 {
            return Err("批次單項參數超過64 KB。".into());
        }
        let mut arguments: Value = serde_json::from_str(&self.arguments_json)
            .map_err(|e| format!("批次參數JSON不正確：{e}"))?;
        let object = arguments
            .as_object_mut()
            .ok_or("批次參數必須為JSON物件。")?;
        if object.contains_key("tool")
            || object.contains_key("progress_note")
            || object.contains_key("mail_note")
        {
            return Err(format!("批次子項 {} 的 arguments_json 不得包含 tool／progress_note／mail_note；只保留子工具參數，progress_note 請放在外層 run_batch。", self.task_id));
        }
        if self.tool.starts_with("cad_")
            || (self.tool == "read_code_section"
                && ["snapshot_id", "details", "filter"]
                    .iter()
                    .any(|key| object.contains_key(*key)))
        {
            return Err("run_batch不支援CAD工具。CAD圖元查詢請直接呼叫cad_query，不要把snapshot_id／details等參數包成read_code_section；後者只讀程式文字。".into());
        }
        arguments = crate::projects::agent::restore_optional(&self.tool, arguments)?;
        arguments
            .as_object_mut()
            .ok_or("批次參數必須為JSON物件。")?
            .insert("tool".into(), json!(self.tool));
        let request: Tool = serde_json::from_value(arguments)
            .map_err(|e| format!("批次工具參數不符（子項 {}）：{e}", self.task_id))?;
        if !matches!(
            request,
            Tool::ListFiles { .. }
                | Tool::ReadFile { .. }
                | Tool::ReadCodeSection { .. }
                | Tool::FindText { .. }
                | Tool::InspectDataset { .. }
                | Tool::ChartDataset { .. }
                | Tool::RunPython { .. }
        ) {
            return Err("此工具不能並行；請單獨執行，修改同一副本必須依序。".into());
        }
        Ok(request)
    }
}

enum Prepared {
    Cached(Value),
    Read(Value),
    Python {
        sources: Vec<Value>,
        response: Value,
    },
    Chart(Box<DatasetChart>),
}
struct DatasetChart {
    table: datasets::Table,
    page: office::excel::Page,
    chart: charts::quality::Prepared,
}

/// scoped threads 只持有不可變輸入；每組先全部啟動再 join，並非依序執行的假並行。
fn pair_map<T: Sync, R: Send>(items: &[T], work: impl Fn(&T) -> R + Sync) -> Vec<R> {
    std::thread::scope(|scope| {
        let handles: Vec<_> = items
            .iter()
            .map(|item| {
                let work = &work;
                scope.spawn(move || work(item))
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| match handle.join() {
                Ok(value) => value,
                Err(panic) => std::panic::resume_unwind(panic),
            })
            .collect()
    })
}

fn compute(
    project: &Project,
    copies: &BTreeMap<String, (String, String, Option<drafts::Draft>)>,
    reading: &crate::projects::reading::Notebook,
    request: &Tool,
    executable: &Path,
    cancel: &AtomicBool,
) -> AppResult<Prepared> {
    crate::projects::runner::check_cancel(cancel)?;
    if let Tool::RunPython {
        purpose,
        code,
        inputs,
    } = request
    {
        if purpose.trim().is_empty() || purpose.chars().count() > 1000 {
            return Err("Python 分析需提供1–1000字目的。".into());
        }
        let snapshot = python::snapshot(project, inputs, cancel)?;
        let sources = snapshot.as_array().ok_or("Python來源格式無效。")?.iter().map(|v|
            json!({"name":v["name"],"path":v["path"],"kind":v["kind"],"revision":v["revision"],"scope":v["scope"],"start_line":v["start_line"],"line_count":v["line_count"],"next_line":v["next_line"],"eof":v["eof"]})).collect();
        // python::execute 每次建立獨立 AppContainer／Job／IPC，不共享 Python 全域變數。
        let response = python::execute(code, snapshot, cancel)?;
        return Ok(Prepared::Python { sources, response });
    }
    if let Tool::ChartDataset {
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
    } = request
    {
        let table = datasets::load(project, path, revision, cancel)?;
        let page = table.page(x_column, y_columns, *start_row, *row_count)?;
        if table.excel.as_ref().is_none_or(|m| m.plan.is_none()) {
            crate::projects::excel_plan::check_unplanned_y(&page)?;
        }
        let chart = charts::prepare_page(&page, kind, title, x_label, y_label, path, revision)?;
        return Ok(Prepared::Chart(Box::new(DatasetChart {
            table,
            page,
            chart,
        })));
    }
    let mut broker = Broker::new(project.clone(), crate::jobs::new_id()?)?;
    broker.reading = reading.clone();
    for (id, (name, text, draft)) in copies {
        broker.copies.insert(
            id.clone(),
            Copy {
                baseline: None,
                reviews: BTreeMap::new(),
                office: None,
                name: name.clone(),
                text: text.clone(),
                encoding: Encoding::Utf8(false),
                saved_revision: None,
                python_checked_revision: None,
                code_review: Default::default(),
                draft: draft.clone(),
                code_sections: vec![],
                paths: vec![],
            },
        );
    }
    let mut worker = Worker::start(executable, cancel)?;
    broker
        .perform(request, &mut worker, cancel)
        .map(Prepared::Read)
}

impl Broker {
    pub(super) fn run_batch(
        &mut self,
        id: &str,
        tasks: &[Task],
        executable: &Path,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        if !(2..=8).contains(&tasks.len()) {
            return Err(
                "run_batch需要2–8項；超過4項只允許read_file／read_code_section文字閱讀。".into(),
            );
        }
        let mut seen = std::collections::BTreeSet::new();
        let requests: Vec<_> = tasks
            .iter()
            .map(|task| {
                if !seen.insert(&task.task_id) {
                    return Err("批次task_id不可重複。".into());
                }
                if !crate::projects::skills::enabled(&task.tool, &self.loaded_skills) {
                    return Err(format!("尚未載入 {} 所需技能。", task.tool));
                }
                task.request()
            })
            .collect::<AppResult<_>>()?;
        let text_batch = requests
            .iter()
            .all(|r| matches!(r, Tool::ReadFile { .. } | Tool::ReadCodeSection { .. }));
        if tasks.len() > 4 && !text_batch {
            return Err("只有獨立文字閱讀可一次提交5–8段；其他工具仍最多4項。".into());
        }
        let read_budget = if text_batch {
            16_000 / requests.len()
        } else {
            6_000
        };
        // 所有來源在批次開始時就必須存在；不能讓後一項依賴前一項剛生成的檔案。
        // 每項各自記錄錯誤，某一來源不存在不會取消其他有效工作。
        let preflight: Vec<AppResult<Option<Value>>> = tasks
            .iter()
            .zip(&requests)
            .map(|(task, request)| {
                let operation = text::revision(&format!("batch:{id}:{}", task.task_id));
                if let Some(cached) = self.cached_result(&operation, request)? {
                    return Ok(Some(cached));
                }
                let paths: Vec<&str> = match request {
                    Tool::ReadFile { path, .. }
                    | Tool::ReadCodeSection { path, .. }
                    | Tool::FindText { path, .. }
                    | Tool::InspectDataset { path, .. }
                    | Tool::ChartDataset { path, .. } => vec![path],
                    Tool::RunPython { inputs, .. } => {
                        inputs.iter().map(|input| input.path.as_str()).collect()
                    }
                    // list_files由perform固定目錄handle並驗證範圍；不可用checked_file
                    // 預先開啟資料夾，否則Windows會誤報檔案存取被拒。
                    _ => vec![],
                };
                for path in paths {
                    if self.copies.contains_key(path) {
                        continue;
                    }
                    let relative = relative(path)?;
                    if !matches!(
                        request,
                        Tool::RunPython { .. }
                            | Tool::ChartDataset { .. }
                            | Tool::InspectDataset { .. }
                    ) && relative
                        .extension()
                        .and_then(|s| s.to_str())
                        .is_some_and(|ext| {
                            matches!(
                                ext.to_ascii_lowercase().as_str(),
                                "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "msg" | "pdf"
                            )
                        })
                    {
                        return Err(
                            "Office／郵件／PDF閱讀請單独執行；批次文字閱讀不共用COM或伺服器連線。"
                                .into(),
                        );
                    }
                    let full = self.project.root.join(relative);
                    let _guards = pin(full.parent().ok_or("來源缺少資料夾。")?)?;
                    reject_internal(&checked_file(&full)?)?;
                }
                Ok(None)
            })
            .collect();
        // 副本只提供不可變文字快照；不將 Office／權限／回呼傳入工作執行緒。
        let copies = self
            .copies
            .iter()
            .filter(|(_, c)| c.office.is_none())
            .map(|(id, c)| {
                (
                    id.clone(),
                    (c.name.clone(), c.text.clone(), c.draft.clone()),
                )
            })
            .collect();
        let mut results = Vec::new();
        for (group, jobs) in requests.chunks(2).enumerate() {
            let started = crate::calendar::local_timestamp();
            let project = &self.project;
            let reading = &self.reading;
            let jobs: Vec<_> = jobs.iter().enumerate().collect();
            let computed = pair_map(&jobs, |(offset, request)| {
                match &preflight[group * 2 + offset] {
                    Ok(Some(result)) => return Ok(Prepared::Cached(result.clone())),
                    Err(error) => return Err(error.clone()),
                    Ok(None) => (),
                }
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    compute(project, &copies, reading, request, executable, cancel)
                }))
                .unwrap_or_else(|_| Err("並行工作非預期中止，未提交該項成果。".into()))
            });
            for (offset, prepared) in computed.into_iter().enumerate() {
                let index = group * 2 + offset;
                let task = &tasks[index];
                let request = &requests[index];
                let operation = text::revision(&format!("batch:{id}:{}", task.task_id));
                if let Ok(Prepared::Cached(result)) = &prepared {
                    results.push(json!({"task_id":task.task_id,"tool":task.tool,"reused":true,"outcome":result}));
                    continue;
                }
                let result = match prepared.and_then(|prepared| {
                    crate::projects::runner::check_cancel(cancel)?;
                    match (prepared, request) {
                        (Prepared::Read(mut value), _) => {
                            if matches!(
                                request,
                                Tool::ReadFile { .. } | Tool::ReadCodeSection { .. }
                            ) {
                                limit_read(&mut value, read_budget)?;
                                // 批次配額可能再次縮短區段，須以實際交付範圍核對筆記。
                                self.reading.project_saved_read(&mut value);
                            }
                            Ok(value)
                        }
                        (
                            Prepared::Python { sources, response },
                            Tool::RunPython { purpose, code, .. },
                        ) => self.publish_python(purpose, code, sources, response, cancel),
                        (
                            Prepared::Chart(chart),
                            Tool::ChartDataset {
                                path,
                                revision,
                                start_row,
                                row_count,
                                ..
                            },
                        ) => self.commit_batch_chart(
                            *chart, path, revision, *start_row, *row_count, cancel,
                        ),
                        _ => Err("批次結果與工具不一致，未提交。".into()),
                    }
                }) {
                    Ok(value) => json!({"ok":true,"operation_id":operation,"result":value}),
                    Err(error) => {
                        json!({"ok":false,"operation_id":operation,"error":error,"retry_same_operation":false})
                    }
                };
                self.remember_result(&operation, request, &result)?;
                self.analysis.observe(request, &result);
                self.archive_results()?;
                results.push(json!({"task_id":task.task_id,"tool":task.tool,"started_local":started,"finished_local":crate::calendar::local_timestamp(),"outcome":result}));
            }
        }
        Ok(
            json!({"execution":"parallel_compute_serial_commit","max_parallel":2,"text_budget_chars":16000,"tasks":results,
            "notice":"每項獨立成功或失敗；成功項目不重做，只另提失敗項目。不能把整批已回傳當作每項均成功。"}),
        )
    }

    fn commit_batch_chart(
        &mut self,
        prepared: DatasetChart,
        path: &str,
        revision: &str,
        start: usize,
        count: usize,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        let DatasetChart { table, page, chart } = prepared;
        let Some(mut chart) = self.review_chart(chart, cancel)? else {
            return Ok(self.deferred_chart());
        };
        datasets::load(&self.project, path, revision, cancel)?;
        table.annotate(&mut chart);
        if let Some(meta) = &table.excel {
            let rows = &table.rows[start - 1..start - 1 + count];
            let first = rows.first().ok_or("作圖範圍沒有資料。")?;
            let mapping = page
                .columns
                .iter()
                .zip(&page.headers)
                .map(|(c, h)| format!("{c}/{}", h.text.chars().take(40).collect::<String>()))
                .collect::<Vec<_>>()
                .join(", ");
            chart.source = format!(
                "{} | {} | 工作表 {} ({}) | X/Y: {} | 原始列 {}–{}（選取{}筆，可能不連續）",
                first.path,
                first.revision,
                first.sheet,
                meta.sheet_name,
                mapping,
                rows.iter().map(|r| r.row).min().unwrap_or(first.row),
                rows.iter().map(|r| r.row).max().unwrap_or(first.row),
                rows.len()
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
}

/// 整批正文共用16K字元，程式閱讀按整行縮短，不冒稱原要求範圍已交付。
/// 所有版本雜湊仍針對完整來源；section_hash只對應實際回傳原文。
fn limit_read(value: &mut Value, budget: usize) -> AppResult<()> {
    let Some(text) = value["text"].as_str() else {
        return Ok(());
    };
    if text.chars().count() <= budget {
        return Ok(());
    }
    let code = value["first_line"].as_u64();
    let text = if code.is_some() {
        let mut part = String::new();
        let mut chars = 0;
        for line in text.split_inclusive('\n') {
            let count = line.chars().count();
            if chars + count > budget {
                break;
            }
            chars += count;
            part.push_str(line);
        }
        if part.is_empty() {
            return Err("單行超過本批正文配額，請減少批次段數再定向閱讀。".into());
        }
        part
    } else {
        text.chars().take(budget).collect()
    };
    let end = value["offset"].as_u64().ok_or("閱讀結果缺少offset。")? + text.chars().count() as u64;
    value["text"] = json!(text);
    value["next_offset"] = json!(end);
    value["truncated"] = json!(true);
    if let Some(first) = code {
        let last = first + text.split_inclusive('\n').count() as u64 - 1;
        value["last_line"] = json!(last);
        value["next_line"] = json!(last + 1);
        value["eof"] = json!(false);
        value["section_hash"] = json!(text::revision(&text));
        if let (Some(copy), Some(revision)) =
            (value["copy_id"].as_str(), value["revision"].as_str())
        {
            value["section_id"] = json!(super::code_sections::section_id(
                copy,
                revision,
                first as usize,
                last as usize
            ));
        }
    }
    value["batch_budget_applied"] = json!(true);
    Ok(())
}

#[cfg(test)]
mod budget_tests {
    use super::*;
    #[test]
    fn whole_lines_and_actual_copy_reference_survive_batch_budget() {
        let mut data = json!({"text":"甲乙\n丙丁\n戊己\n","offset":10,"next_offset":19,"total":99,"first_line":4,"last_line":6,"copy_id":"copy","revision":"revision","section_id":"old","section_hash":"old"});
        limit_read(&mut data, 7).unwrap();
        assert_eq!(data["text"], "甲乙\n丙丁\n");
        assert_eq!(data["next_offset"], 16);
        assert_eq!(data["last_line"], 5);
        assert_eq!(data["next_line"], 6);
        assert_eq!(
            data["section_id"],
            super::super::code_sections::section_id("copy", "revision", 4, 5)
        );
        assert_eq!(data["section_hash"], text::revision("甲乙\n丙丁\n"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn batch_uses_the_same_optional_defaults_as_single_calls() {
        let request = Task {
            task_id: "read".into(),
            tool: "read_file".into(),
            arguments_json: r#"{"path":"README.md","offset":null}"#.into(),
        }
        .request()
        .unwrap();
        assert!(matches!(request, Tool::ReadFile { offset: 0, .. }));
        // 必填path仍不能為null，未知欄位也不會因還原預設而被忽略。
        for arguments in [
            r#"{"path":null,"offset":null}"#,
            r#"{"path":"README.md","offset":null,"unknown":true}"#,
        ] {
            assert!(Task {
                task_id: "read".into(),
                tool: "read_file".into(),
                arguments_json: arguments.into()
            }
            .request()
            .is_err());
        }
    }
    #[test]
    fn argument_errors_identify_the_child_and_explain_where_progress_belongs() {
        let error = Task {
            task_id: "list-root".into(),
            tool: "list_files".into(),
            arguments_json: r#"{"path":"","progress_note":"列出資料夾"}"#.into(),
        }
        .request()
        .err()
        .unwrap();
        assert!(error.contains("list-root") && error.contains("不得包含"));
        assert!(error.contains("外層 run_batch"));
        let error = Task {
            task_id: "bad-fields".into(),
            tool: "list_files".into(),
            arguments_json: r#"{"path":"","unexpected":true}"#.into(),
        }
        .request()
        .err()
        .unwrap();
        assert!(error.contains("bad-fields") && error.contains("unexpected"));
    }

    #[test]
    fn workers_overlap_and_keep_result_order() {
        let barrier = std::sync::Barrier::new(2);
        assert_eq!(
            pair_map(&[1, 2], |n| {
                barrier.wait();
                n * 10
            }),
            vec![10, 20]
        );
    }
    #[test]
    fn batch_cannot_edit_or_nest_and_rejects_unknown_arguments() {
        for (tool, args) in [
            ("delete_copy", r#"{"copy_id":"a"}"#),
            ("read_file", r#"{"path":"a.py","offset":0,"shell":"cmd"}"#),
        ] {
            assert!(Task {
                task_id: "a".into(),
                tool: tool.into(),
                arguments_json: args.into()
            }
            .request()
            .is_err());
        }
    }
}
