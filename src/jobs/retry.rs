//! 聊天與專案共用的推論重連規則：未知結果只查原ID，已確認暫時失敗才重送。
use super::Task;
use crate::AppResult;
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};
pub(crate) const DELAYS: [u64; 5] = [10, 30, 60, 180, 300];

#[cfg(debug_assertions)]
std::thread_local! { static FAST_CLOCK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }

/// 整合測試縮短等待，正式建置沒有這個入口；實際排程另以單元測試核對。
#[cfg(debug_assertions)]
pub(crate) fn with_fast_clock<T>(work: impl FnOnce() -> T) -> T {
    struct Reset(bool);
    impl Drop for Reset {
        fn drop(&mut self) {
            FAST_CLOCK.set(self.0);
        }
    }
    let _reset = Reset(FAST_CLOCK.replace(true));
    work()
}

pub(crate) fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// 舊網站可能把 AI_BACKEND_ERROR 一律標為 retryable=false。
/// 0.8.45 明確限縮為這組上游錯誤；不以錯誤訊息字串或單一 retryable 旗標放行。
pub(crate) fn replaceable(task: &Task) -> bool {
    let Some(status) = task.remote.as_ref() else {
        return false;
    };
    if status.state != "failed" || status.result.as_ref().is_some_and(|v| !v.is_null()) {
        return false;
    }
    let error = status.agent_envelope.get("error").unwrap_or(&Value::Null);
    let code = error["error_code"].as_str().unwrap_or("");
    let details = &error["details"];
    // 已有較明確的永久失敗分類時，不讓外層通用代碼蓋過它。
    if ["status", "status_code", "http_status"].iter().any(|key| {
        details[*key]
            .as_u64()
            .is_some_and(|n| matches!(n, 400 | 401 | 403 | 404 | 413 | 422))
    }) || ["error_code", "code"].iter().any(|key| {
        details[*key].as_str().is_some_and(|s| {
            !s.is_empty()
                && !matches!(
                    s,
                    "AI_BACKEND_ERROR" | "UPSTREAM_UNAVAILABLE" | "UPSTREAM_TIMEOUT"
                )
        })
    }) {
        return false;
    }
    matches!(
        code,
        "AI_BACKEND_ERROR" | "UPSTREAM_UNAVAILABLE" | "UPSTREAM_TIMEOUT"
    )
}

/// 回傳 false 表示五次自動恢復已用完；呼叫端必須保存並暫停。
pub(crate) fn schedule(task: &mut Task, replace: bool, reason: &str) -> bool {
    if task.project_retry.attempts >= DELAYS.len() {
        task.project_retry.exhausted = true;
        task.project_retry.due_at_millis = 0;
        return false;
    }
    let seconds = DELAYS[task.project_retry.attempts];
    let delay = seconds * 1000;
    #[cfg(debug_assertions)]
    let delay = if FAST_CLOCK.get() { 100 } else { delay };
    task.project_retry.attempts += 1;
    task.project_retry.due_at_millis = now_millis().saturating_add(delay);
    task.project_retry.replace_failed = replace;
    task.project_retry
        .history
        .push(json!({"request_id":task.request_id,
        "attempt":task.project_retry.attempts,"delay_seconds":seconds,"lookup_only":!replace,
        "reason":reason.chars().take(800).collect::<String>()}));
    if task.project_retry.history.len() > 10 {
        task.project_retry.history.remove(0);
    }
    true
}

/// 舊終態不可變；建立新請求僅重做這輪推論，messages／tools／parent 維持原樣。
pub(crate) fn replace(task: &mut Task) -> AppResult<()> {
    if !replaceable(task) {
        return Err("原推論不是可重試的已知失敗，未另建請求。".into());
    }
    let id = crate::jobs::new_id()?;
    task.request["client_request_id"] = json!(id);
    if let Some(turn) = task.request["context"]["turn_index"].as_u64() {
        task.request["context"]["turn_index"] = json!(turn.checked_add(1).ok_or("代理序號溢位")?);
    }
    task.request_id = id;
    task.remote = None;
    task.created_at = crate::unix_now();
    task.project_retry.replace_failed = false;
    Ok(())
}

/// 同一次等待只保存一則文字，不把逐秒倒數寫入歷史。
pub(crate) fn notice(task: &Task) -> String {
    format!(
        "AI 連線恢復：第 {}/5 次，{}，等待 {} 秒；可停止等待。",
        task.project_retry.attempts,
        if task.project_retry.replace_failed {
            "準備重試已失敗的推論"
        } else {
            "準備查回原請求"
        },
        task.project_retry
            .due_at_millis
            .saturating_sub(now_millis())
            .div_ceil(1000)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replacement_keeps_one_question_and_replaces_partial_without_replaying() {
        let mut task:Task=serde_json::from_value(json!({"request_id":"original","conversation_id":"c","request":{"client_request_id":"original","messages":[{"role":"user","content":"test"}]},"mode":"stream","title":"t","created_at":0,"remote":null,"applied":false})).unwrap();
        let mut archive = crate::history::Archive::default();
        let mut user = crate::protocol::Message::user("test");
        user.request_id = Some("original".into());
        task.conversation_id = archive.insert(vec![user]).unwrap();
        task.partial = "未完成的回答".into();
        crate::jobs::retain_partial(&mut archive, &task).unwrap();
        task.remote=Some(serde_json::from_value(json!({"contract_version":"desktop-agent-v1","task_id":"failed","client_request_id":"original","state":"failed","error":{"error_code":"UPSTREAM_TIMEOUT"}})).unwrap());
        assert!(schedule(&mut task, true, "timeout"));
        let request = task.request["messages"].clone();
        replace(&mut task).unwrap();
        task.partial.clear();
        task.project_retry.due_at_millis = 0;
        assert_eq!(task.request["messages"], request);
        assert_eq!(task.history_id(), "original");
        let restored: Task = serde_json::from_value(serde_json::to_value(&task).unwrap()).unwrap();
        assert_eq!(restored.history_id(), "original");
        let late = serde_json::from_value(
            json!({"task_id":"failed","client_request_id":"original","state":"running"}),
        )
        .unwrap();
        assert!(task.apply_status(late).is_err());
        task.remote=Some(serde_json::from_value(json!({"task_id":"new","client_request_id":task.request_id,"state":"completed","result":{"choices":[{"message":{"role":"assistant","content":"完成"}}]}})).unwrap());
        assert!(crate::jobs::apply_reply(&mut archive, &task).unwrap());
        assert!(!crate::jobs::apply_reply(&mut archive, &task).unwrap());
        let messages = &archive.conversations[0].messages;
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].content, "完成");
        assert!(!messages[1].incomplete);
        task.stop_tracking();
        assert!(!task.active());
    }
}
