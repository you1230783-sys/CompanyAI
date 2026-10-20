//! 快照、候選查詢及局部預覽的 Broker 邊界；完整圖面留在本機，按需交付詳細資料。
use super::*;
use crate::projects::{
    cad_index::{Filter, Snapshot},
    vision,
};
use sha2::{Digest, Sha256};

/// 年份必須來自本機安裝清單；不從 ProgID 後綴猜年份，也不跨版本回退。
/// 已綁定的任務即使收到年份別名，也只能解析回同一套實際執行器。
fn resolve_copy_year(requested: &str, bound: Option<&str>, inventory: &Value) -> AppResult<String> {
    let selection = super::cad_plan::select_version(requested, inventory)?;
    let resolved = selection["progid"]
        .as_str()
        .ok_or("CAD 安裝資料缺少 ProgID。")?;
    if let Some(bound) = bound {
        if !bound.eq_ignore_ascii_case(resolved) {
            return Err(format!("本任務DWG副本已使用確切CAD版本 {bound}；年份 {requested} 對應 {resolved}，不一致，尚未啟動CAD。請沿用 {bound} 或 latest；切換版本需另開任務。"));
        }
        return Ok(bound.to_owned());
    }
    Ok(resolved.to_owned())
}

impl Broker {
    /// 副本任務也沿用首次成功版本；latest不會在每個新輸出路徑重新選CAD。
    /// 明確不同版本不偷偷替換，先回報目前的確切ProgID，尚未啟動CAD。
    pub(super) fn copy_cad_version(&self, requested: &str) -> AppResult<String> {
        if matches!(
            requested.to_ascii_lowercase().as_str(),
            "latest" | "auto" | "最新版"
        ) {
            return Ok(self
                .cad_copy_version
                .as_deref()
                .unwrap_or("latest")
                .to_owned());
        }
        let Some(selected) = self.cad_copy_version.as_deref() else {
            return Ok(requested.to_owned());
        };
        if requested.eq_ignore_ascii_case(selected) {
            Ok(selected.to_owned())
        } else {
            Err(format!("本任務DWG副本已使用確切CAD版本 {selected}；本次傳入 {requested} 不一致，尚未啟動CAD。沿用本任務請填 {selected} 或 latest；需要切換CAD版本請另開任務。"))
        }
    }
    /// 副本讀取接受與分段新圖一致的年份別名；在建立工作副本前解析。
    /// 明確 ProgID 不多查 inventory；自動別名仍優先沿用本任務首次成功版本。
    pub(super) fn resolve_copy_cad_version(
        &self,
        requested: &str,
        cancel: &AtomicBool,
    ) -> AppResult<String> {
        if requested
            .parse::<u16>()
            .ok()
            .is_some_and(|year| (1990..=2099).contains(&year))
        {
            let inventory = crate::cad::execute(json!({"action":"inventory"}), cancel)?;
            resolve_copy_year(requested, self.cad_copy_version.as_deref(), &inventory)
        } else {
            self.copy_cad_version(requested)
        }
    }

