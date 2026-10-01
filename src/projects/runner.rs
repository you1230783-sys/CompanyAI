//! 專案代理往返。每輪模型請求有獨立 ID；工具只在本次活躍任務內執行。
//! 中斷紀錄保留供檢查，不在重新登入或重啟後自動重播寫入。
use super::{files::Broker, sandbox::Worker, Decision, Project};
use crate::{
    config::Config,
    jobs::{self, Task},
    protocol::Message,
    storage::{self, Session},
    AppResult,
};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

pub struct Run {
    pub id: String,
    pub resume: bool,
    pub project: Project,
    pub conversation: String,
    pub messages: Vec<Message>,
    pub config: Config,
    pub session: Session,
    pub root: PathBuf,
    pub cancel: Arc<AtomicBool>,
}

const MAX_TOOLS: usize = 60;
const MAX_REPLIES: usize = 80;
fn quota_reached(replies: usize, tools: usize) -> bool {
    replies >= MAX_REPLIES || tools >= MAX_TOOLS
}

const SEGMENT_BUDGET: Duration = Duration::from_secs(2 * 60 * 60);
pub(super) const TIME_LIMIT_REASON: &str = "本段專案執行已達 2 小時";
#[derive(serde::Serialize, serde::Deserialize)]
struct PausedRun {
    version: u32,
    available: bool,
    project: String,
    project_root: PathBuf,
    conversation: String,
    principal: String,
    request_text: String,
    broker: super::files::SavedBroker,
    progress: super::progress::Progress,
    /// 0.8.26 暫停檔沒有此欄位；舊版只會在工具全部完成後暫停。
    #[serde(default)]
    pending_model: Option<Task>,
}
fn pause_path(root: &Path, id: &str) -> AppResult<PathBuf> {
    jobs::validate_id(id)?;
    Ok(root.join("project-runs").join(format!("{id}.resume.dpapi")))
}
fn load_pause(root: &Path, id: &str) -> AppResult<PausedRun> {
    let path = pause_path(root, id)?;
    if std::fs::metadata(&path)
        .map_err(|_| "找不到暫停紀錄。")?
        .len()
        > 64_000_000
    {
        return Err("暫停紀錄過大。".into());
    }
    let bytes = storage::protect(&std::fs::read(path).map_err(|e| e.to_string())?, false)?;
    serde_json::from_slice(&bytes).map_err(|_| "暫停紀錄無法解析。".into())
}
/// 按鈕依程式建立的暫存紀錄顯示，不相信模型自行聲稱已暫停。
pub fn paused_available(root: &Path, id: &str) -> bool {
    load_pause(root, id).is_ok_and(|s| s.version == 1 && s.available)
}
fn save_pause(
    run: &Run,
    principal: &str,
    request: &str,
    broker: &Broker,
    progress: &super::progress::Progress,
    pending_model: Option<&Task>,
) -> AppResult<()> {
    let state = json!({"version":1,"available":true,"project":run.project.id,"project_root":run.project.root,
        "conversation":run.conversation,"principal":principal,"request_text":request,
        "broker":broker.saved()?,"progress":progress,"pending_model":pending_model});
    checkpoint(&pause_path(&run.root, &run.id)?, &state)
}

fn checkpoint(path: &Path, value: &Value) -> AppResult<()> {
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    if bytes.len() > 60_000_000 {
        return Err("專案暫存紀錄超過 60 MB，未宣稱已暫存。".into());
    }
    storage::atomic_write(path, &storage::protect(&bytes, true)?)
}

pub(super) fn diagnostic_excerpt(text: &str, limit: usize) -> String {
    let mut chars = text.chars();
    let mut result: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        result.push_str("\n[內容過長，已截斷顯示]");
    }
    result
}

pub fn run(run: Run, progress: impl FnMut(String)) -> AppResult<String> {
    run_for(run, progress, SEGMENT_BUDGET, |_| {}, false, None)
}

/// 圖表為結構化 UI 事件，不混入模型文字或一般進度字串。
pub fn run_with_charts(
    run: Run,
    progress: impl FnMut(String),
    charts: impl FnMut(Vec<super::charts::Chart>),
) -> AppResult<String> {
    run_for(run, progress, SEGMENT_BUDGET, charts, false, None)
}

