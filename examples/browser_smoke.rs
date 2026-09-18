//! 開發用整合驗證：以真正瀏覽器確認授權，再用與 EXE 相同的通訊模組發出訊息。
//! 執行 cargo run --example browser_smoke，開啟印出的本機網址並點「允許登入」。
use company_ai::{
    auth,
    demo::DemoServer,
    jobs,
    protocol::{self, Message},
    storage,
};
use std::sync::atomic::AtomicBool;

fn main() -> Result<(), String> {
    let server = DemoServer::start()?;
    let config = server.config();
    let grant = auth::request_device(&config)?;
    println!("BROWSER_URL={}", grant.browser_url());
    println!("USER_CODE={}", grant.user_code);
    let session = auth::wait_for_login(&config, &grant, &AtomicBool::new(false))?;
    // 只在測試記憶體驗證 DPAPI，避免改動使用者已保存的公司登入。
    let encrypted = storage::protect(session.access_token.as_bytes(), true)?;
    if storage::protect(&encrypted, false)? != session.access_token.as_bytes() {
        return Err("DPAPI mismatch".into());
    }
    let local = jobs::new_id()?;
    let remote = jobs::conversation(&config, &session, &local)?;
    let id = jobs::new_id()?;
    let request = jobs::chat_request(
        "fast",
        &[Message::user("瀏覽器授權與中文 JSON 往返測試")],
        &remote,
        &id,
        "background",
        vec![],
    )?;
    let task = jobs::Task {
        request_id: id,
        conversation_id: local,
        request,
        mode: "background".into(),
        title: "瀏覽器整合測試".into(),
        created_at: company_ai::unix_now(),
        remote: None,
        applied: false,
        message: String::new(),
        mail_analysis: false,
        title_generation: false,
        tool_events: vec![],
        partial: String::new(),
    };
    jobs::submit(&config, &session, &task, |_| {})?;
    let reply = loop {
        let status = jobs::task_status(&config, &session, &task)?;
        if status.state == "completed" {
            break protocol::assistant_text(&status.result.ok_or("Missing result")?.to_string())?;
        }
        if status.terminal() {
            return Err(status.error_message);
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    };
    if !reply.contains("瀏覽器授權與中文 JSON 往返測試") {
        return Err("Unexpected reply".into());
    }
    if auth::poll_once(&config, &grant).is_ok() {
        return Err("One-time grant was reusable".into());
    }
    println!("PASS: browser approval -> token -> DPAPI -> chat reply -> replay rejected");
    Ok(())
}
