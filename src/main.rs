#![windows_subsystem = "windows"]
//! 雙擊 EXE 直接開啟視窗；--demo 提供本機流程示範，--self-check 供編譯腳本驗證。
use company_ai::{demo::DemoServer, ui, AppResult};

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let smoke = arguments.iter().any(|argument| argument == "--self-check");
    let result: AppResult<()> = (|| {
        if arguments
            .iter()
            .any(|argument| !matches!(argument.as_str(), "--demo" | "--self-check"))
        {
            return Err("支援的參數為 --demo 或 --self-check；直接開啟則使用公司連線設定。".into());
        }
        let demo = if arguments.iter().any(|argument| argument == "--demo") {
            Some(DemoServer::start()?)
        } else {
            None
        };
        ui::run(demo.as_ref(), smoke)
    })();
    if let Err(error) = result {
        if smoke {
            eprintln!("{error}");
        } else {
            ui::show_fatal_error(&error);
        }
        std::process::exit(1);
    }
}
