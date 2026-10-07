//! 只對已確認的上游失敗重新推論；未知提交始終查原 ID。
//! 排程寫入 Task，先保存再等待／送出，重啟不會遺失等待位置。
use crate::{jobs::Task, AppResult};
use serde_json::{json, Value};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub(super) const DELAYS: [u64; 5] = [10, 30, 60, 180, 300];

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

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// 舊網站可能把 AI_BACKEND_ERROR 一律標為 retryable=false。
/// 0.8.45 明確限縮為這組上游錯誤；不以錯誤訊息字串或單一 retryable 旗標放行。
pub(super) fn replaceable(task: &Task) -> bool {
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
pub(super) fn schedule(task: &mut Task, replace: bool, reason: &str) -> bool {
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

pub(super) fn wait(
    run: &super::Run,
    task: &Task,
    deadline: Instant,
    notify: &mut impl FnMut(String),
) -> AppResult<bool> {
    let until = task
        .project_retry
        .due_at_millis
        .min(now_millis().saturating_add(300_000));
    let mut shown = u64::MAX;
    loop {
        super::super::runner::check_cancel(&run.cancel)?;
        if Instant::now() >= deadline {
            return Ok(false);
        }
        let remaining = until.saturating_sub(now_millis());
        if remaining == 0 {
            return Ok(true);
        }
        let seconds = remaining.div_ceil(1000);
        if shown != seconds {
            shown = seconds;
            notify(format!(
                "AI 連線恢復：第 {}/5 次，{}，剩餘 {seconds} 秒；可停止等待。",
                task.project_retry.attempts,
                if task.project_retry.replace_failed {
                    "準備重試已失敗的推論"
                } else {
                    "準備查回原請求"
                }
            ));
        }
        std::thread::sleep(Duration::from_millis(remaining.min(100)));
    }
}

/// 舊終態不可變；建立新請求僅重做這輪推論，messages／tools／parent 維持原樣。
pub(super) fn replace(task: &mut Task) -> AppResult<()> {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retry_schedule_is_bounded_and_unknown_or_permanent_failures_never_replace() {
        assert_eq!(DELAYS, [10, 30, 60, 180, 300]);
        let mut task: Task = serde_json::from_value(json!({"request_id":"r","conversation_id":"c",
            "request":{"client_request_id":"r","context":{"turn_index":3}},"mode":"background",
            "title":"t","created_at":0,"remote":null,"applied":false}))
        .unwrap();
        assert!(!replaceable(&task));
        let status = json!({"contract_version":"desktop-agent-v1","task_id":"t","client_request_id":"r","state":"failed","result":null,
            "error":{"error_code":"AI_BACKEND_ERROR","retryable":false,"details":{} }});
        task.remote = Some(serde_json::from_value(status).unwrap());
        assert!(replaceable(&task));
        for attempt in 1..=5 {
            assert!(schedule(&mut task, true, "暫時故障"));
            assert_eq!(task.project_retry.attempts, attempt);
        }
        assert!(!schedule(&mut task, true, "仍失敗"));
        let encoded = serde_json::to_value(&task).unwrap();
        let restored: Task = serde_json::from_value(encoded).unwrap();
        assert!(restored.project_retry.exhausted);
        for code in [
            "UPSTREAM_RESULT_UNKNOWN",
            "UPSTREAM_SCHEMA_VIOLATION",
            "UNAUTHORIZED",
            "QUOTA_EXCEEDED",
        ] {
            task.remote
                .as_mut()
                .unwrap()
                .agent_envelope
                .get_mut("error")
                .unwrap()["error_code"] = json!(code);
            assert!(!replaceable(&task), "{code}");
        }
        task.remote.as_mut().unwrap().agent_envelope.insert(
            "error".into(),
            json!({"error_code":"AI_BACKEND_ERROR","details":{"status_code":401}}),
        );
        assert!(!replaceable(&task));
        task.remote
            .as_mut()
            .unwrap()
            .agent_envelope
            .get_mut("error")
            .unwrap()["details"] = json!({});
        replace(&mut task).unwrap();
        assert_ne!(task.request_id, "r");
        assert_eq!(task.request["context"]["turn_index"], 4);
        assert!(task.remote.is_none());
    }
}
