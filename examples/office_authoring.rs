//! 透過正式 broker 新建、編排及重新開啟三種 Office 文件；不以手工 ZIP 冒充驗證。
use company_ai::{
    projects::{files::Broker, sandbox::Worker, Project, Tool},
    AppResult,
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path, sync::atomic::AtomicBool};

pub fn verify(root: &Path, worker: &mut Worker, cancel: &AtomicBool) -> AppResult<()> {
    for (ext, actions) in [
        (
            "docx",
            vec![
                json!({"kind":"word_paragraph","text":"測試報告😀","format":{"style":"title","font":"Arial","font_east_asia":"微軟正黑體","size":20.0,"bold":true,"alignment":"center"}}),
                json!({"kind":"word_paragraph","text":"測試條件","format":{"style":"heading1","space_after":12.0}}),
                json!({"kind":"word_paragraph","text":"第一項條件","format":{"bullets":true,"line_spacing":1.5}}),
                json!({"kind":"word_table","rows":[["項目","數值"],["速度","12 m/min"]]}),
                json!({"kind":"word_paragraph","before":"p2","text":"插入的摘要","format":{"italic":true}}),
                json!({"kind":"format","target":"p1","format":{"color":"#336699"}}),
                json!({"kind":"word_paragraphs","before":null,"paragraphs":["多段第一段😀","多段第二段"],"format":{"space_after":6.0}}),
                json!({"kind":"word_table_append","table":1,"rows":[["追加列","25"]]}),
            ],
        ),
        (
            "xlsx",
            vec![
                json!({"kind":"excel_write","sheet":1,"cell":"A1","rows":[["項目","數值"],["速度",12],["字面公式","=1+1"]]}),
                json!({"kind":"format","target":"s1:A1:B3","format":{"font":"Arial","size":12.0,"borders":true,"wrap":true,"column_width":24.0}}),
                json!({"kind":"format","target":"s1:A1:B1","format":{"style":"header","alignment":"center"}}),
                json!({"kind":"format","target":"s1:B2","format":{"number_format":"decimal","color":"#FF0000"}}),
                json!({"kind":"excel_sheet","name":"摘要"}),
                json!({"kind":"excel_write","sheet":2,"cell":"A1","rows":[["測試完成"]]}),
                json!({"kind":"excel_append","sheet":2,"rows":[[3,4],[5,6]]}),
            ],
        ),
        (
            "pptx",
            vec![
                json!({"kind":"ppt_slide","layout":"title","title":"測試簡報","body":"副標題"}),
                json!({"kind":"ppt_slide","layout":"content","title":"測試結果","body":"第一點\n第二點"}),
                json!({"kind":"ppt_slide","layout":"two_column","title":"比較","body":"左欄","right":"右欄"}),
                json!({"kind":"format","target":"s2:shape2","format":{"size":20.0,"font":"Arial","bullets":true,"space_after":8.0,"line_spacing":1.2}}),
                json!({"kind":"format","target":"s1:shape1","format":{"size":32.0,"bold":true,"color":"#336699","alignment":"center"}}),
                json!({"kind":"ppt_duplicate","slide":2}),
            ],
        ),
    ] {
        let mut broker = Broker::new(
            Project {
                id: "authoring".into(),
                name: "測試".into(),
                root: root.into(),
                imports: BTreeMap::new(),
            },
            "authoring".into(),
        )?;
        let mut call = |id: &str, tool: Tool| -> AppResult<Value> {
            let reply = broker.execute(id, &tool, worker, cancel)?;
            if reply["ok"] != true {
                return Err(format!("{ext} {id}: {reply}"));
            }
            Ok(reply["result"].clone())
        };
        let created = call(
            "create",
            Tool::CreateWorkingCopy {
                source: None,
                name: format!("新建測試.{ext}"),
            },
        )?;
        let id = created["copy_id"].as_str().unwrap().to_owned();
        let mut revision = created["revision"].as_str().unwrap().to_owned();
        let operations = actions
            .into_iter()
            .map(serde_json::from_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        let result = call(
            "batch",
            Tool::OfficeBatch {
                copy_id: id.clone(),
                revision: revision.clone(),
                operations,
            },
        )?;
        revision = result["revision"].as_str().unwrap().to_owned();
        // 先執行有效格式，再碰到不存在的目標：整批不得改變目前副本。
        let before = call(
            "before_failed_batch",
            Tool::ReadFile {
                path: id.clone(),
                offset: 0,
            },
        )?;
        let invalid = vec![
            json!({"kind":"format","target":if ext=="docx" {"p1"} else if ext=="xlsx" {"s1:A1"} else {"s1:shape1"},"format":{"bold":true}}),
            json!({"kind":"format","target":"does-not-exist","format":{"bold":true}}),
        ];
        assert!(call(
            "failed_batch",
            Tool::OfficeBatch {
                copy_id: id.clone(),
                revision: revision.clone(),
                operations: invalid
                    .into_iter()
                    .map(serde_json::from_value)
                    .collect::<Result<_, _>>()
                    .unwrap()
            }
        )
        .is_err());
        let after = call(
            "after_failed_batch",
            Tool::ReadFile {
                path: id.clone(),
                offset: 0,
            },
        )?;
        assert_eq!(before["revision"], after["revision"], "失敗批次不得提交");
        if ext == "xlsx" {
            let chart = call(
                "chart",
                Tool::ChartFromExcel {
                    path: id.clone(),
                    revision: revision.clone(),
                    sheet: 1,
                    range: "A1:B2".into(),
                    kind: "bar".into(),
                    title: "速度".into(),
                    x_label: "項目".into(),
                    y_label: "m/min".into(),
                },
            )?;
            assert_eq!(chart["chart_index"], 0);
        }
        if ext == "xlsx" {
            let formulas = |revision: &str| Tool::WriteExcelFormulas {
                copy_id: id.clone(),
                revision: revision.into(),
                sheet: 2,
                cell: "C2".into(),
                rows: vec![vec!["=SUM(A2:B2)".into()], vec!["=SUM(A3:B3)".into()]],
                dynamic: false,
            };
            assert!(call("formula_without_skill", formulas(&revision)).is_err());
            call(
                "load_formulas",
                Tool::LoadSkill {
                    id: "excel-formulas".into(),
                },
            )?;
            let calculated = call("formulas", formulas(&revision))?;
            revision = calculated["revision"].as_str().unwrap().to_owned();
            let values = calculated["calculation"]
                .as_array()
                .ok_or("缺少原生公式結果")?;
            for (id, expected) in [("s2:$C$2", "7"), ("s2:$C$3", "11")] {
                let value = values.iter().find(|v| v["id"] == id).ok_or("公式格遺失")?;
                assert_eq!(value["value"], expected);
                assert_eq!(value["error"], false);
            }
            let wrong = call(
                "formula_error",
                Tool::WriteExcelFormulas {
                    copy_id: id.clone(),
                    revision: revision.clone(),
                    sheet: 2,
                    cell: "D2".into(),
                    rows: vec![vec!["=1/0".into()]],
                    dynamic: false,
                },
            )?;
            assert!(wrong["calculation"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v["id"] == "s2:$D$2" && v["error"] == true));
            revision = wrong["revision"].as_str().unwrap().to_owned();
            let fixed = call(
                "formula_fix",
                Tool::WriteExcelFormulas {
                    copy_id: id.clone(),
                    revision: revision.clone(),
                    sheet: 2,
                    cell: "D2".into(),
                    rows: vec![vec!["=IFERROR(1/0,0)".into()]],
                    dynamic: false,
                },
            )?;
            assert!(fixed["calculation"]
                .as_array()
                .unwrap()
                .iter()
                .all(|v| v["error"] == false));
            revision = fixed["revision"].as_str().unwrap().to_owned();
            // 真正走Formula2，核對動態陣列的起始值；發布流程另外保存並重開整份副本。
            let dynamic = call(
                "formula_dynamic_array",
                Tool::WriteExcelFormulas {
                    copy_id: id.clone(),
                    revision: revision.clone(),
                    sheet: 2,
                    cell: "F2".into(),
                    rows: vec![vec!["=SEQUENCE(2,1,10,10)".into()]],
                    dynamic: true,
                },
            )?;
            assert!(dynamic["calculation"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v["id"] == "s2:$F$2" && v["value"] == "10" && v["error"] == false));
            revision = dynamic["revision"].as_str().unwrap().to_owned();
            // 前格合法、後格非法也必須整批不提交，不能留下部分公式。
            assert!(call(
                "formula_partial_rejected",
                Tool::WriteExcelFormulas {
                    copy_id: id.clone(),
                    revision: revision.clone(),
                    sheet: 2,
                    cell: "H2".into(),
                    rows: vec![vec!["=42".into(), "不是公式".into()]],
                    dynamic: false,
                }
            )
            .is_err());
            let unchanged = call(
                "after_formula_rejection",
                Tool::ReadFile {
                    path: id.clone(),
                    offset: 0,
                },
            )?;
            assert_eq!(unchanged["revision"], revision);
        }
        // 驗證陳舊版本被拒絕，拒絕後正確版本仍可發布。
        assert!(call(
            "stale",
            Tool::SaveCopy {
                copy_id: id.clone(),
                revision: "stale".into()
            }
        )
        .is_err());
        let saved = call(
            "save",
            Tool::SaveCopy {
                copy_id: id.clone(),
                revision: revision.clone(),
            },
        )?;
        let repeated = call(
            "save",
            Tool::SaveCopy {
                copy_id: id.clone(),
                revision,
            },
        )?;
        assert_eq!(saved, repeated);
        let paths = broker.finish(&[id])?;
        println!(
            "PASS new {ext}: structures, styles, reopen and dedup: {}",
            paths[0]
        );
    }
    Ok(())
}

#[allow(dead_code)]
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let result = (|| -> AppResult<()> {
        let root = Path::new(args.get(2).ok_or("需要測試資料夾")?);
        std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
        let cancel = AtomicBool::new(false);
        let mut worker = Worker::start(Path::new(args.get(1).ok_or("需要 EXE")?), &cancel)?;
        verify(root, &mut worker, &cancel)
    })();
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
