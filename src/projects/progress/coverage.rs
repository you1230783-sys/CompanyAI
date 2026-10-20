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
    /// CAD 分頁獨立於模型筆記保存；整理上下文不會遺失下一個未讀位置。
    #[serde(default)]
    cad_readings: BTreeMap<String, CadReading>,
    #[serde(skip)]
    pub(super) recovering: bool,
}
#[derive(Serialize, Deserialize)]
struct CadReading {
    source: Value,
    reading: Reading,
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
        if matches!(tool, Tool::CadActiveEdit { .. }) {
            self.cad_readings.remove("active");
        }
        let mut fresh = false;
        let reason = match tool {
            Tool::CadSnapshot { .. } => {
                // 索引模式不要求模型逐頁讀完；舊分頁游標不能指揮這份新快照。
                self.cad_readings.clear();
                fresh = self.mark(json!([
                    "cad-snapshot",
                    data["document"],
                    data["source"],
                    data["revision"]
                ]));
                "快照已存在；請按需求查詢候選，不反覆重建相同內容。"
            }
            Tool::CadQuery {
                filter, details, ..
            } => {
                for row in data["candidates"].as_array().into_iter().flatten() {
                    fresh |= self.mark(json!([
                        "cad-query",
                        data["revision"],
                        row["handle"],
                        details,
                        row
                    ]));
                }
                if data["matched_count"] == 0 {
                    fresh |= self.mark(json!(["cad-empty-query", data["revision"], filter]));
                }
                "相同候選資料已取得；請縮小條件、取詳細資料或完成修改，不反覆查同一範圍。"
            }
            Tool::CadPreview { .. } => {
                fresh = !data["selected_handle"].is_null()
                    && self.mark(json!([
                        "cad-selection",
                        data["revision"],
                        data["selected_handle"]
                    ]));
                "預覽本身不代表新增核對成果；沒有使用者選擇時不得猜目標。"
            }
            Tool::CadActiveRead { offset, .. } | Tool::CadRead { offset, .. } => {
                let active = matches!(tool, Tool::CadActiveRead { .. });
                let source = if active {
                    json!({"tool":"cad_active_read","document":data["document"]})
                } else {
                    json!({"tool":"cad_read","path":data["source"]})
                };
                // 作用中圖面只保留最新文件；切換文件或 revision 改變即作廢舊範圍。
                let key = if active {
                    "active".into()
                } else {
                    source.to_string()
                };
                let revision = data["revision"].as_str().unwrap_or("");
                let total = number(data, "entity_count");
                let entry = self.cad_readings.entry(key).or_insert_with(|| CadReading {
                    source: source.clone(),
                    reading: Reading::default(),
                });
                let changed = entry.source != source || entry.reading.revision != revision;
                if changed {
                    entry.source = source;
                    entry.reading = Reading {
                        revision: revision.into(),
                        total,
                        ..Reading::default()
                    };
                }
                let count = data["entities"].as_array().map_or(0, Vec::len);
                fresh = entry
                    .reading
                    .add(*offset, offset.saturating_add(count).min(total))
                    || (changed && total == 0);
                return (fresh, format!(
                    "CAD 已提供 {}/{} 個圖元；下一個未讀 offset={}。已讀頁面不要從 0 重掃；缺少舊頁細節請查 read_work_log。全部提供不代表規格已核對完成。",
                    entry.reading.ranges.iter().map(|(a, b)| b - a).sum::<usize>(),
                    total, entry.reading.next()
                ));
            }
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
            Tool::EditCodeSection { copy_id, .. } | Tool::EditText { copy_id, .. } => {
                fresh = data["changed"] != false
                    && self.mark(json!(["edited-copy", copy_id, data["revision"]]));
                "副本內容未變更；請修改下一個尚未完成的小段。"
            }
            Tool::PlanCodeChange { .. } => "需求清單已保存；規劃本身不代表程式已修改。",
            Tool::ReviewCodeChange {
                copy_id,
                revision,
                checks,
            } => {
                fresh = self.mark(json!([
                    "code-review",
                    copy_id,
                    revision,
                    checks
                        .iter()
                        .map(|c| json!([c.requirement_id, c.status, c.test_ids]))
                        .collect::<Vec<_>>()
                ]));
                "目前版本已核對；改寫證據文字不算新增成果。"
            }
            Tool::TestPython {
                copy_id, revision, ..
            } => {
                fresh = data["test_report"]["functional_tests_run"] == true
                    && self.mark(json!([
                        "code-tests",
                        copy_id,
                        revision,
                        // traceback 含每次隨機暫存目錄，不可據此假造新進度。
                        data["test_report"]["tests"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(|t| json!([
                                t["id"],
                                t["requirement_ids"],
                                t["status"],
                                t["tests_run"]
                            ]))
                            .collect::<Vec<_>>()
                    ]));
                "本次副本的相同測試結果已記錄；請修正失敗案例或核對成果。"
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
            | Tool::RecordReadingNotes { .. }
            | Tool::RecordAnalysis { .. }
            | Tool::RecordLogEvents { .. }
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

    /// 只傳遞範圍與游標，不把整頁圖元重新塞入每輪上下文。
    pub(super) fn cad_index(&self) -> Value {
        json!(self.cad_readings.values().map(|entry| {
            let reading = &entry.reading;
            json!({"source":entry.source,"revision":reading.revision,
                "entity_count":reading.total,"read_ranges":reading.ranges,
                "next_unread_offset":(reading.next()<reading.total).then(||reading.next()),
                "all_entities_delivered":reading.next()==reading.total,
                "notice":"已交付不代表已核對規格；沿下一個未讀位置續頁，舊細節查 read_work_log，不因上下文整理從 0 重掃。"})
        }).collect::<Vec<_>>())
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
    fn cad_pages_survive_checkpoint_and_repeated_pages_do_not_advance() {
        let mut state = Coverage::default();
        let mut data = json!({"document":{"name":"test.dwg","pid":42},
            "revision":"r1","entity_count":250,"entities":vec![json!({});100]});
        for offset in [0, 100] {
            assert!(
                state
                    .observe(
                        &Tool::CadActiveRead {
                            version: "latest".into(),
                            offset,
                        },
                        &data
                    )
                    .0
            );
        }
        // 模擬 checkpoint 保存／恢復；0、100 循環不能重設無進展計數。
        state = serde_json::from_value(serde_json::to_value(state).unwrap()).unwrap();
        for offset in [0, 100, 0, 100] {
            assert!(
                !state
                    .observe(
                        &Tool::CadActiveRead {
                            version: "latest".into(),
                            offset,
                        },
                        &data
                    )
                    .0
            );
            assert_eq!(state.cad_index()[0]["next_unread_offset"], 200);
        }
        data["entities"] = json!(vec![json!({}); 50]);
        assert!(
            state
                .observe(
                    &Tool::CadActiveRead {
                        version: "latest".into(),
                        offset: 200,
                    },
                    &data
                )
                .0
        );
        assert_eq!(state.cad_index()[0]["all_entities_delivered"], true);
        assert_eq!(state.cad_index()[0]["next_unread_offset"], Value::Null);

        // 圖面變更後即使讀到最後一頁，也不能沿用舊範圍宣稱讀完。
        data["revision"] = json!("r2");
        assert!(
            state
                .observe(
                    &Tool::CadActiveRead {
                        version: "latest".into(),
                        offset: 200,
                    },
                    &data
                )
                .0
        );
        assert_eq!(state.cad_index()[0]["next_unread_offset"], 0);
        assert_eq!(state.cad_index()[0]["all_entities_delivered"], false);
    }

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
