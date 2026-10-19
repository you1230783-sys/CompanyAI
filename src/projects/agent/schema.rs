//! 將內建工具目錄轉為公布的可攜 Schema；限制仍由工具實作檢查。
//! 使用明確白名單轉換，不把模型提交的未知 Schema 當成可執行設定。
use crate::AppResult;
use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{json, Map, Value};

fn catalog() -> AppResult<Value> {
    serde_json::from_str(include_str!("../tools.json")).map_err(|e| e.to_string())
}

/// 暫存區段沿用同一份內建幾何白名單，不讓Broker直呼繞過工具參數結構。
pub(super) fn validate_cad_operations(operations: &[Value]) -> AppResult<()> {
    let source = catalog()?;
    let draw = source["tools"]
        .as_array()
        .and_then(|t| t.iter().find(|t| t["function"]["name"] == "cad_draw"))
        .ok_or("缺少CAD幾何規格。")?;
    let mut schema = json!({"type":"object","properties":{"operations":draw["function"]["parameters"]["properties"]["operations"]},"required":["operations"],"additionalProperties":false});
    // Broker收到的是optional null已還原的內部資料，不能用會將所有欄位列為required
    // 的傳輸normalize再驗一次。只將明確kind區分的oneOf改為matcher認得的anyOf。
    fn variants(value: &mut Value) {
        match value {
            Value::Object(map) => {
                if let Some(branches) = map.remove("oneOf") {
                    map.insert("anyOf".into(), branches);
                }
                for child in map.values_mut() {
                    variants(child);
                }
            }
            Value::Array(items) => {
                for child in items {
                    variants(child);
                }
            }
            _ => (),
        }
    }
    variants(&mut schema);
    decode_arguments(&json!({"operations":operations}).to_string(), &schema)?;
    Ok(())
}

pub(super) fn definitions(strict: bool) -> AppResult<Vec<Value>> {
    let source = catalog()?;
    source["tools"]
        .as_array()
        .ok_or("內建工具目錄遺失。")?
        .iter()
        .map(|tool| {
            let note = tool["function"]["parameters"]["properties"]["progress_note"].clone();
            let mut tool = tool.clone();
            tool["function"]["parameters"] = normalize(&tool["function"]["parameters"])?;
            // 新請求明確要求每輪筆記；回覆相容層仍可讀舊 checkpoint 的 null／缺漏。
            if note.is_object() {
                tool["function"]["parameters"]["properties"]["progress_note"] = normalize(&note)?;
            }
            tool["function"]["strict"] = json!(strict);
            Ok(tool)
        })
        .collect()
}

fn normalize(source: &Value) -> AppResult<Value> {
    let object = source.as_object().ok_or("內建參數 Schema 必須為物件。")?;
    let mut result = Map::new();
    for (key, value) in object {
        match key.as_str() {
            "type" | "description" | "title" | "enum" => {
                result.insert(key.clone(), value.clone());
            }
            "const" => {
                result.insert("enum".into(), json!([value]));
            }
            // 現有 oneOf 的分支由 action 常數區分，轉為 anyOf 不改變其有效集合。
            "oneOf" | "anyOf" => {
                let variants = value.as_array().ok_or("Schema 分支必須為陣列。")?;
                result.insert(
                    "anyOf".into(),
                    Value::Array(variants.iter().map(normalize).collect::<AppResult<_>>()?),
                );
            }
            "items" => {
                result.insert(key.clone(), normalize(value)?);
            }
            "properties" => {
                let required = object
                    .get("required")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let mut properties = Map::new();
                for (name, property) in value.as_object().ok_or("Schema properties 必須為物件。")?
                {
                    let mut converted = normalize(property)?;
                    if !required.contains(&json!(name)) && !allows_null(&converted) {
                        converted = json!({"anyOf":[converted,{"type":"null"}]});
                    }
                    properties.insert(name.clone(), converted);
                }
                result.insert(
                    "required".into(),
                    json!(properties.keys().collect::<Vec<_>>()),
                );
                result.insert(key.clone(), Value::Object(properties));
                result.insert("additionalProperties".into(), json!(false));
            }
            "required" | "additionalProperties" => (),
            // 這些不是共同傳輸子集：保留在人類可讀說明及原工具檢查，不冒充 schema 強制約束。
            "default" | "minimum" | "maximum" | "minLength" | "maxLength" | "minItems"
            | "maxItems" => (),
            _ => return Err(format!("尚未處理內建 Schema 關鍵字：{key}")),
        }
    }
    let constraints: Vec<_> = object
        .iter()
        .filter(|(k, _)| {
            matches!(
                k.as_str(),
                "minimum" | "maximum" | "minLength" | "maxLength" | "minItems" | "maxItems"
            )
        })
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    if !constraints.is_empty() {
        let previous = result
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("");
        result.insert(
            "description".into(),
            json!(format!("{previous} 執行限制：{}。", constraints.join(", "))),
        );
    }
    // 既有 const／enum 簡寫沒有 type；原生解碼器採明確型別，避免各家推論器猜測。
    if !result.contains_key("type")
        && result
            .get("enum")
            .and_then(Value::as_array)
            .is_some_and(|v| !v.is_empty() && v.iter().all(Value::is_string))
    {
        result.insert("type".into(), json!("string"));
    }
    normalize_type_union(result)
}

