//! 程式保存可核對的進度；AI 筆記僅提供摘要，不授予權限、不取代檔案版本。
use super::agent::Message;
use super::{text, Tool};
use crate::AppResult;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

const SOFT_BYTES: usize = 120_000;
const HARD_BYTES: usize = 240_000;
const COMPACT_BYTES: usize = HARD_BYTES * 65 / 100;

/// 交接資料有界且必須包含下一步；摘要不改寫使用者授權。
pub(super) fn validate_handoff(note: &str, superseded: &[String], next: &str) -> AppResult<()> {
    if note.trim().is_empty()
        || note.chars().count() > 2000
        || next.trim().is_empty()
        || next.chars().count() > 500
        || superseded.len() > 12
        || superseded
            .iter()
            .any(|s| s.trim().is_empty() || s.chars().count() > 200)
    {
        return Err(
            "交接需包含 1–2000 字工作筆記、1–500 字下一步，及最多 12 條各 200 字的失效結論。"
                .into(),
        );
    }
    Ok(())
}

#[derive(Default, Serialize, Deserialize)]
struct Reading {
    revision: String,
    total: usize,
    // 半開區間，合併重疊部分；不能以最大 offset 假裝中間所有段落都已讀取。
    ranges: Vec<(usize, usize)>,
    /// 同一文件版本的有效閱讀次數；完整覆蓋全文後歸零。
    read_count: usize,
    /// 可選技能每份文件只提示一次，未採用也不追問。
    note_offered: bool,
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

#[derive(Serialize, Deserialize)]
struct Note {
    text: String,
    covered: usize,
}

#[derive(Serialize, Deserialize)]
pub(super) struct Progress {
    /// None 為已保存的舊文字協定，新任務必須具有原生能力快照。
    #[serde(default)]
    pub agent: Option<super::agent::State>,
    base: Vec<Message>,
    #[serde(default)]
    base_task_ids: Vec<Option<String>>,
    /// 使用者原文獨立保存，精簡工具歷史時不得刪除或交給 AI 改寫。
    #[serde(default)]
    instructions: Vec<super::steering::Instruction>,
    // 一筆為確定解析的操作及真實工具結果；失敗格式另存，不混入成功歷史。
    history: Vec<(Message, Message)>,
    #[serde(default)]
    history_ids: Vec<Option<String>>,
    #[serde(skip)]
    pending_history_id: Option<String>,
    readings: BTreeMap<String, Reading>,
    operations: Vec<Value>,
    #[serde(default)]
    tool_usage: BTreeMap<String, usize>,
    #[serde(default)]
    checkpoints: usize,
    seen_ids: BTreeSet<String>,
    seen_results: BTreeSet<String>,
    note: Option<Note>,
    last_read: Option<String>,
    compact: bool,
    #[serde(default)]
    lean_context: bool,
    #[serde(default)]
    recovery_context: bool,
    #[serde(default)]
    instruction_review_required: bool,
    #[serde(default)]
    superseded: Vec<String>,
    #[serde(default)]
    next_step: String,
    repair: Option<String>,
    consecutive_repairs: usize,
    total_repairs: usize,
    no_progress: usize,
}
impl Progress {
    pub fn new(base: Vec<Message>) -> Self {
        Self {
            agent: None,
            base,
            base_task_ids: vec![],
            instructions: vec![],
            history: vec![],
            history_ids: vec![],
            pending_history_id: None,
            readings: BTreeMap::new(),
            operations: vec![],
            tool_usage: BTreeMap::new(),
            checkpoints: 0,
            seen_ids: BTreeSet::new(),
            seen_results: BTreeSet::new(),
            note: None,
            last_read: None,
            compact: false,
            lean_context: false,
            recovery_context: false,
            instruction_review_required: false,
            superseded: vec![],
            next_step: String::new(),
            repair: None,
            consecutive_repairs: 0,
            total_repairs: 0,
            no_progress: 0,
        }
    }
    pub fn task_references(&mut self, ids: Vec<Option<String>>) {
        self.base_task_ids = ids;
    }
    /// 使用者主動續接才重設每段計數；去重紀錄仍由 broker 保留。
    pub fn add_instructions(&mut self, entries: Vec<super::steering::Instruction>) {
        for entry in entries {
            if entry.status != "withdrawn" && !self.instructions.iter().any(|e| e.id == entry.id) {
                self.instructions.push(entry);
                self.instruction_review_required = true;
                self.no_progress = 0;
                self.repair = None;
            }
        }
    }

