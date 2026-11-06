//! 私聊用 Windows CNG。金鑰與敏感緩衝區離開作用域時盡力覆寫。
use crate::AppResult;
use std::{mem::size_of, ptr};
use windows_sys::Win32::Security::Cryptography::*;

pub(super) const ITERATIONS: u64 = 600_000;
const NONCE_SIZE: usize = 12;
const TAG_SIZE: usize = 16;

pub(super) struct Secret<T: AsMut<[u8]>>(pub(super) T);

impl<T: AsMut<[u8]>> Drop for Secret<T> {
    fn drop(&mut self) {
        for byte in self.0.as_mut() {
            // SAFETY: 位元組屬於此緩衝區，volatile 防止編譯器省略清除。
            unsafe { ptr::write_volatile(byte, 0) };
        }
    }
}

pub(super) type Key = Secret<[u8; 32]>;

struct Algorithm(BCRYPT_ALG_HANDLE);

impl Algorithm {
    fn open(id: *const u16, flags: u32) -> AppResult<Self> {
        let mut handle = ptr::null_mut();
        // SAFETY: id 是 CNG 靜態字串，handle 是有效輸出指標。
        check(unsafe { BCryptOpenAlgorithmProvider(&mut handle, id, ptr::null(), flags) })?;
        Ok(Self(handle))
    }
}

impl Drop for Algorithm {
    fn drop(&mut self) {
        // SAFETY: 此物件持有開啟成功的演算法 handle。
        unsafe { BCryptCloseAlgorithmProvider(self.0, 0) };
    }
}

struct CipherKey(BCRYPT_KEY_HANDLE);

impl Drop for CipherKey {
    fn drop(&mut self) {
        // SAFETY: 此物件持有建立成功的金鑰 handle。
        unsafe { BCryptDestroyKey(self.0) };
    }
}

fn check(status: i32) -> AppResult<()> {
    if status < 0 {
        Err(format!(
            "Windows 無法處理私聊加密資料（0x{:08X}）。",
            status as u32
        ))
    } else {
        Ok(())
    }
}

