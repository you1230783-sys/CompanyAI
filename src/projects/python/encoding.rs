//! Python 文字快照的嚴格解碼。原始 bytes 的版本碼不因轉碼改變。
use crate::{projects::text, AppResult};

pub(crate) struct Decoded {
    pub text: String,
    pub name: &'static str,
    pub ambiguous: bool,
}

fn utf8(bytes: &[u8]) -> AppResult<String> {
    let value = std::str::from_utf8(bytes).map_err(|_| "不是有效 UTF-8。")?;
    text::validate_content(value)?;
    Ok(value.into())
}

fn big5(bytes: &[u8]) -> AppResult<String> {
    let value = text::decode_cp(bytes, 950)?;
    text::validate_content(&value)?;
    if text::encode_cp(&value, 950)? != bytes {
        return Err("Big5 無法無損往返。".into());
    }
    Ok(value)
}

/// LOG／OUT／ERR 無 BOM 時先試 Windows Big5（CP950），再試 UTF-8。
/// UTF-8 BOM 為明確宣告，不可因內容損壞退回 Big5；其他文字預設仍為 UTF-8。
/// 兩種編碼都有效但文字不同時，保留 Big5 優先並在 metadata 標記歧義；
/// 模型需核對樣本，必要時以 encoding=utf8 重讀，不能忽略或替換無效字元。
pub(crate) fn decode(bytes: &[u8], log: bool, requested: Option<&str>) -> AppResult<Decoded> {
    let mode = requested.unwrap_or("auto");
    if !matches!(mode, "auto" | "big5" | "utf8") {
        return Err("encoding 必須為 auto、big5 或 utf8。".into());
    }
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        return Err(
            "來源宣告 UTF-16，請使用既有 LOG 工具或匯入正確文字；不可當 Big5／UTF-8 解讀。".into(),
        );
    }
    let (value, name, ambiguous) = if let Some(content) = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]) {
        if mode == "big5" {
            return Err("UTF-8 BOM 與指定 Big5 不符；請使用 auto 或 utf8。".into());
        }
        (
            utf8(content).map_err(|_| "UTF-8 BOM 與內容不一致。")?,
            "utf8",
            false,
        )
    } else if mode == "big5" {
        (big5(bytes)?, "big5", false)
    } else if mode == "utf8" || !log {
        (utf8(bytes)?, "utf8", false)
    } else {
        match big5(bytes) {
            Ok(value) => {
                let ambiguous = utf8(bytes).is_ok_and(|other| other != value);
                (value, "big5", ambiguous)
            }
            Err(_) => (
                utf8(bytes).map_err(|_| {
                    "Big5 與 UTF-8 均無法可靠解碼；請核對原檔編碼或匯入已解密文字。"
                })?,
                "utf8",
                false,
            ),
        }
    };
    Ok(Decoded {
        text: value,
        name,
        ambiguous,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_encoding_order_bom_fallback_and_explicit_choice() {
        let original = "2026/10/01 拋料當顆辨識序號: 123\r\n".repeat(10000);
        let bytes = text::encode_cp(&original, 950).unwrap();
        let result = decode(&bytes, true, None).unwrap();
        assert_eq!(result.text, original);
        assert_eq!(result.name, "big5");
        assert!(!result.ambiguous);
        assert!(decode(&bytes, false, None).is_err());
        assert_eq!(decode(&bytes, false, Some("big5")).unwrap().text, original);
        // 三 bytes 的單一中文字無法組成有效 Big5，應退回 UTF-8。
        assert_eq!(decode("中".as_bytes(), true, None).unwrap().name, "utf8");
        let bom = [b"\xef\xbb\xbf".as_slice(), "中文".as_bytes()].concat();
        assert_eq!(decode(&bom, true, None).unwrap().text, "中文");
        assert!(decode(&bom, true, Some("big5")).is_err());
        let broken_bom = [b"\xef\xbb\xbf".as_slice(), bytes.as_slice()].concat();
        assert!(decode(&broken_bom, true, None).is_err());
        // C2 A1 同時為 UTF-8 的 ¡ 與合法 CP950；預設照使用者指定的 Big5 優先。
        let ambiguous = decode(b"\xc2\xa1", true, None).unwrap();
        assert_eq!(ambiguous.name, "big5");
        assert!(ambiguous.ambiguous);
        assert_eq!(decode(b"\xc2\xa1", true, Some("utf8")).unwrap().text, "¡");
    }

    #[test]
    fn invalid_text_is_never_silently_replaced_or_treated_as_binary() {
        for bytes in [
            b"\x81".as_slice(),
            b"\0text",
            b"\xff\xfeA\0",
            b"\xfe\xff\0A",
            "�".as_bytes(),
        ] {
            assert!(decode(bytes, true, Some("utf8")).is_err());
        }
        assert!(decode(b"\0text", true, None).is_err());
        assert!(decode(b"\x81", true, None).is_err());
        assert!(decode(b"text", true, Some("ignored")).is_err());
        assert!(!decode(b"ASCII 123\r\n", true, None).unwrap().ambiguous);
    }
}
