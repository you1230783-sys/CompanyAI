//! PNG預覽編碼供截圖及桌寵共用，不依賴桌寵功能。
use crate::AppResult;

/// 保留既有PNG data URL契約；截圖呼叫端仍自行標示JPEG。
pub fn data_url(bytes: &[u8]) -> AppResult<String> {
    use windows_sys::Win32::Security::Cryptography::*;
    let mut encoded = vec![0u8; bytes.len().div_ceil(3) * 4 + 1];
    let mut size = encoded.len() as u32;
    if unsafe {
        CryptBinaryToStringA(
            bytes.as_ptr(),
            bytes.len() as u32,
            CRYPT_STRING_BASE64 | CRYPT_STRING_NOCRLF,
            encoded.as_mut_ptr(),
            &mut size,
        )
    } == 0
    {
        return Err("PNG預覽編碼失敗。".into());
    }
    Ok(format!(
        "data:image/png;base64,{}",
        std::str::from_utf8(&encoded[..size as usize]).map_err(|e| e.to_string())?
    ))
}
