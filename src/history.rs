//! 以結構化 JSON 保存對話，整份使用 DPAPI 加密，不保存 Token 或模型憑證。
//! 小型桌面歷史不需要資料庫服務；schema 欄位保留日後遷移的入口。
use crate::{protocol::Message, storage, unix_now, AppResult};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use windows_sys::Win32::Security::Cryptography::{
    BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
};

const MAX_ARCHIVE_BYTES: usize = 32 * 1024 * 1024;
const MAX_CONVERSATIONS: usize = 200;

#[derive(Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub title_manual: bool,
    #[serde(default)]
    pub draft: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub messages: Vec<Message>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Archive {
    pub schema: u32,
    pub conversations: Vec<Conversation>,
}
impl Default for Archive {
    fn default() -> Self {
        Self {
            schema: 1,
            conversations: Vec::new(),
        }
    }
}
impl Archive {
    /// 新對話直到有訊息才加入；切換空白頁不會產生一堆空紀錄。
    pub fn insert(&mut self, messages: Vec<Message>) -> AppResult<String> {
        if self.conversations.len() >= MAX_CONVERSATIONS {
            return Err("已保存 200 個對話，請先刪除不需要的紀錄。".into());
        }
        let mut bytes = [0u8; 16];
        if unsafe {
            BCryptGenRandom(
                std::ptr::null_mut(),
                bytes.as_mut_ptr(),
                bytes.len() as u32,
                BCRYPT_USE_SYSTEM_PREFERRED_RNG,
            )
        } < 0
        {
            return Err("無法建立對話識別碼。".into());
        }
        let id: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let title = messages
            .first()
            .map(|m| {
                m.content
                    .lines()
                    .next()
                    .unwrap_or("")
                    .chars()
                    .take(50)
                    .collect()
            })
            .unwrap_or_else(|| "新對話".into());
        self.conversations.push(Conversation {
            id: id.clone(),
            title,
            pinned: false,
            title_manual: false,
            draft: String::new(),
            created_at: unix_now(),
            updated_at: unix_now(),
            messages,
        });
        Ok(id)
    }
    pub fn update(&mut self, id: &str, messages: Vec<Message>) -> AppResult<()> {
        let conversation = self
            .conversations
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or("找不到此對話。")?;
        conversation.messages = messages;
        conversation.updated_at = unix_now();
        Ok(())
    }
    pub fn remove(&mut self, id: &str) -> AppResult<()> {
        let index = self
            .conversations
            .iter()
            .position(|c| c.id == id)
            .ok_or("找不到此對話。")?;
        self.conversations.remove(index);
        Ok(())
    }
    fn validate(&self) -> AppResult<()> {
        let mut ids = std::collections::HashSet::new();
        if self.schema != 1 || self.conversations.len() > MAX_CONVERSATIONS {
            return Err("對話紀錄版本或大小不支援。".into());
        }
        for conversation in &self.conversations {
            if conversation.id.len() != 32
                || !conversation.id.bytes().all(|b| b.is_ascii_hexdigit())
                || !ids.insert(&conversation.id)
                || conversation.title.chars().count() > 100
                || conversation.draft.encode_utf16().count() > 16_000
                || conversation.messages.len() > 40
                || conversation.messages.iter().any(|m| {
                    !matches!(m.role.as_str(), "user" | "assistant") || m.content.len() > 1_048_576
                })
            {
                return Err("對話紀錄格式不正確；原檔已保留。".into());
            }
        }
        Ok(())
    }
}
/// 損毀時回報，不以空檔覆寫。呼叫端可改用暫存模式，讓使用者保有原始紀錄。
pub fn load(root: &Path) -> AppResult<Archive> {
    let path = root.join("history.dpapi");
    match fs::metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Archive::default()),
        Err(_) => return Err("無法讀取本機對話紀錄，原檔已保留。".into()),
        Ok(meta) if meta.len() > MAX_ARCHIVE_BYTES as u64 + 4096 => {
            return Err("本機對話紀錄過大，原檔已保留。".into())
        }
        _ => {}
    }
    let encrypted = fs::read(path).map_err(|_| "無法讀取本機對話紀錄。")?;
    let bytes = storage::protect(&encrypted, false)
        .map_err(|_| "無法解密對話紀錄，請確認目前是原本的 Windows 使用者。")?;
    let archive: Archive =
        serde_json::from_slice(&bytes).map_err(|_| "對話紀錄格式損毀，原檔已保留。")?;
    archive.validate()?;
    Ok(archive)
}
/// 先完成序列化與加密，再原子替換；寫入失敗不破壞上一份成功保存的檔案。
pub fn save(root: &Path, archive: &Archive) -> AppResult<()> {
    archive.validate()?;
    let bytes = serde_json::to_vec(archive).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_ARCHIVE_BYTES {
        return Err("本機對話已超過 32 MB，請刪除不需要的紀錄後再保存。".into());
    }
    storage::atomic_write(
        &root.join("history.dpapi"),
        &storage::protect(&bytes, true)?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn older_history_migrates_and_user_metadata_survives_message_updates() {
        let old = r#"{"id":"0123456789abcdef0123456789abcdef","title":"原標題","created_at":1,"updated_at":1,"messages":[]}"#;
        let conversation: Conversation = serde_json::from_str(old).unwrap();
        assert!(
            !conversation.pinned && !conversation.title_manual && conversation.draft.is_empty()
        );
        let mut archive = Archive {
            schema: 1,
            conversations: vec![conversation],
        };
        let id = archive.conversations[0].id.clone();
        archive.conversations[0].pinned = true;
        archive.conversations[0].title_manual = true;
        archive.conversations[0].title = "自訂標題".into();
        archive.conversations[0].draft = "尚未送出的內容".into();
        archive
            .update(&id, vec![crate::protocol::Message::user("新訊息")])
            .unwrap();
        let loaded: Archive =
            serde_json::from_str(&serde_json::to_string(&archive).unwrap()).unwrap();
        assert!(loaded.conversations[0].pinned && loaded.conversations[0].title_manual);
        assert_eq!(loaded.conversations[0].title, "自訂標題");
        assert_eq!(loaded.conversations[0].draft, "尚未送出的內容");
    }
    #[test]
    fn encrypted_history_roundtrip_update_delete_and_corruption() {
        let mut archive = Archive::default();
        let id = archive.insert(vec![Message::user("本機私密測試")]).unwrap();
        let root = std::env::temp_dir().join(format!("lm-ai-history-{id}"));
        save(&root, &archive).unwrap();
        let ciphertext = fs::read(root.join("history.dpapi")).unwrap();
        assert!(!String::from_utf8_lossy(&ciphertext).contains("本機私密測試"));
        let mut loaded = load(&root).unwrap();
        loaded
            .update(
                &id,
                vec![
                    Message::user("本機私密測試"),
                    Message::assistant("**已保存**".into()),
                ],
            )
            .unwrap();
        save(&root, &loaded).unwrap();
        assert_eq!(load(&root).unwrap().conversations[0].messages.len(), 2);
        loaded.remove(&id).unwrap();
        save(&root, &loaded).unwrap();
        assert!(load(&root).unwrap().conversations.is_empty());
        fs::write(root.join("history.dpapi"), b"corrupt").unwrap();
        assert!(load(&root).is_err());
        assert_eq!(fs::read(root.join("history.dpapi")).unwrap(), b"corrupt");
        fs::remove_file(root.join("history.dpapi")).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
