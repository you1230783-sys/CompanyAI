//! 真正的 Broker／CAD 共用目錄回歸。只建立唯一測試目錄，不修改使用者圖面。
//! 這是固定參數測試，與 Strata 原始輸出分開記錄，不能代替模型內容驗收。
use company_ai::{
    projects::{files::Broker, sandbox::Worker, Project, Tool},
    AppResult,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path, sync::atomic::AtomicBool};

struct Fixture {
    broker: Broker,
    worker: Worker,
    cancel: AtomicBool,
    records: Vec<Value>,
}

impl Fixture {
    fn execute(&mut self, tool: Tool) -> AppResult<Value> {
        let id = format!("fixture_{}", self.records.len());
        let result = self
            .broker
            .execute(&id, &tool, &mut self.worker, &self.cancel)?;
        self.records.push(json!({"tool":tool,"outcome":result}));
        if result["ok"] != true {
            return Err(format!("Broker測試失敗：{result}"));
        }
        Ok(result["result"].clone())
    }

    fn read(&mut self, path: &str, version: &str) -> AppResult<(String, Vec<Value>)> {
        let snapshot = self.execute(Tool::CadSnapshot {
            path: Some(path.into()),
            version: version.into(),
            refresh: true,
        })?;
        let snapshot_id = snapshot["snapshot_id"]
            .as_str()
            .ok_or("缺少snapshot_id")?
            .to_owned();
        let revision = snapshot["revision"]
            .as_str()
            .ok_or("缺少revision")?
            .to_owned();
        let mut rows = Vec::new();
        let mut offset = 0;
        loop {
            let result = self.execute(Tool::CadQuery {
                snapshot_id: snapshot_id.clone(),
                filter: Default::default(),
                offset,
                limit: 10,
                details: true,
            })?;
            rows.extend(
                result["candidates"]
                    .as_array()
                    .ok_or("缺少候選")?
                    .iter()
                    .map(|row| row["entity"].clone()),
            );
            match result["next_offset"].as_u64() {
                Some(next) => offset = next as usize,
                None => break,
            }
        }
        Ok((revision, rows))
    }

    fn edit(
        &mut self,
        path: &str,
        revision: String,
        name: &str,
        edits: Vec<Value>,
    ) -> AppResult<String> {
        let result = self.execute(Tool::CadEdit {
            path: path.into(),
            revision,
            name: name.into(),
            edits,
            operations: vec![],
        })?;
        if result["status"] != "DATA_VERIFIED_VISUAL_PENDING" || result["saved"] != true {
            return Err(format!("修改未存檔重開：{result}"));
        }
        Ok(result["path"].as_str().ok_or("缺少修改成果")?.to_owned())
    }
}

fn point(value: &Value, x: f64, y: f64) -> bool {
    value[0].as_f64().is_some_and(|n| (n - x).abs() < 1e-7)
        && value[1].as_f64().is_some_and(|n| (n - y).abs() < 1e-7)
}

fn handle(rows: &[Value], predicate: impl Fn(&Value) -> bool) -> AppResult<String> {
    let selected: Vec<_> = rows.iter().filter(|r| predicate(r)).collect();
    if selected.len() != 1 {
        return Err(format!("測試目標不唯一：{}筆", selected.len()));
    }
    Ok(selected[0]["handle"]
        .as_str()
        .ok_or("缺少Handle")?
        .to_owned())
}

