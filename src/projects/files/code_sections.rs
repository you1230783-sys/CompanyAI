//! 版本綁定的小段編輯，不要求模型在修改時重送整段舊程式碼。
use super::*;

/// 區段代號只引用實際讀回的範圍；不能讓模型自行配對行號與另一段雜湊。
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct Section {
    id: String,
    revision: String,
    first_line: usize,
    last_line: usize,
    text: String,
    #[serde(default)]
    touched: u64,
}

fn section_id(copy: &str, revision: &str, first: usize, last: usize) -> String {
    text::revision(&format!("code-section:{copy}:{revision}:{first}:{last}"))
}

/// 狀態由真實副本／驗證紀錄推導，不採用模型的「已完成」描述。
pub(super) fn code_state(copy: &Copy) -> Value {
    let revision = text::revision(&copy.text);
    let syntax = copy.python_checked_revision.as_deref() == Some(&revision);
    let failed = copy
        .code_review
        .tests
        .iter()
        .any(|t| t["status"] == "failed");
    let next = if copy.saved_revision.as_deref() == Some(&revision) {
        "已發布，可交付"
    } else if copy.code_review.requirements.is_empty() {
        "plan_code_change保存需求"
    } else if !syntax {
        "完成相關小段後check_python；草稿已保存不等於驗證完成"
    } else if failed {
        "依失敗測試只修相關段，重測相同test id"
    } else if copy.code_review.inspected_revision.as_deref() != Some(&revision) {
        "review_code_change(checks=[])核對真實差異"
    } else if copy.code_review.tests.is_empty() {
        "直接test_python提交一組最小需求測試；缺細節只補相應小段"
    } else if copy.code_review.reviewed_revision.as_deref() != Some(&revision) {
        "補必要測試，review_code_change逐項核對；缺環境明示unverified"
    } else {
        "save_copy完成交付"
    };
    json!({"draft_saved":copy.draft.is_some(),"published":copy.saved_revision.as_deref()==Some(&revision),"syntax_checked":syntax,"total_lines":copy.text.split_inclusive('\n').count(),"next_action":next,"tests":copy.code_review.tests.iter().map(|t|json!({"id":t["id"],"status":t["status"],"failures":t["failures"]})).collect::<Vec<_>>()})
}

/// 保留唯一且逐字未變的小段；只有位置與內容都可核對時才更新代號。
pub(super) fn refresh_sections(id: &str, copy: &mut Copy) {
    let revision = text::revision(&copy.text);
    copy.code_sections.retain_mut(|item| {
        if item.text.is_empty() {
            return false;
        }
        let mut matches = copy.text.match_indices(&item.text);
        let Some((start, _)) = matches.next() else {
            return false;
        };
        if matches.next().is_some()
            || (start > 0 && copy.text.as_bytes()[start - 1] != b'\n')
            || (start + item.text.len() < copy.text.len() && !item.text.ends_with('\n'))
        {
            return false;
        }
        item.first_line = copy.text[..start].bytes().filter(|b| *b == b'\n').count() + 1;
        item.last_line =
            item.first_line + item.text.split_inclusive('\n').count().saturating_sub(1);
        item.revision.clone_from(&revision);
        item.id = section_id(id, &revision, item.first_line, item.last_line);
        true
    });
}

fn read_bounds(content: &str, first: usize, requested_last: usize) -> AppResult<(usize, usize)> {
    let lines: Vec<_> = content.split_inclusive('\n').collect();
    if first == 0 || requested_last < first || first > lines.len() + 1 {
        return Err(format!("程式區段範圍無效：要求{first}–{requested_last}，目前共{}行；讀取請使用1–{}，檔尾插入為{}。", lines.len(), lines.len().max(1), lines.len()+1));
    }
    let mut last = first;
    let mut length = 0;
    for (offset, line) in lines
        .iter()
        .skip(first - 1)
        .take(requested_last.saturating_sub(first) + 1)
        .take(200)
        .enumerate()
    {
        let chars = line.chars().count();
        if length + chars > 6000 {
            if offset == 0 {
                return Err(format!(
                    "程式區段第{first}行單行超過6000字；請用read_file分頁閱讀，勿以截斷原文修改。"
                ));
            }
            break;
        }
        length += chars;
        last = first + offset;
    }
    Ok((first, last))
}

