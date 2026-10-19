//! 目前文件的授權與讀取記錄只保留本次執行記憶體，不從磁碟恢復寫入授權。
use super::*;

impl Broker {
    pub(super) fn read_active_cad(
        &mut self,
        version: &str,
        offset: usize,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        // 舊分頁工具沿用完整本機快照；新任務優先 cad_snapshot／cad_query。
        if offset == 0 {
            let previous = self.cad_active.clone();
            self.snapshot_cad(version, None, true, cancel)?;
            if let (Some(previous), Some(current)) = (previous, self.cad_active.as_mut()) {
                if previous["document"] == current["document"]
                    && previous["revision"] == current["revision"]
                {
                    current["handles"] = previous["handles"].clone();
                }
            }
        }
        let id = self
            .cad_snapshot
            .as_ref()
            .filter(|s| s.data["source"].is_null())
            .map(|s| s.id.clone())
            .ok_or("請先讀取目前圖面快照。")?;
        self.guard_cad_snapshot(&id, cancel)?;
        let snapshot = self.cad_snapshot.as_ref().ok_or("缺少目前圖面快照。")?;
        let end = offset.saturating_add(100).min(snapshot.rows.len());
        let rows = snapshot
            .rows
            .get(offset..end)
            .ok_or("分頁超出圖元數量。")?
            .to_vec();
        let mut result = snapshot.data.clone();
        result["snapshot_id"] = json!(id);
        result["entities"] = json!(rows);
        result["offset"] = json!(offset);
        result["next_offset"] = json!((end < snapshot.rows.len()).then_some(end));
        result["advice"] = json!("此頁來自本機快照；需要特定物件請cad_query，不必逐頁讀完。手動改圖請refresh=true；修改前仍核對完整revision。");
        let handles = self
            .cad_active
            .as_mut()
            .and_then(|s| s["handles"].as_array_mut())
            .ok_or("目前圖面授權已失效。")?;
        for row in rows {
            if row["editable"] == true && !handles.contains(&row["handle"]) {
                handles.push(row["handle"].clone());
            }
        }
        Ok(result)
    }

    pub(super) fn edit_active_cad(
        &mut self,
        revision: &str,
        edits: &[Value],
        operations: &[Value],
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        self.cad_session.version("latest")?;
        if self.cad_failed || self.cad_attempts >= 6 {
            return Err("先前 CAD 結果未確認或本任務已達六次操作，請先檢查圖面。".into());
        }
        let state = self.cad_active.clone().ok_or("請先讀取目前圖面。")?;
        if state["document"]["editable_session"] == false {
            return Err(format!(
                "目前圖面身分不足，尚未執行任何指令或修改。scope={}；原因={}。請直接回報此診斷；讀取其餘圖元或反覆回讀工作紀錄不會解除限制。使用者要求不儲存時，不自行改成另存副本。",
                state["document"]["identity_scope"], state["document"]["identity_reason"]
            ));
        }
        let handles = state["handles"].as_array().ok_or("缺少已讀取的 Handle。")?;
        if state["revision"] != revision
            || edits.len() > 100
            || operations.len() > 100
            || edits.iter().any(|e| !handles.contains(&e["handle"]))
        {
            return Err("目前圖面 revision 或 Handle 不符；請重新讀取，單次至多各 100 項。".into());
        }
        let ask = self
            .cad_waiter
            .as_mut()
            .ok_or("目前圖面修改需要桌面確認。")?;
        let choice = ask(
            &json!({"confirm_document":true,"document":state["document"],
            "message":"將修改這份目前開啟的圖面；不儲存、不關閉。操作期間請暫停手動編輯或切換 CAD 文件。完成後可在 CAD 使用 Undo 復原本輪。"}),
            cancel,
            std::time::Instant::now() + std::time::Duration::from_secs(600),
        )?;
        if choice != "confirm" {
            let result = json!({"status":"CANCELLED","saved":false,"drawing_started":false});
            self.cad_session.edited(&result, edits);
            return Ok(result);
        }
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
        let report_path = output.join("cad-report.json");
        crate::storage::atomic_write(
            &report_path,
            &serde_json::to_vec_pretty(&json!({
                "status":"PENDING","document":state["document"],"saved":false,
                "advice":"操作尚未回報；若程序中斷，結果未知，請查看 CAD，不可自動重送。"
            }))
            .map_err(|e| e.to_string())?,
        )?;
        // 寫入前即作廢舊快照；成功、失敗與程序中斷都需重讀後才能再操作。
        self.cad_active = None;
        self.cad_snapshot = None;
        self.cad_failed = true;
        self.cad_attempts += 1;
        let execution = crate::cad::execute_interactive(
            json!({"action":"active_edit","version":state["version"],
            "document":state["document"],"revision":revision,"edits":edits,"operations":operations}),
            cancel,
            self.cad_waiter.as_mut(),
            self.cad_progress.as_mut(),
        );
        let mut result = match execution {
            Ok(result) => result,
            Err(error) => json!({"status":"UNKNOWN","error":error,"saved":false,
                "advice":"結果未知，可能已部分修改；請人工檢查目前圖面，不自動重送。"}),
        };
        result["report_path"] = json!(format!("_AI_Output/{folder}/cad-report.json"));
        self.cad_session.edited(&result, edits);
        crate::storage::atomic_write(
            &report_path,
            &serde_json::to_vec_pretty(&result).map_err(|e| e.to_string())?,
        )?;
        if result["status"] == "EDITED_UNSAVED" || result["drawing_started"] == false {
            self.cad_failed = false;
        }
        // 不註冊 DWG 成果路徑；目前圖面可能尚未儲存，工具紀錄才是本次操作證據。
        Ok(result)
    }
}
