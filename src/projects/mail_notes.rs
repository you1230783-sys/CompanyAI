//! 郵件成果與閱讀範圍分開保存：摘要是模型草稿，來源／版本／已讀頁由桌面核對。
//! 此狀態只進入既有 DPAPI checkpoint／工作紀錄，不另建立明文郵件 MD。
use crate::AppResult;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Disposition {
    Include,
    Exclude,
    Uncertain,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Digest {
    pub source_operation: String,
    pub summary: String,
    pub draft: String,
    pub disposition: Disposition,
    pub rationale: String,
    pub open_questions: String,
}
impl Digest {
    pub fn validate(&self) -> AppResult<()> {
        crate::jobs::validate_id(&self.source_operation)?;
        for (text, max) in [
            (&self.summary, 1000),
            (&self.draft, 700),
            (&self.rationale, 300),
            (&self.open_questions, 300),
        ] {
            if text.chars().count() > max {
                return Err("郵件摘要過長，請保留可寫入的事實、來源及未確認事項。".into());
            }
        }
        if self.summary.trim().is_empty()
            || self.rationale.trim().is_empty()
            || (self.disposition == Disposition::Include && self.draft.trim().is_empty())
        {
            return Err("郵件摘要需有 summary、rationale；include 另需 draft。未核對的主導者／日期請列 open_questions，不能自行確定。".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    #[default]
    Read,
    Organize,
    Write,
    Chart,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Page {
    pub operation: String,
    pub mail: Value,
    pub offset: usize,
    pub end: usize,
    pub total: usize,
}
impl Page {
    pub fn from_result(operation: &str, result: &Value) -> AppResult<Self> {
        let data = &result["result"];
        let number = |name: &str| {
            data[name]
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .ok_or("郵件來源缺少閱讀範圍。".to_owned())
        };
        let page = Self {
            operation: operation.into(),
            mail: data["mail"].clone(),
            offset: number("offset")?,
            end: number("next_offset")?,
            total: number("total_chars")?,
        };
        if result["ok"] != true
            || page.mail["mail_id"].as_str().is_none()
            || page.mail["revision"].as_str().is_none()
            || page.offset > page.end
            || page.end > page.total
            || data["text"]
                .as_str()
                .is_none_or(|s| s.chars().count() != page.end - page.offset)
        {
            return Err("郵件摘要來源不是已成功讀取的內文頁。".into());
        }
        Ok(page)
    }
    pub fn id(&self) -> String {
        super::text::revision(&json!([self.mail["mail_id"], self.mail["revision"]]).to_string())
            [..16]
            .into()
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Note {
    pub digest: Digest,
    pub pages: Vec<Page>,
    pub stale: bool,
    #[serde(default)]
    pub write_operations: Vec<Value>,
}
impl Note {
    pub fn sources(&self) -> Vec<Value> {
        self.pages
            .iter()
            .map(|p| json!({"operation":p.operation,"offset":p.offset,"end":p.end,"total":p.total}))
            .collect()
    }

    pub fn complete(&self) -> bool {
        let mut ranges: Vec<_> = self.pages.iter().map(|p| (p.offset, p.end)).collect();
        ranges.sort_unstable();
        let mut end = 0;
        for (a, b) in ranges {
            if a > end {
                return false;
            }
            end = end.max(b);
        }
        self.pages.first().is_some_and(|p| end == p.total)
    }
    pub fn index(&self, id: &str) -> Value {
        let mail = &self.pages[0].mail;
        json!({"note_id":id,"mail_id":mail["mail_id"],"subject":mail["subject"].as_str().unwrap_or("").chars().take(180).collect::<String>(),"sent_at":mail["sent_at"],"revision":mail["revision"],"disposition":self.digest.disposition,"body_complete":self.complete(),"stale":self.stale,"write_operations":self.write_operations})
    }
}
#[derive(Default, Serialize, Deserialize)]
pub struct State {
    pub stage: Stage,
    pub notes: BTreeMap<String, Note>,
    pub pending: Option<Page>,
    pub selected: Vec<String>,
    pub review_required: bool,
    /// 只能由明確的回讀階段開啟，且每個指定版本只准回讀一次已讀區段。
    pub reread: Vec<String>,
}
impl State {
    /// 舊 checkpoint 預設空狀態；有內容時逐份檢查，避免損壞索引在顯示時 panic。
    pub fn validate(&self) -> AppResult<()> {
        if self.notes.len() > 200 || self.selected.len() > 4 || self.reread.len() > 4 {
            return Err("保存的郵件筆記超過範圍。".into());
        }
        for (id, note) in &self.notes {
            note.digest.validate()?;
            if note.pages.is_empty()
                || note.pages.len() > 100
                || note.write_operations.len() > 8
                || note
                    .pages
                    .iter()
                    .any(|p| p.id() != *id || p.offset > p.end || p.end > p.total)
            {
                return Err("保存的郵件筆記來源不一致。".into());
            }
        }
        if self
            .selected
            .iter()
            .chain(&self.reread)
            .any(|id| !self.notes.contains_key(id))
        {
            return Err("保存的待寫條目不存在。".into());
        }
        Ok(())
    }
    pub fn observe(&mut self, operation: &str, result: &Value) -> AppResult<()> {
        let page = Page::from_result(operation, result)?;
        if self
            .notes
            .get(&page.id())
            .is_some_and(|n| n.pages.iter().any(|p| p.operation == operation))
        {
            return Ok(());
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|p| p.operation != operation)
        {
            return Err("前一封郵件尚未保存摘要。".into());
        }
        self.pending = Some(page);
        Ok(())
    }
    /// 只接受已記錄來源；多頁摘要是累積修訂，不將模型宣稱的「已讀完」當成證據。
    pub fn accept(&mut self, digest: Digest, page: Page) -> AppResult<()> {
        digest.validate()?;
        if self
            .pending
            .as_ref()
            .is_some_and(|p| p.operation != digest.source_operation)
        {
            return Err("請先摘要 pending.source_operation 的內文。".into());
        }
        let id = page.id();
        if self.notes.len() >= 200 && !self.notes.contains_key(&id) {
            return Err("郵件筆記已達 200 個版本，請縮小任務範圍。".into());
        }
        for (old_id, note) in &mut self.notes {
            if *old_id != id && note.pages[0].mail["mail_id"] == page.mail["mail_id"] {
                note.stale = true;
            }
        }
        let note = self.notes.entry(id).or_insert_with(|| Note {
            digest: digest.clone(),
            pages: vec![],
            stale: false,
            write_operations: vec![],
        });
        if !note.pages.iter().any(|p| p.operation == page.operation) {
            if note.pages.len() >= 100 {
                return Err("單封信摘要頁數過多，請縮小閱讀範圍。".into());
            }
            note.pages.push(page);
        }
        note.digest = digest;
        self.pending = None;
        Ok(())
    }
    pub fn context(&self) -> Value {
        let selected: Vec<_> = self
            .selected
            .iter()
            .filter_map(|id| {
                self.notes
                    .get(id)
                    .map(|n| json!({"source":n.index(id),"draft":n.digest,"pages":n.sources()}))
            })
            .collect();
        let previous = self
            .pending
            .as_ref()
            .and_then(|p| self.notes.get(&p.id()))
            .map(|n| &n.digest);
        json!({"stage":self.stage,"total_notes":self.notes.len(),"index_preview":self.notes.iter().take(12).map(|(id,n)| n.index(id)).collect::<Vec<_>>(),"pending":self.pending,"previous_digest":previous,"selected_drafts":selected,"review_required":self.review_required})
    }
    pub fn instructions(&self) -> String {
        if self.pending.is_none() && self.notes.is_empty() {
            return String::new();
        }
        format!("郵件成果（模型摘要，非原文驗證；原文僅按焦點回查）：{}\n優先用 read_mail_notes 查索引／草稿；set_work_stage 依 read→organize→write 或 chart 載入技能並帶入選定草稿。勿重掃已整理郵件。pending 有值時，在下一操作的 mail_note 字串放 JSON：{{\"source_operation\":\"pending.operation\",\"summary\":\"累積事實摘要≤1000字\",\"draft\":\"可寫入文字≤700字\",\"disposition\":\"include/exclude/uncertain 三選一\",\"rationale\":\"收錄判斷≤300字\",\"open_questions\":\"未確認事項≤300字\"}}。照原要求核對主導者、日期與範圍；頁未讀完不可宣称全信已核對。read_mail_notes 可先用 null 回查來源；沒有 pending 時只在修訂既有摘要時提供 mail_note。",self.context())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub fn fixture(op: &str, version: &str, offset: usize, end: usize) -> (Digest, Value) {
        let digest = Digest {
            source_operation: op.into(),
            summary: "已完成試驗；主導者尚待確認".into(),
            draft: "10/08 完成試驗".into(),
            disposition: Disposition::Include,
            rationale: "日期在要求範圍內".into(),
            open_questions: "主導者".into(),
        };
        let result = json!({"ok":true,"result":{"mail":{"mail_id":"m1","revision":version,"subject":"fixture","sent_at":"2026-10-08"},"offset":offset,"next_offset":end,"total_chars":100,"text":"信".repeat(end-offset)}});
        (digest, result)
    }
    #[test]
    fn summaries_survive_one_hundred_turns_and_partial_reads_remain_partial() {
        let mut state = State::default();
        let (digest, result) = fixture("op1", "v1", 0, 50);
        state.observe("op1", &result).unwrap();
        let id = state.pending.as_ref().unwrap().id();
        state
            .accept(digest, Page::from_result("op1", &result).unwrap())
            .unwrap();
        state.selected = vec![id.clone()];
        for _ in 0..120 {
            state = serde_json::from_value(serde_json::to_value(&state).unwrap()).unwrap();
            state.validate().unwrap();
            assert!(state.instructions().contains("10/08 完成試驗"));
            assert!(!state.notes[&id].complete());
        }
        let (digest, result) = fixture("op2", "v1", 50, 100);
        state.observe("op2", &result).unwrap();
        assert!(state.instructions().contains("previous_digest"));
        state
            .accept(digest, Page::from_result("op2", &result).unwrap())
            .unwrap();
        assert!(state.notes[&id].complete());
        let (digest, result) = fixture("op3", "v2", 0, 100);
        state.observe("op3", &result).unwrap();
        state
            .accept(digest, Page::from_result("op3", &result).unwrap())
            .unwrap();
        assert!(state.notes[&id].stale);
        assert_eq!(state.notes.len(), 2);
    }
    #[test]
    fn rejects_mismatched_source_missing_draft_and_corrupt_checkpoint() {
        let mut state = State::default();
        let (mut digest, result) = fixture("op1", "v1", 0, 50);
        state.observe("op1", &result).unwrap();
        digest.source_operation = "op2".into();
        assert!(state
            .accept(digest.clone(), Page::from_result("op1", &result).unwrap())
            .is_err());
        digest.source_operation = "op1".into();
        digest.draft.clear();
        assert!(state
            .accept(digest, Page::from_result("op1", &result).unwrap())
            .is_err());
        assert!(state.pending.is_some());
        let mut invalid = result;
        invalid["result"]["next_offset"] = json!(101);
        assert!(Page::from_result("op1", &invalid).is_err());
        state.selected = vec!["missing".into()];
        assert!(state.validate().is_err());
    }
}
