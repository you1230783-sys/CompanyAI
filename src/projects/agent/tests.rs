use super::*;

fn caps() -> Capabilities {
    serde_json::from_str(include_str!("test_capabilities.json")).unwrap()
}
fn request() -> Value {
    build_request(&caps(),"conversation_demo","req1",
        json!({"project_id":"p","run_id":"r","turn_index":1,"parent_request_id":null,"context_policy":"client_snapshot"}),
        &[Message::user("請閱讀文件")],true).unwrap()
}
fn task(request: Value) -> jobs::Task {
    jobs::Task {
        request_id: "req1".into(),
        conversation_id: "local".into(),
        request,
        mode: "background".into(),
        title: String::new(),
        created_at: 0,
        remote: None,
        applied: false,
        message: String::new(),
        mail_analysis: false,
        title_generation: false,
        tool_events: vec![],
        partial: String::new(),
    }
}
fn status(task: &jobs::Task, args: &str) -> jobs::TaskStatus {
    serde_json::from_value(json!({"task_id":"task1","client_request_id":task.request_id,
        "contract_version":CONTRACT,"conversation_id":task.request["conversation_id"],"context":task.request["context"],
        "state":"completed","error_message":"","result":{"id":"completion_1","object":"chat.completion","created":1,"model":"quality","choices":[{"index":0,"finish_reason":"tool_calls",
        "message":{"role":"assistant","content":null,"tool_calls":[{"id":"call:001","type":"function",
        "function":{"name":"read_file","arguments":args}}]}}]}})).unwrap()
}
#[test]
fn request_uses_native_fields_and_no_legacy_flags() {
    let r = request();
    assert_eq!(r["contract_version"], CONTRACT);
    assert_eq!(r["skills"], false);
    assert_eq!(r["tool_choice"], "required");
    assert_eq!(r["parallel_tool_calls"], false);
    assert!(r.get("attachment_tokens").is_none());
    assert!(r.get("outlook_triage").is_none());
    let c = caps();
    let empty = build_request(&c, "c", "r", json!({}), &[Message::user("摘要")], false).unwrap();
    assert_eq!(empty["tools"], json!([]));
    assert_eq!(empty["tool_choice"], "none");
}
#[test]
fn native_null_content_call_is_accepted_without_parsing_text() {
    let task = task(request());
    let status = status(
        &task,
        r#"{"path":"測試.txt","offset":0,"progress_note":null}"#,
    );
    let Parsed::Operation {
        parsed,
        message,
        call_id,
    } = parse(&status, &task, "owner", "run").unwrap()
    else {
        panic!("expected tool")
    };
    assert_eq!(call_id, "call:001");
    assert!(message.content.is_empty());
    let super::super::Decision::Tool { operation_id, .. } = parsed.decision else {
        panic!("tool")
    };
    assert_eq!(
        operation_id,
        operation_key("owner", "run", "req1", "call:001")
    );
}
#[test]
fn strict_schema_duplicate_keys_unknown_fields_and_malformed_arguments_fail() {
    let task = task(request());
    for args in [
        r#"{"path":"a","path":"b","offset":0,"progress_note":null}"#,
        r#"{"path":"a","offset":0,"progress_note":null,"escape":true}"#,
        r#"{"path":"a"}"#,
        "{broken",
    ] {
        assert!(
            parse(&status(&task, args), &task, "p", "r").is_err(),
            "{args}"
        );
    }
    assert!(schema::decode_json(r#"{"result":{},"result":{}}"#).is_err());
}
#[test]
fn non_strict_repair_retains_native_call_for_tool_error() {
    let mut r = request();
    for tool in r["tools"].as_array_mut().unwrap() {
        tool["function"]["strict"] = json!(false);
    }
    let task = task(r);
    assert!(matches!(
        parse(&status(&task, "{broken"), &task, "p", "r").unwrap(),
        Parsed::Repair {
            message: Some(_),
            call_id: Some(_),
            ..
        }
    ));
}
#[test]
fn truncated_refused_multiple_or_wrong_identity_never_execute() {
    let task = task(request());
    let original = status(&task, r#"{"path":"a","offset":0,"progress_note":null}"#);
    let mut truncated = original.clone();
    truncated.result.as_mut().unwrap()["choices"][0]["finish_reason"] = json!("length");
    assert!(matches!(
        parse(&truncated, &task, "p", "r").unwrap(),
        Parsed::Repair { message: None, .. }
    ));
    let mut refused = original.clone();
    refused.result.as_mut().unwrap()["choices"][0]["message"]["refusal"] = json!("拒絕");
    assert!(parse(&refused, &task, "p", "r").is_err());
    let mut multiple = original.clone();
    let calls = multiple.result.as_mut().unwrap()["choices"][0]["message"]["tool_calls"]
        .as_array_mut()
        .unwrap();
    calls.push(calls[0].clone());
    assert!(parse(&multiple, &task, "p", "r").is_err());
    let mut wrong = original;
    wrong.agent_envelope.insert("context".into(), json!({}));
    assert!(parse(&wrong, &task, "p", "r").is_err());
}
#[test]
fn history_pairs_and_request_scoped_ids_survive_repeated_calls() {
    let mut assistant = Message::assistant("正在閱讀".into());
    assistant.tool_calls = vec![
        json!({"id":"call_001","type":"function","function":{"name":"old_tool","arguments":"{}"}}),
    ];
    let result = Message::result("call_001", &json!({"ok":true}));
    assert!(validate_history(&[assistant.clone()]).is_err());
    assert!(validate_history(std::slice::from_ref(&result)).is_err());
    assert!(validate_history(&[assistant.clone(), result.clone(), assistant, result]).is_ok());
    assert_ne!(
        operation_key("p", "r", "req1", "call_001"),
        operation_key("p", "r", "req2", "call_001")
    );
    assert_ne!(
        operation_key("ab", "c", "d", "e"),
        operation_key("a", "bc", "d", "e")
    );
}
#[test]
fn catalog_is_portable_and_nullable_fields_restore_original_defaults() {
    let tools = schema::definitions(true).unwrap();
    assert_eq!(tools.len(), 31);
    let encoded = serde_json::to_string(&tools).unwrap();
    for key in [
        "\"oneOf\":",
        "\"maxLength\":",
        "\"minimum\":",
        "\"default\":",
        "\"const\":",
    ] {
        assert!(!encoded.contains(key), "{key}");
    }
    let restored = schema::restore_optional(
        "read_file",
        json!({"path":"a","offset":null,"progress_note":null}),
    )
    .unwrap();
    assert!(restored.get("offset").is_none());
}
#[test]
fn unsupported_model_and_limits_do_not_fall_back_or_truncate() {
    let mut c = caps();
    c.native_tool_calls = false;
    assert!(c.validate("quality", true).is_err());
    assert!(c.validate("quality", false).is_ok());
    c.limits.tools = 0;
    c.limits.tools_bytes = 0;
    assert!(build_request(&c, "c", "r", json!({}), &[Message::user("摘要")], false).is_ok());
    c = caps();
    c.limits.request_bytes = 20;
    assert!(build_request(&c, "c", "r", json!({}), &[Message::user("保留全文")], true).is_err());
}

/// 代表模型依當輪 Schema 填入參數；所有內建工具都需能轉回既有 Rust 型別。
/// 此測試不執行工具，也不宣稱虛構檔案／版本具備授權。
#[test]
fn every_native_tool_schema_maps_to_existing_decision() {
    fn example(s: &Value) -> Value {
        if let Some(v) = s["enum"].as_array() {
            return v[0].clone();
        }
        if let Some(v) = s["anyOf"].as_array() {
            if v.iter().any(|s| s["type"] == "null") {
                return Value::Null;
            }
            return example(&v[0]);
        }
        let kind = s["type"]
            .as_str()
            .or_else(|| s["type"].as_array().and_then(|a| a[0].as_str()))
            .unwrap();
        match kind {
            "object" => Value::Object(
                s["properties"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(k, v)| (k.clone(), example(v)))
                    .collect(),
            ),
            "array" => json!([example(&s["items"])]),
            "string" => json!("範例"),
            "integer" => json!(0),
            "number" => json!(1.0),
            "boolean" => json!(true),
            "null" => Value::Null,
            _ => panic!("unexpected type"),
        }
    }
    let task = task(request());
    for tool in task.request["tools"].as_array().unwrap() {
        let name = tool["function"]["name"].as_str().unwrap();
        let args = example(&tool["function"]["parameters"]);
        let mut result = status(&task, &args.to_string());
        result.result.as_mut().unwrap()["choices"][0]["message"]["tool_calls"][0]["function"]
            ["name"] = json!(name);
        assert!(
            matches!(
                parse(&result, &task, "p", "r").unwrap(),
                Parsed::Operation { .. }
            ),
            "{name}: {args}"
        );
    }
}

#[test]
fn nested_office_nullable_format_preserves_explicit_properties() {
    let tool = schema::definitions(true)
        .unwrap()
        .into_iter()
        .find(|t| t["function"]["name"] == "office_action")
        .unwrap();
    let format_schema = &tool["function"]["parameters"]["properties"]["operation"]["anyOf"][0]
        ["properties"]["format"];
    let mut format = serde_json::Map::new();
    for name in format_schema["properties"].as_object().unwrap().keys() {
        format.insert(name.clone(), Value::Null);
    }
    format.insert("bold".into(), json!(true));
    let args = json!({"copy_id":"copy","revision":"revision","operation":{"kind":"format","target":"p1","format":format},"progress_note":null});
    let verified =
        schema::decode_arguments(&args.to_string(), &tool["function"]["parameters"]).unwrap();
    let restored = schema::restore_optional("office_action", verified).unwrap();
    assert_eq!(restored["operation"]["format"], json!({"bold":true}));
}
