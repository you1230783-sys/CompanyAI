//! desktop-agent-v1 的專用傳輸型別。一般聊天的 Message 與路由不受影響。
//! 網頁只推論一次；所有工具仍由本機 broker 驗證並執行。
mod schema;
#[cfg(test)]
mod tests;

use crate::{config::Config, jobs, storage::Session, AppResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const CONTRACT: &str = "desktop-agent-v1";
pub const PATH: &str = "/lm_server/api/desktop/agent/turns";
const CAP_PATH: &str = "/lm_server/api/desktop/agent/capabilities";
const MAX_BYTES: usize = 2 * 1024 * 1024;

/// 僅供專案上下文使用；可讀取舊 checkpoint 的 role/content 形狀。
#[derive(Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}
impl Message {
    pub fn user(content: &str) -> Self {
        crate::protocol::Message::user(content).into()
    }
    pub fn assistant(content: String) -> Self {
        crate::protocol::Message::assistant(content).into()
    }
    pub fn wire(&self) -> Value {
        let mut value = json!({"role":self.role,"content":self.content});
        if !self.tool_calls.is_empty() {
            value["tool_calls"] = json!(self.tool_calls);
            if self.content.is_empty() {
                value["content"] = Value::Null;
            }
        }
        if let Some(id) = &self.tool_call_id {
            value["tool_call_id"] = json!(id);
        }
        value
    }
    pub fn legacy(&self) -> crate::protocol::Message {
        let mut message = crate::protocol::Message::user(&self.content);
        message.role.clone_from(&self.role);
        message
    }
    pub fn result(id: &str, result: &Value) -> Self {
        Self {
            role: "tool".into(),
            content: result.to_string(),
            tool_calls: vec![],
            tool_call_id: Some(id.into()),
        }
    }
}
impl From<crate::protocol::Message> for Message {
    fn from(message: crate::protocol::Message) -> Self {
        Self {
            role: message.role,
            content: message.content,
            tool_calls: vec![],
            tool_call_id: None,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Limits {
    request_bytes: usize,
    response_bytes: usize,
    messages: usize,
    message_content_bytes: usize,
    tools: usize,
    tools_bytes: usize,
    tool_arguments_bytes: usize,
    tool_calls_per_message: usize,
    schema_depth: usize,
    metadata_bytes: usize,
    max_completion_tokens: u64,
    context_window_tokens: u64,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Capabilities {
    contract_version: String,
    pub principal_id: String,
    model: String,
    capability_revision: String,
    execution_modes: Vec<String>,
    input_content_types: Vec<String>,
    tool_types: Vec<String>,
    native_tool_calls: bool,
    strict_tool_arguments: bool,
    tool_choice_modes: Vec<String>,
    parallel_tool_calls: bool,
    response_formats: Vec<String>,
    strict_response_schema: bool,
    schema_profiles: Vec<String>,
    optional_request_fields: Vec<String>,
    extensions: Vec<String>,
    limits: Limits,
    default_max_completion_tokens: u64,
    retention: Value,
    notification_policy: String,
}
impl Capabilities {
    pub fn validate(&self, model: &str, tools: bool) -> AppResult<()> {
        jobs::validate_id(&self.principal_id)?;
        if self.contract_version != CONTRACT
            || self.model != model
            || self.capability_revision.is_empty()
            || self.capability_revision.len() > 256
            || !self.execution_modes.iter().any(|s| s == "background")
            || !self.input_content_types.iter().any(|s| s == "text")
            || !self.response_formats.iter().any(|s| s == "text")
            || self.notification_policy != "desktop_only"
            || self.retention["result_days"].as_u64().unwrap_or(0) < 30
            || self.retention["idempotency"] != "account_lifetime"
            || self.default_max_completion_tokens == 0
            || self.default_max_completion_tokens > self.limits.max_completion_tokens
            || self.limits.context_window_tokens <= self.default_max_completion_tokens
            || [
                self.limits.request_bytes,
                self.limits.response_bytes,
                self.limits.messages,
                self.limits.message_content_bytes,
                self.limits.tool_arguments_bytes,
                self.limits.schema_depth,
                self.limits.metadata_bytes,
            ]
            .contains(&0)
        {
            return Err("網站代理能力與 desktop-agent-v1 契約不符，未改走舊路由。".into());
        }
        let choice = if tools { "required" } else { "none" };
        if !self.tool_choice_modes.iter().any(|s| s == choice)
            || (tools
                && (!self.native_tool_calls
                    || !self.tool_types.iter().any(|s| s == "function")
                    || self.limits.tool_calls_per_message == 0
                    || !self
                        .schema_profiles
                        .iter()
                        .any(|s| s == "lmai-json-schema-v1")))
        {
            return Err("目前模型尚未提供所需的原生工具能力，未改走文字工具協定。".into());
        }
        Ok(())
    }
    pub fn mode_label(&self) -> &'static str {
        if self.strict_tool_arguments {
            "原生工具／嚴格結構約束"
        } else {
            "原生工具／非嚴格參數模式"
        }
    }
}

pub fn capabilities(config: &Config, session: &Session, tools: bool) -> AppResult<Capabilities> {
    let mut url = config.endpoint(CAP_PATH)?;
    url.query_pairs_mut().append_pair("model", &config.model);
    if !session.valid_for(config) {
        return Err("登入已到期，請重新登入。".into());
    }
    let response = crate::transport::get(
        &url,
        Some(("Authorization", &format!("Bearer {}", session.access_token))),
    )?;
    let caps: Capabilities = decode(response, session)
        .map_err(|e| format!("代理能力查詢未成功；網站須先部署 desktop-agent-v1：{e}"))?;
    caps.validate(&config.model, tools)?;
    Ok(caps)
}

/// checkpoint 保存協定及遞增序號；委派與主模型共用序號，但各自核對能力。
#[derive(Clone, Serialize, Deserialize)]
pub struct State {
    pub caps: Capabilities,
    next_turn: u64,
    pub parent: Option<String>,
}
impl State {
    pub fn new(caps: Capabilities) -> Self {
        Self {
            caps,
            next_turn: 1,
            parent: None,
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn request(
        &mut self,
        caps: &Capabilities,
        run: &super::runner::Run,
        conversation: &str,
        id: &str,
        messages: &[Message],
        tools: bool,
        parent: Option<&str>,
    ) -> AppResult<Value> {
        jobs::validate_id(&run.id)?;
        jobs::validate_id(&run.project.id)?;
        if let Some(parent) = parent {
            jobs::validate_id(parent)?;
        }
        if caps.principal_id != self.caps.principal_id {
            return Err("委派與主任務帳號不同。".into());
        }
        let context = json!({"project_id":run.project.id,"run_id":run.id,
            "turn_index":self.next_turn,"parent_request_id":parent,"context_policy":"client_snapshot"});
        let request = build_request(caps, conversation, id, context, messages, tools)?;
        self.next_turn = self.next_turn.checked_add(1).ok_or("代理序號溢位。")?;
        Ok(request)
    }
}

fn build_request(
    caps: &Capabilities,
    conversation: &str,
    id: &str,
    context: Value,
    messages: &[Message],
    use_tools: bool,
) -> AppResult<Value> {
    jobs::validate_id(conversation)?;
    jobs::validate_id(id)?;
    caps.validate(&caps.model, use_tools)?;
    validate_history(messages)?;
    if messages.len() > caps.limits.messages.min(512)
        || messages
            .iter()
            .any(|m| m.content.len() > caps.limits.message_content_bytes.min(262144))
    {
        return Err("代理訊息超過網站公告上限；未截斷內容或提交。".into());
    }
    let mut tools = if use_tools {
        schema::definitions(caps.strict_tool_arguments)?
    } else {
        vec![]
    };
    // 委派只提供給品質模型；快速模型直接使用閱讀工具，避免呼叫自己再撞權限。
    if caps.model != "quality" {
        tools.retain(|tool| tool["function"]["name"] != "summarize_document");
    }
    if use_tools
        && (tools.len() > caps.limits.tools.min(128)
            || serde_json::to_vec(&tools).map_err(|e| e.to_string())?.len()
                > caps.limits.tools_bytes.min(524288)
            || tools.iter().any(|t| {
                schema::depth(&t["function"]["parameters"]) > caps.limits.schema_depth.min(32)
            }))
    {
        return Err("工具定義超過網站能力上限；未刪減工具或提交。".into());
    }
    let value = json!({"contract_version":CONTRACT,"capability_revision":caps.capability_revision,
        "model":caps.model,"conversation_id":conversation,"client_request_id":id,"context":context,
        "messages":messages.iter().map(Message::wire).collect::<Vec<_>>(),"tools":tools,
        "tool_choice":if use_tools {"required"} else {"none"},"parallel_tool_calls":false,
        "stream":false,"execution_mode":"background","skills":false,"metadata":{},"extensions":{}});
    if serde_json::to_vec(&value).map_err(|e| e.to_string())?.len()
        > caps.limits.request_bytes.min(MAX_BYTES)
    {
        return Err("代理請求超過網站公告大小；未截斷或提交。".into());
    }
    Ok(value)
}

fn validate_history(messages: &[Message]) -> AppResult<()> {
    let mut pending: Option<&str> = None;
    for (index, message) in messages.iter().enumerate() {
        if let Some(id) = pending.take() {
            if message.role != "tool"
                || message.tool_call_id.as_deref() != Some(id)
                || !message.tool_calls.is_empty()
            {
                return Err("原生工具歷史未配對，未提交。".into());
            }
            continue;
        }
        if message.tool_call_id.is_some()
            || !matches!(message.role.as_str(), "system" | "user" | "assistant")
            || (message.role == "system" && index != 0)
        {
            return Err("代理訊息角色或位置不符合契約。".into());
        }
        if !message.tool_calls.is_empty() {
            if message.role != "assistant" || message.tool_calls.len() != 1 {
                return Err("每輪只接受一個原生工具。".into());
            }
            let call = &message.tool_calls[0];
            validate_call(call)?;
            pending = call["id"].as_str();
        }
    }
    if pending.is_some() {
        return Err("原生工具尚無結果，未提交新推論。".into());
    }
    Ok(())
}

/// 僅接收原生工具封裝的契約欄位，供應商附加資訊不送入歷史或執行器。
/// arguments 是實際操作要求，必須完整保留，後續仍依工具 Schema 驗證。
fn incoming_call(call: &Value) -> AppResult<Value> {
    let normalized = json!({
        "id": call["id"],
        "type": call["type"],
        "function": {
            "name": call["function"]["name"],
            "arguments": call["function"]["arguments"],
        },
    });
    validate_call(&normalized)?;
    Ok(normalized)
}

/// 內部歷史只保存正規化後的工具封裝，避免後續請求重新帶出未知欄位。
fn validate_call(call: &Value) -> AppResult<()> {
    let id = call["id"].as_str().ok_or("工具缺少 call ID。")?;
    let name = call["function"]["name"]
        .as_str()
        .ok_or("工具缺少 function.name。")?;
    if id.is_empty()
        || id.len() > 128
        || !id.bytes().all(|b| (33..=126).contains(&b))
        || name.is_empty()
        || name.len() > 64
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        || call["type"] != "function"
        || !call["function"]["arguments"].is_string()
        || call.as_object().is_none_or(|m| {
            m.keys()
                .any(|k| !["id", "type", "function"].contains(&k.as_str()))
        })
        || call["function"].as_object().is_none_or(|m| {
            m.keys()
                .any(|k| !["name", "arguments"].contains(&k.as_str()))
        })
    {
        return Err("原生工具呼叫形狀不符合契約，未執行。".into());
    }
    Ok(())
}

/// 固定序列化陣列保留各 ID 邊界，避免跨輪重用 call_001 與字串串接碰撞。
pub fn operation_key(principal: &str, run: &str, request: &str, call: &str) -> String {
    format!(
        "{:x}",
        Sha256::digest(
            json!([principal, run, request, call])
                .to_string()
                .as_bytes()
        )
    )
}

pub enum Parsed {
    Operation {
        parsed: Box<super::reply::Parsed>,
        message: Message,
        call_id: String,
    },
    Repair {
        reason: String,
        message: Option<Message>,
        call_id: Option<String>,
    },
    Text(String),
}

/// 僅讀取原生回覆的已知欄位，其餘供應商資訊忽略；必要欄位仍須存在且合法。
/// 絕不搜尋 content 中的 JSON 或修補引號／括號。
pub fn parse(
    status: &jobs::TaskStatus,
    task: &jobs::Task,
    principal: &str,
    run: &str,
) -> AppResult<Parsed> {
    validate_status(status, task)?;
    let result = status.result.as_ref().ok_or("原生回覆缺少 result。")?;
    if result["object"] != "chat.completion"
        || !result["id"].is_string()
        || result["created"].as_u64().is_none()
        || result["model"] != task.request["model"]
    {
        return Err("原生推論結果的型別、模型或識別資訊不符合契約。".into());
    }
    let choices = result["choices"]
        .as_array()
        .ok_or("原生回覆缺少 choices。")?;
    if choices.len() != 1 || choices[0]["index"] != 0 {
        return Err("原生回覆必須只有 choice 0。".into());
    }
    let choice = &choices[0];
    let value = &choice["message"];
    if value["role"] != "assistant"
        || value.get("content").is_none()
        || (!value["content"].is_null() && !value["content"].is_string())
    {
        return Err("原生 assistant 訊息格式錯誤。".into());
    }
    if !value["refusal"].is_null() && value["refusal"].as_str().is_none_or(|s| !s.is_empty()) {
        return Err("模型拒絕本次請求，未執行工具。".into());
    }
    let reason = choice["finish_reason"]
        .as_str()
        .ok_or("原生回覆缺少 finish_reason。")?;
    if matches!(reason, "length" | "content_filter") {
        return Ok(Parsed::Repair {
            reason: format!(
                "模型輸出因 {reason} 未完整結束，本輪工具未執行；請縮小單次操作或結果。"
            ),
            message: None,
            call_id: None,
        });
    }
    let mut message = Message::assistant(value["content"].as_str().unwrap_or("").into());
    if let Some(calls) = value.get("tool_calls") {
        message.tool_calls = calls
            .as_array()
            .ok_or("tool_calls 不是陣列。")?
            .iter()
            .map(incoming_call)
            .collect::<AppResult<Vec<_>>>()?;
    }
    let expected_tools = task.request["tools"]
        .as_array()
        .is_some_and(|t| !t.is_empty());
    if reason == "stop" && message.tool_calls.is_empty() {
        if !expected_tools && !message.content.trim().is_empty() {
            return Ok(Parsed::Text(message.content));
        }
        return Ok(Parsed::Repair {
            reason: "本輪需要原生工具；交付請使用 finish 並提供實際正文。".into(),
            message: None,
            call_id: None,
        });
    }
    if reason != "tool_calls" || message.tool_calls.len() != 1 || !expected_tools {
        return Err("finish_reason／工具數量與本輪契約不一致，未執行任何工具。".into());
    }
    let call = &message.tool_calls[0];
    validate_call(call)?;
    let call_id = call["id"].as_str().ok_or("缺少 call ID。")?.to_owned();
    let name = call["function"]["name"].as_str().ok_or("缺少工具名稱。")?;
    let definition = task.request["tools"]
        .as_array()
        .and_then(|tools| tools.iter().find(|t| t["function"]["name"] == name))
        .ok_or("模型要求未提供的工具，已停止。")?;
    let encoded = call["function"]["arguments"].as_str().ok_or("缺少參數。")?;
    if encoded.len() > 262144 {
        return Err("工具參數超過桌面大小上限。".into());
    }
    let args = schema::decode_arguments(encoded, &definition["function"]["parameters"]);
    let args = match args {
        Ok(args) => args,
        Err(reason) => {
            if definition["function"]["strict"] == true {
                return Err(format!("嚴格工具參數不符合 Schema：{reason}"));
            }
            return Ok(Parsed::Repair {
                reason,
                message: Some(message),
                call_id: Some(call_id),
            });
        }
    };
    let operation = operation_key(principal, run, &task.request_id, &call_id);
    let arguments = schema::restore_optional(name, args)?;
    for field in ["copy_id", "revision"] {
        if arguments
            .get(field)
            .is_some_and(|v| v.as_str().is_none_or(|s| s.trim().is_empty()))
        {
            return Ok(Parsed::Repair {
                reason: format!("{field} 不可空白。修改前先 create_working_copy，使用工具成功回傳的 copy_id 與最新 revision；只要閱讀時請用 read_file，不要用 edit_text 試探。"),
                message: Some(message),
                call_id: Some(call_id),
            });
        }
    }
    let mut envelope = json!({"content":message.content,"tool_calls":[{"id":operation,"type":"function","function":{"name":name,"arguments":arguments}}]});
    // 既有 Decision 仍負責驗證執行參數；这里只轉接可信欄位，不解析模型正文。
    envelope["content"] = json!(message.content);
    match super::reply::parse(&envelope.to_string())? {
        super::reply::ParseOutcome::Operation(parsed) => Ok(Parsed::Operation {
            parsed,
            message,
            call_id,
        }),
        super::reply::ParseOutcome::Repair(reason) => Ok(Parsed::Repair {
            reason: reason.into(),
            message: Some(message),
            call_id: Some(call_id),
        }),
    }
}

pub fn validate_status(status: &jobs::TaskStatus, task: &jobs::Task) -> AppResult<()> {
    // 身分欄位逐一比對；伺服器新增的 context metadata 不影響原請求配對。
    // get() 區分欄位缺漏與合法 null（例如第一輪 parent_request_id）。
    let expected = &task.request["context"];
    let context = status.agent_envelope.get("context").unwrap_or(&Value::Null);
    let context_matches = context.is_object()
        && [
            "project_id",
            "run_id",
            "turn_index",
            "parent_request_id",
            "context_policy",
        ]
        .iter()
        .all(|key| context.get(key) == expected.get(key));
    if status
        .agent_envelope
        .get("contract_version")
        .and_then(Value::as_str)
        != Some(CONTRACT)
        || status.agent_envelope.get("conversation_id") != task.request.get("conversation_id")
        || !context_matches
        || status.client_request_id != task.request_id
        || task
            .remote
            .as_ref()
            .is_some_and(|s| s.task_id != status.task_id)
    {
        return Err("代理任務的契約／對話／請求／專案身分不一致，已停止。".into());
    }
    Ok(())
}

/// 保留錯誤碼及 accepted 狀態，網路未知與明確拒絕不混成同一種重試。
pub fn decode<T: serde::de::DeserializeOwned>(
    response: crate::transport::HttpResponse,
    session: &Session,
) -> AppResult<T> {
    if !(200..300).contains(&response.status) {
        let error: Value = serde_json::from_str(&response.body).unwrap_or(Value::Null);
        let message =
            crate::protocol::api_error(response.status, &response.body, &session.access_token);
        return Err(format!(
            "{message}；error_code={}；task_accepted={}",
            error["error_code"], error["task_accepted"]
        ));
    }
    let value = schema::decode_json(&response.body)?;
    serde_json::from_value(value).map_err(|_| "網站代理回應欄位格式錯誤。".into())
}

pub fn system_prompt() -> String {
    format!(
        "{}\n技能目錄：{}",
        include_str!("agent/skill.md"),
        super::skills::catalog()
    )
}

pub enum Submission {
    Accepted(jobs::TaskStatus),
    Rejected(String),
    Unknown(String),
}

fn bounded_decode(
    response: crate::transport::HttpResponse,
    session: &Session,
    caps: &Capabilities,
) -> AppResult<jobs::TaskStatus> {
    if response.body.len() > caps.limits.response_bytes.min(MAX_BYTES) {
        return Err("網站代理回應超過協商大小限制。".into());
    }
    decode(response, session)
}

pub fn submit(
    config: &Config,
    session: &Session,
    task: &jobs::Task,
    caps: &Capabilities,
) -> Submission {
    if !session.valid_for(config) {
        return Submission::Rejected("登入已到期，未提交代理任務。".into());
    }
    let response = config.endpoint(PATH).and_then(|url| {
        crate::transport::request(
            &url,
            "application/json",
            &task.request.to_string(),
            Some(("Authorization", &format!("Bearer {}", session.access_token))),
            30_000,
        )
    });
    match response {
        Err(e) => Submission::Unknown(e),
        Ok(response) => {
            let rejected = !(200..300).contains(&response.status)
                && schema::decode_json(&response.body).is_ok_and(|v| v["task_accepted"] == false);
            match bounded_decode(response, session, caps) {
                Ok(status) => Submission::Accepted(status),
                Err(e) if rejected => Submission::Rejected(e),
                Err(e) => Submission::Unknown(e),
            }
        }
    }
}

pub fn status(
    config: &Config,
    session: &Session,
    task: &jobs::Task,
    caps: &Capabilities,
) -> AppResult<jobs::TaskStatus> {
    if !session.valid_for(config) {
        return Err("登入已到期，重新登入後可查回原請求。".into());
    }
    let path = match &task.remote {
        Some(s) => format!("{}/tasks/{}", jobs::PREFIX, s.task_id),
        None => format!("{}/tasks/by-request/{}", jobs::PREFIX, task.request_id),
    };
    let response = crate::transport::get(
        &config.endpoint(&path)?,
        Some(("Authorization", &format!("Bearer {}", session.access_token))),
    )?;
    bounded_decode(response, session, caps)
}

pub fn check_result_limits(status: &jobs::TaskStatus, caps: &Capabilities) -> AppResult<()> {
    let result = status.result.as_ref().ok_or("缺少原生推論結果。")?;
    let message = &result["choices"][0]["message"];
    if message["content"]
        .as_str()
        .is_some_and(|s| s.len() > caps.limits.message_content_bytes.min(262144))
        || message["tool_calls"].as_array().is_some_and(|calls| {
            calls.iter().any(|c| {
                c["function"]["arguments"]
                    .as_str()
                    .is_some_and(|s| s.len() > caps.limits.tool_arguments_bytes.min(262144))
            })
        })
    {
        return Err("模型內容或工具參數超過已協商上限，未執行。".into());
    }
    Ok(())
}

/// 取消僅針對已送出的原請求；若尚無 task ID，先查回，不建立新工作。
pub fn cancel(config: &Config, session: &Session, task: &jobs::Task, caps: &Capabilities) {
    let known = task
        .remote
        .clone()
        .or_else(|| status(config, session, task, caps).ok());
    if let Some(remote) = known.filter(|s| !s.terminal()) {
        if remote.validate_envelope().is_err() || validate_status(&remote, task).is_err() {
            return;
        }
        let path = format!("{}/tasks/{}/cancel", jobs::PREFIX, remote.task_id);
        let _: AppResult<Value> = jobs::post(config, session, &path, &json!({}));
    }
}
