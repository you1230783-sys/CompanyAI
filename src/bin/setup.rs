#![windows_subsystem = "windows"]
//! 安裝包內嵌已驗證的主程式與 WebView2 離線安裝檔，不需使用者自行解壓。
include!(concat!(env!("OUT_DIR"), "/setup_payload.rs"));
fn main() {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments.first().is_some_and(|a| a == "--verify-payload") {
        // 建置檢查逐位元組核對嵌入檔案，避免錯把上一版 EXE 打進新版 Setup。
        if arguments.len() != 3
            || !APP.starts_with(b"MZ")
            || !RUNTIME.starts_with(b"MZ")
            || std::fs::read(&arguments[1]).ok().as_deref() != Some(APP)
            || std::fs::read(&arguments[2]).ok().as_deref() != Some(RUNTIME)
        {
            std::process::exit(1);
        }
        return;
    }
    if let Err(e) = company_ai::deployment::install(APP, RUNTIME) {
        company_ai::ui::show_fatal_error(&e);
        std::process::exit(1);
    }
}