/// 正式桌面提供固定 PNG 繪製服務；非 UI 測試入口不虛構成功匯出。
pub fn run_with_chart_export(
    run: Run,
    progress: impl FnMut(String),
    charts: impl FnMut(Vec<super::charts::Chart>),
    renderer: super::charts::png::Renderer,
) -> AppResult<String> {
    run_for(run, progress, SEGMENT_BUDGET, charts, false, Some(renderer))
}

/// 僅供 debug 整合測試推進期限，不改正式 EXE 的兩小時政策。
#[cfg(debug_assertions)]
pub fn run_with_test_budget(
    run: Run,
    progress: impl FnMut(String),
    budget: Duration,
) -> AppResult<String> {
    run_for(run, progress, budget, |_| {}, false, None)
}

/// 舊文字協定的回歸測試入口；正式 EXE 不編入，不能用它降級新任務。
#[cfg(debug_assertions)]
pub fn run_legacy_test(run: Run, progress: impl FnMut(String)) -> AppResult<String> {
    run_for(run, progress, SEGMENT_BUDGET, |_| {}, true, None)
}
#[cfg(debug_assertions)]
pub fn run_legacy_with_test_budget(
    run: Run,
    progress: impl FnMut(String),
    budget: Duration,
) -> AppResult<String> {
    run_for(run, progress, budget, |_| {}, true, None)
}
#[cfg(debug_assertions)]
pub fn run_legacy_with_charts(
    run: Run,
    progress: impl FnMut(String),
    charts: impl FnMut(Vec<super::charts::Chart>),
) -> AppResult<String> {
    run_for(run, progress, SEGMENT_BUDGET, charts, true, None)
}

