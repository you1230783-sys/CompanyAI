//! 判斷工具是否增加資料或實際處理範圍；隨 checkpoint 保存，不依 AI 筆記認定進度。
use super::{text, Reading, Tool};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default, Serialize, Deserialize)]
pub(super) struct Coverage {
    seen: BTreeSet<String>,
    ranges: BTreeMap<String, Reading>,
    mail_queries: BTreeMap<String, MailQuery>,
    #[serde(default)]
    recovered_headers: BTreeSet<String>,
    #[serde(skip)]
    pub(super) recovering: bool,
}
#[derive(Default, Serialize, Deserialize)]
struct MailQuery {
    folder_id: String,
    start_date: String,
    end_date: String,
    snapshot_id: String,
    total: usize,
    reading: Reading,
    scan_complete: bool,
}
impl Coverage {
    fn mark(&mut self, value: Value) -> bool {
        self.seen.insert(text::revision(&value.to_string()))
    }
    fn range(&mut self, key: String, start: usize, end: usize) -> bool {
        self.ranges.entry(key).or_default().add(start, end)
    }

    /// 回傳實際新增與可解釋原因；未知工具沿用去除暫時欄位後的結果比對。
    pub(super) fn observe(&mut self, tool: &Tool, data: &Value) -> (bool, String) {
        self.recovering = false;
        let mut fresh = false;
        let reason = match tool {
            Tool::OutlookHeaders {
                folder_id,
                start_date,
                end_date,
                ..
            } => {
                let scope = json!([folder_id, start_date, end_date]).to_string();
                // 同一封相同版本不因新查詢／游標／操作 ID 而變成新標題。
                for header in data["headers"].as_array().into_iter().flatten() {
                    fresh |= self.mark(json!([
                        "mail-header",
                        header["mail_id"],
                        header["revision"],
                        header
                    ]));
                }
                // 查完一個新範圍，即使沒有符合項目仍是有效的排除結果。
                if data["scan_complete"] == true {
                    fresh |= self.mark(json!(["mail-scanned", scope]));
                }
                let query = self.mail_queries.entry(scope).or_default();
                let snapshot = data["snapshot_id"].as_str().unwrap_or("");
                if !snapshot.is_empty() {
                    if query.snapshot_id != snapshot {
                        query.reading = Reading::default();
                        query.snapshot_id = snapshot.into();
                    }
                    query.folder_id = folder_id.clone();
                    query.start_date = start_date.clone();
                    query.end_date = end_date.clone();
                    query.total = data["total_unique"].as_u64().unwrap_or(0) as usize;
                    query.scan_complete = data["scan_complete"] == true;
                    let advanced = query
                        .reading
                        .add(number(data, "offset"), number(data, "next_offset"));
                    // 上下文整理後重新取得舊代號屬於恢復，不是假造新郵件。
                    // 同一封同版本最多豁免一次；重建隨機快照不能無限重設額度。
                    if !fresh && advanced {
                        for header in data["headers"].as_array().into_iter().flatten() {
                            let key = json!([
                                folder_id,
                                start_date,
                                end_date,
                                header["mail_id"],
                                header["revision"]
                            ])
                            .to_string();
                            self.recovering |= self.recovered_headers.insert(key);
                        }
                    }
                }
                "同一查詢未新增郵件標題；請沿用 outlook_paging 的下一頁游標，不要重建相同查詢。"
            }
            Tool::ReadMailNotes { mode, .. } => {
                let sources: Vec<_> = data["notes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|n| {
                        let source = n.get("source").unwrap_or(n);
                        json!([source["note_id"], source["revision"]])
                    })
                    .collect();
                fresh = self.mark(json!([
                    "mail-notes-read",
                    mode,
                    sources,
                    data["source_operation"],
                    data["offset"],
                    data["next_offset"]
                ]));
                "此頁筆記／原文已提供，請完成整理或寫入，勿反覆查回。"
            }
            Tool::SetWorkStage { stage, .. } => {
                fresh = self.mark(json!(["work-stage", stage]));
                "已在相同階段；切換理由或改寫筆記不算新進展。"
            }
            Tool::OutlookRead { mail_id, .. } if data["reused_note"] == true => {
                fresh = self.mark(json!([
                    "mail-note-recovery",
                    mail_id,
                    data["source"]["revision"]
                ]));
                "此郵件已有成果，請沿用摘要或定向查回原文。"
            }
            Tool::OutlookRead { mail_id, .. } => {
                fresh = self.range(
                    json!(["mail-body", mail_id, data["mail"]["revision"]]).to_string(),
                    number(data, "offset"),
                    number(data, "next_offset"),
                );
                "此封郵件的這段內文已讀過；請讀未讀區段或完成核對。"
            }
            Tool::OutlookFolders { .. } => {
                for folder in data["folders"].as_array().into_iter().flatten() {
                    fresh |= self.mark(json!(["mail-folder", folder]));
                }
                if data["total"] == 0 {
                    fresh |= self.mark(json!(["empty-folders", tool]));
                }
                "沒有新增 Outlook 資料夾；請選擇已列出的資料夾繼續。"
            }
            Tool::ReadLog { path, .. } => {
                for line in data["lines"].as_array().into_iter().flatten() {
                    fresh |= self.mark(json!(["log-line", path, data["revision"], line]));
                }
                "未讀到新的 LOG 行或區段。"
            }
            Tool::RunPython { .. } => {
                // 程式／purpose 改字或換輸入別名不代表算出了不同結果。
                let sources: Vec<_> = data["sources"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|s| json!([s["path"], s["revision"], s["start_line"], s["line_count"]]))
                    .collect();
                fresh = self.mark(json!([
                    "python-result",
                    sources,
                    data["summary"],
                    data["stdout"],
                    data["datasets"],
                    data["artifacts"]
                ]));
                "Python 的來源範圍與計算結果未新增；請調整分析或使用已有結果。"
            }
            Tool::CompactContext { .. }
            | Tool::RecordAnalysis { .. }
            | Tool::CreateNote { .. }
            | Tool::UpdateNote { .. }
            | Tool::UpdateDocumentNote { .. } => {
                "已保存筆記／整理上下文，但尚未增加資料、核對或成果。"
            }
            _ => {
                let mut key = json!({"tool":tool,"result":data});
                remove_ephemeral(&mut key);
                fresh = self.mark(key);
                "工具回傳與已取得的資料相同；請使用既有結果或處理尚未完成的範圍。"
            }
        };
        (
            fresh,
            if self.recovering {
                "正在恢復已讀郵件代號；已核對分頁前進，本次不增加停滯計數。".into()
            } else if fresh {
                format!("{}增加了資料或已確認範圍", tool.label())
            } else {
                reason.into()
            },
        )
    }

    pub(super) fn mail_index(&self) -> Value {
        let queries: Vec<_> = self.mail_queries.values().take(100).map(|q| {
            let next = q.reading.next();
            json!({"folder_id":q.folder_id,"start_date":q.start_date,"end_date":q.end_date,
                "headers_seen":q.reading.ranges.iter().map(|(a,b)|b-a).sum::<usize>(),"total_headers":q.total,
                "next_cursor":(next<q.total).then(||format!("{}:{next}",q.snapshot_id)),
                "all_headers_delivered":next==q.total,"scan_complete":q.scan_complete,
                "notice":"只代表標題已交給模型；尚未閱讀郵件內文。"})
        }).collect();
        json!(queries)
    }
}
fn number(value: &Value, field: &str) -> usize {
    value[field].as_u64().unwrap_or(0) as usize
}

