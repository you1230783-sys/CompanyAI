//! 只在使用者按全域快捷鍵時取得選字。不監聽一般按鍵，也不持續讀取剪貼簿。
//! 第一版採一般複製行為：剪貼簿會被選取文字取代，不宣稱保留圖片或富文字格式。
use crate::{wide, AppResult};
use std::{
    ptr, thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    System::{DataExchange::*, Memory::*, Ole::CF_UNICODETEXT},
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

pub const HOTKEY_ID: i32 = 0x4341;
const MAX_TEXT_UNITS: usize = 16_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hotkey {
    pub modifiers: u32,
    pub key: u32,
}
impl Hotkey {
    /// 使用 Ctrl+Alt 或 Ctrl+Shift 加字母／功能鍵；不接受 Windows 保留快捷鍵。
    pub fn parse(value: &str) -> AppResult<Self> {
        let parts: Vec<String> = value
            .split('+')
            .map(|s| s.trim().to_ascii_uppercase())
            .collect();
        let mut modifiers = 0;
        let mut key = None;
        for part in parts {
            let modifier = match part.as_str() {
                "CTRL" => MOD_CONTROL,
                "ALT" => MOD_ALT,
                "SHIFT" => MOD_SHIFT,
                _ => 0,
            };
            if modifier != 0 {
                if modifiers & modifier != 0 {
                    return Err("快捷鍵的修飾鍵不可重複。".into());
                }
                modifiers |= modifier;
            } else {
                let code = if part.len() == 1 && part.as_bytes()[0].is_ascii_uppercase() {
                    Some(part.as_bytes()[0] as u32)
                } else {
                    part.strip_prefix('F')
                        .and_then(|s| s.parse::<u32>().ok())
                        .filter(|n| (1..=24).contains(n))
                        .map(|number| VK_F1 as u32 + number - 1)
                };
                if key.is_some() || code.is_none() {
                    return Err("請使用例如 Ctrl+Alt+Q 或 Ctrl+Shift+F8 的快捷鍵。".into());
                }
                key = code;
            }
        }
        if modifiers & MOD_CONTROL == 0 || modifiers & (MOD_ALT | MOD_SHIFT) == 0 {
            return Err("快捷鍵需包含 Ctrl，以及 Alt 或 Shift。".into());
        }
        Ok(Self {
            modifiers,
            key: key.ok_or("快捷鍵缺少字母或功能鍵。")?,
        })
    }
}

pub(crate) fn register(window: HWND, hotkey: Hotkey) -> AppResult<()> {
    // SAFETY: 呼叫者擁有有效主視窗；Windows 以 WM_HOTKEY 通知，不安裝鍵盤 hook。
    if unsafe {
        RegisterHotKey(
            window,
            HOTKEY_ID,
            hotkey.modifiers | MOD_NOREPEAT,
            hotkey.key,
        )
    } == 0
    {
        Err("快捷鍵已被其他程式使用，請修改後再套用。".into())
    } else {
        Ok(())
    }
}
pub(crate) fn unregister(window: HWND) {
    unsafe {
        UnregisterHotKey(window, HOTKEY_ID);
    }
}

fn key_is_down(key: u16) -> bool {
    unsafe { GetAsyncKeyState(key as i32) < 0 }
}
fn same_process(first: HWND, second: HWND) -> bool {
    let (mut first_pid, mut second_pid) = (0, 0);
    unsafe {
        GetWindowThreadProcessId(first, &mut first_pid);
        GetWindowThreadProcessId(second, &mut second_pid);
    }
    first_pid != 0 && first_pid == second_pid
}

/// 從快捷鍵發生時的前景視窗複製；必須先完成這一步，再把本程式帶到前景。
pub(crate) fn capture(source: HWND, hotkey: Hotkey) -> AppResult<String> {
    let released_by = Instant::now() + Duration::from_millis(1500);
    while [
        VK_CONTROL,
        VK_MENU,
        VK_SHIFT,
        VK_LWIN,
        VK_RWIN,
        hotkey.key as u16,
    ]
    .iter()
    .any(|key| key_is_down(*key))
    {
        if Instant::now() >= released_by {
            return Err("請放開快捷鍵後重新擷取。".into());
        }
        thread::sleep(Duration::from_millis(15));
    }
    unsafe {
        if source.is_null() || GetForegroundWindow() != source {
            return Err("來源視窗已切換，這次沒有擷取文字。".into());
        }
        let previous_sequence = GetClipboardSequenceNumber();
        if previous_sequence == 0 {
            return Err("無法存取目前的剪貼簿，請稍後再試。".into());
        }
        let keys = [
            (VK_CONTROL, 0),
            (VK_C, 0),
            (VK_C, KEYEVENTF_KEYUP),
            (VK_CONTROL, KEYEVENTF_KEYUP),
        ];
        let inputs: Vec<INPUT> = keys
            .iter()
            .map(|(key, flags)| INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: *key,
                        dwFlags: *flags,
                        ..std::mem::zeroed()
                    },
                },
            })
            .collect();
        if SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        ) != inputs.len() as u32
        {
            // 若只送入部分按鍵，補上 release，避免留下程式按下的 Ctrl 狀態。
            SendInput(
                2,
                inputs.as_ptr().add(2),
                std::mem::size_of::<INPUT>() as i32,
            );
            return Err("無法複製選取文字；來源程式可能使用較高權限。可手動複製貼上。".into());
        }
        let deadline = Instant::now() + Duration::from_millis(1800);
        while Instant::now() < deadline {
            if GetForegroundWindow() != source {
                return Err("來源視窗已切換，這次沒有擷取文字。".into());
            }
            let sequence = GetClipboardSequenceNumber();
            if sequence != 0 && sequence != previous_sequence {
                // 序號更新還不夠：拒絕同時由其他程式寫入的剪貼簿內容。
                if !same_process(source, GetClipboardOwner()) {
                    return Err("剪貼簿被其他程式更新，請重新選字擷取。".into());
                }
                if OpenClipboard(ptr::null_mut()) != 0 {
                    let result = read_open_clipboard();
                    CloseClipboard();
                    if GetClipboardSequenceNumber() != sequence {
                        return Err("擷取期間剪貼簿又有變更，請再試一次。".into());
                    }
                    return result;
                }
            }
            thread::sleep(Duration::from_millis(25));
        }
    }
    Err("沒有取得這次選取的文字；請先選字再試，或手動複製貼上。程式不會使用舊剪貼簿。".into())
}

