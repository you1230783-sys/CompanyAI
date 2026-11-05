//! Python 成果發布與恢復檢查；所有寫入仍由主程序保留既有專案路徑界線。
use super::*;
use crate::projects::python;
use sha2::{Digest, Sha256};

/// 只提供下一步提示，絕不把模型列印的 JSON 提升成可引用的計算摘要。
fn summary_guidance(response: &Value) -> Option<&'static str> {
    let empty = response["summary"]
        .as_object()
        .is_some_and(|summary| summary.is_empty());
    let printed_object = response["stdout"]
        .as_str()
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
        .is_some_and(|value| value.is_object());
    (empty && printed_object).then_some("數值請指定給 `result = {...}`；`record_analysis` 證據與 `register_report_values` 都使用 `/summary/...`。不需重跑已成功的計算，下一次計算時改用 result。")
}

/// emit 工具已決定成果格式；省略副檔名時補上，明確指定錯誤格式時則拒絕。
/// 仍只接受單一安全檔名，不能藉自動補副檔名取得任意路徑寫入能力。
fn output_name(name: &str, kind: &str) -> AppResult<String> {
    let extension = match kind {
        "table" => "csv",
        "xlsx" => "xlsx",
        "png" => "png",
        _ => {
            return Err(format!(
                "Python 成果種類 {kind:?} 不支援；請使用 emit_table、emit_excel或emit_figure。"
            ))
        }
    };
    let path = relative(name)?;
    if path.components().count() != 1 {
        return Err("Python 成果只接受單一檔名，不可包含資料夾。".into());
    }
    match path.extension() {
        None => Ok(format!("{name}.{extension}")),
        Some(actual) if actual.eq_ignore_ascii_case(extension) => Ok(name.into()),
        Some(_) => Err(format!(
            "Python 成果檔名「{name}」與格式不符；此輸出需使用 .{extension}，例如「分析.{extension}」。不要修改 result 結構或來源 kind。"
        )),
    }
}

#[cfg(test)]
mod output_name_tests {
    use super::output_name;

    #[test]
    fn missing_extensions_are_added_without_changing_explicit_formats() {
        assert_eq!(output_name("eis_data", "table").unwrap(), "eis_data.csv");
        assert_eq!(
            output_name("probe_book", "xlsx").unwrap(),
            "probe_book.xlsx"
        );
        assert_eq!(output_name("量測.CSV", "table").unwrap(), "量測.CSV");
        assert!(output_name("plot.png", "table")
            .unwrap_err()
            .contains(".csv"));
        assert!(output_name("book.csv", "xlsx")
            .unwrap_err()
            .contains(".xlsx"));
    }

    #[test]
    fn automatic_extensions_preserve_path_and_format_boundaries() {
        for name in [
            "",
            "../escape",
            "folder/result",
            "C:\\result",
            "CON",
            "data.",
            "data ",
        ] {
            assert!(output_name(name, "table").is_err(), "{name:?}");
        }
        assert!(output_name("data", "unknown").is_err());
    }
}

impl Broker {
    pub(super) fn run_python(
        &mut self,
        purpose: &str,
        code: &str,
        inputs: &[python::Input],
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        if purpose.trim().is_empty() || purpose.chars().count() > 1000 {
            return Err("Python 分析需提供簡短目的。".into());
        }
        let snapshot = python::snapshot(&self.project, inputs, cancel)?;
        let sources: Vec<Value> = snapshot.as_array().ok_or("Python 來源格式無效。")?.iter()
            .map(|v|json!({"name":v["name"],"path":v["path"],"kind":v["kind"],"revision":v["revision"],"scope":v["scope"],"start_line":v["start_line"],"line_count":v["line_count"],"next_line":v["next_line"],"eof":v["eof"]})).collect();
        let response = python::execute(code, snapshot, cancel)?;
        self.publish_python(purpose, code, sources, response, cancel)
    }

