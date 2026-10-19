//! 既有 DWG 一律先由受保護的來源 handle 複製，再交給 CAD 開啟；不把來源路徑送入 COM。
use super::*;
use sha2::{Digest, Sha256};

struct CadSource {
    _source: File,
    _source_dirs: Vec<File>,
    _output_dirs: Vec<File>,
    _stage: StageGuard,
    absolute: PathBuf,
    relative: String,
    revision: String,
}

impl Broker {
    fn copy_cad_source(&self, path: &str, name: &str) -> AppResult<CadSource> {
        let source_rel = relative(path)?;
        let name_rel = relative(name)?;
        if !source_rel
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("dwg"))
            || name_rel.components().count() != 1
            || !name_rel
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("dwg"))
        {
            return Err("請提供專案內 DWG 路徑及單一 .dwg 副本檔名。".into());
        }
        let source_path = self.project.root.join(&source_rel);
        let source_dirs = pin(source_path.parent().ok_or("來源目錄不存在。")?)?;
        let mut source = checked_file(&source_path)?;
        let size = source.metadata().map_err(|e| e.to_string())?.len();
        if size == 0 || size > 512 * 1024 * 1024 {
            return Err("初版 DWG 限 1 byte–512 MB；請先分割過大圖面。".into());
        }
        let base = self.project.root.join("_AI_Output");
        match fs::create_dir(&base) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e.to_string()),
        }
        let output_dirs = pin(&base)?;
        let folder = create_output_folder(&base)?;
        let output = base.join(&folder);
        let stage = pin_stage(&output)?;
        let absolute = output.join(name_rel);
        let mut destination = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&absolute)
            .map_err(|e| e.to_string())?;
        std::io::copy(&mut source, &mut destination).map_err(|e| e.to_string())?;
        destination.sync_all().map_err(|e| e.to_string())?;
        drop(destination);
        source.rewind().map_err(|e| e.to_string())?;
        let mut hash = Sha256::new();
        std::io::copy(&mut source, &mut hash).map_err(|e| e.to_string())?;
        Ok(CadSource {
            _stage: stage,
            _source: source,
            _source_dirs: source_dirs,
            _output_dirs: output_dirs,
            absolute,
            relative: format!("_AI_Output/{folder}/{name}"),
            revision: format!("{:x}", hash.finalize()),
        })
    }

    pub(super) fn read_cad(
        &mut self,
        path: &str,
        version: &str,
        offset: usize,
        revision: Option<&str>,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        if offset > 0 {
            let entry = self
                .cad_reads
                .get(path)
                .cloned()
                .ok_or("分頁前請先讀取第一頁。")?;
            if revision.is_none() || entry["revision"].as_str() != revision {
                return Err("分頁 revision 不符合已讀取來源。".into());
            }
            if version != "latest" && entry["version"] != version {
                return Err("分頁不可切換 CAD 版本。".into());
            }
            let source_path = self.project.root.join(relative(path)?);
            let _source_dirs = pin(source_path.parent().ok_or("缺少來源目錄。")?)?;
            let mut source = checked_file(&source_path)?;
            let mut hash = Sha256::new();
            std::io::copy(&mut source, &mut hash).map_err(|e| e.to_string())?;
            if entry["revision"] != format!("{:x}", hash.finalize()) {
                return Err("來源已變更，請重新讀取第一頁。".into());
            }
            let index_path = self
                .project
                .root
                .join(relative(entry["index_path"].as_str().ok_or("缺少索引。")?)?);
            let _index_dirs = pin(index_path.parent().ok_or("缺少索引目錄。")?)?;
            let file = checked_file(&index_path)?;
            let mut bytes = Vec::new();
            file.take(70 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() > 70 * 1024 * 1024
                || entry["index_hash"] != format!("{:x}", Sha256::digest(&bytes))
            {
                return Err("CAD 索引已變更，請重新讀取來源。".into());
            }
            let mut result: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            let rows = result["entities"].as_array().ok_or("CAD 索引格式不符。")?;
            let end = offset.saturating_add(100).min(rows.len());
            let page = rows
                .get(offset..end)
                .ok_or("分頁 offset 超出圖元數量。")?
                .to_vec();
            result["next_offset"] = if end < rows.len() {
                json!(end)
            } else {
                Value::Null
            };
            result["entities"] = json!(page);
            result["source"] = json!(path);
            result["revision"] = entry["revision"].clone();
            self.remember_cad_handles(path, &result)?;
            return Ok(result);
        }
        let copy = self.copy_cad_source(path, "read-copy.dwg")?;
        if (offset > 0 && revision.is_none()) || revision.is_some_and(|r| r != copy.revision) {
            return Err("來源版本已變更或缺少分頁 revision；請從 offset=0 重新讀取。".into());
        }
        let mut result = crate::cad::execute_interactive(
            json!({"action":"read","version":version,
            "output":copy.absolute,"source_revision":copy.revision,"offset":offset,"limit":100}),
            cancel,
            self.cad_waiter.as_mut(),
            self.cad_progress.as_mut(),
        )?;
        result["source"] = json!(path);
        result["revision"] = json!(copy.revision);
        result["intended_path"] = json!(copy.relative);
        crate::storage::atomic_write(
            &copy.absolute.with_file_name("cad-report.json"),
            &serde_json::to_vec_pretty(&result).map_err(|e| e.to_string())?,
        )?;
        if result["status"] == "READ_COPY" {
            let index_path = copy.absolute.with_file_name("cad-index.json");
            let mut index = checked_file(&index_path)?;
            let mut hash = Sha256::new();
            std::io::copy(&mut index, &mut hash).map_err(|e| e.to_string())?;
            let entry = self
                .cad_reads
                .entry(path.into())
                .or_insert_with(|| json!({}));
            if entry["revision"] != copy.revision {
                *entry = json!({"revision":copy.revision,"handles":[]});
            }
            entry["index_path"] = json!(Path::new(&copy.relative)
                .with_file_name("cad-index.json")
                .to_string_lossy());
            entry["index_hash"] = json!(format!("{:x}", hash.finalize()));
            entry["units_code"] = result["units_code"].clone();
            entry["version"] = result["selected"]["progid"].clone();
            self.remember_cad_handles(path, &result)?;
            result["instructions"] = json!("僅讀取工作副本的 ModelSpace。來源未修改；不是視覺辨識。修改須使用本 revision 與已讀取 Handle。圖塊僅摘要，讀取副本不是驗證完成的修改成果。");
        }
        Ok(result)
    }

    pub(super) fn remember_cad_handles(&mut self, path: &str, page: &Value) -> AppResult<()> {
        let entry = self.cad_reads.get_mut(path).ok_or("缺少 CAD 讀取記錄。")?;
        let mut handles = entry["handles"].as_array().cloned().unwrap_or_default();
        for row in page["entities"].as_array().into_iter().flatten() {
            if row["editable"] == true && !handles.contains(&row["handle"]) {
                handles.push(row["handle"].clone());
            }
        }
        if handles.len() > 10000 {
            return Err("同來源已讀取可修改圖元超過一萬項，請分割圖面。".into());
        }
        entry["handles"] = json!(handles);
        Ok(())
    }

    pub(super) fn edit_cad(&mut self, tool: &Tool, cancel: &AtomicBool) -> AppResult<Value> {
        let Tool::CadEdit {
            path,
            revision,
            name,
            edits,
            operations,
        } = tool
        else {
            return Err("不是 CAD 修改要求。".into());
        };
        if self.cad_failed || self.cad_attempts >= 6 {
            return Err("CAD 結果未確認或已達六次執行上限。".into());
        }
        let plan = format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&json!({
                    "path":path,"revision":revision,"edits":edits,"operations":operations
                }))
                .map_err(|e| e.to_string())?
            )
        );
        if self.cad_partial_plans.contains(&plan) {
            return Err("此修改計畫曾部分執行；請修正計畫再建立新副本，不可僅換檔名重播。".into());
        }
        let read = self
            .cad_reads
            .get(path)
            .ok_or("修改前請先 cad_read 讀取來源與目標圖元。")?;
        if read["revision"] != *revision {
            return Err("請使用最近 cad_read 回傳的 revision。".into());
        }
        let handles = read["handles"].as_array().ok_or("尚未讀取目標 Handle。")?;
        if edits.len() > 100 || operations.len() > 100 || !cad::edits_are_read(edits, handles) {
            return Err("只能修改已讀取的可修改 Handle，每次最多 100 項修改與 100 項新增。".into());
        }
        let version = read["version"].clone();
        let copy = self.copy_cad_source(path, name)?;
        if copy.revision != *revision {
            return Err("來源 DWG 已變更；未操作 CAD，請重新讀取。".into());
        }
        // 修改失敗保留工作副本供人工查看，不覆寫來源，也不重播半成品。
        self.cad_failed = true;
        self.cad_attempts += 1;
        let mut result = crate::cad::execute_interactive(
            json!({"action":"edit","version":version,"output":copy.absolute,"source_revision":copy.revision,
            "edits":edits,"operations":operations}),
            cancel,
            self.cad_waiter.as_mut(),
            self.cad_progress.as_mut(),
        )?;
        result["source"] = json!(path);
        result["source_revision"] = json!(revision);
        result["intended_path"] = json!(copy.relative);
        if result["drawing_started"] == false || result["step"] == "validate_edit" {
            self.cad_failed = false;
            self.cad_attempts -= 1;
        }
        if result["recovery"] == "user_requested_new_document" {
            self.cad_failed = false;
        }
        if result["recovery"] == "revised_new_document" && super::cad::recoverable(&result) {
            self.cad_failed = false;
            self.cad_partial_plans.push(plan);
        }
        if result["status"] == "DATA_VERIFIED_VISUAL_PENDING" {
            let mut file = checked_file(&copy.absolute)?;
            let mut hash = Sha256::new();
            std::io::copy(&mut file, &mut hash).map_err(|e| e.to_string())?;
            let sha256 = format!("{:x}", hash.finalize());
            result["path"] = json!(copy.relative);
            result["sha256"] = json!(sha256);
            self.cad_artifacts.push(super::super::python::Artifact {
                path: copy.relative.clone(),
                sha256,
            });
            self.published.push(copy.relative.clone());
            self.cad_failed = false;
        }
        crate::storage::atomic_write(
            &copy.absolute.with_file_name("cad-report.json"),
            &serde_json::to_vec_pretty(&result).map_err(|e| e.to_string())?,
        )?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cad_source_copy_is_private_and_source_is_locked() {
        let root =
            std::env::temp_dir().join(format!("lmai-cad-copy-{}", crate::jobs::new_id().unwrap()));
        fs::create_dir(&root).unwrap();
        let source = root.join("source.dwg");
        fs::write(&source, b"synthetic CAD source").unwrap();
        let project = Project {
            id: "copy".into(),
            name: "CAD".into(),
            root: root.clone(),
            imports: Default::default(),
        };
        let broker = Broker::new(project, "task".into()).unwrap();
        let copy = broker
            .copy_cad_source("source.dwg", "modified.dwg")
            .unwrap();
        assert_ne!(copy.absolute, source);
        assert_eq!(fs::read(&copy.absolute).unwrap(), b"synthetic CAD source");
        assert!(OpenOptions::new().write(true).open(&source).is_err());
        fs::write(&copy.absolute, b"modified copy").unwrap();
        assert_eq!(fs::read(&source).unwrap(), b"synthetic CAD source");
        assert!(broker.copy_cad_source("../source.dwg", "bad.dwg").is_err());
        assert!(broker.copy_cad_source("source.dwg", "../bad.dwg").is_err());
    }
}
