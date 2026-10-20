//! 設定以 JSON 儲存；憑證以 Windows DPAPI 加密，綁定目前的 Windows 使用者。
use crate::{config::Config, protocol::TokenResponse, unix_now, AppResult};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    ptr,
};
use windows_sys::Win32::{Foundation::LocalFree, Security::Cryptography::*};

#[derive(Clone, Serialize, Deserialize)]
pub struct Session {
    pub access_token: String,
    pub expires_at: u64,
    pub binding: String,
}
impl Session {
    pub fn from_token(token: TokenResponse, config: &Config) -> AppResult<Self> {
        token.validate()?;
        Ok(Self {
            access_token: token.access_token,
            expires_at: unix_now() + token.expires_in,
            binding: config.binding()?,
        })
    }
    pub fn valid_for(&self, config: &Config) -> bool {
        self.expires_at > unix_now()
            && config
                .binding()
                .is_ok_and(|binding| binding == self.binding)
    }
}

pub fn data_dir() -> AppResult<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(|root| PathBuf::from(root).join("CompanyAI"))
        .ok_or_else(|| "找不到 LOCALAPPDATA，無法保存應用程式設定。".into())
}

/// 只讀取小型設定檔；不存在代表第一次啟動，其餘錯誤應回報而非默默忽略。
fn read_optional(path: &Path) -> AppResult<Option<Vec<u8>>> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.len() > 65_536 => {
            return Err("設定或憑證檔過大，請清除後重新設定。".into())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("無法讀取設定：{error}")),
        _ => {}
    }
    fs::read(path)
        .map(Some)
        .map_err(|error| format!("無法讀取設定：{error}"))
}

pub fn load_config(root: &Path) -> AppResult<Config> {
    match read_optional(&root.join("settings.json"))? {
        Some(bytes) => serde_json::from_slice(&bytes)
            .map_err(|_| "settings.json 格式錯誤，請重新設定並儲存。".into()),
        None => Ok(Config::default()),
    }
}

/// 先寫暫存檔再替換，避免程式中途關閉時留下半份 JSON 或憑證。
pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let parent = path.parent().ok_or("設定路徑無效。")?;
    fs::create_dir_all(parent).map_err(|error| format!("無法建立設定資料夾：{error}"))?;
    let temp = path.with_extension(format!("{}.tmp", std::process::id()));
    fs::write(&temp, bytes)
        .and_then(|()| fs::rename(&temp, path))
        .map_err(|error| format!("無法保存設定或憑證：{error}"))
}

pub fn save_config(root: &Path, config: &Config) -> AppResult<()> {
    config.validate()?;
    let bytes = serde_json::to_vec_pretty(config).map_err(|error| error.to_string())?;
    atomic_write(&root.join("settings.json"), &bytes)
}
pub fn save_session(root: &Path, session: &Session) -> AppResult<()> {
    let bytes = serde_json::to_vec(session).map_err(|error| error.to_string())?;
    atomic_write(&root.join("session.dpapi"), &protect(&bytes, true)?)
}
pub fn load_session(root: &Path, config: &Config) -> AppResult<Option<Session>> {
    let Some(bytes) = read_optional(&root.join("session.dpapi"))? else {
        return Ok(None);
    };
    let decoded = protect(&bytes, false)?;
    let session: Session = serde_json::from_slice(&decoded)
        .map_err(|_| "登入記錄格式錯誤，請清除本機登入後重試。".to_string())?;
    Ok(session.valid_for(config).then_some(session))
}
pub fn clear_session(root: &Path) -> AppResult<()> {
    match fs::remove_file(root.join("session.dpapi")) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("無法清除本機憑證：{error}")),
    }
}

/// DPAPI 預設使用目前使用者範圍，不使用 CRYPTPROTECT_LOCAL_MACHINE。
/// 安裝 EXE 不需管理員；憑證不可透過複製檔案搬到另一位使用者帳號使用。
pub fn protect(bytes: &[u8], encrypt: bool) -> AppResult<Vec<u8>> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output: CRYPT_INTEGER_BLOB = unsafe { std::mem::zeroed() };
    // SAFETY: input 在同步呼叫期間有效；Windows 配置的 output 會在複製後以 LocalFree 釋放。
    let success = unsafe {
        if encrypt {
            CryptProtectData(
                &input,
                ptr::null(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                ptr::null_mut(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    };
    if success == 0 {
        return Err(
            "Windows 無法加密或解密登入記錄。請以原帳號登入，或清除本機登入重新授權。".into(),
        );
    }
    // SAFETY: 成功時 output 指向 Windows 回傳的 cbData 個有效 bytes。
    let result = unsafe {
        let copied = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData.cast());
        copied
    };
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_login_survives_reload_but_not_expiry_or_origin_change() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "company-ai-storage-{}-{unique}",
            std::process::id()
        ));
        let mut config = Config {
            server_url: "https://company.example".into(),
            model: "test".into(),
            ..Config::default()
        };
        save_config(&root, &config).unwrap();
        config.model = "updated-model".into();
        save_config(&root, &config).unwrap();
        assert_eq!(load_config(&root).unwrap().model, "updated-model");
        let token = TokenResponse {
            access_token: "private-test-token".into(),
            token_type: "Bearer".into(),
            expires_in: 3600,
        };
        let mut session = Session::from_token(token, &config).unwrap();
        save_session(&root, &session).unwrap();
        assert_eq!(
            load_session(&root, &config).unwrap().unwrap().access_token,
            "private-test-token"
        );
        let other = Config {
            server_url: "https://other.example".into(),
            ..config.clone()
        };
        assert!(load_session(&root, &other).unwrap().is_none());
        session.expires_at = unix_now() - 1;
        save_session(&root, &session).unwrap();
        assert!(load_session(&root, &config).unwrap().is_none());
        clear_session(&root).unwrap();
        assert!(!root.join("session.dpapi").exists());
        // 僅刪除本測試建立的已知檔案，不遞迴清理共用的暫存根目錄。
        fs::remove_file(root.join("settings.json")).unwrap();
        fs::remove_dir(root).unwrap();
    }
    #[test]
    fn dpapi_roundtrip_and_tampering() {
        let plaintext = b"private-test-token";
        let mut encrypted = protect(plaintext, true).unwrap();
        assert!(!encrypted
            .windows(plaintext.len())
            .any(|window| window == plaintext));
        assert_eq!(protect(&encrypted, false).unwrap(), plaintext);
        encrypted[20] ^= 1;
        assert!(protect(&encrypted, false).is_err());
    }
}
