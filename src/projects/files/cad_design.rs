//! 新圖文字方案與本機分段計畫；暫存計畫不授予既有CAD文件寫入權限。
use crate::AppResult;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// 舊檢查點預設不啟用新門檻，保留已提交要求的相容性。
/// 新載入cad-new才開始規劃，完成交接後才公告完整繪圖Schema。
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Design {
    required: bool,
    note: Option<Note>,
    pub(super) plan: Option<super::cad_plan::Plan>,
    pub(super) next_plan: u64,
}

#[derive(Serialize, Deserialize)]
struct Note {
    working_note: String,
    next_step: String,
}

impl Design {
    pub(super) fn staged(&self) -> bool {
        self.required
    }
    pub(super) fn begin(&mut self) {
        // 回答必要補充後，模型可能先保存交接，再載入cad-new。
        // 同一Broker內先存的筆記可直接沿用；真正重新切入下一個設計才清除舊方案。
        if self.required {
            self.note = None;
        }
        self.required = true;
    }

    pub(super) fn pending(&self) -> bool {
        self.required && self.note.is_none()
    }

    /// 與一般交接採相同長度及空值檢查；無效筆記不能解鎖繪圖工具。
    pub(super) fn record(&mut self, note: &str, next: &str) -> AppResult<()> {
        super::super::progress::validate_handoff(note, &[], next)?;
        self.note = Some(Note {
            working_note: note.to_owned(),
            next_step: next.to_owned(),
        });
        Ok(())
    }

    /// 未進入CAD前只暫存通用交接，不公告為CAD方案；其他技能的後續筆記
    /// 不能覆蓋已啟用CAD的方案，也不把舊任務或外部記憶帶入本次Broker。
    pub(super) fn remember_before_loading(&mut self, note: &str, next: &str) -> AppResult<()> {
        if !self.required {
            self.record(note, next)?;
        }
        Ok(())
    }

    pub(super) fn context(&self) -> Value {
        if !self.required {
            return Value::Null;
        }
        json!({"phase":if self.pending() {"planning"} else {"ready_to_stage"},
            "design":self.note,
            "plan":self.plan.as_ref().map(|plan|plan.context()),
            "notice":"文字方案本身不是繪圖成功證據，也不是使用者實測尺寸或寫入授權；最新要求與原生結果優先。"})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_valid_handoff_completes_planning_and_survives_serialization() {
        let mut design = Design::default();
        design.begin();
        assert!(design.pending());
        assert!(design.record(" ", "繪圖").is_err());
        assert!(design.pending());
        design
            .record("毫米；兩層並排；基地尺寸待確認。", "提交完整新圖")
            .unwrap();
        let restored: Design =
            serde_json::from_value(serde_json::to_value(&design).unwrap()).unwrap();
        assert!(!restored.pending());
        assert_eq!(
            restored.context()["design"]["working_note"],
            "毫米；兩層並排；基地尺寸待確認。"
        );
        design.begin();
        assert!(design.pending());
        assert!(design.context()["design"].is_null());
    }

    #[test]
    fn legacy_design_is_not_a_new_authorization_gate() {
        let mut design: Design = serde_json::from_value(json!({})).unwrap();
        assert!(!design.pending());
        design.record("一般資料交接", "繼續").unwrap();
        assert!(design.context().is_null());
    }

    #[test]
    fn handoff_before_loading_survives_checkpoint_and_later_generic_notes_do_not_replace_it() {
        let mut design = Design::default();
        design
            .remember_before_loading("已確認每層8×12m，兩層並排。", "載入CAD後繪圖")
            .unwrap();
        assert!(design.context().is_null(), "尚未啟用CAD，不應公告CAD方案");
        let mut restored: Design =
            serde_json::from_value(serde_json::to_value(&design).unwrap()).unwrap();
        restored.begin();
        assert!(!restored.pending());
        restored
            .remember_before_loading("另一技能的通用筆記", "整理文件")
            .unwrap();
        assert_eq!(
            restored.context()["design"]["working_note"],
            "已確認每層8×12m，兩層並排。"
        );
        restored.begin();
        assert!(restored.pending(), "下一個新設計不能沿用前張圖的方案");
    }
}
