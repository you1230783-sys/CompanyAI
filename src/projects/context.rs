//! 送出前的文字預算與無正文用量統計。0.48 是使用者四筆 API 實測的初始係數，
//! 不是 tokenizer；只用於整理提示，不作為拒絕任務的硬上限。
use serde_json::{json, Value};

pub const TARGET_TOKENS: usize = 12_000;

fn chars(value: &Value) -> usize {
    value
        .as_str()
        .map_or_else(|| value.to_string().chars().count(), |s| s.chars().count())
}
pub fn estimate(characters: usize) -> usize {
    characters.saturating_mul(48).div_ceil(100)
}

/// 僅輸出欄位序號、角色及長度；不含主旨、路徑、筆記、工具參數或使用者原文。
pub fn measure(request: &Value) -> Value {
    let mut total = 0;
    let mut blocks = Vec::new();
    for (index, message) in request["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let content = message
            .get("content")
            .filter(|v| !v.is_null())
            .map_or(0, chars);
        let calls = message.get("tool_calls").map_or(0, chars);
        total += content + calls;
        let role = match message["role"].as_str() {
            Some("system") => "system",
            Some("assistant") => "assistant",
            Some("tool") => "tool",
            _ => "user",
        };
        blocks.push(json!({"block":"message","index":index,"role":role,
            "content_chars":content,"tool_call_chars":calls,"estimated_tokens":estimate(content+calls)}));
    }
    let tool_chars = request
        .get("tools")
        .filter(|v| v.as_array().is_some_and(|a| !a.is_empty()))
        .map_or(0, chars);
    total += tool_chars;
    blocks.push(
        json!({"block":"tools","count":request["tools"].as_array().map_or(0, Vec::len),
        "characters":tool_chars,"estimated_tokens":estimate(tool_chars)}),
    );
    json!({"estimator":"characters-v1","coefficient":0.48,"target_input_tokens":[8000,TARGET_TOKENS],
        "characters":total,"estimated_input_tokens":estimate(total),"blocks":blocks,
        "notice":"估算未含服務端模板或額外上下文；僅供同模型對照，實際以 API usage 為準。"})
}

/// 統計採數字白名單，不能把供應商的其他回覆欄位混進可單獨分享的報表。
pub fn actual(response: &Value) -> Value {
    fn usage(value: &Value, depth: usize) -> Option<&Value> {
        if depth > 4 {
            return None;
        }
        if let Some(usage) = value.get("usage").filter(|v| v.is_object()) {
            return Some(usage);
        }
        [
            "result",
            "response_payload_json",
            "response",
            "model_response",
        ]
        .iter()
        .filter_map(|key| value.get(*key))
        .find_map(|v| usage(v, depth + 1))
    }
    let Some(usage) = usage(response, 0) else {
        return Value::Null;
    };
    let mut result = serde_json::Map::new();
    for key in [
        "prompt_tokens",
        "completion_tokens",
        "total_tokens",
        "input_tokens",
        "output_tokens",
    ] {
        if let Some(n) = usage[key].as_u64() {
            result.insert(key.into(), json!(n));
        }
    }
    for (group, key) in [
        ("prompt_tokens_details", "cached_tokens"),
        ("completion_tokens_details", "reasoning_tokens"),
    ] {
        if let Some(n) = usage[group][key].as_u64() {
            result.insert(key.into(), json!(n));
        }
    }
    Value::Object(result)
}

/// 已封存的舊工具結果按整組移出本輪請求，保留最後一組及使用者原文。
/// 不改 checkpoint、已提交請求與工具去重表，也不裁斷 JSON 工具參數。
pub fn fit(request: &mut Value) {
    while measure(request)["estimated_input_tokens"]
        .as_u64()
        .unwrap_or(0)
        > TARGET_TOKENS as u64
    {
        let Some(messages) = request["messages"].as_array_mut() else {
            break;
        };
        let pairs: Vec<_> = messages
            .windows(2)
            .enumerate()
            .filter_map(|(i, p)| {
                (p[0]["role"] == "assistant"
                    && p[0]["tool_calls"].is_array()
                    && p[1]["role"] == "tool")
                    .then_some(i)
            })
            .collect();
        if pairs.len() <= 1 {
            break;
        }
        messages.drain(pairs[0]..pairs[0] + 2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calibration_and_report_never_contain_content() {
        for (chars, actual) in [(9331, 4503), (39926, 18850), (61235, 29360), (56810, 27497)] {
            assert!((estimate(chars) as f64 / actual as f64 - 1.0).abs() < 0.02);
        }
        let report =
            measure(&json!({"messages":[{"role":"user","content":"SECRET_PATH"}],"tools":[]}));
        assert!(!report.to_string().contains("SECRET"));
        assert_eq!(
            actual(&json!({"usage":{"prompt_tokens":4503,"secret":"x"}})),
            json!({"prompt_tokens":4503})
        );
    }
    #[test]
    fn soft_budget_keeps_latest_pair_and_never_cuts_user_requirements() {
        let mut messages = vec![json!({"role":"user","content":"條件".repeat(20000)})];
        for i in 0..3 {
            messages.push(json!({"role":"assistant","tool_calls":[{"id":i.to_string()}]}));
            messages.push(
                json!({"role":"tool","tool_call_id":i.to_string(),"content":"資料".repeat(6000)}),
            );
        }
        let mut request = json!({"messages":messages,"tools":[]});
        fit(&mut request);
        assert_eq!(request["messages"].as_array().unwrap().len(), 3);
        assert_eq!(request["messages"][2]["tool_call_id"], "2");
        assert_eq!(request["messages"][0]["content"], "條件".repeat(20000));
        assert!(
            measure(&request)["estimated_input_tokens"]
                .as_u64()
                .unwrap()
                > 12000
        );
    }
    #[test]
    fn usage_in_provider_wrappers_keeps_reasoning_and_does_not_guess_missing_counts() {
        assert_eq!(
            actual(
                &json!({"result":{"response_payload_json":{"usage":{"prompt_tokens":29360,"completion_tokens":12165,"completion_tokens_details":{"reasoning_tokens":11000}}}}})
            )["reasoning_tokens"],
            11000
        );
        assert!(actual(&json!({"result":{"answer":"沒有用量"}})).is_null());
    }
}
