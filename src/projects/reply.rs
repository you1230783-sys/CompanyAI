//! 分離模型說明與單一操作；工具仍須通過完整結構及 broker 授權檢查。
use super::Decision;
use crate::AppResult;

pub struct Parsed {
    pub decision: Decision,
    pub commentary: String,
    pub note: Option<String>,
    pub mail_note: Option<super::mail_notes::Digest>,
    pub task_summary: Option<String>,
}

/// 區分可修正的格式問題與可執行操作，讓 runner 回傳明確原因而不猜測工具。
pub enum ParseOutcome {
    Operation(Box<Parsed>),
    Repair(&'static str),
}
const INVALID_OPERATION: &str = "模型未回傳唯一有效操作或實際完成正文。";

/// Repair 只要求模型重新提供完整操作，絕不執行候選 JSON。
/// 非空但不支援的工具名称仍回傳 Err，維持停止處理的界線。
pub fn parse(text: &str) -> AppResult<ParseOutcome> {
    let text = text.trim();
    if text.len() > 64_000 {
        return Err("模型操作超過大小限制。".into());
    }
    let Some(start) = text.find('{') else {
        return Ok(ParseOutcome::Repair(INVALID_OPERATION));
    };
    let prefix = &text[..start];
    if prefix.contains(['[', ']', '｛', '｝']) {
        return Ok(ParseOutcome::Repair(INVALID_OPERATION));
    }
    let mut stream =
        serde_json::Deserializer::from_str(&text[start..]).into_iter::<serde_json::Value>();
    let Some(Ok(mut value)) = stream.next() else {
        return Ok(ParseOutcome::Repair(INVALID_OPERATION));
    };
    let suffix = &text[start + stream.byte_offset()..];
    if suffix.contains(['{', '}', '[', ']', '｛', '｝']) {
        return Ok(ParseOutcome::Repair(INVALID_OPERATION));
    }
    // 新協定與舊協定共用下方 Decision 驗證；不將含多個操作的內容拆開執行。
    let mut content = String::new();
    if value.get("tool_calls").is_some() {
        match super::tool_calls::convert(value)? {
            super::tool_calls::Conversion::Operation {
                value: converted,
                commentary,
            } => {
                value = converted;
                content = commentary;
            }
            super::tool_calls::Conversion::Repair(reason) => {
                return Ok(ParseOutcome::Repair(reason));
            }
        }
    }
    if value["action"] == "done" {
        return Ok(ParseOutcome::Repair(INVALID_OPERATION));
    }
    if value["action"] == "tool" {
        // 欄位漏寫／型別錯誤是格式問題；只有明確指定不支援的名稱才算未知工具。
        let Some(request) = value.get("request").and_then(serde_json::Value::as_object) else {
            return Ok(ParseOutcome::Repair(
                "舊 action=tool 的 request 必須是 JSON 物件，並包含 request.tool。請依目前 tool_calls 契約重新提供同一操作，使用 function.name 指定工具，將原 operation_id 保留為 id。",
            ));
        };
        let Some(tool_value) = request.get("tool") else {
            return Ok(ParseOutcome::Repair(
                "缺少必要欄位 request.tool（工具名稱）。本次工具尚未執行；請改用 tool_calls，在 function.name 填入原本工具名稱，將 operation_id 原樣作為 id，request 其餘參數放入 function.arguments。",
            ));
        };
        let Some(tool) = tool_value.as_str().filter(|name| !name.trim().is_empty()) else {
            return Ok(ParseOutcome::Repair(
                "request.tool 必須是非空字串。請依目前 tool_calls 契約，在 function.name 填入原本工具名稱，將 operation_id 原樣作為 id，重新提供同一操作。",
            ));
        };
        if matches!(tool, "finish" | "ask_user") || !super::tool_calls::known_function(tool)? {
            return Err("模型要求未知工具，已停止。".into());
        }
    }
    let mail_note = match value.as_object_mut().and_then(|v| v.remove("mail_note")) {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(text)) if text.chars().count() <= 3500 => match serde_json::from_str::<super::mail_notes::Digest>(&text) {
            Ok(note) => Some(note),
            Err(_) => return Ok(ParseOutcome::Repair("mail_note 必須是符合郵件成果契約的 JSON 字串，包含 source_operation、summary、draft、disposition、rationale、open_questions。")),
        },
        _ => return Ok(ParseOutcome::Repair("mail_note 必須是 ≤3500 字的 JSON 字串。")),
    };
    let note = match value
        .as_object_mut()
        .and_then(|v| v.remove("progress_note"))
    {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(note)) if note.chars().count() <= 2000 => Some(note),
        _ => return Ok(ParseOutcome::Repair(INVALID_OPERATION)),
    };
    // 完成摘要附在同一個回覆，不額外啟動模型呼叫；舊模型未提供時保留明確標示的節錄。
    let task_summary = match value.as_object_mut().and_then(|v| v.remove("task_summary")) {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(s)) if s.chars().count() <= 1000 => Some(s),
        _ => return Ok(ParseOutcome::Repair(INVALID_OPERATION)),
    };
    let decision = match serde_json::from_value::<Decision>(value) {
        Ok(decision) => decision,
        Err(_) => return Ok(ParseOutcome::Repair(INVALID_OPERATION)),
    };
    match &decision {
        Decision::Finish { message, artifacts }
            if message.trim().is_empty() || (artifacts.is_empty() && empty_completion(message)) =>
        {
            return Ok(ParseOutcome::Repair(INVALID_OPERATION))
        }
        Decision::AskUser { message } if message.trim().is_empty() => {
            return Ok(ParseOutcome::Repair(INVALID_OPERATION))
        }
        _ => (),
    }
    let commentary = [prefix, &content, suffix]
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
    Ok(ParseOutcome::Operation(Box::new(Parsed {
        decision,
        commentary,
        note,
        mail_note,
        task_summary,
    })))
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
    // 原有案例只關心是否取得操作；新案例另核對傳給模型的修正原因。
    fn parse(text: &str) -> AppResult<Option<Parsed>> {
        Ok(match super::parse(text)? {
            ParseOutcome::Operation(parsed) => Some(*parsed),
            ParseOutcome::Repair(_) => None,
        })
    }
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
    fn finish_summary_is_optional_bounded_and_separate_from_answer() {
        let parsed = parse(r#"{"action":"finish","message":"答案為 42","artifacts":[],"task_summary":"已確認數值 42"}"#).unwrap().unwrap();
        assert_eq!(parsed.task_summary.as_deref(), Some("已確認數值 42"));
        assert!(
            matches!(parsed.decision, Decision::Finish {message, ..} if message == "答案為 42")
        );
        let oversized = serde_json::json!({"action":"finish","message":"答案","artifacts":[],"task_summary":"字".repeat(1001)});
        assert!(parse(&oversized.to_string()).unwrap().is_none());
    }
    #[test]
    fn missing_document_note_tool_is_repairable_without_guessing_the_operation() {
        let mut value = serde_json::json!({
            "action":"tool", "operation_id":"w40_note_cnpdf_001",
            "request":{
                "path":"电流密度对镍镀层结构和性能的影响.pdf",
                "revision":"52328b562d1bc9ddf11e7a37786c8a3ea886843183b3e33e9dde4e04dd7b16ab",
                "note_revision":"1", "section_id":null, "summary":"已讀全文摘要"
            }
        });
        let input = format!("PDF 已全文讀完，接著保存摘要。\n{value}");
        let outcome = super::parse(&input).unwrap();
        assert!(
            matches!(outcome, ParseOutcome::Repair(reason) if reason.contains("缺少必要欄位 request.tool"))
        );
        // 唯有模型補上工具名稱後才產生可執行操作；ID 與版本不由桌面代改。
        value["request"]["tool"] = serde_json::json!("update_document_note");
        let parsed = parse(&value.to_string()).unwrap().unwrap();
        assert!(
            matches!(parsed.decision, Decision::Tool {operation_id, request: super::super::Tool::UpdateDocumentNote {note_revision, ..}}
            if operation_id == "w40_note_cnpdf_001" && note_revision == "1")
        );
    }
    #[test]
    fn absent_or_malformed_tool_fields_return_specific_repair_reasons() {
        for tool_value in [
            serde_json::Value::Null,
            serde_json::json!(""),
            serde_json::json!("  "),
            serde_json::json!(123),
            serde_json::json!([]),
            serde_json::json!({}),
        ] {
            let mut value: serde_json::Value = serde_json::from_str(TOOL).unwrap();
            value["request"]["tool"] = tool_value;
            assert!(
                matches!(super::parse(&value.to_string()).unwrap(), ParseOutcome::Repair(reason)
                if reason.contains("request.tool 必須是非空字串"))
            );
        }
        for request in [
            serde_json::Value::Null,
            serde_json::json!("read_file"),
            serde_json::json!([]),
        ] {
            let value =
                serde_json::json!({"action":"tool","operation_id":"op_001","request":request});
            assert!(
                matches!(super::parse(&value.to_string()).unwrap(), ParseOutcome::Repair(reason)
                if reason.contains("request 必須是 JSON 物件"))
            );
        }
        assert!(
            matches!(super::parse(r#"{"action":"tool","operation_id":"op_001"}"#).unwrap(),
            ParseOutcome::Repair(reason) if reason.contains("request 必須是 JSON 物件"))
        );
        assert!(super::parse(&TOOL.replace("read_file", "shell")).is_err());
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
