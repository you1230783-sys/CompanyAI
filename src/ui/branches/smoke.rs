//! 真正 UI 控制器／DPAPI／loopback HTTP 的分支驗收。只在 --self-check 啟用。
use super::*;
use crate::jobs;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::Mutex,
};

struct Fixture {
    url: String,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
    requests: Arc<Mutex<Vec<serde_json::Value>>>,
}
impl Fixture {
    fn start() -> AppResult<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let url = format!(
            "http://{}",
            listener.local_addr().map_err(|e| e.to_string())?
        );
        let stop = Arc::new(AtomicBool::new(false));
        let halted = stop.clone();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let received = requests.clone();
        let worker = std::thread::spawn(move || {
            while !halted.load(Ordering::Relaxed) {
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(20));
                    continue;
                };
                let result = (|| -> AppResult<()> {
                    // Windows accept 會沿用監聽 socket 的 nonblocking 模式。
                    // 連線建立不代表 HTTP 已到齊；此合成伺服器以有限阻塞讀完整要求，
                    // 否則 WouldBlock 會被當成失敗，造成自檢隨封包時序中斷。
                    stream.set_nonblocking(false).map_err(|e| e.to_string())?;
                    stream
                        .set_read_timeout(Some(Duration::from_secs(3)))
                        .map_err(|e| e.to_string())?;
                    stream
                        .set_write_timeout(Some(Duration::from_secs(3)))
                        .map_err(|e| e.to_string())?;
                    let mut bytes = Vec::new();
                    let mut buffer = [0u8; 4096];
                    let (header, end, length) = loop {
                        let count = stream.read(&mut buffer).map_err(|e| e.to_string())?;
                        if count == 0 || bytes.len() > 1_048_576 {
                            return Err("Invalid fixture request".into());
                        }
                        bytes.extend_from_slice(&buffer[..count]);
                        if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                            let header = String::from_utf8_lossy(&bytes[..end]).into_owned();
                            let length = header
                                .lines()
                                .find_map(|line| {
                                    line.split_once(':')
                                        .filter(|(name, _)| {
                                            name.eq_ignore_ascii_case("content-length")
                                        })
                                        .and_then(|(_, v)| v.trim().parse::<usize>().ok())
                                })
                                .unwrap_or(0);
                            break (header, end + 4, length);
                        }
                    };
                    if length > 1_048_576 {
                        return Err("Fixture body too large".into());
                    }
                    while bytes.len() < end + length {
                        let count = stream.read(&mut buffer).map_err(|e| e.to_string())?;
                        if count == 0 {
                            return Err("Fixture body incomplete".into());
                        }
                        bytes.extend_from_slice(&buffer[..count]);
                    }
                    let body: serde_json::Value =
                        serde_json::from_slice(&bytes[end..end + length]).unwrap_or_default();
                    let route = header.split_whitespace().nth(1).unwrap_or("");
                    let response = if route.contains("/capabilities") {
                        capabilities()
                    } else if route.ends_with("/conversations") {
                        json!({"conversation_id":format!("remote_{}",body["client_conversation_id"].as_str().unwrap_or("missing"))})
                    } else if route.contains("/attachments/") {
                        let id = route.rsplit('/').next().unwrap_or("");
                        json!({"job_id":id,"state":"ready","attachment_token":format!("token_{id}"),"expires_at":crate::unix_now()+3600})
                    } else if route == crate::config::CHAT_PATH {
                        received
                            .lock()
                            .map_err(|_| "Fixture lock poisoned")?
                            .push(body.clone());
                        json!({"task_id":format!("task_{}",body["client_request_id"].as_str().unwrap_or("missing")),"client_request_id":body["client_request_id"],"state":"completed","result":{"choices":[{"message":{"role":"assistant","content":"合成分支回答"}}]}})
                    } else {
                        return Err(format!("Unexpected fixture route: {route}"));
                    };
                    let text = response.to_string();
                    stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",text.len()).as_bytes()).map_err(|e|e.to_string())
                })();
                if result.is_err() {
                    halted.store(true, Ordering::Relaxed);
                }
            }
        });
        Ok(Self {
            url,
            stop,
            worker: Some(worker),
            requests,
        })
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn capabilities() -> serde_json::Value {
    json!({"contract_version":1,"principal_id":"branch_fixture","execution_modes":["background"],"attachments":{"enabled":true,"max_count":20,"max_file_bytes":10000,"max_total_bytes":100000,"allowed_extensions":[".csv"]}})
}
fn verify(ok: bool, name: &str) -> AppResult<()> {
    if ok {
        Ok(())
    } else {
        Err(format!("分支自檢失敗：{name}"))
    }
}
impl App {
    pub(in crate::ui) fn verify_branch_flow(&mut self) -> AppResult<()> {
        if !self.smoke {
            return Err("分支自檢僅供 self-check。".into());
        }
        let root = self.root.clone();
        let config = self.config.clone();
        let models = self.models.clone();
        let (tx, rx) = mpsc::channel();
        let previous_tx = std::mem::replace(&mut self.tx, tx);
        self.root = root.join(format!("branch-{}", jobs::new_id()?));
        let result = self.branch_smoke_cases(&rx);
        self.tx = previous_tx;
        self.root = root;
        self.config = config;
        self.models = models;
        self.session = None;
        self.work = Default::default();
        self.projects = Default::default();
        self.archive = Default::default();
        self.active_id = None;
        self.messages.clear();
        self.draft.clear();
        let cases = result?;
        storage::atomic_write(&self.root.join("branch-verification.json"),&serde_json::to_vec_pretty(&json!({"version":env!("CARGO_PKG_VERSION"),"result":"PASS","cases":cases,"transport":"native controller and loopback HTTP","company_service_tested":false})).map_err(|e|e.to_string())?)
    }
    fn branch_smoke_wait(&mut self, rx: &mpsc::Receiver<Event>) -> AppResult<()> {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if Instant::now() > deadline {
                return Err("Branch HTTP fixture timeout".into());
            }
            if let Ok(Event::Work(_, event)) = rx.recv_timeout(Duration::from_millis(50)) {
                let done = matches!(&event, work::WorkEvent::Submitted(_, _));
                self.work_event(event)?;
                if done {
                    break;
                }
            }
        }
        verify(
            !self.work.store.tasks.iter().any(|t| t.active()),
            "native completed task applied",
        )
    }
    fn branch_smoke_cases(&mut self, rx: &mpsc::Receiver<Event>) -> AppResult<Vec<&'static str>> {
        let server = Fixture::start()?;
        self.config.server_url = server.url.clone();
        self.config.allow_http = true;
        self.config.model = "quality".into();
        self.session = Some(Session {
            access_token: "synthetic_branch".into(),
            expires_at: crate::unix_now() + 3600,
            binding: self.config.binding()?,
        });
        self.models = Some(
            serde_json::from_value(json!({"models":[{"id":"quality","label":"品質"}]}))
                .map_err(|e| e.to_string())?,
        );
        self.busy = "none";
        self.history_error = None;
        self.versions = Default::default();
        self.projects = Default::default();
        self.work = Default::default();
        self.archive = Default::default();
        self.work.caps = Some(serde_json::from_value(capabilities()).map_err(|e| e.to_string())?);
        self.work.capability_model = "quality".into();
        self.work.mode = "background".into();
        self.work.store.principal_id = "branch_fixture".into();
        let mut original = Message::user("second question");
        original.request_id = Some("old_request".into());
        self.messages = vec![
            Message::user("PREFIX"),
            Message::assistant("FIRST_REPLY".into()),
            original,
            Message::assistant("OLD_TAIL".into()),
        ];
        let conversation = self.archive.insert(self.messages.clone())?;
        self.active_id = Some(conversation.clone());
        self.draft = "old draft".into();
        let attachment_message = |text: &str, id: &str| {
            let mut message = Message::user(text);
            message.attachments = vec![format!("{id}.csv")];
            message.retry_settings = Some(crate::protocol::RetrySettings {
                model: "quality".into(),
                mode: "background".into(),
                skills: true,
                attachment_ids: vec![id.into()],
                quick_action: false,
            });
            message
        };
        self.messages[0] = attachment_message("PREFIX", "prefix_file");
        self.messages[2].attachments = vec!["question_file.csv".into()];
        self.messages[2].retry_settings = attachment_message("", "question_file").retry_settings;
        self.messages
            .push(attachment_message("OLD_TAIL_QUESTION", "tail_file"));
        for id in ["prefix_file", "question_file", "tail_file"] {
            self.work.store.attachments.push(crate::attachments::Attachment {id:id.into(),conversation_id:conversation.clone(),name:format!("{id}.csv"),size:20,mime_type:"text/csv".into(),state:"ready".into(),sent:true,removed:false,message:String::new(),uploaded_bytes:20,
                remote:Some(serde_json::from_value(json!({"job_id":id,"state":"ready","attachment_token":format!("token_{id}"),"expires_at":crate::unix_now()+3600})).map_err(|e|e.to_string())?)});
        }
        self.save_history()?;

        self.branch_command(BranchCommand::Edit {
            conversation: conversation.clone(),
            branch_id: "original".into(),
            index: 2,
            message_count: 5,
            request_id: Some("old_request".into()),
            text: "EDITED".into(),
        })?;
        self.branch_smoke_wait(rx)?;
        let branch = self.archive.conversations[0].branch.id.clone();
        let first = server.requests.lock().map_err(|_| "fixture poisoned")?[0].clone();
        verify(
            first["attachment_tokens"] == json!(["token_prefix_file", "token_question_file"]),
            "retained attachments only, old tail excluded",
        )?;
        let first_id = first["client_request_id"]
            .as_str()
            .ok_or("Missing request")?
            .to_owned();
        verify(
            first_id != "old_request"
                && first["conversation_id"] == format!("remote_{first_id}")
                && first["messages"].as_array().is_some_and(|m| m.len() == 4)
                && first.to_string().contains("PREFIX")
                && !first.to_string().contains("OLD_TAIL"),
            "new request and isolated selected prefix",
        )?;
        verify(
            history::load(&self.root)?.conversations[0].branch.id == branch,
            "DPAPI restart selected branch",
        )?;
        self.branch_command(BranchCommand::Select {
            conversation: conversation.clone(),
            current_branch: branch.clone(),
            target_branch: "original".into(),
        })?;
        verify(
            self.messages[3].content == "OLD_TAIL" && self.draft == "old draft",
            "old tail and draft restored",
        )?;
        self.branch_command(BranchCommand::Select {
            conversation: conversation.clone(),
            current_branch: "original".into(),
            target_branch: branch.clone(),
        })?;
        verify(
            server
                .requests
                .lock()
                .map_err(|_| "fixture poisoned")?
                .len()
                == 1
                && self.work.store.tasks.len() == 1,
            "switch makes no HTTP task",
        )?;
        verify(
            self.branch_command(BranchCommand::Edit {
                conversation: conversation.clone(),
                branch_id: "original".into(),
                index: 2,
                message_count: 4,
                request_id: Some("old_request".into()),
                text: "STALE".into(),
            })
            .is_err(),
            "stale branch rejected",
        )?;
        let mut next = self.messages.clone();
        next.push(Message::user("NEXT"));
        self.begin_work_chat(next, "chat")?;
        self.branch_smoke_wait(rx)?;
        let second = server.requests.lock().map_err(|_| "fixture poisoned")?[1].clone();
        verify(
            second["client_request_id"] != first["client_request_id"]
                && second["conversation_id"] != first["conversation_id"]
                && second.to_string().contains("EDITED")
                && !second.to_string().contains("OLD_TAIL"),
            "later branch turn uses fresh isolated full history",
        )?;
        let question_index = self
            .messages
            .iter()
            .rposition(|m| m.role == "user")
            .ok_or("missing question")?;
        let question_id = self.messages[question_index].request_id.clone();
        self.retry_chat(
            &conversation,
            question_index,
            self.messages.len(),
            question_id.as_deref(),
        )?;
        self.branch_smoke_wait(rx)?;
        let retry = server.requests.lock().map_err(|_| "fixture poisoned")?[2].clone();
        verify(
            retry["client_request_id"] != second["client_request_id"]
                && retry["conversation_id"] != second["conversation_id"]
                && retry["messages"] == second["messages"],
            "manual retry retains selected prefix and excludes latest answer",
        )?;
        let retained = self.messages.clone();
        let saved_expiry = self.work.store.attachments[0]
            .remote
            .as_ref()
            .and_then(|r| r.expires_at);
        self.work.store.attachments[0]
            .remote
            .as_mut()
            .ok_or("missing remote")?
            .expires_at = Some(1);
        verify(
            self.branch_attachment_tokens(&conversation, &retained)
                .is_err(),
            "expired retained attachment rejected",
        )?;
        self.work.store.attachments[0]
            .remote
            .as_mut()
            .ok_or("missing remote")?
            .expires_at = saved_expiry;
        self.work.store.attachments[0].conversation_id = "foreign".into();
        verify(
            self.branch_attachment_tokens(&conversation, &retained)
                .is_err(),
            "foreign attachment ownership rejected",
        )?;
        self.work.store.attachments[0].conversation_id = conversation.clone();
        // 有顯示名稱但遺失原生附件識別，不得以無附件快照繼續。
        let mut missing = Message::user("attachment");
        missing.attachments.push("missing.csv".into());
        verify(
            self.branch_attachment_tokens(&conversation, &[missing])
                .is_err(),
            "missing attachment explicitly rejected",
        )?;
        let before_project = server
            .requests
            .lock()
            .map_err(|_| "fixture poisoned")?
            .len();
        verify(
            before_project == 3,
            "exactly edit, follow-up and manual retry requests",
        )?;
        let project_root = self.root.join("project");
        std::fs::create_dir_all(&project_root).map_err(|e| e.to_string())?;
        let project = self.projects.store.add("branch project", project_root)?;
        self.projects
            .store
            .conversations
            .insert(conversation.clone(), project);
        verify(
            self.branch_command(BranchCommand::Edit {
                conversation: conversation.clone(),
                branch_id: branch.clone(),
                index: 2,
                message_count: self.messages.len(),
                request_id: self.messages[2].request_id.clone(),
                text: "illegal".into(),
            })
            .is_err(),
            "executed project cannot fork",
        )?;
        self.messages.push(Message::user("unsent"));
        self.save_history()?;
        self.branch_command(BranchCommand::Edit {
            conversation,
            branch_id: branch,
            index: self.messages.len() - 1,
            message_count: self.messages.len(),
            request_id: None,
            text: "project draft edited".into(),
        })?;
        verify(
            self.draft == "project draft edited"
                && server
                    .requests
                    .lock()
                    .map_err(|_| "fixture poisoned")?
                    .len()
                    == before_project,
            "project draft editing never submits",
        )?;
        Ok(vec![
            "native edit sends fresh request with selected prefix only",
            "later turn isolates server context",
            "manual retry uses same selected prefix and fresh identity",
            "expired or foreign attachment rejected",
            "DPAPI restart retains selected branch",
            "switch restores old tail and draft without HTTP",
            "stale branch rejected",
            "missing attachment explicitly rejected",
            "valid retained attachments rechecked over HTTP; old-tail token excluded",
            "executed project cannot fork",
            "unstarted project edit only updates draft",
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpStream;

    #[test]
    fn branch_fixture_accepts_delayed_and_split_http_requests() {
        let fixture = Fixture::start().unwrap();
        // 先建立連線，等 accept 有機會先讀，再分開送表頭與內容；不讓快速 localhost 掩蓋錯誤。
        for id in ["delayed_one", "delayed_two"] {
            let mut stream = TcpStream::connect(fixture.url.trim_start_matches("http://")).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            std::thread::sleep(Duration::from_millis(100));
            let body = json!({"client_request_id":id}).to_string();
            write!(stream, "POST {} HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", crate::config::CHAT_PATH, body.len()).unwrap();
            std::thread::sleep(Duration::from_millis(100));
            stream.write_all(body.as_bytes()).unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).unwrap();
            assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
            let (_, body) = response.split_once("\r\n\r\n").unwrap();
            let reply: serde_json::Value = serde_json::from_str(body).unwrap();
            assert_eq!(reply["client_request_id"], id);
            assert_eq!(reply["state"], "completed");
        }
        assert_eq!(fixture.requests.lock().unwrap().len(), 2);
        assert!(!fixture.stop.load(Ordering::Relaxed));
    }
}
