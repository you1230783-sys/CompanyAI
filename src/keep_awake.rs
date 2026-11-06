//! 執行中有需要時維持系統喚醒。每個監控執行緒只清除自己的 Windows 執行狀態。
use crate::AppResult;
use std::{sync::mpsc, thread, time::Duration};

const ES_CONTINUOUS: u32 = 0x8000_0000;
const ES_SYSTEM_REQUIRED: u32 = 0x0000_0001;

#[link(name = "kernel32")]
extern "system" {
    fn SetThreadExecutionState(flags: u32) -> u32;
}

struct ThreadState(bool);

impl ThreadState {
    fn update(&mut self, enabled: bool) {
        if self.0 != enabled {
            let flags = ES_CONTINUOUS | if enabled { ES_SYSTEM_REQUIRED } else { 0 };
            if unsafe { SetThreadExecutionState(flags) } != 0 {
                self.0 = enabled;
            }
        }
    }
}

impl Drop for ThreadState {
    fn drop(&mut self) {
        self.update(false);
    }
}

/// 由任務持有，錯誤、暫停或 panic 都會讓執行緒在同一條執行緒恢復 Windows 狀態。
pub(crate) struct KeepAwake {
    stop: mpsc::Sender<()>,
    worker: Option<thread::JoinHandle<()>>,
}

impl KeepAwake {
    pub(crate) fn watch(enabled: impl Fn() -> bool + Send + 'static) -> AppResult<Self> {
        let (stop, receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("lmai-keep-awake".into())
            .spawn(move || {
                let mut state = ThreadState(false);
                loop {
                    state.update(enabled());
                    match receiver.recv_timeout(Duration::from_millis(250)) {
                        Err(mpsc::RecvTimeoutError::Timeout) => (),
                        _ => break,
                    }
                }
            })
            .map_err(|e| format!("無法建立防睡眠監控：{e}"))?;
        Ok(Self {
            stop,
            worker: Some(worker),
        })
    }
}

impl Drop for KeepAwake {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