/// 僅移除協定／分頁暫時值；版本、來源位置、實際資料與副本識別仍保留。
fn remove_ephemeral(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for key in [
                "operation_id",
                "request_id",
                "progress_note",
                "cursor",
                "next_cursor",
                "snapshot_id",
                "query_id",
                "elapsed_seconds",
                "duration_ms",
            ] {
                map.remove(key);
            }
            for child in map.values_mut() {
                remove_ephemeral(child);
            }
        }
        Value::Array(items) => {
            for item in items {
                remove_ephemeral(item);
            }
        }
        _ => (),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ten_pages_resume_and_requery_do_not_inflate_real_coverage() {
        let mut state = Coverage::default();
        let tool = Tool::OutlookHeaders {
            folder_id: "f".into(),
            start_date: "2026-10-01".into(),
            end_date: "2026-10-07".into(),
            cursor: None,
        };
        for page in 0..10 {
            let data = json!({"headers":[{"mail_id":format!("m{page}"),"revision":"v1"}],"snapshot_id":"s",
                "offset":page,"next_offset":page+1,"total_unique":10,"scan_complete":true});
            assert!(state.observe(&tool, &data).0);
            state = serde_json::from_value(serde_json::to_value(&state).unwrap()).unwrap();
        }
        assert_eq!(state.mail_index()[0]["headers_seen"], 10);
        assert_eq!(state.mail_index()[0]["next_cursor"], Value::Null);
        let old = json!({"headers":[{"mail_id":"m0","revision":"v1"}],"snapshot_id":"another",
            "offset":0,"next_offset":1,"total_unique":10,"scan_complete":true});
        assert!(!state.observe(&tool, &old).0);
        assert_eq!(state.mail_index()[0]["next_cursor"], "another:1");
        let mut changed = old;
        changed["headers"][0]["revision"] = json!("v2");
        assert!(state.observe(&tool, &changed).0);
    }
    #[test]
    fn recovering_old_header_ids_is_bounded_and_survives_checkpoint() {
        let mut state = Coverage::default();
        let tool = Tool::OutlookHeaders {
            folder_id: "f".into(),
            start_date: "2026-10-01".into(),
            end_date: "2026-10-07".into(),
            cursor: None,
        };
        let mut data = json!({"headers":[{"mail_id":"m","revision":"v1"}],"snapshot_id":"first","offset":0,"next_offset":1,"total_unique":2,"scan_complete":true});
        assert!(state.observe(&tool, &data).0);
        data["snapshot_id"] = json!("second");
        assert!(!state.observe(&tool, &data).0);
        assert!(state.recovering);
        state = serde_json::from_value(serde_json::to_value(state).unwrap()).unwrap();
        data["snapshot_id"] = json!("third");
        assert!(!state.observe(&tool, &data).0);
        assert!(!state.recovering);
    }
}
