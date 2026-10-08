//! 使用者主動檢視的本機診斷；只解密已保存紀錄，不重送模型或執行工具。
use super::runner::diagnostic_excerpt;
use crate::{jobs, storage, AppResult};
use serde_json::Value;
use std::{fs::File, io::Read, path::Path};

/// 每輪獨立加密保存，避免主 journal 的八輪輪替使長任務診斷消失。
/// 未開 DEBUG 時只保存不含正文的數字統計；切換設定從下一個任務生效。
pub fn save_turn(
    root: &Path,
    run: &str,
    request_id: &str,
    request: &Value,
    response: Option<&Value>,
    debug: bool,
) -> AppResult<()> {
    jobs::validate_id(run)?;
    jobs::validate_id(request_id)?;
    let directory = root.join("project-runs").join(format!("{run}.debug"));
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let estimate = super::context::measure(request);
    let usage = response.map(super::context::actual).unwrap_or(Value::Null);
    let expected = estimate["estimated_input_tokens"].as_f64();
    let actual = usage["prompt_tokens"]
        .as_f64()
        .or_else(|| usage["input_tokens"].as_f64());
    let error = expected
        .zip(actual)
        .filter(|(_, a)| *a > 0.0)
        .map(|(e, a)| (e - a) / a * 100.0);
    let turn = request["context"]["turn_index"].as_u64().unwrap_or(0);
    let metrics =
        serde_json::json!({"turn":turn,"estimate":estimate,"actual":usage,"error_percent":error});
    // 數字與正文使用不同檔案；查看百輪統計時不必解密百份大型請求。
    write(
        &directory.join(format!("{request_id}.usage.dpapi")),
        &metrics,
    )?;
    if debug {
        write(
            &directory.join(format!("{request_id}.dpapi")),
            &serde_json::json!({"turn":turn,"request":request,"response":response}),
        )?;
    }
    Ok(())
}
fn write(path: &Path, value: &Value) -> AppResult<()> {
    storage::atomic_write(
        path,
        &storage::protect(&serde_json::to_vec(value).map_err(|e| e.to_string())?, true)?,
    )
}
fn read_json(path: &Path, limit: u64) -> AppResult<Value> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("診斷檔過大，未載入。".into());
    }
    serde_json::from_slice(&storage::protect(&bytes, false)?)
        .map_err(|_| "診斷紀錄無法解析。".into())
}
/// 每次只載入選定一輪的 JSON；全部用量可獨立複製，不含正文或檔名。
pub fn read_reports(
    root: &Path,
    id: &str,
    conversation: &str,
    debug: bool,
    selected: Option<usize>,
) -> AppResult<String> {
    let legacy = read(root, id, conversation)?;
    let directory = root.join("project-runs").join(format!("{id}.debug"));
    let mut entries = Vec::new();
    if directory.exists() {
        for entry in std::fs::read_dir(&directory).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let Some(request_id) = name.strip_suffix(".usage.dpapi") else {
                continue;
            };
            jobs::validate_id(request_id)?;
            entries.push((request_id.to_owned(), read_json(&entry.path(), 128_000)?));
        }
    }
    entries.sort_by(|a, b| {
        a.1["turn"]
            .as_u64()
            .cmp(&b.1["turn"].as_u64())
            .then(a.0.cmp(&b.0))
    });
    let selected = selected.unwrap_or_else(|| entries.len().saturating_sub(1));
    let trace = if debug && !entries.is_empty() {
        let (request_id, _) = entries.get(selected).ok_or("請重新選擇診斷回合。")?;
        let path = directory.join(format!("{request_id}.dpapi"));
        if path.exists() {
            serde_json::to_string_pretty(&read_json(&path, 24_000_000)?)
                .map_err(|e| e.to_string())?
        } else {
            "這輪未啟用 DEBUG，僅保存用量統計。".into()
        }
    } else {
        legacy
    };
    let metrics:Vec<_>=entries.iter().enumerate().map(|(index,(_,entry))|serde_json::json!({"index":index,"turn":entry["turn"],"estimate":entry["estimate"],"actual":entry["actual"],"error_percent":entry["error_percent"]})).collect();
    let rounds: Vec<_> = entries
        .iter()
        .enumerate()
        .map(|(index, (_, entry))| serde_json::json!({"index":index,"turn":entry["turn"]}))
        .collect();
    serde_json::to_string(&serde_json::json!({"trace":trace,"selected":selected,"rounds":rounds,
        "tokens":serde_json::to_string_pretty(&serde_json::json!({"format":"lmai-usage-v1","contains_content":false,"turns":metrics})).map_err(|e|e.to_string())?})).map_err(|e|e.to_string())
}

