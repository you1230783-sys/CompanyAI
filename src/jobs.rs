//! 0.5 桌面契約：能力、附件工作、持久任務、估時與串流。
//! 網站負責排隊／轉檔／模型執行；REST 是結果真相來源，SSE 與通知只改善即時呈現。
use crate::{
    attachments::{Attachment, AttachmentRules, AttachmentStatus},
    config::{Config, CHAT_PATH},
    protocol::{self, Message},
    storage::{self, Session},
    transport, AppResult,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};
pub const PREFIX: &str = "/lm_server/api/desktop";

#[derive(Clone, Serialize, Deserialize)]
pub struct Capabilities {
    pub contract_version: u32,
    /// 伺服器提供穩定、不具識別個資的帳號代號；Token 換發後仍能接續自己的工作。
    pub principal_id: String,
    pub execution_modes: Vec<String>,
    pub attachments: AttachmentRules,
    #[serde(default)]
    pub timing_estimates: bool,
}
impl Capabilities {
    pub fn validate(&self) -> AppResult<()> {
        validate_id(&self.principal_id)?;
        if self.contract_version != 1
            || self.execution_modes.is_empty()
            || self.execution_modes.len() > 3
            || self
                .execution_modes
                .iter()
                .any(|m| !matches!(m.as_str(), "sync" | "stream" | "background"))
        {
            return Err("網站尚未支援本版長任務契約。".into());
        }
        self.attachments.validate()
    }
    pub fn supports(&self, mode: &str) -> bool {
        self.execution_modes.iter().any(|m| m == mode)
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Timing {
    pub estimated_wait_seconds: Option<f64>,
    pub estimated_processing_seconds: Option<f64>,
    pub estimated_total_seconds: Option<f64>,
    pub sample_count: Option<u64>,
    pub confidence: Option<String>,
    pub generated_at: Option<String>,
}
impl Timing {
    pub fn validate(&self) -> AppResult<()> {
        if [
            self.estimated_wait_seconds,
            self.estimated_processing_seconds,
            self.estimated_total_seconds,
        ]
        .iter()
        .flatten()
        .any(|v| !v.is_finite() || *v < 0.0)
            || self.confidence.as_ref().is_some_and(|v| v.len() > 30)
            || self.generated_at.as_ref().is_some_and(|v| v.len() > 50)
        {
            return Err("估時資料格式不正確。".into());
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct TaskStatus {
    pub task_id: String,
    pub client_request_id: String,
    pub state: String,
    #[serde(default)]
    pub progress: Option<f64>,
    #[serde(default)]
    pub queue_position: Option<u32>,
    #[serde(default)]
    pub timing: Timing,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error_message: String,
}
impl TaskStatus {
    pub fn validate(&self) -> AppResult<()> {
        validate_id(&self.task_id)?;
        validate_id(&self.client_request_id)?;
        if !matches!(
            self.state.as_str(),
            "queued" | "running" | "cancelling" | "completed" | "failed" | "cancelled"
        ) || self
            .progress
            .is_some_and(|p| !p.is_finite() || !(0.0..=100.0).contains(&p))
            || self.error_message.len() > 2000
        {
            return Err("任務回應格式不正確。".into());
        }
        self.timing.validate()?;
        if self.state == "completed" {
            protocol::assistant_text(
                &self
                    .result
                    .as_ref()
                    .ok_or("已完成任務缺少 result。")?
                    .to_string(),
            )?;
        }
        Ok(())
    }
    pub fn terminal(&self) -> bool {
        matches!(self.state.as_str(), "completed" | "failed" | "cancelled")
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Task {
    pub request_id: String,
    pub conversation_id: String,
    pub request: Value,
    pub mode: String,
    pub title: String,
    pub created_at: u64,
    pub remote: Option<TaskStatus>,
    pub applied: bool,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub mail_analysis: bool,
    #[serde(skip)]
    pub partial: String,
}
impl Task {
    pub fn active(&self) -> bool {
        !self.applied
    }
    pub fn stop_tracking(&mut self) {
        self.applied = true;
        self.partial.clear();
        self.request = json!({});
        self.message = "已停止本機追蹤；不保證伺服器工作已取消。".into();
    }
    pub fn apply_status(&mut self, status: TaskStatus) -> AppResult<()> {
        if !self.active() {
            return Ok(());
        }
        status.validate()?;
        if status.client_request_id != self.request_id
            || self
                .remote
                .as_ref()
                .is_some_and(|r| r.task_id != status.task_id)
        {
            return Err("任務回應識別碼不一致。".into());
        }
        // 已確認終態不能被較早的輪詢回應退回執行中。
        if self.remote.as_ref().is_some_and(TaskStatus::terminal) {
            return Ok(());
        }
        self.message = status.error_message.clone();
        self.remote = Some(status);
        Ok(())
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct WorkStore {
    pub principal_id: String,
    pub conversations: std::collections::BTreeMap<String, String>,
    pub attachments: Vec<Attachment>,
    pub tasks: Vec<Task>,
}
impl WorkStore {
    pub fn pending(&self, id: Option<&str>) -> bool {
        id.is_some_and(|id| {
            self.tasks
                .iter()
                .any(|t| t.conversation_id == id && t.active())
        })
    }
    pub fn drafts(&self, id: Option<&str>) -> Vec<&Attachment> {
        self.attachments
            .iter()
            .filter(|a| Some(a.conversation_id.as_str()) == id && !a.sent && !a.removed)
            .collect()
    }
    pub fn save(&self, root: &Path, config: &Config) -> AppResult<()> {
        let bytes = serde_json::to_vec(self).map_err(|_| "無法序列化任務紀錄。")?;
        if bytes.len() > 32 * 1024 * 1024 {
            return Err("任務紀錄超過 32 MB，請清理完成的對話。".into());
        }
        storage::atomic_write(&self.path(root, config)?, &storage::protect(&bytes, true)?)
    }
    fn path(&self, root: &Path, config: &Config) -> AppResult<PathBuf> {
        validate_id(&self.principal_id)?;
        let hash = format!(
            "{:x}",
            Sha256::digest(format!("{}|{}", config.binding()?, self.principal_id))
        );
        Ok(root.join(format!("jobs-{hash}.dpapi")))
    }
    pub fn load(root: &Path, config: &Config, principal: &str) -> AppResult<Self> {
        let empty = Self {
            principal_id: principal.into(),
            ..Self::default()
        };
        let path = empty.path(root, config)?;
        match fs::metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(empty),
            Err(_) => return Err("無法讀取任務紀錄；原檔保留。".into()),
            Ok(meta) if meta.len() > 32 * 1024 * 1024 + 4096 => return Err("任務紀錄過大。".into()),
            _ => {}
        }
        let bytes = storage::protect(&fs::read(path).map_err(|_| "無法讀取任務紀錄。")?, false)?;
        let mut store: Self =
            serde_json::from_slice(&bytes).map_err(|_| "任務紀錄損毀；原檔保留。")?;
        if store.principal_id != principal
            || store.tasks.len() > 4000
            || store.attachments.len() > 8000
        {
            return Err("任務紀錄格式不正確。".into());
        }
        for (local, remote) in &store.conversations {
            validate_id(local)?;
            validate_id(remote)?;
        }
        for task in &store.tasks {
            validate_id(&task.request_id)?;
            validate_id(&task.conversation_id)?;
            if let Some(remote) = &task.remote {
                remote.validate()?;
            }
        }
        for a in &mut store.attachments {
            validate_id(&a.id)?;
            validate_id(&a.conversation_id)?;
            if let Some(remote) = &a.remote {
                remote.validate()?;
            }
            if a.state == "reading" {
                a.state = "failed".into();
                a.message = "檔案接收未完成，請移除後重新選取。".into();
            }
        }
        Ok(store)
    }
}

/// 將權威完成結果投影到本機歷史；request_id 保證重新啟動／重播時只加入一次。
pub fn apply_reply(archive: &mut crate::history::Archive, task: &Task) -> AppResult<bool> {
    let remote = task.remote.as_ref().ok_or("缺少任務結果。")?;
    remote.validate()?;
    if remote.client_request_id != task.request_id || remote.state != "completed" {
        return Err("任務尚未完成或識別碼不符。".into());
    }
    let conversation = archive
        .conversations
        .iter_mut()
        .find(|c| c.id == task.conversation_id)
        .ok_or("任務對應的本機對話不存在，結果仍留在任務紀錄。")?;
    if conversation
        .messages
        .iter()
        .any(|m| m.role == "assistant" && m.request_id.as_deref() == Some(&task.request_id))
    {
        return Ok(false);
    }
    if !conversation
        .messages
        .iter()
        .any(|m| m.role == "user" && m.request_id.as_deref() == Some(&task.request_id))
    {
        return Err("任務缺少對應的使用者訊息，不會加入其他對話。".into());
    }
    let mut reply =
        protocol::assistant_text(&remote.result.as_ref().ok_or("缺少回覆。")?.to_string())?;
    if task.mail_analysis {
        reply = crate::outlook::format_analysis(reply);
    }
    let mut message = Message::assistant(reply);
    message.request_id = Some(task.request_id.clone());
    conversation.messages.push(message);
    conversation.updated_at = crate::unix_now();
    Ok(true)
}

pub fn validate_id(id: &str) -> AppResult<()> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        return Err("工作識別碼格式不正確。".into());
    }
    Ok(())
}
pub fn new_id() -> AppResult<String> {
    use windows_sys::Win32::Security::Cryptography::*;
    let mut bytes = [0u8; 16];
    if unsafe {
        BCryptGenRandom(
            std::ptr::null_mut(),
            bytes.as_mut_ptr(),
            16,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    } < 0
    {
        return Err("無法建立工作識別碼。".into());
    }
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
pub fn decode<T: DeserializeOwned>(
    response: transport::HttpResponse,
    session: &Session,
) -> AppResult<T> {
    if !(200..300).contains(&response.status) {
        return Err(protocol::api_error(
            response.status,
            &response.body,
            &session.access_token,
        ));
    }
    serde_json::from_str(&response.body).map_err(|_| "網站工作回應 JSON 格式不正確。".into())
}
pub fn get<T: DeserializeOwned>(config: &Config, session: &Session, path: &str) -> AppResult<T> {
    if !session.valid_for(config) {
        return Err("登入已到期，重新登入後可接續工作。".into());
    }
    decode(
        transport::get(
            &config.endpoint(path)?,
            Some(("Authorization", &format!("Bearer {}", session.access_token))),
        )?,
        session,
    )
}
pub fn post<T: DeserializeOwned>(
    config: &Config,
    session: &Session,
    path: &str,
    body: &Value,
) -> AppResult<T> {
    if !session.valid_for(config) {
        return Err("登入已到期，重新登入後可接續工作。".into());
    }
    decode(
        transport::request(
            &config.endpoint(path)?,
            "application/json",
            &body.to_string(),
            Some(("Authorization", &format!("Bearer {}", session.access_token))),
            30_000,
        )?,
        session,
    )
}
pub fn capabilities(config: &Config, session: &Session) -> AppResult<Capabilities> {
    if !session.valid_for(config) {
        return Err("請先登入。".into());
    }
    let mut url = config.endpoint(&format!("{PREFIX}/capabilities"))?;
    url.query_pairs_mut().append_pair("model", &config.model);
    let cap: Capabilities = decode(
        transport::get(
            &url,
            Some(("Authorization", &format!("Bearer {}", session.access_token))),
        )?,
        session,
    )?;
    cap.validate()?;
    Ok(cap)
}
pub fn conversation(config: &Config, session: &Session, local: &str) -> AppResult<String> {
    validate_id(local)?;
    let reply: Value = post(
        config,
        session,
        &format!("{PREFIX}/conversations"),
        &json!({"client_conversation_id":local}),
    )?;
    let id = reply["conversation_id"]
        .as_str()
        .ok_or("網站未回傳 conversation_id。")?;
    validate_id(id)?;
    Ok(id.into())
}
pub fn reserve_attachment(
    config: &Config,
    session: &Session,
    remote: &str,
    a: &Attachment,
) -> AppResult<AttachmentStatus> {
    validate_id(remote)?;
    let status: AttachmentStatus = post(
        config,
        session,
        &format!("{PREFIX}/conversations/{remote}/attachments"),
        &json!({"client_attachment_id":a.id,"filename":a.name,"size_bytes":a.size,"mime_type":a.mime_type}),
    )?;
    status.validate()?;
    Ok(status)
}
pub fn task_status(config: &Config, session: &Session, task: &Task) -> AppResult<TaskStatus> {
    let path = match &task.remote {
        Some(r) => format!("{PREFIX}/tasks/{}", r.task_id),
        None => format!("{PREFIX}/tasks/by-request/{}", task.request_id),
    };
    let result: TaskStatus = get(config, session, &path)?;
    result.validate()?;
    Ok(result)
}
/// 所有識別碼在本機固定後才送出；未知結果的重試必須重用同一 client_request_id。
pub fn chat_request(
    model: &str,
    messages: &[Message],
    conversation: &str,
    request_id: &str,
    mode: &str,
    tokens: Vec<String>,
) -> AppResult<Value> {
    validate_id(conversation)?;
    validate_id(request_id)?;
    let mut value: Value = serde_json::from_str(&protocol::chat_json(model, messages)?)
        .map_err(|_| "聊天資料不正確。")?;
    value["conversation_id"] = json!(conversation);
    value["client_request_id"] = json!(request_id);
    value["execution_mode"] = json!(mode);
    value["stream"] = json!(mode == "stream");
    value["attachment_tokens"] = json!(tokens);
    Ok(value)
}

/// SSE 可以在任意位元組中斷（包括 UTF-8 字元）；先累積完整行再解碼。
#[derive(Default)]
pub struct SseDecoder {
    pending: Vec<u8>,
    data: Vec<String>,
    event: String,
    event_bytes: usize,
    pub done: bool,
}
impl SseDecoder {
    pub fn push(
        &mut self,
        bytes: &[u8],
        mut emit: impl FnMut(&str, &str) -> AppResult<()>,
    ) -> AppResult<()> {
        for byte in bytes {
            if *byte != b'\n' {
                self.pending.push(*byte);
                if self.pending.len() > 1_048_576 {
                    return Err("串流事件過大。".into());
                }
                continue;
            }
            if self.pending.last() == Some(&b'\r') {
                self.pending.pop();
            }
            let line = String::from_utf8(std::mem::take(&mut self.pending))
                .map_err(|_| "串流必須為 UTF-8。")?;
            if line.is_empty() {
                if !self.data.is_empty() {
                    let data = self.data.join("\n");
                    if data == "[DONE]" {
                        self.done = true;
                    } else {
                        emit(&self.event, &data)?;
                    }
                }
                self.event.clear();
                self.data.clear();
                self.event_bytes = 0;
            } else if let Some(data) = line.strip_prefix("data:") {
                self.event_bytes += data.len();
                if self.event_bytes > 1_048_576 {
                    return Err("串流事件過大。".into());
                }
                self.data
                    .push(data.strip_prefix(' ').unwrap_or(data).into());
            } else if let Some(event) = line.strip_prefix("event:") {
                self.event = event.trim().into();
            }
            if self.done {
                break;
            }
        }
        Ok(())
    }
}
pub enum StreamUpdate {
    Status(Box<TaskStatus>),
    Delta(String),
}
pub fn submit(
    config: &Config,
    session: &Session,
    task: &Task,
    update: impl FnMut(StreamUpdate),
) -> AppResult<()> {
    submit_cancellable(
        config,
        session,
        task,
        &std::sync::atomic::AtomicBool::new(false),
        update,
    )
}
/// 停止本機串流時在下一個資料／heartbeat 邊界結束；不宣稱取消 server 工作。
pub fn submit_cancellable(
    config: &Config,
    session: &Session,
    task: &Task,
    cancel: &std::sync::atomic::AtomicBool,
    mut update: impl FnMut(StreamUpdate),
) -> AppResult<()> {
    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
        return Err("已停止本機接收。".into());
    }
    if !session.valid_for(config) {
        return Err("請重新登入後接續任務。".into());
    }
    if task.mode != "stream" {
        let status: TaskStatus = post(config, session, CHAT_PATH, &task.request)?;
        status.validate()?;
        update(StreamUpdate::Status(Box::new(status)));
        return Ok(());
    }
    let mut parser = SseDecoder::default();
    let mut total = 0;
    let mut callback = |bytes: &[u8]| {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("已停止本機接收。".into());
        }
        parser.push(bytes, |event, data| {
            let value: Value = serde_json::from_str(data).map_err(|_| "串流 JSON 格式不正確。")?;
            match event {
                "task" | "status" => {
                    let mut status: TaskStatus =
                        serde_json::from_value(value).map_err(|_| "串流任務格式不正確。")?;
                    // SSE 終態只喚醒查詢；完整回答須由 REST 確認，避免保存半段回覆。
                    if status.terminal() {
                        status.state = "running".into();
                        status.result = None;
                    }
                    status.validate()?;
                    update(StreamUpdate::Status(Box::new(status)));
                }
                "error" => return Err("串流服務回報錯誤，將查詢任務狀態。".into()),
                _ => {
                    if let Some(text) = value["choices"][0]["delta"]["content"].as_str() {
                        total += text.len();
                        if total > 1_048_576 {
                            return Err("串流文字超過 1 MB。".into());
                        }
                        update(StreamUpdate::Delta(text.into()));
                    }
                }
            }
            Ok(())
        })?;
        Ok(parser.done)
    };
    let body = task.request.to_string();
    let mut reader = std::io::Cursor::new(body.as_bytes());
    let response = transport::exchange(
        &config.endpoint(CHAT_PATH)?,
        "POST",
        "application/json",
        transport::Payload {
            reader: &mut reader,
            length: body.len() as u32,
        },
        Some(("Authorization", &format!("Bearer {}", session.access_token))),
        60_000,
        Some(&mut callback),
    )?;
    if response.status != 200 {
        return Err(protocol::api_error(
            response.status,
            &response.body,
            &session.access_token,
        ));
    }
    if !parser.done {
        return Err("串流中斷，保留已收到文字並查詢任務。".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sse_handles_utf8_boundaries_heartbeats_multiline_and_done() {
        let source=": heartbeat\r\nevent: status\r\ndata: {\r\ndata: \"state\":\"中文\"}\r\n\r\ndata: [DONE]\n\n";
        let mut parser = SseDecoder::default();
        let mut found = Vec::new();
        for byte in source.as_bytes() {
            parser
                .push(&[*byte], |kind, data| {
                    found.push((kind.to_string(), data.to_string()));
                    Ok(())
                })
                .unwrap();
        }
        assert!(parser.done);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, "status");
        assert!(found[0].1.contains("中文"));
    }
    #[test]
    fn task_correlation_and_terminal_state_cannot_regress() {
        let status = TaskStatus {
            task_id: "task1".into(),
            client_request_id: "request1".into(),
            state: "failed".into(),
            progress: None,
            queue_position: None,
            timing: Timing::default(),
            result: None,
            error_message: "failed".into(),
        };
        let mut task = Task {
            request_id: "request1".into(),
            conversation_id: "local1".into(),
            request: json!({}),
            mode: "background".into(),
            title: "t".into(),
            created_at: crate::unix_now(),
            remote: None,
            applied: false,
            message: String::new(),
            mail_analysis: false,
            partial: String::new(),
        };
        let mut wrong = status.clone();
        wrong.client_request_id = "another".into();
        assert!(task.apply_status(wrong).is_err());
        task.apply_status(status.clone()).unwrap();
        let mut old = status;
        old.state = "queued".into();
        task.apply_status(old).unwrap();
        assert_eq!(task.remote.unwrap().state, "failed");
    }
}