    /// 生成的CAD預覽必須仍屬於有效快照且檔案未被替換；一般使用者圖片沿用原管線。
    pub(crate) fn guard_cad_preview_image(
        &mut self,
        path: &str,
        sha256: &str,
        cancel: &AtomicBool,
    ) -> AppResult<()> {
        let key = relative(path)?
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase();
        if let Some(record) = self.cad_previews.get(&key).cloned() {
            if record["sha256"] != sha256 {
                return Err("CAD預覽檔已變更，請重新產生。".into());
            }
            self.guard_cad_snapshot(
                record["snapshot_id"].as_str().ok_or("CAD預覽缺少快照。")?,
                cancel,
            )?;
        }
        Ok(())
    }
    pub(in crate::projects) fn cad_snapshot_index(&self) -> Value {
        let session = self.cad_session.context();
        if self.cad_snapshot.is_none()
            && session["binding"].is_null()
            && session["last_edit"].is_null()
            && self.loaded_skills.iter().any(|id| id == "cad-new")
        {
            return json!({"available":false,"mode":"new_drawing",
                "notice":"本任務為新建設計，不需要目前圖面快照。依cad_artifacts.last_execution及原要求繼續；已停止時finish回報，不為缺少快照改讀舊圖。"});
        }
        let missing = if self.loaded_skills.iter().any(|id| id.starts_with("cad-"))
            || !session["binding"].is_null()
            || !session["last_edit"].is_null()
        {
            json!({"available":false,"notice":"本次執行沒有有效CAD快照；先cad_snapshot取得總覽，再按需查詢。舊工具紀錄是歷史資料，不能恢復目前圖面的修改授權。"})
        } else {
            Value::Null
        };
        let mut context = self.cad_snapshot.as_ref().map_or(missing, |s| {
            let delivery = s.delivered_context();
            json!({
            "snapshot_id":s.id,"revision":s.data["revision"],"document":s.data["document"],"source":s.data["source"],
            "units_code":s.data["units_code"],"edit_capabilities":s.data["edit_capabilities"],"entity_count":s.rows.len(),
            "recent_details":delivery["facts"],"detail_delivery":delivery["coverage"],
            "omitted_read_entities":delivery["omitted_read_entities"],"detail_properties_scope":delivery["properties_scope"],
            "notice":"recent_details是本版本已交付的精確資料投影，含完整保留的基本幾何頂點，可沿用，不因歷史移出重查。detail_delivery記已讀範圍，不等於需求通過；已核對完成即finish。修改前原生端仍核對全圖。"})
        });
        if !context.is_null() {
            context["resolved_copy_version"] = json!(self.cad_copy_version);
            context["edit_batch_rules"] = json!({"details_required_for_primary_and_secondary_handles":true,
                "same_edit_handle_once_per_batch":true,"geometry_operations_must_be_empty":true,
                "instruction":"先cad_query(details=true)取得所有目標；幾何修改不混改字／半徑／指令。同一Handle的offset、mirror、array須分批。新輸出路徑先建快照再讀必要目標，沿用resolved_copy_version。"});
            if let Some(last) = self
                .cad_last_result
                .as_ref()
                .filter(|r| r["status"] == "DATA_VERIFIED_VISUAL_PENDING" && r["path"].is_string())
            {
                context["latest_saved_output"] = json!({"path":last["path"],
                    "snapshot_is_latest_output":context["source"]==last["path"],
                    "notice":"原生已依實際提交值完成存檔重開核對；若需求已滿足，finish交付。仍有下一步修改或必要獨立量測，才對此新路徑建立一次快照。來源舊快照不能當成新成果的驗證。"});
            }
            context["cad_session"] = session;
            if self.cad_session.stopped() {
                context["notice"] = json!(
                    "本次操作已停止，依cad_session.last_edit回報。缺少快照不代表需要重讀或換CAD。"
                );
            } else if self.cad_snapshot.is_none()
                && context["cad_session"]["last_edit"]["status"] == "EDITED_UNSAVED"
            {
                context["notice"] = json!("修改已由原生端完整核對；舊快照撤銷是正常結果，可直接finish，不必為恢復快照重讀CAD。若使用者需要額外量測才刷新同一文件。");
            }
        }
        context
    }

    /// 去重結果可以保留歷史，但失效快照不能再冒充可使用的候選。
    pub(super) fn current_cad_result(&self, tool: &Tool, result: Value) -> Value {
        let result = cad_preflight::normalize_outcome(tool, result);
        if matches!(
            tool,
            Tool::CadSnapshot { .. }
                | Tool::CadQuery { .. }
                | Tool::CadPreview { .. }
                | Tool::CadActiveRead { .. }
        ) && result["ok"] == true
            && result["result"]["snapshot_id"].is_string()
            && !self
                .cad_snapshot
                .as_ref()
                .is_some_and(|s| result["result"]["snapshot_id"] == s.id)
        {
            return json!({"ok":false,"error":"此操作的歷史快照已失效；使用新操作ID重新cad_snapshot，再查詢目標，不重播舊修改。","retry_same_operation":false});
        }
        result
    }

