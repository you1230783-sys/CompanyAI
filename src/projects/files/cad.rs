//! AI CAD 成果只建立在全新資料夾；不能以工具參數指定既有 DWG 或 COM 方法。
use super::*;
use crate::storage;
use sha2::{Digest, Sha256};

impl Broker {
    pub(super) fn draw_cad(
        &mut self,
        name: &str,
        version: &str,
        operations: &[Value],
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        if self.cad_failed {
            return Err(
                "本任務先前 CAD 結果未確認，已停止後續繪圖；請人工檢查後另開新任務。".into(),
            );
        }
        if self.cad_artifacts.len() >= 10 {
            return Err("每個任務最多建立十份 CAD 圖面。".into());
        }
        let rel = relative(name)?;
        if rel.components().count() != 1 || !name.to_ascii_lowercase().ends_with(".dwg") {
            return Err("CAD 名稱需為單一 .dwg 檔名。".into());
        }
        if version.is_empty()
            || version.len() > 100
            || operations.is_empty()
            || operations.len() > 100
        {
            return Err("CAD 版本或操作數量不合法。".into());
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
        let mut result = crate::cad::execute(
            json!({"action":"draw", "version":version,
            "operations":operations,"output":folder.join(name)}),
            cancel,
        )?;
        result["path"] = json!(path);
        // 保留執行報告，包括失敗所在步驟；不把它偽裝成成功 DWG。
        storage::atomic_write(
            &folder.join("cad-report.json"),
            &serde_json::to_vec_pretty(&result).map_err(|e| e.to_string())?,
        )?;
        if result["status"] != "DATA_VERIFIED_VISUAL_PENDING" {
            return Err(format!(
                "CAD 未完成：{}；診斷：_AI_Output/{folder_name}/cad-report.json；不自動重試。",
                result["error"]
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
        let saved = broker.saved().unwrap();
        let mut restored = Broker::new(project, "task".into()).unwrap();
        restored.restore(saved, &AtomicBool::new(false)).unwrap();
        assert!(restored.cad_failed);
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
