//! 分離模型說明與單一操作；工具仍須通過完整結構及 broker 授權檢查。
use super::Decision;
use crate::AppResult;

pub struct Parsed {
    pub decision: Decision,
    pub commentary: String,
}

/// 接受純 JSON 或說明包住的一個 Decision JSON；不猜測多個操作的執行順序。
/// 不完整 JSON 才要求修正；未知操作、陣列與多份物件直接停止。
pub fn parse(text: &str) -> AppResult<Option<Parsed>> {
    let text = text.trim();
    if text.len() > 64_000 {
        return Err("模型操作超過大小限制。".into());
    }
    if let Ok(decision) = serde_json::from_str::<Decision>(text) {
        return Ok(Some(Parsed {
            decision,
            commentary: String::new(),
        }));
    }
    if let Some(start) = text.find('{') {
        let prefix = &text[..start];
        if prefix.contains(['[', ']']) {
            return Err("不接受陣列包裝的工具操作。".into());
        }
        let mut stream =
            serde_json::Deserializer::from_str(&text[start..]).into_iter::<serde_json::Value>();
        match stream.next() {
            Some(Ok(value)) => {
                let suffix = &text[start + stream.byte_offset()..];
                if !suffix.contains(['{', '}', '[', ']']) {
                    if let Ok(decision) = serde_json::from_value::<Decision>(value) {
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
                        return Ok(Some(Parsed {
                            decision,
                            commentary,
                        }));
                    }
                }
            }
            Some(Err(error)) if error.is_eof() => return Ok(None),
            _ => {}
        }
    }
    Err("模型未回傳可辨識的單一操作，已停止；可按「重新再試一次」。".into())
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
        assert!(parse(&format!("{ask}\n{TOOL}")).is_err());
    }

    #[test]
    fn refuses_ambiguous_or_unknown_commands() {
        for text in [
            format!("{TOOL}\n{TOOL}"),
            format!("[{TOOL}]"),
            TOOL.replace("read_file", "shell"),
        ] {
            assert!(parse(&text).is_err(), "{text}");
        }
    }
}
