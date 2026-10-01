//! 使用者主動檢視的本機診斷；只解密已保存紀錄，不重送模型或執行工具。
use super::runner::diagnostic_excerpt;
use crate::{jobs, storage, AppResult};
use serde_json::Value;
use std::{fs::File, io::Read, path::Path};

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
}
