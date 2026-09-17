//! 快捷鍵的解析、標準名稱與錄製資料驗證；不依賴前端鍵名拼法。
use crate::AppResult;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;

const NAMED_KEYS: &[(&str, u16)] = &[
    ("Esc", VK_ESCAPE),
    ("Space", VK_SPACE),
    ("Enter", VK_RETURN),
    ("Tab", VK_TAB),
    ("Backspace", VK_BACK),
    ("Delete", VK_DELETE),
    ("Insert", VK_INSERT),
    ("Home", VK_HOME),
    ("End", VK_END),
    ("PageUp", VK_PRIOR),
    ("PageDown", VK_NEXT),
    ("Left", VK_LEFT),
    ("Right", VK_RIGHT),
    ("Up", VK_UP),
    ("Down", VK_DOWN),
    ("Minus", VK_OEM_MINUS),
    ("Equal", VK_OEM_PLUS),
    ("Comma", VK_OEM_COMMA),
    ("Period", VK_OEM_PERIOD),
    ("Slash", VK_OEM_2),
    ("Semicolon", VK_OEM_1),
    ("Quote", VK_OEM_7),
    ("Backquote", VK_OEM_3),
    ("BracketLeft", VK_OEM_4),
    ("BracketRight", VK_OEM_6),
    ("Backslash", VK_OEM_5),
    ("NumpadAdd", VK_ADD),
    ("NumpadSubtract", VK_SUBTRACT),
    ("NumpadMultiply", VK_MULTIPLY),
    ("NumpadDivide", VK_DIVIDE),
    ("NumpadDecimal", VK_DECIMAL),
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hotkey {
    pub modifiers: u32,
    pub key: u32,
}
impl Hotkey {
    /// 舊設定與手動字串皆接受常見別名，儲存時統一為 Win+Ctrl+Alt+Shift+主鍵。
    pub fn parse(value: &str) -> AppResult<Self> {
        if value.len() > 100 {
            return Err("快捷鍵格式過長。".into());
        }
        let mut modifiers = 0;
        let mut key = None;
        for part in value.split('+').map(|s| s.trim().to_ascii_uppercase()) {
            let modifier = match part.as_str() {
                "WIN" | "WINDOWS" | "META" | "SUPER" => MOD_WIN,
                "CTRL" | "CONTROL" => MOD_CONTROL,
                "ALT" => MOD_ALT,
                "SHIFT" => MOD_SHIFT,
                _ => 0,
            };
            if modifier != 0 {
                if modifiers & modifier != 0 {
                    return Err("快捷鍵的修飾鍵不可重複。".into());
                }
                modifiers |= modifier;
                continue;
            }
            let alias = match part.as_str() {
                "ESCAPE" => "ESC",
                "RETURN" => "ENTER",
                "SPACEBAR" => "SPACE",
                "DEL" => "DELETE",
                "INS" => "INSERT",
                "PGUP" => "PAGEUP",
                "PGDN" => "PAGEDOWN",
                s => s,
            };
            let code = if alias.len() == 1 && alias.as_bytes()[0].is_ascii_alphanumeric() {
                Some(alias.as_bytes()[0] as u32)
            } else if let Some(n) = alias
                .strip_prefix('F')
                .and_then(|s| s.parse::<u32>().ok())
                .filter(|n| (1..=24).contains(n))
            {
                Some(VK_F1 as u32 + n - 1)
            } else if let Some(n) = alias
                .strip_prefix("NUMPAD")
                .and_then(|s| s.parse::<u32>().ok())
                .filter(|n| *n <= 9)
            {
                Some(VK_NUMPAD0 as u32 + n)
            } else {
                NAMED_KEYS
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case(alias))
                    .map(|(_, code)| *code as u32)
            };
            if key.is_some() || code.is_none() {
                return Err("請按「錄製」，再按一組包含 Win、Ctrl 或 Alt 的快捷鍵。".into());
            }
            key = code;
        }
        Self::from_keys(modifiers, key.ok_or("快捷鍵缺少主鍵。")?)
    }
    /// 原生 WebView2 與前端錄製共用這個驗證，不信任前端自行判定的合法性。
    pub fn from_keys(modifiers: u32, key: u32) -> AppResult<Self> {
        if modifiers & !(MOD_WIN | MOD_CONTROL | MOD_ALT | MOD_SHIFT) != 0
            || modifiers & (MOD_WIN | MOD_CONTROL | MOD_ALT) == 0
        {
            return Err("請搭配 Win、Ctrl 或 Alt；不能只用單鍵或 Shift。".into());
        }
        if key == VK_F12 as u32 {
            return Err("F12 由 Windows 偵錯器保留，請選其他按鍵。".into());
        }
        if modifiers == MOD_CONTROL && key == VK_C as u32 {
            return Err("Ctrl+C 用於擷取文字，請選其他組合。".into());
        }
        let result = Self { modifiers, key };
        if result.key_name().is_none() {
            return Err("這個按鍵尚不支援，請改用字母、數字、功能鍵或一般控制鍵。".into());
        }
        Ok(result)
    }
    fn key_name(&self) -> Option<String> {
        let key = self.key;
        if (VK_A as u32..=VK_Z as u32).contains(&key) || (VK_0 as u32..=VK_9 as u32).contains(&key)
        {
            Some((key as u8 as char).to_string())
        } else if (VK_F1 as u32..=VK_F24 as u32).contains(&key) {
            Some(format!("F{}", key - VK_F1 as u32 + 1))
        } else if (VK_NUMPAD0 as u32..=VK_NUMPAD9 as u32).contains(&key) {
            Some(format!("Numpad{}", key - VK_NUMPAD0 as u32))
        } else {
            NAMED_KEYS
                .iter()
                .find(|(_, code)| *code as u32 == key)
                .map(|(name, _)| name.to_string())
        }
    }
    pub fn label(&self) -> String {
        let mut parts = Vec::new();
        for (flag, name) in [
            (MOD_WIN, "Win"),
            (MOD_CONTROL, "Ctrl"),
            (MOD_ALT, "Alt"),
            (MOD_SHIFT, "Shift"),
        ] {
            if self.modifiers & flag != 0 {
                parts.push(name.to_string());
            }
        }
        if let Some(name) = self.key_name() {
            parts.push(name);
        }
        parts.join("+")
    }
}
/// 僅在錄製／已觸發的快捷鍵期間讀取修飾鍵狀態，不安裝全域鍵盤 hook。
pub(crate) fn pressed_modifiers() -> u32 {
    let mut result = 0;
    for (flag, keys) in [
        (MOD_WIN, [VK_LWIN, VK_RWIN]),
        (MOD_CONTROL, [VK_CONTROL, VK_CONTROL]),
        (MOD_ALT, [VK_MENU, VK_MENU]),
        (MOD_SHIFT, [VK_SHIFT, VK_SHIFT]),
    ] {
        if keys
            .iter()
            .any(|k| unsafe { GetAsyncKeyState(*k as i32) } < 0)
        {
            result |= flag;
        }
    }
    result
}
pub(crate) fn is_modifier(key: u32) -> bool {
    matches!(
        key as u16,
        VK_CONTROL
            | VK_LCONTROL
            | VK_RCONTROL
            | VK_MENU
            | VK_LMENU
            | VK_RMENU
            | VK_SHIFT
            | VK_LSHIFT
            | VK_RSHIFT
            | VK_LWIN
            | VK_RWIN
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aliases_and_recorded_keys_have_one_canonical_form() {
        for (input, expected) in [
            ("windows+Escape", "Win+Esc"),
            ("win+esc", "Win+Esc"),
            ("ctrl+shift+F8", "Ctrl+Shift+F8"),
            ("Alt+1", "Alt+1"),
            ("Ctrl+Spacebar", "Ctrl+Space"),
            ("Meta+PgDn", "Win+PageDown"),
            ("Ctrl+Numpad2", "Ctrl+Numpad2"),
            ("Ctrl+BracketLeft", "Ctrl+BracketLeft"),
        ] {
            let hotkey = Hotkey::parse(input).unwrap();
            assert_eq!(hotkey.label(), expected);
            assert_eq!(
                Hotkey::from_keys(hotkey.modifiers, hotkey.key).unwrap(),
                hotkey
            );
        }
    }
    #[test]
    fn reject_incomplete_duplicate_reserved_and_copy_combinations() {
        for key in [
            "Esc",
            "Shift+A",
            "Ctrl+C",
            "Win+F12",
            "Ctrl+Alt",
            "Ctrl+Q+W",
            "Ctrl+Ctrl+Q",
            "Win+Windows+Esc",
            "Alt+F25",
        ] {
            assert!(Hotkey::parse(key).is_err(), "{key}");
        }
    }
}
