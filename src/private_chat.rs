//! 私聊與 AI 工具的資料路徑完全分開；WebView 只收到目前解鎖的私聊畫面。
mod api;
mod crypto;
mod vault;

pub(crate) use api::{Api, Contacts, Error, Friend, Invitations, Profile};
pub(crate) use vault::{password_rule, Account, Vault};

use crate::AppResult;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(transparent)]
pub(crate) struct Password(String);

impl Password {
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

impl Drop for Password {
    fn drop(&mut self) {
        clear_text(&mut self.0);
    }
}

fn clear_text(value: &mut String) {
    // SAFETY: 將已配置字串的位元組覆寫為 NUL，仍符合 UTF-8。
    for byte in unsafe { value.as_bytes_mut() } {
        unsafe { std::ptr::write_volatile(byte, 0) };
    }
    value.clear();
}

pub(crate) fn valid_device(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

pub(crate) fn uuid() -> AppResult<String> {
    let mut bytes = crypto::random::<16>()?;
    bytes[6] = (bytes[6] & 15) | 0x40;
    bytes[8] = (bytes[8] & 63) | 0x80;
    let encoded: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &encoded[..8],
        &encoded[8..12],
        &encoded[12..16],
        &encoded[16..20],
        &encoded[20..]
    ))
}

/// 過程中的密碼嘗試次數不顯示在診斷或磁碟。上鎖時間一律使用 native 時鐘。
#[derive(Default)]
pub(crate) struct Attempts {
    failed: u8,
    until: u64,
}

impl Attempts {
    pub(crate) fn check(&self, now: u64) -> AppResult<()> {
        if now < self.until {
            return Err(format!("聊天密碼嘗試過多，請等 {} 秒。", self.until - now));
        }
        Ok(())
    }

    pub(crate) fn fail(&mut self, now: u64) {
        self.failed = self.failed.saturating_add(1);
        if self.failed >= 5 {
            let wait = 5 * (1 << self.failed.saturating_sub(5).min(6));
            self.until = now.saturating_add(wait);
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LockWhen {
    #[default]
    Leave,
    SinceUnlock,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct LockPolicy {
    pub(crate) when: LockWhen,
    pub(crate) minutes: u16,
    pub(crate) windows_lock: bool,
}

impl Default for LockPolicy {
    fn default() -> Self {
        Self {
            when: LockWhen::Leave,
            minutes: 0,
            windows_lock: true,
        }
    }
}

impl LockPolicy {
    pub(crate) fn valid(self) -> bool {
        let allowed: &[u16] = match self.when {
            LockWhen::Leave => &[0, 1, 5, 15, 30, 60],
            LockWhen::SinceUnlock => &[5, 15, 30, 60, 240, 480, 720, 1440],
        };
        allowed.contains(&self.minutes)
    }

    /// 離開模式在聊天畫面中不開始上鎖倒數。非正常重開快取仍最多 24 小時。
    pub(crate) fn deadline(self, unlocked_at: u64, left_at: Option<u64>) -> Option<u64> {
        let seconds = u64::from(self.minutes) * 60;
        match self.when {
            LockWhen::Leave => left_at.map(|at| at.saturating_add(seconds)),
            LockWhen::SinceUnlock => Some(unlocked_at.saturating_add(seconds)),
        }
    }

    pub(crate) fn cache_deadline(self, unlocked_at: u64, left_at: Option<u64>) -> u64 {
        let maximum = unlocked_at.saturating_add(86_400);
        self.deadline(unlocked_at, left_at)
            .unwrap_or(maximum)
            .min(maximum)
    }
}

#[derive(Deserialize, Serialize)]
pub(crate) struct Device {
    pub(crate) id: String,
    pub(crate) active: bool,
}

impl Device {
    pub(crate) fn load(account: &Account) -> AppResult<Option<Self>> {
        let path = account.directory.join("device.dpapi");
        if !path.is_file() {
            return Ok(None);
        }
        let raw = vault::read_limited(&path, 4096)?;
        let bytes = crate::storage::protect(&raw, false)?;
        let device: Self = serde_json::from_slice(&bytes).map_err(|_| "私聊裝置記錄格式錯誤。")?;
        if !valid_device(&device.id) {
            return Err("私聊裝置識別格式錯誤。".into());
        }
        Ok(Some(device))
    }

    pub(crate) fn load_or_create(account: &Account) -> AppResult<Self> {
        if let Some(existing) = Self::load(account)? {
            return Ok(existing);
        }
        let device = Self {
            id: uuid()?,
            active: false,
        };
        device.save(account)?;
        Ok(device)
    }

    pub(crate) fn save(&self, account: &Account) -> AppResult<()> {
        let bytes = serde_json::to_vec(self).map_err(|_| "無法編碼私聊裝置記錄。")?;
        vault::durable_write(
            &account.directory.join("device.dpapi"),
            &crate::storage::protect(&bytes, true)?,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_policies_and_password_delay_use_injected_time() {
        let leave = LockPolicy {
            minutes: 5,
            ..LockPolicy::default()
        };
        assert_eq!(leave.deadline(10, None), None);
        assert_eq!(leave.deadline(10, Some(40)), Some(340));
        assert_eq!(leave.cache_deadline(10, Some(100_000)), 86_410);
        let timed = LockPolicy {
            when: LockWhen::SinceUnlock,
            ..leave
        };
        assert_eq!(timed.deadline(10, Some(50)), Some(310));
        let mut attempts = Attempts::default();
        for _ in 0..5 {
            attempts.fail(100);
        }
        assert!(attempts.check(104).is_err());
        assert!(attempts.check(105).is_ok());
        attempts.fail(105);
        assert!(attempts.check(114).is_err());
        assert!(attempts.check(115).is_ok());
    }
}