/// 一般對話本來就有加密 Task 紀錄，直接投影該筆請求，不另外複製聊天歷史。
pub fn task_reports(task: &jobs::Task, debug: bool) -> AppResult<String> {
    let response = serde_json::to_value(&task.remote).map_err(|e| e.to_string())?;
    let estimate = super::context::measure(&task.request);
    let actual = super::context::actual(&response);
    let error = actual["prompt_tokens"]
        .as_f64()
        .or_else(|| actual["input_tokens"].as_f64())
        .filter(|n| *n > 0.0)
        .map(|n| (estimate["estimated_input_tokens"].as_f64().unwrap_or(0.0) - n) / n * 100.0);
    let trace = if debug {
        serde_json::to_string_pretty(
            &serde_json::json!({"request":task.request,"response":response}),
        )
        .map_err(|e| e.to_string())?
    } else {
        "開啟進階設定中的 DEBUG 可檢視本機已保存的請求與回覆。".into()
    };
    let tokens=serde_json::to_string_pretty(&serde_json::json!({"format":"lmai-usage-v1","contains_content":false,
        "notice":"一般聊天由網站附加歷史，因此桌面只能估算本次送出的內容；API usage 可能包含網站歷史與技能。",
        "turns":[{"index":0,"turn":0,"estimate":estimate,"actual":actual,"error_percent":error}]})).map_err(|e|e.to_string())?;
    serde_json::to_string(&serde_json::json!({"trace":trace,"tokens":tokens,"selected":0,"rounds":[{"index":0,"turn":0}]})).map_err(|e|e.to_string())
}

pub fn read(root: &Path, id: &str, conversation: &str) -> AppResult<String> {
    jobs::validate_id(id)?;
    let file = File::open(root.join("project-runs").join(format!("{id}.dpapi")))
        .map_err(|_| "找不到本機執行紀錄；可能尚未建立或已清除。")?;
    let mut bytes = Vec::new();
    file.take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("本機紀錄過大，未載入。".into());
    }
    let record: Value = serde_json::from_slice(&storage::protect(&bytes, false)?)
        .map_err(|_| "本機紀錄無法解析。")?;
    if record["conversation_id"] != conversation {
        return Err("執行紀錄與目前對話不符。".into());
    }
    Ok(format_record(id, &record))
}

