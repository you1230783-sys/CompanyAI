//! 將原生修改前後比較轉成使用者可讀清單，不把模型計畫當成實際修改。
use super::*;
/// 原生完整差異另存於checkpoint與報告；模型常駐狀態只帶計數，避免大圖塞滿請求。
pub(super) fn project_verification(value: &Value) -> Value {
    let mut projected = value.clone();
    if let Some(comparison) = projected.get_mut("comparison") {
        *comparison = json!({"complete":comparison["complete"],"before_count":comparison["before_count"],"after_count":comparison["after_count"],
            "added":comparison["added"].as_array().map(Vec::len),"deleted":comparison["deleted"].as_array().map(Vec::len),"changed":comparison["changed"].as_array().map(Vec::len),
            "notice":"完整差異已保留，請export_cad_comparison匯出修改清單。"});
    }
    projected
}
impl Broker {
    pub(super) fn retain_cad_comparison(&mut self, id: &str, result: &mut Value) -> AppResult<()> {
        let data = &mut result["result"];
        let full = data
            .pointer("/after_reopen/comparison")
            .or_else(|| data.pointer("/verification/comparison"))
            .or_else(|| data.pointer("/before_save/comparison"))
            .cloned();
        if let Some(comparison) = full {
            if self.comparison_records.len() >= 20 && !self.comparison_records.contains_key(id) {
                return Err("CAD比較已達20批上限，完整資料仍在原生報告，未丟棄舊資料。".into());
            }
            self.comparison_records.insert(
                id.into(),
                json!({"comparison":comparison,"status":data["status"],"saved":data["saved"]}),
            );
            for field in ["verification", "before_save", "after_reopen"] {
                if data[field].is_object() {
                    data[field] = project_verification(&data[field]);
                }
            }
            data["comparison_available"] = json!(true);
        }
        Ok(())
    }
    pub(super) fn export_cad_comparison(
        &mut self,
        operation: &str,
        name: &str,
    ) -> AppResult<Value> {
        let (request, _result) = self
            .recorded_operation(operation)?
            .ok_or("CAD來源操作不存在。")?;
        if !matches!(
            request["tool"].as_str(),
            Some("cad_edit" | "cad_active_edit")
        ) {
            return Err("比較報告只能引用實際CAD修改操作。".into());
        }
        let data = self
            .comparison_records
            .get(operation)
            .ok_or("原生結果沒有可用的前後比較；不依提交參數猜測修改。")?;
        let comparison = &data["comparison"];
        let path = relative(name)?;
        if path.components().count() != 1
            || path
                .extension()
                .is_none_or(|e| !e.eq_ignore_ascii_case("txt"))
        {
            return Err("比較報告名稱需為單一TXT檔名。".into());
        }
        let mut lines = vec![
            "CAD 修改比較報告".to_owned(),
            format!("來源操作：{operation}"),
            format!("原生狀態：{}；已儲存：{}", data["status"], data["saved"]),
            format!(
                "修改前圖元：{}；修改後圖元：{}；比較完整：{}",
                comparison["before_count"], comparison["after_count"], comparison["complete"]
            ),
            "範圍：實際可讀屬性；不是使用者規格、外觀或所有尺寸關聯均通過。".into(),
        ];
        for (field, label) in [
            ("added", "新增圖元"),
            ("deleted", "刪除圖元"),
            ("changed", "變更圖元"),
            ("annotation_checks", "尺寸／引線檢查"),
        ] {
            lines.push(format!("\n{label}："));
            if let Some(rows) = comparison[field].as_array() {
                if rows.is_empty() {
                    lines.push("無".into());
                }
                for row in rows {
                    lines.push(serde_json::to_string_pretty(row).map_err(|e| e.to_string())?);
                }
            } else {
                lines.push("未取得完整明細，請查看原生報告。".into());
            }
        }
        lines.push(format!("\n{}", comparison["scope"]));
        let base = self.project.root.join("_AI_Output");
        fs::create_dir_all(&base).map_err(|e| e.to_string())?;
        let _pin = pin(&base)?;
        let folder = shared_output_folder(
            &base,
            &mut self.output_folder,
            self.output_conversation.as_deref(),
        )?;
        let text = lines.join("\n");
        let json_name = format!(
            "{}.json",
            path.file_stem().ok_or("報告檔名無效。")?.to_string_lossy()
        );
        let data = serde_json::to_vec_pretty(comparison).map_err(|e| e.to_string())?;
        let txt_path = diagrams::save(&self.project, &folder, name, text.as_bytes())?;
        let json_path = diagrams::save(&self.project, &folder, &json_name, &data)?;
        use sha2::{Digest, Sha256};
        for (path, bytes) in [(&txt_path, text.as_bytes()), (&json_path, data.as_slice())] {
            self.comparison_artifacts
                .push(super::super::python::Artifact {
                    path: path.clone(),
                    sha256: format!("{:x}", Sha256::digest(bytes)),
                });
            self.published.push(path.clone());
        }
        Ok(
            json!({"text_path":txt_path,"json_path":json_path,"comparison_complete":comparison["complete"],"source_operation":operation,
            "notice":"報告列出實際差異；geometry_consistency=unverified與固定文字警告仍需確認，不視為尺寸關聯通過。"}),
        )
    }
}
