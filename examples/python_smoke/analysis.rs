//! 真正執行分段 Python、證據回查、筆數核對及跨對話方法記憶。
use super::*;
use std::io::Write;

pub(super) fn run(broker: &mut Broker, worker: &mut Worker, project: &Project) -> AppResult<()> {
    let path = "analysis-big5.log";
    std::fs::write(
        project.root.join(path),
        company_ai::projects::text::encode(
            "08:00:00 開始\n08:00:01 結束\n08:00:02 開始\n08:00:03 結束\n",
            company_ai::projects::text::Encoding::CodePage(950),
        )?,
    )
    .map_err(|e| e.to_string())?;
    let first = call(
        broker,
        worker,
        "analysis-first",
        json!({"tool":"run_python","purpose":"分段核對","inputs":[{"name":"a","path":path,"kind":"text","log_range":{"start_line":1,"line_count":2}}],"code":"assert '開始' in texts['a']\nassert metadata['a']['start_line']==1\nresult={'input':2,'parsed':2,'unparsed':0,'counterexamples':1}"}),
    )?;
    assert_eq!(first["sources"][0]["next_line"], 3);
    assert_eq!(first["sources"][0]["eof"], false);
    let last = call(
        broker,
        worker,
        "analysis-last",
        json!({"tool":"run_python","purpose":"分段接續","inputs":[{"name":"a","path":path,"kind":"text","revision":first["sources"][0]["revision"],"log_range":{"start_line":3,"line_count":2}}],"code":"assert metadata['a']['start_line']==3\nassert '結束' in texts['a']\nresult={'lines':len(texts['a'].splitlines())}"}),
    )?;
    assert_eq!(last["sources"][0]["next_line"], 5);
    assert_eq!(last["sources"][0]["eof"], true);
    call(
        broker,
        worker,
        "analysis-read",
        json!({"tool":"read_log","path":path,"start_line":1,"line_count":4}),
    )?;
    let report = json!({"goal":"檢查開始與結束事件","current_step":"核對反例","open_questions":["仍有一個反例，不能推論必然關係"],"superseded":[],
        "findings":[{"claim":"LOG 記錄開始及結束","status":"confirmed","evidence":[{"operation_id":"analysis-read","pointer":"/lines"}]}],
        "checks":[{"label":"解析數量","operation_id":"analysis-first","kind":"balance","total_pointer":"/summary/input","part_pointers":["/summary/parsed","/summary/unparsed"]},
            {"label":"反例","operation_id":"analysis-first","kind":"zero","total_pointer":"/summary/counterexamples","part_pointers":[]}],
        "method":{"title":"開始結束 LOG 配對","applicability":"相同時間與事件格式","steps":"依來源行號解析開始及結束；跨段保留未配對事件。","validation":"檢查解析數量、未配對及反例，再回讀原文。","limitations":"設備重啟、跨午夜或序號重設需重新確認。"}});
    let saved = call(
        broker,
        worker,
        "analysis-report",
        json!({"tool":"record_analysis","report":report}),
    )?;
    assert_eq!(saved["report"]["checks"][0]["passed"], true);
    assert_eq!(saved["report"]["checks"][1]["passed"], false);
    let note = saved["report"]["method_note"]["id"]
        .as_str()
        .ok_or("方法未保存")?;
    let other = company_ai::projects::memory::Memory::open(project.clone(), "another-chat")?;
    assert!(other.read_note(note)?["body"]
        .as_str()
        .unwrap_or("")
        .contains("失效條件"));
    assert!(other
        .context(&[company_ai::protocol::Message::user("開始結束 LOG 配對")])?
        .iter()
        .any(|m| m.content.contains("分析方法：開始結束 LOG 配對")));
    let mut invalid = report.clone();
    invalid["findings"][0]["evidence"][0]["operation_id"] = json!("not-executed");
    let bad: Tool = serde_json::from_value(json!({"tool":"record_analysis","report":invalid}))
        .map_err(|e| e.to_string())?;
    assert_eq!(
        broker.execute("analysis-bad", &bad, worker, &AtomicBool::new(false))?["ok"],
        false
    );
    let again = call(
        broker,
        worker,
        "analysis-report-again",
        json!({"tool":"record_analysis","report":report}),
    )?;
    assert_eq!(again["report"]["method_note"]["id"], note);
    // 來源改版後舊 revision 必須拒絕，不能把兩個版本的分段合併。
    std::fs::write(project.root.join(path), "changed").map_err(|e| e.to_string())?;
    let changed:Tool=serde_json::from_value(json!({"tool":"run_python","purpose":"版本拒絕","inputs":[{"name":"a","path":path,"kind":"text","revision":first["sources"][0]["revision"],"log_range":{"start_line":3,"line_count":2}}],"code":"result={}"})).map_err(|e|e.to_string())?;
    assert_eq!(
        broker.execute("analysis-stale", &changed, worker, &AtomicBool::new(false))?["ok"],
        false
    );
    // 產生超過 77 MiB 的合成資料，確認不會再被原本 32 MiB 門檻擋下。
    let large = "analysis-large.log";
    let mut output = std::io::BufWriter::new(
        std::fs::File::create(project.root.join(large)).map_err(|e| e.to_string())?,
    );
    let line = format!("08:00:00 {}\n", "x".repeat(1014));
    for _ in 0..80_000 {
        output
            .write_all(line.as_bytes())
            .map_err(|e| e.to_string())?;
    }
    output.flush().map_err(|e| e.to_string())?;
    drop(output);
    assert!(
        std::fs::metadata(project.root.join(large))
            .map_err(|e| e.to_string())?
            .len()
            > 77 * 1024 * 1024
    );
    let segment = call(
        broker,
        worker,
        "analysis-large",
        json!({"tool":"run_python","purpose":"大檔完整行分段","inputs":[{"name":"a","path":large,"kind":"text","log_range":{"start_line":1,"line_count":50000}}],"code":"assert len(texts['a'].encode('utf8')) <= 2*1024*1024\nassert metadata['a']['line_count']==len(texts['a'].splitlines())\nresult={'rows':metadata['a']['line_count']}"}),
    )?;
    assert_eq!(segment["sources"][0]["eof"], false);
    assert!(segment["summary"]["rows"]
        .as_u64()
        .is_some_and(|n| n > 1000 && n < 50000));
    println!("PASS analysis: Big5 chunks, source lines/revisions, >77MiB LOG, evidence references, arithmetic/counterexamples, method dedup and cross-chat recall");
    Ok(())
}
