//! 只供建置機執行的真實 Python 分段量測，輸出可保存的 JSON。
#[cfg(debug_assertions)]
fn main() {
    if std::env::args().any(|arg| arg == "--prepare") {
        if let Err(error) = company_ai::projects::python::prepare_runtime() {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    match company_ai::projects::python::measure_runtime() {
        Ok(report) => {
            println!("{report}");
            if std::env::args().any(|arg| arg == "--hold") {
                // 安裝 PID 交接驗收：全域 runtime 鎖保留到此程序正常結束。
                use std::io::Write;
                println!("RUNTIME_LOCKS_HELD");
                std::io::stdout().flush().unwrap();
                std::thread::sleep(std::time::Duration::from_secs(30));
            }
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

#[cfg(not(debug_assertions))]
fn main() {
    eprintln!("此量測工具僅支援 debug 建置。正式程式不提供量測入口。");
    std::process::exit(2);
}
