//! LOG 事件以成功工具的單一原文列為依據；時間、來源與版本均由原生結果取得。
use crate::AppResult;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub label: String,
    /// 同一機台／批次／處理階段的配對鍵，由模型依原文判斷，空字串不配對。
    pub group: String,
    pub phase: String,
    pub evidence: super::analysis::EvidenceRef,
}

pub fn from_evidence(event: &Event, request: &Value, result: &Value) -> AppResult<Value> {
    if event.label.trim().is_empty()
        || event.label.chars().count() > 200
        || event.group.chars().count() > 120
        || !matches!(
            event.phase.as_str(),
            "start" | "end" | "cancel" | "retry" | "error" | "recovery" | "other"
        )
    {
        return Err("事件需1–200字說明、120字內分組及合法phase。".into());
    }
    let tool = request["tool"].as_str().unwrap_or("");
    let prefix = match tool {
        "search_logs" => "/matches/",
        "read_log" => "/lines/",
        _ => return Err("事件只能引用 search_logs／read_log 的原文列。".into()),
    };
    let index = event
        .evidence
        .pointer
        .strip_prefix(prefix)
        .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        .ok_or("事件證據必須指向一筆 /matches/N 或 /lines/N。")?;
    let _ = index.parse::<usize>().map_err(|_| "事件列號不正確。")?;
    let row = super::analysis::select(result, &event.evidence.pointer)?;
    if row["continued"] == true
        || row["column"].as_u64().unwrap_or(0) > 0
        || row["excerpt"]["excerpt_truncated"] == true
    {
        return Err("事件原文尚未完整取得，請先 read_log 讀回完整行。".into());
    }
    let path = row["path"]
        .as_str()
        .or_else(|| request["path"].as_str())
        .ok_or("事件缺少來源路徑。")?;
    let revision = row
        .get("revision")
        .unwrap_or(&result["result"]["revision"])
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("事件缺少來源版本。")?;
    let line = row["line"]
        .as_u64()
        .filter(|n| *n > 0)
        .ok_or("事件缺少有效行號。")?;
    let text = row["text"]
        .as_str()
        .or_else(|| row["excerpt"]["text"].as_str())
        .ok_or("事件缺少原文。")?;
    let stamp = if tool == "search_logs" {
        json!({"date":row["date"],"time_millis":row["time_millis"],"inherited":row["time_inherited"]})
    } else {
        super::logs::event_stamp(text)
    };
    let id = super::text::revision(&format!("{path}\n{revision}\n{line}"));
    Ok(
        json!({"id":id,"label":event.label,"group":event.group,"phase":event.phase,"path":path,"revision":revision,
        "line":row["line"],"text":text,"date":stamp["date"],"time_millis":stamp["time_millis"],"time_inherited":stamp["inherited"],
        "operation_id":event.evidence.operation_id,"pointer":event.evidence.pointer,"annotation":"事件分類與分組為AI判斷；時間順序不代表因果。"}),
    )
}

