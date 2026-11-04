//! Word內建固定版面匯出；先產生並核對DOCX，再由同一Office引擎轉PDF。
use super::*;

pub fn pdf(source: &Path, output: &Path, cancel: &AtomicBool) -> AppResult<()> {
    if output.exists() || cancel.load(Ordering::Relaxed) {
        return Err("PDF輸出已存在或工作已取消。".into());
    }
    // SAFETY: 背景執行緒的STA只在本函式存活；Session先關閉文件，再釋放Apartment。
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() }.map_err(|e| e.to_string())?;
    let _apartment = Apartment;
    let session = Session::open(source)?;
    invoke(
        session.document()?,
        "ExportAsFixedFormat",
        vec![output.to_string_lossy().as_ref().into(), 17i32.into()],
        false,
    )?;
    Ok(())
}
