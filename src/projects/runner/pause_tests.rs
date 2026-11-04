
use super::*;
#[test]
fn activity_keeps_early_entries_and_restores_more_than_one_page() {
    let root = std::env::temp_dir().join(format!("companyai-history-{}", jobs::new_id().unwrap()));
    let mut activity = Vec::new();
    for index in 0..512 {
        report(&mut activity, &mut |_| {}, format!("工具紀錄 {index}"));
    }
    assert_eq!(activity.len(), 512);
    assert_eq!(activity[0].to_string(), "工具紀錄 0");
    let path = root.join("project-runs/history.dpapi");
    checkpoint(&path, &json!({"activity":activity})).unwrap();
    let mut restored = recover_activity(&root, "history").unwrap();
    report(&mut restored, &mut |_| {}, "續接後的紀錄".into());
    checkpoint(&path, &json!({"activity":restored})).unwrap();
    let final_history = recover_activity(&root, "history").unwrap();
    assert_eq!(final_history.len(), 513);
    assert_eq!(final_history[0].to_string(), "工具紀錄 0");
    assert_eq!(final_history.last().unwrap().to_string(), "續接後的紀錄");
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(root.join("project-runs")).unwrap();
    std::fs::remove_dir(root).unwrap();
}
#[test]
fn saved_design_and_precreation_error_are_visible_without_a_dwg_artifact() {
    let empty = json!({"artifacts":[],"blocked_after_error":false,
            "last_execution":null,"design_planning":null});
    assert!(!has_cad_context(&empty));
    let mut planned = empty.clone();
    planned["design_planning"] =
        json!({"phase":"ready_to_draw","design":{"working_note":"8×12m兩層"}});
    assert!(has_cad_context(&planned));
    let mut rejected = empty.clone();
    rejected["last_execution"] = json!({"status":"FAILED","drawing_started":false,"saved":false});
    assert!(has_cad_context(&rejected));
    let mut saved = empty;
    saved["artifacts"] = json!([{"path":"new.dwg"}]);
    assert!(has_cad_context(&saved));
}
#[test]
fn segment_budget_is_twenty_four_hours_and_cancellation_stays_separate() {
    assert_eq!(SEGMENT_BUDGET.as_secs(), 86400);
    assert!(check_cancel(&AtomicBool::new(false)).is_ok());
    assert!(check_cancel(&AtomicBool::new(true)).is_err());
}
#[test]
fn tool_and_model_quotas_pause_at_the_exact_boundary() {
    assert!(!quota_reached(20, 20));
    assert!(!quota_reached(79, 59));
    assert!(quota_reached(80, 59));
    assert!(quota_reached(79, 60));
}
#[test]
fn review_journal_restores_only_the_selected_encrypted_artifact() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(".build")
        .join(format!("review-journal-{}", jobs::new_id().unwrap()));
    std::fs::create_dir_all(root.join("project-runs")).unwrap();
    let id = jobs::new_id().unwrap();
    let path = root.join("project-runs").join(format!("{id}.dpapi"));
    let snapshot = super::super::review::Snapshot {
        name: "test.txt".into(),
        path: "_AI_Output/run/test.txt".into(),
        before: Some("PRIVATE ORIGINAL TEXT".into()),
        after: "固定成果".into(),
        before_revision: Some("r1".into()),
        after_revision: "r2".into(),
        office: false,
        draft: false,
    };
    checkpoint(&path, &json!({"reviews":[snapshot]})).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("PRIVATE ORIGINAL TEXT"));
    let recovered = recover_review(&root, &id, "_AI_Output\\run\\test.txt").unwrap();
    assert_eq!(recovered.before.as_deref(), Some("PRIVATE ORIGINAL TEXT"));
    assert_eq!(recovered.after, "固定成果");
    assert!(recover_review(&root, &id, "_AI_Output/run/other.txt").is_err());
    assert!(recover_review(&root, "../other", "_AI_Output/run/test.txt").is_err());
    checkpoint(&path, &json!({"analysis":{}})).unwrap();
    assert!(recover_review(&root, &id, "_AI_Output/run/test.txt")
        .err()
        .unwrap()
        .contains("舊成果"));
}
#[test]
fn recovery_never_replays_unknown_writes() {
    assert!(replay_safe(&super::super::Tool::ReadFile {
        path: "a.txt".into(),
        offset: 0
    }));
    assert!(!replay_safe(&super::super::Tool::SaveCopy {
        copy_id: "copy".into(),
        revision: "v1".into()
    }));
    assert!(!replay_safe(&super::super::Tool::CreateWorkingCopy {
        source: Some("a.txt".into()),
        name: "copy.txt".into()
    }));
}
