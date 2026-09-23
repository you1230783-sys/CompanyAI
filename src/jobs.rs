//! 0.5 桌面契約：能力、附件工作、持久任務與串流。
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
    pub result: Option<Value>,
    /// 相容網站將結構化回覆放在任務最外層的形式；舊任務缺少此欄位仍可讀取。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_payload_json: Option<Value>,
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
        if self.state == "completed" {
            self.reply()?;
        }
        Ok(())
    }
    /// 背景與串流完成後共用同一結果解析；外層 payload 與 result 皆保留供備援。
    pub fn reply(&self) -> AppResult<Message> {
        protocol::assistant_message(&json!({
            "response_payload_json": self.response_payload_json,
            "result": self.result,
        }))
    }
    /// 標題與 Outlook 工具初篩只需要正文，不能將顯示用章節混入其專用協定。
    pub fn reply_text(&self) -> AppResult<String> {
        let message = self.reply()?;
        Ok(match message.response_payload {
            Some(payload) => payload.answer,
            None => message.content,
        })
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
    #[serde(default)]
    pub title_generation: bool,
    #[serde(default)]
    pub tool_events: Vec<ToolStatus>,
    #[serde(default)]
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
    /// 僅清除已成功完成且已寫入對話的本機任務卡；失敗、取消及尚未套用回覆者保留。
    pub fn remove_completed(&mut self) -> usize {
        let before = self.tasks.len();
        self.tasks.retain(|task| {
            !(task.applied
                && task
                    .remote
                    .as_ref()
                    .is_some_and(|status| status.state == "completed"))
        });
        before - self.tasks.len()
    }
    pub fn pending(&self, id: Option<&str>) -> bool {
        id.is_some_and(|id| {
            self.tasks
                .iter()
                .any(|t| t.conversation_id == id && !t.title_generation && t.active())
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

/// 中斷時保存已收到的文字。與正式回答使用同一 request_id，之後可原位換成完整結果。
pub fn retain_partial(archive: &mut crate::history::Archive, task: &Task) -> AppResult<bool> {
    if task.partial.trim().is_empty() {
        return Ok(false);
    }
    let conversation = archive
        .conversations
        .iter_mut()
        .find(|c| c.id == task.conversation_id)
        .ok_or("找不到部分回覆對應的對話；收到的文字仍保留在任務。")?;
    let user_index = conversation
        .messages
        .iter()
        .position(|m| m.role == "user" && m.request_id.as_deref() == Some(&task.request_id))
        .ok_or("部分回覆缺少對應的使用者訊息。")?;
    let mut message = Message::assistant(task.partial.clone());
    message.request_id = Some(task.request_id.clone());
    message.incomplete = true;
    if let Some(existing) = conversation
        .messages
        .iter_mut()
        .find(|m| m.role == "assistant" && m.request_id.as_deref() == Some(&task.request_id))
    {
        if !existing.incomplete || existing.content.len() >= message.content.len() {
            return Ok(false);
        }
        *existing = message;
    } else {
        conversation.messages.insert(user_index + 1, message);
    }
    conversation.updated_at = crate::unix_now();
    Ok(true)
}
/// 將權威完成結果投影到本機歷史；request_id 保證重播不重複，並取代先前的部分回覆。
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
    if conversation.messages.iter().any(|m| {
        m.role == "assistant" && !m.incomplete && m.request_id.as_deref() == Some(&task.request_id)
    }) {
        return Ok(false);
    }
    if !conversation
        .messages
        .iter()
        .any(|m| m.role == "user" && m.request_id.as_deref() == Some(&task.request_id))
    {
        return Err("任務缺少對應的使用者訊息，不會加入其他對話。".into());
    }
    let mut message = remote.reply()?;
    if task.mail_analysis {
        message = Message::assistant(crate::outlook::format_analysis(remote.reply_text()?));
    } else {
        // REST 的明確欄位優先；缺欄位可從同一 request 的串流補回，其他原文另存供展開。
        protocol::preserve_received_reply(&mut message, &task.partial, true);
    }
    message.request_id = Some(task.request_id.clone());
    if let Some(existing) = conversation
        .messages
        .iter_mut()
        .find(|m| m.role == "assistant" && m.request_id.as_deref() == Some(&task.request_id))
    {
        *existing = message;
    } else {
        conversation.messages.push(message);
    }
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
    if !matches!(mode, "stream" | "background") {
        return Err("聊天僅支援一般（串流）或背景處理。".into());
    }
    let mut value: Value = serde_json::from_str(&protocol::chat_json(model, messages)?)
        .map_err(|_| "聊天資料不正確。")?;
    value["conversation_id"] = json!(conversation);
    value["client_request_id"] = json!(request_id);
    value["execution_mode"] = json!(mode);
    value["stream"] = json!(mode == "stream");
    value["attachment_tokens"] = json!(tokens);
    set_purpose(&mut value, false, false)?;
    Ok(value)
}

/// 用途旗標互斥：Outlook 初篩與標題產生不可同時啟用。
pub fn set_purpose(request: &mut Value, outlook: bool, title: bool) -> AppResult<()> {
    if outlook && title {
        return Err("Outlook 初篩不可同時要求產生對話標題。".into());
    }
    request["outlook_triage"] = json!(outlook);
    request["auto_generate_title"] = json!(title);
    Ok(())
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
                // 網站具名 done 可以沒有 data；與 OpenAI [DONE] 都是正常結束。
                if matches!(self.event.as_str(), "done" | "complete" | "completed") {
                    self.done = true;
                } else if self.event == "start" && self.data.is_empty() {
                    emit("start", "{}")?;
                } else if !self.data.is_empty() {
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
/// 工具狀態僅用來呈現進度，忽略 arguments／result，不把事件轉成本機工具指令。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolStatus {
    pub tool_name: String,
    pub status: String,
}
impl ToolStatus {
    fn from_event(value: Value) -> Option<Self> {
        let result: Self = serde_json::from_value(value).ok()?;
        if result.tool_name.trim().is_empty()
            || result.tool_name.len() > 200
            || result.status.trim().is_empty()
            || result.status.len() > 80
        {
            return None;
        }
        Some(result)
    }
}
pub enum StreamUpdate {
    Status(Box<TaskStatus>),
    Delta(String),
    Started,
    Tool(ToolStatus),
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
                "start" => update(StreamUpdate::Started),
                "tool_status" => {
                    if let Some(status) = ToolStatus::from_event(value) {
                        update(StreamUpdate::Tool(status));
                    }
                }
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
                    // 網站的具名 delta 使用 text；也保留 OpenAI 相容 delta.content。
                    let text = if event == "delta" {
                        value["text"].as_str()
                    } else {
                        None
                    }
                    .or_else(|| value["choices"][0]["delta"]["content"].as_str());
                    if let Some(text) = text {
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
    fn old_timing_fields_do_not_break_task_or_attachment_recovery() {
        // 舊後端與已保存紀錄可能仍帶 timing；移除功能後忽略額外欄位，
        // 實際的狀態、進度及排隊順位仍須正常還原，不必遷移或清除工作。
        let legacy = json!({
            "task_id":"task_old", "client_request_id":"request_old",
            "job_id":"attachment_old", "state":"queued", "progress":25,
            "queue_position":3,
            "timing":{"estimated_total_seconds":60,"confidence":"medium"}
        });
        let task: TaskStatus = serde_json::from_value(legacy.clone()).unwrap();
        task.validate().unwrap();
        assert_eq!(task.progress, Some(25.0));
        assert_eq!(task.queue_position, Some(3));
        assert!(serde_json::to_value(task).unwrap().get("timing").is_none());
        let attachment: AttachmentStatus = serde_json::from_value(legacy).unwrap();
        attachment.validate().unwrap();
        assert_eq!(attachment.progress, Some(25.0));
        assert_eq!(attachment.queue_position, Some(3));
        assert!(serde_json::to_value(attachment)
            .unwrap()
            .get("timing")
            .is_none());
    }

    #[test]
    fn purpose_flags_are_exclusive_and_chat_modes_stay_consistent() {
        let mut value = chat_request(
            "fast",
            &[Message::user("問題")],
            "conversation",
            "request",
            "stream",
            vec![],
        )
        .unwrap();
        assert_eq!(value["stream"], true);
        assert_eq!(value["execution_mode"], "stream");
        set_purpose(&mut value, true, false).unwrap();
        assert_eq!(value["outlook_triage"], true);
        assert_eq!(value["auto_generate_title"], false);
        assert!(set_purpose(&mut value, true, true).is_err());
        let background = chat_request(
            "fast",
            &[Message::user("問題")],
            "conversation",
            "request",
            "background",
            vec![],
        )
        .unwrap();
        assert_eq!(background["stream"], false);
        assert!(chat_request("fast", &[], "conversation", "request", "sync", vec![]).is_err());
    }
    #[test]
    fn completion_aliases_do_not_require_a_later_done_marker() {
        for event in ["done", "complete", "completed"] {
            let mut parser = SseDecoder::default();
            parser
                .push(
                    format!("event: {event}\ndata: {{\"event\":\"{event}\"}}\n\n").as_bytes(),
                    |_, _| Ok(()),
                )
                .unwrap();
            assert!(parser.done);
        }
    }
    #[test]
    fn named_start_and_done_need_no_data_and_ignore_trailing_bytes() {
        let mut parser = SseDecoder::default();
        let mut found = Vec::new();
        parser.push(b"event: start\n\nevent: delta\ndata: {\"text\":\"hello\"}\n\nevent: done\n\ndata: invalid\n\n", |kind, data| {
            found.push((kind.to_string(), data.to_string()));
            Ok(())
        }).unwrap();
        assert!(parser.done);
        assert_eq!(
            found,
            [
                ("start".into(), "{}".into()),
                ("delta".into(), r#"{"text":"hello"}"#.into())
            ]
        );
    }

    #[test]
    fn interrupted_reply_survives_reload_and_full_result_replaces_it_once() {
        let mut archive = crate::history::Archive::default();
        let mut user = Message::user("question");
        user.request_id = Some("request1".into());
        let conversation_id = archive.insert(vec![user]).unwrap();
        let mut task = Task {
            request_id: "request1".into(),
            conversation_id,
            request: json!({}),
            mode: "stream".into(),
            title: "question".into(),
            created_at: 0,
            remote: None,
            applied: false,
            message: String::new(),
            mail_analysis: false,
            title_generation: false,
            tool_events: Vec::new(),
            partial: "已收到的部分".into(),
        };
        assert!(retain_partial(&mut archive, &task).unwrap());
        assert!(!retain_partial(&mut archive, &task).unwrap());
        task.partial.push_str("，更多文字");
        assert!(retain_partial(&mut archive, &task).unwrap());
        // 模擬重新讀取加密保存前的 JSON，部分文字與不完整旗標必須一起保留。
        let mut archive: crate::history::Archive =
            serde_json::from_str(&serde_json::to_string(&archive).unwrap()).unwrap();
        let restored_task: Task =
            serde_json::from_str(&serde_json::to_string(&task).unwrap()).unwrap();
        assert_eq!(restored_task.partial, task.partial);
        assert!(archive.conversations[0].messages[1].incomplete);
        assert_eq!(archive.conversations[0].messages[1].content, task.partial);
        let mut wrong = task.clone();
        wrong.request_id = "another_request".into();
        assert!(retain_partial(&mut archive, &wrong).is_err());
        task.remote = Some(TaskStatus {
            task_id: "task1".into(),
            client_request_id: task.request_id.clone(),
            state: "completed".into(),
            progress: None,
            queue_position: None,
            result: Some(
                json!({"choices":[{"message":{"role":"assistant","content":"完整答案"}}]}),
            ),
            response_payload_json: None,
            error_message: String::new(),
        });
        assert!(apply_reply(&mut archive, &task).unwrap());
        assert!(!apply_reply(&mut archive, &task).unwrap());
        assert!(!retain_partial(&mut archive, &task).unwrap());
        assert_eq!(archive.conversations[0].messages.len(), 2);
        let completed = &archive.conversations[0].messages[1];
        assert_eq!(
            completed.response_payload.as_ref().unwrap().answer,
            "完整答案"
        );
        assert_eq!(completed.received_replies[0].text, task.partial);
        assert!(completed.content.contains(&task.partial));
        assert!(!archive.conversations[0].messages[1].incomplete);
        let old_message: Message =
            serde_json::from_value(json!({"role":"assistant","content":"舊答案"})).unwrap();
        assert!(!old_message.incomplete);
    }
    #[test]
    fn background_and_stream_results_preserve_structured_sections_after_reload() {
        let payload: Value =
            serde_json::from_str(include_str!("../ui/fixtures/structured-reply.json")).unwrap();
        for mode in ["background", "stream"] {
            let mut user = Message::user("test");
            user.request_id = Some("structured-request".into());
            let mut archive = crate::history::Archive::default();
            let conversation_id = archive.insert(vec![user]).unwrap();
            let mut task = Task {
                request_id: "structured-request".into(),
                conversation_id: conversation_id.clone(),
                request: json!({}),
                mode: mode.into(),
                title: "test".into(),
                created_at: 0,
                remote: None,
                applied: false,
                message: String::new(),
                mail_analysis: false,
                title_generation: false,
                tool_events: Vec::new(),
                partial: "串流中的正文".into(),
            };
            if mode == "stream" {
                assert!(retain_partial(&mut archive, &task).unwrap());
            }
            let mut status = json!({
                "task_id":"structured-task", "client_request_id":task.request_id,
                "state":"completed", "result":{"choices":[{"message":{"content":"僅有正文"}}]}
            });
            if mode == "background" {
                status["result"]["response_payload_json"] = payload.clone();
            } else {
                status["response_payload_json"] = json!(payload.to_string());
            }
            let remote: TaskStatus = serde_json::from_value(status).unwrap();
            remote.validate().unwrap();
            task.apply_status(remote).unwrap();
            assert!(apply_reply(&mut archive, &task).unwrap());
            assert!(!apply_reply(&mut archive, &task).unwrap());
            let root = std::env::temp_dir().join(format!("lm-ai-structured-{conversation_id}"));
            crate::history::save(&root, &archive).unwrap();
            let restored = crate::history::load(&root).unwrap();
            let messages = &restored.conversations[0].messages;
            assert_eq!(messages.len(), 2);
            assert!(!messages[1].incomplete);
            assert_eq!(
                messages[1]
                    .response_payload
                    .as_ref()
                    .unwrap()
                    .sections
                    .key_points
                    .len(),
                2
            );
            assert!(messages[1].content.contains("High (高)"));
            assert!(messages[1]
                .content
                .contains("目前沒有具體的技術問題需要解答。"));
            // 只移除本測試在隨機目錄建立的單一檔案及空目錄。
            fs::remove_file(root.join("history.dpapi")).unwrap();
            fs::remove_dir(root).unwrap();
        }
    }

    #[test]
    fn body_only_completion_preserves_inline_sections_through_encrypted_history() {
        let original = include_str!("../ui/fixtures/inline-reply.txt");
        let answer = "您好！這是一個測試訊息。我已準備好為您提供協助。";
        for mode in ["background", "stream"] {
            let mut user = Message::user("test");
            user.request_id = Some("inline-request".into());
            let mut archive = crate::history::Archive::default();
            let conversation_id = archive.insert(vec![user]).unwrap();
            let mut task: Task = serde_json::from_value(json!({
                "request_id":"inline-request", "conversation_id":conversation_id,
                "request":{}, "mode":mode, "title":"test", "created_at":0,
                "remote":null, "applied":false,
                "partial":if mode == "stream" { original } else { "" }
            }))
            .unwrap();
            if mode == "stream" {
                assert!(retain_partial(&mut archive, &task).unwrap());
            }
            // 背景原文在 result，串流原文在 partial；完成 payload 都只有正文。
            let remote: TaskStatus = serde_json::from_value(json!({
                "task_id":"inline-task", "client_request_id":"inline-request", "state":"completed",
                "response_payload_json":{"answer":answer,"sections":{}},
                "result":{"choices":[{"message":{"content":if mode == "background" { original } else { answer }}}]}
            })).unwrap();
            remote.validate().unwrap();
            task.apply_status(remote).unwrap();
            assert!(apply_reply(&mut archive, &task).unwrap());
            assert!(!apply_reply(&mut archive, &task).unwrap());
            let root = std::env::temp_dir().join(format!("lm-ai-inline-{conversation_id}"));
            crate::history::save(&root, &archive).unwrap();
            let restored = crate::history::load(&root).unwrap();
            let messages = &restored.conversations[0].messages;
            assert_eq!(messages.len(), 2);
            let reply = messages[1].response_payload.as_ref().unwrap();
            assert_eq!(reply.answer, answer);
            assert_eq!(reply.sections.key_points.len(), 2);
            assert_eq!(reply.sections.confidence.as_deref(), Some("100%"));
            assert!(messages[1].content.contains("未涉及任何實際的文件分析"));
            assert!(messages[1].received_replies.is_empty());
            fs::remove_file(root.join("history.dpapi")).unwrap();
            fs::remove_dir(root).unwrap();
        }
    }

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
            result: None,
            response_payload_json: None,
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
            title_generation: false,
            tool_events: Vec::new(),
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

    #[test]
    fn tool_status_keeps_dynamic_names_and_ignores_arguments_and_results() {
        let status = ToolStatus::from_event(json!({"event":"tool_status","tool_name":"future_tool_v2","status":"started","arguments":{"query":"private"},"result":{"session_id":"private"}})).unwrap();
        let rendered = serde_json::to_string(&status).unwrap();
        assert!(rendered.contains("future_tool_v2"));
        assert!(!rendered.contains("private"));
        assert!(ToolStatus::from_event(json!({"tool_name":[],"status":"started"})).is_none());
        assert!(
            ToolStatus::from_event(json!({"tool_name":"t".repeat(201),"status":"started"}))
                .is_none()
        );
    }

    #[test]
    fn clearing_cards_requires_completed_reply_already_applied() {
        let mut store = WorkStore::default();
        for (index, (state, applied)) in [
            ("completed", true),
            ("completed", false),
            ("failed", true),
            ("cancelled", true),
            ("running", false),
        ]
        .into_iter()
        .enumerate()
        {
            store.tasks.push(Task {
                request_id: index.to_string(),
                conversation_id: "chat".into(),
                request: json!({}),
                mode: "stream".into(),
                title: "t".into(),
                created_at: 0,
                applied,
                message: String::new(),
                mail_analysis: false,
                title_generation: false,
                tool_events: Vec::new(),
                partial: String::new(),
                remote: Some(TaskStatus {
                    task_id: index.to_string(),
                    client_request_id: index.to_string(),
                    state: state.into(),
                    progress: None,
                    queue_position: None,
                    result: None,
                    response_payload_json: None,
                    error_message: String::new(),
                }),
            });
        }
        assert_eq!(store.remove_completed(), 1);
        assert_eq!(
            store
                .tasks
                .iter()
                .map(|task| task.request_id.as_str())
                .collect::<Vec<_>>(),
            ["1", "2", "3", "4"]
        );
        assert_eq!(store.remove_completed(), 0);
    }
}