pub(super) fn random<const N: usize>() -> AppResult<[u8; N]> {
    let mut bytes = [0; N];
    let len = u32::try_from(N).map_err(|_| "隨機資料長度超出限制。")?;
    // SAFETY: 系統亂數使用空演算法 handle；bytes 有 len 個可寫位元組。
    check(unsafe {
        BCryptGenRandom(
            ptr::null_mut(),
            bytes.as_mut_ptr(),
            len,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    })?;
    Ok(bytes)
}

/// RFC 5869：本機 KDF 與伺服器秘密共 64 位元組，依手冊的 info 分離用途。
pub(super) fn derive_kek(password: &[u8], salt: &[u8; 16], server: &Key) -> AppResult<Key> {
    let hmac = Algorithm::open(BCRYPT_SHA256_ALGORITHM, BCRYPT_ALG_HANDLE_HMAC_FLAG)?;
    let mut material = Secret([0; 64]);
    // SAFETY: 密碼已驗證為 6 至 64 個 ASCII 字元；其餘輸入均為固定長度陣列。
    check(unsafe {
        BCryptDeriveKeyPBKDF2(
            hmac.0,
            password.as_ptr(),
            password.len() as u32,
            salt.as_ptr(),
            salt.len() as u32,
            ITERATIONS,
            material.0.as_mut_ptr(),
            32,
            0,
        )
    })?;
    material.0[32..].copy_from_slice(&server.0);
    hkdf(&hmac, &material.0, salt, b"lmai-chat-kek-v1")
}

fn hkdf(hmac: &Algorithm, input: &[u8], salt: &[u8], info: &[u8]) -> AppResult<Key> {
    fn digest(hmac: &Algorithm, key: &[u8], bytes: &[u8]) -> AppResult<Key> {
        let mut result = Secret([0; 32]);
        // SAFETY: 兩個輸入切片均有效，SHA-256 固定寫入 32 位元組。
        check(unsafe {
            BCryptHash(
                hmac.0,
                key.as_ptr(),
                key.len() as u32,
                bytes.as_ptr(),
                bytes.len() as u32,
                result.0.as_mut_ptr(),
                32,
            )
        })?;
        Ok(result)
    }
    let extracted = digest(hmac, salt, input)?;
    let mut block = info.to_vec();
    block.push(1);
    digest(hmac, &extracted.0, &block)
}

/// 封存格式為 nonce | tag | 密文。AAD 綁定帳號、用途及格式。
pub(super) fn seal(key: &Key, aad: &[u8], plaintext: &[u8]) -> AppResult<Vec<u8>> {
    let mut nonce = random()?;
    let mut tag = [0; TAG_SIZE];
    let ciphertext = crypt(key, aad, plaintext, &mut nonce, &mut tag, BCryptEncrypt)?;
    let mut bytes = Vec::with_capacity(NONCE_SIZE + TAG_SIZE + ciphertext.0.len());
    bytes.extend_from_slice(&nonce);
    bytes.extend_from_slice(&tag);
    bytes.extend_from_slice(&ciphertext.0);
    Ok(bytes)
}

pub(super) fn open(key: &Key, aad: &[u8], bytes: &[u8]) -> AppResult<Secret<Vec<u8>>> {
    let Some((nonce, rest)) = bytes.split_first_chunk::<NONCE_SIZE>() else {
        return Err("私聊加密資料不完整。".into());
    };
    let Some((tag, ciphertext)) = rest.split_first_chunk::<TAG_SIZE>() else {
        return Err("私聊加密資料不完整。".into());
    };
    let mut nonce = *nonce;
    let mut tag = *tag;
    crypt(key, aad, ciphertext, &mut nonce, &mut tag, BCryptDecrypt)
}

type CipherOperation = unsafe extern "system" fn(
    BCRYPT_KEY_HANDLE,
    *const u8,
    u32,
    *const std::ffi::c_void,
    *mut u8,
    u32,
    *mut u8,
    u32,
    *mut u32,
    BCRYPT_FLAGS,
) -> i32;

fn crypt(
    key: &Key,
    aad: &[u8],
    bytes: &[u8],
    nonce: &mut [u8; NONCE_SIZE],
    tag: &mut [u8; TAG_SIZE],
    operation: CipherOperation,
) -> AppResult<Secret<Vec<u8>>> {
    let len = u32::try_from(bytes.len()).map_err(|_| "私聊資料長度超出限制。")?;
    let aad_len = u32::try_from(aad.len()).map_err(|_| "私聊資料識別長度超出限制。")?;
    let algorithm = Algorithm::open(BCRYPT_AES_ALGORITHM, 0)?;
    let mut handle = ptr::null_mut();
    // SAFETY: 鏈結模式是包含 NUL 的 CNG 字串；Windows 管理 AES 金鑰物件的緩衝區。
    unsafe {
        check(BCryptSetProperty(
            algorithm.0,
            BCRYPT_CHAINING_MODE,
            BCRYPT_CHAIN_MODE_GCM.cast(),
            (size_of::<u16>() * ("ChainingModeGCM".len() + 1)) as u32,
            0,
        ))?;
        check(BCryptGenerateSymmetricKey(
            algorithm.0,
            &mut handle,
            ptr::null_mut(),
            0,
            key.0.as_ptr(),
            32,
            0,
        ))?;
    }
    let cipher = CipherKey(handle);
    let info = BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO {
        cbSize: size_of::<BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO>() as u32,
        dwInfoVersion: BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO_VERSION,
        pbNonce: nonce.as_mut_ptr(),
        cbNonce: NONCE_SIZE as u32,
        pbAuthData: aad.as_ptr().cast_mut(),
        cbAuthData: aad_len,
        pbTag: tag.as_mut_ptr(),
        cbTag: TAG_SIZE as u32,
        ..Default::default()
    };
    let mut output = Secret(vec![0; bytes.len()]);
    let mut actual = 0;
    // SAFETY: GCM 不使用 block padding，輸出與輸入長度相同；AAD 只供讀取。
    check(unsafe {
        operation(
            cipher.0,
            bytes.as_ptr(),
            len,
            (&info as *const BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO).cast(),
            ptr::null_mut(),
            0,
            output.0.as_mut_ptr(),
            len,
            &mut actual,
            0,
        )
    })?;
    if actual != len {
        return Err("私聊加密資料長度不符。".into());
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authenticates_content_context_and_tag() {
        let key = Secret([17; 32]);
        let a = seal(&key, b"friend", b"private").unwrap();
        let mut b = seal(&key, b"friend", b"private").unwrap();
        assert_ne!(a[..NONCE_SIZE], b[..NONCE_SIZE]);
        assert_eq!(open(&key, b"friend", &a).unwrap().0, b"private");
        assert!(open(&key, b"other", &a).is_err());
        b[NONCE_SIZE] ^= 1;
        assert!(open(&key, b"friend", &b).is_err());
    }
}
