//! 單次 Windows 框選擷取。只在使用者開始後等待剪貼簿變更，取消或逾時即停止。
use crate::{projects::vision::jpeg, wide, AppResult};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    System::{DataExchange::*, Memory::*},
    UI::Shell::ShellExecuteW,
};

pub const HOTKEY_ID: i32 = 0x4342;
const MAX_DIB: usize = 64 * 1024 * 1024;

/// 僅啟動固定的 Windows 框選 URI，外部文字無法指定程式或參數。
pub fn capture(cancel: &AtomicBool) -> AppResult<Vec<u8>> {
    // SAFETY: GetClipboardSequenceNumber 不取得所有權；URI及動詞在呼叫期間有效。
    let sequence = unsafe { GetClipboardSequenceNumber() };
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            wide("open").as_ptr(),
            wide("ms-screenclip:").as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    };
    if result as isize <= 32 {
        return Err("無法啟動 Windows 剪取工具，請確認已安裝。".into());
    }
    let deadline = Instant::now() + Duration::from_secs(45);
    while Instant::now() < deadline && !cancel.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(100));
        // SAFETY: 只比較序號；持有剪貼簿鎖時才讀取資料。
        if unsafe { GetClipboardSequenceNumber() } != sequence {
            if let Some(dib) = read_dib()? {
                let image = jpeg::convert(&dib_to_bmp(&dib)?, "bmp")?;
                if image.bytes.len() > crate::projects::vision::input::MAX_BYTES {
                    return Err("截圖超過5 MB，請縮小框選範圍。".into());
                }
                return Ok(image.bytes);
            }
        }
    }
    Err("截圖已取消或超過45秒；沒有保存或上傳圖片。".into())
}

/// 剪貼簿由其他程式管理，先確認大小再複製，解鎖後才進行解碼。
fn read_dib() -> AppResult<Option<Vec<u8>>> {
    // SAFETY: OpenClipboard 成功後以RAII關閉；GlobalLock 僅於鎖定期間讀取。
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return Ok(None);
        }
        struct Clipboard;
        impl Drop for Clipboard {
            fn drop(&mut self) {
                unsafe {
                    CloseClipboard();
                }
            }
        }
        let _clipboard = Clipboard;
        // CF_DIB=8。由WIC處理像素；不把 HBITMAP 指標當成位元組。
        if IsClipboardFormatAvailable(8) == 0 {
            return Ok(None);
        }
        let handle = GetClipboardData(8);
        if handle.is_null() {
            return Ok(None);
        }
        let size = GlobalSize(handle);
        if !(40..=MAX_DIB).contains(&size) {
            return Err("剪貼簿圖片大小不合理。".into());
        }
        let memory = GlobalLock(handle);
        if memory.is_null() {
            return Ok(None);
        }
        struct Locked(windows_sys::Win32::Foundation::HGLOBAL);
        impl Drop for Locked {
            fn drop(&mut self) {
                unsafe {
                    GlobalUnlock(self.0);
                }
            }
        }
        let _locked = Locked(handle);
        Ok(Some(
            std::slice::from_raw_parts(memory.cast::<u8>(), size).to_vec(),
        ))
    }
}

/// 只在使用者於私聊貼上時讀取目前剪貼簿，不建立明文暫存檔。
pub(crate) fn private_clipboard_image() -> AppResult<Vec<u8>> {
    let dib = read_dib()?.ok_or("剪貼簿目前沒有可用的截圖。")?;
    Ok(jpeg::convert_private(&dib_to_bmp(&dib)?, "bmp")?.bytes)
}

/// 僅接受常見未壓縮 24/32-bit DIB；嚴格檢查長度，拒絕惡意標頭或超大圖片。
fn dib_to_bmp(dib: &[u8]) -> AppResult<Vec<u8>> {
    if dib.len() < 40 || dib.len() > MAX_DIB {
        return Err("截圖標頭不完整。".into());
    }
    let u32_at = |at| u32::from_le_bytes([dib[at], dib[at + 1], dib[at + 2], dib[at + 3]]);
    let header = u32_at(0) as usize;
    let width = u32_at(4) as i32;
    let height = u32_at(8) as i32;
    let planes = u16::from_le_bytes([dib[12], dib[13]]);
    let bits = u16::from_le_bytes([dib[14], dib[15]]) as usize;
    let compression = u32_at(16);
    if ![40, 108, 124].contains(&header)
        || header > dib.len()
        || !(1..=8192).contains(&width)
        || height == 0
        || height.unsigned_abs() > 8192
        || (width as u64) * u64::from(height.unsigned_abs()) > 16_000_000
        || planes != 1
        || ![24, 32].contains(&bits)
        || ![0, 3].contains(&compression)
        || u32_at(32) != 0
    {
        return Err("截圖格式或尺寸不支援，請重新框選較小範圍。".into());
    }
    let offset = header
        + if header == 40 && compression == 3 {
            12
        } else {
            0
        };
    let stride = (width as usize * bits).div_ceil(32) * 4;
    let required = offset + stride * height.unsigned_abs() as usize;
    if required > dib.len() {
        return Err("截圖像素資料不完整。".into());
    }
    let mut bmp = Vec::with_capacity(required + 14);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&((required + 14) as u32).to_le_bytes());
    bmp.extend_from_slice(&[0; 4]);
    bmp.extend_from_slice(&((offset + 14) as u32).to_le_bytes());
    bmp.extend_from_slice(&dib[..required]);
    Ok(bmp)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(height: i32) -> Vec<u8> {
        let mut dib = vec![0; 56];
        dib[..4].copy_from_slice(&40u32.to_le_bytes());
        dib[4..8].copy_from_slice(&2i32.to_le_bytes());
        dib[8..12].copy_from_slice(&height.to_le_bytes());
        dib[12..14].copy_from_slice(&1u16.to_le_bytes());
        dib[14..16].copy_from_slice(&32u16.to_le_bytes());
        dib[40..].fill(200);
        dib
    }
    #[test]
    fn dib_validates_bounds_and_top_down_images_with_real_wic() {
        for height in [2, -2] {
            let bmp = dib_to_bmp(&fixture(height)).unwrap();
            let jpeg = jpeg::convert(&bmp, "bmp").unwrap();
            assert_eq!((jpeg.width, jpeg.height), (2, 2));
            assert!(jpeg.bytes.starts_with(&[255, 216]));
        }
        for end in 0..56 {
            assert!(dib_to_bmp(&fixture(2)[..end]).is_err());
        }
        assert!(dib_to_bmp(&fixture(i32::MIN)).is_err());
        let mut dib = fixture(2);
        dib[..4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(dib_to_bmp(&dib).is_err());
        let mut dib = fixture(2);
        dib[16..20].copy_from_slice(&1u32.to_le_bytes());
        assert!(dib_to_bmp(&dib).is_err());
    }
}
