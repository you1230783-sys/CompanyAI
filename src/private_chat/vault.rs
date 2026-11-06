//! 每個帳號有獨立資料夾。加密的資料金鑰、快取及資料用不同的驗證範圍。
use super::crypto::{self, Key, Secret};
use crate::{storage, AppResult};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{ErrorKind, Read, Write},
    path::{Path, PathBuf},
};
use windows_sys::Win32::Storage::FileSystem::{
    MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
};

const FORMAT: &[u8; 8] = b"LMAIV001";
const MAX_CACHE_SECONDS: u64 = 24 * 60 * 60;
const SYMBOLS: &str = "!@#$%^&*()-_=+[]{};:,.?/~";
pub(super) const PASSWORD_MISMATCH: &str = "聊天密碼不正確，或本機金鑰無法驗證。";

pub(crate) fn password_rule(password: &str) -> AppResult<()> {
    if let Some(invalid) = password
        .chars()
        .find(|value| !value.is_ascii_alphanumeric() && !SYMBOLS.contains(*value))
    {
        let name = match invalid {
            ' ' => "空白".into(),
            value if value.is_control() => format!("U+{:04X}", value as u32),
            value => format!("「{value}」"),
        };
        return Err(format!("聊天密碼不能包含{name}。"));
    }
    if !(6..=64).contains(&password.len()) {
        return Err("聊天密碼必須為 6 到 64 個字元。".into());
    }
    Ok(())
}

pub(crate) struct ServerSecret {
    pub(super) value: Key,
    pub(super) version: u64,
}

#[derive(Clone)]
pub(crate) struct Account {
    pub(crate) directory: PathBuf,
    scope: [u8; 32],
}

impl Account {
    pub(crate) fn new(root: &Path, origin: &str, principal: &str) -> AppResult<Self> {
        crate::jobs::validate_id(principal)?;
        let scope: [u8; 32] = Sha256::digest(format!("{origin}\n{principal}")).into();
        Ok(Self {
            directory: root
                .join("private-chat")
                .join(format!("{:x}", Sha256::digest(scope))),
            scope,
        })
    }

    fn aad(&self, role: &[u8]) -> Vec<u8> {
        [FORMAT.as_slice(), self.scope.as_slice(), role].concat()
    }

    fn wrapped_path(&self) -> PathBuf {
        self.directory.join("key.dpapi")
    }

    pub(crate) fn exists(&self) -> bool {
        self.wrapped_path().is_file()
    }

    fn write_key(&self, key: &Key, password: &str, server: &ServerSecret) -> AppResult<()> {
        password_rule(password)?;
        let salt = crypto::random::<16>()?;
        let kek = crypto::derive_kek(password.as_bytes(), &salt, &server.value)?;
        let mut header = Vec::from(*FORMAT);
        header.extend_from_slice(&crypto::ITERATIONS.to_le_bytes());
        header.extend_from_slice(&server.version.to_le_bytes());
        header.extend_from_slice(&salt);
        let sealed = crypto::seal(&kek, &self.aad(&header), &key.0)?;
        header.extend(sealed);
        durable_write(&self.wrapped_path(), &storage::protect(&header, true)?)
    }

    pub(crate) fn create(&self, password: &str, server: &ServerSecret) -> AppResult<Vault> {
        if self.exists() {
            return Err("已設定聊天密碼，請解鎖。".into());
        }
        let key = Secret(crypto::random()?);
        self.write_key(&key, password, server)?;
        Ok(Vault {
            account: self.clone(),
            key,
        })
    }

    pub(crate) fn unlock(&self, password: &str, server: &ServerSecret) -> AppResult<Vault> {
        password_rule(password)?;
        let bytes = storage::protect(&read_limited(&self.wrapped_path(), 4096)?, false)?;
        let Some((header, ciphertext)) = bytes.split_first_chunk::<40>() else {
            return Err("私聊金鑰格式錯誤。".into());
        };
        let iterations = u64::from_le_bytes(header[8..16].try_into().unwrap());
        let version = u64::from_le_bytes(header[16..24].try_into().unwrap());
        if header[..8] != FORMAT[..] || iterations != crypto::ITERATIONS {
            return Err("私聊金鑰格式或密碼演算法版本不支援。".into());
        }
        if version != server.version {
            return Err("伺服器的聊天秘密已更換，請重設本機私聊。".into());
        }
        let salt = header[24..].try_into().unwrap();
        let kek = crypto::derive_kek(password.as_bytes(), &salt, &server.value)?;
        let key =
            crypto::open(&kek, &self.aad(header), ciphertext).map_err(|_| PASSWORD_MISMATCH)?;
        let key = Secret(
            key.0
                .as_slice()
                .try_into()
                .map_err(|_| "私聊資料金鑰長度錯誤。")?,
        );
        Ok(Vault {
            account: self.clone(),
            key,
        })
    }

