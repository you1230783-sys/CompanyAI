//! 不連公司服務的實機驗收：使用固定工具請求，沿用正式 Broker／runtime／COM。
//! 每輪建立獨立目錄，不接受外部圖檔；成功關閉後刪除測試 DWG，保留診斷。
use crate::{
    projects::{files::Broker, sandbox::Worker, Project, Tool},
    AppResult,
};
use serde_json::{json, Value};
use std::os::windows::fs::MetadataExt;
use std::{fs, path::Path, sync::atomic::AtomicBool};

fn drawing_plan() -> Vec<Value> {
    let mut plan = Vec::new();
    let points = [
        [20, 0],
        [200, 0],
        [220, 20],
        [220, 120],
        [200, 140],
        [20, 140],
        [0, 120],
        [0, 20],
    ];
    for i in 0..points.len() {
        plan.push(
            json!({"kind":"line","start":points[i],"end":points[(i+1)%points.len()],"color":7}),
        );
    }
    for center in [[30, 30], [190, 30], [190, 110], [30, 110]] {
        plan.push(json!({"kind":"circle","center":center,"radius":6,"color":1}));
    }
    plan.extend([
        json!({"kind":"circle","center":[110,70],"radius":30,"color":4}),
        json!({"kind":"line","start":[-10,70],"end":[230,70],"color":8,"line_style":{"name":"CENTER","scale":1}}),
        json!({"kind":"line","start":[110,-10],"end":[110,150],"color":8,"line_style":{"name":"DASHED","scale":1}}),
        json!({"kind":"aligned_dimension","start":[0,20],"end":[220,20],"location":[110,-25],"color":7}),
        json!({"kind":"aligned_dimension","start":[20,0],"end":[20,140],"location":[-25,70],"color":7}),
        json!({"kind":"diameter_dimension","start":[80,70],"end":[140,70],"leader_length":12,"color":4}),
        json!({"kind":"leader","points":[[190,110],[245,165],[310,165]],"text":"安裝孔 %%c12","height":3,"color":3}),
        json!({"kind":"text","position":[30,180],"text":"離線法蘭板測試","height":5,"color":2}),
        json!({"kind":"mtext","position":[255,140],"text":"單位：mm\n固定測試計畫\n非加工核准圖","height":3,"width":100,"color":7}),
        json!({"kind":"hatch","boundary":[[260,0],[340,0],[340,60],[260,60]],"holes":[{"center":[300,30],"radius":10}],"seed":[265,5],"scale":3,"color":8}),
        json!({"kind":"curve","x":"30+t","y":"-65+12*sin(2*pi*t/160)","t_start":0,"t_end":160,"segments":40,"color":6}),
        json!({"kind":"command","command":"ZOOM E"}),
    ]);
    plan
}

fn call(
    broker: &mut Broker,
    worker: &mut Worker,
    cancel: &AtomicBool,
    tool: Tool,
    cases: &mut Vec<Value>,
) -> AppResult<Value> {
    let request = serde_json::to_value(&tool).map_err(|e| e.to_string())?;
    let response = broker.execute(&crate::jobs::new_id()?, &tool, worker, cancel);
    let response = match response {
        Ok(value) => value,
        Err(error) => json!({"ok":false,"error":error}),
    };
    cases.push(json!({"tool":request,"response":response}));
    // Broker 的 ok 表示工具已回報；可修正的 CAD 錯誤仍可能包在 result 中。
    let expected_status = match request["tool"].as_str() {
        Some("cad_inventory") => "INVENTORY",
        Some("cad_active_read") => "READ_ACTIVE",
        Some("cad_active_edit") => "EDITED_UNSAVED",
        Some("cad_draw") => "DATA_VERIFIED_VISUAL_PENDING",
        Some("cad_read") => "READ_COPY",
        Some("cad_edit") => "DATA_VERIFIED_VISUAL_PENDING",
        _ => return Err("離線計畫含未允許的工具".into()),
    };
    if response["ok"] != true || response["result"]["status"] != expected_status {
        return Err(format!("離線工具失敗：{response}"));
    }
    Ok(response["result"].clone())
}

/// 僅清理本輪新建目錄中的 CAD 成果／備份；不遞迴跟隨連結，不刪除報告。
fn clean_drawings(root: &Path, removed: &mut Vec<String>) -> AppResult<()> {
    for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if meta.file_attributes() & 0x400 != 0 {
            return Err("測試資料夾出現重解析點，停止清理。".into());
        }
        if meta.is_dir() {
            clean_drawings(&path, removed)?;
        } else if path.extension().and_then(|s| s.to_str()).is_some_and(|e| {
            matches!(
                e.to_ascii_lowercase().as_str(),
                "dwg" | "bak" | "dwl" | "dwl2"
            )
        }) {
            fs::remove_file(&path)
                .map_err(|e| format!("測試圖面清理失敗：{}：{e}", path.display()))?;
            removed.push(path.display().to_string());
        }
    }
    Ok(())
}