fn format_record(id: &str, record: &Value) -> String {
    let mut output = format!(
        "本機專案執行紀錄\nRun ID: {id}\n狀態：{}\n",
        record["state"]
    );
    let operations = record["operations"].as_array().cloned().unwrap_or_default();
    for (index, entry) in record["requests"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        if output.chars().count() > 200_000 {
            output.push_str("\n[其餘回合未顯示：紀錄過長]\n");
            break;
        }
        output.push_str(&format!(
            "\n── 請求 {} ──\nRequest ID: {}\nTask ID: {}\n模型：{}\n狀態：{}\n",
            index + 1,
            entry["id"],
            entry["task_id"],
            entry["request"]["model"],
            entry["state"]
        ));
        if let Some(error) = entry["error"].as_str().filter(|s| !s.is_empty()) {
            output.push_str(&format!("請求錯誤：{}\n", diagnostic_excerpt(error, 4000)));
        }
        output.push_str("AI 回覆：\n");
        output.push_str(&diagnostic_excerpt(
            entry["response"]
                .as_str()
                .unwrap_or("此回合未保存回覆（可能尚在等待，或由舊版執行）。"),
            32_000,
        ));
        output.push('\n');
        for operation in operations
            .iter()
            .filter(|op| !op["request_id"].is_null() && op["request_id"] == entry["id"])
        {
            append_operation(&mut output, operation);
        }
    }
    // 舊版已有工具操作紀錄，但未逐輪保存 AI 原始回覆；不可假裝能完整還原。
    for operation in operations.iter().filter(|op| op["request_id"].is_null()) {
        if output.chars().count() > 200_000 {
            break;
        }
        append_operation(&mut output, operation);
    }
    output.push_str(&format!(
        "\n最後 AI 回覆：\n{}\n最終結果：\n{}\n尚未確認的操作：\n{}\n",
        diagnostic_excerpt(
            record["last_model_reply"].as_str().unwrap_or("尚無回覆"),
            16_000
        ),
        diagnostic_excerpt(&record["result"].to_string(), 8000),
        diagnostic_excerpt(&record["pending_operation"].to_string(), 8000)
    ));
    diagnostic_excerpt(&output, 240_000)
}

fn append_operation(output: &mut String, operation: &Value) {
    output.push_str(&format!(
        "\n工具操作 ID: {}\n要求：\n{}\n結果：\n{}\n",
        operation["id"],
        diagnostic_excerpt(&operation["request"].to_string(), 12_000),
        diagnostic_excerpt(&operation["result"].to_string(), 12_000)
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reports_model_arguments_and_errors_without_request_prompt() {
        let record = serde_json::json!({"requests":[{"id":"r1","request":{"model":"quality","messages":["hidden prompt"]},"response":"model reply"}],
            "operations":[{"id":"op1","request_id":"r1","request":{"tool":"edit_text","copy_id":"missing"},"result":{"ok":false,"error":"不是本次任務的工作副本。"}}]});
        let text = format_record("run", &record);
        for expected in [
            "model reply",
            "edit_text",
            "missing",
            "不是本次任務的工作副本。",
        ] {
            assert!(text.contains(expected));
        }
        assert!(!text.contains("hidden prompt"));
    }
    #[test]
    fn numeric_report_is_separate_encrypted_and_survives_more_than_eight_turns() {
        let root = std::env::temp_dir().join(format!("lmai-debug-{}", jobs::new_id().unwrap()));
        std::fs::create_dir_all(root.join("project-runs")).unwrap();
        write(
            &root.join("project-runs/run.dpapi"),
            &serde_json::json!({"conversation_id":"chat"}),
        )
        .unwrap();
        for turn in 0..12 {
            let request = serde_json::json!({"context":{"turn_index":turn},"messages":[{"role":"user","content":"PRIVATE_FILENAME.xlsx"}],"tools":[]});
            save_turn(&root,"run",&format!("request{turn}"),&request,Some(&serde_json::json!({"result":{"usage":{"prompt_tokens":10,"completion_tokens":2}}})),true).unwrap();
        }
        let report: Value =
            serde_json::from_str(&read_reports(&root, "run", "chat", true, Some(0)).unwrap())
                .unwrap();
        assert!(report["trace"]
            .as_str()
            .unwrap()
            .contains("PRIVATE_FILENAME"));
        let stats = report["tokens"].as_str().unwrap();
        assert!(!stats.contains("PRIVATE_FILENAME"));
        let stats: Value = serde_json::from_str(stats).unwrap();
        assert_eq!(stats["turns"].as_array().unwrap().len(), 12);
        assert_eq!(stats["turns"][11]["actual"]["prompt_tokens"], 10);
        assert!(read_reports(&root, "run", "other", true, None).is_err());
        let encrypted = std::fs::read(root.join("project-runs/run.debug/request0.dpapi")).unwrap();
        assert!(!String::from_utf8_lossy(&encrypted).contains("PRIVATE_FILENAME"));
    }
}