    /// 輕量核對不是 revision 驗證。原生修改入口仍全圖掃描，禁止憑快取放行寫入。
    pub(super) fn guard_cad_snapshot(&mut self, id: &str, cancel: &AtomicBool) -> AppResult<()> {
        let snapshot = self
            .cad_snapshot
            .as_ref()
            .filter(|s| s.id == id)
            .ok_or("快照已失效或任務已恢復；請cad_snapshot重新取得，不能沿用舊候選編號。")?;
        let data = &snapshot.data;
        let valid = if let Some(path) = data["source"].as_str() {
            let absolute = self.project.root.join(relative(path)?);
            let _dirs = pin(absolute.parent().ok_or("來源目錄不存在。")?)?;
            let mut file = checked_file(&absolute)?;
            let mut hash = Sha256::new();
            std::io::copy(&mut file, &mut hash).map_err(|e| e.to_string())?;
            data["revision"] == format!("{:x}", hash.finalize())
        } else {
            let probe = crate::cad::execute(
                json!({"action":"active_probe","version":data["selected"]["progid"]}),
                cancel,
            )?;
            probe["status"] == "ACTIVE_PROBE"
                && probe["document"] == data["document"]
                && probe["entity_count"] == data["reported_count"]
                && probe["units_code"] == data["units_code"]
        };
        if !valid {
            self.cad_snapshot = None;
            self.cad_active = None;
            return Err(
                "CAD文件、數量或來源已變更，舊候選作廢；請明確refresh=true重新建立快照。".into(),
            );
        }
        Ok(())
    }