/// 只在剪貼簿已開啟時呼叫，依 GlobalSize 限制讀取範圍，避免無界掃描。
unsafe fn read_open_clipboard() -> AppResult<String> {
    let handle = GetClipboardData(CF_UNICODETEXT as u32);
    if handle.is_null() {
        return Err("選取內容不是可讀取的純文字。".into());
    }
    let size = GlobalSize(handle);
    if !(2..=(MAX_TEXT_UNITS + 1) * 2).contains(&size) {
        return Err("選取文字過長，請縮短至 16,000 字元以內。".into());
    }
    let pointer = GlobalLock(handle).cast::<u16>();
    if pointer.is_null() {
        return Err("剪貼簿正在使用中，請再試一次。".into());
    }
    let units = std::slice::from_raw_parts(pointer, size / 2);
    let result = decode_text(units);
    GlobalUnlock(handle);
    result
}
fn decode_text(units: &[u16]) -> AppResult<String> {
    let end = units
        .iter()
        .position(|value| *value == 0)
        .ok_or("剪貼簿文字缺少結尾。")?;
    if end > MAX_TEXT_UNITS {
        return Err("選取文字過長。".into());
    }
    let text = String::from_utf16(&units[..end]).map_err(|_| "剪貼簿文字編碼不正確。")?;
    if text.trim().is_empty() {
        return Err("沒有選取文字，請先選字再試。".into());
    }
    Ok(text.replace("\r\n", "\n"))
}

/// 明確按「複製回覆」才寫入剪貼簿，不自動貼回其他程式。
pub(crate) fn copy_text(window: HWND, text: &str) -> AppResult<()> {
    let units = wide(text);
    unsafe {
        let allocation = GlobalAlloc(GMEM_MOVEABLE, units.len() * 2);
        if allocation.is_null() {
            return Err("無法配置剪貼簿記憶體。".into());
        }
        let pointer = GlobalLock(allocation).cast::<u16>();
        if pointer.is_null() {
            GlobalFree(allocation);
            return Err("無法寫入剪貼簿。".into());
        }
        ptr::copy_nonoverlapping(units.as_ptr(), pointer, units.len());
        GlobalUnlock(allocation);
        if OpenClipboard(window) == 0 {
            GlobalFree(allocation);
            return Err("剪貼簿正被使用，請再按一次複製。".into());
        }
        let success =
            EmptyClipboard() != 0 && !SetClipboardData(CF_UNICODETEXT as u32, allocation).is_null();
        CloseClipboard();
        if !success {
            GlobalFree(allocation);
            return Err("無法更新剪貼簿。".into());
        }
        // 成功後記憶體由 Windows 擁有，不再 GlobalFree。
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shortcut_parser_rejects_reserved_and_ambiguous_bindings() {
        assert_eq!(Hotkey::parse("Ctrl+Alt+Q").unwrap().key, VK_Q as u32);
        assert_eq!(Hotkey::parse("ctrl+shift+F8").unwrap().key, VK_F8 as u32);
        for key in [
            "Win+Space",
            "Ctrl+C",
            "Ctrl+Alt",
            "Ctrl+Alt+Q+W",
            "Ctrl+Ctrl+Alt+Q",
            "Ctrl+Alt+F25",
        ] {
            assert!(Hotkey::parse(key).is_err());
        }
    }
    #[test]
    fn captured_text_must_be_nonempty_bounded_and_terminated() {
        assert_eq!(
            decode_text(&wide("選取文字\r\n第二行")).unwrap(),
            "選取文字\n第二行"
        );
        assert!(decode_text(&[65]).is_err());
        assert!(decode_text(&wide(" \n")).is_err());
        assert!(decode_text(&wide(&"x".repeat(MAX_TEXT_UNITS + 1))).is_err());
        assert!(decode_text(&[0xd800, 0]).is_err());
    }
}
