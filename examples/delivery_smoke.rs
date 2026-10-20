//! 八項交付功能的原生整合驗收。僅使用呼叫端指定的新目錄及自建CSV／Office文件。
//! 模型品質由另行Strata案例驗收；這裡核對實際Broker及來源、數字、成果的契約。
use company_ai::{
    projects::{artifacts, files::Broker, memory::Memory, python, sandbox::Worker, Project, Tool},
    AppResult,
};
use serde_json::{json, Value};
use sha2::Digest;
use std::{collections::BTreeMap, path::PathBuf, sync::atomic::AtomicBool};

fn call(broker: &mut Broker, worker: &mut Worker, id: &str, request: Value) -> AppResult<Value> {
    let tool: Tool = serde_json::from_value(request).map_err(|e| format!("{id}參數：{e}"))?;
    let result = broker.execute(id, &tool, worker, &AtomicBool::new(false))?;
    if result["ok"] != true {
        return Err(format!("{id}: {result}"));
    }
    Ok(result["result"].clone())
}

fn run() -> AppResult<()> {
    let args: Vec<_> = std::env::args().collect();
    let exe = PathBuf::from(args.get(1).ok_or("需要本次建置的EXE。")?);
    let root = PathBuf::from(args.get(2).ok_or("需要新的測試目錄。")?);
    std::fs::create_dir(&root).map_err(|e| e.to_string())?;
    std::fs::write(root.join("source.csv"), "Value\n1\n2\n4\n7\n").map_err(|e| e.to_string())?;
    std::fs::write(root.join("next.csv"), "Value\n2\n4\n6\n8\n").map_err(|e| e.to_string())?;
    let project = Project {
        id: "delivery-smoke".into(),
        name: "交付契約驗收".into(),
        root: root.clone(),
        imports: BTreeMap::new(),
    };
    let cancel = AtomicBool::new(false);
    python::prepare_runtime()?;
    let mut worker = Worker::start(&exe, &cancel)?;
    let mut broker = Broker::new(project.clone(), "delivery-smoke".into())?;
    broker.enable_memory("delivery-chat")?;
    call(
        &mut broker,
        &mut worker,
        "python-skill",
        json!({"tool":"load_skill","id":"python-analysis"}),
    )?;
    let calculation = json!({"tool":"run_python","purpose":"從固定CSV計算報告數字","inputs":[{"name":"source","path":"source.csv","kind":"csv"}],
        "code":"v = pd.to_numeric(tables['source']['Value'], errors='raise')\nresult = {'min': float(v.min()), 'mean': float(v.mean()), 'max': float(v.max()), 'count': int(v.count())}"});
    let computed = call(&mut broker, &mut worker, "calculate", calculation)?;
    assert_eq!(
        computed["summary"],
        json!({"min":1.0,"mean":3.5,"max":7.0,"count":4})
    );
    let specs:Vec<_>=["min","mean","max","count"].iter().map(|name|json!({"id":name,"operation_id":"calculate","pointer":format!("/summary/{name}"),"decimals":if *name=="count"{0}else{2},"unit":""})).collect();
    call(
        &mut broker,
        &mut worker,
        "register",
        json!({"tool":"register_report_values","values":specs}),
    )?;
    let mut copies = Vec::new();
    let mut word_target = None;
    // 同時涵蓋有預留文字與真正空白的Word儲存格；不能只用「待填」掩蓋空範圍問題。
    for (case, ext, placeholder) in [
        ("docx", "docx", "待填"),
        ("docx-empty", "docx", ""),
        ("xlsx", "xlsx", ""),
    ] {
        let created = call(
            &mut broker,
            &mut worker,
            &format!("create-{case}"),
            json!({"tool":"create_working_copy","source":null,"name":format!("一致性報告_{case}.{ext}")}),
        )?;
        let id = created["copy_id"].as_str().ok_or("沒有副本ID")?.to_owned();
        let action = if ext == "docx" {
            json!({"kind":"word_table","rows":[["統計","數值"],["min",placeholder],["mean",placeholder],["max",placeholder],["count",placeholder]]})
        } else {
            json!({"kind":"excel_write","sheet":1,"cell":"A1","rows":[["統計","數值"],["min",0],["mean",0],["max",0],["count",0]]})
        };
        let layout = call(
            &mut broker,
            &mut worker,
            &format!("layout-{case}"),
            json!({"tool":"office_batch","copy_id":id,"revision":created["revision"],"operations":[action]}),
        )?;
        let read = call(
            &mut broker,
            &mut worker,
            &format!("read-{case}"),
            json!({"tool":"read_file","path":id,"offset":0}),
        )?;
        let snapshot: Value = serde_json::from_str(read["text"].as_str().ok_or("缺少Office快照")?)
            .map_err(|e| e.to_string())?;
        let blocks = snapshot["blocks"].as_array().ok_or("缺少Office區塊")?;
        let targets: Vec<_> = if ext == "docx" {
            ["min", "mean", "max", "count"]
                .iter()
                .map(|name| {
                    let label = blocks
                        .iter()
                        .position(|block| block["text"] == *name)
                        .ok_or("缺少統計名稱儲存格")?;
                    let value = blocks.get(label + 1).ok_or("缺少相鄰數值儲存格")?;
                    assert_eq!(value["text"], placeholder);
                    Ok(value)
                })
                .collect::<AppResult<Vec<_>>>()?
        } else {
            blocks
                .iter()
                .filter(|block| block["kind"] == "number")
                .collect()
        };
        assert_eq!(targets.len(), 4, "{snapshot}");
        let assignments: Vec<_> = ["min", "mean", "max", "count"]
            .iter()
            .zip(&targets)
            .map(|(metric, block)| json!({"metric_id":metric,"block_id":block["id"]}))
            .collect();
        let applied = call(
            &mut broker,
            &mut worker,
            &format!("bind-{case}"),
            json!({"tool":"apply_report_values","copy_id":id,"revision":layout["revision"],"assignments":assignments}),
        )?;
        call(
            &mut broker,
            &mut worker,
            &format!("save-{case}"),
            json!({"tool":"save_copy","copy_id":id,"revision":applied["revision"]}),
        )?;
        if ext == "docx" {
            word_target = Some((
                id.clone(),
                targets[1]["id"].as_str().unwrap().to_owned(),
                applied["revision"].clone(),
            ));
        }
        copies.push(id);
    }
    let csv = call(
        &mut broker,
        &mut worker,
        "export-values",
        json!({"tool":"export_report_values","name":"統一計算值.csv","metric_ids":["min","mean","max"]}),
    )?;
    call(
        &mut broker,
        &mut worker,
        "chart",
        json!({"tool":"chart_dataset","path":csv["dataset"]["path"],"revision":csv["dataset"]["revision"],"x_column":"指標","y_columns":["數值"],"start_row":1,"row_count":3,"kind":"bar","title":"統一計算值","x_label":"統計","y_label":"數值"}),
    )?;
    assert_eq!(
        broker.charts()[0].series[0].values,
        vec![Some(1.0), Some(3.5), Some(7.0)]
    );
    let paths = broker.finish_cancellable(&copies, &cancel)?;
    println!("PASS calculation -> Word/Excel/CSV/chart and saved Office readback");

    // 範本必須以新來源重算，不沿用上一輪結果。跨Broker同一專案檢驗持久化。
    call(
        &mut broker,
        &mut worker,
        "notes-skill",
        json!({"tool":"load_skill","id":"notes"}),
    )?;
    let saved = call(
        &mut broker,
        &mut worker,
        "save-flow",
        json!({"tool":"save_workflow","name":"固定統計","steps":[{"step_id":"calc","operation_id":"calculate","bindings_json":"{\"/inputs/0/path\":\"param:source\"}"}]}),
    )?;
    let mut rerun = Broker::new(project.clone(), "delivery-rerun".into())?;
    rerun.enable_memory("another-chat")?;
    call(
        &mut rerun,
        &mut worker,
        "flow-skill",
        json!({"tool":"load_skill","id":"notes"}),
    )?;
    let view = call(
        &mut rerun,
        &mut worker,
        "view-flow",
        json!({"tool":"read_workflow","workflow_id":saved["workflow_id"],"offset":0}),
    )?;
    assert!(view.to_string().contains("param:source"));
    let plan = call(
        &mut rerun,
        &mut worker,
        "prepare",
        json!({"tool":"prepare_workflow","workflow_id":saved["workflow_id"],"parameters_json":"{\"source\":\"next.csv\"}"}),
    )?;
    assert_eq!(plan["executed"], false);
    call(
        &mut rerun,
        &mut worker,
        "calc-skill",
        json!({"tool":"load_skill","id":"python-analysis"}),
    )?;
    let step = call(
        &mut rerun,
        &mut worker,
        "step",
        json!({"tool":"execute_workflow_step","execution_id":plan["plan"]["execution_id"],"step_index":0}),
    )?;
    assert_eq!(step["outcome"]["result"]["summary"]["mean"], 5.0);
    println!("PASS persisted workflow rerun with new CSV, no reuse of old mean");

    let source_revision = format!(
        "{:x}",
        sha2::Sha256::digest(std::fs::read(root.join("source.csv")).map_err(|e| e.to_string())?)
    );
    let sources = BTreeMap::from([("source.csv".into(), source_revision)]);
    let entries = paths
        .iter()
        .map(|p| {
            artifacts::entry(
                &project,
                p,
                "delivery-smoke",
                "delivery-chat",
                sources.clone(),
                false,
            )
        })
        .collect::<AppResult<Vec<_>>>()?;
    let memory = Memory::open(project.clone(), "delivery-chat")?;
    memory.store_artifacts(&entries)?;
    assert_eq!(memory.artifact_center(0)?["total"], entries.len());
    let zip = memory.export_artifacts(&entries.iter().map(|e| e.id.clone()).collect::<Vec<_>>())?;
    // 交付後來源改動只提醒過期，不能繼續把舊值當成已驗證結果。
    std::fs::write(root.join("source.csv"), "Value\n99\n").map_err(|e| e.to_string())?;
    assert_eq!(
        artifacts::status(&project, &entries[0])["source_status"],
        "needs_update"
    );
    assert!(broker.finish_cancellable(&copies, &cancel).is_err());
    std::fs::write(root.join("source.csv"), "Value\n1\n2\n4\n7\n").map_err(|e| e.to_string())?;
    let (id, block, revision) = word_target.unwrap();
    call(
        &mut broker,
        &mut worker,
        "tamper",
        json!({"tool":"edit_office","copy_id":id,"revision":revision,"block_id":block,"expected":"3.50","replacement":"999"}),
    )?;
    assert!(broker.finish_cancellable(&copies, &cancel).is_err());
    println!("PASS source change and overwritten report value rejected");
    std::fs::write(root.join("delivery-verification.json"),serde_json::to_vec_pretty(&json!({"result":"PASS","zip":zip,"artifacts":paths,"expected":{"min":1,"mean":3.5,"max":7,"count":4,"rerun_mean":5}})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
