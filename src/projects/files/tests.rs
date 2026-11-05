use super::*;

#[test]
fn memory_directory_pin_allows_atomic_files_and_keeps_directory_protected() {
    let root = std::env::current_dir()
        .unwrap()
        .join(".build")
        .join(format!("memory-pin-{}", crate::jobs::new_id().unwrap()));
    fs::create_dir_all(&root).unwrap();
    let guard = pin_memory_directory(&root).unwrap();
    // 模擬替換操作對父目錄的額外開啟，不分享 DELETE；不需要解除原保護。
    let reader = OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(&root)
        .unwrap();
    let output = root.join("record.dpapi");
    let temporary = root.join("record.tmp");
    fs::write(&temporary, b"first").unwrap();
    fs::rename(&temporary, &output).unwrap();
    fs::write(&temporary, b"second").unwrap();
    fs::rename(&temporary, &output).unwrap();
    assert_eq!(fs::read(&output).unwrap(), b"second");
    assert!(fs::rename(&root, root.with_extension("moved")).is_err());
    assert!(fs::remove_file(&guard.anchor_path).is_err());
    assert!(OpenOptions::new()
        .write(true)
        .open(&guard.anchor_path)
        .is_err());
    drop(reader);
    fs::remove_file(output).unwrap();
    assert!(
        fs::remove_dir(&root).is_err(),
        "獨占 anchor 必須保持目錄非空"
    );
    drop(guard);
    fs::remove_dir(root).unwrap();
}

