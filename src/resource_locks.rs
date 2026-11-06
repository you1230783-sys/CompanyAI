//! 本機工具資源擁有權：依資源 FIFO 排隊，等待不呼叫模型；guard 在錯誤與 unwind 時釋放。
//! 鎖只協調本程式，不能取代檔案版本／權限檢查，也不能接管使用者開啟的 Office 文件。
use crate::AppResult;
use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    marker::PhantomData,
    path::Path,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex, OnceLock,
    },
    thread::{self, ThreadId},
    time::{Duration, Instant},
};

const TIMEOUT: Duration = Duration::from_secs(600);
const POLL: Duration = Duration::from_millis(25);

/// 順序是取得多種資源的契約；新增資源時不可讓呼叫端反向巢狀取得。
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Resource {
    Source(String),
    Memory(String),
    Outlook,
    Excel,
    Word,
    PowerPoint,
    Cad,
}
impl Resource {
    fn label(&self) -> &'static str {
        match self {
            Self::Source(_) => "同一來源的副本",
            Self::Memory(_) => "專案記憶",
            Self::Outlook => "Outlook",
            Self::Excel => "Excel",
            Self::Word => "Word",
            Self::PowerPoint => "PowerPoint",
            Self::Cad => "CAD",
        }
    }
    /// 使用解析後的路徑作本機協調鍵，原有拒絕 reparse point／硬連結檢查仍須另外執行。
    pub fn file(path: &Path, memory: bool) -> AppResult<Self> {
        let path = std::fs::canonicalize(path).map_err(|e| format!("無法確認資源路徑：{e}"))?;
        let key = path.to_string_lossy().to_lowercase();
        Ok(if memory {
            Self::Memory(key)
        } else {
            Self::Source(key)
        })
    }
}

/// 原生 worker 的等待資訊只交給介面；不把資源名稱或擁有者插入 AI 對話。
#[derive(Clone)]
pub struct Context {
    pub label: String,
    pub cancel: Arc<AtomicBool>,
    pub notify: Arc<dyn Fn(Option<String>) + Send + Sync>,
}
thread_local! { static CONTEXT: RefCell<Option<Context>> = const { RefCell::new(None) }; }
pub struct ContextGuard(Option<Context>, PhantomData<Rc<()>>);
impl Drop for ContextGuard {
    fn drop(&mut self) {
        CONTEXT.with(|slot| *slot.borrow_mut() = self.0.take());
    }
}
impl Context {
    /// 批次子執行緒必須顯式沿用；不跨執行緒搬移任何 COM 或鎖 guard。
    pub fn current() -> Option<Self> {
        CONTEXT.with(|slot| slot.borrow().clone())
    }
    pub fn enter(context: Option<Self>) -> ContextGuard {
        ContextGuard(CONTEXT.with(|slot| slot.replace(context)), PhantomData)
    }
}

struct Owner {
    thread: ThreadId,
    depth: usize,
    label: String,
}
struct Ticket {
    id: u64,
    resources: Vec<Resource>,
}
#[derive(Default)]
struct State {
    owners: BTreeMap<Resource, Owner>,
    queue: VecDeque<Ticket>,
    next: u64,
}
#[derive(Default)]
struct Manager {
    state: Mutex<State>,
    changed: Condvar,
}
fn manager() -> &'static Arc<Manager> {
    static INSTANCE: OnceLock<Arc<Manager>> = OnceLock::new();
    INSTANCE.get_or_init(|| Arc::new(Manager::default()))
}

/// 只能由取得鎖的執行緒釋放；同執行緒可重入，另一批次執行緒即使屬同任務也必須排隊。
pub struct Guard {
    manager: Arc<Manager>,
    resources: Vec<Resource>,
    _local: PhantomData<Rc<()>>,
}
impl Drop for Guard {
    fn drop(&mut self) {
        let mut state = self.manager.state.lock().unwrap_or_else(|e| e.into_inner());
        for resource in &self.resources {
            if let Some(owner) = state.owners.get_mut(resource) {
                owner.depth -= 1;
                if owner.depth == 0 {
                    state.owners.remove(resource);
                }
            }
        }
        self.manager.changed.notify_all();
    }
}

/// 一次取得全部資源，排序、去重且不先占住其中一把；獨立資源可以同時進行。
pub fn acquire(resources: impl IntoIterator<Item = Resource>) -> AppResult<Guard> {
    manager().acquire(resources.into_iter().collect(), Context::current(), TIMEOUT)
}
/// 停止任務後仍須保存審計與 checkpoint；短暫儲存交易不沿用工具取消旗標。
pub(crate) fn acquire_storage(resource: Resource) -> AppResult<Guard> {
    manager().acquire(vec![resource], None, TIMEOUT)
}

