//! 經真正 AppContainer 驗證兩個 Python 同時執行、混合工作及部分失敗；不用模型筆記當證据。
use super::*;

fn task(id: &str, tool: &str, arguments: Value) -> Value {
    json!({"task_id":id,"tool":tool,"arguments_json":arguments.to_string()})
}
pub(super) fn run(project: &Project, worker: &mut Worker) -> AppResult<()> {
    let mut project = project.clone();
    project.root = project.root.join("batch-fixture");
    std::fs::create_dir(&project.root).map_err(|e| e.to_string())?;
    std::fs::write(
        project.root.join("next.py"),
        "# 下一份資料的讀取準備\nvalue=42\n",
    )
    .map_err(|e| e.to_string())?;
    let mut broker = Broker::new(project.clone(), "parallel-test".into())?;
    broker.enable_memory("parallel-chat")?;
    call(
        &mut broker,
        worker,
        "skill",
        json!({"tool":"load_skill","id":"python-analysis"}),
    )?;
    let request = json!({"tool":"run_batch","tasks":[
        task("a","run_python",json!({"purpose":"並行A","inputs":[],"code":"import time\nstart=time.time()\ntime.sleep(8)\nemit_table('a.csv',pd.DataFrame({'x':[1,2,3],'y':[90,100,110]}))\nresult={'start':start,'end':time.time(),'value':42}"})),
        task("b","run_python",json!({"purpose":"並行B","inputs":[],"code":"import time\nstart=time.time()\ntime.sleep(8)\nemit_table('b.csv',pd.DataFrame({'x':[1,2],'y':[2,4]}))\nresult={'start':start,'end':time.time(),'value':84}"}))]});
    let result = call(&mut broker, worker, "parallel", request.clone())?;
    let a = &result["tasks"][0]["outcome"]["result"];
    let b = &result["tasks"][1]["outcome"]["result"];
    assert_eq!(a["summary"]["value"], 42, "{result}");
    assert_eq!(b["summary"]["value"], 84, "{result}");
    let number = |v: &Value, key: &str| v["summary"][key].as_f64().unwrap();
    assert!(
        number(a, "start").max(number(b, "start")) < number(a, "end").min(number(b, "end")),
        "Python執行區間必須真的重疊"
    );
    assert_eq!(call(&mut broker, worker, "parallel", request)?, result);
    assert_eq!(broker.published().len(), 2, "相同批次不可重複發布");
    // CSV 成果的來源路徑／版本位於 dataset；最外層另包含預覽與統計。
    let artifact = &a["artifacts"][0]["dataset"];
    assert!(artifact["path"].is_string() && artifact["revision"].is_string());
    let mixed = call(
        &mut broker,
        worker,
        "mixed",
        json!({"tool":"run_batch","tasks":[
        task("chart","chart_dataset",json!({"path":artifact["path"],"revision":artifact["revision"],"x_column":"x","y_columns":["y"],"start_row":1,"row_count":3,"kind":"scatter","title":"並行圖","x_label":"x","y_label":"y"})),
        task("read","read_file",json!({"path":"next.py","offset":0})),
        task("fail","run_python",json!({"purpose":"合成失敗","inputs":[],"code":"raise ValueError('fixture')"})),
        task("valid","run_python",json!({"purpose":"仍應成功","inputs":[],"code":"result={'ok':True}"}))]}),
    )?;
    for index in [0, 1, 3] {
        assert_eq!(mixed["tasks"][index]["outcome"]["ok"], true, "{mixed}");
    }
    assert_eq!(mixed["tasks"][2]["outcome"]["ok"], false);
    assert_eq!(broker.charts().len(), 1);
    assert_eq!(
        mixed["tasks"][1]["outcome"]["result"]["text"],
        "# 下一份資料的讀取準備\nvalue=42\n"
    );
    std::fs::write(
        project.root.join("batch-verification.json"),
        serde_json::to_vec_pretty(&json!({"result":"PASS","parallel":result,"mixed":mixed}))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    println!("PASS real batch: overlapping Python processes, independent CSVs, idempotent replay, chart plus read, partial failure preserves success");
    Ok(())
}
