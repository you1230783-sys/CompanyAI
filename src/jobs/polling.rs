//! 一般對話的查詢錯誤與停止條件。HTTP 狀態直接取自回應，避免從顯示文字猜錯誤碼。
use super::*;

pub(crate) const REPEATED_ERROR_LIMIT: u8 = 3;

#[derive(Clone, Debug)]
pub(crate) struct PollFailure {
    pub http_status: Option<u32>,
    pub message: String,
}

impl From<String> for PollFailure {
    fn from(message: String) -> Self {
        Self {
            http_status: None,
            message,
        }
    }
}

impl PollFailure {
    /// 同一 HTTP 狀態算同類失敗；後端每次產生不同診斷 ID 不應重設次數。
    /// 未收到 HTTP 回應或內容無效時，才比對本機產生的錯誤訊息。
    fn same_status(&self, other: &Self) -> bool {
        match (self.http_status, other.http_status) {
            (Some(left), Some(right)) => left == right,
            (None, None) => self.message == other.message,
            _ => false,
        }
    }
}

/// 計數只保留於目前登入工作階段；停止結果另由原有 DPAPI 任務檔保存。
#[derive(Clone, Default)]
pub(crate) struct PollFailures {
    last: Option<PollFailure>,
    count: u8,
}

impl PollFailures {
    pub fn record(&mut self, failure: &PollFailure) {
        self.count = if self
            .last
            .as_ref()
            .is_some_and(|last| last.same_status(failure))
        {
            self.count.saturating_add(1).min(REPEATED_ERROR_LIMIT)
        } else {
            1
        };
        self.last = Some(failure.clone());
    }

    pub fn needs_service_check(&self, failure: &PollFailure) -> bool {
        let mut next = self.clone();
        next.record(failure);
        next.count >= REPEATED_ERROR_LIMIT
    }

    pub fn should_stop(&self, models_healthy: bool) -> bool {
        self.count >= REPEATED_ERROR_LIMIT && models_healthy
    }
}

/// 只查原請求，不重新提交。正常終態（failed／cancelled）仍交原流程立即收尾。
pub(crate) fn task_status(
    config: &Config,
    session: &Session,
    task: &Task,
) -> Result<TaskStatus, PollFailure> {
    if !session.valid_for(config) {
        return Err("登入已到期，重新登入後可接續工作。".to_string().into());
    }
    let path = match &task.remote {
        Some(remote) => format!("{PREFIX}/tasks/{}", remote.task_id),
        None => format!("{PREFIX}/tasks/by-request/{}", task.request_id),
    };
    let response = transport::get(
        &config.endpoint(&path)?,
        Some(("Authorization", &format!("Bearer {}", session.access_token))),
    )?;
    decode_status(response, session, task)
}