/// 行號從1開始、尾行包含在內；空檔只有第1個可插入位置。
fn section(content: &str, first: usize, last: usize) -> AppResult<(usize, usize)> {
    let lines: Vec<_> = content.split_inclusive('\n').collect();
    if first == 0
        || last < first
        || last - first >= 200
        || first > lines.len() + 1
        || last > lines.len().max(first)
    {
        return Err("請指定有效的1起算行號，每段最多200行；在檔尾插入使用總行數+1。".into());
    }
    let start = lines.iter().take(first - 1).map(|s| s.len()).sum();
    let end = lines.iter().take(last).map(|s| s.len()).sum();
    Ok((start, end))
}

impl Broker {
    /// 草稿路徑只是同一工作副本的別名；原件路徑不會被默默改指向副本。
    pub(super) fn code_copy_id(&self, path: &str) -> Option<String> {
        if self.copies.contains_key(path) {
            return Some(path.into());
        }
        self.copies
            .iter()
            .find(|(_, copy)| {
                copy.draft.as_ref().is_some_and(|d| {
                    d.path
                        .replace('\\', "/")
                        .eq_ignore_ascii_case(&path.replace('\\', "/"))
                })
            })
            .map(|(id, _)| id.clone())
    }

    /// 批次子程序回傳後也由主 broker 登記；最多保存六段、合計12000字。
    pub(super) fn retain_code_section(&mut self, result: &Value) {
        let Some(path) = result["path"].as_str() else {
            return;
        };
        let Some(id) = self.code_copy_id(path) else {
            return;
        };
        let touched = self
            .copies
            .values()
            .flat_map(|c| &c.code_sections)
            .map(|s| s.touched)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        let Some(copy) = self.copies.get_mut(&id) else {
            return;
        };
        let Some(value) = result["text"].as_str() else {
            return;
        };
        let revision = text::revision(&copy.text);
        if result["revision"] != revision {
            return;
        }
        let first = result["first_line"].as_u64().unwrap_or(0) as usize;
        let last = result["last_line"].as_u64().unwrap_or(0) as usize;
        let Ok((a, b)) = section(&copy.text, first, last) else {
            return;
        };
        if &copy.text[a..b] != value {
            return;
        }
        let key = section_id(&id, &revision, first, last);
        copy.code_sections
            .retain(|s| s.first_line != first || s.last_line != last);
        copy.code_sections.push(Section {
            id: key,
            revision,
            first_line: first,
            last_line: last,
            text: value.into(),
            touched,
        });
        while copy.code_sections.len() > 6
            || copy
                .code_sections
                .iter()
                .map(|s| s.text.chars().count())
                .sum::<usize>()
                > 12000
        {
            copy.code_sections.remove(0);
        }
    }

    pub(super) fn replace_code_section(
        &mut self,
        id: &str,
        reference: &str,
        replacement: &str,
    ) -> AppResult<Value> {
        let copy = self.copies.get(id).ok_or("不是本次工作的副本。")?;
        let saved = copy
            .code_sections
            .iter()
            .find(|s| s.id == reference)
            .cloned()
            .ok_or("程式區段代號已失效或不屬於此副本；只需重讀要修改的小段。")?;
        self.edit_code_section(
            id,
            &saved.revision,
            saved.first_line,
            saved.last_line,
            &text::revision(&saved.text),
            replacement,
        )
    }

