//! 成果筆記入口：來源驗證、局部回查與技能階段共用 broker 的授權與操作紀錄。
use super::*;
use crate::projects::mail_notes::{Digest, Disposition, Page, Stage};

impl Broker {
    pub(in crate::projects) fn mail_note_context(&self) -> AppResult<String> {
        if self.mail_notes.pending.is_none() && self.mail_notes.notes.is_empty() {
            return Ok(String::new());
        }
        self.outlook.require_consent()?;
        Ok(format!("{}\n每封讀完立即產生可帶入的完整句子，不能以『稍後整理』代替。unwritten_count>=4時，先用set_work_stage(write,note_ids,reason)把目前條目寫入週報／工作日誌副本，再讀下一批。selected_drafts 是可直接使用的文字；疑點只定向查本機source，不重掃Outlook。郵件整理草稿是中間成果，不能當成使用者要求的週報已完成。", self.mail_notes.instructions()))
    }
    /// 僅待摘要回合增加小字串欄位；詳細格式在 context 說一次，避免每工具複製大型 schema。
    pub(in crate::projects) fn mail_note_schema(&self, request: &mut Value) {
        for tool in request["tools"].as_array_mut().into_iter().flatten() {
            let lookup = tool["function"]["name"] == "read_mail_notes";
            let update = lookup || tool["function"]["name"] == "set_work_stage";
            if self.mail_notes.pending.is_none() && !update {
                continue;
            }
            let schema = &mut tool["function"]["parameters"];
            schema["properties"]["mail_note"] = if self.mail_notes.pending.is_some() && !lookup {
                json!({"type":"string","description":"依郵件成果契約提供摘要JSON字串"})
            } else {
                json!({"type":["string","null"],"description":"修訂摘要JSON字串；不修改用null"})
            };
            if let Some(required) = schema["required"].as_array_mut() {
                required.push(json!("mail_note"));
            }
        }
    }
    pub(in crate::projects) fn accept_mail_note(
        &mut self,
        digest: Option<Digest>,
        decision: &super::super::Decision,
        required: bool,
    ) -> AppResult<()> {
        let lookup = matches!(
            decision,
            super::super::Decision::Tool {
                request: Tool::ReadMailNotes { .. },
                ..
            }
        );
        let Some(digest) = digest else {
            if required && self.mail_notes.pending.is_some() && !lookup {
                return Err("請在下一操作附上 mail_note；先保存剛讀完的摘要與待寫草稿。原文不足時用 read_mail_notes(mode=source) 查回。".into());
            }
            return Ok(());
        };
        self.outlook.require_consent()?;
        let (tool, result) = self
            .recorded_operation(&digest.source_operation)?
            .ok_or("郵件摘要來源操作不存在。")?;
        if tool["tool"] != "outlook_read" {
            return Err("郵件摘要來源必須是已成功的 outlook_read。".into());
        }
        let page = Page::from_result(&digest.source_operation, &result)?;
        let previous = self.mail_notes.clone();
        self.mail_notes.accept(digest, page)?;
        if let Err(error) = self.persist_mail_draft() {
            self.mail_notes = previous;
            return Err(error);
        }
        Ok(())
    }

