//! Python 成果發布與恢復檢查；所有寫入仍由主程序保留既有專案路徑界線。
use super::*;
use crate::projects::python;
use sha2::{Digest, Sha256};

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
            .map(|v|json!({"name":v["name"],"path":v["path"],"kind":v["kind"],"revision":v["revision"]})).collect();
        let response = python::execute(code, snapshot, cancel)?;
        let outputs = response["outputs"]
            .as_array()
            .ok_or("Python 成果清單無效。")?;
        if outputs.len() > 8 || self.python_artifacts.len() + outputs.len() > 30 {
            return Err("Python 成果超過每次 8 份／任務 30 份限制。".into());
        }
        let code_hash = format!("{:x}", Sha256::digest(code.as_bytes()));
        // 先完整驗證每份輸出，避免明顯無效的後續檔案造成半批發布。
        let mut tables = Vec::new();
        let mut books = Vec::new();
        for output in outputs {
            let name = output["name"].as_str().ok_or("Python 成果缺少檔名。")?;
            let path = relative(name)?;
            if path.components().count() != 1 {
                return Err("Python 成果只接受單一檔名。".into());
            }
            match output["kind"].as_str() {
                Some("table")
                    if path
                        .extension()
                        .is_some_and(|s| s.eq_ignore_ascii_case("csv")) =>
                {
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
                Some("xlsx")
                    if path
                        .extension()
                        .is_some_and(|s| s.eq_ignore_ascii_case("xlsx")) =>
                {
                    let bytes = python::unhex(output["hex"].as_str().ok_or("缺少 XLSX 資料。")?)?;
                    if !bytes.starts_with(b"PK\x03\x04") {
                        return Err("Python 未產生 XLSX 容器。".into());
                    }
                    books.push((name.to_owned(), bytes));
                }
                _ => return Err("Python 成果格式只允許 CSV 與 XLSX。".into()),
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
            artifacts.push(self.save_python_book(&name, &bytes, cancel)?);
        }
        Ok(
            json!({"purpose":purpose,"summary":response["summary"],"stdout":response["stdout"],"versions":response["versions"],
            "sources":sources,"code_sha256":code_hash,"artifacts":artifacts,"execution":"本機 AppContainer，無網路，原檔唯讀；衍生資料非原始列的一對一副本。"}),
        )
    }

    fn save_python_book(
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
        if self.output_folder.is_none() {
            self.output_folder = Some(create_output_folder(&base)?);
        }
        let folder_name = self.output_folder.as_deref().ok_or("缺少成果資料夾。")?;
        let folder = base.join(folder_name);
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
            return Err("Python XLSX 寫入後讀回不一致。".into());
        }
        let sha256 = format!("{:x}", Sha256::digest(bytes));
        self.python_artifacts.push(python::Artifact {
            path: path.clone(),
            sha256: sha256.clone(),
        });
        Ok(json!({"path":path,"sha256":sha256,"bytes":bytes.len(),"verified":true,"format":"xlsx"}))
    }

    pub(super) fn verify_python_artifact(&self, artifact: &python::Artifact) -> AppResult<()> {
        let rel = relative(&artifact.path)?;
        if rel.components().count() != 3
            || !rel.starts_with("_AI_Output")
            || rel
                .extension()
                .is_none_or(|s| !s.eq_ignore_ascii_case("xlsx"))
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
            .save_python_book("測試.xlsx", b"checkpoint-fixture", &cancel)
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
