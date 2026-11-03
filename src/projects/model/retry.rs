//! 只對已確認的上游失敗重新推論；未知提交始終查原 ID。
//! 排程寫入 Task，先保存再等待／送出，重啟不會遺失等待位置。
use crate::{jobs::Task, AppResult};
#[cfg(test)]
use serde_json::json;
use std::time::{Duration, Instant};

pub(crate) use crate::jobs::retry::*;

pub(super) fn wait(
    cancel: &std::sync::atomic::AtomicBool,
    task: &Task,
    deadline: Instant,
    notify: &mut impl FnMut(String),
) -> AppResult<bool> {
    let until = task
        .project_retry
        .due_at_millis
        .min(now_millis().saturating_add(300_000));
    // 每次等待只記一次；取消與期限仍每100ms檢查，不把逐秒倒數灌進歷史。
    let mut shown = false;
    loop {
        super::super::runner::check_cancel(cancel)?;
        if Instant::now() >= deadline {
            return Ok(false);
        }
        let remaining = until.saturating_sub(now_millis());
        if remaining == 0 {
            return Ok(true);
        }
        let seconds = remaining.div_ceil(1000);
        if !shown {
            shown = true;
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn waiting_across_second_boundaries_emits_once_and_remains_cancellable() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let mut task: Task = serde_json::from_value(json!({"request_id":"r","conversation_id":"c",
            "request":{},"mode":"background","title":"t","created_at":0,"remote":null,"applied":false})).unwrap();
        task.project_retry.attempts = 2;
        task.project_retry.due_at_millis = now_millis() + 1100;
        task.project_retry.replace_failed = true;
        let cancel = AtomicBool::new(false);
        let mut messages = Vec::new();
        assert!(wait(
            &cancel,
            &task,
            Instant::now() + Duration::from_secs(5),
            &mut |text| messages.push(text)
        )
        .unwrap());
        assert_eq!(messages.len(), 1, "跨秒等待不得重複寫倒數");
        assert!(messages[0].contains("第 2/5 次"));
        task.project_retry.due_at_millis = now_millis() + 30_000;
        let started = Instant::now();
        let error = wait(
            &cancel,
            &task,
            started + Duration::from_secs(5),
            &mut |_| cancel.store(true, Ordering::Relaxed),
        )
        .unwrap_err();
        assert!(error.contains("已停止"));
        assert!(started.elapsed() < Duration::from_secs(1));
    }
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