    /// 每次成功接收摘要即更新同一份 TXT。內容為模型草稿，保留来源與待確認標示。
    fn persist_mail_draft(&mut self) -> AppResult<()> {
        let mut text =
            String::from("郵件整理草稿（尚未完成週報／工作日誌；模型整理，需依來源核對）\r\n\r\n");
        for (id, note) in &self.mail_notes.notes {
            let mail = &note.pages[0].mail;
            text.push_str(&format!("筆記：{id}\r\n主旨：{}\r\n日期：{}\r\n判定：{:?}；來源已完整讀取：{}；待重新核對：{}\r\n摘要：{}\r\n可帶入文字：{}\r\n理由：{}\r\n待確認：{}\r\n\r\n",
                mail["subject"].as_str().unwrap_or(""), mail["sent_at"].as_str().unwrap_or(""), note.digest.disposition,
                note.complete(), note.stale, note.digest.summary, note.digest.draft, note.digest.rationale, note.digest.open_questions));
        }
        let previous = self.mail_draft.clone();
        let draft = self.write_draft(
            "郵件整理草稿.txt",
            previous.as_ref(),
            &text::encode(&text, Encoding::Utf8(true))?,
        )?;
        self.mail_notes.draft_path = Some(draft.path.clone());
        self.mail_draft = Some(draft);
        Ok(())
    }
    /// 查回已讀原文只走加密本地操作檔；焦點由模型明確提供，不重新連線 Outlook。
    fn mail_source(&self, operation: &str, offset: usize, focus: &str) -> AppResult<Value> {
        if focus.trim().is_empty() || focus.chars().count() > 300 {
            return Err("回查原文需提供 ≤300 字的核對焦點。".into());
        }
        let (request, result) = self
            .recorded_operation(operation)?
            .ok_or("找不到已保存的原文操作。")?;
        if request["tool"] != "outlook_read" {
            return Err("只能查回已讀郵件原文。".into());
        }
        let page = Page::from_result(operation, &result)?;
        let text = result["result"]["text"].as_str().ok_or("來源沒有內文。")?;
        if offset > text.chars().count() {
            return Err("原文頁內位置超出範圍。".into());
        }
        let excerpt: String = text.chars().skip(offset).take(3000).collect();
        let next = offset + excerpt.chars().count();
        Ok(
            json!({"source_operation":operation,"source":page,"focus":focus,"text":excerpt,"offset":offset,"next_offset":next,"has_more":next<text.chars().count(),"local_snapshot":true}),
        )
    }
    pub(super) fn read_mail_notes(
        &self,
        mode: &str,
        ids: &[String],
        offset: usize,
        focus: &str,
    ) -> AppResult<Value> {
        self.outlook.require_consent()?;
        if ids.len() > 4 {
            return Err("每次最多讀取四份郵件筆記。".into());
        }
        if mode == "source" {
            if ids.len() != 1 {
                return Err("source 請在 note_ids 指定一個已讀來源 operation_id（索引內 pages.operation）。".into());
            }
            return self.mail_source(&ids[0], offset, focus);
        }
        let notes: Vec<_> = if ids.is_empty() {
            self.mail_notes.notes.iter().collect()
        } else {
            ids.iter()
                .map(|id| {
                    self.mail_notes
                        .notes
                        .get_key_value(id)
                        .ok_or("郵件筆記代號不存在。".to_owned())
                })
                .collect::<AppResult<_>>()?
        };
        if offset > notes.len() {
            return Err("筆記位置超出索引。".into());
        }
        let limit = match mode {
            "index" => 20,
            "notes" => 4,
            _ => return Err("mode 只能是 index、notes 或 source。".into()),
        };
        let rows: Vec<_> = notes
            .iter()
            .skip(offset)
            .take(limit)
            .map(|(id, n)| {
                if mode == "index" {
                    n.index(id)
                } else {
                    json!({"source":n.index(id),"digest":n.digest,"pages":n.sources()})
                }
            })
            .collect();
        let next = offset + rows.len();
        Ok(
            json!({"notes":rows,"pending":self.mail_notes.pending,"offset":offset,"next_offset":next,"total":notes.len(),"has_more":next<notes.len(),"review_required":self.mail_notes.review_required}),
        )
    }
    pub(super) fn set_work_stage(
        &mut self,
        stage: Stage,
        ids: &[String],
        reason: &str,
    ) -> AppResult<Value> {
        if reason.trim().is_empty() || reason.chars().count() > 300 || ids.len() > 4 {
            return Err("切換階段需說明理由（≤300字），並最多選四份草稿。".into());
        }
        if self.mail_notes.pending.is_some() {
            return Err("切換前請先保存待摘要郵件。".into());
        }
        if !self.mail_notes.notes.is_empty() {
            self.outlook.require_consent()?;
        }
        for id in ids {
            let note = self
                .mail_notes
                .notes
                .get(id)
                .ok_or("選定的郵件筆記不存在。")?;
            if stage == Stage::Write
                && (note.stale || note.digest.disposition != Disposition::Include)
            {
                return Err(
                    "寫入只能選用目前版本且 include 的草稿；先查筆記處理疑點與舊版。".into(),
                );
            }
        }
        if stage == Stage::Write && !self.mail_notes.notes.is_empty() && ids.is_empty() {
            return Err("郵件寫入階段請選定 note_ids，讓下輪帶入待寫草稿。".into());
        }
        let skill = match stage {
            Stage::Read => "outlook-research",
            Stage::Organize => "notes",
            Stage::Write => "office-edit",
            Stage::Chart => "dataset-charts",
        };
        super::super::skills::activate(&mut self.loaded_skills, skill)?;
        self.mail_notes.stage = stage;
        self.mail_notes.selected = ids.to_vec();
        self.mail_notes.reread = if stage == Stage::Read {
            ids.to_vec()
        } else {
            vec![]
        };
        // 使用者更新需求後，明確重新選定階段與條目代表模型已重新檢視；不代表事實驗證。
        self.mail_notes.review_required = false;
        Ok(
            json!({"stage":stage,"loaded":self.loaded_skills,"selected":ids,"reason":reason,"notice":"草稿及來源仍保留；寫入成功以 Office／檔案工具結果為準，不能以階段當完成。"}),
        )
    }
    /// 同版本已讀區段先回筆記。只有 read 階段選定筆記並說明理由，才准一次定向回讀。
    pub(super) fn reuse_mail_note(
        &mut self,
        mail_id: &str,
        offset: usize,
    ) -> AppResult<Option<Value>> {
        self.outlook.require_consent()?;
        let revision = self.outlook.mail_revision(mail_id);
        let covered = self.mail_notes.notes.iter().find(|(_, n)| {
            !n.stale
                && n.pages.iter().any(|p| {
                    p.mail["mail_id"] == mail_id
                        && revision.as_deref() == p.mail["revision"].as_str()
                        && p.offset <= offset
                        && (offset < p.end || p.total == 0)
                })
        });
        if let Some((id, note)) = covered {
            if let Some(index) = self.mail_notes.reread.iter().position(|n| n == id) {
                self.mail_notes.reread.remove(index);
                return Ok(None);
            }
            return Ok(Some(
                json!({"reused_note":true,"source":note.index(id),"digest":note.digest,"pages":note.sources(),"notice":"此版本區段已有成果，未重新讀Outlook。核對原文請read_mail_notes(source,operation_id,focus)；確有需要才set_work_stage(read,[note_id],reason)定向回讀。"}),
            ));
        }
        Ok(None)
    }
    pub(in crate::projects) fn review_mail_notes(&mut self) {
        if !self.mail_notes.notes.is_empty() {
            self.mail_notes.review_required = true;
            self.mail_notes.selected.clear();
            self.mail_notes.reread.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::{mail_notes::tests::fixture, Decision};

    fn broker() -> Broker {
        Broker::new(
            Project {
                id: "p".into(),
                name: "p".into(),
                root: {
                    let path = std::env::current_dir()
                        .unwrap()
                        .join(".build")
                        .join(format!("mail-notes-{}", crate::jobs::new_id().unwrap()));
                    std::fs::create_dir_all(&path).unwrap();
                    path
                },
                imports: BTreeMap::new(),
            },
            "r".into(),
        )
        .unwrap()
    }
    #[test]
    fn same_turn_digest_binds_source_and_stage_keeps_drafts_without_outlook_tools() {
        let mut b = broker();
        b.outlook
            .set_consent(Some(Box::new(|_, _| Ok(Some(Default::default())))));
        b.outlook
            .authorize(&AtomicBool::new(false), std::time::Instant::now())
            .unwrap();
        let (digest, result) = fixture("read1", "v1", 0, 100);
        b.results.insert(
            "read1".into(),
            (
                json!({"tool":"outlook_read","mail_id":"m1","offset":0}),
                result.clone(),
            ),
        );
        b.mail_notes.observe("read1", &result).unwrap();
        let id = b.mail_notes.pending.as_ref().unwrap().id();
        let finish = Decision::Finish {
            message: "報告".into(),
            artifacts: vec![],
        };
        assert!(b.accept_mail_note(None, &finish, true).is_err());
        let lookup = Decision::Tool {
            operation_id: "next".into(),
            request: Tool::ReadMailNotes {
                mode: "source".into(),
                note_ids: vec!["read1".into()],
                offset: 0,
                focus: "主導者".into(),
            },
        };
        b.accept_mail_note(None, &lookup, true).unwrap();
        assert!(b
            .read_mail_notes("source", &["read1".into()], 0, "")
            .is_err());
        assert_eq!(
            b.read_mail_notes("source", &["read1".into()], 0, "日期")
                .unwrap()["text"],
            "信".repeat(100)
        );
        b.accept_mail_note(Some(digest.clone()), &finish, true)
            .unwrap();
        b.set_work_stage(Stage::Write, std::slice::from_ref(&id), "開始寫週報")
            .unwrap();
        assert!(b.mail_note_context().unwrap().contains(&digest.draft));
        let mut request = json!({"tools":[{"function":{"name":"outlook_read"}},{"function":{"name":"create_working_copy"}},{"function":{"name":"read_mail_notes"}}]});
        b.restrict_tools(&mut request);
        assert_eq!(request["tools"].as_array().unwrap().len(), 2);
        assert!(!request.to_string().contains("outlook_read"));
        let mut restored = Broker::new(b.project.clone(), "restored".into()).unwrap();
        restored
            .restore(b.saved().unwrap(), &AtomicBool::new(false))
            .unwrap();
        assert!(restored.mail_note_context().is_err()); // DPAPI 恢復不能恢復授權。
        assert!(restored.read_mail_notes("notes", &[], 0, "").is_err());
        b.review_mail_notes();
        assert!(b.mail_notes.review_required && b.mail_notes.selected.is_empty());
        assert_eq!(b.mail_notes.notes.len(), 1);
    }
    #[test]
    fn dynamic_schema_and_parser_keep_summary_separate_from_tool_arguments() {
        let mut b = broker();
        let (digest, result) = fixture("read1", "v1", 0, 100);
        b.mail_notes.observe("read1", &result).unwrap();
        let mut request = json!({"tools":[{"function":{"name":"list_files","parameters":{"type":"object","properties":{},"required":[]}}},{"function":{"name":"read_mail_notes","parameters":{"type":"object","properties":{},"required":[]}}}]});
        b.mail_note_schema(&mut request);
        assert_eq!(
            request["tools"][0]["function"]["parameters"]["properties"]["mail_note"]["type"],
            "string"
        );
        assert_eq!(
            request["tools"][1]["function"]["parameters"]["properties"]["mail_note"]["type"],
            json!(["string", "null"])
        );
        let reply = json!({"tool_calls":[{"id":"call1","type":"function","function":{"name":"list_files","arguments":json!({"path":"","mail_note":serde_json::to_string(&digest).unwrap()}).to_string()}}]});
        let crate::projects::reply::ParseOutcome::Operation(parsed) =
            crate::projects::reply::parse(&reply.to_string()).unwrap()
        else {
            panic!("reply rejected");
        };
        assert_eq!(parsed.mail_note.unwrap().source_operation, "read1");
        assert!(matches!(
            parsed.decision,
            Decision::Tool {
                request: Tool::ListFiles { .. },
                ..
            }
        ));
    }
}
