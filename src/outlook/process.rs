//! Outlook COM專用輔助程序。父程序可中止自己的子程序，不終止Outlook。
//! 所有通訊走匿名管線；內部郵件代號與未篩選資料不寫明文暫存檔。
use crate::{
    projects::mail::{Folder, Header, Scan, Source},
    AppResult,
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    io::{BufRead, BufReader, Read, Write},
    os::windows::process::CommandExt,
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

const LIMIT: Duration = Duration::from_secs(600);
const MAX_LINE: u64 = 32 * 1024 * 1024;
static WORKER: AtomicBool = AtomicBool::new(false);

#[derive(Serialize, Deserialize)]
enum Request {
    Folders {
        scope: String,
        parent: Option<Folder>,
    },
    Headers {
        folder: Folder,
        start: String,
        end: String,
    },
    Body {
        folder: Folder,
        header: Header,
    },
    Verify {
        folder: Folder,
        header: Header,
    },
}
#[derive(Serialize, Deserialize)]
enum Event {
    Progress(String),
    Done(Result<Value, String>),
}

fn send(event: &Event) -> AppResult<()> {
    let mut out = std::io::stdout().lock();
    serde_json::to_writer(&mut out, event).map_err(|e| e.to_string())?;
    out.write_all(b"\n")
        .and_then(|_| out.flush())
        .map_err(|e| e.to_string())
}
pub(super) fn notify(message: &str) {
    if WORKER.load(Ordering::Relaxed) {
        let _ = send(&Event::Progress(message.into()));
    }
}

/// 限制每則協定長度，避免意外輸出使常駐程序無界配置記憶體。
fn line(reader: &mut impl BufRead) -> AppResult<Option<String>> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_LINE + 1)
        .read_until(b'\n', &mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_LINE {
        return Err("Outlook本機處理回傳超出大小界線。".into());
    }
    if bytes.is_empty() {
        return Ok(None);
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| "Outlook本機處理通訊不是UTF-8。".into())
}

pub fn run_worker() -> AppResult<()> {
    WORKER.store(true, Ordering::Relaxed);
    let mut input = BufReader::new(std::io::stdin().lock());
    let cancel = AtomicBool::new(false);
    while let Some(raw) = line(&mut input)? {
        let result = (|| {
            let request: Request = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
            let mut reader = super::project::Reader;
            match request {
                Request::Folders { scope, parent } => {
                    serde_json::to_value(reader.folders(&scope, parent.as_ref(), &cancel)?)
                }
                Request::Headers { folder, start, end } => {
                    let parse = |s: &str| {
                        NaiveDate::parse_from_str(s, "%Y-%m-%d")
                            .map_err(|_| "Outlook 日期格式錯誤。".to_string())
                    };
                    serde_json::to_value(reader.headers(
                        &folder,
                        parse(&start)?,
                        parse(&end)?,
                        &cancel,
                    )?)
                }
                Request::Body { folder, header } => {
                    serde_json::to_value(reader.body(&folder, &header, &cancel)?)
                }
                Request::Verify { folder, header } => {
                    reader.verify(&folder, &header, &cancel)?;
                    Ok(Value::Null)
                }
            }
            .map_err(|e| e.to_string())
        })();
        send(&Event::Done(result))?;
    }
    Ok(())
}

pub(crate) struct Process {
    child: Child,
    input: ChildStdin,
    events: mpsc::Receiver<AppResult<Event>>,
    reader: Option<std::thread::JoinHandle<()>>,
}
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        // 管線讀取以非阻塞送出通道，子程序終止後一定可結束，不留下等待的執行緒。
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}
impl Process {
    fn start(executable: &Path) -> AppResult<Self> {
        let child = Command::new(executable)
            .arg("--outlook-worker")
            .creation_flags(0x08000000)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Outlook本機處理輔助程序無法啟動：{e}"))?;
        Self::from_child(child)
    }
    fn from_child(mut child: Child) -> AppResult<Self> {
        let input = child
            .stdin
            .take()
            .ok_or("Outlook本機處理輸入管線不可用。")?;
        let stdout = child
            .stdout
            .take()
            .ok_or("Outlook本機處理輸出管線不可用。")?;
        let (send, events) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let event = line(&mut reader)
                    .and_then(|raw| raw.ok_or("Outlook本機處理程序提前結束。".into()))
                    .and_then(|raw| serde_json::from_str(&raw).map_err(|e| e.to_string()));
                let failed = event.is_err();
                if send.send(event).is_err() || failed {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            input,
            events,
            reader: Some(reader),
        })
    }
    fn call(
        &mut self,
        request: &Request,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(&str),
        deadline: Instant,
    ) -> AppResult<Value> {
        serde_json::to_writer(&mut self.input, request).map_err(|e| e.to_string())?;
        self.input
            .write_all(b"\n")
            .and_then(|_| self.input.flush())
            .map_err(|e| e.to_string())?;
        let mut last_notice = Instant::now() - Duration::from_secs(1);
        loop {
            super::batch::check_cancel(cancel)?;
            if Instant::now() >= deadline {
                return Err("Outlook本機處理逾時（10分鐘）；已中止本次輔助程序，未開放此批資料。請確認Outlook已載入，或縮小日期範圍。".into());
            }
            match self.events.recv_timeout(Duration::from_millis(100)) {
                Ok(Ok(Event::Progress(message))) => {
                    if last_notice.elapsed() >= Duration::from_secs(1)
                        || message.contains("比對完成")
                    {
                        progress(&message);
                        last_notice = Instant::now();
                    }
                }
                Ok(Ok(Event::Done(result))) => return result,
                Ok(Err(error)) => return Err(error),
                Err(mpsc::RecvTimeoutError::Timeout) => (),
                Err(_) => return Err("Outlook本機處理程序連線已關閉。".into()),
            }
        }
    }
}

