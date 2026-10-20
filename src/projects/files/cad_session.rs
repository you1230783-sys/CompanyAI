//! 目前圖面的任務狀態。由程式維護，不依賴模型筆記或可被整理掉的工具歷史。
use crate::AppResult;
use serde_json::{json, Value};

#[derive(Default)]
pub(super) struct ActiveSession {
    /// 第一次成功讀取後固定版本及文件；不從 checkpoint 恢復寫入授權。
    binding: Option<Value>,
    last_edit: Option<Value>,
    terminal: bool,
}

impl ActiveSession {
    /// latest 只在首次連線時選新版；後續刷新沿用已選版本，不偷偷切換 CAD。
    pub(super) fn version(&self, requested: &str) -> AppResult<String> {
        if self.terminal {
            return Err("目前圖面操作已停止；請依 cad_session.last_edit 回報失敗，不重讀、重試或改用其他 CAD。".into());
        }
        let Some(binding) = &self.binding else {
            return Ok(requested.to_owned());
        };
        let selected = binding["version"].as_str().ok_or("CAD 版本狀態不完整。")?;
        if requested != "latest" && !requested.eq_ignore_ascii_case(selected) {
            return Err(format!("本任務目前圖面固定為 {selected}，不能改讀 {requested}。需要另一套 CAD 時請另開任務。"));
        }
        Ok(selected.to_owned())
    }

    pub(super) fn bind(&mut self, version: &str, document: &Value) -> AppResult<()> {
        let binding = json!({"version":version,"document":document});
        if self
            .binding
            .as_ref()
            .is_some_and(|previous| previous != &binding)
        {
            return Err(
                "本任務目前圖面的身分已改變，未授權改用另一份文件；請回報或另開任務。".into(),
            );
        }
        self.binding = Some(binding);
        Ok(())
    }

    /// 記錄原生結果及實際提交值；失敗不再用「缺少快照」覆蓋停止原因。
    pub(super) fn edited(&mut self, result: &Value, edits: &[Value]) {
        self.terminal = result["recovery"] == "unsupported_capability"
            || result["status"] == "CANCELLED"
            || result["status"] == "UNKNOWN"
            || result["recovery"] == "manual_check"
            || (result["status"] != "EDITED_UNSAVED" && result["drawing_started"] == true);
        self.last_edit = Some(json!({
            "status":result["status"],"step":result["step"],"error":result["error"],
            "recovery":result["recovery"],"drawing_started":result["drawing_started"],
            "completed_operations":result["completed_operations"],"saved":result["saved"],
            "modified_handles":result["modified_handles"],"report_path":result["report_path"],
            "verification":super::cad_comparison::project_verification(&result["verification"]),"submitted_edits":edits.iter().take(10).collect::<Vec<_>>(),
            "omitted_submitted_edits":edits.len().saturating_sub(10)
        }));
    }

    pub(super) fn stopped(&self) -> bool {
        self.terminal
    }

    pub(super) fn context(&self) -> Value {
        json!({"binding":self.binding,"stopped":self.terminal,"last_edit":self.last_edit,
        "next_action":if self.terminal {
            "finish回報實際失敗與是否已修改。不要重讀圖面、換CAD或繞過限制。"
        } else if self.last_edit.as_ref().is_some_and(|r|r["status"]=="EDITED_UNSAVED") {
            "原生修改與verification已完成，可finish；只回報工具確認的數值。需要額外量測才刷新同一文件一次，不重送已成功修改。"
        } else {
            "沿用目前快照及recent_details。已取得所需詳細資料且目標明確時直接編輯，不重載技能或重查相同候選。"
        }})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_keeps_original_version_and_document_binding() {
        let mut session = ActiveSession::default();
        let document = json!({"pid":1,"name":"Drawing1"});
        session.bind("Icad.Application.1", &document).unwrap();
        assert_eq!(session.version("latest").unwrap(), "Icad.Application.1");
        assert!(session.version("GStarCAD.Application.22").is_err());
        assert!(session
            .bind("Icad.Application.1", &json!({"pid":1,"name":"Drawing2"}))
            .is_err());
        session.bind("Icad.Application.1", &document).unwrap();
    }

    #[test]
    fn unsupported_edit_survives_missing_snapshot_and_stops_rereads() {
        let mut session = ActiveSession::default();
        session.edited(
            &json!({"status":"FAILED","recovery":"unsupported_capability",
            "step":"prepare_undo_group","error":"StartUndoMark","drawing_started":false}),
            &[],
        );
        assert!(session.stopped());
        assert!(session.version("latest").is_err());
        assert_eq!(session.context()["last_edit"]["error"], "StartUndoMark");
        assert!(session.context()["next_action"]
            .as_str()
            .unwrap()
            .contains("finish"));
    }

    #[test]
    fn corrected_parameters_can_retry_but_success_preserves_native_evidence() {
        let mut session = ActiveSession::default();
        session.edited(
            &json!({"status":"FAILED","recovery":"correct_and_retry","drawing_started":false}),
            &[],
        );
        assert!(!session.stopped());
        session.edited(
            &json!({"status":"EDITED_UNSAVED","saved":false,
            "modified_handles":["A"],"verification":{"handles_verified":645}}),
            &[json!({"handle":"A","kind":"radius","value":40})],
        );
        assert!(!session.stopped());
        assert_eq!(
            session.context()["last_edit"]["submitted_edits"][0]["value"],
            40
        );
        assert_eq!(
            session.context()["last_edit"]["verification"]["handles_verified"],
            645
        );
    }
}