#[test]
#[cfg(not(feature = "cad"))]
fn disabled_cad_rejects_cached_success_and_native_execution() {
    let mut broker = Broker::new(
        Project {
            id: "disabled-cad".into(),
            name: "停用CAD".into(),
            root: std::env::current_dir().unwrap(),
            imports: BTreeMap::new(),
        },
        "run".into(),
    )
    .unwrap();
    let tool = Tool::CadInventory {};
    let id = "a".repeat(64);
    broker.results.insert(
        id.clone(),
        (serde_json::to_value(&tool).unwrap(), json!({"ok":true})),
    );
    assert!(broker
        .cached_result(&id, &tool)
        .unwrap_err()
        .contains("未包含CAD"));
    // 舊紀錄仍保留供檢視，不能把停用模組當成此次成功執行。
    assert_eq!(
        broker.recorded_operation(&id).unwrap().unwrap().1["ok"],
        true
    );
    let cancel = AtomicBool::new(false);
    assert!(crate::cad::inventory(&cancel)
        .unwrap_err()
        .contains("未包含CAD"));
    assert!(crate::cad::execute(json!({}), &cancel)
        .unwrap_err()
        .contains("未包含CAD"));
    assert!(crate::cad::cleanup::obsolete_runtimes().is_err());
}
#[test]
fn work_log_outcome_keeps_failure_before_large_request_pages() {
    let failed = json!({"ok":false,"error":"收到101項operations，上限100"});
    assert_eq!(work_log_outcome(&failed)["error"], failed["error"]);
    let native = json!({"ok":true,"result":{"status":"FAILED","recovery":"manual_check",
            "drawing_started":true,"saved":false,"error":"TextString核對不符","entities":vec!["private details";1000]}});
    let summary = work_log_outcome(&native);
    assert_eq!(summary["status"], "FAILED");
    assert_eq!(summary["drawing_started"], true);
    assert_eq!(summary["native_error"], "TextString核對不符");
    assert!(summary.get("entities").is_none());
    let bounded = work_log_outcome(&json!({"ok":false,"error":"錯".repeat(5000)}));
    assert_eq!(bounded["error"].as_str().unwrap().chars().count(), 2000);
}
#[test]
fn cad_operation_limit_reports_count_without_creating_output() {
    let mut broker = Broker::new(
        Project {
            id: "p".into(),
            name: "p".into(),
            root: std::env::current_dir().unwrap(),
            imports: BTreeMap::new(),
        },
        "r".into(),
    )
    .unwrap();
    let error = broker
        .draw_cad(
            "limit.dwg",
            "Icad.Application.1",
            &vec![json!({}); 101],
            &AtomicBool::new(false),
        )
        .unwrap_err();
    assert!(error.contains("101項operations"));
    assert!(error.contains("尚未建立圖面"));
    assert_eq!(broker.cad_attempts, 0);
    assert!(broker.cad_last_result.is_none());
}
#[test]
fn dataset_checkpoint_reopens_and_rejects_modified_csv_without_touching_sources() {
    let root = std::env::temp_dir().join(format!(
        "dataset-checkpoint-{}",
        crate::jobs::new_id().unwrap()
    ));
    fs::create_dir(&root).unwrap();
    let project = Project {
        id: "dataset-test".into(),
        name: "CSV".into(),
        root: root.clone(),
        imports: Default::default(),
    };
    let cancel = AtomicBool::new(false);
    let mut broker = Broker::new(project.clone(), "task".into()).unwrap();
    let mut table = super::super::datasets::Table::new(vec!["x".into(), "y".into()]).unwrap();
    table
        .push(super::super::datasets::Row {
            path: "source.log".into(),
            revision: "r".into(),
            sheet: 0,
            row: 123,
            values: vec!["1".into(), "2".into()],
            kinds: vec![],
            texts: vec![],
        })
        .unwrap();
    let result = broker.save_dataset("資料.csv", &table, &cancel).unwrap();
    let mut restored = Broker::new(project.clone(), "task".into()).unwrap();
    restored.restore(broker.saved().unwrap(), &cancel).unwrap();
    assert_eq!(restored.dataset_index()[0]["rows"], 1);
    let path = result["dataset"]["path"].as_str().unwrap();
    let (_, revision) = super::super::datasets::inspect(&project, path, None, &cancel).unwrap();
    assert_eq!(revision, result["dataset"]["revision"].as_str().unwrap());
    assert_eq!(restored.finish(&[]).unwrap(), vec![path]);
    fs::write(root.join(path), "externally changed").unwrap();
    assert!(restored.finish(&[]).is_err());
    assert!(Broker::new(project, "task".into())
        .unwrap()
        .restore(broker.saved().unwrap(), &cancel)
        .is_err());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn archived_results_keep_operation_identity_and_survive_restore() {
    let root =
        std::env::temp_dir().join(format!("lmai-archive-{}", crate::jobs::new_id().unwrap()));
    fs::create_dir_all(&root).unwrap();
    let project = Project {
        id: "archive".into(),
        name: "archive".into(),
        root,
        imports: BTreeMap::new(),
    };
    let mut broker = Broker::new(project.clone(), "run".into()).unwrap();
    broker.enable_memory("chat").unwrap();
    let tool = Tool::ReadFile {
        path: "source.txt".into(),
        offset: 0,
    };
    let result = json!({"ok":true,"result":{"text":"已確認證據","revision":"v1"}});
    broker.remember_result("operation", &tool, &result).unwrap();
    broker.archive_results().unwrap();
    assert!(broker.results.is_empty());
    assert_eq!(
        broker.cached_result("operation", &tool).unwrap(),
        Some(result.clone())
    );
    let mut restored = Broker::new(project, "run".into()).unwrap();
    restored.enable_memory("chat").unwrap();
    restored
        .restore(broker.saved().unwrap(), &AtomicBool::new(false))
        .unwrap();
    assert_eq!(
        restored.cached_result("operation", &tool).unwrap(),
        Some(result)
    );
    assert!(restored
        .cached_result(
            "operation",
            &Tool::ReadFile {
                path: "other.txt".into(),
                offset: 0
            }
        )
        .is_err());
}
#[test]
fn new_drawing_context_keeps_failure_without_requiring_active_snapshot() {
    let mut broker = Broker::new(
        Project {
            id: "p".into(),
            name: "p".into(),
            root: std::env::current_dir().unwrap(),
            imports: BTreeMap::new(),
        },
        "r".into(),
    )
    .unwrap();
    super::super::skills::activate(&mut broker.loaded_skills, "cad-new").unwrap();
    assert_eq!(broker.cad_snapshot_index()["mode"], "new_drawing");
    broker.cad_failed = true;
    broker.cad_last_result = Some(json!({"status":"FAILED","recovery":"manual_check",
            "error":"TextString","saved":false,"drawing_started":true}));
    let mut request: Value = serde_json::from_str(include_str!("../tools.json")).unwrap();
    broker.restrict_tools(&mut request);
    assert!(!request["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["function"]["name"]
            .as_str()
            .is_some_and(|n| n.starts_with("cad_") && n != "cad_inventory")));
    assert_eq!(
        broker.cad_artifact_index()["last_execution"]["error"],
        "TextString"
    );
    assert_eq!(
        broker.saved().unwrap().cad_last_result.unwrap()["recovery"],
        "manual_check"
    );
    let finish = request["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["function"]["name"] == "finish")
        .unwrap();
    assert!(
        finish["function"]["parameters"]["properties"]["message"]["description"]
            .as_str()
            .unwrap()
            .contains("不能說未建圖")
    );
}

#[test]
fn new_design_hides_drawing_schema_until_saved_and_keeps_state_after_restore() {
    let project = Project {
        id: "planning".into(),
        name: "planning".into(),
        root: std::env::current_dir().unwrap(),
        imports: BTreeMap::new(),
    };
    let mut broker = Broker::new(project.clone(), "r".into()).unwrap();
    let available = |broker: &Broker, name: &str| {
        let mut request: Value = serde_json::from_str(include_str!("../tools.json")).unwrap();
        broker.restrict_tools(&mut request);
        request["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["function"]["name"] == name)
    };
    broker.activate_skill("cad-new").unwrap();
    assert!(!available(&broker, "cad_plan_start"));
    assert!(available(&broker, "compact_context"));
    assert!(available(&broker, "ask_user"));
    assert!(broker
        .skill_context("fast")
        .unwrap()
        .contains("暫不提供cad_draw"));
    // 即使模型猜出尚未公告的工具，也不接觸COM或建立成果目錄。
    let error = broker
        .draw_cad(
            "new.dwg",
            "latest",
            &[json!({"kind":"line"})],
            &AtomicBool::new(false),
        )
        .unwrap_err();
    assert!(error.contains("尚未保存新圖配置方案"));
    assert_eq!(broker.cad_attempts, 0);
    assert!(!broker.cad_failed);
    broker
        .cad_design
        .record("毫米，兩層並排；現場尺寸待確認。", "完整繪圖")
        .unwrap();
    assert!(available(&broker, "cad_plan_start"));
    let saved = broker.saved().unwrap();
    let mut restored = Broker::new(project, "r".into()).unwrap();
    restored.restore(saved, &AtomicBool::new(false)).unwrap();
    assert!(available(&restored, "cad_plan_start"));
    assert_eq!(
        restored.cad_artifact_index()["design_planning"]["design"]["working_note"],
        "毫米，兩層並排；現場尺寸待確認。"
    );
    restored.activate_skill("cad-new").unwrap();
    assert!(
        available(&restored, "cad_plan_start"),
        "重複載入不能丟失方案"
    );
    restored.activate_skill("cad-active").unwrap();
    restored.activate_skill("cad-new").unwrap();
    assert!(
        !available(&restored, "cad_plan_start"),
        "重新切入新圖需新的方案"
    );
}

#[test]
fn old_broker_checkpoint_does_not_gain_a_new_drawing_gate() {
    let project = Project {
        id: "legacy-planning".into(),
        name: "legacy".into(),
        root: std::env::current_dir().unwrap(),
        imports: BTreeMap::new(),
    };
    let mut broker = Broker::new(project.clone(), "r".into()).unwrap();
    broker.activate_skill("cad-new").unwrap();
    let mut value = serde_json::to_value(broker.saved().unwrap()).unwrap();
    value.as_object_mut().unwrap().remove("cad_design");
    let saved = serde_json::from_value(value).unwrap();
    let mut restored = Broker::new(project, "r".into()).unwrap();
    restored.restore(saved, &AtomicBool::new(false)).unwrap();
    assert!(!restored.cad_design.pending());
    assert!(restored.cad_artifact_index()["design_planning"].is_null());
}

#[test]
fn staged_cad_geometry_survives_broker_restore_without_becoming_a_drawing() {
    let project = Project {
        id: "staged".into(),
        name: "staged".into(),
        root: std::env::current_dir().unwrap(),
        imports: BTreeMap::new(),
    };
    let mut broker = Broker::new(project.clone(), "r".into()).unwrap();
    broker.activate_skill("cad-new").unwrap();
    broker
        .cad_design
        .record("兩層同圖，單位毫米。", "先一樓牆")
        .unwrap();
    let installed =
        json!({"versions":[{"progid":"Icad.Application.1","year":2008,"server_exists":true}]});
    broker
        .start_cad_plan_with_inventory("new.dwg", "latest", &installed)
        .unwrap();
    let op = json!({"kind":"line","color":7,"start":[0,0],"end":[100,0]});
    broker
        .put_cad_plan_part(
            "plan-1",
            0,
            "floor1",
            "一樓底邊x0–100",
            std::slice::from_ref(&op),
        )
        .unwrap();
    let saved = broker.saved().unwrap();
    let mut restored = Broker::new(project, "r".into()).unwrap();
    restored.restore(saved, &AtomicBool::new(false)).unwrap();
    let mut wire_op = op.clone();
    wire_op["line_style"] = Value::Null;
    wire_op["layer"] = Value::Null;
    assert_eq!(
        restored.inspect_cad_plan("plan-1", Some("floor1")).unwrap()["part"]["operations"],
        json!([wire_op])
    );
    restored
        .cad_design
        .record("繼續安排下一段，前段參數由本機保留。", "新增二樓")
        .unwrap();
    assert_eq!(
        restored.cad_artifact_index()["design_planning"]["plan"]["recent_details"][0]["id"],
        "floor1"
    );
    assert!(restored
        .start_cad_plan_with_inventory("other.dwg", "GStarCAD.Application.22", &installed)
        .is_err());
    assert!(restored
        .draw_cad(
            "new.dwg",
            "Icad.Application.1",
            &[],
            &AtomicBool::new(false)
        )
        .is_err());
    assert!(restored
        .commit_cad_plan("plan-1", 1, &AtomicBool::new(true))
        .is_err());
    assert_eq!(
        restored.cad_design.plan.as_ref().unwrap().context()["phase"],
        "collecting"
    );
    assert_eq!(restored.cad_attempts, 0);
    assert!(!restored.cad_failed);
    let mut request: Value = serde_json::from_str(include_str!("../tools.json")).unwrap();
    restored.restrict_tools(&mut request);
    let names: Vec<_> = request["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["function"]["name"].as_str())
        .collect();
    assert!(names.contains(&"cad_plan_part"));
    assert!(names.contains(&"cad_plan_commit"));
    assert!(
        !names.contains(&"cad_plan_inspect"),
        "已常駐精確資料，不公告多餘查詢"
    );
    assert!(!names.contains(&"cad_draw"));
    assert!(!names.contains(&"cad_plan_start"));
}

#[test]
fn same_task_handoff_before_loading_cad_exposes_drawing_without_another_handoff() {
    let mut broker = Broker::new(
        Project {
            id: "before-load".into(),
            name: "before-load".into(),
            root: std::env::current_dir().unwrap(),
            imports: BTreeMap::new(),
        },
        "r".into(),
    )
    .unwrap();
    broker
        .cad_design
        .remember_before_loading("基地8×12m，入口前側，兩層同圖。", "載入CAD並繪圖")
        .unwrap();
    broker.activate_skill("cad-new").unwrap();
    let mut request: Value = serde_json::from_str(include_str!("../tools.json")).unwrap();
    broker.restrict_tools(&mut request);
    assert!(request["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["function"]["name"] == "cad_plan_start"));
    assert_eq!(
        broker.cad_artifact_index()["design_planning"]["design"]["working_note"],
        "基地8×12m，入口前側，兩層同圖。"
    );
}

#[test]
fn cad_tools_follow_snapshot_and_active_document_lifetime() {
    let mut broker = Broker::new(
        Project {
            id: "p".into(),
            name: "p".into(),
            root: std::env::current_dir().unwrap(),
            imports: BTreeMap::new(),
        },
        "r".into(),
    )
    .unwrap();
    super::super::skills::activate(&mut broker.loaded_skills, "cad-active").unwrap();
    let definitions: Value = serde_json::from_str(include_str!("../tools.json")).unwrap();
    let available = |broker: &Broker, name: &str| {
        let mut request = definitions.clone();
        broker.restrict_tools(&mut request);
        request["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["function"]["name"] == name)
    };
    assert!(available(&broker, "cad_snapshot"));
    for name in ["cad_query", "cad_preview", "cad_active_edit"] {
        assert!(!available(&broker, name));
    }
    broker.cad_snapshot = Some(
        super::super::cad_index::Snapshot::new(
            json!({"entities":[],"entity_count":0,"revision":"r"}),
        )
        .unwrap(),
    );
    broker.cad_active = Some(json!({"revision":"r"}));
    for name in ["cad_query", "cad_preview", "cad_active_edit"] {
        assert!(available(&broker, name));
    }
    // 寫入撤銷快照後仍可 finish 或取得新快照，不提供失效的查詢入口。
    broker.cad_snapshot = None;
    broker.cad_active = None;
    assert!(available(&broker, "finish") && available(&broker, "cad_snapshot"));
    assert!(!available(&broker, "cad_query") && !available(&broker, "cad_active_edit"));
    broker.cad_session.edited(
        &json!({"status":"EDITED_UNSAVED","verification":{"handles_verified":645}}),
        &[json!({"handle":"A","kind":"radius","value":40})],
    );
    let mut completed = definitions.clone();
    broker.restrict_tools(&mut completed);
    let finish = completed["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["function"]["name"] == "finish")
        .unwrap();
    assert!(
        finish["function"]["parameters"]["properties"]["message"]["description"]
            .as_str()
            .unwrap()
            .contains("2–3句")
    );
}

#[test]
fn tools_require_existing_edit_and_export_targets() {
    let mut broker = Broker::new(
        Project {
            id: "p".into(),
            name: "p".into(),
            root: std::env::current_dir().unwrap(),
            imports: BTreeMap::new(),
        },
        "r".into(),
    )
    .unwrap();
    let definitions: Value = serde_json::from_str(include_str!("../tools.json")).unwrap();
    let mut request = definitions.clone();
    broker.restrict_tools(&mut request);
    let has = |r: &Value, name: &str| {
        r["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["function"]["name"] == name)
    };
    for name in [
        "edit_text",
        "office_action",
        "office_batch",
        "save_copy",
        "delete_copy",
        "export_chart_png",
    ] {
        assert!(!has(&request, name));
    }
    assert!(!has(&request, "create_working_copy") && has(&request, "finish"));
    super::super::skills::activate(&mut broker.loaded_skills, "text-edit").unwrap();
    broker.copies.insert(
        "c".into(),
        Copy {
            baseline: None,
            reviews: BTreeMap::new(),
            office: None,
            name: "a.txt".into(),
            text: String::new(),
            encoding: Encoding::Utf8(false),
            saved_revision: None,
            python_checked_revision: None,
            code_review: Default::default(),
            draft: None,
            code_sections: vec![],
            paths: vec![],
        },
    );
    let mut request = definitions;
    broker.restrict_tools(&mut request);
    assert!(has(&request, "edit_text") && has(&request, "save_copy"));
    assert!(!has(&request, "office_action") && !has(&request, "export_chart_png"));
}
#[test]
fn excel_binding_survives_broker_checkpoint_and_blocks_unplanned_export() {
    let project = Project {
        id: "p".into(),
        name: "p".into(),
        root: std::env::current_dir().unwrap(),
        imports: BTreeMap::new(),
    };
    let mut broker = Broker::new(project.clone(), "r".into()).unwrap();
    let (proposal, page) = super::super::excel_plan::tests::fixture();
    let plan = super::super::excel_plan::Plan::create(proposal, &page).unwrap();
    broker.excel_plans.insert(plan.id.clone(), plan);
    let mut restored = Broker::new(project, "r".into()).unwrap();
    restored
        .restore(broker.saved().unwrap(), &AtomicBool::new(false))
        .unwrap();
    assert_eq!(broker.excel_plan_index(), restored.excel_plan_index());
    assert!(restored.require_planned_export("a.xlsx", 1).is_err());
    assert!(restored.require_planned_export("other.xlsx", 1).is_ok());
    assert!(restored
        .excel_plans
        .values()
        .next()
        .unwrap()
        .check_axes("B", &["B".into()])
        .is_err());
}
#[test]
fn png_exports_preserve_names_reuse_verified_files_and_survive_restore() {
    let root = std::env::current_dir()
        .unwrap()
        .join(".build")
        .join(format!("png-test-{}", crate::jobs::new_id().unwrap()));
    fs::create_dir_all(&root).unwrap();
    let project = Project {
        id: "png".into(),
        name: "png".into(),
        root: root.clone(),
        imports: BTreeMap::new(),
    };
    let mut broker = Broker::new(project.clone(), "run".into()).unwrap();
    let chart = super::super::charts::Chart {
        quality: None,
        reference_lines: Vec::new(),
        style: None,
        transform: None,
        kind: "line".into(),
        title: "圖".into(),
        x_label: "x".into(),
        y_label: "y".into(),
        x: vec![json!(1)],
        series: vec![super::super::charts::Series {
            skip_indices: vec![],
            name: "A".into(),
            values: vec![Some(2.0)],
        }],
        source: "測試".into(),
        data_note: String::new(),
        data_issues: vec![],
    };
    broker.add_chart(chart.clone()).unwrap();
    broker.add_chart(chart).unwrap();
    broker.set_png_renderer(Box::new(|_, _| Ok(super::super::charts::png::fixture())));
    let cancel = AtomicBool::new(false);
    assert!(broker.export_chart_png(0, "../bad.png", &cancel).is_err());
    assert!(broker.export_chart_png(99, "bad.png", &cancel).is_err());
    let first = broker.export_chart_png(0, "趨勢.png", &cancel).unwrap();
    let again = broker.export_chart_png(0, "趨勢.png", &cancel).unwrap();
    assert_eq!(first["path"], again["path"]);
    assert_eq!(again["reused"], true);
    let second = broker.export_chart_png(1, "趨勢.png", &cancel).unwrap();
    assert!(second["path"].as_str().unwrap().ends_with("趨勢_2.png"));
    let mut transform = super::super::charts::transform::Transform::default();
    transform.x.mode = super::super::charts::transform::Mode::Index;
    transform.x.start = 10.0;
    broker.charts[0].transform = Some(Box::new(transform));
    let changed = broker.export_chart_png(0, "趨勢.png", &cancel).unwrap();
    assert_ne!(changed["path"], first["path"], "轉換後不得重用舊 PNG");
    assert_eq!(
        broker.export_chart_png(0, "趨勢.png", &cancel).unwrap()["reused"],
        true
    );
    let mut restored = Broker::new(project, "run".into()).unwrap();
    restored.restore(broker.saved().unwrap(), &cancel).unwrap();
    let paths = restored.finish(&[]).unwrap();
    assert_eq!(paths.len(), 3);
    fs::write(root.join(&paths[0]), b"changed").unwrap();
    assert!(restored.finish(&[]).is_err());
    for path in &paths {
        fs::remove_file(root.join(path)).unwrap();
    }
    fs::remove_dir(root.join(paths[0].rsplit_once('/').unwrap().0)).unwrap();
    fs::remove_dir(root.join("_AI_Output")).unwrap();
    fs::remove_dir(root).unwrap();
}
#[test]
fn reader_copy_allows_reader_write_access_without_unlocking_original() {
    let root = std::env::current_dir()
        .unwrap()
        .join(".build")
        .join(format!("reader-copy-{}", crate::jobs::new_id().unwrap()));
    fs::create_dir_all(&root).unwrap();
    let original = root.join("source.msg");
    fs::write(&original, b"original bytes").unwrap();
    let mut source = checked_file(&original).unwrap();
    let project = Project {
        id: "copy-test".into(),
        name: "copy-test".into(),
        root: root.clone(),
        imports: BTreeMap::new(),
    };
    let (result, warning) = with_reader_copy(&project, &mut source, "msg", |copy| {
        assert!(OpenOptions::new().write(true).open(&original).is_err());
        assert!(fs::rename(copy, copy.with_extension("moved")).is_err());
        let mut writer = OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(FILE_SHARE_READ)
            .open(copy)
            .map_err(|e| e.to_string())?;
        writer.write_all(b"updated").map_err(|e| e.to_string())?;
        Ok(copy.to_path_buf())
    })
    .unwrap();
    assert!(warning.is_none());
    assert!(!result.exists());
    assert_eq!(fs::read(&original).unwrap(), b"original bytes");
    let error = with_reader_copy(&project, &mut source, "msg", |_| {
        Err::<(), _>("reader failed".into())
    })
    .unwrap_err();
    assert_eq!(error, "reader failed");
    // 模擬 Outlook 在讀取成功後暫留 handle：回傳正文與提醒，不誤報整次失敗。
    let ((held, copy), warning) = with_reader_copy(&project, &mut source, "msg", |copy| {
        let held = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .open(copy)
            .map_err(|e| e.to_string())?;
        Ok((held, copy.to_path_buf()))
    })
    .unwrap();
    assert!(warning.unwrap().contains("閱讀暫存尚未能清除"));
    assert!(copy.exists());
    drop(held);
    fs::remove_file(&copy).unwrap();
    fs::remove_dir(copy.parent().unwrap()).unwrap();
    assert_eq!(fs::read_dir(root.join("_AI_Output")).unwrap().count(), 0);
    drop(source);
    fs::remove_file(original).unwrap();
    fs::remove_dir(root.join("_AI_Output")).unwrap();
    fs::remove_dir(root).unwrap();
}
#[test]
fn user_chart_save_reuses_conversation_folder_without_overwrite() {
    let root = std::env::current_dir()
        .unwrap()
        .join(".build")
        .join(format!("user-chart-{}", crate::jobs::new_id().unwrap()));
    fs::create_dir_all(&root).unwrap();
    let project = Project {
        id: "test".into(),
        name: "test".into(),
        root: root.clone(),
        imports: BTreeMap::new(),
    };
    let bytes = super::super::charts::png::fixture();
    let first = save_user_chart(&project, "chart-conversation", &bytes).unwrap();
    let second = save_user_chart(&project, "chart-conversation", &bytes).unwrap();
    assert_ne!(first, second);
    assert!(first.starts_with("_AI_Output/"));
    assert_eq!(fs::read(root.join(&first)).unwrap(), bytes);
    assert!(save_user_chart(&project, "chart-conversation", b"invalid").is_err());
    let folder = root.join(&first).parent().unwrap().to_owned();
    for path in [first, second] {
        let file = root.join(path);
        fs::remove_file(&file).unwrap();
    }
    fs::remove_dir(folder).unwrap();
    fs::remove_dir(root.join("_AI_Output")).unwrap();
    fs::remove_dir(root).unwrap();
}

#[test]
fn names_collide_without_overwrite_and_directory_pins_still_block_changes() {
    let root = std::env::current_dir()
        .unwrap()
        .join(".build")
        .join(format!("name-test-{}", crate::jobs::new_id().unwrap()));
    fs::create_dir_all(&root).unwrap();
    let first = create_output_folder(&root).unwrap();
    let second = create_output_folder(&root).unwrap();
    assert_ne!(first, second);
    assert_eq!(&first[8..9], "_");
    assert!(first[..8].bytes().all(|c| c.is_ascii_digit()));
    let folder = root.join(&first);
    let pins = pin(&folder).unwrap();
    assert!(OpenOptions::new()
        .access_mode(windows_sys::Win32::Foundation::GENERIC_WRITE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(&folder)
        .is_err());
    assert!(fs::rename(&folder, root.join("moved")).is_err());
    let (name, mut output) = reserve_output(&folder, "報告.txt").unwrap();
    output.write_all(b"original").unwrap();
    drop(output);
    let (next, output) = reserve_output(&folder, "報告.txt").unwrap();
    drop(output);
    assert_eq!(name, "報告.txt");
    assert_eq!(next, "報告_2.txt");
    assert_eq!(fs::read(folder.join(&name)).unwrap(), b"original");
    // 換鎖時 anchor 單獨持有，Windows 也必須拒絕更名父目錄。
    let transition = folder.join("transition");
    fs::create_dir(&transition).unwrap();
    let anchor = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .share_mode(0)
        .open(transition.join(".anchor"))
        .unwrap();
    assert!(fs::rename(&transition, folder.join("transition-moved")).is_err());
    drop(anchor);
    fs::remove_file(transition.join(".anchor")).unwrap();
    fs::remove_dir(transition).unwrap();
    let stage = folder.join("stage");
    fs::create_dir(&stage).unwrap();
    let stage_pin = pin_stage(&stage).unwrap();
    // 直接嘗試設 Junction，確保「非空目錄」防線真正在 OS 層生效。
    let junction = OpenOptions::new()
        .access_mode(windows_sys::Win32::Foundation::GENERIC_WRITE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(&stage)
        .unwrap();
    let substitute: Vec<u16> = format!("\\??\\{}", root.display()).encode_utf16().collect();
    let display: Vec<u16> = root.to_string_lossy().encode_utf16().collect();
    let mut reparse = Vec::new();
    reparse.extend_from_slice(&0xa0000003u32.to_le_bytes());
    reparse.extend_from_slice(
        &((8 + (substitute.len() + display.len() + 2) * 2) as u16).to_le_bytes(),
    );
    for value in [
        0,
        0,
        (substitute.len() * 2) as u16,
        ((substitute.len() + 1) * 2) as u16,
        (display.len() * 2) as u16,
    ] {
        reparse.extend_from_slice(&value.to_le_bytes());
    }
    for value in substitute.into_iter().chain([0]).chain(display).chain([0]) {
        reparse.extend_from_slice(&value.to_le_bytes());
    }
    let mut returned = 0;
    let result = unsafe {
        windows_sys::Win32::System::IO::DeviceIoControl(
            junction.as_raw_handle(),
            0x000900a4,
            reparse.as_ptr().cast(),
            reparse.len() as u32,
            std::ptr::null_mut(),
            0,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    assert_eq!(result, 0);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(145),
        "必須因目錄非空拒絕，而不是測試資料格式錯誤"
    );
    drop(junction);
    assert!(fs::remove_file(stage.join(".anchor")).is_err());
    assert!(fs::rename(&stage, folder.join("moved-stage")).is_err());
    drop(stage_pin);
    fs::remove_dir(stage).unwrap();
    fs::remove_file(folder.join(name)).unwrap();
    fs::remove_file(folder.join(next)).unwrap();
    drop(pins);
    fs::remove_dir(folder).unwrap();
    fs::remove_dir(root.join(second)).unwrap();
    fs::remove_dir(root).unwrap();
}
#[test]
fn reject_windows_escape_and_device_paths() {
    for value in [
        "../secret.txt",
        "C:\\secret.txt",
        "\\\\host\\share",
        "file.txt:secret",
        "NUL.txt",
        "a/../b",
        "a.",
    ] {
        assert!(relative(value).is_err(), "{value}");
    }
    assert_eq!(
        relative("資料/文件.txt").unwrap().to_str().unwrap(),
        r"資料\文件.txt"
    );
}
#[test]
fn historical_diagram_export_keeps_its_original_source_version() {
    let root = std::env::temp_dir().join(format!(
        "diagram-lineage-{}",
        crate::jobs::new_id().unwrap()
    ));
    fs::create_dir(&root).unwrap();
    let project = Project {
        id: "lineage".into(),
        name: "來源版本".into(),
        root: root.clone(),
        imports: Default::default(),
    };
    let mut broker = Broker::new(project, "lineage".into()).unwrap();
    let source = "flowchart TD\nA-->B";
    let revision = text::revision(source);
    let diagram = super::super::diagrams::Diagram {
        name: "test.mmd".into(),
        source: source.into(),
        revision: revision.clone(),
        path: "_AI_Output/old/test.mmd".into(),
        based_on: None,
        node_sources: vec![super::super::diagram_sources::Origin {
            node_id: "A".into(),
            operation_id: "read-old".into(),
            path: "main.cpp".into(),
            revision: "a".repeat(64),
            source_sha256: "b".repeat(64),
            locator: "第1行".into(),
            excerpt: "entry".into(),
            first_line: Some(1),
            last_line: Some(1),
            detail_diagram_index: None,
        }],
    };
    broker.previous_diagrams.insert(2, vec![diagram]);
    broker
        .source_versions
        .insert("main.cpp".into(), "c".repeat(64));
    broker.diagram_exports.push(super::super::diagrams::Export {
        path: "_AI_Output/new/test.svg".into(),
        sha256: "d".repeat(64),
        format: super::super::diagrams::Format::Svg,
        source: Some(super::super::diagrams::ExportSource {
            revision,
            message_index: Some(2),
            diagram_index: 0,
        }),
    });
    assert_eq!(
        broker.artifact_sources("_AI_Output/new/test.svg")["main.cpp"],
        "b".repeat(64)
    );
    assert_eq!(
        broker.artifact_sources("_AI_Output/new/unrelated.csv")["main.cpp"],
        "c".repeat(64)
    );
    drop(broker);
    fs::remove_dir(root).unwrap();
}
