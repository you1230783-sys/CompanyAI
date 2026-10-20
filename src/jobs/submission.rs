//! 提交回應需區分「明確拒絕」與「結果未知」，避免把 413 轉成無限任務查詢。
use super::*;

#[derive(Debug)]
pub(crate) struct SubmitFailure {
    pub message: String,
    pub rejected: bool,
}

impl SubmitFailure {
    pub fn rejected(message: &str) -> Self {
        Self {
            message: message.into(),
            rejected: true,
        }
    }

    /// 4xx 通常是受理前拒絕；408／5xx 可能發生在後端建立工作後，保留查回。
    /// 網站若明確提供 task_accepted，以該欄位優先，不從錯誤描述猜受理狀態。
    pub fn from_response(response: &transport::HttpResponse, session: &Session) -> Self {
        let body = serde_json::from_str::<Value>(&response.body).ok();
        let accepted = body.as_ref().and_then(|body| {
            body.get("task_accepted")
                .or_else(|| {
                    body.get("error")
                        .and_then(|error| error.get("task_accepted"))
                })
                .and_then(Value::as_bool)
        });
        Self {
            message: protocol::api_error(response.status, &response.body, &session.access_token),
            rejected: accepted
                .map(|accepted| !accepted)
                .unwrap_or((400..500).contains(&response.status) && response.status != 408),
        }
    }
}

// 傳輸中斷、JSON／串流格式錯誤不代表伺服器沒有建立任務。
impl From<String> for SubmitFailure {
    fn from(message: String) -> Self {
        Self {
            message,
            rejected: false,
        }
    }
}

impl From<&str> for SubmitFailure {
    fn from(message: &str) -> Self {
        message.to_string().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    #[test]
    fn explicit_rejection_stops_but_unknown_acceptance_remains_queryable() {
        let session = Session {
            access_token: "test-secret".into(),
            expires_at: 0,
            binding: String::new(),
        };
        let failure = |status, body: &str| {
            SubmitFailure::from_response(
                &transport::HttpResponse {
                    status,
                    body: body.into(),
                },
                &session,
            )
        };
        for status in [400, 401, 403, 404, 409, 413, 415, 422, 426, 429] {
            let rejected = failure(
                status,
                r#"{"error":{"message":"文件ID數量超過20 test-secret"}}"#,
            );
            assert!(rejected.rejected);
            assert!(rejected.message.contains(&format!("HTTP {status}")));
            assert!(rejected.message.contains("文件ID數量超過20"));
            assert!(!rejected.message.contains("test-secret"));
        }
        for status in [408, 500, 502, 503, 504] {
            assert!(!failure(status, "{}").rejected);
        }
        assert!(failure(503, r#"{"task_accepted":false}"#).rejected);
        assert!(!failure(413, r#"{"task_accepted":true}"#).rejected);
        assert!(failure(500, r#"{"error":{"task_accepted":false}}"#).rejected);
        assert!(!SubmitFailure::from("網路逾時").rejected);
        assert!(SubmitFailure::rejected("提交前能力檢查失敗").rejected);
    }

    /// 真實 WinHTTP 契約回歸：相同 413 在背景／串流入口都須保留為明確拒絕。
    #[test]
    fn both_submission_modes_preserve_413_without_emitting_a_task() {
        for mode in ["background", "stream"] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .unwrap();
                let mut received = Vec::new();
                loop {
                    let mut chunk = [0u8; 4096];
                    let count = stream.read(&mut chunk).unwrap();
                    assert!(count > 0);
                    received.extend_from_slice(&chunk[..count]);
                    if let Some(end) = received.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&received[..end]);
                        let length: usize = headers
                            .lines()
                            .find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                name.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse().unwrap())
                            })
                            .unwrap();
                        if received.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                let body =
                    r#"{"error":{"code":"too_many_documents","message":"附件文件ID總數超過20"}}"#;
                write!(stream, "HTTP/1.1 413 Content Too Large\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
            });
            let config = Config {
                server_url: format!("http://{address}"),
                ..Config::default()
            };
            let session = Session {
                access_token: "test-secret".into(),
                expires_at: crate::unix_now() + 60,
                binding: config.binding().unwrap(),
            };
            let task = serde_json::from_value(json!({
                "request_id":"request1", "conversation_id":"local1", "request":{},
                "mode":mode, "title":"test", "created_at":0, "remote":null, "applied":false
            }))
            .unwrap();
            let mut updates = 0;
            let result = submit_tracked(
                &config,
                &session,
                &task,
                &std::sync::atomic::AtomicBool::new(false),
                |_| updates += 1,
            );
            server.join().unwrap();
            let failure = result.unwrap_err();
            assert!(failure.rejected);
            assert!(failure.message.contains("HTTP 413"));
            assert!(failure.message.contains("附件文件ID總數超過20"));
            assert_eq!(updates, 0);
        }
    }
}
