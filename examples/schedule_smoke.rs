//! 使用本次正式 EXE 的真 Broker／Worker 驗證固定流程與故障邊界；不使用公司資料。
use company_ai::{
    projects::{
        files,
        memory::Memory,
        schedule::{
            execution::{self, Boundary},
            Grant, Journal, Phase,
        },
        text,
        workflows::{Step, Template},
        Project,
    },
    reminders::WorkflowTarget,
    AppResult,
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

fn run() -> AppResult<()> {
    let args: Vec<_> = std::env::args().collect();
    let exe = PathBuf::from(args.get(1).ok_or("需要本次EXE。")?);
    let root = PathBuf::from(args.get(2).ok_or("需要新的測試目錄。")?);
    std::fs::create_dir(&root).map_err(|e| e.to_string())?;
    let source = "# 原始資料\n\n42\n";
    std::fs::write(root.join("source.md"), source).map_err(|e| e.to_string())?;
    let project = Project {
        id: "schedule-project".into(),
        name: "排程驗證".into(),
        root: root.clone(),
        imports: BTreeMap::new(),
    };
    let mut template = Template {
        id: String::new(),
        name: "固定副本".into(),
        created: 1,
        steps: vec![
            Step {
                id: "copy".into(),
                result_format: None,
                request: json!({"tool":"create_working_copy","source":"source.md","name":"scheduled.md"}),
                bindings: BTreeMap::new(),
            },
            Step {
                id: "save".into(),
                result_format: None,
                request: json!({"tool":"save_copy","copy_id":"placeholder","revision":"placeholder"}),
                bindings: BTreeMap::from([
                    ("/copy_id".into(), "step:copy:/result/copy_id".into()),
                    ("/revision".into(), "step:copy:/result/revision".into()),
                ]),
            },
        ],
    };
    template.validate()?;
    template.id = text::revision(&serde_json::to_string(&template).map_err(|e| e.to_string())?);
    Memory::open(project.clone(), "schedule-chat")?.store_workflow(&template)?;
    let grant = Grant {
        target: WorkflowTarget {
            conversation: "schedule-chat".into(),
            project: project.id.clone(),
            principal: "fixture-principal".into(),
            root_identity: files::directory_identity(&root)?,
            template: template.id.clone(),
            revision: text::revision(&serde_json::to_string(&template).map_err(|e| e.to_string())?),
        },
        template,
        parameters: BTreeMap::new(),
        due_at: company_ai::unix_now() + 60,
        repeat_days: 0,
        continuous_retry: true,
    };
    let cancel = AtomicBool::new(false);
    // 外層只開啟自己建立的 Excel 工作簿；確認在任何工具派送之前略過，
    // 並透過正式 Store.reconcile 保留重複排程，不只檢查錯誤文字。
    if args.get(3).is_some_and(|arg| arg == "--office-busy") {
        let mut busy_grant = grant.clone();
        busy_grant.template.steps[0].request["source"] = json!("busy.xlsx");
        busy_grant.template.steps[0].request["name"] = json!("scheduled.xlsx");
        busy_grant.template.id = "busy-template".into();
        busy_grant.target.template = busy_grant.template.id.clone();
        busy_grant.target.revision = text::revision(
            &serde_json::to_string(&busy_grant.template).map_err(|e| e.to_string())?,
        );
        busy_grant.repeat_days = 1;
        Memory::open(project.clone(), "schedule-chat")?.store_workflow(&busy_grant.template)?;
        let mut store = company_ai::projects::schedule::Store::default();
        let due = busy_grant.due_at;
        let id = store.add(busy_grant, due - 60)?;
        let (entry, execution_id) = store.claim(&id, due)?;
        execution::run(
            &root,
            &exe,
            project,
            &entry.grant,
            &execution_id,
            &cancel,
            false,
        )?;
        let journal = Journal::load(&root, &execution_id)?;
        assert!(journal.phase == Phase::Skipped, "{}", journal.detail);
        assert_eq!(journal.next, 0);
        assert!(journal.outputs.is_empty() && !root.join("_AI_Output").exists());
        store.reconcile(&execution_id, &journal, due + 1)?;
        assert!(store.entries[0].enabled && store.entries[0].execution.is_none());
        assert_eq!(store.entries[0].next_due, Some(due + 86400));
        println!("PASS occupied Office: no dispatch/output, audited skip, recurring schedule remains enabled.");
        return Ok(());
    }
    execution::run(
        &root,
        &exe,
        project.clone(),
        &grant,
        "normal",
        &cancel,
        false,
    )?;
    let journal = Journal::load(&root, "normal")?;
    assert!(journal.phase == Phase::Completed);
    assert_eq!(journal.next, 2);
    assert_eq!(journal.outputs.len(), 1);
    assert_eq!(
        std::fs::read_to_string(root.join(&journal.outputs[0])).map_err(|e| e.to_string())?,
        source
    );
    assert!(execution::run(
        &root,
        &exe,
        project.clone(),
        &grant,
        "normal",
        &cancel,
        false
    )
    .is_err());
    assert!(execution::run(
        &root,
        &exe,
        project.clone(),
        &grant,
        "normal",
        &cancel,
        true
    )
    .is_err());
    assert!(execution::check_target(&project, &grant, "other-principal").is_err());
    let mut moved = grant.clone();
    moved.target.root_identity[0] ^= 1;
    assert!(execution::check_target(&project, &moved, "fixture-principal").is_err());
    // 只在成功快照已落盤、下一步未提交時停止；續接必須沿用副本 ID。
    execution::run_observed(
        &root,
        &exe,
        project.clone(),
        &grant,
        "resume",
        &cancel,
        false,
        |point, next| {
            if point == Boundary::Ready && next == 1 {
                cancel.store(true, Ordering::Relaxed);
            }
            Ok(())
        },
    )?;
    let paused = Journal::load(&root, "resume")?;
    assert!(paused.phase == Phase::Ready);
    assert_eq!(paused.next, 1);
    cancel.store(false, Ordering::Relaxed);
    let mut visited = vec![];
    execution::run_observed(
        &root,
        &exe,
        project.clone(),
        &grant,
        "resume",
        &cancel,
        true,
        |point, next| {
            if point == Boundary::BeforeStep {
                visited.push(next);
            }
            Ok(())
        },
    )?;
    assert_eq!(visited, vec![1]);
    assert!(Journal::load(&root, "resume")?.phase == Phase::Completed);
    // 送出前及實際保存副本後但尚未保存成功快照，兩者都不能猜成可重試。
    for (id, boundary, step) in [
        ("before", Boundary::BeforeStep, 0),
        ("after", Boundary::AfterStep, 1),
    ] {
        assert!(execution::run_observed(
            &root,
            &exe,
            project.clone(),
            &grant,
            id,
            &cancel,
            false,
            |point, next| {
                if point == boundary && next == step {
                    Err("fixture durable boundary interruption".into())
                } else {
                    Ok(())
                }
            }
        )
        .is_err());
        let journal = Journal::load(&root, id)?;
        assert!(journal.phase == Phase::InFlight);
        assert!(execution::run(&root, &exe, project.clone(), &grant, id, &cancel, true).is_err());
        assert!(execution::run(&root, &exe, project.clone(), &grant, id, &cancel, false).is_err());
    }
    assert_eq!(
        std::fs::read_to_string(root.join("source.md")).map_err(|e| e.to_string())?,
        source
    );
    let report = json!({"result":"PASS","checks":["fixed template creates and saves actual copy","completed execution cannot replay","principal and root changes rejected","durably idle resume executes only next step","before-submit interruption cannot resume","successful write before checkpoint cannot replay","original source unchanged"],"company_environment_validated":false});
    std::fs::write(
        ".build/schedule-verification.json",
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    println!("{report}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
