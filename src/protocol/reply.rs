//! 回覆格式相容層：結構化資料優先，沒有有效 payload 才使用 Chat Completions 原文。
//! 僅讀取顯示欄位，不把 tool_calls、masked_entities 或其他內部資料交給畫面。
use super::Message;
use crate::AppResult;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ReplySections {
    pub key_points: Vec<String>,
    pub sources: Vec<String>,
    pub confidence: Option<String>,
    pub limitations: Vec<String>,
}

/// 網頁 response_payload_json 中可供使用者閱讀的欄位。
/// 信心只取 sections.confidence，不以最外層 confidence 覆寫。
#[derive(Clone, Serialize, Deserialize)]
pub struct ReplyPayload {
    pub answer: String,
    #[serde(default)]
    pub sections: ReplySections,
    /// 引用項目尚未限定物件 schema；保留資料供唯讀呈現，不自動開啟連結／檔案。
    #[serde(default)]
    pub citations: Vec<Value>,
}

impl ReplyPayload {
    /// 產生完整可複製文字及後續對話內容；畫面仍直接使用欄位，不重新解析這些標題。
    fn markdown(&self) -> String {
        let mut text = self.answer.clone();
        for (title, values) in [
            ("回答重點", &self.sections.key_points),
            ("來源摘要", &self.sections.sources),
            ("回答限制", &self.sections.limitations),
        ] {
            if values.iter().any(|value| !value.trim().is_empty()) {
                text.push_str(&format!("\n\n## {title}\n\n"));
                for value in values.iter().filter(|value| !value.trim().is_empty()) {
                    text.push_str(&format!("- {}\n", value.replace('\n', "\n  ")));
                }
            }
        }
        if let Some(confidence) = &self.sections.confidence {
            if !confidence.trim().is_empty() {
                text.push_str(&format!("\n\n## 信心度\n\n{confidence}"));
            }
        }
        if !self.citations.is_empty() {
            text.push_str("\n\n## 引用文件\n\n");
            for citation in &self.citations {
                if let Some(value) = citation.as_str() {
                    text.push_str(value);
                } else {
                    // 未知引用結構完整保留為文字，不猜測檔案路徑或操作指令。
                    text.push_str(&citation.to_string());
                }
                text.push_str("\n\n");
            }
        }
        text
    }
}

/// payload 可能是 JSON 物件或資料庫序列化的 JSON 字串；無效時交回舊格式備援。
fn payload(value: &Value) -> Option<ReplyPayload> {
    let decoded;
    let value = if let Some(text) = value.as_str() {
        decoded = serde_json::from_str::<Value>(text).ok()?;
        &decoded
    } else {
        value
    };
    let reply: ReplyPayload = serde_json::from_value(value.clone()).ok()?;
    (!reply.answer.trim().is_empty()).then_some(reply)
}

/// 支援直接結果、result 包裝及 response_payload_json；只查固定位置，不遞迴搜尋任意 JSON。
/// 結構化欄位一律優先於 choices 的正文，避免完成時重點與來源消失。
pub fn assistant_message(value: &Value) -> AppResult<Message> {
    let result = &value["result"];
    let message = &value["choices"][0]["message"];
    let result_message = &result["choices"][0]["message"];
    // 先查所有明確的 payload 欄位，再接受直接物件或 content 內完整的 JSON。
    // 避免外層較舊的正文搶先覆蓋 result 內的新結構化資料。
    for candidate in [
        &value["response_payload_json"],
        &result["response_payload_json"],
        &message["response_payload_json"],
        &result_message["response_payload_json"],
        value,
        result,
        &message["content"],
        &result_message["content"],
    ] {
        if let Some(reply) = payload(candidate) {
            let mut message = Message::assistant(reply.markdown());
            message.response_payload = Some(reply);
            return Ok(message);
        }
    }
    for scope in [value, result] {
        if let Some(text) = scope["choices"][0]["message"]["content"].as_str() {
            if !text.trim().is_empty() {
                return Ok(Message::assistant(text.to_string()));
            }
        }
    }
    Err(
        "API 沒有可讀取的回答；請檢查 response_payload_json.answer 或 choices[0].message.content。"
            .into(),
    )
}

