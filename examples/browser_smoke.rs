//! 開發用整合驗證：以真正瀏覽器確認授權，再用與 EXE 相同的通訊模組發出訊息。
//! 執行 cargo run --example browser_smoke，開啟印出的本機網址並點「允許登入」。
use company_ai::{
    auth,
    demo::DemoServer,
    protocol::{chat_json, Message},
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
    let body = chat_json(
        "demo-echo",
        &[Message::user("瀏覽器授權與中文 JSON 往返測試")],
    )?;
    let reply = auth::send_chat(&config, &session, &body).reply?;
    if !reply.contains("瀏覽器授權與中文 JSON 往返測試") {
        return Err("Unexpected reply".into());
    }
    if auth::poll_once(&config, &grant).is_ok() {
        return Err("One-time grant was reusable".into());
    }
    println!("PASS: browser approval -> token -> DPAPI -> chat reply -> replay rejected");
    Ok(())
}
