//! PDF 僅在 AppContainer 解析記憶體資料，不存取外部字型／附件，不執行 JavaScript。
use crate::AppResult;
use std::io::{self, Write};
pub const MAX_PDF: usize = 20_000_000;

/// 限制擷取輸出，避免小型 PDF 展開成無界文字；程序本身另有記憶體及時間限制。
struct LimitedText(Vec<u8>);
impl Write for LimitedText {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.0.len().saturating_add(bytes.len()) > super::text::MAX_TEXT {
            return Err(io::Error::other("PDF 文字超過 200 KB"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub fn extract(bytes: &[u8]) -> AppResult<String> {
    if bytes.len() > MAX_PDF {
        return Err("PDF 上限為 20 MB。".into());
    }
    let doc = pdf_extract::Document::load_mem(bytes).map_err(|error| {
        format!("本程式無法解析讀到的 PDF 資料（{error}）。即使 Adobe 可以開啟，也不代表本程式取得的是解密後內容；這個錯誤不能直接判定檔案損壞。請用 ask_user 請使用者從 Adobe 複製所需文字，再透過專案「匯入文字」提供內容。")
    })?;
    if doc.is_encrypted() {
        return Err("此 PDF 需要解密，請以核准的閱讀器開啟後匯入文字。".into());
    }
    let pages = doc.get_pages();
    if pages.is_empty() || pages.len() > 200 {
        return Err("PDF 須為 1 至 200 頁。".into());
    }
    let mut result = String::from("PDF 文字擷取（不含圖片／OCR，表格及閱讀順序可能需核對）\n");
    let mut has_text = false;
    for page in pages.keys() {
        let mut buffer = LimitedText(Vec::new());
        let mut output = pdf_extract::PlainTextOutput::new(&mut buffer as &mut dyn Write);
        pdf_extract::output_doc_page(&doc, &mut output, *page)
            .map_err(|_| "PDF 文字擷取失敗或超過 200 KB，請匯入所需段落。")?;
        let text = String::from_utf8(buffer.0).map_err(|_| "PDF 文字無法解碼。")?;
        let text = text.trim();
        result.push_str(&format!("\n[第 {page} 頁]\n"));
        if text.is_empty() {
            result.push_str("（此頁未擷取到文字）\n");
        } else {
            has_text = true;
            result.push_str(text);
            result.push('\n');
        }
        super::text::validate(&result)?;
    }
    if !has_text {
        return Err("此 PDF 未擷取到文字，可能是掃描文件；本版不支援 OCR。".into());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_pdf_and_limits_text() {
        assert!(extract(b"not a pdf").is_err());
        let mut text = LimitedText(Vec::new());
        assert!(text
            .write_all(&vec![b'x'; super::super::text::MAX_TEXT + 1])
            .is_err());
    }
}
