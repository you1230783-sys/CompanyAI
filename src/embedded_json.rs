//! 從模型的說明文字／Markdown 中找出 JSON 物件。
//! 僅協助抽取格式；工具名稱、郵件範圍與授權仍由呼叫端逐一驗證。
use crate::AppResult;
use serde_json::Value;

/// 限制長度與候選次數，避免畸形回覆造成無界重複解析。
/// 成功解析的物件整段跳過，不會將其中的巢狀物件誤當另一份決策。
pub fn objects(text: &str) -> AppResult<Vec<Value>> {
    if text.len() > 1_048_576 {
        return Err("AI 回覆超過可解析大小。".into());
    }
    let mut offset = 0;
    let mut attempts = 0;
    let mut found = Vec::new();
    while let Some(relative) = text[offset..].find('{') {
        let start = offset + relative;
        attempts += 1;
        if attempts > 64 {
            return Err("AI 回覆包含過多不完整 JSON；已保留原文。".into());
        }
        let mut stream = serde_json::Deserializer::from_str(&text[start..]).into_iter::<Value>();
        if let Some(Ok(value)) = stream.next() {
            offset = start + stream.byte_offset();
            found.push(value);
        } else {
            offset = start + 1;
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extracts_nested_json_with_escaped_braces_without_reading_prose_as_commands() {
        let values = objects("說明。```json\n{\"summary\":\"a } \\\" b\",\"requests\":[{\"mail_id\":\"one\"}]}\n```尾文").unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0]["requests"][0]["mail_id"], "one");
        assert!(objects("只有自然語言回答").unwrap().is_empty());
    }
}