    fn cache_path(&self) -> PathBuf {
        self.directory.join("unlocked.dpapi")
    }

    pub(crate) fn lock(&self) -> AppResult<()> {
        remove(&self.cache_path())
    }

    pub(crate) fn change_password(
        &self,
        old: &str,
        new: &str,
        server: &ServerSecret,
    ) -> AppResult<()> {
        let current = self.unlock(old, server)?;
        // 只重新包覆同一把資料金鑰；不必重寫好友或訊息資料。
        self.lock()?;
        self.write_key(&current.key, new, server)
    }

    /// 正常退出、登出與主動上鎖都會移除快取。只在非正常重開時復原未到期的金鑰。
    pub(crate) fn recover(&self, now: u64) -> AppResult<Option<(Vault, u64, u64)>> {
        if !self.cache_path().is_file() {
            return Ok(None);
        }
        let recovered = self.read_cache(now);
        if !matches!(&recovered, Ok(Some(_))) {
            self.lock()?;
        }
        recovered
    }

    fn read_cache(&self, now: u64) -> AppResult<Option<(Vault, u64, u64)>> {
        let bytes = Secret(storage::protect(
            &read_limited(&self.cache_path(), 4096)?,
            false,
        )?);
        if let Ok(bytes) = <&[u8; 80]>::try_from(bytes.0.as_slice()) {
            let unlocked_at = u64::from_le_bytes(bytes[..8].try_into().unwrap());
            let expiry = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
            let wrapped = read_limited(&self.wrapped_path(), 4096)?;
            if unlocked_at <= now
                && now < expiry
                && expiry <= unlocked_at.saturating_add(MAX_CACHE_SECONDS)
                && Sha256::digest(wrapped)[..] == bytes[16..48]
            {
                let key = Secret(bytes[48..].try_into().unwrap());
                return Ok(Some((
                    Vault {
                        account: self.clone(),
                        key,
                    },
                    unlocked_at,
                    expiry,
                )));
            }
        }
        Ok(None)
    }

    /// 先刪除快取與包覆金鑰，剩餘檔案即失去解密能力。裝置識別不影響本機加密抹除。
    pub(crate) fn erase(&self) -> AppResult<()> {
        self.lock()?;
        remove(&self.wrapped_path())?;
        remove(&self.directory.join("contacts.gcm"))
    }
}

pub(crate) struct Vault {
    account: Account,
    key: Key,
}

impl Vault {
    pub(crate) fn cache(&self, unlocked_at: u64, expiry: u64) -> AppResult<()> {
        if expiry <= unlocked_at || expiry > unlocked_at.saturating_add(MAX_CACHE_SECONDS) {
            return self.account.lock();
        }
        let wrapped = read_limited(&self.account.wrapped_path(), 4096)?;
        let mut bytes = Secret([0; 80]);
        bytes.0[..8].copy_from_slice(&unlocked_at.to_le_bytes());
        bytes.0[8..16].copy_from_slice(&expiry.to_le_bytes());
        bytes.0[16..48].copy_from_slice(&Sha256::digest(wrapped));
        bytes.0[48..].copy_from_slice(&self.key.0);
        durable_write(
            &self.account.cache_path(),
            &storage::protect(&bytes.0, true)?,
        )
    }

    pub(crate) fn read<T: DeserializeOwned + Default>(&self, file: &str, max: u64) -> AppResult<T> {
        let path = self.account.directory.join(file);
        if !path.exists() {
            return Ok(T::default());
        }
        let ciphertext = read_limited(&path, max)?;
        let bytes = crypto::open(&self.key, &self.account.aad(file.as_bytes()), &ciphertext)?;
        serde_json::from_slice(&bytes.0).map_err(|_| "私聊本機資料格式錯誤。".into())
    }

    pub(crate) fn write<T: Serialize>(&self, file: &str, value: &T) -> AppResult<()> {
        let data = Secret(serde_json::to_vec(value).map_err(|_| "無法編碼私聊本機資料。")?);
        let ciphertext = crypto::seal(&self.key, &self.account.aad(file.as_bytes()), &data.0)?;
        durable_write(&self.account.directory.join(file), &ciphertext)
    }
}

fn remove(path: &Path) -> AppResult<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("無法移除私聊本機資料：{error}")),
    }
}

