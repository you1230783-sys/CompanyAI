//! 原生代理的正式入口整合測試：真實 HTTP、DPAPI 與受限檔案 worker。
//! 每轮故意重用 call_001，確認只按請求範圍去重，不跳過後續合法修改。
use company_ai::{
    config::Config,
    projects::{
        runner::{self, Run},
        Project,
    },
    protocol::{Message, TokenResponse},
    storage::Session,
    AppResult,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    io::Write,
    net::TcpListener,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

pub fn verify(root: &Path) -> AppResult<()> {
    let first = if std::env::args().nth(3).as_deref() == Some("--native-only") {
        std::env::args()
            .nth(4)
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0)
    } else {
        0
    };
    if first > 25 {
        return Err("原生測試案例需介於 0–25。".into());
    }
    // Python 從呼叫端 EXE 旁啟動；先依正式規則授予 AppContainer 唯讀權限。
    company_ai::projects::python::prepare_runtime()?;
    for case in first..=25 {
        verify_case(root, case)?;
    }
    Ok(())
}

pub(super) fn capability(model: &str, strict: bool) -> Value {
    let mut caps: Value = serde_json::from_str(include_str!(
        "../../src/projects/agent/test_capabilities.json"
    ))
    .unwrap();
    caps["model"] = json!(model);
    caps["principal_id"] = json!("fixture_owner");
    caps["strict_tool_arguments"] = json!(strict);
    caps
}
fn fill_optional(args: &mut Value, schema: &Value) {
    if args.is_object() {
        if let Some(props) = schema["properties"].as_object() {
            for (name, child) in props {
                if args.get(name).is_none() {
                    args[name] = Value::Null;
                } else {
                    fill_optional(&mut args[name], child);
                }
            }
        }
    } else if let Some(items) = args.as_array_mut() {
        for item in items {
            fill_optional(item, &schema["items"]);
        }
    }
}

