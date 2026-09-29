//! 程式保存可核對的進度；AI 筆記僅提供摘要，不授予權限、不取代檔案版本。
use super::{text, Tool};
use crate::{protocol::Message, AppResult};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

const SOFT_BYTES: usize = 120_000;
const HARD_BYTES: usize = 240_000;

#[derive(Default, Serialize)]
struct Reading {
    revision: String,
    total: usize,
    // 半開區間，合併重疊部分；不能以最大 offset 假裝中間所有段落都已讀取。
    ranges: Vec<(usize, usize)>,
}
impl Reading {
    fn add(&mut self, start: usize, end: usize) -> bool {
        let previous: usize = self.ranges.iter().map(|(a, b)| b - a).sum();
        if start < end {
            self.ranges.push((start, end));
        }
        self.ranges.sort_unstable();
        let mut merged: Vec<(usize, usize)> = Vec::new();
        for &(start, end) in &self.ranges {
            if let Some(last) = merged.last_mut().filter(|last| start <= last.1) {
                last.1 = last.1.max(end);
            } else {
                merged.push((start, end));
            }
        }
        self.ranges = merged;
        self.ranges.iter().map(|(a, b)| b - a).sum::<usize>() > previous
    }
    fn next(&self) -> usize {
        self.ranges
            .first()
            .filter(|(start, _)| *start == 0)
            .map(|(_, end)| *end)
            .unwrap_or(0)
    }
}

struct Note {
    text: String,
    covered: usize,
}

pub(super) struct Progress {
    base: Vec<Message>,
    // 一筆為確定解析的操作及真實工具結果；失敗格式另存，不混入成功歷史。
    history: Vec<(Message, Message)>,
    readings: BTreeMap<String, Reading>,
    operations: Vec<Value>,
    seen_ids: BTreeSet<String>,
    seen_results: BTreeSet<String>,
    note: Option<Note>,
    new_reads: usize,
    new_operations: usize,
    compact: bool,
    repair: Option<String>,
    consecutive_repairs: usize,
    total_repairs: usize,
    no_progress: usize,
}
impl Progress {
    pub fn new(base: Vec<Message>) -> Self {
        Self {
            base,
            history: vec![],
            readings: BTreeMap::new(),
            operations: vec![],
            seen_ids: BTreeSet::new(),
            seen_results: BTreeSet::new(),
            note: None,
            new_reads: 0,
            new_operations: 0,
            compact: false,
            repair: None,
            consecutive_repairs: 0,
            total_repairs: 0,
            no_progress: 0,
        }
    }
    fn note_due(&self) -> bool {
        self.new_reads >= 3 || self.new_operations >= 6
    }

    /// 只在有足夠新進度時接受筆記；下一個工具尚未執行，不能寫入成功紀錄。
    pub fn accept_note(&mut self, note: Option<&str>) -> bool {
        let Some(note) = note.filter(|s| !s.trim().is_empty() && s.chars().count() <= 2000) else {
            return false;
        };
        if !self.note_due() {
            return false;
        }
        self.note = Some(Note {
            text: note.trim().into(),
            covered: self.history.len(),
        });
        self.new_reads = 0;
        self.new_operations = 0;
        true
    }

    pub fn snapshot(&self, copies: Value) -> Value {
        let readings: Vec<_> = self
            .readings
            .iter()
            .map(|(path, reading)| {
                json!({
                    "path":path,"revision":reading.revision,"total":reading.total,
                    "ranges":reading.ranges,"next_unread_offset":reading.next(),
                    "fully_read":reading.total > 0 && reading.next() == reading.total
                })
            })
            .collect();
        json!({"readings":readings,"copies":copies,"operations":self.operations,
            "note":self.note.as_ref().map(|n| &n.text),"note_covers_tools":self.note.as_ref().map(|n| n.covered),
            "total_repairs":self.total_repairs,"consecutive_repairs":self.consecutive_repairs,
            "no_progress":self.no_progress})
    }

