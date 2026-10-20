//! 固定 PNG 經正式 broker 嵌入三種 Office 文件；不讀取使用者文件。
use company_ai::{
    projects::{files::Broker, office, sandbox::Worker, Project, Tool},
    AppResult,
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path, sync::atomic::AtomicBool};

fn verify(exe: &Path, root: &Path) -> AppResult<()> {
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let png = include_bytes!("../src/projects/charts/test-export.png");
    let cancel = AtomicBool::new(false);
    let mut worker = Worker::start(exe, &cancel)?;
    let mut outputs = Vec::new();
    for (ext, target) in [
        ("docx", json!({"kind":"word","before":null})),
        ("xlsx", json!({"kind":"excel","sheet":1,"cell":"H2"})),
        ("pptx", json!({"kind":"ppt","slide":1,"left":40,"top":110})),
    ] {
        std::fs::write(root.join("chart.png"), png).map_err(|e| e.to_string())?;
        let mut broker = Broker::new(
            Project {
                id: "images".into(),
                name: "images".into(),
                root: root.into(),
                imports: BTreeMap::new(),
            },
            "images".into(),
        )?;
        let mut call = |id: &str, tool: Tool| -> AppResult<Value> {
            let response = broker.execute(id, &tool, &mut worker, &cancel)?;
            if response["ok"] != true {
                return Err(format!("{ext} operation {id}: {response}"));
            }
            Ok(response["result"].clone())
        };
        let created = call(
            "create",
            Tool::CreateWorkingCopy {
                source: None,
                name: format!("圖片測試.{ext}"),
            },
        )?;
        let id = created["copy_id"].as_str().unwrap().to_owned();
        let mut revision = created["revision"].as_str().unwrap().to_owned();
        if ext == "pptx" {
            let result = call(
                "slide",
                Tool::OfficeAction {
                    copy_id: id.clone(),
                    revision: revision.clone(),
                    operation: Box::new(
                        serde_json::from_value(
                            json!({"kind":"ppt_slide","layout":"title","title":"圖表","body":""}),
                        )
                        .unwrap(),
                    ),
                },
            )?;
            revision = result["revision"].as_str().unwrap().to_owned();
        }
        let make = |path: &str, revision: &str| Tool::OfficeAction {
            copy_id: id.clone(),
            revision: revision.into(),
            operation: Box::new(
                serde_json::from_value(
                    json!({"kind":"insert_image","path":path,"width":320,"target":target}),
                )
                .unwrap(),
            ),
        };
        assert!(call("escape", make("../chart.png", &revision)).is_err());
        let result = call("image", make("chart.png", &revision))?;
        assert_eq!(result["structure"]["images"].as_array().unwrap().len(), 1);
        assert_ne!(result["revision"], revision);
        assert_eq!(call("image", make("chart.png", &revision))?, result);
        revision = result["revision"].as_str().unwrap().to_owned();
        std::fs::write(root.join("chart.png"), b"changed").map_err(|e| e.to_string())?;
        assert!(call(
            "changed",
            Tool::SaveCopy {
                copy_id: id.clone(),
                revision: revision.clone()
            }
        )
        .is_err());
        std::fs::write(root.join("chart.png"), png).map_err(|e| e.to_string())?;
        let saved = call(
            "save",
            Tool::SaveCopy {
                copy_id: id.clone(),
                revision,
            },
        )?;
        let source = saved["path"].as_str().unwrap();
        let source_bytes = std::fs::read(root.join(source)).map_err(|e| e.to_string())?;
        let next = call(
            "existing",
            Tool::CreateWorkingCopy {
                source: Some(source.into()),
                name: format!("插入第二張.{ext}"),
            },
        )?;
        let next_id = next["copy_id"].as_str().unwrap().to_owned();
        let mut next_target = target.clone();
        if ext == "docx" {
            next_target["before"] = json!("p1");
        }
        let inserted = call("existing_image", Tool::OfficeAction {copy_id:next_id.clone(),revision:next["revision"].as_str().unwrap().into(),operation:Box::new(serde_json::from_value(json!({"kind":"insert_image","path":"chart.png","width":160,"target":next_target})).unwrap())})?;
        assert_eq!(inserted["structure"]["images"].as_array().unwrap().len(), 2);
        call(
            "save_existing",
            Tool::SaveCopy {
                copy_id: next_id.clone(),
                revision: inserted["revision"].as_str().unwrap().into(),
            },
        )?;
        assert_eq!(
            std::fs::read(root.join(source)).map_err(|e| e.to_string())?,
            source_bytes
        );
        // 正式交付要求列出本次所有副本，不能分成兩次各交付一份。
        outputs.extend(broker.finish(&[id, next_id])?.into_iter().map(|path| {
            let count = if path.contains("/插入第二張") {
                2
            } else {
                1
            };
            (path, count)
        }));
        println!(
            "PASS {ext}: embedded PNG, revision, replay, changed-source rejection and save/reopen."
        );
    }
    std::fs::remove_file(root.join("chart.png")).map_err(|e| e.to_string())?;
    for (output, count) in outputs {
        let snapshot = office::process(&root.join(&output), None, None, None, &cancel)?;
        assert_eq!(
            snapshot.structure["images"].as_array().unwrap().len(),
            count
        );
        println!("PASS original PNG removed: {output}");
    }
    Ok(())
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let result = (|| {
        verify(
            Path::new(args.get(1).ok_or("需要 EXE")?),
            Path::new(args.get(2).ok_or("需要獨立測試目錄")?),
        )
    })();
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
