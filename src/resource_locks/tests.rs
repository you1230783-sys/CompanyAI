use super::*;
use std::sync::mpsc;

fn context(label: &str, cancel: Arc<AtomicBool>, tx: mpsc::Sender<Option<String>>) -> Context {
    Context {
        label: label.into(),
        cancel,
        notify: Arc::new(move |message| {
            let _ = tx.send(message);
        }),
    }
}

#[test]
fn fifo_waiters_and_independent_resources() {
    let manager = Arc::new(Manager::default());
    let owner = manager
        .acquire(vec![Resource::Word], None, TIMEOUT)
        .unwrap();
    let (tx, rx) = mpsc::channel();
    let mut workers = Vec::new();
    for n in 0..2 {
        let manager = manager.clone();
        let tx = tx.clone();
        let (waiting, observed) = mpsc::channel();
        workers.push(thread::spawn(move || {
            let ctx = context("測試", Arc::new(AtomicBool::new(false)), waiting);
            let _guard = manager
                .acquire(vec![Resource::Word], Some(ctx), TIMEOUT)
                .unwrap();
            tx.send(n).unwrap();
        }));
        assert!(observed
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .is_some());
    }
    let excel = manager.acquire(vec![Resource::Excel], None, TIMEOUT);
    // 同執行緒反向取得會被拒絕；不同執行緒的獨立 Excel 則不必等待 Word。
    assert!(excel.is_err());
    let other = manager.clone();
    thread::spawn(move || {
        let _excel = other
            .acquire(vec![Resource::Excel], None, Duration::from_millis(100))
            .unwrap();
    })
    .join()
    .unwrap();
    drop(owner);
    assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap(), 0);
    assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap(), 1);
    for worker in workers {
        worker.join().unwrap();
    }
}

#[test]
fn cancelled_and_timed_out_waiters_leave_no_ticket() {
    let manager = Arc::new(Manager::default());
    let owner = manager
        .acquire(vec![Resource::Word], None, TIMEOUT)
        .unwrap();
    let cancelled = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();
    let ctx = context("取消測試", cancelled.clone(), tx);
    let other = manager.clone();
    let waiter = thread::spawn(move || {
        other
            .acquire(vec![Resource::Word], Some(ctx), TIMEOUT)
            .err()
            .unwrap()
    });
    rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let now = Instant::now();
    cancelled.store(true, Ordering::Relaxed);
    assert!(waiter.join().unwrap().contains("取消"));
    assert!(now.elapsed() < Duration::from_secs(1));
    let other = manager.clone();
    let error = thread::spawn(move || {
        other
            .acquire(vec![Resource::Word], None, Duration::from_millis(40))
            .err()
            .unwrap()
    })
    .join()
    .unwrap();
    assert!(error.contains("資源等待逾時"));
    assert!(manager.state.lock().unwrap().queue.is_empty());
    drop(owner);
}

#[test]
fn sorted_group_reentrancy_and_unwind_release() {
    let manager = Arc::new(Manager::default());
    let other = manager.clone();
    thread::spawn(move || {
        let panic = std::panic::catch_unwind(|| {
            let _group = other
                .acquire(
                    vec![Resource::Word, Resource::Excel, Resource::Word],
                    None,
                    TIMEOUT,
                )
                .unwrap();
            let _nested = other.acquire(vec![Resource::Excel], None, TIMEOUT).unwrap();
            panic!("驗證 guard unwind");
        });
        assert!(panic.is_err());
    })
    .join()
    .unwrap();
    let _next = manager
        .acquire(
            vec![Resource::Excel, Resource::Word],
            None,
            Duration::from_millis(100),
        )
        .unwrap();
}

#[test]
fn observer_panic_removes_waiting_ticket() {
    let manager = Arc::new(Manager::default());
    let _owner = manager
        .acquire(vec![Resource::Word], None, TIMEOUT)
        .unwrap();
    let other = manager.clone();
    let worker = thread::spawn(move || {
        let ctx = Context {
            label: "測試".into(),
            cancel: Arc::new(AtomicBool::new(false)),
            notify: Arc::new(|_| panic!("通知錯誤")),
        };
        let _ = other.acquire(vec![Resource::Word], Some(ctx), TIMEOUT);
    });
    assert!(worker.join().is_err());
    assert!(manager.state.lock().unwrap().queue.is_empty());
}
