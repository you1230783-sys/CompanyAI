//! 專案模型查詢：已完成的空白／壞格式結果不能被誤當斷線而反覆輪詢。
use super::runner::Run;
use crate::{
    jobs::{self, Task},
    AppResult,
};
use std::time::{Duration, Instant};

pub(super) enum Reply {
    Text(String),
    Native,
    Invalid {
        reason: String,
        raw: String,
    },
    /// 原請求尚未確認終態；只保存並查回，禁止重新提交。
    Pending(String),
}

pub(super) fn receive(
    run: &Run,
    task: &mut Task,
    deadline: Instant,
    lookup_only: bool,
    agent_caps: Option<&super::agent::Capabilities>,
) -> AppResult<Reply> {
    super::runner::check_cancel(&run.cancel)?;
    // 手動續接只 GET 原 request/task ID；即使連續 404，也不能再次 POST。
    let (mut submitted_status, submit_error) = if lookup_only {
        (task.remote.clone().filter(|s| s.terminal()), None)
    } else if let Some(caps) = agent_caps {
        match super::agent::submit(&run.config, &run.session, task, caps) {
            super::agent::Submission::Accepted(status) => (Some(status), None),
            super::agent::Submission::Rejected(error) => return Err(error),
            super::agent::Submission::Unknown(error) => (None, Some(error)),
        }
    } else {
        let submit = jobs::project_submit(&run.config, &run.session, task);
        (submit.as_ref().ok().cloned(), submit.err())
    };
    let mut connection_errors = 0;
    loop {
        if run.cancel.load(std::sync::atomic::Ordering::Relaxed) {
            if let Some(caps) = agent_caps {
                super::agent::cancel(&run.config, &run.session, task, caps);
            }
            super::runner::check_cancel(&run.cancel)?;
        }
        if submitted_status.is_none() && Instant::now() >= deadline {
            return Ok(Reply::Pending(super::runner::TIME_LIMIT_REASON.into()));
        }
        // POST 有可解析狀態便立即核對；不丟棄錯誤身分再以後續 GET 掩蓋它。
        let received = match submitted_status.take() {
            Some(status) => Ok(status),
            None => match agent_caps {
                Some(caps) => super::agent::status(&run.config, &run.session, task, caps),
                None => jobs::project_task_status(&run.config, &run.session, task),
            },
        };
        match received {
            Ok(status) => {
                // 身分／未知狀態是契約錯誤，不能當成模型正文錯誤而重新送出工作。
                status.validate_envelope()?;
                if agent_caps.is_some() {
                    super::agent::validate_status(&status, task)?;
                }
                if status.client_request_id != task.request_id
                    || task
                        .remote
                        .as_ref()
                        .is_some_and(|r| r.task_id != status.task_id)
                {
                    return Err("任務回應識別碼不一致，已停止。".into());
                }
                super::events::register(&run.root, &status.task_id)?;
                connection_errors = 0;
                task.remote = Some(status.clone());
                if status.state == "completed" {
                    if let Some(caps) = agent_caps {
                        super::agent::check_result_limits(&status, caps)?;
                        return Ok(Reply::Native);
                    }
                    return Ok(match status.reply_text() {
                        Ok(text) => Reply::Text(text),
                        Err(error) => Reply::Invalid {
                            reason: format!("伺服器已完成，但模型正文無效：{error}"),
                            raw: serde_json::to_string(&status.result)
                                .unwrap_or_default()
                                .replace(&run.session.access_token, "[已隱藏]")
                                .chars()
                                .take(16_000)
                                .collect(),
                        },
                    });
                }
                if status.terminal() {
                    return Err(format!(
                        "模型任務未完成：{} {}",
                        status.error_message,
                        status
                            .agent_envelope
                            .get("error")
                            .unwrap_or(&serde_json::Value::Null)
                    ));
                }
            }
            Err(error) => {
                connection_errors += 1;
                if connection_errors >= 3 {
                    return Ok(Reply::Pending(format!(
                        "無法確認原請求 {} 的結果，未另建請求或重播工具。{}最後一次{error}",
                        task.request_id,
                        submit_error
                            .as_ref()
                            .map(|e| format!("最初{e}；"))
                            .unwrap_or_default()
                    )));
                }
            }
        }
        for _ in 0..10 {
            if run.cancel.load(std::sync::atomic::Ordering::Relaxed) {
                if let Some(caps) = agent_caps {
                    super::agent::cancel(&run.config, &run.session, task, caps);
                }
                super::runner::check_cancel(&run.cancel)?;
            }
            if Instant::now() >= deadline {
                return Ok(Reply::Pending(super::runner::TIME_LIMIT_REASON.into()));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}
