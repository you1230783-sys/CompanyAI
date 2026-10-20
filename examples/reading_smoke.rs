//! 使用下載的公開C++專案，核對八段批次、實際截斷、筆記閘門與同版重讀投影。
//! 不執行第三方程式；固定筆記僅供工具契約驗收，不代表模型理解測試。
use company_ai::{
    projects::{files::Broker, sandbox::Worker, Project, Tool},
    AppResult,
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::PathBuf, sync::atomic::AtomicBool};
fn call(b: &mut Broker, w: &mut Worker, id: &str, v: Value) -> AppResult<Value> {
    let tool: Tool = serde_json::from_value(v).map_err(|e| e.to_string())?;
    b.execute(id, &tool, w, &AtomicBool::new(false))
}
fn run() -> AppResult<()> {
    let args: Vec<_> = std::env::args().collect();
    let exe = PathBuf::from(args.get(1).ok_or("需要EXE")?);
    let source = PathBuf::from(args.get(2).ok_or("需要公開C++專案")?);
    let root = PathBuf::from(args.get(3).ok_or("需要新測試目錄")?);
    std::fs::create_dir(&root).map_err(|e| e.to_string())?;
    for name in [
        "Bank.cpp",
        "Bank.h",
        "Account.cpp",
        "Account.h",
        "Transaction.h",
    ] {
        std::fs::copy(source.join(name), root.join(name)).map_err(|e| e.to_string())?;
    }
    let project = Project {
        id: "reading-smoke".into(),
        name: "公開C++閱讀契約".into(),
        root: root.clone(),
        imports: BTreeMap::new(),
    };
    let mut broker = Broker::new(project, "reading-smoke".into())?;
    broker.enable_memory("reading-chat")?;
    broker.enable_reading_notes();
    let mut worker = Worker::start(&exe, &AtomicBool::new(false))?;
    let tasks:Vec<_>=[("Bank.cpp",1,70),("Bank.cpp",71,140),("Bank.cpp",141,210),("Bank.cpp",211,278),("Bank.h",1,50),("Account.cpp",1,36),("Account.h",1,29),("Transaction.h",1,33)].iter().enumerate().map(|(i,(path,first,last))|json!({"task_id":format!("part-{i}"),"tool":"read_code_section","arguments_json":json!({"path":path,"first_line":first,"last_line":last}).to_string()})).collect();
    let request = json!({"tool":"run_batch","tasks":tasks});
    let first = call(&mut broker, &mut worker, "batch-one", request.clone())?;
    assert_eq!(first["ok"], true, "{first}");
    let outcomes = first["result"]["tasks"].as_array().ok_or("缺少批次結果")?;
    let mut total = 0;
    let mut notes = Vec::new();
    for item in outcomes {
        assert_eq!(item["outcome"]["ok"], true, "{item}");
        let text = item["outcome"]["result"]["text"]
            .as_str()
            .ok_or("缺少正文")?;
        total += text.chars().count();
        assert!(text.chars().count() <= 2000);
        notes.push(json!({"operation_id":item["outcome"]["operation_id"],"summary":"原生工具契約測試筆記；非模型理解結論","relationships":"依原始檔案區段核對；宣告不當成實作","open_questions":"語意品質另由真正Strata測試"}));
    }
    assert!(total <= 16000);
    let gate = call(
        &mut broker,
        &mut worker,
        "blocked-list",
        json!({"tool":"list_files","path":"","offset":0}),
    )?;
    assert_eq!(gate["ok"], false);
    let saved = call(
        &mut broker,
        &mut worker,
        "notes",
        json!({"tool":"record_reading_notes","notes":notes}),
    )?;
    assert_eq!(saved["ok"], true, "{saved}");
    let repeated = call(&mut broker, &mut worker, "batch-two", request)?;
    for item in repeated["result"]["tasks"]
        .as_array()
        .ok_or("缺少重讀結果")?
    {
        assert_eq!(item["outcome"]["ok"], true, "{item}");
        assert_eq!(
            item["outcome"]["result"]["reused_reading_note"], true,
            "{item}"
        );
        assert!(item["outcome"]["result"]["text"].is_null());
    }
    let precision_request =
        json!({"tool":"read_code_section","path":"Bank.cpp","first_line":1,"last_line":10});
    let precision = call(
        &mut broker,
        &mut worker,
        "precision",
        precision_request.clone(),
    )?;
    assert_eq!(precision["ok"], true, "{precision}");
    assert_eq!(precision["result"]["targeted_reread"], true, "{precision}");
    assert!(precision["result"]["text"]
        .as_str()
        .is_some_and(|text| text.contains("#include")));
    let saved_precision = call(
        &mut broker,
        &mut worker,
        "precision-note",
        json!({"tool":"record_reading_notes","notes":[{"operation_id":"precision","summary":"精讀已核對Bank.cpp第1–10行include依賴","relationships":"Bank.cpp使用Bank.h及標準函式庫","open_questions":""}]}),
    )?;
    assert_eq!(saved_precision["ok"], true, "{saved_precision}");
    let precision_repeat = call(
        &mut broker,
        &mut worker,
        "precision-repeat",
        precision_request,
    )?;
    assert_eq!(
        precision_repeat["result"]["reused_reading_note"], true,
        "{precision_repeat}"
    );
    assert!(precision_repeat["result"]["text"].is_null());
    std::fs::write(
        root.join("reading-verification.json"),
        serde_json::to_vec_pretty(
            &json!({"result":"PASS","batch":first,"repeated":repeated,"precision":precision,"precision_repeat":precision_repeat,"actual_text_chars":total}),
        )
        .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    println!("PASS eight read-only sections, total 16K limit, note gate, reused cropped ranges and one bounded precision read");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