/// 一次工具操作共用10分鐘期限，含該工具的多次verify/body，不各自重設期限。
pub(crate) struct Reader<'a> {
    pub process: &'a mut Option<Process>,
    pub executable: &'a Path,
    pub progress: &'a mut dyn FnMut(&str),
    pub deadline: Instant,
}
impl Reader<'_> {
    pub fn deadline() -> Instant {
        Instant::now() + LIMIT
    }
    fn call<T: serde::de::DeserializeOwned>(
        &mut self,
        request: Request,
        cancel: &AtomicBool,
    ) -> AppResult<T> {
        super::batch::check_cancel(cancel)?;
        if self.process.is_none() {
            *self.process = Some(Process::start(self.executable)?);
        }
        let result = self.process.as_mut().ok_or("Outlook程序不存在。")?.call(
            &request,
            cancel,
            self.progress,
            self.deadline,
        );
        // 任何未知錯誤丟棄此程序，避免下一次讀到前次逾時的回覆。
        match result {
            Ok(value) => serde_json::from_value(value).map_err(|e| e.to_string()),
            Err(error) => {
                self.process.take();
                Err(error)
            }
        }
    }
}
impl Source for Reader<'_> {
    fn folders(
        &mut self,
        scope: &str,
        parent: Option<&Folder>,
        cancel: &AtomicBool,
    ) -> AppResult<(Vec<Folder>, Vec<String>)> {
        self.call(
            Request::Folders {
                scope: scope.into(),
                parent: parent.cloned(),
            },
            cancel,
        )
    }
    fn headers(
        &mut self,
        folder: &Folder,
        start: NaiveDate,
        end: NaiveDate,
        cancel: &AtomicBool,
    ) -> AppResult<Scan> {
        self.call(
            Request::Headers {
                folder: folder.clone(),
                start: start.to_string(),
                end: end.to_string(),
            },
            cancel,
        )
    }
    fn body(&mut self, folder: &Folder, header: &Header, cancel: &AtomicBool) -> AppResult<String> {
        self.call(
            Request::Body {
                folder: folder.clone(),
                header: header.clone(),
            },
            cancel,
        )
    }
    fn verify(&mut self, folder: &Folder, header: &Header, cancel: &AtomicBool) -> AppResult<()> {
        self.call(
            Request::Verify {
                folder: folder.clone(),
                header: header.clone(),
            },
            cancel,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// 使用固定本機sleep程序模擬COM永不回覆；驗證真的kill/reap，不只是回傳逾時文字。
    #[test]
    fn blocked_child_is_terminated_on_timeout_and_cancel() {
        for stopped in [false, true] {
            let shell = std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap())
                .join("System32/WindowsPowerShell/v1.0/powershell.exe");
            let child = Command::new(shell)
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "[Threading.Thread]::Sleep(60000)",
                ])
                .creation_flags(0x08000000)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            let mut process = Some(Process::from_child(child).unwrap());
            let started = Instant::now();
            let cancel = AtomicBool::new(false);
            std::thread::scope(|scope| {
                if stopped {
                    scope.spawn(|| {
                        std::thread::sleep(Duration::from_millis(150));
                        cancel.store(true, Ordering::Relaxed);
                    });
                }
                let mut progress = |_: &str| {};
                let mut reader = Reader {
                    process: &mut process,
                    executable: Path::new("unused"),
                    progress: &mut progress,
                    deadline: started + Duration::from_millis(if stopped { 5000 } else { 200 }),
                };
                let result: AppResult<Value> = reader.call(
                    Request::Folders {
                        scope: "online_inbox".into(),
                        parent: None,
                    },
                    &cancel,
                );
                assert!(result.is_err());
            });
            assert!(process.is_none(), "失敗後必須丟棄程序及管線");
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "COM阻塞不可拖住父程序"
            );
        }
    }
}
