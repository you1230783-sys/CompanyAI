//! 將內建工具目錄轉為公布的可攜 Schema；限制仍由工具實作檢查。
//! 使用明確白名單轉換，不把模型提交的未知 Schema 當成可執行設定。
use crate::AppResult;
use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{json, Map, Value};

fn catalog() -> AppResult<Value> {
    serde_json::from_str(include_str!("../tools.json")).map_err(|e| e.to_string())
}

pub(super) fn definitions(strict: bool) -> AppResult<Vec<Value>> {
    let source = catalog()?;
    source["tools"]
        .as_array()
        .ok_or("內建工具目錄遺失。")?
        .iter()
        .map(|tool| {
            let mut tool = tool.clone();
            tool["function"]["parameters"] = normalize(&tool["function"]["parameters"])?;
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
    Ok(Value::Object(result))
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
        return Err("工具參數缺少欄位、型別不符或包含未提供欄位；本輪工具未執行。".into());
    }
    Ok(args)
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
