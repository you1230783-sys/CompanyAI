//! 簡短識別碼只供查閱來源；內部冪等鍵仍使用完整 ID，避免把查詢別名當成寫入操作。
use super::*;

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct References {
    /// 以完整 ID 為鍵，序號一經分配就隨 checkpoint 保存，不因封存或排序改變。
    ids: BTreeMap<String, String>,
    next: u64,
}

impl References {
    pub fn register(&mut self, id: &str) -> String {
        if let Some(alias) = self.ids.get(id) {
            return alias.clone();
        }
        let alias = loop {
            self.next += 1;
            let candidate = format!("op_{:04}", self.next);
            if candidate != id
                && !self.ids.contains_key(&candidate)
                && !self.ids.values().any(|value| value == &candidate)
            {
                break candidate;
            }
        };
        self.ids.insert(id.into(), alias.clone());
        alias
    }

    pub fn resolve(&self, value: &str) -> AppResult<String> {
        let canonical = self.ids.iter().find(|(_, alias)| alias.as_str() == value);
        if let Some((id, _)) = canonical {
            // 舊版允許任意合法操作名；若碰巧與別名相同，拒絕猜測其指向。
            if self.ids.contains_key(value) && id != value {
                return Err("此識別碼有歧義，請重新列出操作索引並使用明確的來源 ID。".into());
            }
            return Ok(id.clone());
        }
        Ok(value.into())
    }

    pub fn alias(&self, id: &str) -> String {
        self.ids.get(id).cloned().unwrap_or_else(|| id.into())
    }

    /// 只轉換筆記的結構欄位，絕不改動原文、摘要或程式碼中的相同字串。
    pub fn project(&self, value: &mut Value) {
        match value {
            Value::Array(items) => items.iter_mut().for_each(|v| self.project(v)),
            Value::Object(fields) => {
                for (key, value) in fields {
                    if key == "operation_id" {
                        if let Some(id) = value.as_str() {
                            *value = json!(self.alias(id));
                        }
                    } else {
                        self.project(value);
                    }
                }
            }
            _ => (),
        }
    }
}

impl Broker {
    /// Python 結果是衍生計算，不冒充原文區段；僅查原生操作索引，不載入或執行程式。
    pub(super) fn record_reading_notes(
        &mut self,
        notes: &[crate::projects::reading::Digest],
    ) -> AppResult<Value> {
        let mut reading = self.reading.clone();
        let mut resolved = notes.to_vec();
        for note in &mut resolved {
            note.operation_id = self.references.resolve(&note.operation_id)?;
            let tool = self
                .results
                .get(&note.operation_id)
                .map(|(request, _)| &request["tool"])
                .or_else(|| {
                    self.archived_results
                        .get(&note.operation_id)
                        .map(|entry| &entry["tool"])
                });
            if tool.is_some_and(|name| name == "run_python") {
                return Err("Python 結果不是閱讀來源；原文請用read_file閱讀後再記錄，計算結果請用register_report_values登記。不要重跑已成功的計算。".into());
            }
            // 舊 checkpoint 的 Excel 結果已成功，但當時沒有接入 Notebook。
            // 僅從通過封存校驗的原生結果補登記，不重開 Excel，也不依模型自報範圍。
            if !reading.pending.contains_key(&note.operation_id)
                && !reading.notes.contains_key(&note.operation_id)
            {
                if let Some((request, result)) = self.recorded_operation(&note.operation_id)? {
                    if matches!(
                        request["tool"].as_str(),
                        Some("read_excel_range" | "inspect_excel")
                    ) {
                        reading.observe_excel(&note.operation_id, &result)?;
                    }
                }
            }
        }
        let mut result = reading.accept(&resolved).map_err(|error| {
            let ids: Vec<_> = reading.pending.keys().chain(reading.notes.keys())
                .take(32).map(|id| self.references.alias(id)).collect();
            format!("{error} 目前可引用來源：{}。只能使用 reading.pending／notes 中的來源 ID；失敗工具自己的 reference_id 不是閱讀來源。若列表為空，無須補存閱讀筆記，可用 compact_context 保存工作摘要並繼續下一步，不要為取得 ID 重讀。", if ids.is_empty() { "無".into() } else { ids.join("、") })
        })?;
        self.reading = reading;
        self.references.project(&mut result);
        Ok(result)
    }

