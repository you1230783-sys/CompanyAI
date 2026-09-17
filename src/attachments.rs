//! 附件規則由網站提供；檔案先分塊加密暫存，再由背景執行緒上傳原始位元組。
//! 不接受前端提供磁碟路徑；前端只能交付使用者選檔／貼上的 File 資料。
use crate::{jobs, storage, AppResult};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};
use windows_sys::Win32::Security::Cryptography::*;

pub const CHUNK_BYTES: usize = 192 * 1024;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct AttachmentRules {
    pub enabled: bool,
    pub max_count: usize,
    pub max_file_bytes: u64,
    pub max_total_bytes: u64,
    pub allowed_extensions: Vec<String>,
    #[serde(default)]
    pub allowed_mime_types: Vec<String>,
}
impl AttachmentRules {
    pub fn validate(&self) -> AppResult<()> {
        if self.enabled
            && (self.max_count == 0
                || self.max_file_bytes == 0
                || self.max_total_bytes < self.max_file_bytes
                || self.allowed_extensions.is_empty()
                || self.allowed_extensions.len() > 200
                || self.allowed_extensions.iter().any(|e| {
                    e.len() > 30
                        || !e.starts_with('.')
                        || !e[1..].bytes().all(|c| c.is_ascii_alphanumeric())
                })
                || self.allowed_mime_types.len() > 200
                || self
                    .allowed_mime_types
                    .iter()
                    .any(|m| m.len() > 100 || m.chars().any(char::is_control)))
        {
            return Err("附件能力規則格式不正確。".into());
        }
        Ok(())
    }
    /// 20 個是產品上限；大小由網站決定，單一 HTTP 上傳最多支援 u32 長度。
    pub fn check(&self, name: &str, size: u64, count: usize, total: u64) -> AppResult<()> {
        self.validate()?;
        if !self.enabled {
            return Err("網站尚未啟用附件。".into());
        }
        if name.is_empty()
            || name.len() > 512
            || name.chars().any(|c| c.is_control() || "\\/:".contains(c))
        {
            return Err("附件檔名不正確。".into());
        }
        if count >= self.max_count.min(20) {
            return Err("每次訊息的文件與圖片已達附件數量上限（最多 20 個）。".into());
        }
        if size == 0
            || size > self.max_file_bytes
            || size > u32::MAX as u64
            || total.saturating_add(size) > self.max_total_bytes
        {
            return Err("附件大小超過限制或檔案為空，請查看網站提供的附件規則。".into());
        }
        let extension = name
            .rsplit_once('.')
            .map(|(_, e)| format!(".{}", e.to_ascii_lowercase()))
            .unwrap_or_default();
        if !self
            .allowed_extensions
            .iter()
            .any(|e| e.eq_ignore_ascii_case(&extension))
        {
            return Err(format!("網站目前不接受 {extension} 檔案。"));
        }
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub id: String,
    pub conversation_id: String,
    pub name: String,
    pub size: u64,
    pub mime_type: String,
    pub remote: Option<AttachmentStatus>,
    pub state: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub sent: bool,
    #[serde(default)]
    pub removed: bool,
    #[serde(default)]
    pub uploaded_bytes: u64,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct AttachmentStatus {
    pub job_id: String,
    pub state: String,
    #[serde(default)]
    pub progress: Option<f64>,
    #[serde(default)]
    pub queue_position: Option<u32>,
    #[serde(default)]
    pub attachment_token: Option<String>,
    #[serde(default)]
    pub expires_at: Option<u64>,
    #[serde(default)]
    pub error_message: String,
    #[serde(default)]
    pub timing: jobs::Timing,
}
impl AttachmentStatus {
    pub fn validate(&self) -> AppResult<()> {
        jobs::validate_id(&self.job_id)?;
        if !matches!(
            self.state.as_str(),
            "awaiting_upload"
                | "uploaded"
                | "queued"
                | "processing"
                | "ready"
                | "failed"
                | "cancelled"
        ) || self
            .progress
            .is_some_and(|p| !p.is_finite() || !(0.0..=100.0).contains(&p))
            || self.error_message.len() > 2000
            || self
                .attachment_token
                .as_ref()
                .is_some_and(|t| t.is_empty() || t.len() > 4096 || t.chars().any(char::is_control))
            || (self.state == "ready"
                && (self.attachment_token.is_none() || self.expires_at.is_none()))
        {
            return Err("附件狀態回應不正確。".into());
        }
        self.timing.validate()
    }
}
impl Attachment {
    pub fn token(&self) -> Option<&str> {
        let remote = self.remote.as_ref()?;
        if self.state != "ready" || remote.expires_at? <= crate::unix_now() {
            return None;
        }
        remote.attachment_token.as_deref()
    }
    pub fn apply(&mut self, status: AttachmentStatus) -> AppResult<()> {
        status.validate()?;
        if self
            .remote
            .as_ref()
            .is_some_and(|r| r.job_id != status.job_id)
        {
            return Err("附件工作識別碼不一致。".into());
        }
        self.state = status.state.clone();
        self.message = status.error_message.clone();
        self.remote = Some(status);
        Ok(())
    }
}

/// 輸入中的檔案一次只保留一小塊明文；磁碟上的每塊各自使用目前使用者 DPAPI 加密。
pub struct Incoming {
    pub id: String,
    pub expected: u64,
    pub received: u64,
    path: PathBuf,
    file: File,
    complete: bool,
}
pub fn spool_path(root: &Path, id: &str) -> AppResult<PathBuf> {
    jobs::validate_id(id)?;
    Ok(root.join("uploads").join(format!("{id}.dpapi")))
}
impl Incoming {
    pub fn new(root: &Path, id: String, expected: u64) -> AppResult<Self> {
        let path = spool_path(root, &id)?;
        fs::create_dir_all(path.parent().ok_or("附件暫存路徑錯誤。")?)
            .map_err(|_| "無法建立附件暫存目錄。")?;
        let file = File::options()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|_| "無法建立附件暫存檔。")?;
        Ok(Self {
            id,
            expected,
            received: 0,
            path,
            file,
            complete: false,
        })
    }
    pub fn append(&mut self, offset: u64, encoded: &str) -> AppResult<()> {
        if offset != self.received {
            return Err("附件區塊順序不正確。".into());
        }
        let bytes = decode_chunk(encoded)?;
        self.append_bytes(&bytes)
    }
    /// 原生 Outlook 匯出走同樣的有界加密區塊，不經 WebView 傳本機路徑。
    pub fn append_bytes(&mut self, bytes: &[u8]) -> AppResult<()> {
        if bytes.is_empty()
            || bytes.len() > CHUNK_BYTES
            || self.received + bytes.len() as u64 > self.expected
        {
            return Err("附件區塊大小不正確。".into());
        }
        let encrypted = storage::protect(bytes, true)?;
        self.file
            .write_all(&(encrypted.len() as u32).to_le_bytes())
            .and_then(|()| self.file.write_all(&encrypted))
            .map_err(|_| "附件暫存寫入失敗。")?;
        self.received += bytes.len() as u64;
        Ok(())
    }
    pub fn finish(mut self) -> AppResult<()> {
        if self.received != self.expected {
            return Err("附件尚未接收完整。".into());
        }
        self.file.sync_all().map_err(|_| "無法保存附件暫存檔。")?;
        self.complete = true;
        Ok(())
    }
}
impl Drop for Incoming {
    fn drop(&mut self) {
        if !self.complete {
            let _ = fs::remove_file(&self.path);
        }
    }
}
fn decode_chunk(encoded: &str) -> AppResult<Vec<u8>> {
    if encoded.is_empty()
        || encoded.len() > CHUNK_BYTES * 4 / 3
        || !encoded
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b))
    {
        return Err("附件區塊編碼不正確。".into());
    }
    let mut bytes = vec![0; CHUNK_BYTES];
    let mut length = bytes.len() as u32;
    // SAFETY: ASCII Base64 有界，Windows 僅能寫入 length 指定的緩衝區。
    if unsafe {
        CryptStringToBinaryA(
            encoded.as_ptr(),
            encoded.len() as u32,
            CRYPT_STRING_BASE64 | CRYPT_STRING_STRICT,
            bytes.as_mut_ptr(),
            &mut length,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err("附件區塊無法解碼。".into());
    }
    bytes.truncate(length as usize);
    Ok(bytes)
}
pub struct SpoolReader {
    file: File,
    chunk: io::Cursor<Vec<u8>>,
}
impl SpoolReader {
    pub fn open(root: &Path, id: &str) -> AppResult<Self> {
        Ok(Self {
            file: File::open(spool_path(root, id)?)
                .map_err(|_| "找不到附件暫存檔，請重新選取。")?,
            chunk: io::Cursor::new(Vec::new()),
        })
    }
}
impl Read for SpoolReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = self.chunk.read(buffer)?;
        if count > 0 || buffer.is_empty() {
            return Ok(count);
        }
        let mut length = [0u8; 4];
        let first = self.file.read(&mut length[..1])?;
        if first == 0 {
            return Ok(0);
        }
        self.file.read_exact(&mut length[1..])?;
        let length = u32::from_le_bytes(length) as usize;
        if length > CHUNK_BYTES + 4096 {
            return Err(io::Error::other("附件區塊過大"));
        }
        let mut bytes = vec![0; length];
        self.file.read_exact(&mut bytes)?;
        let decoded = storage::protect(&bytes, false).map_err(io::Error::other)?;
        if decoded.len() > CHUNK_BYTES {
            return Err(io::Error::other("附件解密區塊過大"));
        }
        self.chunk = io::Cursor::new(decoded);
        self.chunk.read(buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backend_rules_and_combined_count_are_enforced() {
        let rules = AttachmentRules {
            enabled: true,
            max_count: 99,
            max_file_bytes: 100,
            max_total_bytes: 200,
            allowed_extensions: vec![".pdf".into(), ".png".into()],
            ..Default::default()
        };
        assert!(rules.check("文件.PDF", 100, 19, 100).is_ok());
        assert!(rules.check("圖.png", 1, 20, 0).is_err());
        assert!(rules.check("檔.exe", 1, 0, 0).is_err());
        assert!(rules.check("../檔.pdf", 1, 0, 0).is_err());
        assert!(rules.check("檔.pdf", 101, 0, 0).is_err());
        assert!(rules.check("檔.pdf", 100, 1, 101).is_err());
    }
    #[test]
    fn encrypted_chunks_are_bounded_ordered_and_streamable() {
        let id = jobs::new_id().unwrap();
        let root = std::env::temp_dir().join(format!("lm-upload-{id}"));
        let mut incoming = Incoming::new(&root, id.clone(), 6).unwrap();
        assert!(incoming.append(1, "YWJj").is_err());
        incoming.append(0, "YWJj").unwrap();
        incoming.append(3, "ZGVm").unwrap();
        incoming.finish().unwrap();
        let mut read = Vec::new();
        SpoolReader::open(&root, &id)
            .unwrap()
            .read_to_end(&mut read)
            .unwrap();
        assert_eq!(read, b"abcdef");
        assert!(decode_chunk("not valid!!!").is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
