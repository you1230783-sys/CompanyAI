//! 真實讀檔版本交給Python的串接回歸；雜湊表示相容不代表允許來源變更。
use super::*;
use sha2::{Digest, Sha256};

pub(super) fn run(broker: &mut Broker, worker: &mut Worker, project: &Project) -> AppResult<()> {
    let bytes = b"value\n10\n";
    let path = "revision_bridge.csv";
    std::fs::write(project.root.join(path), bytes).map_err(|e| e.to_string())?;
    let read = call(
        broker,
        worker,
        "revision-file-read",
        json!({"tool":"read_file","path":path,"offset":0}),
    )?;
    let revision = read["revision"].as_str().ok_or("讀檔缺少來源版本。")?;
    assert_eq!(revision, format!("{:x}", Sha256::digest(bytes)));
    call(
        broker,
        worker,
        "revision-load-python",
        json!({"tool":"load_skill","id":"python-analysis"}),
    )?;
    let request = json!({"tool":"run_python","purpose":"核對一般讀檔的原始雜湊可直接用於CSV分析",
        "inputs":[{"name":"data","path":path,"kind":"csv","revision":revision}],
        "code":"result = {'sum': float(pd.to_numeric(tables['data']['value'], errors='raise').sum())}"});
    let result = call(broker, worker, "revision-same-file", request.clone())?;
    assert_eq!(result["summary"]["sum"], 10.0);
    // 只變更本回歸自己建立的合成來源；不同operation id仍不能沿用舊雜湊。
    std::fs::write(project.root.join(path), b"value\n12\n").map_err(|e| e.to_string())?;
    let changed = call(broker, worker, "revision-changed-file", request).unwrap_err();
    assert!(changed.contains("Python 來源版本已變更"), "{changed}");
    println!("PASS Python revision bridge: actual read_file SHA accepted, CSV sum10 verified, changed source rejected.");
    Ok(())
}