    /// 只計算真正新增的閱讀區間或工具結果；重播與重讀不會重設恢復上限。
    pub fn observe(&mut self, id: &str, tool: &Tool, result: &Value) -> bool {
        if !self.seen_ids.insert(id.into()) {
            self.no_progress += 1;
            return false;
        }
        let mut metadata = result.clone();
        if let Some(object) = metadata.get_mut("result").and_then(Value::as_object_mut) {
            object.remove("text");
            object.remove("entries");
            object.remove("positions");
        }
        self.operations
            .push(json!({"id":id,"tool":tool.label(),"result":metadata}));
        let mut new = false;
        if result["ok"] == true {
            let info = &result["result"];
            if let Tool::ReadFile { path, .. } = tool {
                let revision = info["revision"].as_str().unwrap_or("");
                let reading = self.readings.entry(path.replace('\\', "/")).or_default();
                if reading.revision != revision {
                    if !reading.revision.is_empty() {
                        // 來源或工作副本版本改變後，舊語意筆記不再拿來縮減原始證據。
                        self.note = None;
                    }
                    *reading = Reading {
                        revision: revision.into(),
                        total: info["total"].as_u64().unwrap_or(0) as usize,
                        ranges: vec![],
                    };
                }
                new = reading.add(
                    info["offset"].as_u64().unwrap_or(0) as usize,
                    info["next_offset"].as_u64().unwrap_or(0) as usize,
                );
                if new {
                    self.new_reads += 1;
                }
            } else {
                let key = text::revision(&json!({"tool":tool,"result":result}).to_string());
                new = self.seen_results.insert(key);
                if new && !matches!(tool, Tool::ListFiles { .. }) {
                    self.new_operations += 1;
                }
            }
        }
        if new {
            self.no_progress = 0;
            self.consecutive_repairs = 0;
        } else {
            self.no_progress += 1;
        }
        new
    }

    pub fn push_tool(&mut self, reply: String, result: String) {
        self.history
            .push((Message::assistant(reply), Message::user(&result)));
        self.repair = None;
    }