/// 契約只允許 type 陣列表達「單一型別加 null」。Office 儲存格與圖表橫軸
/// 可同時接受文字、數字，須轉為非根位置的 anyOf，不能原樣送給網站。
/// 各分支保留既有條件，避免覆蓋原有 anyOf 或遺失 enum／items 等限制。
fn normalize_type_union(mut schema: Map<String, Value>) -> AppResult<Value> {
    let Some(types) = schema.get("type").and_then(Value::as_array).cloned() else {
        return Ok(Value::Object(schema));
    };
    let mut kinds = Vec::new();
    for value in &types {
        let kind = value.as_str().ok_or("Schema type 陣列必須包含型別名稱。")?;
        if ![
            "object", "array", "string", "integer", "number", "boolean", "null",
        ]
        .contains(&kind)
            || kinds.contains(&kind)
        {
            return Err("Schema type 陣列包含未知或重複型別。".into());
        }
        kinds.push(kind);
    }
    if kinds.is_empty() {
        return Err("Schema type 陣列不可空白。".into());
    }
    if kinds.len() == 1 {
        schema.insert("type".into(), json!(kinds[0]));
        return Ok(Value::Object(schema));
    }
    if kinds.len() == 2 && kinds.contains(&"null") {
        return Ok(Value::Object(schema));
    }
    let branches: Vec<_> = kinds
        .into_iter()
        .map(|kind| {
            let mut branch = schema.clone();
            branch.insert("type".into(), json!(kind));
            Value::Object(branch)
        })
        .collect();
    Ok(json!({"anyOf": branches}))
}

fn allows_null(schema: &Value) -> bool {
    schema["type"] == "null"
        || schema["type"]
            .as_array()
            .is_some_and(|a| a.contains(&json!("null")))
        || schema["anyOf"]
            .as_array()
            .is_some_and(|a| a.iter().any(allows_null))
}

pub(super) fn depth(value: &Value) -> usize {
    match value {
        Value::Object(map) => 1 + map.values().map(depth).max().unwrap_or(0),
        Value::Array(array) => 1 + array.iter().map(depth).max().unwrap_or(0),
        _ => 0,
    }
}