    /// 使用者主動續接才重設每段計數；去重紀錄仍由 broker 保留。
    pub fn resume_segment(&mut self) {
        self.total_repairs = 0;
        self.consecutive_repairs = 0;
        self.no_progress = 0;
        self.repair = None;
        self.compact_batch();
    }

    /// 自動換批只縮短工作上下文，不重設無進展／修復計數，避免無限循環。
    /// 呼叫前必須先由 broker 保存完整原文，筆記永遠不取代操作去重表。
    pub fn compact_batch(&mut self) {
        self.compact_keep(2);
    }

    /// 呼叫端需先封存 broker 原文；主動整理僅留下本次工具的一對訊息。
    pub fn compact_now(&mut self) {
        self.lean_context = true;
        self.compact_keep(1);
    }
    /// 新補充尚未經模型核對，不杜撰新筆記；舊筆記明示需要重新檢查。
    pub fn compact_for_instruction(&mut self) {
        self.lean_context = true;
        self.instruction_review_required = true;
        self.compact_keep(0);
    }
    pub fn handoff(&mut self, note: &str, superseded: &[String], next: &str) {
        self.note = Some(Note {
            text: note.trim().into(),
            covered: self.history.len(),
        });
        self.superseded = superseded.to_vec();
        self.next_step = next.trim().into();
        self.instruction_review_required = false;
        self.compact_now();
    }
    fn compact_keep(&mut self, keep: usize) {
        self.checkpoints += 1;
        self.compact = true;
        // 完整原始結果仍存在加密 broker 操作簿，可用 read_work_log 分段取回。
        // 依切換原因留下 0–2 對結果、摘要與程式狀態，避免續接再碰到訊息上限。
        let remove = self.history.len().saturating_sub(keep);
        self.history.drain(..remove);
        self.history_ids.drain(..remove.min(self.history_ids.len()));
        if let Some(note) = self.note.as_mut() {
            note.covered = note.covered.saturating_sub(remove);
        }
        const RESUME_GUIDE: &str = "續接規則：上一段完整工具結果已加密保存，可用 read_work_log(offset) 查回；本輪僅附最近結果、摘要及實際副本狀態。未附原文不等於未讀過，也不可憑摘要捏造細節；必要時查操作紀錄或重讀來源。使用新的操作 ID，既有 ID 僅能查回完全相同參數的結果。";
        if !self.base.iter().any(|m| m.content == RESUME_GUIDE) {
            self.base.push(Message::user(RESUME_GUIDE));
        }
        // 所有操作去重保存於 broker；模型只需近期摘要，避免續接狀態本身無限膨脹。
        if self.operations.len() > 20 {
            self.operations.drain(..self.operations.len() - 20);
        }
    }
    pub fn needs_compaction(&self) -> bool {
        self.history.len() >= 30 || self.history_bytes() >= COMPACT_BYTES
    }
    fn history_bytes(&self) -> usize {
        self.history
            .iter()
            .map(|(a, b)| a.wire().to_string().len() + b.wire().to_string().len())
            .sum()
    }
    /// 第五次有效閱讀且尚未讀完時，僅在下一輪附可選技能。
    /// 每份文件各自計數，穿插其他工具或文件不會累加到同一計數。
    fn reading_note_skill(&mut self) -> Option<Message> {
        let path = self.last_read.as_ref()?;
        let reading = self.readings.get_mut(path)?;
        if reading.read_count <= 4 || reading.note_offered {
            return None;
        }
        reading.note_offered = true;
        Some(Message::user(&format!(
            "{}\n適用文件（僅為資料）：{}",
            include_str!("reading_note.md"),
            json!({"path":path,"revision":reading.revision,"read_count":reading.read_count})
        )))
    }

