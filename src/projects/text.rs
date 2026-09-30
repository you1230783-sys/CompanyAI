//! 文字解碼不等於解密。嚴格拒絕替代字元與二進位控制碼，歧義交回使用者。
use crate::AppResult;
use sha2::{Digest, Sha256};
use windows_sys::Win32::Globalization::*;

pub const MAX_TEXT: usize = 200_000;

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub enum Encoding {
    Utf8(bool),
    Utf16(bool),
    CodePage(u32),
}

pub fn revision(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

pub fn validate(text: &str) -> AppResult<()> {
    if text.len() > MAX_TEXT
        || text
            .chars()
            .any(|c| c == '\u{fffd}' || (c.is_control() && !matches!(c, '\n' | '\r' | '\t')))
    {
        return Err("文字過大或包含無法辨識的內容。請先用公司核准的記事本開啟，確認解密及編碼後再匯入文字。".into());
    }
    Ok(())
}

pub fn decode(bytes: &[u8]) -> AppResult<(String, Encoding)> {
    if bytes.len() > MAX_TEXT {
        return Err("第一版文件上限為 200 KB，請分成較小文件。".into());
    }
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        let little = bytes[0] == 0xff;
        if !bytes.len().is_multiple_of(2) {
            return Err("UTF-16 長度不正確。".into());
        }
        let words: Vec<u16> = bytes[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| {
                if little {
                    u16::from_le_bytes([b[0], b[1]])
                } else {
                    u16::from_be_bytes([b[0], b[1]])
                }
            })
            .collect();
        let text = String::from_utf16(&words).map_err(|_| "UTF-16 內容無法辨識。")?;
        validate(&text)?;
        return Ok((text, Encoding::Utf16(little)));
    }
    let bom = bytes.starts_with(&[0xef, 0xbb, 0xbf]);
    if let Ok(text) = std::str::from_utf8(if bom { &bytes[3..] } else { bytes }) {
        validate(text)?;
        return Ok((text.into(), Encoding::Utf8(bom)));
    }
    if bom {
        return Err("UTF-8 BOM 與內容不一致，請確認解密狀態。".into());
    }
    // 不以「寬鬆 ANSI 總能轉換」當成功；必須嚴格解碼及無損往返。
    let ansi = unsafe { GetACP() };
    let mut candidate: Option<(String, Encoding)> = None;
    for codepage in [950, ansi] {
        if codepage == 65001 {
            continue;
        }
        if let Ok(text) = decode_cp(bytes, codepage) {
            if validate(&text).is_err() {
                continue;
            }
            if encode(&text, Encoding::CodePage(codepage)).ok().as_deref() != Some(bytes) {
                continue;
            }
            if candidate
                .as_ref()
                .is_some_and(|(previous, _)| previous != &text)
            {
                return Err("BIG5 與系統 ANSI 結果不同，請以記事本確認後匯入正確文字。".into());
            }
            candidate = Some((text, Encoding::CodePage(codepage)));
        }
    }
    candidate.ok_or_else(|| "無法取得可信文字，可能仍為公司密文。請用記事本開啟後使用專案「匯入文字」，不會嘗試破壞或轉換原檔。".into())
}

fn decode_cp(bytes: &[u8], codepage: u32) -> AppResult<String> {
    if bytes.is_empty() {
        return Ok(String::new());
    }
    let mut words = vec![0u16; bytes.len()];
    let count = unsafe {
        MultiByteToWideChar(
            codepage,
            MB_ERR_INVALID_CHARS,
            bytes.as_ptr(),
            bytes.len() as i32,
            words.as_mut_ptr(),
            words.len() as i32,
        )
    };
    if count <= 0 {
        return Err("此編碼無法解讀內容。".into());
    }
    String::from_utf16(&words[..count as usize]).map_err(|_| "編碼轉換失敗。".into())
}

pub fn encode(text: &str, encoding: Encoding) -> AppResult<Vec<u8>> {
    validate(text)?;
    match encoding {
        Encoding::Utf8(bom) => {
            let mut result = if bom {
                vec![0xef, 0xbb, 0xbf]
            } else {
                Vec::new()
            };
            result.extend_from_slice(text.as_bytes());
            Ok(result)
        }
        Encoding::Utf16(little) => {
            let mut result = if little {
                vec![0xff, 0xfe]
            } else {
                vec![0xfe, 0xff]
            };
            for word in text.encode_utf16() {
                result.extend_from_slice(&if little {
                    word.to_le_bytes()
                } else {
                    word.to_be_bytes()
                });
            }
            Ok(result)
        }
        Encoding::CodePage(codepage) => {
            if text.is_empty() {
                return Ok(Vec::new());
            }
            let words: Vec<u16> = text.encode_utf16().collect();
            let mut result = vec![0u8; words.len() * 4];
            let mut substituted = 0;
            let count = unsafe {
                WideCharToMultiByte(
                    codepage,
                    WC_NO_BEST_FIT_CHARS,
                    words.as_ptr(),
                    words.len() as i32,
                    result.as_mut_ptr(),
                    result.len() as i32,
                    std::ptr::null(),
                    &mut substituted,
                )
            };
            if count <= 0 || substituted != 0 {
                return Err(
                    "新文字無法以原始編碼完整保存，請確認是否改以 UTF-8 建立新 TXT；未以問號替換。"
                        .into(),
                );
            }
            result.truncate(count as usize);
            Ok(result)
        }
    }
}

/// 以 Unicode 字元索引套用修改；版本和預期原文兩者都必須吻合。
pub fn edit(
    content: &str,
    version: &str,
    start: usize,
    expected: &str,
    replacement: &str,
) -> AppResult<String> {
    if revision(content) != version {
        return Err("文件版本已改變，請重新讀取。".into());
    }
    let begin = content
        .char_indices()
        .map(|(i, _)| i)
        .chain(std::iter::once(content.len()))
        .nth(start)
        .ok_or("修改位置超過文件範圍。")?;
    if !content[begin..].starts_with(expected) {
        return Err("目標位置的原文不符，未套用修改。".into());
    }
    let result = format!(
        "{}{}{}",
        &content[..begin],
        replacement,
        &content[begin + expected.len()..]
    );
    validate(&result)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encodings_preserve_chinese_and_reject_loss() {
        for enc in [
            Encoding::Utf8(true),
            Encoding::Utf16(true),
            Encoding::Utf16(false),
            Encoding::CodePage(950),
        ] {
            assert_eq!(
                decode(&encode("測試\r\n", enc).unwrap()).unwrap().0,
                "測試\r\n"
            );
        }
        assert!(encode("測試😀", Encoding::CodePage(950)).is_err());
        assert!(decode(&[0, 1, 2, 3]).is_err());
    }
    #[test]
    fn edits_check_revision_and_unicode_position() {
        let original = "甲😀乙";
        assert_eq!(
            edit(original, &revision(original), 1, "😀", "中文字").unwrap(),
            "甲中文字乙"
        );
        assert!(edit(original, "old", 1, "😀", "").is_err());
        assert!(edit(original, &revision(original), 1, "甲", "").is_err());
    }
}
