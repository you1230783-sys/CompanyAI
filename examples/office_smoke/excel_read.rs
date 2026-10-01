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
        assert_eq!(header["result"]["used_range"]["last_row"], 1001);
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
        request["start_row"] = json!(1000);
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
            )?["ok"],
            false
        );
        assert_eq!(
            std::fs::read(root.join(&path)).map_err(|e| e.to_string())?,
            original,
            "所有讀取／畫圖不得修改來源"
        );
        let mut changed = original.clone();
        changed.push(0);
        std::fs::write(root.join(&path), changed).map_err(|e| e.to_string())?;
        let stale = call("stale", request)?;
        std::fs::write(root.join(&path), &original).map_err(|e| e.to_string())?;
        assert_eq!(stale["ok"], false, "{stale}");
        assert!(stale["error"].as_str().unwrap().contains("版本已變更"));
        println!("PASS large {ext}: noncontiguous A/F, 100-row pages, headers, native chart values/gaps, version and source protection.");
    }
    Ok(())
}