    /// 固定放在本輪工作資料中，與工具歷史分開；只有目前版本原文可附給模型。
    pub(crate) fn code_work_context(&self) -> Option<Value> {
        if !self.loaded_skills.iter().any(|id| id == "python-edit") {
            return None;
        }
        let mut remaining = 10000usize;
        let mut active = Vec::new();
        let mut copies: Vec<_> = self
            .copies
            .iter()
            .filter(|(_, c)| c.name.to_ascii_lowercase().ends_with(".py"))
            .collect();
        copies.sort_by_key(|(_, c)| {
            std::cmp::Reverse(c.code_sections.iter().map(|s| s.touched).max().unwrap_or(0))
        });
        for (id, copy) in copies.into_iter().take(2) {
            let revision = text::revision(&copy.text);
            let mut sections = Vec::new();
            for item in copy
                .code_sections
                .iter()
                .rev()
                .filter(|s| s.revision == revision)
            {
                let length = item.text.chars().count();
                if length > remaining {
                    continue;
                }
                remaining -= length;
                sections.push(json!({"section_id":item.id,"first_line":item.first_line,"last_line":item.last_line,"text":item.text}));
            }
            active.push(json!({"copy_id":id,"revision":revision,"sections":sections,"state":code_state(copy),"instruction":"以上是目前副本原文；足夠時直接修改或提交一組最小test_python，不為取得範例重讀整份程式。預期值依使用者需求，不能將實作值直接當正解。"}));
        }
        (!active.is_empty()).then(|| json!({"active_code":active,"test_example":"class Behavior(unittest.TestCase):\n    def test_requirement(self):\n        target = load_target()\n        self.assertEqual(target.convert(0), 0)\n# convert/期望值只是格式示意，必須改成實際需求；從一组測試開始。", "test_contract":"工具已提供unittest、mock、workspace、load_target(argv=None,as_main=False)。先載入真實副本，外部依賴模擬需列mocked_dependencies；不必反覆查指南。"}))
    }

