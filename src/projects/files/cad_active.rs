//! 目前文件的授權與讀取記錄只保留本次執行記憶體，不從磁碟恢復寫入授權。
use super::*;

impl Broker {
    pub(super) fn read_active_cad(
        &mut self,
        version: &str,
        offset: usize,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        if self.cad_active_unsupported.as_deref() == Some(version) {
            return Err("此 CAD 的目前圖面介面已回報不支援；本任務不再重試相同版本，請改用 DWG 副本或保留診斷。".into());
        }
        let previous = self.cad_active.clone().unwrap_or(Value::Null);
        if offset > 0 && previous.is_null() {
            return Err("請先讀取目前圖面的第一頁。".into());
        }
        let version = if offset > 0 {
            previous["version"].as_str().ok_or("缺少 CAD 版本。")?
        } else {
            version
        };
        let result = crate::cad::execute_interactive(
            json!({"action":"active_read", "version":version,
            "offset":offset,"document":previous["document"],"revision":previous["revision"]}),
            cancel,
            self.cad_waiter.as_mut(),
        )?;
        if result["status"] == "READ_ACTIVE" {
            let mut handles = if offset > 0 {
                previous["handles"].as_array().cloned().unwrap_or_default()
            } else {
                vec![]
            };
            for row in result["entities"].as_array().into_iter().flatten() {
                if row["editable"] == true && !handles.contains(&row["handle"]) {
                    handles.push(row["handle"].clone());
                }
            }
            self.cad_active = Some(
                json!({"document":result["document"],"revision":result["revision"],
                "version":result["selected"]["progid"],"units_code":result["units_code"],"handles":handles}),
            );
        } else {
            self.cad_active = None;
            if result["recovery"] == "unsupported_capability" {
                self.cad_active_unsupported = Some(version.to_owned());
            }
            return Err(format!("讀取目前 CAD 圖面失敗：{result}"));
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
            return Ok(json!({"status":"CANCELLED","saved":false,"drawing_started":false}));
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
        self.cad_failed = true;
        self.cad_attempts += 1;
        let execution = crate::cad::execute_interactive(
            json!({"action":"active_edit","version":state["version"],
            "document":state["document"],"revision":revision,"edits":edits,"operations":operations}),
            cancel,
            self.cad_waiter.as_mut(),
        );
        let mut result = match execution {
            Ok(result) => result,
            Err(error) => json!({"status":"UNKNOWN","error":error,"saved":false,
                "advice":"結果未知，可能已部分修改；請人工檢查目前圖面，不自動重送。"}),
        };
        result["report_path"] = json!(format!("_AI_Output/{folder}/cad-report.json"));
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
