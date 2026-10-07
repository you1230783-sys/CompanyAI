//! 專案模型查詢：已完成的空白／壞格式結果不能被誤當斷線而反覆輪詢。
use super::runner::Run;
use crate::{
    jobs::{self, Task},
    AppResult,
};
use std::time::{Duration, Instant};
pub(crate) mod retry;

pub(super) enum Reply {
    /// 網站明確未受理；與提交結果不明分開，子請求可清除待查圖片。
    Rejected(String),
    Text(String),
    Native,
    Invalid {
        reason: String,
        raw: String,
    },
    /// 尚待查回或恢復額度耗盡；保存 Task，由其恢復狀態決定續接方式。
    Pending(String),
}

pub(super) fn receive(
    run: &Run,
    task: &mut Task,
    deadline: Instant,
    lookup_only: bool,
    agent_caps: Option<&super::agent::Capabilities>,
    hooks: (impl FnMut(&Task) -> AppResult<()>, impl FnMut(String)),
) -> AppResult<Reply> {
    let (mut checkpoint, mut notify) = hooks;
    super::runner::check_cancel(&run.cancel)?;
    // 只有一次 receive 完整耗盡後才回 Pending；再次進入代表使用者手動續接。
    // 等待途中退出則保留原次數與到期時間，不因重啟重設。
    if task.project_retry.exhausted {
        task.project_retry.attempts = 0;
        task.project_retry.exhausted = false;
        task.project_retry.due_at_millis = 0;
        task.project_retry.replace_failed = false;
        checkpoint(task)?;
    }
    // 手動續接只 GET 原 request/task ID；即使連續 404，也不能再次 POST。
    let (mut submitted_status, submit_error) =
        if lookup_only || task.project_retry.due_at_millis > 0 {
            (task.remote.clone().filter(|s| s.terminal()), None)
        } else if let Some(caps) = agent_caps {
            match super::agent::submit(&run.config, &run.session, task, caps) {
                super::agent::Submission::Accepted(status) => (Some(status), None),
                super::agent::Submission::Rejected(error) => return Ok(Reply::Rejected(error)),
                super::agent::Submission::Unknown(error) => (None, Some(error)),
            }
        } else {
            let submit = jobs::project_submit(&run.config, &run.session, task);
            (submit.as_ref().ok().cloned(), submit.err())
        };
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
        if task.project_retry.due_at_millis > 0 {
            let waited = retry::wait(run, task, deadline, &mut notify);
            // 等待中的停止也沿用原生取消流程；可能仍在執行的原請求不能被漏掉。
            if run.cancel.load(std::sync::atomic::Ordering::Relaxed) {
                if let Some(caps) = agent_caps {
                    super::agent::cancel(&run.config, &run.session, task, caps);
                }
            }
            if !waited? {
                checkpoint(task)?;
                return Ok(Reply::Pending(super::runner::TIME_LIMIT_REASON.into()));
            }
            let replace = task.project_retry.replace_failed;
            if replace {
                retry::replace(task)?;
            }
            task.project_retry.due_at_millis = 0;
            // 新 ID 必須先保存再 POST；此間當機，下次只查新 ID。
            checkpoint(task)?;
            super::events::register(&run.root, &task.request_id)?;
            submitted_status = if replace {
                if let Some(caps) = agent_caps {
                    match super::agent::submit(&run.config, &run.session, task, caps) {
                        super::agent::Submission::Accepted(status) => Some(status),
                        super::agent::Submission::Rejected(reason) => {
                            return Ok(Reply::Rejected(reason))
                        }
                        super::agent::Submission::Unknown(_) => None,
                    }
                } else {
                    jobs::project_submit(&run.config, &run.session, task).ok()
                }
            } else {
                None
            };
        }
        // POST 有可解析狀態便立即核對；不丟棄錯誤身分再以後續 GET 掩蓋它。
        let received =
            match submitted_status.take() {
                Some(status) => Ok(status),
                None => match agent_caps {
                    Some(caps) => super::agent::status(&run.config, &run.session, task, caps),
                    None => jobs::project_task_status(&run.config, &run.session, task).map_err(
                        |message| super::agent::StatusError {
                            message,
                            retryable: true,
                        },
                    ),
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
                    let reason = format!(
                        "模型任務未完成：{} {}",
                        status.error_message,
                        status
                            .agent_envelope
                            .get("error")
                            .unwrap_or(&serde_json::Value::Null)
                    );
                    if retry::replaceable(task) {
                        let more = retry::schedule(task, true, &reason);
                        checkpoint(task)?;
                        if !more {
                            return Ok(Reply::Pending(format!(
                                "上游推論經五次自動重試仍失敗，已保存進度，可稍後按繼續。{reason}"
                            )));
                        }
                        continue;
                    }
                    return Err(reason);
                }
            }
            Err(error) => {
                if !error.retryable {
                    checkpoint(task)?;
                    return Ok(Reply::Pending(format!(
                        "原請求查詢需要先處理登入、權限或協定問題，已保存進度；未自動重試。{}",
                        error.message
                    )));
                }
                let error = error.message;
                let more = retry::schedule(task, false, &error);
                checkpoint(task)?;
                if !more {
                    return Ok(Reply::Pending(format!(
                        "無法確認原請求 {} 的結果，未另建請求或重播工具。{}最後一次{error}",
                        task.request_id,
                        submit_error
                            .as_ref()
                            .map(|e| format!("最初{e}；"))
                            .unwrap_or_default()
                    )));
                }
                continue;
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
