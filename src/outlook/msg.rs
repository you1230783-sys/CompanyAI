//! 僅開啟經專案 broker 驗證的 MSG，不枚舉信箱、不匯入郵件或讀取附件。
use super::*;
use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

struct MessageFile(IDispatch);
impl Drop for MessageFile {
    fn drop(&mut self) {
        let _ = get(&self.0, "Close", &mut [1i32.into()]);
    } // olDiscard
}

pub fn read(path: &Path, cancel: &AtomicBool) -> AppResult<String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("MSG 讀取已取消。".into());
    }
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() }.map_err(|e| e.to_string())?;
    let _apartment = ComApartment;
    let class = unsafe { CLSIDFromProgID(PCWSTR(wide("Outlook.Application").as_ptr())) }
        .map_err(|_| "MSG 需要已安裝的 Classic Outlook。")?;
    // Outlook 為共用桌面程式；不變更安全設定，也不 Quit 使用者的 Outlook。
    let mut active: Option<IUnknown> = None;
    unsafe { GetActiveObject(&class, None, &mut active) }.map_err(|_| {
        "請先開啟已完成帳號設定的 Classic Outlook，再讀取 MSG；或使用專案匯入文字。"
    })?;
    let app: IDispatch = active
        .ok_or("找不到已開啟的 Classic Outlook。")?
        .cast()
        .map_err(|_| "無法連接 Classic Outlook。")?;
    let namespace = object(&get(&app, "GetNamespace", &mut ["MAPI".into()])?)?;
    let mail = MessageFile(object(&get(
        &namespace,
        "OpenSharedItem",
        &mut [path.to_string_lossy().as_ref().into()],
    )?)?);
    if i32::try_from(&get(&mail.0, "Class", &mut [])?).ok() != Some(43) {
        return Err("本版 MSG 僅支援郵件，不支援行事曆或其他 Outlook 項目。".into());
    }
    let mut result = String::from("MSG 郵件文字（不含附件）\n");
    for (label, property) in [
        ("主旨", "Subject"),
        ("寄件者", "SenderName"),
        ("收件者", "To"),
        ("副本", "CC"),
        ("寄件時間", "SentOn"),
    ] {
        result.push_str(&format!("{label}：{}\n", text(&mail.0, property, 10_000)?));
    }
    result.push('\n');
    result.push_str(&text(&mail.0, "Body", crate::projects::text::MAX_TEXT)?);
    crate::projects::text::validate(&result)?;
    if cancel.load(Ordering::Relaxed) {
        return Err("MSG 讀取已取消。".into());
    }
    Ok(result)
}
