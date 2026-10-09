//! 專案圖片的唯讀輸入：將專案內常見點陣圖片轉成 JPG，不下載網址、不改動來源。
use crate::{
    projects::{files, Project},
    AppResult,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path};

/// 介面使用十進位 MB：5 MB = 5,000,000 bytes；Base64 後另檢查整份請求。
pub const MAX_BYTES: usize = 5_000_000;

/// 快速模型沒有圖片能力；技能公告、原生入口與工具執行共用此判斷。
pub fn model_supported(model: &str) -> bool {
    model != "fast"
}
pub const UNSUPPORTED_MODEL: &str = "此模型不支援圖片傳入，請切換至支援圖片的模型。";

pub struct Image {
    pub path: String,
    pub sha256: String,
    pub mime: &'static str,
    pub width: u32,
    pub height: u32,
    pub(crate) bytes: Vec<u8>,
    pub source_bytes: usize,
    pub frames: u32,
}

pub fn supported(path: &Path) -> bool {
    path.extension().and_then(|s| s.to_str()).is_some_and(|s| {
        matches!(
            s.to_ascii_lowercase().as_str(),
            "jpg" | "jpeg" | "png" | "bmp" | "tif" | "tiff" | "gif"
        )
    })
}

/// 鎖定祖先目錄與檔案至讀取完成；內容雜湊識別實際送出的版本。
pub fn load(project: &Project, value: &str) -> AppResult<Image> {
    let relative = files::relative(value)?;
    if !supported(&relative) {
        return Err("圖片辨識支援專案內 JPG／JPEG／PNG／BMP／TIF／TIFF／GIF。".into());
    }
    let path = project.root.join(&relative);
    let _guards = files::pin(path.parent().ok_or("缺少圖片資料夾。")?)?;
    let mut file = files::checked_file(&path)?;
    files::reject_internal(&file)?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(64_000_001)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.is_empty() || bytes.len() > 64_000_000 {
        return Err("原始圖片限64 MB以内；轉成JPG後另檢查5 MB上限。".into());
    }
    let extension = relative
        .extension()
        .and_then(|s| s.to_str())
        .ok_or("圖片缺少副檔名。")?
        .to_ascii_lowercase();
    let converted = super::jpeg::convert(&bytes, &extension)?;
    Ok(Image {
        path: relative.to_string_lossy().into_owned(),
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        mime: "image/jpeg",
        width: converted.width,
        height: converted.height,
        source_bytes: bytes.len(),
        frames: converted.frames,
        bytes: converted.bytes,
    })
}

impl Image {
    pub fn metadata(&self) -> Value {
        json!({"path":self.path,"sha256":self.sha256,"mime":self.mime,"width":self.width,"height":self.height,"bytes":self.bytes.len(),"source_bytes":self.source_bytes,"jpeg_sha256":format!("{:x}",Sha256::digest(&self.bytes)),"conversion":"jpeg-quality-85-white-exif-v1","frame":1,"frames":self.frames,"frame_note":"多頁TIFF或動畫只辨識第一頁／第一幀，不代表已讀其餘頁面"})
    }
    /// Base64 只進入圖片請求及 DPAPI 待查快照，不放入一般工具結果或進度筆記。
    pub fn data_url(&self) -> AppResult<String> {
        use windows_sys::Win32::Security::Cryptography::*;
        let mut encoded = vec![0u8; self.bytes.len().div_ceil(3) * 4 + 1];
        let mut size = encoded.len() as u32;
        let ok = unsafe {
            CryptBinaryToStringA(
                self.bytes.as_ptr(),
                self.bytes.len() as u32,
                CRYPT_STRING_BASE64 | CRYPT_STRING_NOCRLF,
                encoded.as_mut_ptr(),
                &mut size,
            )
        };
        if ok == 0 {
            return Err("圖片 Base64 編碼失敗。".into());
        }
        let text =
            std::str::from_utf8(&encoded[..size as usize]).map_err(|_| "圖片編碼不是 ASCII。")?;
        Ok(format!(
            "data:{};base64,{}",
            self.mime,
            text.trim_end_matches('\0')
        ))
    }
}

/// 一次最多兩張，先逐張轉檔，再核對二進位總量；Base64 開銷由請求層檢查。
pub fn validate_batch(images: &[Image]) -> AppResult<()> {
    if !(1..=2).contains(&images.len()) || images.iter().any(|image| image.bytes.len() > MAX_BYTES)
    {
        return Err("一次辨識需1–2張圖片，每張JPG不超過5 MB。".into());
    }
    if images.iter().map(|image| image.bytes.len()).sum::<usize>() > 8_000_000 {
        return Err("兩張圖片轉成JPG後合計超過8 MB（8,000,000 bytes），請縮小後再試。".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn converted_sizes_use_decimal_limits_and_two_image_total() {
        let image = |size| Image {
            path: "test.jpg".into(),
            sha256: "fixture".into(),
            mime: "image/jpeg",
            width: 1,
            height: 1,
            source_bytes: size,
            frames: 1,
            bytes: vec![0; size],
        };
        assert!(validate_batch(&[image(5_000_000)]).is_ok());
        assert!(validate_batch(&[image(5_000_001)]).is_err());
        assert!(validate_batch(&[image(5_000_000), image(3_000_000)]).is_ok());
        assert!(validate_batch(&[image(5_000_000), image(3_000_001)]).is_err());
        assert!(validate_batch(&[image(1), image(1), image(1)]).is_err());
    }
}
