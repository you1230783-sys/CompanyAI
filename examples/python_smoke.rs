//! 真正經過 Broker、AppContainer 與（選用）Excel COM 的 Python 驗收。
//! 只建立指定的新測試目錄，不接觸使用者文件或登入資料。
use company_ai::{
    projects::{files::Broker, python, sandbox::Worker, Project, Tool},
    AppResult,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

fn call(broker: &mut Broker, worker: &mut Worker, id: &str, value: Value) -> AppResult<Value> {
    let tool: Tool = serde_json::from_value(value).map_err(|e| e.to_string())?;
    let response = broker.execute(id, &tool, worker, &AtomicBool::new(false))?;
    if response["ok"] != true {
        return Err(format!("{id}: {response}"));
    }
    Ok(response["result"].clone())
}

fn run() -> AppResult<()> {
    let args: Vec<_> = std::env::args().collect();
    let exe = PathBuf::from(args.get(1).ok_or("需要主程式位置。")?);
    let root = PathBuf::from(args.get(2).ok_or("需要新的測試目錄。")?);
    let office = args.iter().any(|s| s == "--office");
    std::fs::create_dir(&root).map_err(|e| e.to_string())?;
    let started = Instant::now();
    python::prepare_runtime()?;
    let csv = "批號,機台,值,備註\n001,A,2,NA\n002,A,4,=1+1\n003,B,9,正常\n";
    std::fs::write(root.join("input.csv"), csv).map_err(|e| e.to_string())?;
    std::fs::write(root.join("events.log"), "08:00:00 start\n08:00:05 done\n")
        .map_err(|e| e.to_string())?;
    let project = Project {
        id: "python-smoke".into(),
        name: "Python 驗收".into(),
        root: root.clone(),
        imports: BTreeMap::new(),
    };
    let mut broker = Broker::new(project, "python-smoke".into())?;
    let cancel = AtomicBool::new(false);
    let mut worker = Worker::start(&exe, &cancel)?;
    call(
        &mut broker,
        &mut worker,
        "skill",
        json!({"tool":"load_skill","id":"python-analysis"}),
    )?;
    let request = json!({"tool":"run_python","purpose":"合成 CSV 分組、LOG 配對與 XLSX 生成驗收", "inputs":[
        {"name":"data","path":"input.csv","kind":"csv"}, {"name":"log","path":"events.log","kind":"text"}], "code":r#"
from datetime import datetime
df = tables['data'].copy()
assert df['批號'].tolist() == ['001', '002', '003']
assert df['備註'][0] == 'NA'
df['值'] = pd.to_numeric(df['值'], errors='raise')
summary = df.groupby('機台')['值'].agg(['count','mean','sum']).reset_index()
lines = texts['log'].splitlines()
seconds = (datetime.strptime(lines[1][:8], '%H:%M:%S') - datetime.strptime(lines[0][:8], '%H:%M:%S')).total_seconds()
emit_table('統計.csv', summary)
emit_excel('分析.xlsx', {'原始': df, '統計': summary})
result = {'total': int(df['值'].sum()), 'seconds': seconds, 'rows': len(df)}
"#});
    let first = call(&mut broker, &mut worker, "analyze", request.clone())?;
    assert_eq!(first["summary"], json!({"total":15,"seconds":5.0,"rows":3}));
    assert_eq!(first["versions"]["pandas"], "2.2.3");
    // 真正經過原生快照與 Python 管道，驗證 Big5 不再被 UTF-8 前置檢查擋下。
    let chinese = "08:00:00 拋料當顆辨識序號: 123\r\n";
    let big5 = company_ai::projects::text::encode(
        chinese,
        company_ai::projects::text::Encoding::CodePage(950),
    )?;
    std::fs::write(root.join("big5.log"), &big5).map_err(|e| e.to_string())?;
    std::fs::write(
        root.join("utf8.log"),
        "\u{feff}08:00:00 拋料當顆辨識序號: 123\r\n",
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(root.join("fallback.log"), "中").map_err(|e| e.to_string())?;
    let encoded = call(
        &mut broker,
        &mut worker,
        "log-encodings",
        json!({"tool":"run_python", "purpose":"Big5及UTF-8 LOG快照驗收", "inputs":[
        {"name":"legacy","path":"big5.log","kind":"text"},
        {"name":"unicode","path":"utf8.log","kind":"text"},
        {"name":"fallback","path":"fallback.log","kind":"text"}], "code":r#"
assert texts['legacy'] == texts['unicode']
assert '拋料當顆辨識序號: 123' in texts['legacy']
assert texts['fallback'] == '中'
assert metadata['legacy']['encoding'] == 'big5'
assert metadata['unicode']['encoding'] == metadata['fallback']['encoding'] == 'utf8'
assert not metadata['legacy']['encoding_ambiguous']
result = {'encoding_bridge': True}
"#}),
    )?;
    assert_eq!(encoded["summary"]["encoding_bridge"], true);
    assert_eq!(
        std::fs::read(root.join("big5.log")).map_err(|e| e.to_string())?,
        big5
    );
    let repeated = call(&mut broker, &mut worker, "analyze", request)?;
    assert_eq!(first, repeated, "相同 operation id 不得重複發布");
    let table_path = first["artifacts"][0]["dataset"]["path"]
        .as_str()
        .ok_or_else(|| first.to_string())?;
    let book_path = first["artifacts"][1]["path"]
        .as_str()
        .ok_or_else(|| first.to_string())?;
    let readback = call(
        &mut broker,
        &mut worker,
        "readback",
        json!({"tool":"run_python", "purpose":"再次读取生成成果", "inputs":[
        {"name":"table","path":table_path,"kind":"dataset"}, {"name":"book","path":book_path,"kind":"xlsx"}], "code":r#"
assert tables['table']['sum'].tolist() == [6,9]
assert tables['book/原始']['批號'].tolist() == ['001','002','003']
assert tables['book/原始']['備註'][1] == '=1+1'
assert metadata['table']['provenance'][0]['kinds'][3] == 'number'
result = {'ok': True}
"#}),
    )?;
    assert_eq!(readback["summary"]["ok"], true);
    println!("PASS Python: CSV/text snapshots, pandas grouping, literal formulas, tracked CSV and XLSX round trip, operation dedup.");
    if office {
        // 將生成檔複製為原始檔，實際經過 Excel 唯讀 COM -> 追蹤 CSV -> pandas。
        std::fs::copy(root.join(book_path), root.join("source.xlsx")).map_err(|e| e.to_string())?;
        let original = std::fs::read(root.join("source.xlsx")).map_err(|e| e.to_string())?;
        let inspected = call(
            &mut broker,
            &mut worker,
            "inspect",
            json!({"tool":"inspect_excel","path":"source.xlsx"}),
        )?;
        let exported = call(
            &mut broker,
            &mut worker,
            "com-export",
            json!({"tool":"export_excel_dataset","path":"source.xlsx",
            "revision":inspected["revision"],"sheet":1,"columns":["A","C"],"header_row":1,"start_row":2,"row_count":3,"name":"COM資料.csv"}),
        )?;
        let computed = call(
            &mut broker,
            &mut worker,
            "com-python",
            json!({"tool":"run_python","purpose":"Excel COM 資料交給 pandas",
            "inputs":[{"name":"excel","path":exported["dataset"]["path"],"kind":"dataset","revision":exported["dataset"]["revision"]}],"code":r#"
df = tables['excel']
assert df['A'].tolist() == ['001','002','003']
assert metadata['excel']['provenance'][0]['row'] == 2
assert metadata['excel']['provenance'][0]['path'] == 'source.xlsx'
result = {'sum': float(pd.to_numeric(df['C'], errors='raise').sum())}
"#}),
        )?;
        assert_eq!(computed["summary"]["sum"], 15.0);
        assert_eq!(
            std::fs::read(root.join("source.xlsx")).map_err(|e| e.to_string())?,
            original
        );
        println!(
            "PASS Python: actual Excel COM -> tracked CSV -> pandas; original workbook unchanged."
        );
    }
    // 路徑、來源版本及輸出檔名必須由原生 broker 拒絕，不靠提示詞約束。
    for (id, inputs, code) in [
        (
            "escape",
            json!([{"name":"a","path":"../outside.csv","kind":"csv"}]),
            "result={}",
        ),
        (
            "stale",
            json!([{"name":"a","path":"input.csv","kind":"csv","revision":"sha256:stale"}]),
            "result={}",
        ),
        (
            "raw-excel",
            json!([{"name":"a","path":"source.xlsx","kind":"xlsx"}]),
            "result={}",
        ),
        (
            "output-escape",
            json!([]),
            "emit_excel('../escape.xlsx', {'a':pd.DataFrame({'x':[1]})})",
        ),
        ("too-large-summary", json!([]), "result='x'*12001"),
    ] {
        assert!(
            call(
                &mut broker,
                &mut worker,
                id,
                json!({"tool":"run_python","purpose":"拒絕測試","inputs":inputs,"code":code})
            )
            .is_err(),
            "{id}"
        );
    }
    // 真正測試 OS 檔案、網路、程序及環境隔離；只返回是否遭拒，不回傳任何私有內容。
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let private = serde_json::to_string(&root.join("input.csv").to_string_lossy())
        .map_err(|e| e.to_string())?;
    let code = format!(
        r#"
import os, socket, subprocess, sys
denied = []
for operation in [lambda: open({private}, 'rb'), lambda: socket.create_connection(('127.0.0.1', {port}), timeout=1), lambda: subprocess.run([sys.executable, '-I', '-B', '-c', 'pass'], check=True)]:
    try:
        operation()
        denied.append(False)
    except OSError:
        denied.append(True)
assert denied == [True, True, True], denied
assert 'LM_PYTHON_SMOKE_SECRET' not in os.environ
result = {{'denied': denied}}
"#
    );
    let isolated = python::execute(&code, json!([]), &cancel)?;
    assert_eq!(isolated["summary"]["denied"], json!([true, true, true]));
    // 取消要在正在計算時終止；不是只測啟動前的取消旗標。
    let cancellation = AtomicBool::new(false);
    let cancellation_time = Instant::now();
    std::thread::scope(|scope| {
        scope.spawn(|| {
            std::thread::sleep(Duration::from_secs(4));
            cancellation.store(true, Ordering::Relaxed);
        });
        let error = python::execute("while True: pass", json!([]), &cancellation).unwrap_err();
        assert!(error.contains("取消"), "{error}");
    });
    assert!(cancellation_time.elapsed() < Duration::from_secs(15));
    println!("PASS Python: OS file/network/process isolation, environment allowlist, active cancellation.");
    let timeout_started = Instant::now();
    let timeout = python::execute("while True: pass", json!([]), &cancel).unwrap_err();
    assert!(timeout.contains("逾時"), "{timeout}");
    assert!(timeout_started.elapsed() < Duration::from_secs(145));
    // 只更動此驗收 EXE 旁的環境：清單外模組與被改寫的已知模組都要拒絕。
    let runtime = python::runtime_root()?;
    let extra = runtime.join("smoke-unlisted.py");
    let extra_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&extra)
        .map_err(|e| e.to_string())?;
    drop(extra_file);
    let unlisted = python::execute("result={}", json!([]), &cancel);
    std::fs::remove_file(&extra).map_err(|e| e.to_string())?;
    assert!(unlisted.unwrap_err().contains("清單外"));
    let known = runtime.join("lm_worker.py");
    let original_worker = std::fs::read(&known).map_err(|e| e.to_string())?;
    std::fs::write(&known, b"changed runtime fixture").map_err(|e| e.to_string())?;
    let changed_runtime = python::execute("result={}", json!([]), &cancel);
    std::fs::write(&known, &original_worker).map_err(|e| e.to_string())?;
    assert!(changed_runtime.unwrap_err().contains("損毀或版本不符"));
    let completed = broker.finish(&[])?;
    assert_eq!(completed.len(), if office { 3 } else { 2 });
    assert_eq!(
        std::fs::read_to_string(root.join("input.csv")).map_err(|e| e.to_string())?,
        csv
    );
    // 交付時再次核對，不接受被其他程式改寫的生成檔。
    std::fs::write(root.join(book_path), b"modified fixture").map_err(|e| e.to_string())?;
    assert!(broker.finish(&[]).is_err());
    let report = json!({"result":"PASS","version":env!("CARGO_PKG_VERSION"),"actual_excel_com":office,
        "python":"3.13.12","pandas":"2.2.3","numpy":"2.2.6","openpyxl":"3.1.5","duration_seconds":started.elapsed().as_secs_f64(),
        "checks":["CSV leading zeros and literal NA","LOG event duration","Big5 LOG plus UTF-8 BOM and fallback snapshots through real Python", "groupby count/mean/sum","tracked CSV and generated XLSX round trip",
            "literal XLSX formula text","idempotent results","source/output path and revision rejection","OS file/network/child-process isolation",
            "environment allowlist","active cancellation","120 second timeout","unlisted and modified runtime files rejected","original unchanged","modified output rejected at finish"]});
    std::fs::write(
        root.join("python-verification.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    println!(
        "PASS Python: timeout, unchanged sources, verified finish and changed-artifact rejection."
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
