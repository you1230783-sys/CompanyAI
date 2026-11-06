//! 只對已確認的上游失敗重新推論；未知提交始終查原 ID。
//! 排程寫入 Task，先保存再等待／送出，重啟不會遺失等待位置。
use crate::{jobs::Task, AppResult};
#[cfg(test)]
use serde_json::json;
use std::time::{Duration, Instant};

pub(crate) use crate::jobs::retry::*;

use super::super::steering::Inbox;

pub(super) const BATCH_SUGGESTION: &str = "目前 AI 服務可能因使用人數較多而排隊，本次處理未成功。若任務不急，可改為今晚批次處理（約 21:00 後依序執行）。";

/// 保存任務層級額度後才建立等待，避免崩潰或換輪次使計數倒退。
pub(super) fn schedule_project(
    control: &Inbox,
    task: &mut Task,
    replace: bool,
    reason: &str,
) -> AppResult<Option<String>> {
    let (state, seconds) = control.reserve_retry(task.project_retry.attempts)?;
    if let Some(seconds) = seconds {
        set_wait(task, replace, reason, seconds);
        return Ok(None);
    }
    task.project_retry.exhausted = true;
    task.project_retry.due_at_millis = 0;
    let reason = if state.total >= PROJECT_RETRY_LIMIT {
        format!("已自動重試 {PROJECT_RETRY_LIMIT} 次，仍無法連線；可稍後按繼續。")
    } else {
        "已自動重試兩次，仍無法連線，已保存進度。".into()
    };
    Ok(Some(format!("{reason}{BATCH_SUGGESTION}")))
}

#[derive(Debug)]
pub(super) enum Wait {
    Ready,
    Deadline,
    Disabled,
}

pub(super) fn wait(
    cancel: &std::sync::atomic::AtomicBool,
    control: &Inbox,
    task: &Task,
    deadline: Instant,
    notify: &mut impl FnMut(String),
) -> AppResult<Wait> {
    let until = task.project_retry.due_at_millis;
    // 每次等待只記一次；取消與期限仍每100ms檢查，不把逐秒倒數灌進歷史。
    let mut shown = false;
    loop {
        super::super::runner::check_cancel(cancel)?;
        if Instant::now() >= deadline {
            return Ok(Wait::Deadline);
        }
        if task.project_retry.attempts > 2 && !control.retry_state()?.enabled {
            return Ok(Wait::Disabled);
        }
        let remaining = until.saturating_sub(now_millis());
        if remaining == 0 {
            return Ok(Wait::Ready);
        }
        let seconds = remaining.div_ceil(1000);
        if !shown {
            shown = true;
            notify(format!(
                "AI 連線恢復：第 {} 次，{}，剩餘 {seconds} 秒；可停止等待。",
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
    fn control() -> Inbox {
        let root =
            std::env::temp_dir().join(format!("lmai-retry-{}", crate::jobs::new_id().unwrap()));
        Inbox::open(&root, "retry-test").unwrap()
    }
    #[test]
    fn waiting_across_second_boundaries_emits_once_and_remains_cancellable() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let mut task: Task = serde_json::from_value(json!({"request_id":"r","conversation_id":"c",
            "request":{},"mode":"background","title":"t","created_at":0,"remote":null,"applied":false})).unwrap();
        task.project_retry.attempts = 2;
        task.project_retry.due_at_millis = now_millis() + 1100;
        task.project_retry.replace_failed = true;
        let cancel = AtomicBool::new(false);
        let control = control();
        let mut messages = Vec::new();
        assert!(matches!(
            wait(
                &cancel,
                &control,
                &task,
                Instant::now() + Duration::from_secs(5),
                &mut |text| messages.push(text)
            )
            .unwrap(),
            Wait::Ready
        ));
        assert_eq!(messages.len(), 1, "跨秒等待不得重複寫倒數");
        assert!(messages[0].contains("第 2 次"));
        task.project_retry.due_at_millis = now_millis() + 30_000;
        let started = Instant::now();
        let error = wait(
            &cancel,
            &control,
            &task,
            started + Duration::from_secs(5),
            &mut |_| cancel.store(true, Ordering::Relaxed),
        )
        .unwrap_err();
        assert!(error.contains("已停止"));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn project_retry_limit_and_preference_survive_reopening() {
        let root =
            std::env::temp_dir().join(format!("lmai-retry-{}", crate::jobs::new_id().unwrap()));
        let control = Inbox::open(&root, "retry-test").unwrap();
        assert!(!control.retry_state().unwrap().enabled);
        assert_eq!(control.reserve_retry(0).unwrap().1, Some(30));
        assert_eq!(control.reserve_retry(1).unwrap().1, Some(60));
        assert_eq!(control.reserve_retry(2).unwrap().1, None);
        control.set_continuous_retry(true).unwrap();
        assert_eq!(control.reserve_retry(2).unwrap().1, Some(120));
        for attempt in 3..25 {
            assert!((500..=700).contains(&control.reserve_retry(attempt).unwrap().1.unwrap()));
        }
        drop(control);
        let restored = Inbox::open(&root, "retry-test").unwrap();
        assert_eq!(restored.retry_state().unwrap().total, 25);
        assert!(restored.retry_state().unwrap().enabled);
        // 新推論輪次與手動續接都從單輪第 0 次開始，但本任務的上限保留。
        for _ in 25..PROJECT_RETRY_LIMIT {
            assert_eq!(restored.reserve_retry(0).unwrap().1, Some(30));
        }
        assert_eq!(restored.reserve_retry(0).unwrap().1, None);
        restored.set_continuous_retry(false).unwrap();
        assert_eq!(restored.reserve_retry(0).unwrap().1, None);
    }

    #[test]
    fn switching_retry_during_wait_only_interrupts_the_extended_waits() {
        use std::sync::atomic::AtomicBool;
        let control = control();
        let cancel = AtomicBool::new(false);
        let mut task: Task = serde_json::from_value(json!({"request_id":"r","conversation_id":"c",
            "request":{},"mode":"background","title":"t","created_at":0,"remote":null,"applied":false})).unwrap();
        for (attempt, enabled, interrupted) in
            [(2, false, false), (3, false, true), (3, true, false)]
        {
            control.set_continuous_retry(enabled).unwrap();
            task.project_retry.attempts = attempt;
            task.project_retry.due_at_millis = now_millis() + 50;
            let result = wait(
                &cancel,
                &control,
                &task,
                Instant::now() + Duration::from_secs(2),
                &mut |_| {},
            )
            .unwrap();
            assert_eq!(matches!(result, Wait::Disabled), interrupted);
        }
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
