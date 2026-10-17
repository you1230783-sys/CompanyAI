//! CAD 子程序的唯讀進度管線。只接收固定數字，不接收圖面正文或可執行指令。
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Read},
    process::ChildStderr,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

/// 通知不等待使用者回覆；取消仍使用整個專案任務的 AtomicBool。
pub type Callback = Box<dyn FnMut(Value) + Send>;

#[derive(Clone, Default, Deserialize)]
struct Counts {
    completed: u64,
    reported_count: Option<u64>,
    phase: String,
}

/// drop 保證成功、錯誤、取消與逾時均清除提示，不把提示留到 AI 思考階段。
pub(super) struct Monitor<'a> {
    callback: Option<&'a mut Callback>,
    latest: Arc<Mutex<Counts>>,
    started: Instant,
    last_sent: Option<Instant>,
}

impl<'a> Monitor<'a> {
    pub(super) fn new(
        stderr: ChildStderr,
        callback: Option<&'a mut Callback>,
        reading: bool,
    ) -> Self {
        let latest = Arc::new(Mutex::new(Counts {
            phase: if reading { "" } else { "idle" }.into(),
            ..Default::default()
        }));
        let target = latest.clone();
        // 這條執行緒只讀管線，絕不持有或呼叫 CAD COM 物件。
        thread::spawn(move || {
            for line in BufReader::new(stderr.take(1_000_000)).lines() {
                let Ok(line) = line else { break };
                if line.len() > 256 {
                    continue;
                }
                if let Ok(counts) = serde_json::from_str::<Counts>(&line) {
                    if counts.completed <= 200_000
                        && matches!(counts.phase.as_str(), "reading" | "checking" | "idle")
                    {
                        if let Ok(mut value) = target.lock() {
                            *value = counts;
                        }
                    }
                }
            }
        });
        let mut monitor = Self {
            callback,
            latest,
            started: Instant::now(),
            last_sent: None,
        };
        monitor.tick();
        monitor
    }

    pub(super) fn tick(&mut self) {
        if self
            .last_sent
            .is_some_and(|last| last.elapsed() < Duration::from_secs(1))
        {
            return;
        }
        self.last_sent = Some(Instant::now());
        if let (Some(callback), Ok(counts)) = (self.callback.as_mut(), self.latest.lock()) {
            callback(
                json!({"active":counts.phase != "idle","completed":counts.completed,
                "reported_count":counts.reported_count,"phase":counts.phase,
                "elapsed_seconds":self.started.elapsed().as_secs(),"timeout_seconds":600}),
            );
        }
    }
}

impl Drop for Monitor<'_> {
    fn drop(&mut self) {
        if let Some(callback) = self.callback.as_mut() {
            callback(json!({"active":false}));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_clears_notice_on_early_return() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let saved = events.clone();
        let mut callback: Callback = Box::new(move |event| saved.lock().unwrap().push(event));
        {
            let mut monitor = Monitor {
                callback: Some(&mut callback),
                latest: Arc::new(Mutex::new(Counts {
                    completed: 51,
                    reported_count: Some(643),
                    phase: "reading".into(),
                })),
                started: Instant::now(),
                last_sent: None,
            };
            monitor.tick();
            monitor.tick();
        }
        let events = events.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["completed"], 51);
        assert_eq!(events[0]["timeout_seconds"], 600);
        assert_eq!(events[1]["active"], false);
    }
}
