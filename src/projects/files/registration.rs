//! 分析／數值登記的驗證失敗與保存失敗分開，避免把部分寫入誤報為未執行。
#[derive(Debug)]
pub(super) struct RegistrationError {
    pub message: String,
    pub contract: bool,
}

impl RegistrationError {
    /// I/O 或已進入記憶保存階段：不能保證未執行，沿用一般錯誤分類。
    pub fn execution(message: String) -> Self {
        Self {
            message,
            contract: false,
        }
    }
}

// 驗證函式沿用 AppResult；只有明確標記的 I/O／保存呼叫轉成 execution。
impl From<String> for RegistrationError {
    fn from(message: String) -> Self {
        Self {
            message,
            contract: true,
        }
    }
}
impl From<&str> for RegistrationError {
    fn from(message: &str) -> Self {
        message.to_owned().into()
    }
}
