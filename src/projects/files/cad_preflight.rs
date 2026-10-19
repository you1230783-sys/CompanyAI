//! 在複製DWG、要求桌面確認或啟動CAD之前，一次列出可判定的批次問題。
//! 這是早期參數檢查；原生端的完整圖面／幾何／Undo驗證仍然保留。
use super::*;
use std::collections::{BTreeMap, BTreeSet};

const GEOMETRY_EDITS: &[&str] = &[
    "copy", "rotate", "mirror", "array", "offset", "trim", "extend", "fillet", "layer",
];

pub(super) fn check(
    edits: &[Value],
    operations: &[Value],
    read_handles: &[Value],
    snapshot: Option<&super::super::cad_index::Snapshot>,
) -> Option<Value> {
    let mut issues = Vec::new();
    if edits.len() > 100 || operations.len() > 100 {
        issues
            .push(json!({"code":"batch_limit","message":"每批最多100項修改及100項新增，請拆批。"}));
    } else {
        let geometry = edits
            .iter()
            .any(|e| GEOMETRY_EDITS.contains(&e["kind"].as_str().unwrap_or("")));
        if geometry
            && (!operations.is_empty()
                || edits
                    .iter()
                    .any(|e| !GEOMETRY_EDITS.contains(&e["kind"].as_str().unwrap_or(""))))
        {
            issues.push(json!({"code":"mixed_geometry_batch","message":"幾何修改需獨立一批；operations必須為[]，改字／半徑另批處理。ZOOM等指令也不能放入此批。"}));
        }
        let known: BTreeSet<_> = read_handles.iter().filter_map(Value::as_str).collect();
        let mut missing = BTreeSet::new();
        let mut used = BTreeMap::<String, Vec<usize>>::new();
        let rows: BTreeMap<_, _> = snapshot
            .into_iter()
            .flat_map(|s| &s.rows)
            .filter_map(|row| row["handle"].as_str().map(|h| (h, row)))
            .collect();
        for (index, edit) in edits.iter().enumerate() {
            let kind = edit["kind"].as_str().unwrap_or("");
            for (role, handle) in [
                ("handle", edit["handle"].as_str()),
                (
                    "boundary",
                    edit["value"].get("boundary").and_then(Value::as_str),
                ),
                (
                    "other_handle",
                    edit["value"].get("other_handle").and_then(Value::as_str),
                ),
            ] {
                let Some(handle) = handle else {
                    continue;
                };
                if !known.contains(handle) {
                    missing.insert(handle.to_owned());
                }
                // trim／extend邊界只讀；fillet第二條線會被修改，也必須參與重複檢查。
                if role == "handle" || (kind == "fillet" && role == "other_handle") {
                    used.entry(handle.to_owned()).or_default().push(index + 1);
                    if let Some(row) = rows.get(handle) {
                        if row["allowed_edits"].as_array().is_some_and(|allowed| {
                            !allowed.iter().any(|v| v.as_str() == Some(kind))
                        }) {
                            issues.push(json!({"code":"edit_not_allowed","handle":handle,"edit":index+1,"kind":kind,
                                "allowed_edits":row["allowed_edits"],"message":format!("Handle {handle} 未開放 {kind}，請依allowed_edits修改計畫。此限制不會因重新讀取而解除。" )}));
                        }
                    }
                }
            }
        }
        if !cad::edits_are_read(edits, read_handles) && missing.is_empty() {
            issues.push(json!({"code":"invalid_handle","message":"Handle必須取自已讀詳細結果，主／副目標皆需為有效字串，不可填null或其他型別。"}));
        }
        if !missing.is_empty() {
            let names = missing.iter().cloned().collect::<Vec<_>>().join(", ");
            issues.push(json!({"code":"details_not_delivered","handles":missing,
                "message":format!("Handle {names} 尚未交付可修改的詳細資料；使用同一snapshot_id的cad_query(details=true)按handles取得，最多10筆一頁。不必重建已有效的快照。" )}));
        }
        let repeated: BTreeMap<_, _> = used
            .into_iter()
            .filter(|(_, positions)| positions.len() > 1)
            .collect();
        if !repeated.is_empty() {
            let names = repeated.keys().cloned().collect::<Vec<_>>().join(", ");
            issues.push(json!({"code":"duplicate_edit_handle","handles_and_edit_numbers":repeated,
                "message":format!("Handle {names} 在同批重複使用（包含fillet第二條線）；每個修改Handle每批一次，offset／mirror／array等動作請分到後續批次。" )}));
        }
    }
    if issues.is_empty() {
        return None;
    }
    let messages: Vec<_> = issues
        .iter()
        .filter_map(|v| v["message"].as_str())
        .collect();
    Some(
        json!({"status":"PRECHECK_REJECTED","native_executed":false,"drawing_started":false,
        "completed_operations":0,"saved":false,"issues":issues,"snapshot_id":snapshot.map(|s|&s.id),
        "recovery":"correct_parameters","error":format!("寫入前檢查未通過：{} CAD尚未啟動本次修改，未修改圖元。請一次處理上述問題後再提交。",messages.join("；"))}),
    )
}

