//! 固定資料的原生沙箱並行驗收，不提供給模型。
use super::*;

/// 固定合成資料的雙 AppContainer 驗收；兩個 Job 同時存在，取消其中一個不影響另一個。
#[cfg(debug_assertions)]
pub fn verify_parallel_workers() -> AppResult<Value> {
    let root = runtime_root()?;
    verified_runtime(&root, &AtomicBool::new(false))?;
    let barrier = std::sync::Barrier::new(2);
    let reports = std::thread::scope(|scope| -> AppResult<Vec<Value>> {
        let workers: Vec<_> = [1,2].into_iter().map(|id| {
            let barrier = &barrier; let root = &root;
            scope.spawn(move || -> AppResult<Value> {
                let cancel = AtomicBool::new(false);
                let started = Worker::start_python(&root.join("python.exe"), &cancel);
                // 即使啟動失敗也必須到達屏障，避免另一個驗收執行緒卡住。
                barrier.wait();
                let result: AppResult<(Worker, Value)> = (|| {
                    let mut worker = started?;
                    let code = format!("a=np.full(8*1024*1024, {id}, dtype=np.float64)\nresult={{'sum':float(a.sum()),'bytes':int(a.nbytes)}}");
                    let response = worker.analyze(&json!({"code":code,"inputs":[]}), &cancel)?;
                    if response["ok"] != true || response["summary"]["sum"].as_f64() != Some((8*1024*1024*id) as f64) {
                        return Err("並行 Python 計算結果不符。".into());
                    }
                    let evidence = worker.memory_evidence()?;
                    Ok((worker, evidence))
                })();
                // 保留兩個 Worker 到彼此都取得證據，不能以序列 Job 冒充並行。
                barrier.wait();
                let (_worker, evidence) = result?;
                Ok(evidence)
            })
        }).collect();
        workers
            .into_iter()
            .map(|worker| {
                worker
                    .join()
                    .map_err(|_| "Python 驗收執行緒中止".to_owned())?
            })
            .collect()
    })?;
    if reports[0]["pid"] == reports[1]["pid"]
        || reports[0]["job_handle"] == reports[1]["job_handle"]
    {
        return Err("Python 沒有使用獨立程序與 Job。".into());
    }
    // Python 協定每個程序只處理一筆要求；取消隔離用新的一對程序驗證，不重用已退出的 worker。
    let cancel = AtomicBool::new(false);
    let mut cancelled_worker = Worker::start_python(&root.join("python.exe"), &cancel)?;
    let mut survivor = Worker::start_python(&root.join("python.exe"), &AtomicBool::new(false))?;
    cancel.store(true, Ordering::Relaxed);
    if cancelled_worker
        .analyze(
            &json!({"code":"result={'unexpected':True}","inputs":[]}),
            &cancel,
        )
        .is_ok()
    {
        return Err("已取消的 Python worker 仍回覆成功。".into());
    }
    let response = survivor.analyze(
        &json!({"code":"result={'alive':True}","inputs":[]}),
        &AtomicBool::new(false),
    )?;
    if response["summary"]["alive"] != true {
        return Err("另一個 Python worker 受取消影響。".into());
    }
    Ok(
        json!({"result":"PASS","workers":reports,"per_process_limit_bytes":1073741824u64,
        "two_worker_limit_bytes":2147483648u64,"two_tasks_with_two_item_batches_max_python_processes":4,
        "batch_worst_case_process_limits_bytes":4294967296u64,"excludes":"host process, Office and CAD memory"}),
    )
}
