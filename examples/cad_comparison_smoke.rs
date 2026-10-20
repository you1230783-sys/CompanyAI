//! 比較報告的真CAD驗收：只建立獨立圖面及修改副本，刻意不重建原直徑標註。
//! 這裡驗證原生資料與報告，模型及外觀品質另行檢視，不以固定參數冒充模型通過。
use company_ai::{
    projects::{files::Broker, sandbox::Worker, Project, Tool},
    AppResult,
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path, sync::atomic::AtomicBool};

fn call(broker: &mut Broker, worker: &mut Worker, id: &str, request: Value) -> AppResult<Value> {
    let tool: Tool = serde_json::from_value(request).map_err(|e| e.to_string())?;
    let result = broker.execute(id, &tool, worker, &AtomicBool::new(false))?;
    if result["ok"] != true {
        return Err(format!("{id}: {result}"));
    }
    Ok(result["result"].clone())
}

fn verify(exe: &Path, root: &Path, version: &str) -> AppResult<()> {
    std::fs::create_dir(root).map_err(|e| e.to_string())?;
    let source = root.join("comparison-base.dwg");
    let creation = company_ai::cad::execute(
        json!({"action":"draw","version":version,"output":source,"operations":[
            {"kind":"circle","center":[0,0],"radius":60,"color":7},
            {"kind":"diameter_dimension","start":[-60,0],"end":[60,0],"leader_length":30,"color":7}
        ]}),
        &AtomicBool::new(false),
    )?;
    if creation["status"] != "DATA_VERIFIED_VISUAL_PENDING" {
        return Err(format!("測試新圖未通過資料核對：{creation}"));
    }
    let original = std::fs::read(&source).map_err(|e| e.to_string())?;
    let mut broker = Broker::new(
        Project {
            id: "comparison-smoke".into(),
            name: "修改比較驗收".into(),
            root: root.into(),
            imports: BTreeMap::new(),
        },
        company_ai::jobs::new_id()?,
    )?;
    broker.enable_memory("comparison-smoke")?;
    let mut worker = Worker::start(exe, &AtomicBool::new(false))?;
    let snapshot = call(
        &mut broker,
        &mut worker,
        "read-base",
        json!({"tool":"cad_snapshot","path":"comparison-base.dwg","version":version,"refresh":true}),
    )?;
    let rows = call(
        &mut broker,
        &mut worker,
        "query-base",
        json!({"tool":"cad_query","snapshot_id":snapshot["snapshot_id"],"filter":{},"offset":0,"limit":10,"details":true}),
    )?;
    let rows = rows["candidates"].as_array().ok_or("沒有CAD候選資料。")?;
    let find = |kind: &str| -> AppResult<String> {
        let selected: Vec<_> = rows
            .iter()
            .filter(|row| row["entity"]["kind"] == kind)
            .collect();
        if selected.len() != 1 {
            return Err(format!("{kind}測試圖元不唯一。"));
        }
        Ok(selected[0]["entity"]["handle"]
            .as_str()
            .ok_or("缺少Handle。")?
            .into())
    };
    let circle = find("circle")?;
    let dimension = find("dimension")?;
    // 只改圓的半徑，不修改尺寸本身，也不繞過原生allowed_edits去塞固定文字。
    // 報告必須將尺寸與圓的幾何關係維持unverified，提醒人工確認未更新的標註。
    let edited = call(
        &mut broker,
        &mut worker,
        "change-radius",
        json!({"tool":"cad_edit","path":"comparison-base.dwg","revision":snapshot["revision"],"name":"radius-80.dwg","edits":[{"handle":circle,"kind":"radius","value":80}],"operations":[]}),
    )?;
    if edited["saved"] != true
        || edited["status"] != "DATA_VERIFIED_VISUAL_PENDING"
        || edited["comparison_available"] != true
    {
        return Err(format!("圓修改未取得存檔重開與比較結果：{edited}"));
    }
    let exported = call(
        &mut broker,
        &mut worker,
        "export-comparison",
        json!({"tool":"export_cad_comparison","source_operation":"change-radius","name":"CAD修改比較.txt"}),
    )?;
    let comparison: Value = serde_json::from_slice(
        &std::fs::read(root.join(exported["json_path"].as_str().ok_or("缺少JSON路徑。")?))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    assert_eq!(comparison["complete"], true);
    assert!(comparison["changed"]
        .as_array()
        .ok_or("缺少差異。")?
        .iter()
        .any(|row| row["handle"] == circle
            && row["measurements"]["before_diameter"] == 120.0
            && row["measurements"]["after_diameter"] == 160.0));
    assert!(comparison["annotation_checks"]
        .as_array()
        .ok_or("缺少尺寸核對。")?
        .iter()
        .any(|row| row["handle"] == dimension
            && row["geometry_consistency"] == "unverified"
            && row.get("warning").is_some()));
    broker.finish_cancellable(&[], &AtomicBool::new(false))?;
    assert_eq!(std::fs::read(&source).map_err(|e| e.to_string())?, original);
    std::fs::write(root.join("verification.json"), serde_json::to_vec_pretty(&json!({"status":"PASS_NATIVE_COMPARISON","version":version,"source_unchanged":true,"fixed_text_override_tested":false,"creation":creation,"edited":edited,"report":exported,"comparison":comparison})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    println!("PASS native CAD comparison: {version}; diameter 120 to 160; original dimension retained; geometry association warning. Fixed TextOverride is a separate contract test.");
    Ok(())
}

fn run() -> AppResult<()> {
    let args: Vec<_> = std::env::args().collect();
    let exe = Path::new(args.get(1).ok_or("需要待測EXE。")?);
    let root = Path::new(args.get(2).ok_or("需要新的測試目錄。")?);
    std::fs::create_dir(root).map_err(|e| e.to_string())?;
    for (name, version) in [
        ("2008", "Icad.Application.1"),
        ("2022", "GStarCAD.Application.22"),
    ] {
        verify(exe, &root.join(name), version)?;
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
