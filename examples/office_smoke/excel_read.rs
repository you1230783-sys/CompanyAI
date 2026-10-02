//! 大於 2000 格的真實 Excel 選欄／分頁／圖表驗收，資料只來自專用測試目錄。
use company_ai::{
    projects::{files::Broker, sandbox::Worker, Project, Tool},
    AppResult,
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path, sync::atomic::AtomicBool};

fn tool(value: Value) -> AppResult<Tool> {
    serde_json::from_value(value).map_err(|e| e.to_string())
}
pub fn verify(root: &Path, worker: &mut Worker, cancel: &AtomicBool) -> AppResult<()> {
    for ext in ["xlsx", "xls"] {
        let path = format!("large.{ext}");
        let original = std::fs::read(root.join(&path)).map_err(|e| e.to_string())?;
        let mut broker = Broker::new(
            Project {
                id: format!("large-{ext}"),
                name: "大量 Excel".into(),
                root: root.into(),
                imports: BTreeMap::new(),
            },
            "excel-read".into(),
        )?;
        let mut count = 0;
        let mut call = |value: Value| -> AppResult<Value> {
            count += 1;
            broker.execute(&format!("excel_{count}"), &tool(value)?, worker, cancel)
        };
        let info = call(json!({"tool":"inspect_excel","path":path}))?;
        assert_eq!(info["ok"], true, "{info}");
        assert_eq!(info["result"]["sheet_count"], 2);
        let sheet = info["result"]["sheets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["name"] == "大量資料")
            .unwrap()["sheet"]
            .as_u64()
            .unwrap();
        let shifted = info["result"]["sheets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["name"] == "偏移表頭")
            .unwrap()["sheet"]
            .as_u64()
            .unwrap();
        let header =
            call(json!({"tool":"inspect_excel","path":path,"sheet":sheet,"column_count":3}))?;
        assert_eq!(header["ok"], true, "{header}");
        assert_eq!(header["result"]["next_header_column"], "D");
        assert_eq!(header["result"]["used_range"]["last_row"], 10001);
        let revision = header["result"]["revision"].clone();
        let mut request = json!({"tool":"read_excel_range","path":path,"revision":revision,"sheet":sheet,"columns":["A","F"],"start_row":2});
        let first = call(request.clone())?;
        assert_eq!(first["ok"], true, "{first}");
        assert_eq!(first["result"]["row_count"], 100);
        assert_eq!(first["result"]["data_cells"], 200);
        assert_eq!(first["result"]["next_row"], 102);
        assert_eq!(first["result"]["columns"], json!(["A", "F"]));
        assert_eq!(
            first["result"]["rows"][0]["cells"][1]["value"].as_f64(),
            Some(2.0)
        );
        assert_eq!(first["result"]["rows"][1]["row"], 3);
        assert_eq!(first["result"]["rows"][1]["cells"][1]["kind"], "blank");
        assert!(!first.to_string().contains("不應讀到的中間欄"));
        request["start_row"] = json!(102);
        let second = call(request.clone())?;
        assert_eq!(second["ok"], true, "{second}");
        assert_eq!(second["result"]["rows"][0]["row"], 102);
        assert_eq!(
            second["result"]["rows"][0]["cells"][1]["value"].as_f64(),
            Some(202.0)
        );
        assert_eq!(second["result"]["revision"], revision);
        let chart = call(
            json!({"tool":"chart_excel_range","path":path,"revision":revision,"sheet":sheet,"x_column":"A","y_columns":["F"],"start_row":2,"kind":"line","title":"量測趨勢","x_label":"序號","y_label":"數值"}),
        )?;
        assert_eq!(chart["ok"], true, "{chart}");
        // 原始行序、公式的數值及缺值均由 Excel 取得，沒有模型抄寫。
        let displayed = &broker.charts()[0];
        assert_eq!(displayed.x.len(), 100);
        assert_eq!(
            &displayed.series[0].values[..3],
            &[Some(2.0), None, Some(6.0)]
        );
        assert!(displayed.source.contains("欄 A,F | 列 2–101"));
        let mut call = |id: &str, value: Value| broker.execute(id, &tool(value)?, worker, cancel);
        request["row_count"] = json!(1000);
        request["columns"] = json!(["A", "F", "H"]);
        assert_eq!(call("too_many", request.clone())?["ok"], false);
        request["columns"] = json!(["A", "a"]);
        request["row_count"] = json!(100);
        assert_eq!(call("duplicate", request.clone())?["ok"], false);
        request["columns"] = json!(["A", "F"]);
        request["start_row"] = json!(10000);
        let last = call("last", request.clone())?;
        assert_eq!(last["ok"], true, "{last}");
        assert_eq!(last["result"]["row_count"], 2);
        assert_eq!(last["result"]["next_row"], Value::Null);
        let offset = call(
            "offset",
            json!({"tool":"inspect_excel","path":path,"sheet":shifted,"header_row":5,"start_column":"D","column_count":2}),
        )?;
        assert_eq!(offset["ok"], true, "{offset}");
        assert_eq!(offset["result"]["used_range"]["first_row"], 5);
        assert_eq!(offset["result"]["headers"][0]["cell"]["text"], "標題");
        assert_eq!(offset["result"]["next_header_column"], "F");
        assert_eq!(
            call(
                "escape",
                json!({"tool":"inspect_excel","path":"../large.xlsx"})
            )?["ok"],
            false
        );
        assert_eq!(
            call(
                "private",
                json!({"tool":"inspect_excel","path":".lmai/large.xlsx"})
            )?["ok"],
            false
        );
        assert_eq!(
            call(
                "error_chart",
                json!({"tool":"chart_excel_range","path":path,"revision":revision,"sheet":sheet,"x_column":"A","y_columns":["H"],"start_row":2,"row_count":1,"kind":"line","title":"錯誤值","x_label":"X","y_label":"Y"})
            )?["result"]["waiting_for_user"],
            true
        );
        assert_eq!(
            std::fs::read(root.join(&path)).map_err(|e| e.to_string())?,
            original,
            "所有讀取／畫圖不得修改來源"
        );
        let ten_thousand = call(
            "ten_thousand",
            json!({"tool":"chart_excel_range","path":path,"revision":revision,"sheet":sheet,
            "x_column":"A","y_columns":["F"],"start_row":2,"row_count":10000,"kind":"line","title":"一萬筆趨勢","x_label":"序號","y_label":"量測"}),
        )?;
        assert_eq!(ten_thousand["ok"], true, "{ten_thousand}");
        let over_limit = call(
            "over_chart_limit",
            json!({"tool":"chart_excel_range","path":path,"revision":revision,"sheet":sheet,
            "x_column":"A","y_columns":["F"],"start_row":2,"row_count":10001,"kind":"line","title":"超量","x_label":"X","y_label":"Y"}),
        )?;
        assert_eq!(over_limit["ok"], false);
        let mut changed = original.clone();
        changed.push(0);
        std::fs::write(root.join(&path), changed).map_err(|e| e.to_string())?;
        let stale = call("stale", request)?;
        std::fs::write(root.join(&path), &original).map_err(|e| e.to_string())?;
        assert_eq!(stale["ok"], false, "{stale}");
        assert!(stale["error"].as_str().unwrap().contains("版本已變更"));
        let full_chart = broker.charts().last().unwrap();
        assert_eq!(full_chart.x.len(), 10000);
        assert_eq!(full_chart.series[0].values[1], None);
        assert_eq!(full_chart.series[0].values[9999], Some(20000.0));
        assert_eq!(
            std::fs::read(root.join(&path)).map_err(|e| e.to_string())?,
            original
        );
        use company_ai::projects::charts::quality::Choice;
        use std::time::{Duration, Instant};
        let quality = tool(
            json!({"tool":"chart_excel_range","path":path,"revision":revision,"sheet":sheet,"x_column":"A","y_columns":["H","F"],"start_row":2,"row_count":7,"kind":"line","title":"資料決策","x_label":"X","y_label":"Y"}),
        )?;
        let waiting = broker.execute("quality_gap", &quality, worker, cancel)?;
        assert_eq!(waiting["result"]["waiting_for_user"], true);
        for (id, choice) in [
            ("quality_gap", Choice::Gap),
            ("quality_skip", Choice::Skip),
            ("quality_zero", Choice::Zero),
        ] {
            let selected = choice.clone();
            broker.set_chart_chooser(
                Some(Box::new(move |review, _, _| {
                    assert_eq!(review.converted, 1);
                    for kind in ["Excel 錯誤", "非數字文字", "格式不明的文字", "合併儲存格"]
                    {
                        assert!(
                            review.groups.iter().any(|g| g.category == kind),
                            "{review:?}"
                        );
                    }
                    Ok(Some(vec![selected.clone(); review.groups.len()]))
                })),
                Instant::now() + Duration::from_secs(60),
            );
            let result = broker.execute(id, &quality, worker, cancel)?;
            assert_eq!(result["ok"], true, "{result}");
            assert_eq!(
                broker.execute(id, &quality, worker, cancel)?,
                result,
                "已確認建圖不可重複詢問或追加"
            );
            let chart = broker.charts().last().unwrap();
            assert_eq!(chart.x.len(), 7);
            assert_eq!(chart.series[0].values[1], Some(339.0));
            assert_eq!(chart.series[0].values[3], None, "原空白不因 zero 而改變");
            assert_eq!(chart.series[1].values[2], Some(6.0), "其他系列不刪列");
            assert_eq!(
                chart.series[0].values[2],
                if choice == Choice::Zero {
                    Some(0.0)
                } else {
                    None
                }
            );
            assert_eq!(
                !chart.series[0].skip_indices.is_empty(),
                choice == Choice::Skip
            );
        }
        // 模擬使用者等待期間更新來源：對舊版本的選擇不得套到新內容。
        let target = root.join(&path);
        let mut modified = original.clone();
        modified.push(0);
        broker.set_chart_chooser(
            Some(Box::new(move |review, _, _| {
                std::fs::write(&target, &modified).map_err(|e| e.to_string())?;
                Ok(Some(vec![Choice::Zero; review.groups.len()]))
            })),
            Instant::now() + Duration::from_secs(60),
        );
        let stale_choice = broker.execute("quality_stale", &quality, worker, cancel)?;
        std::fs::write(root.join(&path), &original).map_err(|e| e.to_string())?;
        assert_eq!(stale_choice["ok"], false, "{stale_choice}");
        assert!(stale_choice["error"]
            .as_str()
            .unwrap()
            .contains("版本已變更"));
        assert_eq!(
            std::fs::read(root.join(&path)).map_err(|e| e.to_string())?,
            original
        );
        println!("PASS real {ext} chart quality: numeric text, NG, units, errors, merged, all choices, waiting/replay and stale-source rejection.");
        println!("PASS large {ext}: noncontiguous A/F, 100-row pages, headers, native 10000-point chart values/gaps, version and source protection.");
    }
    Ok(())
}