/// 工具程序回傳正常，不代表CAD操作成功。將內層失敗提升到共同外層，
/// 讓活動列、失敗計數及續接狀態都看到相同事實，仍保留完整原生結果。
pub(super) fn normalize_outcome(tool: &Tool, mut envelope: Value) -> Value {
    if !matches!(
        tool,
        Tool::CadEdit { .. }
            | Tool::CadActiveEdit { .. }
            | Tool::CadRead { .. }
            | Tool::CadActiveRead { .. }
            | Tool::CadDraw { .. }
            | Tool::CadPlanCommit { .. }
            | Tool::CadExcelApply { .. }
    ) || envelope["ok"] != true
    {
        return envelope;
    }
    if matches!(
        envelope["result"]["status"].as_str(),
        Some("FAILED" | "UNKNOWN" | "PRECHECK_REJECTED")
    ) {
        envelope["ok"] = json!(false);
        envelope["error"] = json!(envelope["result"]["error"]
            .as_str()
            .unwrap_or("CAD沒有回報修改成功；請依原生結果確認是否已動到圖面，勿重播未知結果。"));
        envelope["retry_same_operation"] = json!(false);
    }
    envelope
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_all_batch_problems_without_starting_cad() {
        let edits = vec![
            json!({"handle":"A","kind":"offset","value":{"distance":5}}),
            json!({"handle":"A","kind":"mirror","value":{}}),
            json!({"handle":"B","kind":"fillet","value":{"other_handle":"C"}}),
        ];
        let result = check(&edits, &[json!({"kind":"zoom_extents"})], &[], None).unwrap();
        let codes: Vec<_> = result["issues"]
            .as_array()
            .unwrap()
            .iter()
            .map(|issue| issue["code"].as_str().unwrap())
            .collect();
        assert!(codes.contains(&"mixed_geometry_batch"));
        assert!(codes.contains(&"details_not_delivered"));
        assert!(codes.contains(&"duplicate_edit_handle"));
        assert_eq!(result["native_executed"], false);
        assert_eq!(result["saved"], false);
        assert!(result["error"].as_str().unwrap().contains("A, B, C"));
    }

    #[test]
    fn readonly_boundary_can_repeat_but_fillet_secondary_cannot() {
        let handles = vec![json!("A"), json!("B"), json!("C")];
        let trim = json!({"handle":"A","kind":"trim","value":{"boundary":"C"}});
        let extend = json!({"handle":"B","kind":"extend","value":{"boundary":"C"}});
        assert!(check(&[trim.clone(), extend], &[], &handles, None).is_none());
        let fillet = json!({"handle":"B","kind":"fillet","value":{"other_handle":"A"}});
        assert_eq!(
            check(&[trim, fillet], &[], &handles, None).unwrap()["issues"][0]["code"],
            "duplicate_edit_handle"
        );
    }

    #[test]
    fn local_allowed_edits_are_enforced_before_native_execution() {
        let snapshot = crate::projects::cad_index::Snapshot::new(json!({"revision":"r",
            "entity_count":1,"entities":[{"handle":"A","allowed_edits":["color"]}]}))
        .unwrap();
        let edit = json!({"handle":"A","kind":"rotate","value":{"angle":90}});
        let result = check(&[edit], &[], &[json!("A")], Some(&snapshot)).unwrap();
        assert_eq!(result["issues"][0]["code"], "edit_not_allowed");
    }

    #[test]
    fn native_failure_is_not_reported_as_success() {
        let tool = Tool::CadEdit {
            path: "input.dwg".into(),
            revision: "r".into(),
            name: "output.dwg".into(),
            edits: vec![],
            operations: vec![],
        };
        for status in ["FAILED", "UNKNOWN", "PRECHECK_REJECTED"] {
            let result = normalize_outcome(
                &tool,
                json!({"ok":true,"result":{"status":status,"error":"synthetic failure"}}),
            );
            assert_eq!(result["ok"], false);
            assert_eq!(result["result"]["status"], status);
            assert_eq!(result["retry_same_operation"], false);
        }
        assert_eq!(
            normalize_outcome(
                &tool,
                json!({"ok":true,"result":{"status":"DATA_VERIFIED_VISUAL_PENDING"}})
            )["ok"],
            true
        );
    }
}