/// 標題生成及 Outlook 初篩需要原始正文，不加入顯示用的重點／來源標題。
pub fn assistant_text(body: &str) -> AppResult<String> {
    let value: Value = serde_json::from_str(body).map_err(|_| "API 回覆不是有效的 JSON。")?;
    let message = assistant_message(&value)?;
    Ok(match message.response_payload {
        Some(payload) => payload.answer,
        None => message.content,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> Value {
        serde_json::from_str(include_str!("../../ui/fixtures/structured-reply.json")).unwrap()
    }

    #[test]
    fn structured_payload_wins_over_body_only_in_supported_envelopes() {
        let payload = sample();
        let legacy = json!({"choices":[{"message":{"content":"只有正文的舊欄位"}}]});
        for value in [
            payload.clone(),
            json!({"result":payload}),
            json!({"response_payload_json":payload,"result":legacy}),
            json!({"result":{"response_payload_json":payload.to_string(),"choices":legacy["choices"]}}),
            json!({"choices":[{"message":{"response_payload_json":payload,"content":"只有正文"}}]}),
            json!({"choices":[{"message":{"content":payload.to_string()}}]}),
            json!({"answer":"外層舊正文","result":{"response_payload_json":payload}}),
        ] {
            let message = assistant_message(&value).unwrap();
            let structured = message.response_payload.as_ref().unwrap();
            assert_eq!(structured.answer, payload["answer"].as_str().unwrap());
            assert_eq!(structured.sections.key_points.len(), 2);
            assert_eq!(structured.sections.confidence.as_deref(), Some("High (高)"));
            assert!(message.content.contains("使用者發送了測試指令。"));
            assert!(message.content.contains("無（此為對測試訊息的直接回應）。"));
            assert!(message.content.contains("由於沒有提供具體問題"));
            assert!(!message.content.contains("medium"));
            assert_eq!(
                assistant_text(&value.to_string()).unwrap(),
                structured.answer
            );
        }
    }

    #[test]
    fn missing_or_malformed_payload_keeps_legacy_text_and_invalid_results_fail() {
        for candidate in [
            Value::Null,
            json!("not JSON"),
            json!({"answer":42}),
            json!({"answer":""}),
        ] {
            let value = json!({
                "response_payload_json":candidate,
                "choices":[{"message":{"content":"1. 回答：舊回答\n2. 回答重點：原文"}}]
            });
            let message = assistant_message(&value).unwrap();
            assert!(message.response_payload.is_none());
            assert_eq!(message.content, value["choices"][0]["message"]["content"]);
        }
        for value in [
            Value::Null,
            json!([]),
            json!({"choices":[]}),
            json!({"answer":" "}),
        ] {
            assert!(assistant_message(&value).is_err());
        }
        let simple =
            assistant_message(&json!({"answer":"正文","sections":{},"citations":[]})).unwrap();
        assert_eq!(simple.content, "正文");
    }

    #[test]
    fn structured_history_and_followup_keep_sections_without_internal_metadata() {
        let mut payload = sample();
        payload["citations"] = json!(["文件一", {"title":"文件二","page":2}]);
        payload["tool_calls"] = json!([{"name":"must_not_execute"}]);
        payload["masked_entities"] = json!(["internal-only"]);
        let message = assistant_message(&payload).unwrap();
        let saved = serde_json::to_string(&message).unwrap();
        let restored: Message = serde_json::from_str(&saved).unwrap();
        assert_eq!(restored.content, message.content);
        assert_eq!(
            serde_json::to_value(restored.response_payload.unwrap().citations).unwrap(),
            payload["citations"]
        );
        assert!(!saved.contains("must_not_execute"));
        assert!(!saved.contains("internal-only"));
        let outgoing: Value =
            serde_json::from_str(&super::super::chat_json("fast", &[message]).unwrap()).unwrap();
        assert!(outgoing["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("文件二"));
        assert!(outgoing["messages"][0].get("response_payload").is_none());
        let old: Message =
            serde_json::from_value(json!({"role":"assistant","content":"舊歷史"})).unwrap();
        assert!(old.response_payload.is_none());
    }
}
