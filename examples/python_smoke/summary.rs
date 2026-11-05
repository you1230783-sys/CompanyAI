//! 真正 AppContainer 的摘要序列化失敗：保留鍵路徑，也不能冒稱計算尚未執行。
use super::*;

pub(super) fn run(broker: &mut Broker, worker: &mut Worker) -> AppResult<()> {
    let good = call(
        broker,
        worker,
        "summary-numpy",
        json!({"tool":"run_python",
        "purpose":"NumPy 摘要驗收", "inputs":[],
        "code":"result={'n':np.int64(121),'peaks':np.array([1.,2.]),'ok':np.bool_(True),'nested':{'value':np.float32(1.5)}}"}),
    )?;
    assert_eq!(
        good["summary"],
        json!({"n":121,"peaks":[1.0,2.0],"ok":true,"nested":{"value":1.5}})
    );
    for (index, code, expected) in [
        (0, "result={'x':np.float64('nan')}", "result['x']"),
        (
            1,
            "result={'outer':[np.float64('inf')]}",
            "result['outer'][0]",
        ),
        (2, "result={'x':np.zeros(12001)}", "12000"),
    ] {
        let tool: Tool = serde_json::from_value(
            json!({"tool":"run_python", "purpose":"摘要邊界拒絕", "inputs":[], "code":code}),
        )
        .map_err(|e| e.to_string())?;
        let response = broker.execute(
            &format!("summary-reject-{index}"),
            &tool,
            worker,
            &AtomicBool::new(false),
        )?;
        assert_eq!(response["ok"], false, "{response}");
        assert!(
            response["error"].as_str().unwrap().contains(expected),
            "{response}"
        );
        assert_ne!(
            response["error_kind"], "contract",
            "Python 已執行，不能標示未執行"
        );
    }
    println!("PASS Python summary: real AppContainer NumPy scalar/array, nested values, NaN/Inf path and 12000-character boundary");
    Ok(())
}
