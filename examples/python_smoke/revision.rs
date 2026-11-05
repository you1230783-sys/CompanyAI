//! 真實讀檔版本交給Python的串接回歸；雜湊表示相容不代表允許來源變更。
use super::*;
use sha2::{Digest, Sha256};

pub(super) fn run(broker: &mut Broker, worker: &mut Worker, project: &Project) -> AppResult<()> {
    // BOM不屬於解碼後文字；先前無BOM案例無法發現兩種版本混用。
    let bytes = b"\xef\xbb\xbfvalue\n10\n";
    let path = "revision_bridge.csv";
    std::fs::write(project.root.join(path), bytes).map_err(|e| e.to_string())?;
    let read = call(
        broker,
        worker,
        "revision-file-read",
        json!({"tool":"read_file","path":path,"offset":0}),
    )?;
    let revision = read["source_revision"]
        .as_str()
        .ok_or("讀檔缺少來源版本。")?;
    assert_eq!(revision, format!("sha256:{:x}", Sha256::digest(bytes)));
    assert_eq!(
        read["revision"],
        format!("{:x}", Sha256::digest(b"value\n10\n"))
    );
    call(
        broker,
        worker,
        "revision-load-python",
        json!({"tool":"load_skill","id":"python-analysis"}),
    )?;
    let request = json!({"tool":"run_python","purpose":"核對一般讀檔的原始雜湊可直接用於CSV分析",
        "inputs":[{"name":"data","path":path,"kind":"csv","revision":revision}],
        "code":"from scipy.optimize import nnls\ndf=tables['data'].copy()\nx,residual=nnls(np.array([[1.,0.],[0.,1.],[1.,1.]]),np.array([2.,3.,5.]))\nassert np.allclose(x,[2.,3.]) and residual<1e-10\nresult = {'sum': float(pd.to_numeric(df['value'], errors='raise').sum()),'nnls':x.tolist()}\nemit_table('eis_data', df)\nemit_excel('probe_book', {'資料':df})\nfig,ax=plt.subplots(figsize=(4,3))\nax.plot([1,2],[2,3]);ax.set_title('科學計算驗證')\nemit_figure('analysis_plot',fig)"});
    let result = call(broker, worker, "revision-same-file", request.clone())?;
    assert_eq!(result["summary"]["sum"], 10.0);
    assert_eq!(result["artifacts"].as_array().unwrap().len(), 3);
    assert!(result["artifacts"][0]["dataset"]["path"]
        .as_str()
        .unwrap()
        .ends_with(".csv"));
    assert!(result["artifacts"][1]["path"]
        .as_str()
        .unwrap()
        .ends_with(".xlsx"));
    assert!(result["artifacts"][2]["path"]
        .as_str()
        .unwrap()
        .ends_with(".png"));
    for (actual, expected) in result["summary"]["nnls"]
        .as_array()
        .unwrap()
        .iter()
        .zip([2.0, 3.0])
    {
        assert!((actual.as_f64().unwrap() - expected).abs() < 1e-10);
    }
    // 只變更本回歸自己建立的合成來源；不同operation id仍不能沿用舊雜湊。
    std::fs::write(project.root.join(path), b"value\n12\n").map_err(|e| e.to_string())?;
    let changed = call(broker, worker, "revision-changed-file", request).unwrap_err();
    assert!(changed.contains("Python 來源版本已變更"), "{changed}");
    println!("PASS Python revision bridge: BOM source_revision accepted, SciPy NNLS verified, extension-free CSV/XLSX/PNG published, changed source rejected.");
    Ok(())
}
