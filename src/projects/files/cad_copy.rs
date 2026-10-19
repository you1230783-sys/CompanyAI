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
    report: PathBuf,
    report_file: Option<File>,
    revision: String,
}

/// 報告使用本次以 create_new 保留的獨占 handle 寫入，不替換既有路徑。
/// 來源與成果共用資料夾時，來源的唯讀目錄鎖會禁止 Windows rename；
/// 若改用 atomic_write，原生 CAD 的真正錯誤反而會被報告儲存失敗蓋掉。
/// 保留檔在寫完前不分享存取，讀者不會讀到半份 JSON，也不會覆寫舊報告。
fn write_reserved_cad_report(mut file: File, result: &Value) -> AppResult<()> {
    let bytes = serde_json::to_vec_pretty(result).map_err(|e| e.to_string())?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| format!("無法寫入本次 CAD 報告：{e}"))
}

impl CadSource {
    fn save_report(&mut self, result: &Value) -> AppResult<()> {
        write_reserved_cad_report(self.report_file.take().ok_or("CAD 報告已寫入。")?, result)
    }
}

/// 來源與成果為同一目錄時，用已建立的 StageGuard 接手最末層目錄保護。
/// 一般唯讀 pin 不分享目錄寫入，會阻擋 CAD 將 .dwg.$$$ 改名完成存檔。
/// 以 Windows 檔案識別核對目錄，避免大小寫／別名比較造成誤判；只移除
/// 重複的末層 pin，祖先、原檔唯讀 handle、不可刪除的目錄 handle 與 anchor 均保留。
fn coordinate_cad_directory_pins(source_dirs: &mut Vec<File>, stage: &StageGuard) -> AppResult<()> {
    let source = source_dirs.last().ok_or("缺少 CAD 來源目錄保護。")?;
    let output = stage.directory.as_ref().ok_or("缺少 CAD 成果目錄保護。")?;
    let identity = |file: &File| -> AppResult<(u32, u32, u32)> {
        let mut info = BY_HANDLE_FILE_INFORMATION::default();
        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0
            || info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0
            || (info.nFileIndexHigh == 0 && info.nFileIndexLow == 0)
        {
            return Err("無法核對 CAD 來源與成果目錄識別，尚未修改圖面。".into());
        }
        Ok((
            info.dwVolumeSerialNumber,
            info.nFileIndexHigh,
            info.nFileIndexLow,
        ))
    };
    if identity(source)? == identity(output)? {
        // StageGuard 已先取得不分享 DELETE 的 handle 與獨占 anchor，沒有保護空窗。
        source_dirs.pop();
    }
    Ok(())
}