fn decode_status(
    response: transport::HttpResponse,
    session: &Session,
    task: &Task,
) -> Result<TaskStatus, PollFailure> {
    if !(200..300).contains(&response.status) {
        return Err(PollFailure {
            http_status: Some(response.status),
            message: protocol::api_error(response.status, &response.body, &session.access_token),
        });
    }
    let status: TaskStatus = decode(response, session)?;
    task.validate_status(&status)?;
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn http(status: u32, message: &str) -> PollFailure {
        PollFailure {
            http_status: Some(status),
            message: message.into(),
        }
    }

    #[test]
    fn three_same_http_errors_require_a_healthy_service() {
        let mut failures = PollFailures::default();
        for message in ["request A", "request B"] {
            let failure = http(404, message);
            assert!(!failures.needs_service_check(&failure));
            failures.record(&failure);
            assert!(!failures.should_stop(true));
        }
        let third = http(404, "request C");
        assert!(failures.needs_service_check(&third));
        failures.record(&third);
        assert!(!failures.should_stop(false));
        assert!(failures.should_stop(true));
        // 健康檢查失敗後保留上限；後續健康恢復仍可停止，不會溢位。
        for _ in 0..300 {
            failures.record(&third);
        }
        assert!(failures.should_stop(true));
    }

    #[test]
    fn changed_errors_and_success_reset_consecutive_failures() {
        let mut failures = PollFailures::default();
        for status in [404, 500, 404, 500] {
            failures.record(&http(status, "error"));
            assert!(!failures.should_stop(true));
        }
        failures.record(&http(500, "another diagnostic ID"));
        assert!(failures.needs_service_check(&http(500, "error")));
        // 控制器在成功、改用遠端 task ID 或重新送出時移除該任務的計數。
        failures = PollFailures::default();
        assert!(!failures.needs_service_check(&http(500, "error")));
        for _ in 0..3 {
            failures.record(&"網路逾時".to_string().into());
        }
        assert!(failures.should_stop(true));
        failures.record(&"回應 JSON 無效".to_string().into());
        assert!(!failures.should_stop(true));
    }

    fn task() -> Task {
        serde_json::from_value(json!({
            "request_id": "request1", "conversation_id": "local1",
            "request": {}, "mode": "stream", "title": "查詢測試", "created_at": 0,
            "remote": null, "applied": false
        }))
        .unwrap()
    }

    #[test]
    fn response_validation_preserves_http_status_and_rejects_foreign_tasks() {
        let session = Session {
            access_token: "test-secret".into(),
            expires_at: 0,
            binding: String::new(),
        };
        let response = |status, body: &str| transport::HttpResponse {
            status,
            body: body.into(),
        };
        let failure = decode_status(
            response(404, r#"{"error":{"message":"missing test-secret"}}"#),
            &session,
            &task(),
        )
        .err()
        .unwrap();
        assert_eq!(failure.http_status, Some(404));
        assert!(!failure.message.contains("test-secret"));

        let wrong = decode_status(
            response(
                200,
                r#"{"task_id":"remote1","client_request_id":"other","state":"running"}"#,
            ),
            &session,
            &task(),
        )
        .err()
        .unwrap();
        assert_eq!(wrong.http_status, None);
        assert!(wrong.message.contains("識別碼不一致"));
        assert!(decode_status(response(200, "<html>login</html>"), &session, &task()).is_err());

        let failed = decode_status(
            response(200, r#"{"task_id":"remote1","client_request_id":"request1","state":"failed","error_message":"conversion failed"}"#),
            &session,
            &task(),
        )
        .unwrap();
        assert!(failed.terminal());
        assert_eq!(failed.error_message, "conversion failed");
    }

    #[test]
    fn stopped_task_preserves_partial_history_and_ignores_late_status_after_reload() {
        let mut task = task();
        let mut archive = crate::history::Archive::default();
        let mut user = Message::user("請分析附件");
        user.request_id = Some(task.request_id.clone());
        task.conversation_id = archive.insert(vec![user]).unwrap();
        task.partial = "已收到的回答片段".into();
        retain_partial(&mut archive, &task).unwrap();
        task.stop_tracking();
        let mut restored: Task =
            serde_json::from_value(serde_json::to_value(task).unwrap()).unwrap();
        let late: TaskStatus = serde_json::from_value(json!({
            "task_id":"remote1", "client_request_id":"request1", "state":"running"
        }))
        .unwrap();
        restored.apply_status(late).unwrap();
        assert!(!restored.active());
        assert!(restored.remote.is_none());
        assert_eq!(
            archive.conversations[0].messages[1].content,
            "已收到的回答片段"
        );
        assert!(archive.conversations[0].messages[1].incomplete);
    }

    #[test]
    fn polling_waits_for_submission_but_recovers_unknown_requests() {
        let mut task = task();
        assert!(!task.can_poll(true));
        assert!(task.can_poll(false)); // 回應遺失／重新開啟後，只查原 ID。
        let status = serde_json::from_value(json!({
            "task_id":"remote1", "client_request_id":"request1", "state":"running"
        }))
        .unwrap();
        task.apply_status(status).unwrap();
        assert!(task.can_poll(true)); // SSE 已回任務 ID，可與串流並行查詢。
        task.stop_tracking();
        assert!(!task.can_poll(true));
        assert!(!task.can_poll(false));
    }
}