/// 驗證本機自己提供的有限 schema；未知關鍵字在 normalize 階段就已阻擋。
fn matches(value: &Value, schema: &Value) -> bool {
    if let Some(choices) = schema["anyOf"].as_array() {
        if !choices.iter().any(|s| matches(value, s)) {
            return false;
        }
    }
    if let Some(choices) = schema["enum"].as_array() {
        if !choices.contains(value) {
            return false;
        }
    }
    let type_matches = |kind: &str| match kind {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "integer" => value.is_i64() || value.is_u64(),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        _ => false,
    };
    if let Some(kind) = schema["type"].as_str() {
        if !type_matches(kind) {
            return false;
        }
    } else if let Some(types) = schema["type"].as_array() {
        if !types.iter().any(|t| t.as_str().is_some_and(type_matches)) {
            return false;
        }
    }
    if let Some(object) = value.as_object() {
        if let Some(required) = schema["required"].as_array() {
            if required
                .iter()
                .any(|k| k.as_str().is_none_or(|k| !object.contains_key(k)))
            {
                return false;
            }
        }
        if let Some(properties) = schema["properties"].as_object() {
            for (name, item) in object {
                match properties.get(name) {
                    Some(s) if !matches(item, s) => return false,
                    None if schema["additionalProperties"] == false => return false,
                    _ => (),
                }
            }
        }
    }
    if let Some(items) = value.as_array() {
        if let Some(item_schema) = schema.get("items") {
            if items.iter().any(|v| !matches(v, item_schema)) {
                return false;
            }
        }
    }
    true
}

pub(super) fn decode_arguments(encoded: &str, schema: &Value) -> AppResult<Value> {
    let args = decode_json(encoded)?;
    if !args.is_object() || !matches(&args, schema) {
        let mut errors = Vec::new();
        diagnose(&args, schema, "$", &mut errors);
        if errors.is_empty() {
            errors.push("$: 參數必須為符合工具規格的物件".into());
        }
        return Err(format!(
            "工具參數不符：{}。最多列出6項；陣列索引從0開始。請補正指定欄位後重試，本輪工具未執行。",
            errors.join("；")
        ));
    }
    Ok(args)
}

/// 只補充固定格式說明，不改寫模型參數，也不放寬 Schema 的接受條件。
/// 幾何修改誤放 operations 時，anyOf 的一般錯誤無法清楚指出兩組陣列的用途。
pub(super) fn argument_repair_hint(name: &str, reason: String) -> String {
    if matches!(name, "cad_edit" | "cad_active_edit") {
        format!("{reason} CAD修改請放在頂層edits陣列，每項為{{handle,kind,value}}；trim/fillet等的boundary、pick、other_handle、radius等參數需放進value物件，不可攤到edits項目本身。operations只放新增圖元，沒有新增時填[]；不要把既有圖元修改移到operations。保留已讀Handle、座標及revision，只修正參數封套；詳細格式見cad-advanced。")
    } else {
        reason
    }
}