    /// 並行子程序只計算；驗證與檔案發布由主 broker 依序提交，避免共享狀態競爭。
    pub(super) fn publish_python(
        &mut self,
        purpose: &str,
        code: &str,
        sources: Vec<Value>,
        response: Value,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        let outputs = response["outputs"]
            .as_array()
            .ok_or("Python 成果清單無效。")?;
        if outputs.len() > 8 || self.python_artifacts.len() + outputs.len() > 30 {
            return Err("Python 成果超過每次 8 份／任務 30 份限制。".into());
        }
        let code_hash = format!("{:x}", Sha256::digest(code.as_bytes()));
        let progress_identity = semantic_progress::calculation_identity(&sources, &response);
        // 先完整驗證每份輸出，避免明顯無效的後續檔案造成半批發布。
        let mut tables = Vec::new();
        let mut books = Vec::new();
        for output in outputs {
            let kind = output["kind"].as_str().ok_or("Python 成果缺少種類。")?;
            let name = output_name(
                output["name"].as_str().ok_or("Python 成果缺少檔名。")?,
                kind,
            )?;
            match output["kind"].as_str() {
                Some("table") => {
                    let columns: Vec<String> = serde_json::from_value(output["columns"].clone())
                        .map_err(|_| "Python 表頭無效。")?;
                    let rows = output["rows"].as_array().ok_or("Python 資料列無效。")?;
                    if rows.len() > 100000 {
                        return Err("Python 成果表最多 100000 列。".into());
                    }
                    let mut table = super::super::datasets::Table::new(columns)?;
                    for (index, row) in rows.iter().enumerate() {
                        let cells = row.as_array().ok_or("Python 資料列不是陣列。")?;
                        let mut values = Vec::new();
                        let mut kinds = Vec::new();
                        for cell in cells {
                            let (text, kind) = match cell {
                                Value::Null => (String::new(), "blank"),
                                Value::String(s) => (s.clone(), "text"),
                                Value::Number(n) => (n.to_string(), "number"),
                                Value::Bool(b) => (b.to_string(), "boolean"),
                                _ => {
                                    return Err(
                                        "Python 儲存格只能是文字、有限數字、布林或空值。".into()
                                    )
                                }
                            };
                            values.push(text);
                            kinds.push(kind.into());
                        }
                        table.push(super::super::datasets::Row {
                            path: "Python衍生資料（原始來源見本次工具紀錄）".into(),
                            revision: code_hash.clone(),
                            sheet: 0,
                            row: index + 1,
                            texts: values.clone(),
                            values,
                            kinds,
                        })?;
                    }
                    table.csv()?;
                    tables.push((name.to_owned(), table));
                }
                Some("xlsx") => {
                    let bytes = python::unhex(output["hex"].as_str().ok_or("缺少 XLSX 資料。")?)?;
                    if !bytes.starts_with(b"PK\x03\x04") {
                        return Err("Python 未產生 XLSX 容器。".into());
                    }
                    books.push((name.to_owned(), bytes));
                }
                Some("png") => {
                    let bytes = python::unhex(output["hex"].as_str().ok_or("缺少PNG資料。")?)?;
                    python::validate_png(&bytes)?;
                    books.push((name.to_owned(), bytes));
                }
                _ => return Err("Python 成果格式只允許 CSV、XLSX與PNG。".into()),
            }
        }
        if self.datasets.len() + tables.len() > 30 {
            return Err("任務 CSV 數量超過上限。".into());
        }
        let mut artifacts = Vec::new();
        for (name, table) in tables {
            artifacts.push(self.save_dataset(&name, &table, cancel)?);
        }
        for (name, bytes) in books {
            artifacts.push(self.save_python_binary(&name, &bytes, cancel)?);
        }
        let mut result = json!({"purpose":purpose,"summary":response["summary"],"stdout":response["stdout"],"versions":response["versions"],
            "sources":sources,"code_sha256":code_hash,"artifacts":artifacts,"progress_identity":progress_identity,"execution":"本機 AppContainer，無網路，原檔唯讀；衍生資料非原始列的一對一副本。"});
        if let Some(guidance) = summary_guidance(&response) {
            result["guidance"] = json!(guidance);
        }
        Ok(result)
    }

