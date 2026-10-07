//! 專案圖片辨識：沿用目前模型與正式代理 API，以獨立、無工具的子請求按需看圖。
//! 主任務只取得辨識文字和來源；未知提交保留原 ID 查回，不重送圖片。
pub(crate) mod input;
use super::{agent, delegation::Outcome, files::Broker, model, runner::Run, text};
use crate::{
    jobs::{self, Task},
    AppResult,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Instant;

/// 一次只送一張，允許任務按需逐張閱讀；相同來源與焦點的完成快取不扣新額度。
const MAX_REQUESTS: usize = 20;

#[derive(Default, Serialize, Deserialize)]
struct State {
    identity: Value,
    pending: Option<Task>,
    completed: Option<Value>,
}

fn failed(reason: &str) -> Outcome {
    Outcome::Complete(json!({"ok":false,"error":reason,"retry_same_operation":false}))
}

/// 圖片是資料而非指令；無檔案／工具能力的子請求只回傳有限文字。
#[allow(clippy::too_many_arguments)]
pub(super) fn analyze(
    run: &Run,
    broker: &mut Broker,
    operation: &str,
    agent: Option<&mut agent::State>,
    parent: &Task,
    path: &str,
    focus: &str,
    deadline: Instant,
    mut progress: impl FnMut(String),
) -> AppResult<Outcome> {
    if !input::model_supported(&run.config.model) {
        return Ok(failed(input::UNSUPPORTED_MODEL));
    }
    let Some(agent) = agent else {
        return Ok(failed("圖片辨識需要原生專案代理；請建立新任務。"));
    };
    if focus.trim().is_empty() || focus.chars().count() > 1000 {
        return Ok(failed("圖片辨識要求需為 1–1000 字。"));
    }
    super::runner::check_cancel(&run.cancel)?;
    let image = match input::load(&run.project, path) {
        Ok(image) => image,
        Err(error) => return Ok(failed(&error)),
    };
    let caps = agent.caps.clone();
    caps.validate(&run.config.model, false)?;
    // 保留既有識別碼，讓試驗版的待查請求與快取仍可按原 ID 續接。
    let identity = json!({"profile":"project-image-trial-v1","image":image.metadata(),"focus":focus,
        "model":run.config.model,"principal":caps.principal_id,"binding":run.config.binding()?});
    let key = text::revision(&format!(
        "vision|{}|{operation}|{}",
        run.id, caps.principal_id
    ));
    let mut state = broker
        .memory()?
        .delegation_read::<State>(&key)?
        .unwrap_or_default();
    if !state.identity.is_null() && state.identity != identity {
        return Ok(failed(
            "圖片或辨識要求已變更；保留原請求，未重播，請重新指定圖片。",
        ));
    }
    state.identity = identity.clone();
    if let Some(result) = state.completed.as_ref() {
        return Ok(Outcome::Complete(result.clone()));
    }
    // 相同任務／圖片版本／焦點已完成時重用文字，避免不同工具 ID 重複上傳。
    let cache = text::revision(&format!("vision-cache|{}|{identity}", run.id));
    if let Some(result) = broker.memory()?.delegation_read::<Value>(&cache)? {
        return Ok(Outcome::Complete(result));
    }
    if Instant::now() >= deadline {
        return Ok(Outcome::Pending(
            "圖片辨識已到本段時限，請繼續查回。".into(),
        ));
    }
    let lookup_only = state.pending.is_some();
    if state.pending.is_none() {
        let budget_key = text::revision(&format!("vision-budget|{}|{}", run.id, caps.principal_id));
        let mut attempts = broker
            .memory()?
            .delegation_read::<Vec<String>>(&budget_key)?
            .unwrap_or_default();
        if !attempts.contains(&key) {
            if attempts.len() >= MAX_REQUESTS {
                return Ok(failed(
                    "每次任務最多 20 次不同圖片辨識要求；請沿用已取得的文字重點，仍不足時再另開任務。",
                ));
            }
            attempts.push(key.clone());
            broker.memory()?.delegation_write(&budget_key, &attempts)?;
        }
        let id = jobs::new_id()?;
        // 直接沿用已核對父請求的遠端 ID；不可誤用本機對話 ID 或另建對話。
        let remote = parent.request["conversation_id"]
            .as_str()
            .ok_or("父請求缺少遠端對話。")?;
        let request = agent.image_request(run, remote, &id, &parent.request_id, &image, focus)?;
        state.pending = Some(Task {
            request_id: id.clone(),
            conversation_id: run.conversation.clone(),
            request,
            mode: "background".into(),
            title: "專案圖片辨識".into(),
            created_at: crate::unix_now(),
            remote: None,
            applied: false,
            message: String::new(),
            mail_analysis: false,
            title_generation: false,
            tool_events: vec![],
            partial: String::new(),
            project_retry: Default::default(),
        });
        super::events::register(&run.root, &id)?;
        // 先以 DPAPI 保存包含圖片的請求，後續即使斷線也只查同一個 ID。
        broker.memory()?.delegation_write(&key, &state)?;
    }
    let mut task = state.pending.take().ok_or("缺少圖片辨識請求。")?;
    agent.advance_past(&task.request);
    progress(
        if lookup_only {
            "正在查回原圖片辨識結果"
        } else {
            "正在以目前模型辨識圖片"
        }
        .into(),
    );
    let response = model::receive(
        run,
        &mut task,
        deadline,
        lookup_only,
        Some(&caps),
        (
            |task: &Task| {
                state.pending = Some(task.clone());
                broker.memory()?.delegation_write(&key, &state)
            },
            &mut progress,
        ),
    );
    agent.advance_past(&task.request);
    state.pending = Some(task);
    broker.memory()?.delegation_write(&key, &state)?;
    let result = match response? {
        model::Reply::Rejected(error) => {
            json!({"ok":false,"error":error,"retry_same_operation":false})
        }
        model::Reply::Pending(reason) => return Ok(Outcome::Pending(reason)),
        model::Reply::Native => {
            let task = state.pending.as_ref().ok_or("缺少圖片原請求。")?;
            match agent::parse(
                task.remote.as_ref().ok_or("缺少圖片回覆。")?,
                task,
                &caps.principal_id,
                &run.id,
            )? {
                agent::Parsed::Text(answer) if answer.chars().count() <= 6000 => {
                    json!({"ok":true,"result":{
                    "image":image.metadata(),"focus":focus,"model":run.config.model,"analysis":answer,
                    "context_note":"後續只使用這份文字重點及來源；不再附原圖。未辨識或不確定的內容不可視為已讀取。",
                    "evidence_status":"模型對單張圖片的辨識，並非已核實事實；看不清的文字、數字及推測需另行確認。"}})
                }
                agent::Parsed::Repair { reason, .. } => json!({"ok":false,"error":reason}),
                _ => json!({"ok":false,"error":"圖片辨識未回傳有效的有限文字；不接受工具呼叫。"}),
            }
        }
        _ => json!({"ok":false,"error":"圖片代理未回傳原生圖片辨識結果。"}),
    };
    // 成功／已知終態均移除大圖，只保存辨識文字；未確認終態絕不走到這裡。
    state.pending = None;
    state.completed = Some(result.clone());
    broker.memory()?.delegation_write(&key, &state)?;
    broker.memory()?.delegation_write(&cache, &result)?;
    Ok(Outcome::Complete(result))
}
