//! 回覆格式相容層：結構化資料優先，沒有有效 payload 才使用 Chat Completions 原文。
//! 僅讀取顯示欄位，不把 tool_calls、masked_entities 或其他內部資料交給畫面。
use super::{legacy_reply, Message, ReceivedReply};
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
    fn conflicts_with(&self, other: &Self) -> bool {
        let lists_conflict = [
            (&self.sections.key_points, &other.sections.key_points),
            (&self.sections.sources, &other.sections.sources),
            (&self.sections.limitations, &other.sections.limitations),
        ]
        .into_iter()
        .any(|(left, right)| {
            left.iter().any(|text| !text.trim().is_empty())
                && right.iter().any(|text| !text.trim().is_empty())
                && left != right
        });
        let confidence_conflicts = match (&self.sections.confidence, &other.sections.confidence) {
            (Some(left), Some(right)) => {
                !left.trim().is_empty() && !right.trim().is_empty() && !same_text(left, right)
            }
            _ => false,
        };
        lists_conflict
            || confidence_conflicts
            || (!self.citations.is_empty()
                && !other.citations.is_empty()
                && self.citations != other.citations)
    }
    /// 只有正文一致才從較完整的回覆補欄位；非空的新欄位永遠優先。
    fn fill_missing(&mut self, other: &Self) -> bool {
        if !same_text(&self.answer, &other.answer) {
            return false;
        }
        let mut changed = false;
        for (target, source) in [
            (&mut self.sections.key_points, &other.sections.key_points),
            (&mut self.sections.sources, &other.sections.sources),
            (&mut self.sections.limitations, &other.sections.limitations),
        ] {
            if target.iter().all(|text| text.trim().is_empty())
                && source.iter().any(|text| !text.trim().is_empty())
            {
                *target = source.clone();
                changed = true;
            }
        }
        if self
            .sections
            .confidence
            .as_ref()
            .is_none_or(|text| text.trim().is_empty())
            && other
                .sections
                .confidence
                .as_ref()
                .is_some_and(|text| !text.trim().is_empty())
        {
            self.sections
                .confidence
                .clone_from(&other.sections.confidence);
            changed = true;
        }
        if self.citations.is_empty() && !other.citations.is_empty() {
            self.citations.clone_from(&other.citations);
            changed = true;
        }
        changed
    }
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
    let mut reply: ReplyPayload = serde_json::from_value(value.clone()).ok()?;
    // 部分服務把舊格式全文放在 answer，但 sections 為空；先拆出正文，再補缺漏。
    if let Some(legacy) = legacy_reply::parse(&reply.answer) {
        reply.answer = legacy.answer.clone();
        reply.fill_missing(&legacy);
    }
    (!reply.answer.trim().is_empty()).then_some(reply)
}

fn same_text(left: &str, right: &str) -> bool {
    left.split_whitespace().eq(right.split_whitespace())
}

/// 完成回覆不可讓已收到的內容無聲消失：能確認正文相同時只補空欄位，
/// 不一致或無法辨識時另存可展開原文，不把舊來源冒充成新答案的欄位。
pub fn preserve_received_reply(message: &mut Message, text: &str, from_stream: bool) {
    if text.trim().is_empty() || same_text(&message.content, text) {
        return;
    }
    let recovered = payload(&Value::String(text.to_string())).or_else(|| legacy_reply::parse(text));
    if let Some(recovered) = &recovered {
        let mut authoritative = message.response_payload.clone().unwrap_or(ReplyPayload {
            answer: message.content.clone(),
            sections: ReplySections::default(),
            citations: Vec::new(),
        });
        if same_text(&authoritative.answer, &recovered.answer) {
            let conflict = authoritative.conflicts_with(recovered);
            if authoritative.fill_missing(recovered) {
                message.content = authoritative.markdown();
                message.response_payload = Some(authoritative);
                // 先前保留的原文仍需包含在複製文字，不能因補欄位而再次遺失。
                for retained in &message.received_replies {
                    append_received_text(&mut message.content, retained);
                }
            }
            if !conflict {
                return;
            }
        }
    }
    if message
        .response_payload
        .as_ref()
        .is_some_and(|reply| same_text(&reply.answer, text))
        || message
            .received_replies
            .iter()
            .any(|reply| reply.text == text)
    {
        return;
    }
    // 完整 JSON 只保留可讀欄位，不把其中的內部工具資料再送進 UI。
    let retained = ReceivedReply {
        text: recovered
            .map(|reply| reply.markdown())
            .unwrap_or_else(|| text.to_string()),
        from_stream,
    };
    // REST 與串流可能各帶一次同樣的舊全文，正規化後仍只保留一份。
    if message
        .received_replies
        .iter()
        .any(|reply| same_text(&reply.text, &retained.text))
    {
        return;
    }
    if message.response_payload.is_none() {
        message.response_payload = Some(ReplyPayload {
            answer: message.content.clone(),
            sections: ReplySections::default(),
            citations: Vec::new(),
        });
    }
    append_received_text(&mut message.content, &retained);
    message.received_replies.push(retained);
}

fn append_received_text(text: &mut String, retained: &ReceivedReply) {
    let title = if retained.from_stream {
        "串流期間收到的原文"
    } else {
        "完整原始回覆"
    };
    text.push_str(&format!("\n\n## {title}\n\n{}", retained.text));
}