fn run_for(
    mut run: Run,
    mut progress: impl FnMut(String),
    budget: Duration,
    mut charts: impl FnMut(Vec<super::charts::Chart>),
    legacy_test: bool,
    png_renderer: Option<super::charts::png::Renderer>,
) -> AppResult<String> {
    let mut activity = Vec::new();
    let journal = run
        .root
        .join("project-runs")
        .join(format!("{}.dpapi", run.id));
    let mut record = json!({"project_id":run.project.id,"conversation_id":run.conversation,"state":"starting","requests":[],"operations":[],"outputs":[]});
    checkpoint(&journal, &record)?;
    let mut broker = Broker::new(run.project.clone(), run.id.clone())?;
    if let Some(renderer) = png_renderer {
        broker.set_png_renderer(renderer);
    }
    broker.enable_server_pdf(run.config.clone(), run.session.clone())?;
    broker.enable_memory(&run.conversation)?;
    let mut request_text = run
        .messages
        .last()
        .map(|m| m.content.clone())
        .unwrap_or_default();
    let mut task_summary = None;
    // 在建立模型請求前保存開始狀態；中途停止也保留可核對的任務索引。
    broker
        .memory()?
        .save_run(&run.id, &request_text, "尚未完成", "running", None, &[])?;
    let result = (|| {
        run.messages = broker.memory()?.context(&run.messages)?;
        let mut worker = Worker::start(
            &std::env::current_exe().map_err(|e| e.to_string())?,
            &run.cancel,
        )?;
        report(&mut activity, &mut progress, "正在確認模型能力…".into());
        let caps = jobs::capabilities(&run.config, &run.session)?;
        if !caps.supports("background") {
            return Err("文件工作區需要後端支援背景請求。".into());
        }
        let mut pending_model = None;
        let resumed = if run.resume {
            let mut saved = load_pause(&run.root, &run.id)?;
            if saved.version != 1
                || !saved.available
                || saved.project != run.project.id
                || saved.project_root != run.project.root
                || saved.conversation != run.conversation
                || saved.principal != caps.principal_id
            {
                return Err("暫停紀錄已使用、帳號不同或專案授權已變更，未續接。".into());
            }
            // 所有檢查成功才消耗此續接點；當機後不自動重播可能已執行的操作。
            if let Some(task) = &saved.pending_model {
                jobs::validate_id(&task.request_id)?;
                if task.conversation_id != run.conversation {
                    return Err("待查請求不屬於本次對話，未續接。".into());
                }
            }
            broker.restore(saved.broker, &run.cancel)?;
            pending_model = saved.pending_model;
            request_text = saved.request_text;
            saved.progress.resume_segment();
            checkpoint(
                &pause_path(&run.root, &run.id)?,
                &json!({"version":1,"available":false}),
            )?;
            Some(saved.progress)
        } else {
            None
        };
        charts(broker.charts().to_vec());
        let remote = jobs::conversation(&run.config, &run.session, &run.conversation)?;
        let native = resumed
            .as_ref()
            .map(|p| p.agent.is_some())
            .unwrap_or(!legacy_test);
        let agent_caps = if native {
            let saved_caps = resumed
                .as_ref()
                .and_then(|p| p.agent.as_ref())
                .map(|s| s.caps.clone());
            let caps = match saved_caps {
                Some(caps) => caps,
                None => super::agent::capabilities(&run.config, &run.session, true)?,
            };
            caps.validate(&run.config.model, true)?;
            Some(caps)
        } else {
            None
        };
        if let Some(agent) = &agent_caps {
            if agent.principal_id != caps.principal_id {
                return Err("代理帳號與目前登入身分不一致。".into());
            }
            report(&mut activity, &mut progress, agent.mode_label().into());
        }
        let prompt = if native {
            super::agent::system_prompt()
        } else {
            super::tool_calls::system_prompt()?
        };
        let mut skill = Message::user(&prompt);
        skill.role = "system".into();
        run.messages.insert(0, skill);
        let deadline = Instant::now() + budget;
        let mut failures = 0;
        let mut progress_state = resumed.unwrap_or_else(|| {
            super::progress::Progress::new(
                run.messages.clone().into_iter().map(Into::into).collect(),
            )
        });
        if progress_state.agent.is_none() {
            progress_state.agent = agent_caps.clone().map(super::agent::State::new);
        }
        let mut tool_calls = 0;
        // 只有已確認本機工具結果時才使用此暫停入口；寫入中斷仍是一般錯誤。
        let pause = |reason: &str,
                     broker: &Broker,
                     state: &super::progress::Progress,
                     pending: Option<&Task>| {
            check_cancel(&run.cancel)?;
            save_pause(
                &run,
                &caps.principal_id,
                &request_text,
                broker,
                state,
                pending,
            )?;
            Ok::<String, String>(format!(
                "{reason}。目前狀態已加密暫存，請按「繼續」以接續未完成的任務。"
            ))
        };
        for turn in 0..=MAX_REPLIES {
            check_cancel(&run.cancel)?;
            if quota_reached(turn, tool_calls) {
                return pause("來回次數已達到上限", &broker, &progress_state, None);
            }
            if Instant::now() >= deadline {
                return pause(
                    TIME_LIMIT_REASON,
                    &broker,
                    &progress_state,
                    pending_model.as_ref(),
                );
            }
            if progress_state.stalled() {
                return pause("連續八次未增加有效進度", &broker, &progress_state, None);
            }
            let mut messages = match progress_state.messages(broker.progress_snapshot()) {
                Ok(messages) => messages,
                Err(error) => {
                    return pause(&error, &broker, &progress_state, pending_model.as_ref())
                }
            };
            let instructions = broker.skill_context()?;
            if !instructions.is_empty() {
                if native {
                    // 契約僅一則最前面的 system；按需技能併入同一則。
                    messages[0].content.push_str(&format!("\n{instructions}"));
                } else {
                    let mut message = super::agent::Message::user(&instructions);
                    message.role = "system".into();
                    messages.insert(1, message);
                }
            }
            if turn >= MAX_REPLIES - 4 || tool_calls >= MAX_TOOLS - 4 {
                messages.push(super::agent::Message::user(&format!("即將暫停：本段剩餘 {} 次模型回覆、{} 次工具操作。請在這次 arguments.progress_note 總結已完成、來源與版本、未完成事項及下一步；保持正常工具呼叫，不要假裝完成。程式會保存工作副本，等待使用者按繼續。",MAX_REPLIES-turn,MAX_TOOLS-tool_calls)));
            }
            record["progress"] = progress_state.snapshot(broker.progress_snapshot());
            report(
                &mut activity,
                &mut progress,
                format!("等待 AI 回覆（第 {} 輪）", turn + 1),
            );
            let lookup_only = pending_model.is_some();
            let id = pending_model
                .as_ref()
                .map(|t| t.request_id.clone())
                .map_or_else(jobs::new_id, Ok)?;
            let request = if let Some(task) = &pending_model {
                task.request.clone()
            } else if let Some(agent) = progress_state.agent.as_mut() {
                let capabilities = agent.caps.clone();
                let parent = agent.parent.clone();
                let mut request = agent.request(
                    &capabilities,
                    &run,
                    &remote,
                    &id,
                    &messages,
                    true,
                    parent.as_deref(),
                )?;
                broker.restrict_tools(&mut request);
                request
            } else {
                let legacy = messages
                    .iter()
                    .map(super::agent::Message::legacy)
                    .collect::<Vec<_>>();
                jobs::project_chat_request(&run.config.model, &legacy, &remote, &id)?
            };
            let mut task = pending_model.take().unwrap_or(Task {
                request_id: id.clone(),
                conversation_id: run.conversation.clone(),
                request,
                mode: "background".into(),
                title: run.project.name.clone(),
                created_at: crate::unix_now(),
                remote: None,
                applied: false,
                message: String::new(),
                mail_analysis: false,
                title_generation: false,
                tool_events: vec![],
                partial: String::new(),
            });
            record["state"] = json!("waiting_model");
            record["activity"] = json!(activity);
            record["requests"]
                .as_array_mut()
                .ok_or("任務記錄不正確。")?
                .push(json!({"id":id,"request":task.request}));
            checkpoint(&journal, &record)?;
            super::events::register(&run.root, &id)?;
            let received =
                super::model::receive(&run, &mut task, deadline, lookup_only, agent_caps.as_ref());
            record["last_remote_status"] = json!(task.remote);
            // 每輪保留可讀的回覆摘錄與伺服器識別，不依賴網站是否建立聊天紀錄。
            // 不另外複製整份 messages／tools；長回覆明確標示截斷。
            if let Some(entry) = record["requests"].as_array_mut().and_then(|a| a.last_mut()) {
                entry["turn"] = json!(turn + 1);
                if let Some(status) = &task.remote {
                    entry["task_id"] = json!(status.task_id);
                    entry["state"] = json!(status.state);
                    if let Some(result) = &status.result {
                        let reply = result
                            .to_string()
                            .replace(&run.session.access_token, "[已隱藏]");
                        entry["response"] = json!(diagnostic_excerpt(&reply, 32_000));
                    }
                    entry["error"] = json!(status
                        .error_message
                        .replace(&run.session.access_token, "[已隱藏]"));
                }
                if let Err(error) = &received {
                    entry["error"] = json!(error.replace(&run.session.access_token, "[已隱藏]"));
                }
            }
            checkpoint(&journal, &record)?;
            if let Err(error) = &received {
                record["request_error"] = json!({"turn":turn+1,"request_id":id,"message":error});
                checkpoint(&journal, &record)?;
            }
            let outcome = received.map_err(|e| format!("第 {} 輪：{e}", turn + 1))?;
            check_cancel(&run.cancel)?;
            // 已拿到完成回覆但剛好到期：保存該回覆，續接後才解析／執行。
            if Instant::now() >= deadline {
                return pause(TIME_LIMIT_REASON, &broker, &progress_state, Some(&task));
            }
            let mut native_call = None;
            let (reply, parsed, reason) = match outcome {
                super::model::Reply::Native => {
                    if let Some(agent) = progress_state.agent.as_mut() {
                        agent.parent = Some(id.clone());
                    }
                    let status = task.remote.as_ref().ok_or("原生任務缺少結果。")?;
                    let raw = status
                        .result
                        .as_ref()
                        .map(Value::to_string)
                        .unwrap_or_default();
                    match super::agent::parse(status, &task, &caps.principal_id, &run.id)? {
                        super::agent::Parsed::Operation {
                            parsed,
                            message,
                            call_id,
                        } => {
                            native_call = Some((message, call_id));
                            (raw, Some(*parsed), String::new())
                        }
                        super::agent::Parsed::Repair {
                            reason,
                            message,
                            call_id,
                        } => {
                            if let (Some(message), Some(call_id)) = (message, call_id) {
                                progress_state.push_native(
                                    message,
                                    &call_id,
                                    &json!({"ok":false,"error":reason,"executed":false}),
                                );
                            }
                            (raw, None, reason)
                        }
                        super::agent::Parsed::Text(_) => {
                            return Err("主代理未取得工具回覆。".into())
                        }
                    }
                }
                super::model::Reply::Text(text) => match super::reply::parse(&text)? {
                    super::reply::ParseOutcome::Operation(parsed) => {
                        (text, Some(*parsed), String::new())
                    }
                    super::reply::ParseOutcome::Repair(reason) => (text, None, reason.to_owned()),
                },
                super::model::Reply::Invalid { reason, raw } => (raw, None, reason),
                super::model::Reply::Pending(reason) => {
                    return pause(&reason, &broker, &progress_state, Some(&task));
                }
            };
            record["last_model_reply"] = json!(reply);
            if let Some(entry) = record["requests"].as_array_mut().and_then(|a| a.last_mut()) {
                if entry.get("response").is_none() {
                    entry["response"] = json!(diagnostic_excerpt(
                        &reply.replace(&run.session.access_token, "[已隱藏]"),
                        32_000
                    ));
                }
            }
            record["last_remote_status"] = json!(task.remote);
            // 已知終態才可發起修復；多 JSON、空白完成均不執行候選工具。
            let Some(parsed) = parsed else {
                let label = match progress_state.repair(&reason, &reply) {
                    Ok(label) => label,
                    Err(error) => return pause(&error, &broker, &progress_state, None),
                };
                report(&mut activity, &mut progress, label.into());
                record["progress"] = progress_state.snapshot(broker.progress_snapshot());
                checkpoint(&journal, &record)?;
                continue;
            };
            if matches!(
                parsed.decision,
                Decision::Finish { .. } | Decision::AskUser { .. }
            ) {
                task_summary = parsed.task_summary.clone();
            }
            if progress_state.accept_note(parsed.note.as_deref()) {
                report(&mut activity, &mut progress, "已更新任務筆記".into());
                record["progress"] = progress_state.snapshot(broker.progress_snapshot());
            }
            if !parsed.commentary.is_empty() {
                report(
                    &mut activity,
                    &mut progress,
                    format!("AI 說明：{}", parsed.commentary),
                );
            }
            match parsed.decision {
                Decision::Tool {
                    operation_id,
                    request,
                } => {
                    if tool_calls >= MAX_TOOLS {
                        return Err("已達本次 60 次工具操作上限，已保留進度與成果。".into());
                    }
                    tool_calls += 1;
                    let pdf_source = match &request {
                        super::Tool::ReadFile { path, .. } | super::Tool::FindText { path, .. } => {
                            Some(path)
                        }
                        super::Tool::CreateWorkingCopy { source, .. } => source.as_ref(),
                        _ => None,
                    };
                    let label =
                        if pdf_source.is_some_and(|p| p.to_ascii_lowercase().ends_with(".pdf")) {
                            "讀取 PDF 文字"
                        } else {
                            request.label()
                        };
                    report(&mut activity, &mut progress, format!("{label}…"));
                    record["activity"] = json!(activity);
                    record["state"] = json!("executing_tool");
                    record["pending_operation"] = json!({"id":operation_id,"request":request});
                    checkpoint(&journal, &record)?;
                    check_cancel(&run.cancel)?;
                    if Instant::now() >= deadline {
                        return pause(TIME_LIMIT_REASON, &broker, &progress_state, Some(&task));
                    }
                    record["pending_operation"]["state"] = json!("started");
                    checkpoint(&journal, &record)?;
                    let result =
                        if let Some(result) = broker.cached_result(&operation_id, &request)? {
                            result
                        } else if let super::Tool::SummarizeDocument { path, focus } = &request {
                            match super::delegation::summarize(
                                &run,
                                &mut broker,
                                &mut worker,
                                &caps.principal_id,
                                &operation_id,
                                progress_state.agent.as_mut(),
                                &id,
                                path,
                                focus,
                                deadline,
                                |text| report(&mut activity, &mut progress, text),
                            )? {
                                super::delegation::Outcome::Complete(result) => result,
                                super::delegation::Outcome::Pending(reason) => {
                                    return pause(&reason, &broker, &progress_state, Some(&task))
                                }
                            }
                        } else {
                            broker.execute(&operation_id, &request, &mut worker, &run.cancel)?
                        };
                    broker.remember_result(&operation_id, &request, &result)?;
                    record["charts"] = json!(broker.charts());
                    charts(broker.charts().to_vec());
                    report(
                        &mut activity,
                        &mut progress,
                        if result["ok"] == false {
                            let reason = result["error"].as_str().unwrap_or("工具未提供錯誤原因");
                            format!(
                                "{label}：失敗（操作 {operation_id}）\n{}",
                                diagnostic_excerpt(
                                    &reason.replace(&run.session.access_token, "[已隱藏]"),
                                    800
                                )
                            )
                        } else {
                            format!("{label}：完成")
                        },
                    );
                    record["activity"] = json!(activity);
                    record["operations"]
                        .as_array_mut()
                        .ok_or("任務記錄不正確。")?
                        .push(json!({"id":operation_id,"request_id":id,"turn":turn+1,"request":request,"result":result}));
                    progress_state.observe(&operation_id, &request, &result);
                    if let Some((message, call_id)) = native_call {
                        progress_state.push_native(message, &call_id, &result);
                    } else {
                        progress_state.push_tool(
                            reply,
                            super::tool_calls::result_text(&operation_id, &result),
                        );
                    }
                    record["pending_operation"] = Value::Null;
                    record["progress"] = progress_state.snapshot(broker.progress_snapshot());
                    record["outputs"] = json!(broker.published());
                    record["pending_operation"] = Value::Null;
                    checkpoint(&journal, &record)?;
                    failures = if result["ok"] == false {
                        failures + 1
                    } else {
                        0
                    };
                    if failures >= 3 {
                        return Err(format!(
                            "連續三次操作失敗，已停止：{}",
                            result["error"].as_str().unwrap_or("請檢查文件存取方式。")
                        ));
                    }
                }
                Decision::Finish { message, artifacts } => {
                    report(&mut activity, &mut progress, "正在核對成果…".into());
                    match broker.finish_cancellable(&artifacts, &run.cancel) {
                        Ok(paths) => {
                            let locations = if paths.is_empty() {
                                String::new()
                            } else {
                                format!(
                                    "\n\n成果檔案（相對專案資料夾）：\n{}",
                                    paths
                                        .iter()
                                        .map(|p| format!("- `{p}`"))
                                        .collect::<Vec<_>>()
                                        .join("\n")
                                )
                            };
                            if let Some((call, call_id)) = native_call {
                                record["terminal_tool_result"] = json!({"call":call,"result":super::agent::Message::result(&call_id,&json!({"ok":true,"state":"completed"}))});
                            }
                            return Ok(format!("{message}{locations}"));
                        }
                        Err(error) => {
                            failures += 1;
                            if failures >= 3 {
                                return Err(error);
                            }
                            if let Some((message, call_id)) = native_call {
                                progress_state.push_native(
                                    message,
                                    &call_id,
                                    &json!({"ok":false,"error":error,"executed":false}),
                                );
                            }
                            let label = progress_state
                                .repair(&format!("交付檢查未通過：{error}"), &reply)?;
                            report(&mut activity, &mut progress, label.into());
                            record["progress"] =
                                progress_state.snapshot(broker.progress_snapshot());
                        }
                    }
                }
                Decision::AskUser { message } => {
                    if let Some((call, call_id)) = native_call {
                        record["terminal_tool_result"] = json!({"call":call,"result":super::agent::Message::result(&call_id,&json!({"ok":true,"state":"waiting_user"}))});
                    }
                    return Ok(format!("需要你的補充：{message}"));
                }
            }
        }
        Err("已達本次 80 次模型回覆上限，已停止並保留輸出。".into())
    })();
    report(
        &mut activity,
        &mut progress,
        if result.is_ok() {
            "本次執行已結束"
        } else {
            "任務未完成，請查看對話"
        }
        .into(),
    );
    record["activity"] = json!(activity);
    let paused = result.is_ok() && paused_available(&run.root, &run.id);
    record["state"] = json!(if paused {
        "paused"
    } else if result.is_ok() {
        "ended"
    } else {
        "stopped"
    });
    record["charts"] = json!(broker.charts());
    record["outputs"] = json!(broker.published());
    record["result"] = json!(result);
    checkpoint(&journal, &record)?;
    let outcome = result
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or_else(|s| s.as_str());
    let state = if run.cancel.load(Ordering::Relaxed) {
        "cancelled"
    } else if result.is_err() {
        "failed"
    } else if paused {
        "paused"
    } else if outcome.starts_with("需要你的補充：") {
        "waiting_user"
    } else {
        "completed"
    };
    if let Err(error) = broker.memory()?.save_run(
        &run.id,
        &request_text,
        outcome,
        state,
        task_summary.as_deref(),
        broker.published(),
    ) {
        // 摘要保存失败不能推翻已核對的交付；完整結果仍在本機對話及 DPAPI 任務紀錄。
        report(
            &mut activity,
            &mut progress,
            format!("專案記憶暫未保存：{error}"),
        );
    }
    result.map_err(|error| {
        if broker.published().is_empty() {
            error
        } else {
            format!(
                "{error}\n\n以下檔案可能已建立，請檢查；未驗證的檔案不算已交付：\n{}",
                broker.published().join("\n")
            )
        }
    })
}
/// 重複輪詢訊息不重複加入，歷程有明確大小上限，不保存文件全文。
fn report(activity: &mut Vec<String>, progress: &mut impl FnMut(String), message: String) {
    let message: String = message.chars().take(2048).collect();
    if activity.last() == Some(&message) {
        return;
    }
    if activity.len() >= 120 {
        activity.remove(0);
    }
    activity.push(message.clone());
    progress(message);
}