pub(super) fn call(body: &Value, name: &str, mut args: Value) -> Value {
    let tool = body["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["function"]["name"] == name)
        .unwrap();
    // 合成模型遵守當輪公告的筆記契約；案例有指定累積筆記時保留原值。
    if args.get("progress_note").is_none() {
        args["progress_note"] = json!("依測試案例已確認上一輪結果；接著執行本輪指定步驟。");
    }
    fill_optional(&mut args, &tool["function"]["parameters"]);
    json!({"role":"assistant","content":"**處理中**","tool_calls":[{"id":"call_001","type":"function",
        "function":{"name":name,"arguments":args.to_string()}}]})
}
pub(super) fn last_result(body: &Value) -> Value {
    body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .find(|m| m["role"] == "tool")
        .map(|m| serde_json::from_str(m["content"].as_str().unwrap()).unwrap())
        .unwrap_or(Value::Null)
}

/// 0 正常；1 非 strict 格式修復；2 原 POST 回覆遺失＋暫停續接；3 截斷不執行；
/// 4 能力缺少不降級；5 原生快速委派；6 身分錯誤；7 多工具不執行。
/// 8 子請求未知後續接；9 明確拒絕不輪詢；10 取消查回本請求後取消。
fn verify_case(root: &Path, case: usize) -> AppResult<()> {
    let id = format!("native_{case}");
    let workspace = root.join(&id);
    std::fs::create_dir(&workspace).map_err(|e| e.to_string())?;
    // 固定 Excel 快照文字用於驗證協調器等待／續接，實際 Excel COM 另由 Office 測試覆蓋。
    let original = if case == 11 {
        let blocks = [
            ("A1", "text", "X"),
            ("B1", "text", "Y"),
            ("A2", "number", "1"),
            ("B2", "number", "1"),
            ("A3", "number", "2"),
            ("B3", "text", "NG"),
            ("A4", "number", "3"),
            ("B4", "number", "3"),
        ]
        .iter()
        .map(|(id, kind, text)| json!({"id":format!("s1:{id}"),"label":id,"kind":kind,"text":text}))
        .collect::<Vec<_>>();
        json!({"scope":"Excel","blocks":blocks}).to_string()
    } else if case == 21 {
        "原始文字".repeat(3000)
    } else {
        "原始文字".into()
    };
    std::fs::write(workspace.join("source.txt"), &original).map_err(|e| e.to_string())?;
    if case == 21 {
        let log = (1..=100)
            .map(|i| {
                format!(
                    "10:03:00.000 sample x={}; pressure={}; corrected={};\n",
                    i + 7000,
                    i * 2,
                    i * 3
                )
            })
            .collect::<String>();
        std::fs::write(workspace.join("20260623_system_A01-01.log"), log)
            .map_err(|e| e.to_string())?;
    }
    if case == 14 {
        std::fs::write(
            workspace.join("20260623_connection_Z01-CY.log"),
            "2026/06/23, 12:25:48.084 timeout Device02\n2026/06/23, 12:34:00.000 outside\n",
        )
        .map_err(|e| e.to_string())?;
    }
    if matches!(case, 18 | 19) {
        for i in 1..=65 {
            std::fs::write(
                workspace.join(format!("part{i}.txt")),
                format!("第 {i} 份證據：{}", "測試".repeat(1900)),
            )
            .map_err(|e| e.to_string())?;
        }
    }
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let config = Config {
        server_url: format!(
            "http://{}",
            listener.local_addr().map_err(|e| e.to_string())?
        ),
        model: "quality".into(),
        ..Config::default()
    };
    let stopped = Arc::new(AtomicBool::new(false));
    let stop = stopped.clone();
    let resumed = Arc::new(AtomicBool::new(false));
    let resumed_server = resumed.clone();
    let cancel = Arc::new(AtomicBool::new(false));
    let server_cancel = cancel.clone();
    let cancel_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let server_cancel_count = cancel_count.clone();
    let instructions = company_ai::projects::steering::Inbox::open(&root.join("native-app"), &id)?;
    let server_instructions = instructions.clone();
    use std::os::windows::fs::OpenOptionsExt;
    let held = if matches!(case, 16 | 17 | 20) {
        Some(
            std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .share_mode(1)
                .open(workspace.join("source.txt"))
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };
    let held = Arc::new(std::sync::Mutex::new(held));
    let file_prompts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let server = std::thread::spawn(move || -> AppResult<(usize, usize)> {
        let mut posts = 0;
        let mut dataset = Value::Null;
        let mut before_csv_bytes = 0usize;
        let mut old_operation = String::new();
        let mut fast = 0;
        let mut step = 0;
        let mut statuses = BTreeMap::<String, Value>::new();
        let mut copy = String::new();
        let mut repaired = false;
        let mut ids = std::collections::BTreeSet::new();
        while !stop.load(Ordering::Relaxed) {
            let (mut stream, _) = match listener.accept() {
                Ok(v) => v,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(e) => return Err(e.to_string()),
            };
            let (route, body) = super::roundtrip::request(&mut stream)?;
            let mut http = 200;
            let response = if route.contains("/agent/capabilities") {
                let mut cap = capability(
                    if route.ends_with("=fast") {
                        "fast"
                    } else {
                        "quality"
                    },
                    !matches!(case, 1 | 22),
                );
                if case == 4 {
                    cap["native_tool_calls"] = json!(false);
                }
                cap
            } else if route.contains("/capabilities") {
                json!({"contract_version":1,"principal_id":"fixture_owner","execution_modes":["background"],"attachments":{"enabled":false,"max_count":0,"max_file_bytes":0,"max_total_bytes":0,"allowed_extensions":[]}})
            } else if route.ends_with("/conversations") {
                json!({"conversation_id":format!("remote_{}", body["client_conversation_id"].as_str().unwrap())})
            } else if route.ends_with("/models") {
                json!({"models":[{"id":"quality","label":"品質"},{"id":"fast","label":"快速"}],"default_model":"quality"})
            } else if route.ends_with("/agent/turns") {
                posts += 1;
                assert_eq!(body["contract_version"], "desktop-agent-v1");
                assert_eq!(body["skills"], false);
                assert_eq!(body["context"]["context_policy"], "client_snapshot");
                assert_eq!(body["parallel_tool_calls"], false);
                assert!(body.get("attachment_tokens").is_none());
                assert!(ids.insert(body["client_request_id"].as_str().unwrap().to_string()));
                let messages = body["messages"].as_array().unwrap();
                assert_eq!(messages.iter().filter(|m| m["role"] == "system").count(), 1);
                for (i, message) in messages.iter().enumerate() {
                    assert!(message.get("provider_specific_fields").is_none());
                    if let Some(calls) = message["tool_calls"].as_array() {
                        for call in calls {
                            assert!(call.get("provider_specific_fields").is_none());
                            assert!(call["function"].get("provider_specific_fields").is_none());
                        }
                    }
                    if message["role"] == "tool" {
                        assert_eq!(
                            message["tool_call_id"],
                            messages[i - 1]["tool_calls"][0]["id"]
                        );
                    }
                }
                if posts == 1 {
                    assert_eq!(
                        body["tools"].as_array().unwrap().len(),
                        12,
                        "首次提供基本閱讀、按需圖片、技能手冊、工作階段與可選問題"
                    );
                    assert!(body["tools"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|t| t["function"]["name"] == "set_work_stage"));
                    assert!(!body["tools"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|t| t["function"]["name"] == "read_mail_notes"));
                    println!(
                        "Initial native tools: {} bytes, system: {} bytes",
                        body["tools"].to_string().len(),
                        body["messages"][0]["content"].as_str().unwrap().len()
                    );
                }
                let previous = last_result(&body);
                if matches!(case, 18 | 19) && posts == 2 {
                    let state = messages
                        .iter()
                        .filter_map(|m| m["content"].as_str())
                        .find(|t| t.starts_with("本機續接資料"))
                        .unwrap();
                    let state: Value = serde_json::from_str(state.split_once('\n').unwrap().1)
                        .map_err(|e| e.to_string())?;
                    copy = state["recent_operations"][0]["id"]
                        .as_str()
                        .unwrap()
                        .to_owned();
                }
                let mut message = if body["model"] == "fast" {
                    fast += 1;
                    assert_eq!(body["tools"], json!([]));
                    assert_eq!(body["tool_choice"], "none");
                    assert!(body["context"]["parent_request_id"].is_string());
                    let parent = statuses
                        .get(body["context"]["parent_request_id"].as_str().unwrap())
                        .expect("委派父請求已受理");
                    assert_eq!(parent["conversation_id"], body["conversation_id"]);
                    for key in ["project_id", "run_id"] {
                        assert_eq!(parent["context"][key], body["context"][key]);
                    }
                    json!({"role":"assistant","content":"原文為原始文字，無其他數據。"})
                } else if case == 25 {
                    // 使用者整段未回答：詢問後仍讀資料，完成前只通知一次預設並正常交付。
                    match posts {
                        1 => call(
                            &body,
                            "ask_preference",
                            json!({"question":"圖例位置", "options":["右側","下方"], "default_choice":"右側"}),
                        ),
                        2 => {
                            assert_eq!(previous["result"]["state"], "pending");
                            assert_eq!(server_instructions.questions()?[0].state, "pending");
                            call(&body, "read_file", json!({"path":"source.txt"}))
                        }
                        3 => {
                            assert_eq!(previous["ok"], true);
                            assert_eq!(server_instructions.questions()?[0].state, "pending");
                            call(
                                &body,
                                "finish",
                                json!({"message":"資料已讀取", "artifacts":[]}),
                            )
                        }
                        4 => {
                            assert_eq!(server_instructions.questions()?[0].state, "default");
                            assert!(body["messages"].to_string().contains("不是使用者回答"));
                            call(
                                &body,
                                "finish",
                                json!({"message":"已讀取來源，圖例採預設右側", "artifacts":[]}),
                            )
                        }
                        _ => panic!("可選偏好不能重問或無限延後交付"),
                    }
                } else if matches!(case, 22..=24) {
                    // 22 混合 Python 參數／執行失敗，確實到第 10 次；23 其他工具第 5 次。
                    // 24 Python 第 10 次修正成功可交付，不能被 8 次無進展提前攔截。
                    if case == 23 {
                        assert!(posts <= 5, "其他工具應在第 5 次失敗暫停");
                        call(&body, "read_file", json!({"path":"missing.txt","offset":0}))
                    } else if posts == 1 {
                        call(&body, "load_skill", json!({"id":"python-analysis"}))
                    } else if case == 24 && posts == 12 {
                        assert_eq!(
                            previous["ok"], true,
                            "第 10 次 Python 應修正成功：{previous}"
                        );
                        call(
                            &body,
                            "finish",
                            json!({"message":"Python 修正後完成","artifacts":[]}),
                        )
                    } else {
                        assert!(posts <= 11, "Python 應在第 10 次失敗暫停");
                        if posts > 2 {
                            assert_eq!(previous["ok"], false);
                        }
                        let code = if case == 24 && posts == 11 {
                            "result = {'summary': 'done'}"
                        } else {
                            "raise ValueError('budget fixture')"
                        };
                        let mut value = call(
                            &body,
                            "run_python",
                            json!({"purpose":"驗證失敗計數","code":code,"inputs":[]}),
                        );
                        if case == 22 && posts % 2 == 0 {
                            value["tool_calls"][0]["function"]["arguments"] = json!("{}");
                        }
                        value
                    }
                } else if case == 21 {
                    let state_text = messages
                        .iter()
                        .filter_map(|m| m["content"].as_str())
                        .find(|s| s.starts_with("本機續接資料"))
                        .unwrap();
                    let state: Value = serde_json::from_str(state_text.split_once('\n').unwrap().1)
                        .map_err(|e| e.to_string())?;
                    if posts == 3 {
                        before_csv_bytes = body["messages"].to_string().len();
                    }
                    if posts == 4 {
                        let after = body["messages"].to_string().len();
                        assert!(
                            !body["messages"]
                                .to_string()
                                .contains(&"原始文字".repeat(50)),
                            "CSV 後只保留索引與預覽，不得帶回已封存原文"
                        );
                        assert_eq!(messages.iter().filter(|m| m["role"] == "tool").count(), 1);
                        // 軟預算可能在匯出前就已移出全文；CSV 新增索引不要求總長度繼續下降。
                        println!("CSV native request messages: {before_csv_bytes} -> {after} bytes (raw evidence absent; 100 rows retained locally)");
                    }
                    match posts {
                        1 => call(&body, "read_file", json!({"path":"source.txt","offset":0})),
                        2 => {
                            old_operation =
                                state["recent_operations"][0]["id"].as_str().unwrap().into();
                            call(&body, "load_skill", json!({"id":"dataset-charts"}))
                        }
                        3 => call(
                            &body,
                            "export_log_dataset",
                            json!({"query":{"paths":["20260623_system_A01-01.log"],"terms":["sample"],"start_time":"09:58","end_time":"10:08","date":"2026-06-23","context_lines":0},"revisions":[],"fields":[
                            {"name":"x","mode":"between","start":"x=","end":";"},
                            {"name":"pressure","mode":"between","start":"pressure=","end":";"},
                            {"name":"corrected","mode":"between","start":"corrected=","end":";"}],"name":"量測.csv"}),
                        ),
                        4 => {
                            assert_eq!(previous["ok"], true, "{previous}");
                            dataset = previous["result"]["dataset"].clone();
                            assert_eq!(dataset["rows"], 100);
                            assert_eq!(previous["result"]["head"].as_array().unwrap().len(), 10);
                            assert_eq!(previous["result"]["tail"].as_array().unwrap().len(), 10);
                            assert!(
                                !body["messages"].to_string().contains("原始文字"),
                                "CSV 後不重送舊原文"
                            );
                            call(
                                &body,
                                "chart_dataset",
                                json!({"path":dataset["path"],"revision":dataset["revision"],"x_column":"x","y_columns":["pressure"],"start_row":1,"row_count":100,"kind":"scatter","title":"舊圖","x_label":"x","y_label":"y"}),
                            )
                        }
                        5 => {
                            assert_eq!(previous["result"]["chart_index"], 0);
                            assert!(
                                previous["result"].get("x").is_none()
                                    && previous["result"].get("series").is_none()
                            );
                            server_instructions.submit(
                                None,
                                "改用 corrected 欄，舊 pressure 圖無效；保留原目標。",
                            )?;
                            call(
                                &body,
                                "finish",
                                json!({"message":"這個舊候選不得執行","artifacts":[]}),
                            )
                        }
                        6 => {
                            assert!(state["review_required"] == true);
                            assert!(body["messages"].to_string().contains("改用 corrected 欄"));
                            assert!(!body["messages"].to_string().contains("原始文字"));
                            call(
                                &body,
                                "compact_context",
                                json!({"working_note":format!("CSV {} 100筆，欄x/corrected；原始操作 {} 可查回",dataset["path"],old_operation),"superseded":["pressure 欄及舊圖無效"],"next_step":"用 corrected 重畫，不重讀 LOG"}),
                            )
                        }
                        7 => {
                            assert_eq!(state["superseded"][0], "pressure 欄及舊圖無效");
                            assert_eq!(messages.iter().filter(|m| m["role"] == "tool").count(), 1);
                            call(
                                &body,
                                "chart_dataset",
                                json!({"path":dataset["path"],"revision":dataset["revision"],"x_column":"x","y_columns":["corrected"],"start_row":1,"row_count":100,"kind":"scatter","title":"更正圖","x_label":"x","y_label":"y"}),
                            )
                        }
                        8 => {
                            assert_eq!(previous["result"]["chart_index"], 1);
                            call(
                                &body,
                                "read_work_log",
                                json!({"operation_id":old_operation,"offset":0}),
                            )
                        }
                        9 => {
                            assert!(
                                previous.to_string().contains("原始文字"),
                                "整理後原文仍可按需查回"
                            );
                            call(
                                &body,
                                "transform_chart",
                                json!({"chart_index":1,"transform":{
                                "x":{"mode":"index","offset":0,"start":1,"step":1},
                                "y":{"mode":"offset","offset":-1,"start":1,"step":1},"drop_empty":true}}),
                            )
                        }
                        10 => {
                            assert_eq!(previous["ok"], true, "{previous}");
                            assert_eq!(previous["result"]["first_x"], 1.0);
                            assert_eq!(previous["result"]["last_x"], 100.0);
                            assert_eq!(previous["result"]["source_preserved"], true);
                            call(
                                &body,
                                "finish",
                                json!({"message":"CSV 與更正整理完成","artifacts":[]}),
                            )
                        }
                        _ => panic!("不應額外重讀原檔或重跑操作"),
                    }
                } else if matches!(case, 18 | 19) {
                    assert!(
                        body["messages"].to_string().len() < 260_000,
                        "模型上下文應受控"
                    );
                    assert!(messages.iter().any(|m| m["content"]
                        .as_str()
                        .unwrap_or("")
                        .contains("請修訂來源並交付副本")));
                    match posts {
                        1..=65 => call(
                            &body,
                            "read_file",
                            json!({"path":format!("part{posts}.txt"),"progress_note":format!("目標：核對 65 份證據。限制：來源唯讀。已完成 {} 份；待辦：讀下一份及查回第一份。",posts-1)}),
                        ),
                        66 => call(&body, "read_work_log", json!({"offset":0})),
                        67 => {
                            assert_eq!(
                                previous["result"]["operations"].as_array().unwrap().len(),
                                20
                            );
                            assert!(previous["result"]["has_more"] == true);
                            call(
                                &body,
                                "read_work_log",
                                json!({"operation_id":copy,"offset":0}),
                            )
                        }
                        _ => {
                            assert!(previous["result"]["text"]
                                .as_str()
                                .unwrap()
                                .contains("證據"));
                            call(
                                &body,
                                "finish",
                                json!({"message":"跨批次 checkpoint 與查回完成","artifacts":[]}),
                            )
                        }
                    }
                } else if case == 15 {
                    match posts {
                        1 => call(&body, "load_skill", json!({"id":"outlook-research"})),
                        2 => call(&body, "outlook_folders", json!({"scope":"local_inbox"})),
                        _ => {
                            assert_eq!(previous["result"]["declined"], true);
                            call(
                                &body,
                                "finish",
                                json!({"message":"已尊重 Outlook 拒絕，未讀取郵件","artifacts":[]}),
                            )
                        }
                    }
                } else if case == 20 {
                    match posts {
                        1 => call(&body, "load_skill", json!({"id":"text-edit"})),
                        2 => call(
                            &body,
                            "create_working_copy",
                            json!({"source":"source.txt","name":"locked-copy.txt"}),
                        ),
                        3 => {
                            copy = previous["result"]["copy_id"].as_str().unwrap().into();
                            call(
                                &body,
                                "save_copy",
                                json!({"copy_id":copy,"revision":previous["result"]["revision"]}),
                            )
                        }
                        _ => call(
                            &body,
                            "finish",
                            json!({"message":"檔案關閉後已建立副本","artifacts":[copy]}),
                        ),
                    }
                } else if matches!(case, 16 | 17) {
                    match posts {
                        1 => call(&body, "read_file", json!({"path":"source.txt"})),
                        _ => {
                            assert_eq!(previous["result"]["text"], "原始文字");
                            call(
                                &body,
                                "finish",
                                json!({"message":"關閉檔案後已接續讀取","artifacts":[]}),
                            )
                        }
                    }
                } else if case == 14 {
                    match posts {
                        1 => call(&body, "load_skill", json!({"id":"log-analysis"})),
                        2 => call(
                            &body,
                            "list_logs",
                            json!({"path":"","date":"2026-06-23","category":"connection","station":"Z01-CY"}),
                        ),
                        3 => {
                            assert_eq!(previous["result"]["total"], 1);
                            call(
                                &body,
                                "search_logs",
                                json!({"query":{"paths":["20260623_connection_Z01-CY.log"],"terms":["timeout"],"start_time":"12:25","end_time":"12:33","date":"2026-06-23","case_sensitive":null,"context_lines":null},"cursor":null}),
                            )
                        }
                        4 => {
                            assert_eq!(previous["result"]["complete"], true, "{previous}");
                            assert_eq!(previous["result"]["matches"].as_array().unwrap().len(), 1);
                            call(
                                &body,
                                "read_log",
                                json!({"path":"20260623_connection_Z01-CY.log","revision":previous["result"]["matches"][0]["revision"],"start_line":1,"line_count":1}),
                            )
                        }
                        _ => {
                            assert!(previous["result"]["lines"][0]["text"]
                                .as_str()
                                .unwrap()
                                .contains("Device02"));
                            call(
                                &body,
                                "finish",
                                json!({"message":"LOG 原生搜尋完成，來源第 1 行","artifacts":[]}),
                            )
                        }
                    }
                } else if matches!(case, 12 | 13) {
                    match posts {
                        1 => call(&body, "load_skill", json!({"id":"text-edit"})),
                        2 => {
                            server_instructions.submit(None, "只看 Device02，先不要產生報告")?;
                            if case == 12 {
                                call(
                                    &body,
                                    "create_working_copy",
                                    json!({"source":"source.txt","name":"不應建立.txt"}),
                                )
                            } else {
                                call(
                                    &body,
                                    "finish",
                                    json!({"message":"不應提前完成","artifacts":[]}),
                                )
                            }
                        }
                        _ => {
                            assert_eq!(previous["executed"], false, "舊候選操作不得執行");
                            assert!(messages.iter().any(|m| m["role"] == "user"
                                && m["content"]
                                    .as_str()
                                    .unwrap_or("")
                                    .contains("只看 Device02")));
                            assert!(messages.iter().any(|m| m["content"]
                                .as_str()
                                .unwrap_or("")
                                .contains("請修訂來源並交付副本")));
                            call(
                                &body,
                                "finish",
                                json!({"message":"已依補充重新分析，保留原始目標","artifacts":[]}),
                            )
                        }
                    }
                } else if case == 11 {
                    let message = match step {
                        0 => call(&body, "load_skill", json!({"id":"charts"})),
                        1 => call(&body, "read_file", json!({"path":"source.txt","offset":0})),
                        2 => call(
                            &body,
                            "chart_from_excel",
                            json!({"path":"source.txt","revision":previous["result"]["revision"],"sheet":1,"range":"A1:B4","kind":"line","title":"等待決策","x_label":"X","y_label":"Y"}),
                        ),
                        _ => {
                            assert_eq!(previous["ok"], true, "{previous}");
                            call(
                                &body,
                                "finish",
                                json!({"message":"圖表選擇完成","artifacts":[]}),
                            )
                        }
                    };
                    step += 1;
                    message
                } else if matches!(case, 0..=2 | 5 | 8)
                    && !body["tools"].as_array().unwrap().iter().any(|t| {
                        t["function"]["name"]
                            == if matches!(case, 5 | 8) {
                                "summarize_document"
                            } else {
                                "create_working_copy"
                            }
                    })
                {
                    call(
                        &body,
                        "load_skill",
                        json!({"id":if matches!(case,5|8) {"paper-evidence"} else {"text-edit"}}),
                    )
                } else if case == 3 {
                    call(&body, "read_file", json!({"path":"source.txt","offset":0}))
                } else if matches!(case, 5 | 8) {
                    if step == 0 {
                        step += 1;
                        call(
                            &body,
                            "summarize_document",
                            json!({"path":"source.txt","focus":"文字摘要"}),
                        )
                    } else {
                        assert_eq!(previous["ok"], true, "{previous}");
                        call(
                            &body,
                            "finish",
                            json!({"message":"委派完成","artifacts":[]}),
                        )
                    }
                } else if case == 1 && step == 1 && !repaired {
                    repaired = true;
                    let mut m = call(&body, "read_file", json!({"path":"source.txt","offset":0}));
                    m["tool_calls"][0]["function"]["arguments"] = json!("{bad");
                    m
                } else {
                    if case == 1 && step == 1 {
                        assert_eq!(previous["executed"], false);
                    }
                    let message = match step {
                        0 => call(&body, "read_file", json!({"path":"source.txt","offset":0})),
                        1 => call(
                            &body,
                            "create_working_copy",
                            json!({"source":"source.txt","name":"修訂.txt"}),
                        ),
                        2 => {
                            copy = previous["result"]["copy_id"].as_str().unwrap().into();
                            call(
                                &body,
                                "edit_text",
                                json!({"copy_id":copy,"revision":previous["result"]["revision"],"start":0,"expected":"原始","replacement":"修改"}),
                            )
                        }
                        3 => call(
                            &body,
                            "save_copy",
                            json!({"copy_id":copy,"revision":previous["result"]["revision"]}),
                        ),
                        _ => call(
                            &body,
                            "finish",
                            json!({"message":"已完成原生工具修訂","artifacts":[copy]}),
                        ),
                    };
                    step += 1;
                    message
                };
                if case == 7 {
                    let calls = message["tool_calls"].as_array_mut().unwrap();
                    calls.push(calls[0].clone());
                }
                let reason = if case == 3 {
                    "length"
                } else if body["model"] == "fast" {
                    "stop"
                } else {
                    "tool_calls"
                };
                let task_id = format!("task_{posts}");
                let mut completed = json!({"contract_version":"desktop-agent-v1","task_id":task_id,
                    "client_request_id":body["client_request_id"],"conversation_id":body["conversation_id"],"context":body["context"],
                    "state":"completed","result":{"id":format!("completion_{posts}"),"object":"chat.completion","created":1,"model":body["model"],
                    "choices":[{"index":0,"message":message,"finish_reason":reason}]},"error":null,"error_message":""});
                if case == 0 {
                    // 正式 HTTP 往返同時帶上游額外欄位，下一輪只能回送白名單訊息。
                    for pointer in [
                        "",
                        "/context",
                        "/result",
                        "/result/choices/0",
                        "/result/choices/0/message",
                        "/result/choices/0/message/tool_calls/0",
                        "/result/choices/0/message/tool_calls/0/function",
                    ] {
                        completed.pointer_mut(pointer).unwrap()["provider_specific_fields"] =
                            json!({"ignored":true});
                    }
                }
                if case == 10 {
                    completed["state"] = json!("running");
                    completed["result"] = Value::Null;
                    server_cancel.store(true, Ordering::Relaxed);
                }
                statuses.insert(
                    body["client_request_id"].as_str().unwrap().into(),
                    completed.clone(),
                );
                statuses.insert(task_id, completed.clone());
                if (case == 2 && posts == 1) || (case == 8 && body["model"] == "fast" && fast == 1)
                {
                    continue;
                }
                let mut accepted = completed;
                accepted["state"] = json!("queued");
                accepted["result"] = Value::Null;
                if case == 6 {
                    accepted["context"]["run_id"] = json!("another_run");
                }
                http = if case == 9 { 409 } else { 202 };
                if case == 9 {
                    json!({"error_code":"CAPABILITY_CHANGED","message":"設定已變更","task_accepted":false})
                } else {
                    accepted
                }
            } else if route.ends_with("/cancel") {
                server_cancel_count.fetch_add(1, Ordering::Relaxed);
                let task_id = route.split('/').nth_back(1).unwrap();
                let mut status = statuses[task_id].clone();
                status["state"] = json!("cancelled");
                status
            } else if route.contains("/tasks/") {
                assert_ne!(case, 9, "明確拒絕不可當成未知提交持續輪詢");
                if matches!(case, 2 | 8)
                    && !resumed_server.load(Ordering::Relaxed)
                    && (case == 2 || fast > 0)
                {
                    http = 404;
                    json!({"error_code":"TASK_NOT_FOUND","message":"暫未可查"})
                } else {
                    statuses
                        .get(route.rsplit('/').next().unwrap())
                        .cloned()
                        .ok_or("查詢未知 task")?
                }
            } else {
                return Err(format!("原生測試收到不應使用的路由：{route}"));
            };
            let bytes = response.to_string();
            write!(stream,"HTTP/1.1 {http} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{bytes}",bytes.len()).map_err(|e|e.to_string())?;
        }
        Ok((posts, fast))
    });
    let session = Session::from_token(
        TokenResponse {
            access_token: "fixture_token".into(),
            token_type: "Bearer".into(),
            expires_in: 3600,
        },
        &config,
    )?;
    let make_run = |resume| Run {
        id: id.clone(),
        resume,
        project: Project {
            id: id.clone(),
            name: "原生測試".into(),
            root: workspace.clone(),
            imports: BTreeMap::new(),
        },
        conversation: format!("conversation_{id}"),
        messages: vec![Message::user("請修訂來源並交付副本")],
        config: config.clone(),
        session: session.clone(),
        root: root.join("native-app"),
        cancel: cancel.clone(),
        instructions: matches!(case, 12 | 13 | 21 | 25).then(|| instructions.clone()),
        outlook_consent: (case == 15)
            .then(|| Box::new(|_: &AtomicBool, _| Ok(None)) as company_ai::projects::mail::Consent),
        file_waiter: if matches!(case, 16 | 17 | 20) {
            let held = held.clone();
            let prompts = file_prompts.clone();
            Some(Box::new(move |message, _, _| {
                assert!(message.contains("source.txt"));
                let count = prompts.fetch_add(1, Ordering::Relaxed);
                if case == 17 && count == 0 {
                    return Ok(false);
                }
                held.lock().unwrap().take();
                Ok(true)
            }))
        } else {
            None
        },
    };
    let mut activity = vec![];
    let mut transformed_seen = false;
    let mut result = if case == 21 {
        runner::run_with_charts(
            make_run(false),
            |s| activity.push(s),
            |charts| {
                if charts.len() == 2 {
                    assert_eq!(charts[0].series[0].values[49], Some(100.0));
                    assert_eq!(charts[1].series[0].values[49], Some(150.0));
                    assert_eq!(charts[1].x.len(), 100);
                    assert_eq!(charts[1].x[0], 7001.0);
                    if let Some(transform) = &charts[1].transform {
                        let view = transform.view(&charts[1], "scatter").unwrap();
                        assert_eq!(view.x[0], 1.0);
                        assert_eq!(view.series[0].values[49], Some(149.0));
                        transformed_seen = true;
                    }
                }
            },
        )
    } else if case == 19 {
        // 模擬程序在已保存 checkpoint 的界線中斷，跳過 runner 的正常收尾。
        let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            runner::run(make_run(false), |s| {
                if s.contains("已保存階段 checkpoint") {
                    panic!("fixture: simulated interruption");
                }
                activity.push(s);
            })
        }));
        assert!(interrupted.is_err());
        assert!(runner::paused_available(&root.join("native-app"), &id));
        assert!(runner::recover(&root.join("native-app"), &id)?.contains("安全 checkpoint"));
        runner::run(make_run(true), |s| activity.push(s))
    } else {
        runner::run(make_run(false), |s| activity.push(s))
    };
    if case == 17 {
        assert!(result.as_ref().unwrap().contains("等待關閉"), "{result:?}");
        assert!(runner::paused_available(&root.join("native-app"), &id));
        result = runner::run(make_run(true), |s| activity.push(s));
    }
    if matches!(case, 2 | 8) {
        assert!(result.as_ref().unwrap().contains("繼續"));
        assert!(runner::paused_available(&root.join("native-app"), &id));
        resumed.store(true, Ordering::Relaxed);
        result = runner::run(make_run(true), |s| activity.push(s));
    }
    if case == 11 {
        assert!(result.as_ref().unwrap().contains("繼續"), "{result:?}");
        assert!(runner::paused_available(&root.join("native-app"), &id));
        result = runner::run_with_chart_export(
            make_run(true),
            |s| activity.push(s),
            |charts| {
                if let Some(chart) = charts.first() {
                    assert_eq!(
                        chart.series[0].values,
                        vec![Some(1.0), Some(0.0), Some(3.0)]
                    );
                }
            },
            |_| {},
            Box::new(|_, _| Err("未使用 PNG".into())),
            Box::new(|review, _, _| {
                Ok(Some(
                    vec![company_ai::projects::charts::quality::Choice::Zero; review.groups.len()],
                ))
            }),
        );
    }
    stopped.store(true, Ordering::Relaxed);
    let (posts, fast) = server.join().map_err(|_| "原生測試 server 中斷")??;
    match case {
        0..=2 => {
            let answer = result?;
            assert!(answer.contains("已完成原生工具修訂"), "{answer}");
            assert_eq!(posts, if case == 1 { 7 } else { 6 });
            assert_eq!(
                activity
                    .iter()
                    .filter(|s| s.as_str() == "編輯文字：完成")
                    .count(),
                1
            );
            assert_eq!(
                activity
                    .iter()
                    .filter(|s| s.as_str() == "儲存副本：完成")
                    .count(),
                1
            );
        }
        3 => {
            assert!(result?.contains("繼續"));
            assert!(!workspace.join("_AI_Output").exists());
        }
        4 => {
            assert!(result.is_err());
            assert_eq!(posts, 0);
        }
        5 | 8 => {
            assert!(result?.contains("委派完成"));
            assert_eq!(fast, 1);
        }
        6 | 7 | 9 | 10 => {
            assert!(result.is_err());
            assert_eq!(posts, 1);
            assert!(!workspace.join("_AI_Output").exists());
        }
        11 => {
            assert!(result?.contains("圖表選擇完成"));
            assert_eq!(posts, 4, "續接只能查回原工具請求，不重送推論");
        }
        12 | 13 => {
            assert!(result?.contains("已依補充重新分析"));
            assert_eq!(posts, 3);
            assert!(!workspace.join("_AI_Output").exists());
            assert_eq!(instructions.entries()?[0].status, "sent");
            assert!(instructions.submit(None, "最後才送的指示").is_err());
        }
        14 => {
            assert!(result?.contains("LOG 原生搜尋完成"));
            assert_eq!(posts, 5);
        }
        15 => {
            assert!(result?.contains("Outlook 拒絕"));
            assert_eq!(posts, 3);
        }
        16 | 17 => {
            assert!(result?.contains("接續讀取"));
            assert_eq!(posts, 2, "本機讀取等待不能重送原模型請求");
            assert_eq!(
                file_prompts.load(Ordering::Relaxed),
                if case == 17 { 2 } else { 1 }
            );
        }
        20 => {
            assert!(result?.contains("已建立副本"));
            assert_eq!(posts, 4);
            assert_eq!(file_prompts.load(Ordering::Relaxed), 1);
        }
        18 | 19 => {
            assert!(result?.contains("跨批次 checkpoint"));
            assert_eq!(posts, 68, "崩潰續接不能重做已完成的 60 次閱讀或重送 POST");
            assert!(
                !runner::paused_available(&root.join("native-app"), &id),
                "完成後須關閉 checkpoint"
            );
            assert!(activity.iter().any(|s| s.contains("已整理工作筆記")));
        }
        21 => {
            assert!(result?.contains("CSV 與更正整理完成"));
            assert_eq!(posts, 10);
            assert!(transformed_seen, "轉換後的圖表必須透過 UI 事件送出");
            assert!(activity.iter().any(|s| s.contains("CSV 已保存")));
            assert!(activity.iter().any(|s| s.contains("已保存交接筆記")));
        }
        22 | 23 => {
            let answer = result?;
            let limit = if case == 22 { "10 次" } else { "5 次" };
            assert!(answer.contains(limit), "{answer}");
            assert_eq!(posts, if case == 22 { 11 } else { 5 });
            assert!(runner::paused_available(&root.join("native-app"), &id));
        }
        24 => {
            assert!(result?.contains("Python 修正後完成"));
            assert_eq!(posts, 12);
        }
        25 => {
            assert!(result?.contains("圖例採預設右側"));
            assert_eq!(posts, 4);
            assert_eq!(instructions.questions()?[0].state, "default");
            assert!(instructions.questions()?[0].answer.is_none());
            assert!(!runner::paused_available(&root.join("native-app"), &id));
        }
        _ => unreachable!(),
    }
    if case == 10 {
        assert_eq!(cancel_count.load(Ordering::Relaxed), 1);
    }
    assert_eq!(
        std::fs::read_to_string(workspace.join("source.txt")).map_err(|e| e.to_string())?,
        original
    );
    println!(
        "PASS native agent case {case}: {posts} POST, {fast} delegated calls; original preserved."
    );
    Ok(())
}