    pub(super) fn snapshot_cad(
        &mut self,
        version: &str,
        path: Option<&str>,
        refresh: bool,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        // 固定的是首次成功讀取的實際版本；即使舊快照已作廢也不退回 latest。
        let bound_version;
        let version = if path.is_none() {
            bound_version = self.cad_session.version(version)?;
            bound_version.as_str()
        } else {
            if self.cad_session.stopped() {
                return Err("目前圖面操作已停止，不能改走副本流程；請回報原生失敗。".into());
            }
            bound_version = self.resolve_copy_cad_version(version, cancel)?;
            bound_version.as_str()
        };
        if !refresh {
            if let Some(s) = self.cad_snapshot.as_ref() {
                if s.data["source"].as_str() == path
                    && (version == "latest" || s.data["selected"]["progid"] == version)
                {
                    let id = s.id.clone();
                    self.guard_cad_snapshot(&id, cancel)?;
                    let mut result = self.cad_snapshot.as_ref().ok_or("快照已失效。")?.overview();
                    result["reused"] = json!(true);
                    return Ok(result);
                }
            }
        }
        // 重建前先撤銷候選；即使此次讀取失敗，也不能退回舊圖面快取。
        self.cad_snapshot = None;
        self.cad_active = None;
        let data = if let Some(path) = path {
            let page = self.read_cad(path, version, 0, None, cancel)?;
            if page["status"] != "READ_COPY" {
                return Err(format!("CAD副本讀取失敗：{page}"));
            }
            let entry = self.cad_reads.get_mut(path).ok_or("缺少副本索引。")?;
            // read_cad 的相容首頁未送給模型；不能把它當作這次已讀詳細資料。
            entry["handles"] = json!([]);
            let absolute = self.project.root.join(relative(
                entry["index_path"].as_str().ok_or("缺少索引路徑。")?,
            )?);
            let _dirs = pin(absolute.parent().ok_or("索引目錄不存在。")?)?;
            let mut bytes = Vec::new();
            checked_file(&absolute)?
                .take(70_000_001)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() > 70_000_000
                || entry["index_hash"] != format!("{:x}", Sha256::digest(&bytes))
            {
                return Err("副本索引完整性不符。".into());
            }
            let mut data: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            data["source"] = json!(path);
            data["revision"] = entry["revision"].clone();
            data["selected"] = page["selected"].clone();
            data
        } else {
            if self.cad_active_unsupported.as_deref() == Some(version) {
                return Err(
                    "此CAD目前圖面介面不支援，本任務不再重試；請保留診斷或使用副本。".into(),
                );
            }
            let data = crate::cad::execute_interactive(
                json!({"action":"active_snapshot","version":version}),
                cancel,
                self.cad_waiter.as_mut(),
                self.cad_progress.as_mut(),
            )?;
            if data["status"] != "READ_ACTIVE" {
                if data["recovery"] == "unsupported_capability" {
                    self.cad_active_unsupported = Some(version.to_owned());
                }
                return Err(format!("建立目前圖面快照失敗：{data}"));
            }
            data
        };
        let snapshot = Snapshot::new(data)?;
        // 只有完整索引通過檢查後才建立可用的文件狀態。
        if snapshot.data["source"].is_null() {
            self.cad_session.bind(
                snapshot.data["selected"]["progid"]
                    .as_str()
                    .ok_or("缺少實際CAD版本。")?,
                &snapshot.data["document"],
            )?;
            self.cad_active = Some(
                json!({"document":snapshot.data["document"],"revision":snapshot.data["revision"],
                "version":snapshot.data["selected"]["progid"],"units_code":snapshot.data["units_code"],"handles":[]}),
            );
        }
        let result = snapshot.overview();
        self.cad_snapshot = Some(snapshot);
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn query_cad(
        &mut self,
        id: &str,
        filter: &Filter,
        offset: usize,
        limit: usize,
        details: bool,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        self.guard_cad_snapshot(id, cancel)?;
        let snapshot = self.cad_snapshot.as_mut().ok_or("缺少快照。")?;
        let mut result = snapshot.query(filter, offset, limit, details)?;
        // 只有實際交付詳細資料才授予已讀 Handle；索引、候選或縮圖不等於已核對。
        if details {
            let previous = snapshot.delivered_count();
            snapshot.remember_details(&result);
            result["new_details_delivered"] = json!(snapshot.delivered_count() - previous);
            result["detail_delivery"] = snapshot.delivery_status();
            let rows: Vec<_> = result["candidates"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|v| v["entity"].clone())
                .collect();
            if let Some(path) = snapshot.data["source"].as_str().map(str::to_owned) {
                self.remember_cad_handles(&path, &json!({"entities":rows}))?;
            } else {
                let state = self.cad_active.as_mut().ok_or("目前圖面授權已失效。")?;
                let handles = state["handles"].as_array_mut().ok_or("缺少Handle記錄。")?;
                for row in rows {
                    if row["editable"] == true && !handles.contains(&row["handle"]) {
                        handles.push(row["handle"].clone());
                    }
                }
            }
        }
        Ok(result)
    }

    pub(super) fn preview_cad(
        &mut self,
        id: &str,
        candidates: &[usize],
        region: Option<[f64; 4]>,
        show_user: bool,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        if !show_user && !vision::input::model_supported(&self.model) {
            return Err("快速模型只提供使用者局部預覽，不能要求模型圖片辨識用途。".into());
        }
        self.guard_cad_snapshot(id, cancel)?;
        let snapshot = self.cad_snapshot.as_ref().ok_or("缺少快照。")?;
        let (bmp, metadata) =
            super::super::cad_index::preview::render(snapshot, candidates, region)?;
        let jpeg = vision::jpeg::convert(&bmp, "bmp")?;
        let query = snapshot.query(
            &Filter {
                candidates: Some(candidates.to_vec()),
                ..Filter::default()
            },
            0,
            20,
            false,
        )?;
        let revision = snapshot.data["revision"].clone();
        let _root = pin(&self.project.root)?;
        let base = self.project.root.join("_AI_Output");
        match fs::create_dir(&base) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e.to_string()),
        }
        let _base = pin(&base)?;
        let folder = shared_output_folder(
            &base,
            &mut self.output_folder,
            self.output_conversation.as_deref(),
        )?;
        let output = base.join(&folder);
        let _stage = pin_output_directory(&output)?;
        let (name, mut file) = reserve_output(&output, "cad-preview.jpg")?;
        file.write_all(&jpeg.bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        file.rewind().map_err(|e| e.to_string())?;
        let mut readback = Vec::new();
        file.read_to_end(&mut readback).map_err(|e| e.to_string())?;
        if readback != jpeg.bytes {
            return Err("CAD預覽讀回不符，未顯示。".into());
        }
        let path = format!("_AI_Output/{folder}/{name}");
        self.cad_previews.insert(
            path.to_ascii_lowercase(),
            json!({"snapshot_id":id,"sha256":format!("{:x}",Sha256::digest(&jpeg.bytes))}),
        );
        // 圖片只在明確要求時另走既有品質模型辨識管線，不隨工具回覆附上Base64。
        if !show_user {
            return Ok(
                json!({"snapshot_id":id,"revision":revision,"path":path,"preview":metadata,
                "notice":"尚未傳圖或完成辨識；需要圖片輔助時以analyze_image讀此路徑，修改仍以精確圖元資料為準。"}),
            );
        }
        let image = vision::input::Image {
            path: path.clone(),
            sha256: format!("{:x}", Sha256::digest(&jpeg.bytes)),
            mime: "image/jpeg",
            width: jpeg.width,
            height: jpeg.height,
            source_bytes: bmp.len(),
            frames: 1,
            bytes: jpeg.bytes,
        };
        let prompt = json!({"preview":true,"snapshot_id":id,"image":image.data_url()?,"candidates":query["candidates"],
            "message":metadata["notice"],"document":self.cad_active.as_ref().map(|s|&s["document"])});
        let choice = self.cad_waiter.as_mut().ok_or("目前沒有桌面預覽入口。")?(
            &prompt,
            cancel,
            std::time::Instant::now() + std::time::Duration::from_secs(600),
        )?;
        let selected = choice
            .strip_prefix("candidate:")
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|v| candidates.contains(v));
        if choice != "cancel" && selected.is_none() {
            return Err("候選回覆不合法。".into());
        }
        // 等待期間可能切圖；重新核對後才把選擇交給模型，但不把選擇當寫入同意。
        self.guard_cad_snapshot(id, cancel)?;
        Ok(
            json!({"snapshot_id":id,"revision":revision,"path":path,"preview":metadata,"selected_candidate":selected,
            "selected_handle":selected.map(|i|self.cad_snapshot.as_ref().map(|s|s.rows[i-1]["handle"].clone())),
            "notice":if selected.is_some() {
                "使用者已在預覽完成指認，selected_candidate／selected_handle 就是其選擇，不需再次詢問選哪個。若原要求已指定修改，可先讀編輯方法及 cad_query(details=true)，再呼叫對應修改工具；原生端仍會依既有規則處理寫入確認，預覽本身不繞過確認。"
            } else {
                "使用者已關閉預覽，沒有選擇候選；selected_candidate／selected_handle 為 null，不得猜測目標或直接修改。"
            }}),
        )
    }
}

