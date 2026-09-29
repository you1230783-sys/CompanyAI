//! IDispatch 的 HRESULT 可能只代表「應用程式例外」；保留 EXCEPINFO 的真正原因。
use std::mem::ManuallyDrop;
use windows::{core::Error, Win32::System::Com::EXCEPINFO};

/// EXCEPINFO 的 BSTR 是呼叫端所有；所有回傳路徑皆由 Drop 釋放。
#[derive(Default)]
pub(crate) struct DispatchException(pub EXCEPINFO);
impl DispatchException {
    pub fn describe(&mut self, operation: &str, error: &Error) -> String {
        if let Some(fill) = self.0.pfnDeferredFillIn.take() {
            unsafe {
                let _ = fill(&mut self.0);
            }
        }
        let description: String = self
            .0
            .bstrDescription
            .to_string()
            .chars()
            .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
            .take(1200)
            .collect();
        let mut message = format!("{operation}（{}）", error.code());
        if self.0.scode != 0 {
            message.push_str(&format!("；應用程式錯誤碼 {:#010x}", self.0.scode as u32));
        }
        if self.0.wCode != 0 {
            message.push_str(&format!("；代碼 {}", self.0.wCode));
        }
        if !description.trim().is_empty() {
            message.push_str(&format!("：{}", description.trim()));
        }
        message
    }
}
impl Drop for DispatchException {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.0.bstrSource);
            ManuallyDrop::drop(&mut self.0.bstrDescription);
            ManuallyDrop::drop(&mut self.0.bstrHelpFile);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use windows::core::{BSTR, HRESULT};
    #[test]
    fn preserves_application_reason_and_deferred_details() {
        unsafe extern "system" fn fill(info: *mut EXCEPINFO) -> HRESULT {
            unsafe {
                (*info).bstrDescription = ManuallyDrop::new(BSTR::from("檔案已開啟或權限不足"));
                (*info).scode = 0x80070020u32 as i32;
            }
            HRESULT(0)
        }
        let mut exception = DispatchException::default();
        exception.0.pfnDeferredFillIn = Some(fill);
        let result = exception.describe(
            "OpenSharedItem",
            &Error::from_hresult(HRESULT(0x80020009u32 as i32)),
        );
        assert!(
            result.contains("檔案已開啟")
                && result.contains("80070020")
                && result.contains("80020009")
        );
        assert!(exception.0.pfnDeferredFillIn.is_none());
    }
}
