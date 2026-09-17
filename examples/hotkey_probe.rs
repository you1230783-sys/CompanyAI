//! 手動驗證本機是否能註冊 Win+Esc；不產生按鍵、不讀剪貼簿，驗證後立即解除。
use company_ai::hotkey::Hotkey;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, MOD_NOREPEAT,
};
fn main() {
    let hotkey = Hotkey::parse("Win+Esc").expect("fixed valid shortcut");
    unsafe {
        if RegisterHotKey(
            std::ptr::null_mut(),
            0x4c4d,
            hotkey.modifiers | MOD_NOREPEAT,
            hotkey.key,
        ) != 0
        {
            UnregisterHotKey(std::ptr::null_mut(), 0x4c4d);
            println!("Win+Esc: registration succeeded; released immediately.");
        } else {
            eprintln!(
                "Win+Esc is unavailable on this desktop: {}",
                std::io::Error::last_os_error()
            );
            std::process::exit(1);
        }
    }
}
