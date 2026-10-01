//! PNG 僅由內嵌 ECharts 產生；模型只指定既有圖表編號及檔名，不能提交影像或腳本。
use super::Chart;
use crate::AppResult;
use std::sync::atomic::AtomicBool;
use windows_sys::Win32::Security::Cryptography::{
    CryptStringToBinaryA, CRYPT_STRING_BASE64, CRYPT_STRING_STRICT,
};

pub const WIDTH: u32 = 1600;
pub const HEIGHT: u32 = 1000;
pub const MAX_BYTES: usize = 8 * 1024 * 1024;
pub type Renderer = Box<dyn FnMut(&Chart, &AtomicBool) -> AppResult<Vec<u8>>>;

/// 回應從 WebView2 的原生 ExecuteScript callback 取得，不經模型或一般訊息命令。
pub fn decode_url(url: &str) -> AppResult<Vec<u8>> {
    let encoded = url
        .strip_prefix("data:image/png;base64,")
        .ok_or("圖表未回傳 PNG。")?;
    if encoded.is_empty()
        || encoded.len() > MAX_BYTES.div_ceil(3) * 4
        || !encoded
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b))
    {
        return Err("圖表 PNG 編碼或大小不合法。".into());
    }
    let mut bytes = vec![0; MAX_BYTES];
    let mut size = bytes.len() as u32;
    // SAFETY: 輸入為有界 ASCII，輸出配置為 size bytes，Windows 不超過此容量。
    if unsafe {
        CryptStringToBinaryA(
            encoded.as_ptr(),
            encoded.len() as u32,
            CRYPT_STRING_BASE64 | CRYPT_STRING_STRICT,
            bytes.as_mut_ptr(),
            &mut size,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err("圖表 PNG 解碼失敗。".into());
    }
    bytes.truncate(size as usize);
    validate(&bytes)?;
    Ok(bytes)
}

/// 檢查容器邊界、固定尺寸及 CRC；不信任副檔名，也不把截斷影像當成成果。
pub fn validate(bytes: &[u8]) -> AppResult<()> {
    if bytes.len() > MAX_BYTES || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("圖表 PNG 標頭或大小不合法。".into());
    }
    let mut offset = 8;
    let mut has_header = false;
    let mut has_data = false;
    while offset + 12 <= bytes.len() {
        let size = u32::from_be_bytes(
            bytes[offset..offset + 4]
                .try_into()
                .map_err(|_| "PNG 長度錯誤。")?,
        ) as usize;
        let end = offset
            .checked_add(size)
            .and_then(|n| n.checked_add(12))
            .filter(|n| *n <= bytes.len())
            .ok_or("PNG 區塊截斷。")?;
        let kind = &bytes[offset + 4..offset + 8];
        let body = &bytes[offset + 8..end - 4];
        let expected = u32::from_be_bytes(
            bytes[end - 4..end]
                .try_into()
                .map_err(|_| "PNG CRC 錯誤。")?,
        );
        if crc32(&bytes[offset + 4..end - 4]) != expected {
            return Err("PNG 校驗失敗。".into());
        }
        if !has_header {
            if kind != b"IHDR"
                || size != 13
                || body[..4] != WIDTH.to_be_bytes()
                || body[4..8] != HEIGHT.to_be_bytes()
                || body[8] != 8
                || ![2, 6].contains(&body[9])
                || body[10..] != [0, 0, 0]
            {
                return Err("PNG 尺寸或像素格式不符。".into());
            }
            has_header = true;
        } else if kind == b"IHDR" {
            return Err("PNG 重複標頭。".into());
        }
        if kind == b"IDAT" && size > 0 {
            has_data = true;
        }
        if kind == b"IEND" {
            return if size == 0 && has_data && end == bytes.len() {
                Ok(())
            } else {
                Err("PNG 結尾或影像資料不完整。".into())
            };
        }
        offset = end;
    }
    Err("PNG 缺少完整結尾。".into())
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

#[cfg(test)]
pub(crate) fn fixture() -> Vec<u8> {
    include_bytes!("test-export.png").to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn valid_png_requires_dimensions_crc_and_complete_end() {
        let bytes = fixture();
        validate(&bytes).unwrap();
        assert!(validate(&bytes[..bytes.len() - 1]).is_err());
        let mut corrupt = bytes.clone();
        corrupt[50] ^= 1;
        assert!(validate(&corrupt).is_err());
        let mut wrong_size = bytes.clone();
        wrong_size[16..20].copy_from_slice(&1u32.to_be_bytes());
        let crc = crc32(&wrong_size[12..29]);
        wrong_size[29..33].copy_from_slice(&crc.to_be_bytes());
        assert!(validate(&wrong_size).is_err());
        assert!(decode_url("data:image/svg+xml;base64,AAAA").is_err());
        assert!(decode_url("data:image/png;base64,!!!!").is_err());
    }
}