/// 支援直接結果、result 包裝及 response_payload_json；只查固定位置，不遞迴搜尋任意 JSON。
/// 結構化欄位一律優先於 choices 的正文，避免完成時重點與來源消失。
pub fn assistant_message(value: &Value) -> AppResult<Message> {
    let result = &value["result"];
    let message = &value["choices"][0]["message"];
    let result_message = &result["choices"][0]["message"];
    // 先查所有明確的 payload 欄位，再接受直接物件或 content 內完整的 JSON。
    // 避免外層較舊的正文搶先覆蓋 result 內的新結構化資料。
    let mut selected: Option<ReplyPayload> = None;
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
            if let Some(existing) = &mut selected {
                existing.fill_missing(&reply);
            } else {
                selected = Some(reply);
            }
        }
    }
    if let Some(reply) = selected {
        let mut parsed = Message::assistant(reply.markdown());
        parsed.response_payload = Some(reply);
        for candidate in [message, result_message] {
            if let Some(text) = candidate["content"].as_str() {
                preserve_received_reply(&mut parsed, text, false);
            }
        }
        return Ok(parsed);
    }
    for scope in [value, result] {
        if let Some(text) = scope["choices"][0]["message"]["content"].as_str() {
            if !text.trim().is_empty() {
                let mut parsed = Message::assistant(text.to_string());
                parsed.response_payload = legacy_reply::parse(text);
                return Ok(parsed);
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
    fn body_only_payload_recovers_the_reported_inline_reply_without_dropping_sections() {
        let text = include_str!("../../ui/fixtures/inline-reply.txt");
        let answer = "您好！這是一個測試訊息。我已準備好為您提供協助。";
        for value in [
            json!({"response_payload_json":{"answer":answer,"sections":{}},"choices":[{"message":{"content":text}}]}),
            json!({"answer":text,"sections":{}}),
            json!({"choices":[{"message":{"content":text}}]}),
            json!({"response_payload_json":{"answer":answer},"result":sample_with_same_answer(answer)}),
        ] {
            let message = assistant_message(&value).unwrap();
            let reply = message.response_payload.unwrap();
            assert_eq!(reply.answer, answer);
            assert_eq!(reply.sections.key_points.len(), 2);
            assert!(!reply.sections.sources.is_empty());
            assert!(!reply.sections.limitations.is_empty());
            assert!(
                message.content.contains("系統回應正常。")
                    || message.content.contains("使用者發送了測試指令。")
            );
        }
    }

    fn sample_with_same_answer(answer: &str) -> Value {
        let mut value = sample();
        value["answer"] = json!(answer);
        value
    }

    #[test]
    fn completed_body_retains_stream_sections_but_does_not_replace_explicit_final_values() {
        let text = include_str!("../../ui/fixtures/inline-reply.txt");
        let answer = "您好！這是一個測試訊息。我已準備好為您提供協助。";
        let mut message =
            assistant_message(&json!({"choices":[{"message":{"content":answer}}]})).unwrap();
        preserve_received_reply(&mut message, text, true);
        let reply = message.response_payload.as_ref().unwrap();
        assert_eq!(reply.sections.key_points.len(), 2);
        assert_eq!(reply.sections.confidence.as_deref(), Some("100%"));
        assert!(message.content.contains("未涉及任何實際的文件分析"));

        let mut authoritative =
            assistant_message(&json!({"answer":answer,"sections":{"confidence":"Low"}})).unwrap();
        preserve_received_reply(&mut authoritative, text, true);
        assert_eq!(
            authoritative
                .response_payload
                .as_ref()
                .unwrap()
                .sections
                .confidence
                .as_deref(),
            Some("Low")
        );
        assert_eq!(
            authoritative
                .response_payload
                .as_ref()
                .unwrap()
                .sections
                .key_points
                .len(),
            2
        );
        assert!(authoritative.received_replies[0].from_stream);
        assert!(authoritative.received_replies[0].text.contains("100%"));
    }

    #[test]
    fn unrecognized_or_changed_stream_is_preserved_separately_without_merging_answers() {
        let mut message = assistant_message(&json!({"answer":"最終答案已更正"})).unwrap();
        preserve_received_reply(&mut message, "串流原始內容，無法辨識章節。", true);
        assert_eq!(
            message.response_payload.as_ref().unwrap().answer,
            "最終答案已更正"
        );
        assert!(message
            .response_payload
            .as_ref()
            .unwrap()
            .sections
            .key_points
            .is_empty());
        assert!(message.content.contains("串流原始內容，無法辨識章節。"));
        assert_eq!(message.received_replies.len(), 1);
        let restored: Message =
            serde_json::from_str(&serde_json::to_string(&message).unwrap()).unwrap();
        assert_eq!(
            restored.received_replies[0].text,
            message.received_replies[0].text
        );
        let original = include_str!("../../ui/fixtures/inline-reply.txt");
        let mut changed = assistant_message(&json!({"answer":"完成後已更正"})).unwrap();
        preserve_received_reply(&mut changed, original, false);
        preserve_received_reply(&mut changed, original, true);
        assert_eq!(changed.received_replies.len(), 1);
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