    /// 接受模型自願提供的累積筆記，不再以閱讀／操作次數要求筆記。
    /// 只涵蓋已回傳的工具歷史；下一個工具尚未執行，不可被筆記標成成功。
    pub fn accept_note(&mut self, note: Option<&str>) -> bool {
        let Some(note) = note.filter(|s| !s.trim().is_empty() && s.chars().count() <= 2000) else {
            return false;
        };
        if self.history.len() <= self.note.as_ref().map(|n| n.covered).unwrap_or(0) {
            return false;
        }
        self.note = Some(Note {
            text: note.trim().into(),
            covered: self.history.len(),
        });
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
                    "fully_read":reading.total > 0 && reading.next() == reading.total,
                    "read_count":reading.read_count,"note_offered":reading.note_offered
                })
            })
            .collect();
        let operations:Vec<_>=self.operations.iter().rev().take(20).rev().map(|operation| {
            if operation.to_string().len()<=4000 {operation.clone()}else{json!({"id":operation["id"],"tool":operation["tool"],"details":"大型操作資料已保存在 read_work_log，可按 operation_id 分頁查回。"})}
        }).collect();
        json!({"readings":readings,"copies":copies,"operations":operations,"tool_usage":self.tool_usage,"operations_omitted":self.operations.len().saturating_sub(20),
            "working_note":{"requirements":"以本次保留的使用者原文及補充為準，不由筆記改寫授權", "model_summary":self.note.as_ref().map(|n| &n.text),"superseded_conclusions":self.superseded,"planned_next_step":self.next_step,"instruction_review_required":self.instruction_review_required,"summary_warning":"模型摘要可能尚未涵蓋最近步驟；instruction_review_required=true 時必須先依最新補充重新核對欄位與結論，不沿用被否定的圖表。完成狀態依 operations、copies 及工具原文核對","checkpoint_count":self.checkpoints,"next_step":"依原始要求、最近結果及摘要待辦繼續；缺少細節時先 read_work_log，不重做已成功修改"},
            "note":self.note.as_ref().map(|n| &n.text),"note_covers_tools":self.note.as_ref().map(|n| n.covered),
            "total_repairs":self.total_repairs,"consecutive_repairs":self.consecutive_repairs,
            "no_progress":self.no_progress})
    }

    /// 只計算真正新增的閱讀區間或工具結果；重播與重讀不會重設恢復上限。
    pub fn observe(&mut self, id: &str, tool: &Tool, result: &Value) -> bool {
        self.pending_history_id = Some(id.into());
        self.last_read = None;
        if !self.seen_ids.insert(id.into()) {
            self.no_progress += 1;
            return false;
        }
        *self.tool_usage.entry(tool.label().into()).or_default() += 1;
        let mut metadata = result.clone();
        if let Some(object) = metadata.get_mut("result").and_then(Value::as_object_mut) {
            object.remove("text");
            object.remove("entries");
            object.remove("positions");
            object.remove("document");
            object.remove("blocks_tail");
            // Excel 分批正文只保留在成對工具歷史，進度摘要不要再複製整批資料。
            object.remove("rows");
            object.remove("headers");
            object.remove("sheets");
            object.remove("lines");
            object.remove("matches");
            object.remove("head");
            object.remove("tail");
        }
        self.operations
            .push(json!({"id":id,"tool":tool.label(),"result":metadata}));
        let mut new = false;
        // 使用者拒絕的工具雖正常返回，仍沒有執行；換參數不能製造假進度。
        if result["ok"] == true
            && result["result"]["executed"] != false
            && !matches!(tool, Tool::CompactContext { .. })
        {
            let info = &result["result"];
            if let Tool::ReadFile { path, .. } | Tool::ReadDocumentSection { path, .. } = tool {
                let revision = info["revision"].as_str().unwrap_or("");
                let key = path.replace('\\', "/").to_lowercase();
                let reading = self.readings.entry(key.clone()).or_default();
                if reading.revision != revision {
                    if !reading.revision.is_empty() {
                        // 來源或工作副本版本改變後，舊語意筆記不再拿來縮減原始證據。
                        self.note = None;
                    }
                    *reading = Reading {
                        revision: revision.into(),
                        total: info["total"].as_u64().unwrap_or(0) as usize,
                        ..Reading::default()
                    };
                }
                new = reading.add(
                    info["offset"].as_u64().unwrap_or(0) as usize,
                    info["next_offset"].as_u64().unwrap_or(0) as usize,
                );
                if reading.next() == reading.total {
                    // 依區間聯集確認全文已讀，不能僅因讀到最後一段就歸零。
                    reading.read_count = 0;
                    reading.note_offered = false;
                } else if new {
                    reading.read_count += 1;
                    self.last_read = Some(key);
                }
            } else {
                let key = text::revision(&json!({"tool":tool,"result":result}).to_string());
                new = self.seen_results.insert(key);
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
        self.history_ids.resize(self.history.len(), None);
        self.history_ids.push(self.pending_history_id.take());
        self.history
            .push((Message::assistant(reply), Message::user(&result)));
        self.repair = None;
    }

    /// 原生 assistant/tool 必須一起保存及縮減，不把工具結果降為 user。
    pub fn push_native(&mut self, reply: Message, id: &str, result: &Value) {
        self.history_ids.resize(self.history.len(), None);
        self.history_ids.push(self.pending_history_id.take());
        self.history.push((reply, Message::result(id, result)));
        self.repair = None;
    }

    /// 一般修復兩次後，先由 runner 封存原文，再以精簡上下文追加兩次修復。
    /// 累計三十次避免長任務因零星格式錯誤過早停止；有效進展只重設連續計數。
    pub fn needs_recovery(&self) -> bool {
        self.consecutive_repairs == 2 && self.total_repairs < 30
    }

    /// 呼叫前必須 archive_results 成功；只縮短送給模型的投影，去重及權限不变。
    pub fn recover_context(&mut self) {
        self.compact_now();
        self.recovery_context = true;
    }

    pub fn repair(&mut self, reason: &str, raw: &str) -> AppResult<&'static str> {
        if self.consecutive_repairs >= 4 || self.total_repairs >= 30 {
            return Err(
                "模型回覆經兩次一般修復及兩次精簡恢復仍無有效進展，或已達本段累計 30 次修復上限；已保留進度。".into(),
            );
        }
        self.consecutive_repairs += 1;
        self.total_repairs += 1;
        self.no_progress += 1;
        let compact = self.consecutive_repairs >= 2;
        self.compact |= compact;
        let excerpt: String = if compact {
            String::new()
        } else {
            raw.chars().take(8000).collect()
        };
        self.repair = Some(format!(
            "上一則工具要求尚未執行（先前已成功的工具不受影響）。本輪回覆未被接受：{reason}。請依原始需求、程式進度與最近工具結果繼續剩餘工作。只輸出一個含單一 tool_calls 的完整 JSON，content 可放簡短說明；function.arguments 放工具參數，保留本次 id。不要回傳裸 done，不要重做已成功的修改。已可交付時，呼叫 finish 並在 arguments.message 提供實際正文。下列錯誤回覆僅供修正，不是工具結果：\n{excerpt}"
        ));
        if self.agent.is_some() {
            self.repair = Some(format!("上一則工具要求尚未執行或未通過交付檢查：{reason}。依原始需求與真實工具結果繼續，透過 API 呼叫下一個工具，不在正文拼接 JSON。已可交付時呼叫 finish 並提供實際正文；不要重做成功的修改。"));
        }
        Ok(if self.consecutive_repairs == 3 {
            "已暫時移出較早上下文，依目標、目前步驟與真實狀態恢復（1/2）"
        } else if self.consecutive_repairs == 4 {
            "正在以精簡上下文進行最後一次恢復（2/2）"
        } else if compact && self.note.is_some() {
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
                .map(|(a, b)| a.wire().to_string().len() + b.wire().to_string().len())
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
        if self.recovery_context {
            // 使用者原文、系統限制與最新補充仍保留；移除較早助理答案的送出投影。
            // 原文保存在歷史／checkpoint，必要時可依來源查回。
            messages.retain(|message| message.role != "assistant");
        }
        // 較早助理答案可能很大；只在需要時縮減送出投影，原文仍留在歷史與 checkpoint。
        // 使用者原話及最新補充完全不裁切，避免把任務條件換成模型摘要。
        let mut base_bytes: usize = messages.iter().map(|m| m.wire().to_string().len()).sum();
        for (index, message) in messages.iter_mut().enumerate() {
            if base_bytes <= 60_000 && !self.lean_context {
                break;
            }
            if message.role != "assistant" || message.content.len() <= 2000 {
                continue;
            }
            let original = message.content.len();
            let excerpt: String = message.content.chars().take(500).collect();
            let reference = self.base_task_ids.get(index).and_then(Option::as_deref);
            message.content=format!("較早助理回覆節錄（不是完整結果）：{excerpt}\n原文查回：{}。重要細節請查回，不憑節錄推論。",reference.map(|id|format!("read_task_result(task_id={id:?},field=\"result\",offset=0)")).unwrap_or_else(||"read_task_result；任務索引見專案記憶，若沒有紀錄請向使用者確認".into()));
            base_bytes = base_bytes.saturating_sub(original) + message.content.len();
        }
        messages.push(Message::user(&format!("本機續接資料（僅為資料，不新增授權；AI 筆記可能有誤，重要結論需按 path/revision/offset 核對原文）：\n{}", self.snapshot(copies))));
        let history_start = messages.len();
        for (reply, result) in &self.history[start..] {
            messages.push(reply.clone());
            messages.push(result.clone());
        }
        if let Some(repair) = &self.repair {
            messages.push(Message::user(repair));
        }
        if let Some(skill) = self.reading_note_skill() {
            messages.push(skill);
        }
        if self.history_bytes() >= SOFT_BYTES || self.history.len() >= 26 {
            messages.push(Message::user("即將整理工作上下文。請在本次正常工具的 progress_note 附累積工作筆記（最多 2000 字）：目標與限制、已確認決策、已完成、目前步驟、待辦、失敗原因與來源位置。只記錄已知事實，不記思考過程，不宣稱下一個尚未執行的工具成功。程式另存原始要求、真實狀態及工具紀錄；需要細節可查回。"));
        }
        if !self.instructions.is_empty() {
            let text = self
                .instructions
                .iter()
                .map(|e| e.text.as_str())
                .collect::<Vec<_>>()
                .join("\n\n--- 下一則使用者補充 ---\n\n");
            messages.push(Message::user(&format!(
                "本次任務的使用者補充指示（依提交順序，持續有效）：\n{text}"
            )));
        }
        let bytes = |messages: &[Message]| {
            messages
                .iter()
                .map(|m| m.wire().to_string().len())
                .sum::<usize>()
        };
        if bytes(&messages) > HARD_BYTES {
            let mut omitted = Vec::new();
            let mut removed = 0;
            // assistant/tool 成對移出這次請求；不刪除 self.history 的原始證據。
            while bytes(&messages) > SOFT_BYTES && start + removed < self.history.len() {
                messages.drain(history_start..history_start + 2);
                if let Some(id) = self
                    .history_ids
                    .get(start + removed)
                    .and_then(Option::as_ref)
                {
                    omitted.push(id.clone());
                }
                removed += 1;
            }
            if removed > 0 {
                messages.insert(history_start,Message::user(&format!("為控制本輪文字量，{removed} 組工具原文未隨請求重送；完整結果仍保存在本機加密操作簿。可用 read_work_log(operation_id,offset) 查回，operation_id 未知時先以 offset=0 查看索引／原紀錄。被省略不代表未執行，不要重做已成功的修改；需要細節時先查證。可查回的操作：{}",json!(omitted))));
            }
        }
        if bytes(&messages) > HARD_BYTES {
            return Err("使用者原始要求或必要狀態本身已超過本機文字預算，無法藉由繼續減少。請縮短新提問或另開專案對話；目前進度已保留。".into());
        }
        Ok(messages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_handoff_survives_checkpoint_without_raw_history_or_counter_reset() {
        let mut state = Progress::new(vec![Message::user("原始要求：保持來源唯讀")]);
        for i in 0..6 {
            state.push_tool(format!("tool{i}"), "大量原始資料".repeat(1000));
        }
        state.total_repairs = 3;
        state.no_progress = 5;
        let request = Tool::CompactContext {
            working_note: "正確欄位 F；資料集 data.csv 100筆".into(),
            superseded: vec!["C 欄錯誤".into()],
            next_step: "由 F 重畫".into(),
        };
        state.observe("compact", &request, &json!({"ok":true,"result":{}}));
        state.push_tool("compact_context".into(), "已封存".into());
        state.handoff(
            "正確欄位 F；資料集 data.csv 100筆",
            &["C 欄錯誤".into()],
            "由 F 重畫",
        );
        let encoded = serde_json::to_vec(&state).unwrap();
        let mut restored: Progress = serde_json::from_slice(&encoded).unwrap();
        let messages = restored.messages(json!([])).unwrap();
        let wire = serde_json::to_string(&messages).unwrap();
        assert!(!wire.contains("大量原始資料"));
        assert!(wire.contains("保持來源唯讀"));
        assert!(wire.contains("C 欄錯誤"));
        assert_eq!(restored.total_repairs, 3);
        assert_eq!(restored.no_progress, 6);
        assert!(restored.seen_ids.contains("compact"));
        assert!(validate_handoff("", &[], "下一步").is_err());
    }
    #[test]
    fn correction_retains_verbatim_instruction_and_marks_old_note_for_review() {
        let mut state = Progress::new(vec![Message::user("原始要求")]);
        state.push_tool("read".into(), "舊大量數值".repeat(2000));
        state.accept_note(Some("用 C 欄"));
        state.add_instructions(vec![super::super::steering::Instruction {
            id: "i".into(),
            text: "改用 F，原件唯讀".into(),
            status: "staged".into(),
        }]);
        state.compact_for_instruction();
        let messages = serde_json::to_string(&state.messages(json!([])).unwrap()).unwrap();
        assert!(!messages.contains("舊大量數值"));
        assert!(messages.contains("改用 F，原件唯讀"));
        assert_eq!(
            state.snapshot(json!([]))["working_note"]["instruction_review_required"],
            true
        );
    }
    #[test]
    fn declined_tools_cannot_keep_a_long_task_alive_by_changing_parameters() {
        let mut state = Progress::new(vec![Message::user("整理週報")]);
        for i in 0..8 {
            assert!(!state.observe(
                &format!("denied{i}"),
                &Tool::OutlookFolders {
                    scope: "local_inbox".into(),
                    parent_id: Some(format!("folder{i}")),
                    offset: 0
                },
                &json!({"ok":true,"result":{"declined":true,"executed":false}})
            ));
        }
        assert!(state.stalled());
        state.compact_batch();
        assert!(state.stalled(), "換批不能清除無進展保護");
    }
    #[test]
    fn automatic_compaction_keeps_notes_requirements_and_safety_counters() {
        let mut state = Progress::new(vec![Message::user("目標：整理週報；不能修改來源")]);
        for i in 0..30 {
            state.push_tool(format!("call{i}"), "證據".repeat(2000));
        }
        assert!(state.accept_note(Some("已確認前三十份證據；待辦：核對缺漏並寫週報")));
        state.total_repairs = 5;
        state.no_progress = 7;
        assert!(state.needs_compaction());
        state.compact_batch();
        assert_eq!(state.history.len(), 2);
        assert_eq!(state.total_repairs, 5);
        assert_eq!(state.no_progress, 7);
        assert!(state.snapshot(json!([]))["working_note"]["model_summary"]
            .as_str()
            .unwrap()
            .contains("待辦"));
        assert!(state
            .messages(json!([]))
            .unwrap()
            .iter()
            .any(|m| m.content.contains("不能修改來源")));
    }
    #[test]
    fn user_supplements_survive_compaction_and_serialized_resume() {
        let mut progress = Progress::new(vec![Message::user("原始目標：分析 12:25 到 12:33")]);
        progress.add_instructions(vec![super::super::steering::Instruction {
            id: "first".into(),
            text: "只看 Z01-CY，保留原始時間".into(),
            status: "staged".into(),
        }]);
        for i in 0..6 {
            progress.push_tool(format!("call{i}"), "舊結果".into());
        }
        progress.resume_segment();
        let mut restored: Progress =
            serde_json::from_slice(&serde_json::to_vec(&progress).unwrap()).unwrap();
        let messages = restored.messages(json!([])).unwrap();
        assert!(messages.iter().any(|m| m.content.contains("原始目標")));
        assert_eq!(
            messages
                .iter()
                .filter(|m| m.content.contains("只看 Z01-CY"))
                .count(),
            1
        );
    }
    fn read(offset: usize, revision: &str) -> Value {
        json!({"ok":true,"result":{"offset":offset,"next_offset":offset+10,"total":100,"revision":revision,"text":"original evidence"}})
    }
    #[test]
    fn explicit_resume_keeps_state_but_bounds_model_history_without_a_note() {
        let mut state = Progress::new(vec![Message::user("原始要求")]);
        for i in 0..60 {
            state.push_tool(format!("call{i}"), format!("evidence{i}"));
        }
        state.resume_segment();
        let messages = state.messages(json!([])).unwrap();
        assert!(messages.len() < 10);
        assert!(messages.iter().any(|m| m.content.contains("read_work_log")));
        assert!(messages.iter().any(|m| m.content == "evidence59"));
        assert!(messages.iter().any(|m| m.content == "原始要求"));
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
        assert_eq!(state.readings["a.pdf"].read_count, 3);
        state.push_tool("已執行閱讀".into(), "原文".into());
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
        assert!(state.needs_recovery());
        state.recover_context();
        assert!(state.repair("again", "").is_ok());
        let messages = state.messages(json!([])).unwrap();
        assert!(!messages.iter().any(|m| m.content == "evidence4"));
        assert!(messages.iter().any(|m| m.content == "尚未摘要的原文"));
        assert!(state.repair("last", "").is_ok());
        assert!(state.repair("stop", "").is_err());
    }
    #[test]
    fn repeated_reads_do_not_earn_unlimited_repairs_or_notes() {
        let mut state = Progress::new(vec![Message::user("read")]);
        for i in 0..30 {
            let mut result = read(i * 10, "v1");
            result["result"]["total"] = json!(1000);
            state.repair("empty", "done").unwrap();
            assert!(state.observe(
                &format!("r{i}"),
                &Tool::ReadFile {
                    path: "a.txt".into(),
                    offset: i * 10
                },
                &result
            ));
        }
        assert!(state.repair("thirty-first", "done").is_err());
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
        assert_eq!(state.readings["a.txt"].read_count, 30);
    }

    #[test]
    fn tools_and_browsing_never_request_notes() {
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
        assert!(state.reading_note_skill().is_none());
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
        assert!(state.reading_note_skill().is_none());
        assert!(!state
            .messages(json!([]))
            .unwrap()
            .iter()
            .any(|m| m.content.contains("可選技能：長文件閱讀筆記")));
        assert!(state.reading_note_skill().is_none());
    }

    fn observe_read(state: &mut Progress, path: &str, offset: usize, total: usize, revision: &str) {
        let id = format!("{path}_{revision}_{offset}");
        state.observe(&id, &Tool::ReadFile {path:path.into(),offset},
            &json!({"ok":true,"result":{"offset":offset,"next_offset":offset+10,"total":total,"revision":revision}}));
        state.push_tool(id, "原文".into());
    }
    fn has_optional_skill(state: &mut Progress) -> bool {
        state
            .messages(json!([]))
            .unwrap()
            .iter()
            .any(|m| m.content.starts_with("可選技能：長文件閱讀筆記"))
    }
    #[test]
    fn fifth_incomplete_read_offers_once_and_ignoring_it_does_not_interrupt() {
        let mut state = Progress::new(vec![Message::user("讀取")]);
        for offset in [0, 10, 20, 30] {
            observe_read(&mut state, "paper.pdf", offset, 100, "v1");
            assert!(!has_optional_skill(&mut state));
        }
        observe_read(&mut state, "paper.pdf", 40, 100, "v1");
        assert!(has_optional_skill(&mut state));
        assert!(!has_optional_skill(&mut state));
        for offset in [50, 60, 70, 80, 90] {
            observe_read(&mut state, "paper.pdf", offset, 100, "v1");
            assert!(!has_optional_skill(&mut state));
        }
        assert_eq!(state.readings["paper.pdf"].read_count, 0);
        assert!(!state.stalled());
        assert_eq!(state.total_repairs, 0);
        assert_eq!(state.history.len(), 10);
        assert!(state.note.is_none());
    }
    #[test]
    fn files_have_independent_counts_and_finishing_the_fifth_read_needs_no_hint() {
        let mut state = Progress::new(vec![Message::user("讀取兩份")]);
        for offset in [0, 10, 20, 30] {
            observe_read(&mut state, "a.pdf", offset, 50, "v1");
            observe_read(&mut state, "b.pdf", offset, 60, "v1");
            assert!(!has_optional_skill(&mut state));
        }
        observe_read(&mut state, "a.pdf", 40, 50, "v1");
        assert!(!has_optional_skill(&mut state));
        assert_eq!(state.readings["a.pdf"].read_count, 0);
        assert_eq!(state.readings["b.pdf"].read_count, 4);
        observe_read(&mut state, "b.pdf", 40, 60, "v1");
        assert!(has_optional_skill(&mut state));
        observe_read(&mut state, "b.pdf", 50, 60, "v1");
        assert_eq!(state.readings["b.pdf"].read_count, 0);
    }
    #[test]
    fn section_reads_share_file_counts_but_replay_failure_and_reread_do_not() {
        let mut state = Progress::new(vec![]);
        for offset in [0, 10, 20, 30] {
            observe_read(&mut state, "Folder/A.PDF", offset, 100, "v1");
        }
        let tool = Tool::ReadDocumentSection {
            path: "folder/a.pdf".into(),
            revision: "v1".into(),
            section_id: "s5".into(),
        };
        state.observe("failure", &tool, &json!({"ok":false}));
        assert!(!has_optional_skill(&mut state));
        state.observe("reread", &tool, &read(0, "v1"));
        assert!(!has_optional_skill(&mut state));
        state.observe("section", &tool, &read(40, "v1"));
        assert!(has_optional_skill(&mut state));
        state.observe("section", &tool, &read(40, "v1"));
        assert!(!has_optional_skill(&mut state));
        assert_eq!(state.readings["folder/a.pdf"].read_count, 5);
    }
    #[test]
    fn reading_the_end_with_gaps_does_not_reset_and_new_versions_start_over() {
        let mut state = Progress::new(vec![]);
        for offset in [0, 20, 30, 40, 90] {
            observe_read(&mut state, "a.txt", offset, 100, "v1");
        }
        assert_eq!(state.readings["a.txt"].read_count, 5);
        assert!(has_optional_skill(&mut state));
        assert!(state.accept_note(Some("已讀部分，仍有缺段")));
        observe_read(&mut state, "a.txt", 0, 100, "v2");
        assert_eq!(state.readings["a.txt"].read_count, 1);
        assert!(state.note.is_none());
        assert!(!has_optional_skill(&mut state));
        for offset in [10, 20, 30, 40] {
            observe_read(&mut state, "a.txt", offset, 100, "v2");
        }
        assert!(has_optional_skill(&mut state));
    }
    #[test]
    fn missing_tool_uses_existing_bounded_repair_without_recording_an_operation() {
        let mut state = Progress::new(vec![Message::user("保存文件摘要")]);
        let raw = r#"{"action":"tool","operation_id":"note_001","request":{"summary":"摘要"}}"#;
        for attempt in 0..5 {
            let super::super::reply::ParseOutcome::Repair(reason) =
                super::super::reply::parse(raw).unwrap()
            else {
                panic!("缺少工具名稱不得產生可執行操作");
            };
            let repaired = state.repair(reason, raw);
            if attempt < 4 {
                assert!(repaired.is_ok());
                let messages = state.messages(json!([])).unwrap();
                assert!(messages
                    .last()
                    .unwrap()
                    .content
                    .contains("缺少必要欄位 request.tool"));
                assert!(state.history.is_empty());
                assert!(state.operations.is_empty());
            } else {
                assert!(repaired.is_err(), "缺欄位也不得無限重試");
            }
        }
    }
    #[test]
    fn oversized_results_are_referenced_without_destroying_original_evidence() {
        let mut state = Progress::new(vec![Message::user("original")]);
        for _ in 0..5 {
            state.push_tool("tool".into(), "x".repeat(60_000));
        }
        let messages = state.messages(json!([])).unwrap();
        assert!(messages.iter().any(|m| m.content.contains("read_work_log")));
        assert!(
            messages
                .iter()
                .map(|m| m.wire().to_string().len())
                .sum::<usize>()
                < HARD_BYTES
        );
        assert_eq!(state.history.len(), 5);
    }
    #[test]
    fn resume_with_two_large_results_does_not_repeat_the_budget_pause() {
        let mut state = Progress::new(vec![Message::user("保留原始要求")]);
        state.add_instructions(vec![super::super::steering::Instruction {
            id: "new".into(),
            text: "只看 A01-01".into(),
            status: "sent".into(),
        }]);
        for i in 0..3 {
            state.push_tool(format!("call{i}"), "中".repeat(80_000));
        }
        state.resume_segment();
        let mut restored: Progress =
            serde_json::from_slice(&serde_json::to_vec(&state).unwrap()).unwrap();
        for _ in 0..2 {
            let messages = restored.messages(json!([])).unwrap();
            assert!(messages.iter().any(|m| m.content == "保留原始要求"));
            assert!(messages.iter().any(|m| m.content.contains("只看 A01-01")));
            assert!(
                messages
                    .iter()
                    .map(|m| m.wire().to_string().len())
                    .sum::<usize>()
                    < HARD_BYTES
            );
        }
        assert_eq!(restored.history.len(), 2);
    }
    #[test]
    fn prior_final_answers_have_task_references_and_user_requirements_stay_exact() {
        let mut state = Progress::new(vec![
            Message::user("要求一"),
            Message::assistant("答".repeat(80_000)),
            Message::user("要求二"),
            Message::assistant("案".repeat(80_000)),
            Message::user("第三輪請繼續"),
        ]);
        state.task_references(vec![
            None,
            Some("task-one".into()),
            None,
            Some("task-two".into()),
            None,
        ]);
        let messages = state.messages(json!([])).unwrap();
        for requirement in ["要求一", "要求二", "第三輪請繼續"] {
            assert!(messages.iter().any(|m| m.content == requirement));
        }
        assert!(messages.iter().any(|m| m.content.contains("task-one")));
        assert!(messages.iter().any(|m| m.content.contains("task-two")));
        assert_eq!(state.base[1].content.chars().count(), 80_000);
        let mut impossible = Progress::new(vec![Message::user(&"中".repeat(90_000))]);
        assert!(impossible
            .messages(json!([]))
            .err()
            .unwrap()
            .contains("無法藉由繼續減少"));
    }
}