/// 只對日期時間明確、單一起點的同組事件配對。重複起點與無日期資料不猜測配對。
pub fn reconcile(events: &mut [Value]) {
    use std::collections::{BTreeMap, BTreeSet};
    // 先依原文行號檢查倒退時間；不能先排序而掩蓋校時／跨日未標日期的問題。
    let mut by_source = BTreeMap::<(String, String), Vec<usize>>::new();
    let mut revisions = BTreeMap::<String, BTreeSet<String>>::new();
    for (i, event) in events.iter().enumerate() {
        let path = event["path"]
            .as_str()
            .unwrap_or("")
            .replace('\\', "/")
            .to_lowercase();
        let revision = event["revision"].as_str().unwrap_or("").to_owned();
        revisions
            .entry(path.clone())
            .or_default()
            .insert(revision.clone());
        by_source.entry((path, revision)).or_default().push(i);
    }
    let mut uncertain_groups = BTreeSet::new();
    for ((path, _), indices) in &mut by_source {
        indices.sort_by_key(|&i| events[i]["line"].as_u64().unwrap_or(0));
        let mut previous = None;
        let mut inconsistent = revisions.get(path).is_some_and(|r| r.len() > 1);
        for &i in indices.iter() {
            if let (Some(date), Some(time)) = (
                events[i]["date"].as_str(),
                events[i]["time_millis"].as_u64(),
            ) {
                let current = (date.to_owned(), time);
                if previous.as_ref().is_some_and(|p| p > &current) {
                    inconsistent = true;
                }
                previous = Some(current);
            }
        }
        if inconsistent {
            for &i in indices.iter() {
                uncertain_groups.insert(events[i]["group"].as_str().unwrap_or("").to_owned());
            }
        }
    }
    events.sort_by_key(|e| {
        (
            e["date"].as_str().unwrap_or("~").to_owned(),
            e["time_millis"].as_u64().unwrap_or(u64::MAX),
            e["path"].as_str().unwrap_or("").to_owned(),
            e["line"].as_u64().unwrap_or(0),
        )
    });
    let mut open = std::collections::BTreeMap::<String, Vec<usize>>::new();
    for i in 0..events.len() {
        events[i]["pair_status"] = json!("not_applicable");
        events[i]["paired_with"] = Value::Null;
        let group = events[i]["group"].as_str().unwrap_or("").to_owned();
        let phase = events[i]["phase"].as_str().unwrap_or("").to_owned();
        if !matches!(phase.as_str(), "start" | "end" | "cancel") {
            continue;
        }
        if group.is_empty()
            || uncertain_groups.contains(&group)
            || events[i]["date"].is_null()
            || events[i]["time_millis"].is_null()
            || events[i]["time_inherited"] == true
        {
            events[i]["pair_status"] = json!("uncertain");
            continue;
        }
        let pending = open.entry(group).or_default();
        if phase == "start" {
            events[i]["pair_status"] = json!("missing_end");
            pending.push(i);
        } else if pending.len() == 1 {
            let first = pending.pop().unwrap_or(i);
            if events[first]["date"] == events[i]["date"]
                && events[first]["time_millis"] == events[i]["time_millis"]
            {
                events[first]["pair_status"] = json!("uncertain");
                events[i]["pair_status"] = json!("uncertain");
            } else {
                events[first]["pair_status"] = json!("paired");
                events[i]["pair_status"] = json!("paired");
                events[first]["paired_with"] = events[i]["id"].clone();
                events[i]["paired_with"] = events[first]["id"].clone();
            }
        } else if pending.is_empty() {
            events[i]["pair_status"] = json!("missing_start");
        } else {
            for first in pending.drain(..) {
                events[first]["pair_status"] = json!("ambiguous");
            }
            events[i]["pair_status"] = json!("ambiguous");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event(phase: &str, pointer: &str) -> Event {
        Event {
            label: "事件".into(),
            group: "機台A/批次1".into(),
            phase: phase.into(),
            evidence: super::super::analysis::EvidenceRef {
                operation_id: "read".into(),
                pointer: pointer.into(),
            },
        }
    }
    fn row(phase: &str, line: u64, time: u64) -> Value {
        json!({"id":format!("id{line}"),"phase":phase,"group":"batch","date":"2026-10-12", "time_millis":time,"path":"a.log","revision":"r1","line":line})
    }
    #[test]
    fn events_require_successful_complete_source_and_native_timestamp() {
        let request = json!({"tool":"read_log","path":"a.log"});
        let result = json!({"ok":true,"result":{"revision":"r1","lines":[{"line":5,"column":0,"continued":false,"text":"2026/10/12, 10:03:21.084 Start"}]}});
        let e = event("start", "/lines/0");
        let value = from_evidence(&e, &request, &result).unwrap();
        assert_eq!(value["date"], "2026-10-12");
        assert_eq!(value["time_millis"], 36_201_084);
        assert_eq!(value["revision"], "r1");
        assert_eq!(value["line"], 5);
        let mut invalid = result.clone();
        invalid["ok"] = json!(false);
        assert!(from_evidence(&e, &request, &invalid).is_err());
        invalid = result.clone();
        invalid["result"]["lines"][0]["continued"] = json!(true);
        assert!(from_evidence(&e, &request, &invalid).is_err());
        invalid = result.clone();
        invalid["result"]["revision"] = Value::Null;
        assert!(from_evidence(&e, &request, &invalid).is_err());
        assert!(from_evidence(&event("start", "/lines/0/text"), &request, &result).is_err());
        assert!(from_evidence(&e, &json!({"tool":"run_python"}), &result).is_err());
    }
    #[test]
    fn pairing_marks_missing_ambiguous_undated_and_reversed_events() {
        let mut events = vec![row("start", 1, 1), row("end", 2, 2)];
        reconcile(&mut events);
        assert!(events.iter().all(|e| e["pair_status"] == "paired"));
        let original = events.clone();
        reconcile(&mut events);
        assert_eq!(events, original);
        let mut events = vec![row("end", 1, 1), row("start", 2, 2)];
        reconcile(&mut events);
        assert_eq!(events[0]["pair_status"], "missing_start");
        assert_eq!(events[1]["pair_status"], "missing_end");
        let mut events = vec![row("start", 1, 1), row("start", 2, 2), row("end", 3, 3)];
        reconcile(&mut events);
        assert!(events.iter().all(|e| e["pair_status"] == "ambiguous"));
        for mode in ["reversed", "revision", "undated", "equal", "inherited"] {
            let mut events = vec![row("start", 1, 1), row("end", 2, 2)];
            match mode {
                "reversed" => events[0]["time_millis"] = json!(3),
                "revision" => events[1]["revision"] = json!("r2"),
                "undated" => events[1]["date"] = Value::Null,
                "equal" => events[1]["time_millis"] = json!(1),
                _ => events[1]["time_inherited"] = json!(true),
            }
            reconcile(&mut events);
            assert!(
                events.iter().all(|e| e["pair_status"] != "paired"),
                "{mode}"
            );
        }
    }
}
