//! 開發用量測入口；使用正式驗證與 AppContainer，不略過完整性檢查。
use super::*;
use std::time::Instant;

/// 同一程序連續兩次最小分析，拆開驗證、隔離程序啟動與分析管道耗時。
/// 分析管道包含 import、初始化、執行與序列化；不把此數字冒稱純 import。
pub fn measure_runtime() -> AppResult<Value> {
    let root = runtime_root()?;
    let cancel = AtomicBool::new(false);
    let mut measurements = Vec::new();
    for _ in 0..2 {
        let total = Instant::now();
        let phase = Instant::now();
        verified_runtime(&root, &cancel)?;
        let verify_ms = phase.elapsed().as_secs_f64() * 1000.0;
        let phase = Instant::now();
        let mut worker = Worker::start_python(&root.join("python.exe"), &cancel)?;
        let startup_ms = phase.elapsed().as_secs_f64() * 1000.0;
        let phase = Instant::now();
        let response = worker.analyze(&json!({"code":
            "import time\nt = time.perf_counter()\nresult={'x':1}\nresult['execution_ms']=(time.perf_counter()-t)*1000",
            "inputs":[]}), &cancel)?;
        if response["ok"] != true || response["summary"]["x"] != 1 {
            return Err(format!("量測分析失敗：{response}"));
        }
        measurements.push(json!({"verify_ms":verify_ms,"startup_ms":startup_ms,
            "analysis_ms":phase.elapsed().as_secs_f64()*1000.0,
            "execution_ms":response["summary"]["execution_ms"],
            "total_ms":total.elapsed().as_secs_f64()*1000.0}));
    }
    Ok(json!({"measurements":measurements}))
}