#[cfg(test)]
mod version_alias_tests {
    use super::*;

    #[test]
    fn copy_year_uses_inventory_and_preserves_bound_executor() {
        let inventory = json!({"versions":[
            {"progid":"GStarCAD.Application.22","year":2022,"server_exists":true},
            {"progid":"Icad.Application.1","year":2008,"server_exists":true}
        ]});
        assert_eq!(
            resolve_copy_year("2008", None, &inventory).unwrap(),
            "Icad.Application.1"
        );
        assert_eq!(
            resolve_copy_year("2022", None, &inventory).unwrap(),
            "GStarCAD.Application.22"
        );
        assert_eq!(
            resolve_copy_year("2008", Some("icad.application.1"), &inventory).unwrap(),
            "icad.application.1"
        );
        assert!(
            resolve_copy_year("2022", Some("Icad.Application.1"), &inventory)
                .unwrap_err()
                .contains("尚未啟動CAD")
        );
    }

    #[test]
    fn copy_year_rejects_missing_or_ambiguous_installations_without_fallback() {
        let ambiguous = json!({"versions":[
            {"progid":"a","year":2008,"server_exists":true},
            {"progid":"b","year":2008,"server_exists":true},
            {"progid":"c","year":2022,"server_exists":false}
        ]});
        assert!(resolve_copy_year("2008", None, &ambiguous).is_err());
        assert!(resolve_copy_year("2022", None, &ambiguous).is_err());
        assert!(resolve_copy_year("2019", None, &ambiguous).is_err());
        assert!(resolve_copy_year("2008", None, &json!({"versions":[]})).is_err());
    }

    #[test]
    fn copy_snapshot_overview_returns_actual_native_version() {
        let selected = json!({"progid":"Icad.Application.1","year":2008});
        let snapshot = Snapshot::new(
            json!({"entities":[],"entity_count":0,"selected":selected,"revision":"fixture"}),
        )
        .unwrap();
        assert_eq!(snapshot.overview()["selected"], selected);
    }
}