    /// 待整理階段只允許保存／補查筆記，非同步圖片路徑也必須遵守同一入口守衛。
    pub(in crate::projects) fn reading_gate(&self, tool: &Tool) -> Option<Value> {
        if self.reading.enabled
            && !self.reading.pending.is_empty()
            && !matches!(
                tool,
                Tool::RecordReadingNotes { .. }
                    | Tool::ReadReadingNotes { .. }
                    | Tool::ReadWorkLog { .. }
                    | Tool::LoadSkill { .. }
                    | Tool::ReadSkillGuide { .. }
            )
        {
            Some(
                json!({"ok":false,"error":"請先record_reading_notes保存pending來源，包含使用者要求相關事實、數字／單位、推論及缺項，再處理下一來源。"}),
            )
        } else {
            None
        }
    }

    pub(in crate::projects) fn project_reading_references(&self, value: &mut Value) {
        self.references.project(value);
    }

    pub(in crate::projects) fn reading_context(&self) -> Option<Value> {
        self.reading.context().map(|mut value| {
            self.references.project(&mut value);
            value
        })
    }

    pub(in crate::projects) fn reference_id(&self, id: &str) -> String {
        self.references.alias(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aliases_survive_restore_and_never_guess_hash_prefixes() {
        let mut refs = References::default();
        let first = "12345678aaaaaaaaaaaaaaaaaaaaaaaa";
        let second = "12345678bbbbbbbbbbbbbbbbbbbbbbbb";
        assert_eq!(refs.register(first), "op_0001");
        assert_eq!(refs.register(second), "op_0002");
        let mut restored: References = serde_json::from_value(json!(refs)).unwrap();
        assert_eq!(restored.register(first), "op_0001");
        assert_eq!(restored.resolve("op_0002").unwrap(), second);
        assert_eq!(restored.resolve("12345678").unwrap(), "12345678");
        assert_eq!(restored.register("third"), "op_0003");
        let mut note = json!({"operation_id":first,"summary":first});
        restored.project(&mut note);
        assert_eq!(note["operation_id"], "op_0001");
        assert_eq!(note["summary"], first);
        restored.register("op_0001");
        assert!(restored.resolve("op_0001").is_err());
    }
}

#[cfg(test)]
mod python_note_tests {
    use super::*;
    #[test]
    fn live_and_archived_python_aliases_give_actionable_guidance_without_mutation() {
        let mut broker = Broker::new(
            Project {
                id: "p".into(),
                name: "p".into(),
                root: std::env::current_dir().unwrap(),
                imports: BTreeMap::new(),
            },
            "r".into(),
        )
        .unwrap();
        let alias = broker.references.register("python-operation");
        broker.results.insert(
            "python-operation".into(),
            (json!({"tool":"run_python"}), json!({"ok":true})),
        );
        let notes = [crate::projects::reading::Digest {
            operation_id: alias,
            summary: "121筆".into(),
            relationships: String::new(),
            open_questions: String::new(),
        }];
        for archived in [false, true] {
            if archived {
                broker.results.clear();
                broker.archived_results.insert(
                    "python-operation".into(),
                    json!({"tool":"run_python","ok":true}),
                );
            }
            let before = serde_json::to_value(&broker.reading).unwrap();
            let error = broker.record_reading_notes(&notes).unwrap_err();
            assert!(error.contains("read_file") && error.contains("register_report_values"));
            assert_eq!(before, serde_json::to_value(&broker.reading).unwrap());
        }
        assert!(include_str!("../skills/python-analysis.md")
            .contains("kind=text的衍生輸出不是閱讀來源"));
    }
}

#[cfg(test)]
mod excel_note_tests {
    use super::*;
    fn broker() -> Broker {
        let root =
            std::env::temp_dir().join(format!("excel-notes-{}", crate::jobs::new_id().unwrap()));
        fs::create_dir(&root).unwrap();
        let mut broker = Broker::new(
            Project {
                id: "p".into(),
                name: "p".into(),
                root,
                imports: BTreeMap::new(),
            },
            "run".into(),
        )
        .unwrap();
        broker.enable_memory("chat").unwrap();
        broker.enable_reading_notes();
        broker
    }
    fn outcome() -> Value {
        json!({"ok":true,"result":{"path":"book.xlsx","revision":format!("excel:{}", "a".repeat(64)),
            "sheet":1,"sheet_name":"資料","columns":["L","M"],"headers":[],"header_row":6,
            "rows":[{"row":7,"cells":[{"value":1.2345678901234567e-21}]}],"next_row":8,
            "scope":"僅本次選取欄列"}})
    }
    fn digest(id: String) -> crate::projects::reading::Digest {
        crate::projects::reading::Digest {
            operation_id: id,
            summary: "L/M 第7列數值已讀；頻率未知".into(),
            relationships: String::new(),
            open_questions: String::new(),
        }
    }
    #[test]
    fn successful_excel_alias_is_accepted_after_archive_without_rereading() {
        let mut broker = broker();
        let id = "excel-read";
        broker.references.register(id);
        // 模擬舊 checkpoint：有成功原始結果但尚未登記 Notebook。
        broker
            .results
            .insert(id.into(), (json!({"tool":"read_excel_range"}), outcome()));
        broker.archive_results().unwrap();
        let note = digest(broker.reference_id(id));
        assert_eq!(broker.record_reading_notes(&[note]).unwrap()["saved"], 1);
        let source = &broker.reading.notes[id].source;
        assert_eq!(source.evidence["first_row"], 7);
        assert_eq!(source.evidence["row_count"], 1);
        assert_eq!(source.evidence["columns"], json!(["L", "M"]));
        assert!(broker.reading.pending.is_empty());
    }
    #[test]
    fn excel_old_and_compact_archive_notes_and_analysis_keep_exact_evidence() {
        for compact in [false, true] {
            let mut broker = broker();
            let mut result = outcome();
            let pointer = if compact {
                result["result"]["format"] = json!("excel-compact-v1");
                result["result"]["headers"] = json!(["值", "量"]);
                result["result"]["rows"] = json!([[7, 1.2345678901234567e-21, 2.0]]);
                "/rows/0/1"
            } else {
                "/rows/0/cells/0/value"
            };
            let id = "excel-evidence";
            broker.references.register(id);
            broker.reading.observe_excel(id, &result).unwrap();
            assert_eq!(broker.reading.pending[id].evidence["first_row"], 7);
            assert_eq!(broker.reading.pending[id].evidence["last_row"], 7);
            broker.results.insert(
                id.into(),
                (json!({"tool":"read_excel_range"}), result.clone()),
            );
            broker.archive_results().unwrap();
            let (_, recalled) = broker.recorded_operation(id).unwrap().unwrap();
            assert_eq!(recalled, result);
            let report = serde_json::from_value(json!({"goal":"核對","current_step":"引用",
                "open_questions":[],"superseded":[],"checks":[],"method":null,
                "findings":[{"claim":"原始值","status":"confirmed","evidence":[{"operation_id":id,"pointer":pointer}]}]})).unwrap();
            assert!(broker.record_analysis(&report).is_ok());
            assert_eq!(
                crate::projects::analysis::select(&recalled, pointer).unwrap(),
                json!(1.2345678901234567e-21)
            );
            let saved = broker.saved().unwrap();
            let mut restored = Broker::new(broker.project.clone(), "restore".into()).unwrap();
            restored.enable_memory("chat").unwrap();
            restored.restore(saved, &AtomicBool::new(false)).unwrap();
            assert_eq!(restored.recorded_operation(id).unwrap().unwrap().1, result);
        }
    }
    #[test]
    fn wrong_or_failed_alias_does_not_replace_valid_source_or_mutate_notes() {
        let mut broker = broker();
        let good = "successful";
        let bad = "failed-note";
        let good_alias = broker.references.register(good);
        let bad_alias = broker.references.register(bad);
        broker
            .results
            .insert(good.into(), (json!({"tool":"read_excel_range"}), outcome()));
        broker.results.insert(
            bad.into(),
            (json!({"tool":"record_reading_notes"}), json!({"ok":false})),
        );
        let before = serde_json::to_value(&broker.reading).unwrap();
        let error = broker
            .record_reading_notes(&[digest(good_alias.clone()), digest(bad_alias)])
            .unwrap_err();
        assert!(error.contains(&good_alias) && error.contains("失敗工具自己的 reference_id"));
        assert_eq!(before, serde_json::to_value(&broker.reading).unwrap());
        broker.record_reading_notes(&[digest(good_alias)]).unwrap();
        assert_eq!(broker.reading.notes.len(), 1);
    }
}