/// 派送前探測只在沒有本機擁有者時執行，避免把另一任務的文件當成使用者占用。
pub(crate) fn try_acquire(resource: Resource) -> Option<Guard> {
    manager().acquire(vec![resource], None, Duration::ZERO).ok()
}
impl Manager {
    fn acquire(
        self: &Arc<Self>,
        mut resources: Vec<Resource>,
        context: Option<Context>,
        timeout: Duration,
    ) -> AppResult<Guard> {
        resources.sort();
        resources.dedup();
        let thread = thread::current().id();
        let started = Instant::now();
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let held: Vec<_> = state
            .owners
            .iter()
            .filter(|(_, owner)| owner.thread == thread)
            .map(|(r, _)| r.clone())
            .collect();
        let needed: Vec<_> = resources
            .iter()
            .filter(|r| !held.contains(r))
            .cloned()
            .collect();
        // 記憶交易只持有檔案／DPAPI，不會在持鎖時呼叫 Office；可在工具提交摘要時短暫取得。
        // 來源與 Office 仍遵守固定順序；若未來記憶交易要呼叫工具，必須先重設這個邊界。
        if needed
            .iter()
            .find(|resource| !matches!(resource, Resource::Memory(_)))
            .zip(held.last())
            .is_some_and(|(new, old)| new < old)
        {
            return Err("資源取得順序不符；請在操作前一次取得所需資源。".into());
        }
        let ticket = state.next;
        state.next = state.next.checked_add(1).ok_or("資源排隊識別已達上限。")?;
        state.queue.push_back(Ticket {
            id: ticket,
            resources: needed.clone(),
        });
        // 票券也有 RAII：通知回呼 panic 時仍從隊列移除，避免阻塞後續任務。
        let mut pending = Pending {
            manager: self.clone(),
            ticket,
            waiting: false,
            context: context.clone(),
        };
        loop {
            if context
                .as_ref()
                .is_some_and(|c| c.cancel.load(Ordering::Relaxed))
            {
                drop(state);
                return Err("資源等待已取消，尚未取得擁有權。".into());
            }
            let blocked_owner = needed.iter().find_map(|r| {
                state
                    .owners
                    .get(r)
                    .map(|owner| (r.label(), owner.label.clone()))
            });
            let queued = state
                .queue
                .iter()
                .take_while(|t| t.id != ticket)
                .any(|t| t.resources.iter().any(|r| needed.contains(r)));
            if blocked_owner.is_none() && !queued {
                state.queue.retain(|t| t.id != ticket);
                for resource in &resources {
                    state
                        .owners
                        .entry(resource.clone())
                        .and_modify(|o| o.depth += 1)
                        .or_insert_with(|| Owner {
                            thread,
                            depth: 1,
                            label: context
                                .as_ref()
                                .map(|c| c.label.clone())
                                .unwrap_or_else(|| "本機操作".into()),
                        });
                }
                // 先建立 guard 再發通知，回呼即使 panic 也不遺留已取得的鎖。
                let guard = Guard {
                    manager: self.clone(),
                    resources,
                    _local: PhantomData,
                };
                drop(state);
                drop(pending);
                return Ok(guard);
            }
            if started.elapsed() >= timeout {
                let resource = blocked_owner.as_ref().map(|(r, _)| *r).unwrap_or("資源");
                drop(state);
                return Err(format!(
                    "資源等待逾時：另一個任務占用 {resource}，尚未取得擁有權。"
                ));
            }
            if !pending.waiting {
                let message = match blocked_owner {
                    Some((resource, owner)) => format!("等待{owner}完成 {resource} 操作…"),
                    None => "等待先到的任務取得資源…".into(),
                };
                pending.waiting = true;
                drop(state);
                if let Some(context) = &context {
                    (context.notify)(Some(message));
                }
                state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            } else {
                state = self
                    .changed
                    .wait_timeout(state, POLL)
                    .unwrap_or_else(|e| e.into_inner())
                    .0;
            }
        }
    }
}
struct Pending {
    manager: Arc<Manager>,
    ticket: u64,
    waiting: bool,
    context: Option<Context>,
}
impl Drop for Pending {
    fn drop(&mut self) {
        {
            let mut state = self.manager.state.lock().unwrap_or_else(|e| e.into_inner());
            state.queue.retain(|t| t.id != self.ticket);
            self.manager.changed.notify_all();
        }
        // 不在 unwind 中再呼叫外部程式，避免二次 panic。
        if self.waiting && !thread::panicking() {
            if let Some(context) = &self.context {
                (context.notify)(None);
            }
        }
    }
}

#[cfg(test)]
mod tests;
