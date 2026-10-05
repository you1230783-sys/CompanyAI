//! 以使用者提供的檔名／時間格式建立 30 份約 10 MiB LOG，驗證真實 broker 分頁。
use company_ai::{
    projects::{files::Broker, logs::Query, sandbox::Worker, Project, Tool},
    AppResult,
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path, sync::atomic::AtomicBool};

pub fn verify(exe: &Path, root: &Path) -> AppResult<()> {
    let report_path = root
        .parent()
        .ok_or("測試根目錄缺少父層。")?
        .join("log-verification.json");
    let root = root.join("large-logs");
    std::fs::create_dir(&root).map_err(|e| e.to_string())?;
    let mut payload = "2026/06/23, 00:00:00.000 INFO normal ".to_owned();
    payload.push_str(&"x".repeat(990));
    payload.push('\n');
    let padding = payload.repeat(10_240);
    let mut paths = Vec::new();
    for n in 0..30 {
        let name = format!("20260623_connection_A01-{n:02}.log");
        let content=format!("{padding}2026/06/23, 12:25:00.000 timeout Device{n}\n  socket detail\n2026/06/23, 12:33:59.999 retry\n2026/06/23, 12:34:00.000 outside\n");
        std::fs::write(root.join(&name), content).map_err(|e| e.to_string())?;
        paths.push(name);
    }
    let cancel = AtomicBool::new(false);
    let mut worker = Worker::start(exe, &cancel)?;
    let mut broker = Broker::new(
        Project {
            id: "logs".into(),
            name: "LOG".into(),
            root: root.clone(),
            imports: BTreeMap::new(),
        },
        "logs".into(),
    )?;
    let mut n = 0;
    let mut call = |tool: Tool| -> AppResult<Value> {
        n += 1;
        let value = broker.execute(&format!("log_{n}"), &tool, &mut worker, &cancel)?;
        assert_eq!(value["ok"], true, "{value}");
        Ok(value["result"].clone())
    };
    let listed = call(Tool::ListLogs {
        path: "".into(),
        date: Some("2026-06-23".into()),
        category: Some("CONNECTION".into()),
        station: Some("a01-01".into()),
        offset: 0,
    })?;
    assert_eq!(listed["total"], 1);
    let query = Query {
        paths: paths.clone(),
        terms: vec!["TIMEOUT".into(), "retry".into()],
        start_time: Some("12:25".into()),
        end_time: Some("12:33".into()),
        date: Some("2026-06-23".into()),
        context_lines: 1,
        ..Default::default()
    };
    let started = std::time::Instant::now();
    let mut cursor = None;
    let mut found = Vec::new();
    let mut pages = 0;
    loop {
        let result = call(Tool::SearchLogs {
            query: query.clone(),
            cursor,
        })?;
        pages += 1;
        found.extend(result["matches"].as_array().unwrap().iter().cloned());
        assert!(result.to_string().len() < 150_000, "每頁輸出必須有界");
        cursor = result["next_cursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            assert_eq!(result["complete"], true, "{result}");
            break;
        }
        assert!(pages < 30);
    }
    assert_eq!(found.len(), 60);
    assert!(found.iter().all(|r| r["line"].as_u64().unwrap() > 10_240));
    let mut unique = std::collections::BTreeSet::new();
    for found in &found {
        assert!(unique.insert((
            found["path"].as_str().unwrap(),
            found["line"].as_u64().unwrap()
        )));
    }
    let read = call(Tool::ReadLog {
        path: paths[0].clone(),
        revision: None,
        start_line: 1,
        start_column: 0,
        line_count: 1,
    })?;
    assert_eq!(read["lines"][0]["line"], 1);
    let more = call(Tool::ReadLog {
        path: paths[0].clone(),
        revision: read["revision"].as_str().map(str::to_owned),
        start_line: 2,
        start_column: 0,
        line_count: 1,
    })?;
    assert_eq!(more["lines"][0]["line"], 2);
    // 長行跨頁必須完整接回；不遺失 Unicode 字元。
    std::fs::write(root.join("long.log"), "中".repeat(20_000)).map_err(|e| e.to_string())?;
    let first = call(Tool::ReadLog {
        path: "long.log".into(),
        revision: None,
        start_line: 1,
        start_column: 0,
        line_count: 100,
    })?;
    assert_eq!(first["next_line"], 1);
    assert_eq!(first["next_column"], 12000);
    let second = call(Tool::ReadLog {
        path: "long.log".into(),
        revision: first["revision"].as_str().map(str::to_owned),
        start_line: 1,
        start_column: 12000,
        line_count: 100,
    })?;
    assert_eq!(
        second["lines"][0]["text"].as_str().unwrap().chars().count(),
        8000
    );
    assert_eq!(second["eof"], true);
    // 查詢期間的來源變更不可用舊游標續讀。
    let first = call(Tool::SearchLogs {
        query: query.clone(),
        cursor: None,
    })?;
    let cursor = first["next_cursor"].as_str().map(str::to_owned);
    assert!(cursor.is_some());
    std::fs::write(root.join(&paths[0]), "changed").map_err(|e| e.to_string())?;
    let changed = broker.execute(
        "changed",
        &Tool::SearchLogs { query, cursor },
        &mut worker,
        &cancel,
    )?;
    assert_eq!(changed["ok"], false);
    let outside = broker.execute(
        "escape",
        &Tool::ReadLog {
            path: "../outside.log".into(),
            revision: None,
            start_line: 1,
            start_column: 0,
            line_count: 1,
        },
        &mut worker,
        &cancel,
    )?;
    assert_eq!(outside["ok"], false);
    let report = json!({"files":30,"bytes_per_file":padding.len(),"pages":pages,"matches":found.len(),"elapsed_seconds":started.elapsed().as_secs_f64(),"source_revision_rejection":true,"long_line_continuation":true});
    std::fs::write(
        root.join("verification.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(report_path, serde_json::to_vec_pretty(&report).unwrap())
        .map_err(|e| e.to_string())?;
    println!("PASS large LOG: {report}");
    Ok(())
}
