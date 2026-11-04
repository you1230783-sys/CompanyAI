//! 每種模式以獨立程序執行，避免panic hook／保護狀態污染一般單元測試。
use company_ai::{reliability, AppResult};
use std::path::Path;

unsafe extern "system" fn uncaught_callback() {
    panic!("fixture-private-user-path-must-not-be-recorded");
}

fn run() -> AppResult<()> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.len() != 2 {
        return Err("reliability_smoke <mode> <new-root>".into());
    }
    let root = Path::new(&arguments[1]);
    if root.exists() {
        return Err("異常測試只接受全新目錄。".into());
    }
    reliability::install(root)?;
    if arguments[0] == "abort" {
        // 證明未攔截ABI的中止前，hook已同步寫檔。父測試程序核對非零退出與檔案。
        unsafe {
            uncaught_callback();
        }
        return Err("未捕捉回呼應中止程序。".into());
    }
    company_ai::ui::verify_panic_recovery(&arguments[0], root)?;
    let events = serde_json::to_string(&reliability::events(root)).map_err(|e| e.to_string())?;
    if events == "[]"
        || events.contains("fixture-private")
        || !reliability::faulted()
        || reliability::log_failed()
    {
        return Err("異常紀錄／隱私／保護狀態驗證失敗。".into());
    }
    println!(
        "PASS: {} native callback/background, diagnostics and late-event quarantine",
        arguments[0]
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