/// 重啟後只恢復顯示紀錄，不執行任何原有操作。
pub fn recover_activity(root: &Path, id: &str) -> AppResult<Vec<String>> {
    jobs::validate_id(id)?;
    let path = root.join("project-runs").join(format!("{id}.dpapi"));
    if !path.exists() {
        return Ok(Vec::new());
    }
    if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 64_000_000 {
        return Err("上次任務紀錄過大。".into());
    }
    let bytes = storage::protect(&std::fs::read(path).map_err(|e| e.to_string())?, false)?;
    let record: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    Ok(record["activity"]
        .as_array()
        .into_iter()
        .flatten()
        .take(120)
        .filter_map(|value| value.as_str())
        .map(|text| text.chars().take(2048).collect())
        .collect())
}

pub(super) fn check_cancel(cancel: &AtomicBool) -> AppResult<()> {
    if cancel.load(Ordering::Relaxed) {
        return Err("專案任務已停止，不會繼續執行本機工具。".into());
    }
    Ok(())
}

/// 重啟只恢復可見結果／中斷說明，不恢復檔案授權，也不重播工具。
pub fn recover(root: &Path, id: &str) -> AppResult<String> {
    jobs::validate_id(id)?;
    let path = root.join("project-runs").join(format!("{id}.dpapi"));
    if !path.exists() {
        return Ok("上次專案任務在開始前中斷；未自動重新執行，請重新送出需求。".into());
    }
    if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 64_000_000 {
        return Err("上次任務紀錄過大，原檔保留。".into());
    }
    let bytes = storage::protect(&std::fs::read(path).map_err(|e| e.to_string())?, false)?;
    let record: Value = serde_json::from_slice(&bytes).map_err(|_| "上次專案任務紀錄無法解析。")?;
    if let Some(text) = record["result"]["Ok"].as_str() {
        return Ok(text.into());
    }
    let reason = record["result"]["Err"].as_str().unwrap_or(
        "應用程式已退出，先前的本機操作未完成；不會自動重播。尚未發布的記憶體副本已釋放。",
    );
    let outputs: Vec<_> = record["outputs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .collect();
    Ok(format!(
        "上次專案任務未完成：{reason}\n\n可能已建立的檔案（需重新確認）：\n{}",
        outputs.join("\n")
    ))
}

/// 歷史對話與異常復原共用已保存的結構化圖表，不重新呼叫模型。
pub fn recover_charts(root: &Path, id: &str) -> Vec<super::charts::Chart> {
    let read = || -> AppResult<Vec<super::charts::Chart>> {
        jobs::validate_id(id)?;
        let bytes = std::fs::read(root.join("project-runs").join(format!("{id}.dpapi")))
            .map_err(|e| e.to_string())?;
        let data: Value =
            serde_json::from_slice(&storage::protect(&bytes, false)?).map_err(|e| e.to_string())?;
        let charts: Vec<super::charts::Chart> =
            serde_json::from_value(data.get("charts").cloned().unwrap_or(json!([])))
                .map_err(|e| e.to_string())?;
        for chart in &charts {
            chart.validate()?;
        }
        Ok(charts)
    };
    read().unwrap_or_default()
}

#[cfg(test)]
mod pause_tests {
    use super::*;
    #[test]
    fn segment_budget_is_two_hours_and_cancellation_stays_separate() {
        assert_eq!(SEGMENT_BUDGET.as_secs(), 7200);
        assert!(check_cancel(&AtomicBool::new(false)).is_ok());
        assert!(check_cancel(&AtomicBool::new(true)).is_err());
    }
    #[test]
    fn tool_and_model_quotas_pause_at_the_exact_boundary() {
        assert!(!quota_reached(20, 20));
        assert!(!quota_reached(79, 59));
        assert!(quota_reached(80, 59));
        assert!(quota_reached(79, 60));
    }
}