pub(super) fn read_limited(path: &Path, max: u64) -> AppResult<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .and_then(|file| file.take(max + 1).read_to_end(&mut bytes))
        .map_err(|error| format!("無法讀取私聊本機資料：{error}"))?;
    if bytes.len() as u64 > max {
        return Err("私聊本機資料超出大小上限。".into());
    }
    Ok(bytes)
}

/// 先 flush 完整密文，再以寫透方式替換。上層只有在本函式成功後才能確認收件。
pub(super) fn durable_write(path: &Path, data: &[u8]) -> AppResult<()> {
    use std::os::windows::ffi::OsStrExt;
    let directory = path.parent().ok_or("私聊本機路徑不正確。")?;
    fs::create_dir_all(directory).map_err(|error| format!("無法建立私聊資料夾：{error}"))?;
    let temp = directory.join(format!("{}.tmp", crate::jobs::new_id()?));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(data)?;
        file.sync_all()?;
        drop(file);
        let wide = |value: &Path| {
            value
                .as_os_str()
                .encode_wide()
                .chain([0])
                .collect::<Vec<_>>()
        };
        // SAFETY: 兩個 UTF-16 檔名都在同一目錄且以 NUL 結尾；同步 API 不留存指標。
        if unsafe {
            MoveFileExW(
                wide(&temp).as_ptr(),
                wide(path).as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result.map_err(|error| format!("無法保存私聊本機資料：{error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_exact_password_alphabet_and_reports_bad_character() {
        for accepted in ["123456", "AbcD!@", SYMBOLS, &"x".repeat(64)] {
            assert!(password_rule(accepted).is_ok());
        }
        for length in [5, 65] {
            assert!(password_rule(&"a".repeat(length)).is_err());
        }
        for invalid in [' ', '\'', '"', '\\', '`', '<', '>', '|', '中', '\t', '\n'] {
            let name = match invalid {
                ' ' => "空白".into(),
                value if value.is_control() => format!("U+{:04X}", value as u32),
                value => value.to_string(),
            };
            assert!(password_rule(&format!("Abcd12{invalid}"))
                .unwrap_err()
                .contains(&name));
        }
    }

    #[test]
    fn encrypts_contacts_and_rewraps_without_exposing_key_or_plaintext() {
        let root =
            std::env::temp_dir().join(format!("lmai-chat-{}", crate::jobs::new_id().unwrap()));
        let account = Account::new(&root, "fixture-origin", "account-a").unwrap();
        let other = Account::new(&root, "fixture-origin", "account-b").unwrap();
        let secret = ServerSecret {
            version: 1,
            value: Secret([3; 32]),
        };
        let wrong = ServerSecret {
            version: 1,
            value: Secret([4; 32]),
        };
        let reset = ServerSecret {
            version: 2,
            value: Secret([4; 32]),
        };
        let vault = account.create("Test123", &secret).unwrap();
        let contents = vec!["不應出現在磁碟的測試文字".to_string(), "合成暱稱".into()];
        vault.write("contacts.gcm", &contents).unwrap();
        let encrypted = fs::read(account.directory.join("contacts.gcm")).unwrap();
        for value in &contents {
            assert!(!encrypted
                .windows(value.len())
                .any(|bytes| bytes == value.as_bytes()));
        }
        assert!(account.unlock("test123", &secret).is_err());
        assert!(account.unlock("Test123", &wrong).is_err());
        assert!(account.unlock("Test123", &reset).is_err());
        assert!(other.unlock("Test123", &secret).is_err());
        vault.cache(100, 160).unwrap();
        assert!(account.recover(159).unwrap().is_some());
        assert!(account.recover(160).unwrap().is_none());
        assert!(!account.cache_path().exists());
        vault.cache(100, 100_000).unwrap();
        assert!(!account.cache_path().exists());
        vault.cache(100, 160).unwrap();
        account.lock().unwrap();
        assert!(account.recover(120).unwrap().is_none());
        account
            .change_password("Test123", "Next234", &secret)
            .unwrap();
        assert_eq!(
            encrypted,
            fs::read(account.directory.join("contacts.gcm")).unwrap()
        );
        assert!(account.unlock("Test123", &secret).is_err());
        let unlocked = account.unlock("Next234", &secret).unwrap();
        assert_eq!(
            unlocked.read::<Vec<String>>("contacts.gcm", 4096).unwrap(),
            contents
        );
        drop(unlocked);
        drop(vault);
        account.erase().unwrap();
        assert!(account.unlock("Next234", &secret).is_err());
        assert!(!account.directory.join("contacts.gcm").exists());
    }
}