fn verify(exe: &Path, root: &Path, version: &str) -> AppResult<Value> {
    let conversation = company_ai::jobs::new_id()?;
    let folder = format!(
        "_AI_Output/{}",
        company_ai::projects::files::conversation_folder_name(&conversation)?
    );
    let directory = root.join(&folder);
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let source = format!("{folder}/native-base.dwg");
    let cancel = AtomicBool::new(false);
    let creation = company_ai::cad::execute(
        json!({"action":"draw","version":version,"output":root.join(&source),"operations":[
            {"kind":"line","start":[0,0],"end":[80,0],"color":7},
            {"kind":"line","start":[40,-30],"end":[40,30],"color":7},
            {"kind":"line","start":[120,0],"end":[180,0],"color":7},
            {"kind":"line","start":[120,0],"end":[120,60],"color":7},
            {"kind":"circle","center":[220,20],"radius":10,"color":7},
            {"kind":"polyline","points":[[260,0],[300,0],[300,40],[260,40]],"closed":true,"color":7}
        ]}),
        &cancel,
    )?;
    if creation["status"] != "DATA_VERIFIED_VISUAL_PENDING" {
        return Err(format!("建圖失敗：{creation}"));
    }
    let original = std::fs::read(root.join(&source)).map_err(|e| e.to_string())?;
    let project = Project {
        id: conversation.clone(),
        name: "CAD共用目錄回歸".into(),
        root: root.into(),
        imports: BTreeMap::new(),
    };
    let mut broker = Broker::new(project, company_ai::jobs::new_id()?)?;
    broker.enable_memory(&conversation)?;
    let mut test = Fixture {
        broker,
        worker: Worker::start(exe, &cancel)?,
        cancel,
        records: vec![],
    };
    let result = (|| {
        // 首次讀取刻意傳入安裝年份，後續使用自動別名，核對解析後始終綁定同一 ProgID。
        let requested_year = match version {
            "Icad.Application.1" => "2008",
            "GStarCAD.Application.22" => "2022",
            _ => return Err("測試版本未定義年份。".into()),
        };
        let (revision, rows) = test.read(&source, requested_year)?;
        let line = |start: (f64, f64), end: (f64, f64)| {
            handle(&rows, |r| {
                r["kind"] == "line"
                    && point(&r["properties"]["StartPoint"], start.0, start.1)
                    && point(&r["properties"]["EndPoint"], end.0, end.1)
            })
        };
        let l1 = line((0., 0.), (80., 0.))?;
        let l2 = line((40., -30.), (40., 30.))?;
        let l3 = line((120., 0.), (180., 0.))?;
        let l4 = line((120., 0.), (120., 60.))?;
        let circle = handle(&rows, |r| r["kind"] == "circle")?;
        let poly = handle(&rows, |r| r["kind"] == "polyline")?;
        let second = test.edit(&source, revision, "native-v2.dwg", vec![
            json!({"handle":l1,"kind":"trim","value":{"boundary":l2,"pick":[60,0]}}),
            json!({"handle":l3,"kind":"fillet","value":{"other_handle":l4,"radius":10,"pick":[175,0],"other_pick":[120,55]}}),
            json!({"handle":circle,"kind":"offset","value":{"distance":5}}),
            json!({"handle":poly,"kind":"copy","value":{"offset":[0,100]}}),
        ])?;
        let (revision, rows) = test.read(&second, "latest")?;
        let copy = handle(&rows, |r| {
            r["kind"] == "polyline" && point(&r["geometry"]["vertices"][0], 260., 100.)
        })?;
        let third = test.edit(&second, revision, "native-v3.dwg", vec![
            json!({"handle":copy,"kind":"rotate","value":{"base":[260,100],"angle":90}}),
            json!({"handle":circle,"kind":"mirror","value":{"axis_start":[200,-100],"axis_end":[200,100],"copy":true}}),
        ])?;
        let (revision, _) = test.read(&third, "auto")?;
        let final_path = test.edit(&third, revision, "native-v4.dwg", vec![
            json!({"handle":circle,"kind":"array","value":{"rows":2,"columns":3,"row_spacing":40,"column_spacing":40}}),
        ])?;
        let (_, rows) = test.read(&final_path, "最新版")?;
        assert_eq!(rows.len(), 15);
        for (kind, count) in [("line", 4), ("arc", 1), ("circle", 8), ("polyline", 2)] {
            assert_eq!(rows.iter().filter(|r| r["kind"] == kind).count(), count);
        }
        for (x, y, radius) in [
            (220., 20., 10.),
            (220., 20., 15.),
            (180., 20., 10.),
            (260., 20., 10.),
            (300., 20., 10.),
            (220., 60., 10.),
            (260., 60., 10.),
            (300., 60., 10.),
        ] {
            handle(&rows, |r| {
                r["kind"] == "circle"
                    && point(&r["properties"]["Center"], x, y)
                    && r["properties"]["Radius"]
                        .as_f64()
                        .is_some_and(|v| (v - radius).abs() < 1e-7)
            })?;
        }
        for path in [&source, &second, &third, &final_path] {
            assert_eq!(Path::new(path).parent(), Some(Path::new(&folder)));
        }
        assert_eq!(
            std::fs::read(root.join(&source)).map_err(|e| e.to_string())?,
            original
        );
        Ok(
            json!({"status":"PASS_SHARED_CAD_BROKER_SAVE_REOPEN","version":version,"source":source,"final_path":final_path,
            "original_sha256":format!("{:x}",Sha256::digest(&original)),"original_unchanged":true,"successful_writes":3,
            "shared_output_directory":true,"final_entities":rows,"creation":creation}),
        )
    })();
    let report = json!({"result":result.as_ref().ok(),"error":result.as_ref().err(),"operations":test.records});
    std::fs::write(
        root.join("native-broker-report.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    result
}

fn run() -> AppResult<()> {
    let args: Vec<_> = std::env::args().collect();
    let exe = Path::new(args.get(1).ok_or("需要待測EXE路徑")?);
    let base = Path::new(args.get(2).ok_or("需要測試資料夾")?).join(company_ai::jobs::new_id()?);
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    for (name, version) in [
        ("2008", "Icad.Application.1"),
        ("2022", "GStarCAD.Application.22"),
    ] {
        let root = base.join(name);
        std::fs::create_dir(&root).map_err(|e| e.to_string())?;
        verify(exe, &root, version)?;
        println!("PASS shared CAD Broker: {version}; three saved/reopened copies, 15 entities, original unchanged.");
    }
    println!("Fixture: {}", base.display());
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