pub fn run(base: &Path, version: &str) -> AppResult<()> {
    if !base.is_absolute() || version.is_empty() {
        return Err("需要絕對報告路徑與 CAD ProgID。".into());
    }
    fs::create_dir_all(base).map_err(|e| e.to_string())?;
    let root = base.join(format!("cad-offline-{}", crate::jobs::new_id()?));
    fs::create_dir(&root).map_err(|e| e.to_string())?;
    let cancel = AtomicBool::new(false);
    let mut cases = Vec::new();
    let result = (|| -> AppResult<()> {
        let mut worker = Worker::start(
            &std::env::current_exe().map_err(|e| e.to_string())?,
            &cancel,
        )?;
        let mut broker = Broker::new(
            Project {
                id: "cad-offline".into(),
                name: "CAD 離線測試".into(),
                root: root.clone(),
                imports: Default::default(),
            },
            crate::jobs::new_id()?,
        )?;
        call(
            &mut broker,
            &mut worker,
            &cancel,
            Tool::CadInventory {},
            &mut cases,
        )?;
        // 讀取原本作用中文件只驗證身分／列舉，不寫入也不改變視野。
        let active = call(
            &mut broker,
            &mut worker,
            &cancel,
            Tool::CadActiveRead {
                version: version.into(),
                offset: 0,
            },
            &mut cases,
        )?;
        // 啟動固定離線測試即授權此單一縮放；只確認剛讀到的同一文件。
        // 不批准啟動 CAD、重建或任意圖元修改，也不更動正式 UI 的確認流程。
        let document = active["document"].clone();
        broker.set_cad_waiter(Some(Box::new(move |request, _, _| {
            Ok(
                if request["confirm_document"] == true && request["document"] == document {
                    "confirm"
                } else {
                    "cancel"
                }
                .into(),
            )
        })));
        call(
            &mut broker,
            &mut worker,
            &cancel,
            Tool::CadActiveEdit {
                revision: active["revision"]
                    .as_str()
                    .ok_or("缺少目前圖面 revision")?
                    .into(),
                edits: vec![],
                operations: vec![json!({"kind":"command","command":"ZOOM E"})],
            },
            &mut cases,
        )?;
        broker.set_cad_waiter(None);
        let drawn = call(
            &mut broker,
            &mut worker,
            &cancel,
            Tool::CadDraw {
                name: "offline-flange.dwg".into(),
                version: version.into(),
                operations: drawing_plan(),
            },
            &mut cases,
        )?;
        let path = drawn["path"]
            .as_str()
            .ok_or("新建未回傳成果路徑")?
            .to_owned();
        let read = call(
            &mut broker,
            &mut worker,
            &cancel,
            Tool::CadRead {
                path: path.clone(),
                version: version.into(),
                offset: 0,
                revision: None,
            },
            &mut cases,
        )?;
        let revision = read["revision"]
            .as_str()
            .ok_or("缺少讀取 revision")?
            .to_owned();
        let circles = read["entities"]
            .as_array()
            .ok_or("缺少圖元清單")?
            .iter()
            .filter(|r| r["kind"] == "circle" && r["editable"] == true)
            .take(2)
            .collect::<Vec<_>>();
        if circles.len() != 2 {
            return Err("缺少兩個可編輯的圓".into());
        }
        call(
            &mut broker,
            &mut worker,
            &cancel,
            Tool::CadEdit {
                path,
                revision,
                name: "offline-edited.dwg".into(),
                edits: vec![
                    json!({"kind":"color","handle":circles[0]["handle"],"value":5}),
                    json!({"kind":"radius","handle":circles[1]["handle"],"value":7}),
                ],
                operations: vec![
                    json!({"kind":"text","position":[30,195],"text":"副本修改完成","height":3,"color":3}),
                ],
            },
            &mut cases,
        )?;
        Ok(())
    })();
    let mut removed = Vec::new();
    // 成功路徑的正式工具已關閉自建／副本圖面；失敗時保留部分檔案及診斷，避免隱瞞未知結果。
    let cleanup = if result.is_ok() {
        clean_drawings(&root, &mut removed)
    } else {
        Ok(())
    };
    let report = json!({"version":crate::service::CURRENT_VERSION,"cad_version":version,
        "status":if result.is_ok() && cleanup.is_ok() {"PASS_DATA_VISUAL_PENDING"}else{"FAILED"},
        "uses_production_broker":true,"uses_company_service":false,"visual_verified":false,
        "cases":cases,"error":result.as_ref().err(),"cleanup_error":cleanup.as_ref().err(),
        "removed_test_files":removed,"failed_drawings_require_manual_cleanup":result.is_err()});
    crate::storage::atomic_write(
        &root.join("offline-report.json"),
        &serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )?;
    println!("{}", root.join("offline-report.json").display());
    result.and(cleanup)
}
