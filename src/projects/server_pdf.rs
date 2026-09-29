//! 每次專案任務獨立的 PDF 轉換服務。Token 不傳模型，Markdown 不寫入明文快取。
use crate::{config::Config, storage::Session, transport::pdf, AppResult};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Seek},
    sync::atomic::{AtomicBool, Ordering},
};

pub(super) struct Reader {
    config: Config,
    session: Session,
    // 內容雜湊避免同一份 PDF 的分頁讀取、搜尋、建立 TXT 副本反覆上傳。
    // 任務結束即釋放；不跨帳號、任務或程序共用。
    cache: BTreeMap<String, String>,
}
impl Reader {
    pub fn new(config: Config, session: Session) -> AppResult<Self> {
        config.pdf_endpoint()?;
        if !session.valid_for(&config) {
            return Err("登入已到期，請重新登入後轉換 PDF。".into());
        }
        Ok(Self {
            config,
            session,
            cache: BTreeMap::new(),
        })
    }

    /// file 由 broker 開啟並保持唯讀鎖；雜湊、上傳、取得結果全程固定相同來源。
    pub fn read(&mut self, file: &mut File, name: &str, cancel: &AtomicBool) -> AppResult<String> {
        if !self.session.valid_for(&self.config) {
            return Err("登入已到期，請重新登入後轉換 PDF。".into());
        }
        let length = file.metadata().map_err(|e| e.to_string())?.len();
        if length == 0 || length > pdf::MAX_FILE {
            return Err("PDF 須為 1 byte 至 50 MiB（52,428,800 bytes）。".into());
        }
        file.rewind().map_err(|e| e.to_string())?;
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 65_536];
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err("PDF 讀取已取消。".into());
            }
            let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
        let revision = format!("{:x}", hash.finalize());
        if let Some(markdown) = self.cache.get(&revision) {
            return Ok(markdown.clone());
        }
        file.rewind().map_err(|e| e.to_string())?;
        let response = pdf::convert(
            &self.config.pdf_endpoint()?,
            &self.session.access_token,
            name,
            file,
            length,
            cancel,
        )?;
        let markdown = decode(response, &self.session.access_token)?;
        if cancel.load(Ordering::Relaxed) {
            return Err("PDF 讀取已取消。".into());
        }
        // 上限 20 份／約 4 MB，超過時清除舊快取；不影響已建立的工作副本。
        if self.cache.len() >= 20 {
            self.cache.clear();
        }
        self.cache.insert(revision, markdown.clone());
        Ok(markdown)
    }
}

fn concise(value: &str, token: &str) -> String {
    value
        .replace(token, "[已隱藏]")
        .chars()
        .filter(|c| !c.is_control())
        .take(1000)
        .collect()
}

fn decode(response: pdf::Response, token: &str) -> AppResult<String> {
    if response.status != 200 {
        let value: serde_json::Value = serde_json::from_slice(&response.body).unwrap_or_default();
        let code = concise(
            value["error_code"]
                .as_str()
                .unwrap_or("PDF_CONVERSION_FAILED"),
            token,
        );
        let message = concise(
            value["message"]
                .as_str()
                .unwrap_or("伺服器未提供有效錯誤說明。"),
            token,
        );
        let id = concise(
            value["request_id"].as_str().unwrap_or(&response.request_id),
            token,
        );
        return Err(format!(
            "PDF 轉換失敗（HTTP {}，{code}）：{message}{}；未自動重送。",
            response.status,
            if id.is_empty() {
                String::new()
            } else {
                format!("；request_id={id}")
            }
        ));
    }
    let media_type = response.content_type.split(';').next().unwrap_or("").trim();
    if !media_type.eq_ignore_ascii_case("text/markdown") {
        return Err(
            "PDF 路由應回傳 text/markdown，卻收到其他格式；未將 HTML／JSON 當作文件。".into(),
        );
    }
    let bytes = response
        .body
        .strip_prefix(&[0xef, 0xbb, 0xbf])
        .unwrap_or(&response.body);
    let markdown = std::str::from_utf8(bytes).map_err(|_| "PDF 轉換文字不是有效 UTF-8。")?;
    if markdown.trim().is_empty() {
        return Err("PDF 轉換未取得文字，請確認文件內容。".into());
    }
    super::text::validate(markdown)
        .map_err(|_| "PDF Markdown 超過 200 KB 或含無法辨識的文字，請拆分文件或匯入所需段落。")?;
    Ok(markdown.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn markdown_and_contract_errors_are_not_confused_with_json() {
        let reply = |status, media: &str, body: &[u8]| pdf::Response {
            status,
            content_type: media.into(),
            body: body.into(),
            request_id: "header-id".into(),
        };
        assert_eq!(
            decode(
                reply(
                    200,
                    "text/markdown; charset=utf-8",
                    "\u{feff}# 中文\n正文".as_bytes()
                ),
                "test-token"
            )
            .unwrap(),
            "# 中文\n正文"
        );
        for (media, body) in [
            ("text/html", &b"<html>login</html>"[..]),
            ("application/json", &b"{}"[..]),
            ("text/markdown", &b"\xff"[..]),
            ("text/markdown", &b"  "[..]),
        ] {
            assert!(decode(reply(200, media, body), "test-token").is_err());
        }
        let error = decode(reply(504,"application/json", br#"{"error_code":"SERVER_C_TIMEOUT","message":"test-token timeout","request_id":"r1"}"#),"test-token").unwrap_err();
        assert!(
            error.contains("SERVER_C_TIMEOUT")
                && error.contains("r1")
                && !error.contains("test-token")
        );
    }
}
