//! --demo 的 0.5 模擬契約；資料僅保存在記憶體，不能當成正式背景 worker。
use super::*;
use serde_json::Value;
#[cfg(test)]
mod tests;
#[derive(Default)]
pub(super) struct DemoWork {
    conversations: HashMap<(String, String), String>,
    files: HashMap<String, FileJob>,
    tasks: HashMap<String, ChatJob>,
}
struct FileJob {
    owner: String,
    local: String,
    conversation: String,
    size: usize,
    state: String,
    polls: usize,
    digest: String,
}
struct ChatJob {
    owner: String,
    request: Value,
    state: String,
    polls: usize,
    answer: String,
}
fn timing() -> Value {
    json!({"estimated_wait_seconds":2,"estimated_processing_seconds":4,"estimated_total_seconds":6,"sample_count":12,"confidence":"medium","generated_at":crate::notifications::now_text()})
}
fn attachment(id: &str, file: &FileJob) -> Value {
    json!({"job_id":id,"state":file.state,"progress":if file.state=="ready"{Some(100)}else{None},"queue_position":if file.state=="queued"{Some(1)}else{None},"attachment_token":if file.state=="ready"{Some(format!("attachment_{id}"))}else{None},"expires_at":crate::unix_now()+3600,"timing":timing(),"error_message":""})
}
fn task(id: &str, task: &ChatJob) -> Value {
    json!({"task_id":id,"client_request_id":task.request["client_request_id"],"state":task.state,"queue_position":if task.state=="queued"{Some(1)}else{None},"progress":null,"timing":timing(),"error_message":"","result":if task.state=="completed"{json!({"choices":[{"message":{"role":"assistant","content":task.answer}}]})}else{Value::Null}})
}
fn write_reply(stream: &mut TcpStream, status: u32, mime: &str, body: &str) -> AppResult<()> {
    stream.write_all(format!("HTTP/1.1 {status} Result\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).map_err(|e|e.to_string())
}
pub(super) fn serve_work(
    stream: &mut TcpStream,
    method: &str,
    route: &str,
    body: &[u8],
    owner: Option<&str>,
    state: &mut DemoWork,
    model: &str,
) -> AppResult<bool> {
    let prefix = crate::jobs::PREFIX;
    let json_body: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
    let enhanced_chat = route == CHAT_PATH && json_body.get("execution_mode").is_some();
    let relevant = enhanced_chat
        || route == format!("{prefix}/capabilities")
        || route.starts_with(&format!("{prefix}/conversations"))
        || route.starts_with(&format!("{prefix}/attachments"))
        || route.starts_with(&format!("{prefix}/tasks"))
        || route == format!("{prefix}/chat/estimate");
    if !relevant {
        return Ok(false);
    }
    let Some(owner) = owner else {
        write_reply(
            stream,
            401,
            "application/json",
            "{\"error\":{\"message\":\"Please log in\"}}",
        )?;
        return Ok(true);
    };
    let mut code = 200;
    let result = if method == "GET" && route == format!("{prefix}/capabilities") {
        let extensions = if model == "text_only" {
            vec![".pdf", ".docx", ".txt", ".md", ".msg"]
        } else {
            vec![
                ".pdf", ".docx", ".xlsx", ".pptx", ".txt", ".md", ".csv", ".png", ".jpg", ".jpeg",
                ".webp", ".msg",
            ]
        };
        json!({"contract_version":1,"principal_id":format!("demo_{owner}"),"execution_modes":["sync","stream","background"],"timing_estimates":true,"attachments":{"enabled":true,"max_count":20,"max_file_bytes":10485760,"max_total_bytes":52428800,"allowed_extensions":extensions,"allowed_mime_types":[]}})
    } else if method == "POST" && route == format!("{prefix}/conversations") {
        let local = json_body["client_conversation_id"]
            .as_str()
            .ok_or("缺少 client_conversation_id。")?;
        crate::jobs::validate_id(local)?;
        let id = state
            .conversations
            .entry((owner.into(), local.into()))
            .or_insert_with(|| format!("conversation_{local}"));
        json!({"conversation_id":id})
    } else if method == "POST" && route == format!("{prefix}/chat/estimate") {
        timing()
    } else if method == "POST"
        && route.starts_with(&format!("{prefix}/conversations/"))
        && route.ends_with("/attachments")
    {
        let conversation = route
            .trim_start_matches(&format!("{prefix}/conversations/"))
            .trim_end_matches("/attachments");
        if !state
            .conversations
            .iter()
            .any(|((o, _), c)| o == owner && c == conversation)
        {
            code = 404;
            json!({"error":{"message":"Unknown conversation"}})
        } else {
            let local = json_body["client_attachment_id"]
                .as_str()
                .ok_or("缺少附件 ID。")?;
            let existing = state
                .files
                .iter()
                .find(|(_, f)| f.owner == owner && f.local == local)
                .map(|(id, _)| id.clone());
            let id = existing.unwrap_or(crate::jobs::new_id()?);
            let file = state.files.entry(id.clone()).or_insert(FileJob {
                owner: owner.into(),
                local: local.into(),
                conversation: conversation.into(),
                size: json_body["size_bytes"].as_u64().unwrap_or(0) as usize,
                state: "awaiting_upload".into(),
                polls: 0,
                digest: String::new(),
            });
            code = 201;
            attachment(&id, file)
        }
    } else if route.starts_with(&format!("{prefix}/attachments/")) {
        let tail = route.trim_start_matches(&format!("{prefix}/attachments/"));
        let (id, action) = tail.split_once('/').unwrap_or((tail, ""));
        if let Some(file) = state.files.get_mut(id).filter(|f| f.owner == owner) {
            if method == "PUT" && action == "content" {
                use sha2::Digest;
                file.digest = format!("{:x}", sha2::Sha256::digest(body));
                if body.len() != file.size {
                    code = 400;
                    json!({"error":{"message":"Wrong attachment size"}})
                } else {
                    if file.state == "awaiting_upload" {
                        file.state = "queued".into();
                    }
                    attachment(id, file)
                }
            } else if method == "POST" && action == "cancel" {
                file.state = "cancelled".into();
                attachment(id, file)
            } else {
                if matches!(file.state.as_str(), "queued" | "processing") {
                    file.polls += 1;
                    file.state = if file.polls >= 2 {
                        "ready"
                    } else {
                        "processing"
                    }
                    .into();
                }
                attachment(id, file)
            }
        } else {
            code = 404;
            json!({"error":{"message":"Unknown attachment"}})
        }
    } else if enhanced_chat && method == "POST" {
        let request_id = json_body["client_request_id"]
            .as_str()
            .ok_or("缺少請求 ID。")?;
        let conversation = json_body["conversation_id"].as_str().unwrap_or("");
        let valid_conversation = state
            .conversations
            .iter()
            .any(|((o, _), c)| o == owner && c == conversation);
        let tokens = json_body["attachment_tokens"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let valid_tokens = tokens.len() <= 20
            && tokens.iter().all(|token| {
                token
                    .as_str()
                    .and_then(|t| t.strip_prefix("attachment_"))
                    .and_then(|id| state.files.get(id))
                    .is_some_and(|f| {
                        f.owner == owner && f.conversation == conversation && f.state == "ready"
                    })
            });
        if !valid_conversation || !valid_tokens {
            code = 403;
            json!({"error":{"message":"Attachment or conversation is not ready/owned"}})
        } else {
            let existing = state
                .tasks
                .iter()
                .find(|(_, t)| t.owner == owner && t.request["client_request_id"] == request_id)
                .map(|(id, _)| id.clone());
            let id = existing.unwrap_or(crate::jobs::new_id()?);
            let chat = state.tasks.entry(id.clone()).or_insert_with(|| ChatJob {
                owner: owner.into(),
                request: json_body.clone(),
                state: "queued".into(),
                polls: 0,
                answer: format!(
                    "【本機模擬，非真實 AI】\n\n已接收 {} 個附件。\n\n{}\n\n**任務流程測試完成。**",
                    tokens.len(),
                    json_body["messages"]
                        .as_array()
                        .and_then(|m| m.last())
                        .and_then(|m| m["content"].as_str())
                        .unwrap_or("")
                ),
            });
            if json_body["execution_mode"] == "stream" {
                chat.state = "running".into();
                let initial = task(&id, chat);
                chat.state = "completed".into();
                let final_status = task(&id, chat);
                let mut sse = format!("event: task\ndata: {initial}\n\n: heartbeat\n\n");
                sse.push_str("event: start\ndata: {}\n\nevent: tool_status\ndata: {\"event\":\"tool_status\",\"tool_name\":\"search_session_documents\",\"status\":\"started\",\"arguments\":{\"query\":\"demo\"}}\n\n");
                for (index, chunk) in chat
                    .answer
                    .chars()
                    .collect::<Vec<_>>()
                    .chunks(5)
                    .enumerate()
                {
                    let text: String = chunk.iter().collect();
                    if index % 2 == 0 {
                        sse.push_str(&format!(
                            "event: delta\ndata: {}\n\n",
                            json!({"event":"delta","text":text})
                        ));
                    } else {
                        sse.push_str(&format!(
                            "data: {}\n\n",
                            json!({"choices":[{"index":0,"delta":{"content":text}}]})
                        ));
                    }
                }
                sse.push_str("event: tool_status\ndata: {\"tool_name\":\"search_session_documents\",\"status\":\"completed\",\"result\":{\"matches\":[]}}\n\n");
                sse.push_str(&format!(
                    "event: status\ndata: {final_status}\n\nevent: done\n\n"
                ));
                if chat.request["messages"][0]["content"]
                    .as_str()
                    .is_some_and(|text| text.starts_with("[demo:disconnect]"))
                {
                    if let Some(end) = sse.find("event: status") {
                        sse.truncate(end);
                    }
                }
                write_reply(stream, 200, "text/event-stream; charset=utf-8", &sse)?;
                return Ok(true);
            }
            code = 202;
            task(&id, chat)
        }
    } else if route.starts_with(&format!("{prefix}/tasks/")) {
        let tail = route.trim_start_matches(&format!("{prefix}/tasks/"));
        let (key, action) = tail.split_once('/').unwrap_or((tail, ""));
        let id = if key == "by-request" {
            state
                .tasks
                .iter()
                .find(|(_, t)| t.owner == owner && t.request["client_request_id"] == action)
                .map(|(id, _)| id.clone())
                .unwrap_or_default()
        } else {
            key.into()
        };
        if let Some(chat) = state.tasks.get_mut(&id).filter(|t| t.owner == owner) {
            if method == "POST" && action == "cancel" {
                if !matches!(chat.state.as_str(), "completed" | "failed") {
                    chat.state = "cancelled".into();
                }
            } else if matches!(chat.state.as_str(), "queued" | "running") {
                chat.polls += 1;
                chat.state = if chat.polls >= 3 {
                    "completed"
                } else {
                    "running"
                }
                .into();
            }
            task(&id, chat)
        } else {
            code = 404;
            json!({"error":{"message":"Unknown task; retry with the same client_request_id"}})
        }
    } else {
        code = 404;
        json!({"error":{"message":"Unknown desktop route"}})
    };
    write_reply(
        stream,
        code,
        "application/json; charset=utf-8",
        &result.to_string(),
    )?;
    Ok(true)
}
