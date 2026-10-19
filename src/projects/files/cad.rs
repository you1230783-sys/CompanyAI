//! AI CAD 成果只建立在全新資料夾；不能以工具參數指定既有 DWG 或 COM 方法。
use super::*;
use crate::storage;
use sha2::{Digest, Sha256};

/// Python 只有明確未繪圖、或已回報的無效參數錯誤才允許復原。
/// 缺欄位、程序中斷與未知結果一律保留鎖定，不能憑檔案不存在判斷安全。
pub(super) fn recoverable(result: &Value) -> bool {
    result["recovery"] == "user_requested_new_document"
        || (result["recovery"] == "correct_and_retry" && result["drawing_started"] == false)
        || (result["recovery"] == "revised_new_document"
            && result["owned_document_created"] == true)
}

impl Broker {
    pub(super) fn draw_cad(
        &mut self,
        name: &str,
        version: &str,
        operations: &[Value],
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        if self.cad_design.plan.is_some() {
            return Err(
                "已有本機分段計畫；請以cad_plan_commit合併，不要重送cad_draw。尚未建立新圖面。"
                    .into(),
            );
        }
        self.draw_cad_with_limit(name, version, operations, 100, cancel)
    }

    /// 上限由固定的呼叫入口決定，模型不能自行提高。分段計畫仍一次原生執行。
    pub(super) fn draw_cad_with_limit(
        &mut self,
        name: &str,
        version: &str,
        operations: &[Value],
        limit: usize,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        if self.cad_failed {
            return Err(
                "本任務先前 CAD 結果未確認，已停止後續繪圖；請人工檢查後另開新任務。".into(),
            );
        }
        if self.cad_design.pending() {
            return Err("尚未保存新圖配置方案；先compact_context保存已知條件、假設與配置，再提交完整cad_draw。尚未建立圖面。".into());
        }
        if self.cad_artifacts.len() >= 10 {
            return Err("每個任務最多建立十份 CAD 圖面。".into());
        }
        if self.cad_attempts >= 6 {
            return Err("本任務 CAD 已達六次執行上限，請檢查診斷後另開新任務。".into());
        }
        // 不以新檔名或 operation_id 規避已部分執行計畫的重播限制。
        let plan = format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&json!({"version":version,"operations":operations}))
                    .map_err(|e| e.to_string())?
            )
        );
        if self.cad_partial_plans.contains(&plan) {
            return Err(
                "此計畫曾部分執行或確認含不支援指令，不能原樣重送；請修改計畫後再提交。".into(),
            );
        }
        let name = if std::path::Path::new(name).extension().is_none() {
            format!("{name}.dwg")
        } else {
            name.to_owned()
        };
        let name = name.as_str();
        let rel = relative(name)?;
        if rel.components().count() != 1 || !name.to_ascii_lowercase().ends_with(".dwg") {
            return Err("CAD 名稱需為單一 .dwg 檔名。".into());
        }
        if version.is_empty() || version.len() > 100 {
            return Err("新圖需使用已確認的 CAD ProgID，長度為1至100字元。".into());
        }
        // 新圖不能讓latest重新選到另一套CAD；不替模型猜測或改寫使用者的版本。
        // 使用者未指定時，可先inventory取得確切ProgID，再自行選擇並提交。
        if version.eq_ignore_ascii_case("latest") {
            return Err("cad_draw需使用使用者指定或cad_inventory回傳的確切ProgID，不接受latest；尚未建立圖面。請依原要求重送正確版本，未指定版本時先查inventory。".into());
        }
        if operations.is_empty() || operations.len() > limit {
            return Err(format!(
                "CAD 收到{}項operations，本入口必須為1至{limit}項；尚未建立圖面。請重新規劃，勿刪減使用者要求後宣稱完成。",
                operations.len()
            ));
        }
        let _root = pin(&self.project.root)?;
        let base = self.project.root.join("_AI_Output");
        match fs::create_dir(&base) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e.to_string()),
        }
        let _base = pin(&base)?;
        let folder_name = create_output_folder(&base)?;
        let folder = base.join(&folder_name);
        // CAD SaveAs 需要目錄寫入共用，與 Office 相同以 anchor 固定新目錄。
        let _folder = pin_stage(&folder)?;
        let path = format!("_AI_Output/{folder_name}/{name}");
        self.cad_failed = true; // 出現錯誤或未知結果時，不允許模型換 operation_id 立即重播。
        self.cad_last_result = Some(json!({"status":"UNKNOWN","saved":false,
            "report_path":format!("_AI_Output/{folder_name}/cad-report.json"),
            "notice":"尚未收到原生結果；程序中斷時不能憑檔案不存在判斷可重送。"}));
        self.cad_attempts += 1;
        let mut result = crate::cad::execute_interactive(
            json!({"action":"draw", "version":version,
            "operations":operations,"staged_plan":limit>100,"output":folder.join(name)}),
            cancel,
            self.cad_waiter.as_mut(),
            self.cad_progress.as_mut(),
        )?;
        if result["drawing_started"] == false {
            self.cad_attempts -= 1;
        }
        result["intended_path"] = json!(path);
        self.cad_last_result = Some(json!({"status":result["status"],"step":result["step"],
            "error":result["error"].as_str().map(|s|s.chars().take(4096).collect::<String>()),
            "recovery":result["recovery"],"drawing_started":result["drawing_started"],
            "completed_operations":result["completed_operations"],"saved":result["saved"],
            "file_exists":result["file_exists"],"intended_path":path,
            "report_path":format!("_AI_Output/{folder_name}/cad-report.json"),
            "verification_difference":result["verification_difference"]}));
        if result["status"] == "DATA_VERIFIED_VISUAL_PENDING" {
            result["path"] = json!(path);
        }
        // 保留執行報告，包括失敗所在步驟；不把它偽裝成成功 DWG。
        storage::atomic_write(
            &folder.join("cad-report.json"),
            &serde_json::to_vec_pretty(&result).map_err(|e| e.to_string())?,
        )?;
        if result["status"] != "DATA_VERIFIED_VISUAL_PENDING" {
            if recoverable(&result) {
                self.cad_failed = false;
                if result["recovery"] == "revised_new_document"
                    || result["unsupported_commands"].is_array()
                {
                    self.cad_partial_plans.push(plan);
                }
            }
            return Err(format!(
                "CAD 未完成：{}；診斷：_AI_Output/{folder_name}/cad-report.json；file_exists={}，saved={}，completed_operations={}。{} 預定路徑不是已交付成果，未存檔的部分圖面可能只在 CAD 記憶體中。",
                result["error"], result["file_exists"], result["saved"], result["completed_operations"], result["advice"]
            ));
        }
        let mut file = checked_file(&folder.join(name))?;
        let mut hash = Sha256::new();
        std::io::copy(&mut file, &mut hash).map_err(|e| e.to_string())?;
        let sha256 = format!("{:x}", hash.finalize());
        result["sha256"] = json!(sha256);
        result["instructions"] = json!("圖面已存檔重開核對並關閉；請從成果位置開啟，人工確認外觀。修改需求請建立另一份新 DWG，不會修改原件。");
        self.cad_artifacts.push(super::super::python::Artifact {
            path: path.clone(),
            sha256,
        });
        self.published.push(path);
        self.cad_failed = false;
        Ok(result)
    }

    pub(super) fn verify_cad_artifact(
        &self,
        artifact: &super::super::python::Artifact,
    ) -> AppResult<()> {
        let rel = relative(&artifact.path)?;
        if rel.components().count() != 3
            || !rel.starts_with("_AI_Output")
            || rel
                .extension()
                .is_none_or(|s| !s.eq_ignore_ascii_case("dwg"))
        {
            return Err("CAD 成果路徑不合法。".into());
        }
        let target = self.project.root.join(rel);
        let _dirs = pin(target.parent().ok_or("缺少 CAD 成果目錄。")?)?;
        let mut file = checked_file(&target)?;
        let mut hash = Sha256::new();
        std::io::copy(&mut file, &mut hash).map_err(|e| e.to_string())?;
        if format!("{:x}", hash.finalize()) != artifact.sha256 {
            return Err("CAD 成果已變更，不能沿用舊版交付驗證。".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_drawing_rejects_latest_before_creating_output_or_calling_cad() {
        let root = std::env::temp_dir().join(format!(
            "lmai-cad-version-{}",
            crate::jobs::new_id().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let project = Project {
            id: "version".into(),
            name: "CAD".into(),
            root: root.clone(),
            imports: Default::default(),
        };
        let mut broker = Broker::new(project, "task".into()).unwrap();
        let operations = [json!({"kind":"line","start":[0,0],"end":[1,0],"color":7})];
        let error = broker
            .draw_cad("test.dwg", "LaTeSt", &operations, &AtomicBool::new(false))
            .unwrap_err();
        assert!(error.contains("確切ProgID"));
        assert_eq!(broker.cad_attempts, 0);
        assert!(!broker.cad_failed);
        assert!(!root.join("_AI_Output").exists());
    }

    #[test]
    fn cad_recovery_requires_explicit_evidence() {
        assert!(recoverable(
            &json!({"recovery":"correct_and_retry", "drawing_started":false})
        ));
        assert!(recoverable(
            &json!({"recovery":"revised_new_document", "owned_document_created":true})
        ));
        assert!(!recoverable(&json!({"recovery":"correct_and_retry"})));
        assert!(!recoverable(
            &json!({"recovery":"manual_check", "owned_document_created":true})
        ));
        assert!(!recoverable(&json!({})));
    }

    #[test]
    fn cad_active_unsupported_does_not_call_worker_again() {
        let root =
            std::env::temp_dir().join(format!("lmai-cad-read-{}", crate::jobs::new_id().unwrap()));
        fs::create_dir_all(&root).unwrap();
        let project = Project {
            id: "cad-read".into(),
            name: "CAD".into(),
            root,
            imports: Default::default(),
        };
        let mut broker = Broker::new(project, "task".into()).unwrap();
        broker.cad_active_unsupported = Some("Icad.Application.1".into());
        let error = broker
            .read_active_cad("Icad.Application.1", 0, &AtomicBool::new(false))
            .unwrap_err();
        assert!(error.contains("不再重試"));
    }

    #[test]
    fn cad_checkpoint_preserves_artifact_and_rejects_external_changes() {
        // 合成檔案只驗證 broker 所有權／雜湊，不將它稱為真實 DWG 相容性測試。
        let root =
            std::env::temp_dir().join(format!("lmai-cad-basic-{}", crate::jobs::new_id().unwrap()));
        let folder = root.join("_AI_Output/case");
        fs::create_dir_all(&folder).unwrap();
        let bytes = b"synthetic CAD artifact, not a real DWG";
        fs::write(folder.join("test.dwg"), bytes).unwrap();
        let project = Project {
            id: "cad-basic".into(),
            name: "CAD".into(),
            root,
            imports: Default::default(),
        };
        let mut broker = Broker::new(project.clone(), "task".into()).unwrap();
        broker
            .cad_artifacts
            .push(super::super::super::python::Artifact {
                path: "_AI_Output/case/test.dwg".into(),
                sha256: format!("{:x}", Sha256::digest(bytes)),
            });
        broker.cad_failed = true;
        broker.cad_attempts = 2;
        broker.cad_partial_plans.push("partial-plan".into());
        let saved = broker.saved().unwrap();
        let mut restored = Broker::new(project, "task".into()).unwrap();
        restored.restore(saved, &AtomicBool::new(false)).unwrap();
        assert!(restored.cad_failed);
        assert_eq!(restored.cad_attempts, 2);
        assert_eq!(restored.cad_partial_plans, vec!["partial-plan"]);
        assert_eq!(
            restored.finish(&[]).unwrap(),
            vec!["_AI_Output/case/test.dwg"]
        );
        assert!(restored
            .draw_cad("next.dwg", "latest", &[], &AtomicBool::new(false))
            .unwrap_err()
            .contains("先前 CAD"));
        fs::write(folder.join("test.dwg"), b"changed").unwrap();
        assert!(restored.finish(&[]).is_err());
    }
}
