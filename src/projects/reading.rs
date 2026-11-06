//! 與滑動工具歷史分開保存的閱讀成果。來源範圍由程式產生，語意由模型提供；
//! 即使原文移出請求，前段筆記也不會被最新的 progress_note 取代。
use crate::AppResult;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Digest {
    pub operation_id: String,
    pub summary: String,
    pub relationships: String,
    pub open_questions: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Source {
    pub operation_id: String,
    pub path: String,
    pub revision: String,
    pub offset: usize,
    pub end: usize,
    pub total: usize,
    pub first_line: Option<usize>,
    pub last_line: Option<usize>,
    /// 圖片保留所有來源雜湊、幀與原始辨識焦點；文字沿用實際字元範圍。
    #[serde(default)]
    pub evidence: Value,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Note {
    pub source: Source,
    pub digest: Digest,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Notebook {
    /// 舊 checkpoint 及單獨 Broker 呼叫預設不啟用新任務階段限制。
    pub enabled: bool,
    pub pending: BTreeMap<String, Source>,
    pub notes: BTreeMap<String, Note>,
    /// 最近三頁實際目錄清單的短索引；技能切換後仍保留完整相對路徑，不當成已讀內容。
    #[serde(default)]
    pub directories: Vec<Value>,
    /// 先查封存辨識後才可做一次同版圖片補辨識；消耗狀態隨 checkpoint 保存。
    #[serde(default)]
    image_followups: BTreeMap<String, Option<String>>,
    /// 同一封存頁只補查一次；後續需把已取得細節補回筆記，不在原文間來回。
    #[serde(default)]
    pub(crate) recalled_pages: std::collections::BTreeSet<String>,
}

/// 精讀例外的同一界線用於交付與保存，避免其中一側誤認成一般重讀。
fn precision_window(data: &Value) -> bool {
    data["first_line"]
        .as_u64()
        .zip(data["last_line"].as_u64())
        .is_some_and(|(first, last)| last >= first && last - first < 40)
        && data["text"]
            .as_str()
            .is_some_and(|text| text.chars().count() <= 2000)
}

impl Notebook {
    /// Excel 選欄工具回傳結構化儲存格，不能套用一般文字的 64 位 revision／字元頁協定。
    /// 只登記工具實際交付的範圍；UsedRange 不代表全部讀取，數值原文仍從操作封存查回。
    pub fn observe_excel(&mut self, id: &str, result: &Value) -> AppResult<()> {
        if !self.enabled
            || result["ok"] != true
            || self.pending.contains_key(id)
            || self.notes.contains_key(id)
        {
            return Ok(());
        }
        let data = &result["result"];
        let path = data["path"].as_str().ok_or("Excel 閱讀缺少來源路徑。")?;
        let revision = data["revision"]
            .as_str()
            .ok_or("Excel 閱讀缺少來源版本。")?;
        if !revision
            .strip_prefix("excel:")
            .is_some_and(|hash| hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err("Excel 閱讀來源版本格式無效。".into());
        }
        let headers = data["headers"]
            .as_array()
            .ok_or("Excel 閱讀缺少實際表頭。")?;
        let rows = data["rows"].as_array();
        if rows.is_some_and(|rows| rows.is_empty()) && headers.is_empty() {
            return Ok(());
        }
        if self.pending.len() >= 8 || self.notes.len() >= 400 {
            return Err("閱讀筆記已達待整理8段或保存400段上限，請先整理現有成果。".into());
        }
        let count = data.to_string().chars().count();
        let normalized = path.replace('\\', "/");
        self.pending.retain(|_, source| {
            !source.path.eq_ignore_ascii_case(&normalized) || source.revision == revision
        });
        self.notes.retain(|_, note| {
            !note.source.path.eq_ignore_ascii_case(&normalized) || note.source.revision == revision
        });
        self.pending.insert(id.into(), Source {
            operation_id: id.into(), path: normalized, revision: revision.into(),
            offset: 0, end: count, total: count, first_line: None, last_line: None,
            evidence: json!({"kind":"excel","sheet":data["sheet"],"sheet_name":data["sheet_name"],
                "columns":data["columns"],"header_row":data["header_row"],"header_count":headers.len(),
                "first_row":rows.and_then(|rows| rows.first()).map(|row| &row["row"]),
                "last_row":rows.and_then(|rows| rows.last()).map(|row| &row["row"]),
                "row_count":rows.map_or(0, Vec::len),"next_row":data["next_row"],
                "scope":data["scope"],"coverage_unit":"本次實際交付的結構化 JSON 字元，非工作表總列數；表頭檢查不代表已讀資料列"}),
        });
        Ok(())
    }

    /// 將實際辨識文字接入同一筆記契約；範圍是辨識文字，並非圖片像素已全部讀懂。
    pub fn observe_image(&mut self, id: &str, result: &Value) -> AppResult<()> {
        let data = &result["result"];
        if result["ok"] != true || data["reused_reading_note"] == true {
            return Ok(());
        }
        let (Some(analysis), Some(images)) = (data["analysis"].as_str(), data["images"].as_array())
        else {
            return Ok(());
        };
        let Some(path) = images.first().and_then(|image| image["path"].as_str()) else {
            return Ok(());
        };
        let revision = super::text::revision(&json!(images).to_string());
        let count = analysis.chars().count();
        let normalized = json!({"ok":true,"result":{"path":path,"revision":revision,
            "text":analysis,"offset":0,"next_offset":count,"total":count}});
        // 補辨識即使文字較短也必須重新整理，不能誤認舊文字範圍已涵蓋新內容。
        if self.pending.contains_key(id) || self.notes.contains_key(id) {
            return Ok(());
        }
        let previous: Vec<_> = self
            .notes
            .iter()
            .filter(|(_, n)| n.source.revision == revision)
            .map(|(key, _)| key.clone())
            .collect();
        let mut next = self.clone();
        for key in &previous {
            next.notes.remove(key);
        }
        next.observe(id, &normalized)?;
        if let Some(source) = next.pending.get_mut(id) {
            source.evidence = json!({"kind":"image","images":images,"focus":data["focus"],
                "coverage_unit":"辨識文字字元；多頁圖片僅已送出的頁／幀", "previous_notes":
                previous.iter().filter_map(|key| self.notes.get(key)).collect::<Vec<_>>()});
        }
        *self = next;
        Ok(())
    }

    /// 已讀圖片預設交付筆記；具體缺項先查封存，最多一次同版原圖補辨識。
    pub fn reuse_image(&mut self, images: &[Value], focus: &str) -> Option<Value> {
        if !self.enabled {
            return None;
        }
        let revision = super::text::revision(&json!(images).to_string());
        let notes: Vec<_> = self
            .notes
            .values()
            .filter(|n| n.source.revision == revision)
            .collect();
        if notes.is_empty() {
            return None;
        }
        if self
            .image_followups
            .get(&revision)
            .and_then(Option::as_deref)
            == Some(focus)
        {
            self.image_followups.insert(revision, None);
            return None;
        }
        Some(
            json!({"ok":true,"result":{"reused_reading_note":true,"images":images,"reading_notes":notes,
            "guidance":"同版圖片已有筆記，請據此完成分析。具體缺項用read_reading_notes(source_operation,reason)先查已保存辨識；仍不足才用回傳的image_followup補辨識一次。不要只改focus反覆重讀。"}}),
        )
    }

    pub fn allow_image_followup(&mut self, id: &str, reason: &str) -> Option<Value> {
        let source = &self.notes.get(id)?.source;
        if source.evidence["kind"] != "image" || self.image_followups.contains_key(&source.revision)
        {
            return None;
        }
        self.image_followups
            .insert(source.revision.clone(), Some(reason.into()));
        Some(json!({"images":source.evidence["images"],"focus":reason,
            "notice":"先核對此次封存原文；原文仍缺少此細節才以相同focus補辨識一次。新結果須與既有筆記合併保存，不能覆蓋掉已確認的數值。"}))
    }
    pub fn remember_directory(&mut self, path: &str, offset: usize, outcome: &Value) {
        if !self.enabled || outcome["ok"] != true {
            return;
        }
        let Some(entries) = outcome["result"]["entries"].as_array() else {
            return;
        };
        let path = path.replace('\\', "/");
        let mut short = Vec::new();
        for entry in entries.iter().take(32) {
            let (Some(name), Some(directory)) =
                (entry["name"].as_str(), entry["directory"].as_bool())
            else {
                continue;
            };
            let full = if path.is_empty() {
                name.to_owned()
            } else {
                format!("{path}/{name}")
            };
            short.push(json!({"path":full,"directory":directory}));
            if serde_json::to_vec(&short).map_or(true, |bytes| bytes.len() > 4000) {
                short.pop();
                break;
            }
        }
        self.directories
            .retain(|page| page["path"] != path || page["offset"] != offset);
        self.directories.push(json!({"path":path,"offset":offset,"next_offset":outcome["result"]["next_offset"],"entries_omitted":entries.len().saturating_sub(short.len()),"entries":short}));
        // 不截斷路徑／JSON。超過6KB時移出最舊整頁；完整清單仍在原操作紀錄。
        while self.directories.len() > 3
            || serde_json::to_vec(&self.directories).map_or(true, |bytes| bytes.len() > 6000)
        {
            if self.directories.is_empty() {
                break;
            }
            self.directories.remove(0);
        }
    }
    /// 相鄰區段可以共同覆蓋新要求；區段間有缺口時仍交付原文。
    /// 大範圍重讀重用成果；尚未精讀過的小窗格允許一次核對原文，避免摘要不足時反覆查詢。
    pub fn project_saved_read(&self, result: &mut Value) {
        if !self.enabled || result["working_copy"] == true || result["copy_id"].is_string() {
            return;
        }
        let (Some(path), Some(revision), Some(start), Some(end)) = (
            result["path"].as_str(),
            result["revision"].as_str(),
            result["offset"].as_u64(),
            result["next_offset"].as_u64(),
        ) else {
            return;
        };
        if end <= start {
            return;
        }
        let path = path.replace('\\', "/");
        let mut notes: Vec<_> = self
            .notes
            .values()
            .filter(|n| {
                n.source.path.eq_ignore_ascii_case(&path)
                    && n.source.revision == revision
                    && n.source.end > start as usize
                    && n.source.offset < end as usize
            })
            .collect();
        // 優先回傳同一精讀窗格的筆記，不能再次用較早的大段摘要蓋過剛核對的細節。
        notes.sort_by_key(|n| {
            (
                n.source.offset != start as usize || n.source.end != end as usize,
                n.source.offset,
                n.source.end,
            )
        });
        let exact_note = notes
            .iter()
            .any(|note| note.source.offset == start as usize && note.source.end == end as usize);
        let mut covered = Vec::new();
        let mut cursor = start as usize;
        for note in notes {
            if note.source.offset > cursor {
                break;
            }
            if note.source.end > cursor {
                cursor = note.source.end;
                covered.push(note);
            }
            if cursor >= end as usize {
                break;
            }
        }
        if cursor < end as usize {
            return;
        }
        // 摘要可能漏掉常數或分支。已有大段筆記時，仍讓模型取得一次有界精讀：
        // 最多40行及2000字，保存後同一窗格就回到筆記重用，不另外建立可遺失的計數器。
        if precision_window(result) && !exact_note {
            result["targeted_reread"] = json!(true);
            result["guidance"] = json!("本次交付少量精確原文以核對摘要缺項。先record_reading_notes保存確認的常數／分支與行號，接著完成目前成果；同一小窗格保存後會重用筆記，不要再重讀全文。");
            return;
        }
        let operations: Vec<_> = covered
            .iter()
            .map(|n| n.source.operation_id.clone())
            .collect();
        // 大量極小區段只帶索引及前兩則，避免回傳筆記反而超過工具訊息上限。
        let preview: Vec<_> = covered.into_iter().take(2).collect();
        result["text"] = Value::Null;
        result["reused_reading_note"] = json!(true);
        result["reading_notes"] = json!(preview);
        result["note_operations"] = json!(operations);
        result["guidance"]=json!("此版本範圍已有閱讀成果，先整合筆記。完整筆記用read_reading_notes(path,offset)分頁；需要精確原文才指定source_operation與具體reason定向查回，不因原文不在上下文重讀全文。");
    }
    /// 只登記真正交付的文字，不接受模型自報已讀或要求行號。
    pub fn observe(&mut self, id: &str, result: &Value) -> AppResult<()> {
        if !self.enabled || result["ok"] != true {
            return Ok(());
        }
        let data = &result["result"];
        if data["reused_reading_note"] == true
            || data["working_copy"] == true
            || data["copy_id"].is_string()
            || data["text"].as_str().is_none_or(str::is_empty)
        {
            return Ok(());
        }
        let Some(path) = data["path"].as_str() else {
            return Ok(());
        };
        let Some(revision) = data["revision"]
            .as_str()
            .filter(|r| r.len() == 64 && r.bytes().all(|b| b.is_ascii_hexdigit()))
        else {
            return Ok(());
        };
        let number = |key: &str| data[key].as_u64().and_then(|n| usize::try_from(n).ok());
        let (Some(offset), Some(end), Some(total)) =
            (number("offset"), number("next_offset"), number("total"))
        else {
            return Ok(());
        };
        if offset >= end
            || end > total
            || data["text"].as_str().unwrap().chars().count() != end - offset
        {
            return Err("閱讀成果來源範圍不一致，未建立筆記要求。".into());
        }
        let normalized = path.replace('\\', "/");
        // 新版本使舊筆記退出當前工作集，完整舊內容仍保留於操作封存。
        self.pending
            .retain(|_, n| !n.path.eq_ignore_ascii_case(&normalized) || n.revision == revision);
        self.notes.retain(|_, n| {
            !n.source.path.eq_ignore_ascii_case(&normalized) || n.source.revision == revision
        });
        let precise = data["targeted_reread"] == true && precision_window(data);
        if self.notes.values().any(|n| {
            n.source.path.eq_ignore_ascii_case(&normalized)
                && n.source.revision == revision
                && if precise {
                    n.source.offset == offset && n.source.end == end
                } else {
                    n.source.offset <= offset && n.source.end >= end
                }
        }) {
            return Ok(());
        }
        if self.pending.contains_key(id) {
            return Ok(());
        }
        if self.pending.len() >= 8 || self.notes.len() >= 400 {
            return Err("閱讀筆記已達待整理8段或保存400段上限，請先整理現有成果。".into());
        }
        self.pending.insert(
            id.into(),
            Source {
                operation_id: id.into(),
                path: normalized,
                revision: revision.into(),
                offset,
                end,
                total,
                first_line: number("first_line"),
                last_line: number("last_line"),
                evidence: Value::Null,
            },
        );
        Ok(())
    }

    /// 一批全部驗證後才提交；失敗不清空待整理來源，也不接受假造來源範圍。
    pub fn accept(&mut self, digests: &[Digest]) -> AppResult<Value> {
        if digests.is_empty() || digests.len() > 8 {
            return Err("每次保存1–8份閱讀成果。".into());
        }
        let mut next = self.clone();
        let mut seen = std::collections::BTreeSet::new();
        for (index, digest) in digests.iter().enumerate() {
            if !seen.insert(&digest.operation_id) {
                return Err("同批閱讀筆記來源不可重複。".into());
            }
            for (field, value, limit, required) in [
                ("summary", &digest.summary, 3000, true),
                ("relationships", &digest.relationships, 1500, false),
                ("open_questions", &digest.open_questions, 400, false),
            ] {
                if (required && value.trim().is_empty()) || value.chars().count() > limit {
                    return Err(format!("notes[{index}].{field}目前{}字，上限{limit}字{}。只需修正這個欄位後重送本批筆記，不必重新閱讀來源。",value.chars().count(),if required{"且不可空白"}else{""}));
                }
            }
            let source = next
                .pending
                .remove(&digest.operation_id)
                .or_else(|| {
                    next.notes
                        .get(&digest.operation_id)
                        .map(|n| n.source.clone())
                })
                .ok_or("筆記只能引用已成功交付的閱讀操作，不能猜測operation_id。")?;
            next.notes.insert(
                digest.operation_id.clone(),
                Note {
                    source,
                    digest: digest.clone(),
                },
            );
        }
        if serde_json::to_vec(&next).map_err(|e| e.to_string())?.len() > 1_500_000 {
            return Err("閱讀成果超過1.5MB，請縮小本次任務範圍。".into());
        }
        *self = next;
        Ok(
            json!({"saved":digests.len(),"pending":self.pending.values().collect::<Vec<_>>(),
            "notice":"來源範圍已核對；摘要與關係是模型判斷。先用這些成果整合或畫圖，確切缺項才定向回讀。"}),
        )
    }

    pub fn read(&self, path: Option<&str>, offset: usize) -> AppResult<Value> {
        let mut notes: Vec<_> = self
            .notes
            .values()
            .filter(|n| {
                path.is_none_or(|p| n.source.path.eq_ignore_ascii_case(&p.replace('\\', "/")))
            })
            .collect();
        notes.sort_by(|a, b| {
            a.source
                .path
                .cmp(&b.source.path)
                .then(a.source.offset.cmp(&b.source.offset))
        });
        if offset > notes.len() {
            return Err("閱讀筆記位置超出範圍。".into());
        }
        let mut page = Vec::new();
        let mut bytes = 0;
        for note in notes.iter().skip(offset).take(8) {
            let size = serde_json::to_vec(note).map_err(|e| e.to_string())?.len();
            if bytes + size > 48_000 {
                break;
            }
            bytes += size;
            page.push(note);
        }
        Ok(
            json!({"notes":page,"total":notes.len(),"next_offset":offset+page.len(),
            "notice":"原文已封存，摘要不是執行或語意驗證。先整合已有筆記；只有缺少具體細節才用read_reading_notes的source_operation與reason定向查回。"}),
        )
    }

    pub fn context(&self) -> Option<Value> {
        if !self.enabled
            || (self.pending.is_empty() && self.notes.is_empty() && self.directories.is_empty())
        {
            return None;
        }
        let mut notes: Vec<_> = self.notes.values().collect();
        notes.sort_by(|a, b| {
            a.source
                .path
                .cmp(&b.source.path)
                .then(a.source.offset.cmp(&b.source.offset))
        });
        // 字元不等於位元組。單則訊息上限64KB，正文留32KB，其餘留給索引與待辦。
        let mut budget = 32_000usize;
        let paths: std::collections::BTreeSet<_> =
            notes.iter().map(|n| n.source.path.as_str()).collect();
        let per_file = 32_000 / paths.len().max(1);
        let mut file_bytes: BTreeMap<&str, usize> = BTreeMap::new();
        let mut loaded = Vec::new();
        let mut omitted = Vec::new();
        let mut omitted_total = 0;
        for note in notes {
            let size = serde_json::to_string(note).unwrap_or_default().len();
            let used = file_bytes.entry(&note.source.path).or_default();
            if size <= budget && *used + size <= per_file {
                budget -= size;
                *used += size;
                loaded.push(note);
            } else {
                omitted_total += 1;
                if omitted.len() < 24 {
                    omitted.push(json!({"path":note.source.path.chars().take(80).collect::<String>(),"offset":note.source.offset,"operation_id":note.source.operation_id}));
                }
            }
        }
        Some(
            json!({"pending":self.pending.values().collect::<Vec<_>>(),"notes":loaded,"omitted":omitted,"omitted_total":omitted_total,
            "directory_index":self.directories,"directory_notice":"這是最近已走訪目錄的精簡快照，不是已讀原文。直接用entries.path選讀README、套件入口或必要模組；切換技能不必重列相同目錄。只有資料夾已變更或需省略／下一頁項目才重新列出。",
            "notice":"pending非空時先record_reading_notes保存上一批所有成果再讀下一批／整理／畫圖。筆記依來源累積，不由progress_note取代。按使用者要求保存可直接寫入答覆的事實、數字與單位、圖軸範圍／線性或對數尺度、峰位與估讀誤差、推論依據及未確認事項；程式碼另保存流程／分支／行號，不把宣告當實作。來源及進度由程式提供，不自行宣稱全文件已讀完。已有足夠成果就交付，不因原文不在本輪就重讀。operation_id可直接用op_簡短ID查筆記或read_work_log，不截斷或猜完整ID。省略筆記用read_reading_notes(path,offset)分頁查回。"}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_notes_keep_evidence_and_allow_only_one_focused_followup_after_restore() {
        let mut notebook = Notebook {
            enabled: true,
            ..Notebook::default()
        };
        let images = vec![json!({"path":"EIS/Nyquist.png","sha256":"a".repeat(64),"frames":1})];
        let result = json!({"ok":true,"result":{"images":images,"analysis":"Rs約0.85Ω，圖軸為線性；誤差約0.1Ω。","focus":"解釋阻抗圖"}});
        notebook.observe_image("first", &result).unwrap();
        assert_eq!(notebook.pending["first"].evidence["kind"], "image");
        assert!(notebook.pending["first"].evidence["coverage_unit"]
            .as_str()
            .unwrap()
            .contains("辨識文字"));
        notebook
            .accept(&[digest("first", "Rs約0.85Ω；圖面估讀，不是原始數據擬合")])
            .unwrap();
        assert_eq!(
            notebook.reuse_image(&images, "再讀一次").unwrap()["result"]["reused_reading_note"],
            true
        );
        assert!(notebook
            .allow_image_followup("first", "核對低頻端點與單位")
            .is_some());
        let mut restored: Notebook = serde_json::from_value(json!(notebook)).unwrap();
        assert!(restored
            .reuse_image(&images, "核對低頻端點與單位")
            .is_none());
        assert!(restored
            .reuse_image(&images, "核對低頻端點與單位")
            .is_some());
        assert!(restored.allow_image_followup("first", "另一理由").is_none());
        let next = json!({"ok":true,"result":{"images":images,"analysis":"端點11.7Ω","focus":"核對低頻端點與單位"}});
        restored.observe_image("followup", &next).unwrap();
        assert_eq!(
            restored.pending["followup"].evidence["previous_notes"][0]["digest"]["summary"],
            "Rs約0.85Ω；圖面估讀，不是原始數據擬合"
        );
        restored
            .accept(&[digest("followup", "Rs約0.85Ω；端點約11.7Ω，均為估讀")])
            .unwrap();
        let changed = vec![json!({"path":"EIS/Nyquist.png","sha256":"b".repeat(64),"frames":1})];
        assert!(restored.reuse_image(&changed, "新版本").is_none());
        assert!(restored
            .reuse_image(
                &[json!({"path":"other.png","sha256":"a".repeat(64)})],
                "其他來源"
            )
            .is_none());
    }
    #[test]
    fn directory_paths_survive_handoff_without_becoming_content_notes() {
        let mut notebook = Notebook {
            enabled: true,
            ..Notebook::default()
        };
        let listed = json!({"ok":true,"result":{"entries":[{"name":"__init__.py","directory":false},{"name":"core.py","directory":false}],"next_offset":null}});
        notebook.remember_directory("src/click", 0, &listed);
        let restored: Notebook =
            serde_json::from_value(serde_json::to_value(&notebook).unwrap()).unwrap();
        let context = restored.context().unwrap();
        assert_eq!(
            context["directory_index"][0]["entries"][0]["path"],
            "src/click/__init__.py"
        );
        assert!(context["pending"].as_array().unwrap().is_empty());
        assert!(context["notes"].as_array().unwrap().is_empty());
        for n in 0..10 {
            notebook.remember_directory(&format!("dir{n}"), 0, &listed);
        }
        assert_eq!(notebook.directories.len(), 3);
        assert_eq!(notebook.directories[2]["path"], "dir9");
        notebook.remember_directory(
            "dir9",
            0,
            &json!({"ok":true,"result":{"entries":[{"name":"new.py","directory":false}]}}),
        );
        assert_eq!(notebook.directories.len(), 3);
        assert_eq!(notebook.directories[2]["entries"][0]["path"], "dir9/new.py");
        let old: Notebook =
            serde_json::from_value(json!({"enabled":true,"pending":{},"notes":{}})).unwrap();
        assert!(old.directories.is_empty());
    }
    #[test]
    fn a_small_precision_window_is_delivered_once_then_reuses_its_saved_note() {
        let mut notebook = Notebook {
            enabled: true,
            ..Notebook::default()
        };
        let revision = "a".repeat(64);
        let whole = json!({"ok":true,"result":{"path":"main.cpp","revision":revision,"offset":0,"next_offset":200,"total":200,"first_line":1,"last_line":100,"text":"x\n".repeat(100)}});
        notebook.observe("whole", &whole).unwrap();
        notebook
            .accept(&[digest("whole", "整體流程；尚未記錄個別常數")])
            .unwrap();
        let small = json!({"path":"main.cpp","revision":revision,"offset":0,"next_offset":4,"total":200,"first_line":1,"last_line":2,"text":"x\nx\n"});
        let mut first = small.clone();
        notebook.project_saved_read(&mut first);
        assert_eq!(first["targeted_reread"], true);
        assert_eq!(first["text"], "x\nx\n");
        notebook
            .observe("small", &json!({"ok":true,"result":first}))
            .unwrap();
        notebook
            .accept(&[digest("small", "已核對第1–2行的精確值")])
            .unwrap();
        let mut repeated = small;
        notebook.project_saved_read(&mut repeated);
        assert_eq!(repeated["reused_reading_note"], true);
        assert!(repeated["text"].is_null());
        assert_eq!(
            repeated["reading_notes"][0]["digest"]["summary"],
            "已核對第1–2行的精確值"
        );
        let mut broad = json!({"path":"main.cpp","revision":revision,"offset":0,"next_offset":100,"total":200,"first_line":1,"last_line":50,"text":"x\n".repeat(50)});
        notebook.project_saved_read(&mut broad);
        assert_eq!(broad["reused_reading_note"], true);
    }
    fn page(path: &str, revision: &str, start: usize) -> Value {
        json!({"ok":true,"result":{"path":path,"revision":revision,"offset":start,"next_offset":start+3,"total":30,"first_line":start+1,"last_line":start+3,"text":"abc"}})
    }
    fn digest(id: &str, text: &str) -> Digest {
        Digest {
            operation_id: id.into(),
            summary: text.into(),
            relationships: "入口呼叫工作函式；宣告不等於實作".into(),
            open_questions: String::new(),
        }
    }
    #[test]
    fn batch_notes_are_atomic_and_survive_resume_without_overwriting_first_file() {
        let mut notebook = Notebook {
            enabled: true,
            ..Notebook::default()
        };
        let revision = "a".repeat(64);
        notebook
            .observe("cpp-1", &page("main.cpp", &revision, 0))
            .unwrap();
        notebook
            .observe("h-1", &page("main.h", &revision, 0))
            .unwrap();
        assert!(notebook
            .accept(&[digest("cpp-1", "主流程"), digest("fake", "不能冒用")])
            .is_err());
        assert_eq!(notebook.pending.len(), 2);
        assert!(notebook.notes.is_empty());
        notebook
            .accept(&[digest("cpp-1", "cpp主流程"), digest("h-1", "h類別宣告")])
            .unwrap();
        let restored: Notebook =
            serde_json::from_value(serde_json::to_value(notebook).unwrap()).unwrap();
        let context = restored.context().unwrap().to_string();
        assert!(context.contains("cpp主流程"));
        assert!(context.contains("h類別宣告"));
        assert!(restored.pending.is_empty());
    }
    #[test]
    fn duplicate_delivery_does_not_overflow_eight_pending_and_new_version_invalidates_notes() {
        let mut n = Notebook {
            enabled: true,
            ..Notebook::default()
        };
        let old = "a".repeat(64);
        for i in 0..8 {
            n.observe(&format!("r{i}"), &page("a.h", &old, i * 3))
                .unwrap();
        }
        n.observe("r0", &page("a.h", &old, 0)).unwrap();
        assert_eq!(n.pending.len(), 8);
        n.accept(&[digest("r0", "舊版本內容")]).unwrap();
        n.observe("new", &page("a.h", &"b".repeat(64), 0)).unwrap();
        assert!(n.notes.is_empty());
        assert_eq!(n.pending.len(), 1);
    }
    #[test]
    fn large_notes_keep_both_files_visible_and_pages_fit_the_transport_budget() {
        let mut notebook = Notebook {
            enabled: true,
            ..Notebook::default()
        };
        let revision = "a".repeat(64);
        for path in ["main.cpp", "main.h"] {
            for section in 0..16 {
                let id = format!("{path}-{section}");
                let mut source = page(path, &revision, section * 3);
                source["result"]["total"] = json!(1000);
                notebook.observe(&id, &source).unwrap();
                notebook
                    .accept(&[Digest {
                        operation_id: id,
                        summary: "流程條件".repeat(240),
                        relationships: "呼叫關係".repeat(180),
                        open_questions: "待確認".repeat(100),
                    }])
                    .unwrap();
            }
        }
        let context = notebook.context().unwrap();
        let notes = context["notes"].as_array().unwrap();
        assert!(notes.iter().any(|n| n["source"]["path"] == "main.cpp"));
        assert!(notes.iter().any(|n| n["source"]["path"] == "main.h"));
        assert!(context.to_string().len() < 64_000);
        assert!(context["omitted_total"].as_u64().unwrap() > 0);
        let mut offset = 0;
        while offset < 32 {
            let page = notebook.read(None, offset).unwrap();
            assert!(page.to_string().len() < 64_000);
            let next = page["next_offset"].as_u64().unwrap() as usize;
            assert!(next > offset);
            offset = next;
        }
    }
    #[test]
    fn saved_adjacent_ranges_reuse_notes_but_gaps_new_versions_and_copies_do_not() {
        let mut notebook = Notebook {
            enabled: true,
            ..Notebook::default()
        };
        let revision = "a".repeat(64);
        for start in [0, 3, 9] {
            let id = format!("range-{start}");
            notebook
                .observe(&id, &page("main.h", &revision, start))
                .unwrap();
            notebook.accept(&[digest(&id, "分區與呼叫關係")]).unwrap();
        }
        let mut adjacent =
            json!({"path":"main.h","revision":revision,"offset":0,"next_offset":6,"text":"abcdef"});
        notebook.project_saved_read(&mut adjacent);
        assert_eq!(adjacent["reused_reading_note"], true);
        assert!(adjacent["text"].is_null());
        assert_eq!(adjacent["note_operations"].as_array().unwrap().len(), 2);
        for mut raw in [
            json!({"path":"main.h","revision":revision,"offset":0,"next_offset":12,"text":"有缺口"}),
            json!({"path":"main.h","revision":"b".repeat(64),"offset":0,"next_offset":6,"text":"新版本"}),
            json!({"path":"main.h","revision":revision,"offset":0,"next_offset":6,"text":"工作副本","working_copy":true}),
        ] {
            let original = raw["text"].clone();
            notebook.project_saved_read(&mut raw);
            assert_eq!(raw["text"], original);
        }
    }
    #[test]
    fn note_length_rejection_names_the_field_and_preserves_pending_source() {
        let mut notebook = Notebook {
            enabled: true,
            ..Notebook::default()
        };
        notebook
            .observe("source", &page("README.md", &"a".repeat(64), 0))
            .unwrap();
        let mut note = digest("source", "專案入口與限制");
        note.relationships = "a".repeat(1501);
        let error = notebook.accept(&[note.clone()]).unwrap_err();
        assert!(
            error.contains("notes[0].relationships")
                && error.contains("1501")
                && error.contains("1500")
        );
        assert!(notebook.pending.contains_key("source"));
        note.relationships = "a".repeat(1053);
        notebook.accept(&[note]).unwrap();
        assert!(notebook.pending.is_empty());
    }
}