/// 拒絕後才補充有限診斷，不改變 matches 的接受條件，也不補猜參數。
/// 只回傳欄位位置及內建規格，避免重送整段程式或檔案內容。
fn diagnose(value: &Value, schema: &Value, path: &str, errors: &mut Vec<String>) {
    if errors.len() >= 6 || matches(value, schema) {
        return;
    }
    if let Some(choices) = schema["anyOf"].as_array() {
        if !choices.iter().any(|s| matches(value, s)) {
            // nullable 包裝中，非 null 值只需檢查原分支。
            let candidates: Vec<_> = choices
                .iter()
                .filter(|s| {
                    s.get("type")
                        .is_none_or(|t| matches(value, &json!({"type": t})))
                })
                .collect();
            // 操作聯集已有kind/action常數時，可依模型明示的標籤定位欄位錯誤。
            // 所有分支都須有單一常數，且只能命中一支；其餘情況保留一般診斷。
            // 這裡只改善拒絕後的訊息，matches的接受條件完全不變。
            let tagged = ["kind", "action"].into_iter().find_map(|tag| {
                let selected = value.get(tag)?;
                if !candidates.iter().all(|choice| {
                    choice["properties"][tag]["enum"]
                        .as_array()
                        .is_some_and(|values| values.len() == 1)
                }) {
                    return None;
                }
                let mut matching = candidates
                    .iter()
                    .copied()
                    .filter(|choice| choice["properties"][tag]["enum"].get(0) == Some(selected));
                let first = matching.next()?;
                matching.next().is_none().then_some(first)
            });
            if let Some(rule) = tagged {
                diagnose(value, rule, path, errors);
            } else if candidates.len() == 1 {
                diagnose(value, candidates[0], path, errors);
            } else {
                errors.push(format!("{path}: 不符合允許的型別或任一參數格式"));
            }
            return;
        }
    }
    if let Some(kind) = schema.get("type") {
        if !matches(value, &json!({"type":kind})) {
            errors.push(format!("{path}: 型別應為 {kind}"));
            return;
        }
    }
    if let Some(choices) = schema["enum"].as_array() {
        if !choices.contains(value) {
            errors.push(format!("{path}: 值應為 {}", json!(choices)));
            return;
        }
    }
    if let (Some(object), Some(properties)) = (value.as_object(), schema["properties"].as_object())
    {
        if let Some(required) = schema["required"].as_array() {
            for name in required.iter().filter_map(Value::as_str) {
                if !object.contains_key(name) && errors.len() < 6 {
                    errors.push(format!("{}: 缺少必填欄位", field_path(path, name)));
                }
            }
        }
        for (name, item) in object {
            if errors.len() >= 6 {
                break;
            }
            let field = field_path(path, name);
            match properties.get(name) {
                Some(rule) => diagnose(item, rule, &field, errors),
                None if schema["additionalProperties"] == false => {
                    errors.push(format!("{field}: 未提供的欄位，請移除或修正欄名"));
                }
                _ => (),
            }
        }
    }
    if let (Some(items), Some(rule)) = (value.as_array(), schema.get("items")) {
        for (index, item) in items.iter().enumerate() {
            if errors.len() >= 6 {
                break;
            }
            diagnose(item, rule, &format!("{path}[{index}]"), errors);
        }
    }
}

/// 未知欄名由模型提供，需限制長度及轉義控制字元；不輸出其值。
fn field_path(parent: &str, name: &str) -> String {
    let parent: String = parent.chars().take(200).collect();
    let short: String = name.chars().take(64).collect();
    if short.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        format!("{parent}.{short}")
    } else {
        format!("{parent}[{}]", json!(short))
    }
}

pub(super) fn restore_optional(name: &str, mut args: Value) -> AppResult<Value> {
    let source = catalog()?;
    let tool = source["tools"]
        .as_array()
        .and_then(|tools| tools.iter().find(|t| t["function"]["name"] == name))
        .ok_or("未知工具，未轉換參數。")?;
    restore(&mut args, &tool["function"]["parameters"])?;
    Ok(args)
}
fn restore(value: &mut Value, source: &Value) -> AppResult<()> {
    if let Some(variants) = source["oneOf"]
        .as_array()
        .or_else(|| source["anyOf"].as_array())
    {
        for variant in variants {
            if matches(value, &normalize(variant)?) {
                return restore(value, variant);
            }
        }
        // 舊checkpoint提交時可能還沒有新加入的可選欄位。只依唯一明示標籤
        // 還原省略預設，不放寬decode驗證、不補必填值，也不猜測未知操作。
        for tag in ["kind", "action"] {
            if let Some(selected) = value.get(tag) {
                let matching: Vec<_> = variants
                    .iter()
                    .filter(|variant| {
                        variant["properties"][tag]["enum"]
                            .as_array()
                            .is_some_and(|values| {
                                values.len() == 1 && values.first() == Some(selected)
                            })
                    })
                    .collect();
                if matching.len() == 1 {
                    return restore(value, matching[0]);
                }
            }
        }
    }
    if let (Some(object), Some(properties)) =
        (value.as_object_mut(), source["properties"].as_object())
    {
        for (name, property) in properties {
            let required = source["required"]
                .as_array()
                .is_some_and(|r| r.contains(&json!(name)));
            if !required && object.get(name).is_some_and(Value::is_null) && !allows_null(property) {
                object.remove(name);
            } else if let Some(item) = object.get_mut(name) {
                restore(item, property)?;
            }
        }
    } else if let Some(items) = value.as_array_mut() {
        if let Some(schema) = source.get("items") {
            for item in items {
                restore(item, schema)?;
            }
        }
    }
    Ok(())
}