    /// 只縮減本輪送出投影：已固定帶入的同一區段不再於工具結果重複送全文。
    /// 逐字比對及代號都相符才替換；原始工具紀錄／checkpoint完全保留。
    pub(crate) fn project_code_reads(
        messages: &mut [super::super::agent::Message],
        context: &Value,
    ) {
        let mut pinned = BTreeMap::new();
        for copy in context["active_code"].as_array().into_iter().flatten() {
            for section in copy["sections"].as_array().into_iter().flatten() {
                if let (Some(id), Some(text)) =
                    (section["section_id"].as_str(), section["text"].as_str())
                {
                    pinned.insert(id, text);
                }
            }
        }
        fn replace(value: &mut Value, pinned: &BTreeMap<&str, &str>, depth: usize) {
            if depth > 8 {
                return;
            }
            if let (Some(id), Some(text)) = (value["section_id"].as_str(), value["text"].as_str()) {
                if pinned.get(id).is_some_and(|stored| *stored == text) {
                    let reference = format!("active_code.sections中section_id={id}的完整原文");
                    if let Some(object) = value.as_object_mut() {
                        object.remove("text");
                        object.insert("text_ref".into(), json!(reference));
                    }
                }
            }
            match value {
                Value::Object(values) => {
                    for item in values.values_mut() {
                        replace(item, pinned, depth + 1);
                    }
                }
                Value::Array(values) => {
                    for item in values {
                        replace(item, pinned, depth + 1);
                    }
                }
                _ => (),
            }
        }
        for message in messages.iter_mut().filter(|m| m.role == "tool") {
            if let Ok(mut value) = serde_json::from_str::<Value>(&message.content) {
                replace(&mut value, &pinned, 0);
                message.content = value.to_string();
            }
        }
    }
    pub(super) fn read_code_section(
        &mut self,
        path: &str,
        first: usize,
        last: usize,
        cancel: &AtomicBool,
        worker: &mut Worker,
    ) -> AppResult<Value> {
        let canonical = self.code_copy_id(path).unwrap_or_else(|| path.into());
        let name = self
            .copies
            .get(&canonical)
            .map(|c| c.name.as_str())
            .unwrap_or(path);
        if extension(Path::new(name))? != "py" {
            return Err("程式區段工具目前只支援PY。".into());
        }
        let content = self.content(&canonical, cancel, worker)?;
        let (first, actual_last) = read_bounds(&content, first, last)?;
        let (start, end) = section(&content, first, actual_last)?;
        let value = &content[start..end];
        let copy_id = self.code_copy_id(path);
        let revision = text::revision(&content);
        let result = json!({"path":canonical,"copy_id":copy_id,"source_kind":if copy_id.is_some(){"working_copy"}else{"original_read_only"},"revision":revision,"first_line":first,"last_line":actual_last,
            "requested_last_line":last,"next_line":(end<content.len()).then_some(actual_last+1),"truncated":actual_last<last && end<content.len(),"eof":end==content.len(),
            "section_id":copy_id.as_ref().map(|id|section_id(id,&revision,first,actual_last)),
            "total_lines":content.split_inclusive('\n').count(),"section_hash":text::revision(value),"text":value,
            "offset":content[..start].chars().count(),"next_offset":content[..end].chars().count(),"total":content.chars().count(),
            "guidance":"副本修改優先傳copy_id、section_id、replacement。若用舊介面，範圍須等於實際first_line/last_line，不能拿大段hash改小段。原件只供對照；修改與測試讀copy_id。"});
        self.retain_code_section(&result);
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn edit_code_section(
        &mut self,
        id: &str,
        revision: &str,
        first: usize,
        last: usize,
        hash: &str,
        replacement: &str,
    ) -> AppResult<Value> {
        let copy = self.copies.get(id).ok_or("不是本次工作的副本。")?;
        if extension(Path::new(&copy.name))? != "py" || text::revision(&copy.text) != revision {
            return Err("PY副本版本不符；只需重新讀取要修改的小段，不重讀全文。".into());
        }
        if replacement.chars().count() > 6000 {
            return Err(
                "單段新程式碼最多6000字；先完成一個函式或一組imports，再修改下一段。".into(),
            );
        }
        let (start, end) = section(&copy.text, first, last)?;
        if text::revision(&copy.text[start..end]) != hash {
            return Err(
                "區段雜湊不符，未修改；請用read_code_section取得正確行號與section_hash。".into(),
            );
        }
        let replacement = if copy.text.contains("\r\n") {
            replacement.replace("\r\n", "\n").replace('\n', "\r\n")
        } else {
            replacement.into()
        };
        let next = format!(
            "{}{}{}",
            &copy.text[..start],
            replacement,
            &copy.text[end..]
        );
        text::validate(&next)?;
        let mut result = self.save_python_draft(id, &next)?;
        // 新寫入段落也屬於正在驗證的原文；修改後不需再讀一次才可寫測試。
        let lines = replacement.split_inclusive('\n').count();
        if lines > 0 && lines <= 200 {
            let copy = self.copies.get(id).ok_or("工作副本不存在。")?;
            if let Ok((a, b)) = section(&copy.text, first, first + lines - 1) {
                let retained = json!({"path":id,"revision":text::revision(&copy.text),"first_line":first,"last_line":first+lines-1,"text":&copy.text[a..b]});
                self.retain_code_section(&retained);
            }
        }
        result["state"] = code_state(self.copies.get(id).ok_or("工作副本不存在。")?);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pinned_projection_deduplicates_exact_text_without_mutating_history() {
        use super::super::super::agent::Message;
        let value =
            json!({"ok":true,"result":{"section_id":"ref","text":"精確原文","revision":"v"}});
        let mut original = Message::user(&value.to_string());
        original.role = "tool".into();
        let mut messages = vec![original.clone()];
        let context =
            json!({"active_code":[{"sections":[{"section_id":"ref","text":"精確原文"}]}]});
        Broker::project_code_reads(&mut messages, &context);
        assert!(messages[0].content.contains("text_ref"));
        assert!(original.content.contains("精確原文"));
        let different =
            json!({"active_code":[{"sections":[{"section_id":"ref","text":"不同原文"}]}]});
        let mut messages = vec![original.clone()];
        Broker::project_code_reads(&mut messages, &different);
        assert_eq!(messages[0].content, original.content);
    }
    #[test]
    fn sections_preserve_crlf_unicode_and_eof() {
        let text = "甲\r\ndef f():\r\n    pass\r\n";
        let (a, b) = section(text, 2, 3).unwrap();
        assert_eq!(&text[a..b], "def f():\r\n    pass\r\n");
        assert_eq!(section(text, 4, 4).unwrap(), (text.len(), text.len()));
        assert_eq!(section("", 1, 1).unwrap(), (0, 0));
        assert!(section(text, 0, 1).is_err());
        assert!(section(text, 1, 201).is_err());
    }

    #[test]
    fn reads_page_whole_lines_and_clip_eof_but_edits_remain_exact() {
        let source = "1234567890\n".repeat(429);
        assert_eq!(read_bounds(&source, 330, 470).unwrap(), (330, 429));
        assert!(section(&source, 330, 470).is_err());
        let large = format!("{}\n", "中".repeat(80)).repeat(200);
        assert_eq!(read_bounds(&large, 1, 200).unwrap(), (1, 74));
        assert!(read_bounds(&source, 500, 501).unwrap_err().contains("429"));
    }
}
