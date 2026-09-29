//! 分離模型說明與單一操作；工具仍須通過完整結構及 broker 授權檢查。
use super::Decision;
use crate::AppResult;

pub struct Parsed {
    pub decision: Decision,
    pub commentary: String,
    pub note: Option<String>,
}

/// None 表示可要求模型重新給出唯一操作；此時絕不執行任何候選 JSON。
/// 未知工具仍拒絕，不自動挑最後一個物件，也不改寫 JSON 字串內的全形字元。
pub fn parse(text: &str) -> AppResult<Option<Parsed>> {
    let text = text.trim();
    if text.len() > 64_000 {
        return Err("模型操作超過大小限制。".into());
    }
    let Some(start) = text.find('{') else {
        return Ok(None);
    };
    let prefix = &text[..start];
    if prefix.contains(['[', ']', '｛', '｝']) {
        return Ok(None);
    }
    let mut stream =
        serde_json::Deserializer::from_str(&text[start..]).into_iter::<serde_json::Value>();
    let Some(Ok(mut value)) = stream.next() else {
        return Ok(None);
    };
    let suffix = &text[start + stream.byte_offset()..];
    if suffix.contains(['{', '}', '[', ']', '｛', '｝']) {
        return Ok(None);
    }
    if value["action"] == "done" {
        return Ok(None);
    }
    if value["action"] == "tool" {
        let tool = value["request"]["tool"].as_str().unwrap_or("");
        if !matches!(
            tool,
            "list_files"
                | "read_file"
                | "find_text"
                | "create_working_copy"
                | "edit_text"
                | "edit_office"
                | "save_copy"
                | "delete_copy"
        ) {
            return Err("模型要求未知工具，已停止。".into());
        }
    }
    let note = match value
        .as_object_mut()
        .and_then(|v| v.remove("progress_note"))
    {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(note)) if note.chars().count() <= 2000 => Some(note),
        _ => return Ok(None),
    };
    let decision = match serde_json::from_value::<Decision>(value) {
        Ok(decision) => decision,
        Err(_) => return Ok(None),
    };
    match &decision {
        Decision::Finish { message, artifacts }
            if message.trim().is_empty() || (artifacts.is_empty() && empty_completion(message)) =>
        {
            return Ok(None)
        }
        Decision::AskUser { message } if message.trim().is_empty() => return Ok(None),
        _ => (),
    }
    let commentary = [prefix, suffix]
        .iter()
        .flat_map(|part| part.lines())
        .map(str::trim)
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with("```")
                && !matches!(*line, "工具：" | "詢問：" | "結果：" | "完成：")
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(Some(Parsed {
        decision,
        commentary,
        note,
    }))
}

/// 只攔下空白或裸完成標記，不用長度或關鍵字猜測正常短答案是否正確。
fn empty_completion(message: &str) -> bool {
    let message = message
        .trim()
        .trim_end_matches(['。', '.', '!', '！'])
        .trim()
        .to_ascii_lowercase();
    matches!(
        message.as_str(),
        "" | "done" | "[done]" | "ok" | "完成" | "已完成"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    const TOOL: &str = r#"{"action":"tool","operation_id":"op_001","request":{"tool":"read_file","path":"colab_download_models.txt","offset":0}}"#;
    #[test]
    fn executes_complete_wrapped_tool_and_preserves_commentary() {
        assert!(matches!(
            parse(TOOL).unwrap().unwrap().decision,
            Decision::Tool { .. }
        ));
        let parsed = parse(&format!("現在儲存副本。\n{TOOL}")).unwrap().unwrap();
        assert_eq!(parsed.commentary, "現在儲存副本。");
        assert!(parse(&format!("```json\n{TOOL}\n```"))
            .unwrap()
            .unwrap()
            .commentary
            .is_empty());
        assert!(parse("工具：{\"action\":\"tool\"").unwrap().is_none());
    }
    #[test]
    fn wrapped_questions_and_finish_are_valid_decisions() {
        let ask = r#"週報已讀取，但 PDF 無法解析。請協助匯入文字：
{"action":"ask_user","message":"兩篇 PDF 需要匯入。\n請提供文字。"}"#;
        let result = parse(ask).unwrap().unwrap();
        assert!(
            matches!(result.decision, Decision::AskUser { ref message } if message.contains("\n請提供文字"))
        );
        assert!(result.commentary.contains("週報已讀取"));
        let finish = parse(r#"已完成。{"action":"finish","message":"已整理","artifacts":[]}"#)
            .unwrap()
            .unwrap();
        assert!(matches!(finish.decision, Decision::Finish { .. }));
        assert!(parse(&format!("{ask}\n{TOOL}")).unwrap().is_none());
    }

    #[test]
    fn refuses_ambiguous_or_unknown_commands() {
        assert!(parse(&TOOL.replace("read_file", "shell")).is_err());
        for text in [format!("{TOOL}\n{TOOL}"), format!("[{TOOL}]")] {
            assert!(parse(&text).unwrap().is_none(), "{text}");
        }
    }
    #[test]
    fn empty_done_and_multiple_json_are_repaired_but_short_answers_survive() {
        for input in [
            "",
            "done",
            r#"{"action":"done"}"#,
            r#"{"action":"finish","message":"  ","artifacts":[]}"#,
            r#"{"action":"finish","message":"done","artifacts":[]}"#,
        ] {
            assert!(parse(input).unwrap().is_none(), "{input}");
        }
        for input in [
            r#"{"action":"finish","message":"42","artifacts":[]}"#,
            r#"{"action":"finish","message":"已完成","artifacts":["real_copy"]}"#,
        ] {
            assert!(parse(input).unwrap().is_some());
        }
        assert!(parse(&format!("我現在閱讀。{TOOL} 等等，重新輸出。{TOOL}"))
            .unwrap()
            .is_none());
        assert!(parse(&format!("｛不完整｝ {TOOL}")).unwrap().is_none());
        let mut value: serde_json::Value = serde_json::from_str(TOOL).unwrap();
        value["progress_note"] = serde_json::json!("已確認來源與待辦");
        assert_eq!(
            parse(&value.to_string()).unwrap().unwrap().note.as_deref(),
            Some("已確認來源與待辦")
        );
        value["progress_note"] = serde_json::json!("字".repeat(2001));
        assert!(parse(&value.to_string()).unwrap().is_none());
    }
}