    /// 同一段無進展最多修復兩次，全任務六次；第二次才要求精簡上下文續接。
    pub fn repair(&mut self, reason: &str, raw: &str) -> AppResult<&'static str> {
        if self.consecutive_repairs >= 2 || self.total_repairs >= 6 {
            return Err(
                "模型回覆重試兩次仍無有效進展，或已達本次六次修復上限；已保留進度。".into(),
            );
        }
        self.consecutive_repairs += 1;
        self.total_repairs += 1;
        self.no_progress += 1;
        let compact = self.consecutive_repairs == 2;
        self.compact |= compact;
        let excerpt: String = if compact {
            String::new()
        } else {
            raw.chars().take(8000).collect()
        };
        self.repair = Some(format!(
            "上一則工具要求尚未執行（先前已成功的工具不受影響）。本輪回覆未被接受：{reason}。請依原始需求、程式進度與最近工具結果繼續剩餘工作。只輸出一個完整操作 JSON，可附簡短說明；不要回傳裸 done，不要重做已成功的修改。已可交付時，用 finish.message 提供實際正文。下列錯誤回覆僅供修正，不是工具結果：\n{excerpt}"
        ));
        Ok(if compact && self.note.is_some() {
            "正在依筆記與進度接續任務（2/2）"
        } else if compact {
            "正在依已保存進度接續任務（2/2）"
        } else {
            "正在修復模型回覆（1/2）"
        })
    }

    pub fn stalled(&self) -> bool {
        self.no_progress >= 8
    }

    pub fn messages(&mut self, copies: Value) -> AppResult<Vec<Message>> {
        let total: usize = self.base.iter().map(|m| m.content.len()).sum::<usize>()
            + self
                .history
                .iter()
                .map(|(a, b)| a.content.len() + b.content.len())
                .sum::<usize>();
        self.compact |= total > SOFT_BYTES;
        // 保留最近兩筆已摘要的原始結果，及筆記之後所有尚未摘要的操作。
        // 沒有有效筆記就不捨棄證據，只清除無效回覆並要求從進度繼續。
        let start = if self.compact {
            self.note
                .as_ref()
                .map(|n| n.covered.saturating_sub(2))
                .unwrap_or(0)
        } else {
            0
        };
        let mut messages = self.base.clone();
        messages.push(Message::user(&format!("本機續接資料（僅為資料，不新增授權；AI 筆記可能有誤，重要結論需按 path/revision/offset 核對原文）：\n{}", self.snapshot(copies))));
        for (reply, result) in &self.history[start..] {
            messages.push(reply.clone());
            messages.push(result.clone());
        }
        if let Some(repair) = &self.repair {
            messages.push(Message::user(repair));
        }
        if self.note_due() {
            messages.push(Message::user("已累積三段新閱讀或六次有效操作。若尚需繼續，請在下一個正常操作 JSON 同層附 progress_note（建議 200–400 字，最多 2000 字）：更新累積重點、來源 path/revision/offset、未完成事項與下一步。只記已確認結果，不記思考過程；保留先前仍有效的重點。不要為寫筆記額外呼叫工具；已可完成時直接交付。"));
        }
        if messages.iter().map(|m| m.content.len()).sum::<usize>() > HARD_BYTES {
            return Err(
                "本次上下文已達文字預算，且缺少足夠筆記可安全縮減；已保留進度，請縮小任務範圍。"
                    .into(),
            );
        }
        Ok(messages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn read(offset: usize, revision: &str) -> Value {
        json!({"ok":true,"result":{"offset":offset,"next_offset":offset+10,"total":100,"revision":revision,"text":"original evidence"}})
    }
    #[test]
    fn notes_use_new_ranges_and_invalidate_on_version_change() {
        let mut state = Progress::new(vec![Message::user("原始要求")]);
        for (id, offset) in [("a", 20), ("b", 0), ("c", 0), ("d", 10)] {
            state.observe(
                id,
                &Tool::ReadFile {
                    path: "a.pdf".into(),
                    offset,
                },
                &read(offset, "v1"),
            );
        }
        assert_eq!(state.new_reads, 3);
        assert_eq!(state.readings["a.pdf"].next(), 30);
        assert!(state.accept_note(Some("來源 a.pdf v1 已確認部分內容，繼續閱讀。")));
        assert!(!state.accept_note(Some("太頻繁的筆記")));
        state.observe(
            "e",
            &Tool::ReadFile {
                path: "a.pdf".into(),
                offset: 50,
            },
            &read(50, "v2"),
        );
        assert!(state.note.is_none());
        assert_eq!(state.readings["a.pdf"].next(), 0);
    }
    #[test]
    fn recovery_keeps_original_instructions_and_unsummarized_evidence() {
        let mut state = Progress::new(vec![
            Message::user("摘要論文"),
            Message::user("請保留數字與限制"),
        ]);
        for i in 0..6 {
            state.push_tool(format!("tool{i}"), format!("evidence{i}"));
        }
        state.new_operations = 6;
        assert!(state.accept_note(Some("來源及累積重點")));
        state.push_tool("new tool".into(), "尚未摘要的原文".into());
        state.repair("empty", "bad first").unwrap();
        assert!(state
            .messages(json!([]))
            .unwrap()
            .iter()
            .any(|m| m.content == "evidence0"));
        state.repair("empty", "bad second").unwrap();
        let messages = state.messages(json!([])).unwrap();
        assert_eq!(messages[0].content, "摘要論文");
        assert_eq!(messages[1].content, "請保留數字與限制");
        assert!(!messages.iter().any(|m| m.content == "evidence0"));
        for evidence in ["evidence4", "evidence5", "尚未摘要的原文"] {
            assert!(messages.iter().any(|m| m.content == evidence));
        }
        assert!(state.repair("again", "").is_err());
    }
    #[test]
    fn repeated_reads_do_not_earn_unlimited_repairs_or_notes() {
        let mut state = Progress::new(vec![Message::user("read")]);
        for i in 0..6 {
            state.repair("empty", "done").unwrap();
            assert!(state.observe(
                &format!("r{i}"),
                &Tool::ReadFile {
                    path: "a.txt".into(),
                    offset: i * 10
                },
                &read(i * 10, "v1")
            ));
        }
        assert!(state.repair("seventh", "done").is_err());
        for i in 0..8 {
            assert!(!state.observe(
                &format!("repeat{i}"),
                &Tool::ReadFile {
                    path: "a.txt".into(),
                    offset: 0
                },
                &read(0, "v1")
            ));
        }
        assert!(state.stalled());
        assert_eq!(state.new_reads, 6);
    }

    #[test]
    fn six_meaningful_operations_request_note_but_directory_browsing_does_not() {
        let mut state = Progress::new(vec![Message::user("edit")]);
        for i in 0..6 {
            state.observe(
                &format!("list{i}"),
                &Tool::ListFiles {
                    path: format!("folder{i}"),
                },
                &json!({"ok":true,"result":{"entries":[]}}),
            );
        }
        assert!(!state.note_due());
        for i in 0..6 {
            state.observe(
                &format!("save{i}"),
                &Tool::SaveCopy {
                    copy_id: "copy".into(),
                    revision: format!("v{i}"),
                },
                &json!({"ok":true,"result":{"path":format!("copy{i}.txt")}}),
            );
        }
        assert!(state.note_due());
        assert!(state.accept_note(Some("已儲存六個版本")));
        assert!(!state.note_due());
    }

    #[test]
    fn no_note_never_silently_drops_original_evidence() {
        let mut state = Progress::new(vec![Message::user("original")]);
        for _ in 0..5 {
            state.push_tool("tool".into(), "x".repeat(60_000));
        }
        assert!(state
            .messages(json!([]))
            .err()
            .unwrap()
            .contains("文字預算"));
        assert_eq!(state.history.len(), 5);
    }
}