impl Broker {
    fn copy_cad_source(
        &mut self,
        path: &str,
        name: &str,
        deliverable: bool,
    ) -> AppResult<CadSource> {
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
        let mut source_dirs = pin(source_path.parent().ok_or("來源目錄不存在。")?)?;
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
        let shared = shared_output_folder(
            &base,
            &mut self.output_folder,
            self.output_conversation.as_deref(),
        )?;
        // 唯讀副本的原生索引有固定檔名，放在對話資料夾內的獨立暫存目錄；
        // 正式修改成果直接放在共用資料夾，避免多次CAD讀取覆寫舊索引。
        let folder = if deliverable {
            shared
        } else {
            format!("{shared}/.cad-read-{}", crate::jobs::new_id()?)
        };
        let output = base.join(&folder);
        if !deliverable {
            fs::create_dir(&output).map_err(|e| e.to_string())?;
        }
        let stage = pin_output_directory(&output)?;
        coordinate_cad_directory_pins(&mut source_dirs, &stage)?;
        let (name, mut destination) = reserve_output(&output, name)?;
        let absolute = output.join(&name);
        std::io::copy(&mut source, &mut destination).map_err(|e| e.to_string())?;
        destination.sync_all().map_err(|e| e.to_string())?;
        drop(destination);
        source.rewind().map_err(|e| e.to_string())?;
        let mut hash = Sha256::new();
        std::io::copy(&mut source, &mut hash).map_err(|e| e.to_string())?;
        let (report_name, report_placeholder) = reserve_output(&output, "cad-report.json")?;
        Ok(CadSource {
            _stage: stage,
            _source: source,
            _source_dirs: source_dirs,
            _output_dirs: output_dirs,
            absolute,
            relative: format!("_AI_Output/{folder}/{name}"),
            report: output.join(report_name),
            report_file: Some(report_placeholder),
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
        let bound_version = self.resolve_copy_cad_version(version, cancel)?;
        let version = bound_version.as_str();
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
        let mut copy = self.copy_cad_source(path, "read-copy.dwg", false)?;
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
        copy.save_report(&result)?;
        if result["status"] == "READ_COPY" {
            self.cad_copy_version = Some(
                result["selected"]["progid"]
                    .as_str()
                    .ok_or("成功讀取未回報確切CAD ProgID。")?
                    .to_owned(),
            );
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
        let read = self.cad_reads.get(path).ok_or(
            "此來源尚無讀取紀錄；先cad_snapshot(path=此DWG)，再cad_query(details=true)取得目標。",
        )?;
        if read["revision"] != *revision {
            return Err("請使用最近 cad_read 回傳的 revision。".into());
        }
        let handles = read["handles"].as_array().ok_or("尚未讀取目標 Handle。")?;
        let snapshot = self
            .cad_snapshot
            .as_ref()
            .filter(|s| s.data["source"] == *path && s.data["revision"] == *revision);
        if let Some(rejection) = cad_preflight::check(edits, operations, handles, snapshot) {
            return Ok(rejection);
        }
        let version = read["version"].clone();
        let mut copy = self.copy_cad_source(path, name, true)?;
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
        copy.save_report(&result)?;
        self.cad_last_result = Some(json!({"status":result["status"],"step":result["step"],
            "error":result["error"].as_str().map(|s|s.chars().take(4096).collect::<String>()),
            "source":path,"source_revision":revision,"intended_path":copy.relative,
            "path":result["path"],"saved":result["saved"],"drawing_started":result["drawing_started"],
            "recovery":result["recovery"],"before_save":result["before_save"],"after_reopen":result["after_reopen"],
            "modified_handles":result["modified_handles"],"added_handles":result["added_handles"],
            "report_path":copy.report.strip_prefix(&self.project.root).ok().map(|p|p.to_string_lossy()),
            "verification_scope":"原生核對實際提交的修改與未指定圖元，不代表使用者規格或外觀已全部通過。",
            "next_action":"若要求的修改都已完成且原生存檔重開成功，直接finish交付；仍有後續修改才對新path建立一次快照並讀取必要Handle。需要獨立量測時一次讀所需目標、保存結論，之後不重複收尾查核。"}));
        Ok(result)
    }
}

#[cfg(test)]
mod report_tests {
    use super::*;

    #[test]
    fn shared_cad_directory_allows_save_rename_without_unlocking_source() {
        let root = std::env::current_dir()
            .unwrap()
            .join(".build")
            .join(format!("cad-save-{}", crate::jobs::new_id().unwrap()));
        fs::create_dir_all(&root).unwrap();
        let source_path = root.join("source.dwg");
        fs::write(&source_path, b"original").unwrap();
        let mut pins = pin(&root).unwrap();
        let source = checked_file(&source_path).unwrap();
        let stage = pin_output_directory(&root).unwrap();
        let old_count = pins.len();
        coordinate_cad_directory_pins(&mut pins, &stage).unwrap();
        assert_eq!(pins.len(), old_count - 1);
        let output = root.join("new.dwg");
        let temporary = root.join("new.dwg.$$$");
        fs::write(&output, b"copied").unwrap();
        fs::write(&temporary, b"saved").unwrap();
        fs::rename(&temporary, &output).unwrap();
        assert_eq!(fs::read(&output).unwrap(), b"saved");
        assert!(OpenOptions::new().write(true).open(&source_path).is_err());
        assert!(fs::rename(&source_path, root.join("source-moved.dwg")).is_err());
        assert!(fs::rename(&root, root.with_extension("moved")).is_err());
        assert_eq!(fs::read(&source_path).unwrap(), b"original");

        // 不同來源目錄仍保留全部嚴格 pin；不能為了共用輸出而全面放寬目錄。
        let other = root.join("other");
        fs::create_dir(&other).unwrap();
        let mut other_pins = pin(&other).unwrap();
        let other_count = other_pins.len();
        coordinate_cad_directory_pins(&mut other_pins, &stage).unwrap();
        assert_eq!(other_pins.len(), other_count);
        drop(other_pins);
        drop(stage);
        drop(source);
        drop(pins);
        fs::remove_dir(other).unwrap();
        fs::remove_file(output).unwrap();
        fs::remove_file(source_path).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn reserved_report_preserves_native_failure_under_source_directory_pin() {
        let root = std::env::current_dir()
            .unwrap()
            .join(".build")
            .join(format!("cad-report-{}", crate::jobs::new_id().unwrap()));
        fs::create_dir_all(&root).unwrap();
        let pins = pin(&root).unwrap();
        let stage = pin_output_directory(&root).unwrap();
        let (name, file) = reserve_output(&root, "cad-report.json").unwrap();
        assert!(
            checked_file(&root.join(&name)).is_err(),
            "保留報告寫完前不可被其他讀者開啟"
        );
        let result = json!({"status":"FAILED","step":"validate_edit",
            "error":"選取點無法確定要處理哪一端","completed_operations":0});
        write_reserved_cad_report(file, &result).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(root.join(&name)).unwrap()).unwrap(),
            result
        );
        let (second, file) = reserve_output(&root, "cad-report.json").unwrap();
        assert_ne!(name, second);
        write_reserved_cad_report(file, &json!({"status":"DATA_VERIFIED_VISUAL_PENDING"})).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(root.join(&name)).unwrap()).unwrap(),
            result
        );
        drop(stage);
        drop(pins);
        fs::remove_file(root.join(name)).unwrap();
        fs::remove_file(root.join(second)).unwrap();
        fs::remove_dir(root).unwrap();
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
        let mut broker = Broker::new(project, "task".into()).unwrap();
        let copy = broker
            .copy_cad_source("source.dwg", "modified.dwg", true)
            .unwrap();
        assert_ne!(copy.absolute, source);
        assert_eq!(fs::read(&copy.absolute).unwrap(), b"synthetic CAD source");
        assert!(OpenOptions::new().write(true).open(&source).is_err());
        fs::write(&copy.absolute, b"modified copy").unwrap();
        assert_eq!(fs::read(&source).unwrap(), b"synthetic CAD source");
        assert!(broker
            .copy_cad_source("../source.dwg", "bad.dwg", true)
            .is_err());
        assert!(broker
            .copy_cad_source("source.dwg", "../bad.dwg", true)
            .is_err());
    }
}
