//! 真正 Excel COM → 欄位規劃 → 時間篩選 → CSV → 圖表；不使用模型猜測作為驗收。
use company_ai::{
    projects::{files::Broker, sandbox::Worker, Project, Tool},
    AppResult,
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path, sync::atomic::AtomicBool};

pub fn verify(root: &Path, worker: &mut Worker, cancel: &AtomicBool) -> AppResult<()> {
    for (file, time, mean) in [(1, "B", "D"), (2, "D", "B"), (3, "B", "D")] {
        let path = format!("time{file}.xlsx");
        let original = std::fs::read(root.join(&path)).map_err(|e| e.to_string())?;
        let mut broker = Broker::new(
            Project {
                id: format!("plan-{file}"),
                name: "時間篩選".into(),
                root: root.into(),
                imports: BTreeMap::new(),
            },
            format!("plan-{file}"),
        )?;
        let mut count = 0;
        let mut call = |value: Value| -> AppResult<Value> {
            count += 1;
            let tool: Tool = serde_json::from_value(value).map_err(|e| e.to_string())?;
            broker.execute(&format!("call{count}"), &tool, worker, cancel)
        };
        let info = call(json!({"tool":"inspect_excel","path":path,"sheet":1}))?;
        assert_eq!(info["ok"], true, "{info}");
        let revision = info["result"]["revision"].clone();
        let proposal = json!({"path":path,"revision":revision,"sheet":1,"header_row":1,"purpose":"12:00到20:00選五個時間段各畫一張透光值圖","reason":"圖樣Mean值對應透光值；紀錄時間僅作篩選和X","x":{"column":time,"header":"紀錄時間"},"y":[{"column":mean,"header":"圖樣Mean值"}],"time":{"column":time,"header":"紀錄時間"},"time_mode":"time_of_day","y_kind":"measurement"});
        let mut wrong = proposal.clone();
        wrong["x"] = json!({"column":"A","header":"Index"});
        wrong["y"] = json!([{"column":time,"header":"紀錄時間"}]);
        assert_eq!(
            call(json!({"tool":"plan_excel_analysis","proposal":wrong}))?["ok"],
            false
        );
        let plan = call(json!({"tool":"plan_excel_analysis","proposal":proposal}))?;
        assert_eq!(plan["ok"], true, "{plan}");
        assert!(!plan.to_string().contains("未選欄的私有識別字"));
        for (index, (start, end)) in [
            ("12:00", "13:00"),
            ("13:00", "14:00"),
            ("14:00", "15:00"),
            ("16:00", "17:00"),
            ("19:00", "20:00"),
        ]
        .iter()
        .enumerate()
        {
            let data = call(
                json!({"tool":"export_planned_excel","plan_id":plan["result"]["plan_id"],"start_row":2,"scan_rows":1440,"window":{"start":start,"end":end},"name":format!("時段{index}.csv")}),
            )?;
            assert_eq!(data["ok"], true, "{data}");
            let result = &data["result"];
            assert_eq!(result["dataset"]["rows"], 60, "{result}");
            assert_eq!(result["scan_complete"], true);
            assert!(!result.to_string().contains("未選欄的私有識別字"));
            let mut chart = json!({"tool":"chart_dataset","path":result["dataset"]["path"],"revision":result["dataset"]["revision"],"x_column":time,"y_columns":[time],"start_row":1,"row_count":60,"kind":"line","title":format!("來源{file} {start}–{end}"),"x_label":"紀錄時間","y_label":"透光值"});
            assert_eq!(call(chart.clone())?["ok"], false, "錯誤時間Y不得成圖");
            chart["y_columns"] = json!([mean]);
            let drawn = call(chart)?;
            assert_eq!(drawn["ok"], true, "{drawn}");
        }
        for (chart, hour) in broker.charts().iter().zip([12, 13, 14, 16, 19]) {
            assert_eq!(chart.x.len(), 60);
            // 核對全部900個點，不只核對首尾，才能攔截中途錯列、漏列或排序位移。
            for minute in 0..60 {
                assert_eq!(chart.x[minute], format!("{hour:02}:{minute:02}:00"));
                let expected = 168.4 + (f64::from(hour) * 60.0 + minute as f64) / 100.0;
                assert!((chart.series[0].values[minute].unwrap() - expected).abs() < 1e-9);
            }
            assert_eq!(chart.x[0], format!("{hour:02}:00:00"));
            assert!(
                (chart.series[0].values[0].unwrap() - (168.4 + f64::from(hour) * 0.6)).abs() < 1e-9
            );
            assert!(
                (chart.series[0].values[59].unwrap()
                    - (168.4 + (f64::from(hour) * 60.0 + 59.0) / 100.0))
                    .abs()
                    < 1e-9
            );
            assert_eq!(chart.series[0].name, "圖樣Mean值");
        }
        assert_eq!(
            std::fs::read(root.join(&path)).map_err(|e| e.to_string())?,
            original
        );
    }
    println!("PASS Excel plans: 3 files with distinct column orders x 5 time windows = 15 charts; wrong Y rejected; exact endpoints/values; sources unchanged.");
    Ok(())
}
