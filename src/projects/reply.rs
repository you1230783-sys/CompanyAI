//! 僅辨識可要求模型修正的包裝文字；不直接執行從散文擷取出的工具。
use super::Decision;
use crate::AppResult;

/// 純 JSON 直接驗證。若只有一個完整且合法的工具物件被文字包住，
/// 回傳 None 讓呼叫者請模型重新輸出；完成結果、多份物件及未知工具均不猜測。
pub fn parse(text: &str) -> AppResult<Option<Decision>> {
    let text = text.trim();
    if let Ok(decision) = serde_json::from_str::<Decision>(text) {
        return Ok(Some(decision));
    }
    if text.len() <= 64_000 {
        if let Some(start) = text.find('{') {
            let mut stream =
                serde_json::Deserializer::from_str(&text[start..]).into_iter::<serde_json::Value>();
            if let Some(Ok(value)) = stream.next() {
                let suffix = &text[start + stream.byte_offset()..];
                // 保守拒絕其他 JSON／陣列，不能挑其中一個當作操作意圖。
                if !text[..start].contains(['[', ']'])
                    && !suffix.contains(['{', '}', '[', ']'])
                    && matches!(
                        serde_json::from_value::<Decision>(value),
                        Ok(Decision::Tool { .. })
                    )
                {
                    return Ok(None);
                }
            }
        }
    }
    Err("模型未回傳可辨識的單一操作，已停止；可按「重新再試一次」。".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    const TOOL: &str = r#"{"action":"tool","operation_id":"op_001","request":{"tool":"read_file","path":"colab_download_models.txt","offset":0}}"#;

    #[test]
    fn wrapped_tool_requires_model_repair_instead_of_execution() {
        assert!(matches!(parse(TOOL).unwrap(), Some(Decision::Tool { .. })));
        for text in [
            format!("工具：{TOOL}"),
            format!("```json\n{TOOL}\n```"),
            format!("請執行 {TOOL}。"),
        ] {
            assert!(parse(&text).unwrap().is_none());
        }
        let braces = TOOL.replace("colab_download_models.txt", r#"a{\"b}.txt"#);
        assert!(parse(&format!("工具：{braces}")).unwrap().is_none());
    }

    #[test]
    fn does_not_repair_results_multiple_objects_or_unknown_operations() {
        for text in [
            format!("{TOOL}\n{TOOL}"),
            format!("[{TOOL}]"),
            TOOL.replace("read_file", "shell"),
            "工具：{\"action\":\"tool\"".into(),
            "結果：{\"action\":\"finish\",\"message\":\"完成\",\"artifacts\":[]}".into(),
        ] {
            assert!(parse(&text).is_err(), "{text}");
        }
    }
}