    fn save_python_binary(
        &mut self,
        name: &str,
        bytes: &[u8],
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("Python 成果發布已取消。".into());
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
        let (actual_name, mut file) = reserve_output(&folder, name)?;
        let path = format!("_AI_Output/{folder_name}/{actual_name}");
        self.published.push(path.clone());
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| format!("Python 成果 {path} 可能部分寫入，未交付：{e}"))?;
        file.rewind().map_err(|e| e.to_string())?;
        let mut readback = Vec::new();
        Read::by_ref(&mut file)
            .take((python::MAX_OUTPUT + 1) as u64)
            .read_to_end(&mut readback)
            .map_err(|e| e.to_string())?;
        if readback != bytes {
            return Err("Python 成果寫入後讀回不一致。".into());
        }
        let sha256 = format!("{:x}", Sha256::digest(bytes));
        self.python_artifacts.push(python::Artifact {
            path: path.clone(),
            sha256: sha256.clone(),
        });
        let format = Path::new(name)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        Ok(json!({"path":path,"sha256":sha256,"bytes":bytes.len(),"verified":true,"format":format}))
    }

    pub(super) fn verify_python_artifact(&self, artifact: &python::Artifact) -> AppResult<()> {
        let rel = relative(&artifact.path)?;
        if rel.components().count() != 3
            || !rel.starts_with("_AI_Output")
            || rel
                .extension()
                .is_none_or(|s| !s.eq_ignore_ascii_case("xlsx") && !s.eq_ignore_ascii_case("png"))
        {
            return Err("Python 成果路徑不合法。".into());
        }
        let target = self.project.root.join(rel);
        let _guards = pin(target.parent().ok_or("缺少成果目錄。")?)?;
        let mut file = checked_file(&target)?;
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take((python::MAX_OUTPUT + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > python::MAX_OUTPUT
            || format!("{:x}", Sha256::digest(&bytes)) != artifact.sha256
        {
            return Err("Python 成果已變更，不能沿用舊版交付。".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_only_json_gets_guidance_without_promoting_stdout() {
        let response = json!({"summary":{},"stdout":"{\"n\":121}\n"});
        assert!(summary_guidance(&response).unwrap().contains("不需重跑"));
        assert_eq!(response["summary"], json!({}));
        for response in [
            json!({"summary":{"n":121},"stdout":"{\"n\":0}"}),
            json!({"summary":{},"stdout":"[121]"}),
            json!({"summary":{},"stdout":"debug\n{\"n\":121}"}),
            json!({"summary":{},"stdout":""}),
        ] {
            assert!(summary_guidance(&response).is_none());
        }
    }

    /// 只驗證成果 checkpoint 的路徑／版本契約；真正 XLSX 由 python_smoke 驗證。
    #[test]
    fn saved_python_artifacts_survive_resume_and_reject_changed_bytes() {
        let root = std::env::temp_dir().join(format!(
            "CompanyAI-python-state-{}",
            crate::jobs::new_id().unwrap()
        ));
        fs::create_dir(&root).unwrap();
        let project = Project {
            id: "python-state".into(),
            name: "合成驗收".into(),
            root: root.clone(),
            imports: BTreeMap::new(),
        };
        let cancel = AtomicBool::new(false);
        let mut broker = Broker::new(project.clone(), "test".into()).unwrap();
        let artifact = broker
            .save_python_binary("測試.xlsx", b"checkpoint-fixture", &cancel)
            .unwrap();
        let state = broker.saved().unwrap();
        let mut restored = Broker::new(project.clone(), "test".into()).unwrap();
        restored.restore(state, &cancel).unwrap();
        let path = artifact["path"].as_str().unwrap();
        assert_eq!(restored.finish(&[]).unwrap(), vec![path]);
        assert_eq!(restored.python_artifact_index()[0]["path"], path);
        let state = restored.saved().unwrap();
        fs::write(root.join(path), b"changed-fixture").unwrap();
        let mut stale = Broker::new(project, "test".into()).unwrap();
        assert!(stale.restore(state, &cancel).is_err());
        assert!(restored.finish(&[]).is_err());
        fs::remove_file(root.join(path)).unwrap();
        fs::remove_dir(root.join(path).parent().unwrap()).unwrap();
        fs::remove_dir(root.join("_AI_Output")).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
