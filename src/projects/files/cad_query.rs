//! 快照、候選查詢及局部預覽的 Broker 邊界；原始圖元不放入模型工具結果。
use super::*;
use crate::projects::{
    cad_index::{Filter, Snapshot},
    vision,
};
use sha2::{Digest, Sha256};

impl Broker {
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
        let missing = if self.loaded_skills.iter().any(|id| id.starts_with("cad-")) {
            json!({"available":false,"notice":"本次執行沒有有效CAD快照；先cad_snapshot取得總覽，再按需查詢。舊工具紀錄是歷史資料，不能恢復目前圖面的修改授權。"})
        } else {
            Value::Null
        };
        self.cad_snapshot.as_ref().map_or(missing, |s| json!({
            "snapshot_id":s.id,"revision":s.data["revision"],"document":s.data["document"],"source":s.data["source"],
            "entity_count":s.rows.len(),"notice":"完整快照仍在本機；以cad_query取得指定候選詳細資料，不因上下文整理重讀CAD。作用中快照不等同即時狀態，修改前重新核對。"}))
    }

    /// 去重結果可以保留歷史，但失效快照不能再冒充可使用的候選。
    pub(super) fn current_cad_result(&self, tool: &Tool, result: Value) -> Value {
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
        let snapshot = self.cad_snapshot.as_ref().ok_or("缺少快照。")?;
        let result = snapshot.query(filter, offset, limit, details)?;
        // 只有實際交付詳細資料才授予已讀 Handle；索引、候選或縮圖不等於已核對。
        if details {
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
        let folder = create_output_folder(&base)?;
        let output = base.join(&folder);
        let _stage = pin_stage(&output)?;
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(output.join("cad-preview.jpg"))
            .map_err(|e| e.to_string())?;
        file.write_all(&jpeg.bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        file.rewind().map_err(|e| e.to_string())?;
        let mut readback = Vec::new();
        file.read_to_end(&mut readback).map_err(|e| e.to_string())?;
        if readback != jpeg.bytes {
            return Err("CAD預覽讀回不符，未顯示。".into());
        }
        let path = format!("_AI_Output/{folder}/cad-preview.jpg");
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
