//! 圖片試驗的唯讀輸入：只接受專案內 JPG／PNG，不下載網址、不改動來源。
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
    bytes: Vec<u8>,
}

pub fn supported(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| matches!(s.to_ascii_lowercase().as_str(), "jpg" | "jpeg" | "png"))
}

/// 鎖定祖先目錄與檔案至讀取完成；內容雜湊識別實際送出的版本。
pub fn load(project: &Project, value: &str) -> AppResult<Image> {
    let relative = files::relative(value)?;
    if !supported(&relative) {
        return Err("圖片試驗只支援專案內 JPG／JPEG／PNG。".into());
    }
    let path = project.root.join(&relative);
    let _guards = files::pin(path.parent().ok_or("缺少圖片資料夾。")?)?;
    let mut file = files::checked_file(&path)?;
    files::reject_internal(&file)?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let (mime, width, height) = inspect(&bytes)?;
    let png_name = relative
        .extension()
        .is_some_and(|s| s.eq_ignore_ascii_case("png"));
    if png_name != (mime == "image/png") {
        return Err("圖片副檔名與內容格式不一致。".into());
    }
    Ok(Image {
        path: relative.to_string_lossy().into_owned(),
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        mime,
        width,
        height,
        bytes,
    })
}

impl Image {
    pub fn metadata(&self) -> Value {
        json!({"path":self.path,"sha256":self.sha256,"mime":self.mime,"width":self.width,"height":self.height,"bytes":self.bytes.len()})
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

/// 檢查容器與尺寸，不在桌面執行影像解碼。JPEG 的像素可解碼性由模型端回報。
fn inspect(bytes: &[u8]) -> AppResult<(&'static str, u32, u32)> {
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err(
            "單張圖片最大 5 MB（5,000,000 bytes），請自行縮小圖片後再試；程式不會自動壓縮。".into(),
        );
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        let (w, h) = crate::projects::charts::png::dimensions(bytes)?;
        return Ok(("image/png", w, h));
    }
    if !bytes.starts_with(&[0xff, 0xd8]) || !bytes.ends_with(&[0xff, 0xd9]) {
        return Err("不是完整的 JPG 或 PNG 圖片。".into());
    }
    let mut offset = 2;
    let mut dimensions = None;
    while offset + 4 <= bytes.len() {
        if bytes[offset] != 0xff {
            break;
        }
        while offset < bytes.len() && bytes[offset] == 0xff {
            offset += 1;
        }
        let marker = *bytes.get(offset).ok_or("JPEG 標記不完整。")?;
        offset += 1;
        if marker == 0xd9 || marker == 0 || (0xd0..=0xd8).contains(&marker) {
            break;
        }
        let length = bytes.get(offset..offset + 2).ok_or("JPEG 區段不完整。")?;
        let length = u16::from_be_bytes([length[0], length[1]]) as usize;
        let end = offset
            .checked_add(length)
            .filter(|end| length >= 2 && *end <= bytes.len())
            .ok_or("JPEG 區段超出範圍。")?;
        if marker == 0xda {
            // SOS 後是壓縮資料；仍檢查掃描標頭，不能只看到標記就放行。
            let components = *bytes.get(offset + 2).ok_or("JPEG 掃描標頭不完整。")? as usize;
            if !matches!(components, 1..=4)
                || length != 6 + 2 * components
                || end + 2 >= bytes.len()
            {
                return Err("JPEG 掃描資料不完整。".into());
            }
            if let Some((w, h)) = dimensions {
                return Ok(("image/jpeg", w, h));
            }
            break;
        }
        if matches!(marker, 0xc0..=0xc2) {
            if length < 8 || bytes[offset + 2] != 8 || dimensions.is_some() {
                return Err("JPEG 尺寸標頭不支援。".into());
            }
            let h = u16::from_be_bytes([bytes[offset + 3], bytes[offset + 4]]) as u32;
            let w = u16::from_be_bytes([bytes[offset + 5], bytes[offset + 6]]) as u32;
            let components = bytes[offset + 7] as usize;
            if !matches!(components, 1 | 3 | 4)
                || length != 8 + 3 * components
                || w == 0
                || h == 0
                || w > 8192
                || h > 8192
                || u64::from(w) * u64::from(h) > 16_000_000
            {
                return Err("圖片限 8192×8192 以內、總計 1600 萬像素。".into());
            }
            dimensions = Some((w, h));
        }
        offset = end;
    }
    Err("JPEG 缺少可支援的尺寸或影像掃描資料。".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_containers_and_limits_are_checked() {
        assert_eq!(
            inspect(include_bytes!("../../../examples/fixtures/vision.png")).unwrap(),
            ("image/png", 96, 64)
        );
        assert_eq!(
            inspect(include_bytes!("../../../examples/fixtures/vision.jpg")).unwrap(),
            ("image/jpeg", 96, 64)
        );
        assert!(inspect(b"<svg>not a bitmap</svg>").is_err());
        assert!(inspect(&vec![0; MAX_BYTES + 1]).is_err());
        let mut jpeg = include_bytes!("../../../examples/fixtures/vision.jpg").to_vec();
        jpeg.truncate(jpeg.len() - 2);
        assert!(inspect(&jpeg).is_err());
    }
}