/// serde_json::Value 預設覆蓋重複鍵；代理輸入改為拒絕，避免兩層驗證看到不同操作。
pub(super) fn decode_json(text: &str) -> AppResult<Value> {
    #[derive(Debug)]
    struct Unique(Value);
    impl<'de> Deserialize<'de> for Unique {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            struct JsonVisitor;
            impl<'de> Visitor<'de> for JsonVisitor {
                type Value = Unique;
                fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                    f.write_str("JSON without duplicate keys")
                }
                fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Unique, A::Error> {
                    let mut value = Map::new();
                    while let Some((key, item)) = map.next_entry::<String, Unique>()? {
                        if value.insert(key, item.0).is_some() {
                            return Err(serde::de::Error::custom("duplicate JSON key"));
                        }
                    }
                    Ok(Unique(Value::Object(value)))
                }
                fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Unique, A::Error> {
                    let mut values = Vec::new();
                    while let Some(item) = seq.next_element::<Unique>()? {
                        values.push(item.0);
                    }
                    Ok(Unique(Value::Array(values)))
                }
                fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Unique, E> {
                    Ok(Unique(json!(v)))
                }
                fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Unique, E> {
                    Ok(Unique(json!(v)))
                }
                fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Unique, E> {
                    Ok(Unique(json!(v)))
                }
                fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Unique, E> {
                    Ok(Unique(json!(v)))
                }
                fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Unique, E> {
                    Ok(Unique(json!(v)))
                }
                fn visit_unit<E: serde::de::Error>(self) -> Result<Unique, E> {
                    Ok(Unique(Value::Null))
                }
            }
            deserializer.deserialize_any(JsonVisitor)
        }
    }
    serde_json::from_str::<Unique>(text)
        .map(|v| v.0)
        .map_err(|e| format!("JSON 無法解析（行 {}、欄 {}）：{e}", e.line(), e.column()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_preserves_the_progress_note_contract() {
        // CAD 工具曾漏掉此欄位，模型只能翻頁卻無法累積核對結果。
        // 檢查整份目錄，避免未來新增工具再次繞過共用交接規則。
        for strict in [false, true] {
            for tool in definitions(strict).unwrap() {
                let parameters = &tool["function"]["parameters"];
                assert_eq!(
                    parameters["properties"]["progress_note"]["type"], "string",
                    "{} 缺少交接筆記",
                    tool["function"]["name"]
                );
                assert!(parameters["required"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("progress_note")));
            }
        }
    }

    #[test]
    fn cad_read_accepts_a_note_with_the_next_page() {
        let tools = definitions(false).unwrap();
        let schema = &tools
            .iter()
            .find(|tool| tool["function"]["name"] == "cad_active_read")
            .unwrap()["function"]["parameters"];
        let args = json!({"version":"latest","offset":100,
            "progress_note":"已核對第一頁外框與安裝孔；接著閱讀尚未提供的圖元。"});
        assert!(decode_arguments(&args.to_string(), schema).is_ok());
    }

    #[test]
    fn python_input_errors_locate_missing_kind_and_unknown_key_without_echoing_code() {
        let source = catalog().unwrap();
        let schema = &source["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["function"]["name"] == "run_python")
            .unwrap()["function"]["parameters"];
        let mut args = json!({"purpose":"LOG分析", "code":"private code is not echoed", "inputs":[
            {"name":"day1","path":"LOG/day1.log","kind":"text"},
            {"name":"day2","path":"LOG/day2.log","kind":"text"},
            {"name":"day3","path":"LOG/day3.log","錯字欄名":"private value"}
        ]});
        let error = decode_arguments(&args.to_string(), schema).unwrap_err();
        assert!(error.contains("$.inputs[2].kind: 缺少必填欄位"), "{error}");
        assert!(error.contains("錯字欄名") && error.contains("未提供的欄位"));
        assert!(!error.contains("private"));
        args["inputs"][2]
            .as_object_mut()
            .unwrap()
            .remove("錯字欄名");
        args["inputs"][2]["kind"] = json!("text");
        assert!(decode_arguments(&args.to_string(), schema).is_ok());
        args["inputs"][2]["kind"] = json!("wrong");
        assert!(decode_arguments(&args.to_string(), schema)
            .unwrap_err()
            .contains("值應為"));
    }

    #[test]
    fn diagnostics_follow_nullable_arrays_and_limit_model_supplied_field_names() {
        let schema = normalize(&json!({"type":"object","properties":{
            "inputs":{"type":"array","items":{"type":"object","properties":{
                "name":{"type":"string"}},"required":["name"]}}
        }}))
        .unwrap();
        let args = json!({"inputs":[{"name":2},{"name":false}]});
        let error = decode_arguments(&args.to_string(), &schema).unwrap_err();
        assert!(error.contains("$.inputs[0].name: 型別應為"));
        assert!(error.contains("$.inputs[1].name: 型別應為"));
        let mut bad = Map::new();
        for i in 0..100 {
            bad.insert(format!("{i}\n{}", "x".repeat(10000)), json!("secret value"));
        }
        let error = decode_arguments(&Value::Object(bad).to_string(), &schema).unwrap_err();
        assert!(error.len() < 3000);
        assert!(!error.contains("secret value") && !error.contains('\n'));
    }

    #[test]
    fn cad_operation_diagnostics_follow_kind_for_missing_and_extra_fields() {
        let source = catalog().unwrap();
        let operation = source["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["function"]["name"] == "cad_draw")
            .unwrap()["function"]["parameters"]["properties"]["operations"]["items"]
            .clone();
        let schema = normalize(&json!({"type":"object", "required":["operations"],
            "properties":{"operations":{"type":"array", "items":operation}}}))
        .unwrap();
        let mut args = json!({"operations":[{"kind":"leader", "color":1,
            "points":[[100,390],[150,470],[230,470]], "text":"private annotation"}]});
        let error = decode_arguments(&args.to_string(), &schema).unwrap_err();
        assert!(
            error.contains("$.operations[0].height: 缺少必填欄位"),
            "{error}"
        );
        assert!(!error.contains("private annotation"));
        assert!(!matches(&args, &schema));
        args["operations"][0]["height"] = json!(3);
        args["operations"][0]["layer"] = Value::Null;
        assert!(decode_arguments(&args.to_string(), &schema).is_ok());
        args["operations"][0] = json!({"kind":"command", "command":"ZOOM E", "color":7});
        let error = decode_arguments(&args.to_string(), &schema).unwrap_err();
        assert!(error.contains("$.operations[0].color: 未提供的欄位"));
        // ZOOM E原本就是合法別名，不能為改善診斷而改動接受條件。
        args["operations"][0]
            .as_object_mut()
            .unwrap()
            .remove("color");
        assert!(decode_arguments(&args.to_string(), &schema).is_ok());
        args["operations"][0] = json!({"kind":"circle", "color":4, "center":[0,0],
            "radius":5, "layer":null, "line_style":{"name":"unsupported", "scale":1}});
        let error = decode_arguments(&args.to_string(), &schema).unwrap_err();
        assert!(error.contains("$.operations[0].line_style.name: 值應為"));
    }

    #[test]
    fn union_diagnostics_do_not_choose_ambiguous_or_unknown_tags() {
        let schema = normalize(&json!({"type":"object", "required":["operation"],
        "properties":{"operation":{"oneOf":[
            {"type":"object", "required":["kind","value"], "properties":{
                "kind":{"const":"item"},"value":{"type":"string"}}},
            {"type":"object", "required":["kind","value"], "properties":{
                "kind":{"const":"item"},"value":{"type":"number"}}}
        ]}}}))
        .unwrap();
        for tag in ["item", "unknown"] {
            let args = json!({"operation":{"kind":tag,"value":false}});
            let error = decode_arguments(&args.to_string(), &schema).unwrap_err();
            assert!(error.contains("$.operation: 不符合允許"));
            assert!(!error.contains("$.operation.value"));
            assert!(!matches(&args, &schema));
        }
    }

    #[test]
    fn mixed_types_preserve_values_and_existing_constraints() {
        // Office 表格允許空白；圖表橫軸不允許 null，兩者不可互相放寬。
        for nullable in [false, true] {
            let types = if nullable {
                json!(["string", "number", "null"])
            } else {
                json!(["string", "number"])
            };
            let converted = normalize(&json!({"type":types})).unwrap();
            assert!(converted.get("type").is_none());
            assert!(matches(&json!("A"), &converted));
            assert!(matches(&json!(1.25), &converted));
            assert_eq!(matches(&Value::Null, &converted), nullable);
            for invalid in [json!(true), json!([]), json!({})] {
                assert!(!matches(&invalid, &converted));
            }
        }
        let constrained = normalize(&json!({
            "type":["string","number"], "enum":["A",2],
            "anyOf":[{"type":"string"},{"type":"number"}]
        }))
        .unwrap();
        assert!(matches(&json!("A"), &constrained));
        assert!(matches(&json!(2), &constrained));
        assert!(!matches(&json!("B"), &constrained));
        assert!(!matches(&json!(3), &constrained));
    }

    #[test]
    fn cad_optional_line_style_uses_null_on_wire_and_is_omitted_for_original_tool() {
        let old=restore_optional("cad_draw",json!({"name":"old.dwg","version":"Icad.Application.1","operations":[{"kind":"line","color":7,"start":[0,0],"end":[10,0],"line_style":null}]})).unwrap();
        assert!(old["operations"][0].get("line_style").is_none());
        assert!(old["operations"][0].get("layer").is_none());
        // 傳輸Schema採所有欄位必填；原工具的可選線型以null表示，不能教模型省略。
        for strict in [false, true] {
            let tools = definitions(strict).unwrap();
            let draw = tools
                .iter()
                .find(|tool| tool["function"]["name"] == "cad_draw")
                .unwrap();
            let schema = &draw["function"]["parameters"];
            let args = json!({"name":"nullable-check.dwg","version":"Icad.Application.1",
                "progress_note":"建立新圖測試可選線型，不讀取或修改其他文件。",
                "operations":[{"kind":"line","color":7,"start":[0,0],"end":[10,0],"line_style":null,"layer":null}]});
            assert!(decode_arguments(&args.to_string(), schema).is_ok());
            let restored = restore_optional("cad_draw", args.clone()).unwrap();
            assert!(restored["operations"][0].get("line_style").is_none());
            assert!(restored["operations"][0].get("layer").is_none());
            assert_eq!(restored["operations"][0]["end"], json!([10, 0]));
            let mut missing = args;
            missing["operations"][0]
                .as_object_mut()
                .unwrap()
                .remove("line_style");
            assert!(decode_arguments(&missing.to_string(), schema).is_err());
        }
    }

    #[test]
    fn cad_repair_hint_explains_edit_envelope_without_rewriting_arguments() {
        for name in ["cad_edit", "cad_active_edit"] {
            let hint = argument_repair_hint(name, "$.operations[0]: anyOf不符".into());
            assert!(hint.starts_with("$.operations[0]: anyOf不符"));
            assert!(hint.contains("{handle,kind,value}"));
            assert!(hint.contains("operations只放新增圖元"));
            assert!(hint.contains("value物件"));
        }
        assert_eq!(
            argument_repair_hint("read_file", "原始錯誤".into()),
            "原始錯誤"
        );
    }

    #[test]
    fn type_arrays_reject_invalid_kinds_and_normalize_single_kind() {
        for types in [
            json!([]),
            json!(["string", "string"]),
            json!(["unknown"]),
            json!([1]),
        ] {
            assert!(normalize(&json!({"type":types})).is_err());
        }
        assert_eq!(
            normalize(&json!({"type":["string"]})).unwrap()["type"],
            "string"
        );
        assert_eq!(
            normalize(&json!({"type":["number","null"]})).unwrap()["type"],
            json!(["number", "null"])
        );
    }
}
