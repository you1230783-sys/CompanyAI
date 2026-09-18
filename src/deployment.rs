//! 原生下載及更新驗證。使用者端只執行已驗證的 NSIS EXE，不啟動命令殼層或腳本。
use crate::{config::Config, AppResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, io::Read, os::windows::fs::OpenOptionsExt, path::PathBuf, process::Command, ptr};
use windows_sys::Win32::Security::Cryptography::*;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateArtifact {
    pub schema_version: u32,
    pub version: String,
    pub platform: String,
    pub kind: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
    /// RSA-3072 / PKCS#1 v1.5 / SHA256，十六進位編碼；信任金鑰隨程式內嵌。
    pub signature: String,
}

fn decode_hex(value: &str) -> AppResult<Vec<u8>> {
    if !value.len().is_multiple_of(2) || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("更新驗證資料格式錯誤。".into());
    }
    value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            u8::from_str_radix(std::str::from_utf8(pair).map_err(|_| "編碼錯誤。")?, 16)
                .map_err(|_| "更新驗證資料格式錯誤。".into())
        })
        .collect()
}

/// 簽章涵蓋版本、平台、包裝格式、長度與內容雜湊；網址可由網站改為同來源的最終下载位置。
fn signed_message(a: &UpdateArtifact) -> String {
    format!(
        "LM_AI_UPDATE_V1\n{}\n{}\n{}\n{}\n{}\n",
        a.version, a.platform, a.kind, a.size, a.sha256
    )
}

fn verify_signature(message: &[u8], signature: &[u8]) -> AppResult<()> {
    let public = include_bytes!("../assets/update-public-key.blob");
    let digest = Sha256::digest(message);
    // 所有 handle 在此範圍結束前釋放；公鑰固定，不接受 manifest 提供的金鑰。
    unsafe {
        let mut algorithm = ptr::null_mut();
        if BCryptOpenAlgorithmProvider(&mut algorithm, BCRYPT_RSA_ALGORITHM, ptr::null(), 0) < 0 {
            return Err("Windows 無法初始化更新簽章驗證。".into());
        }
        let mut key = ptr::null_mut();
        let imported = BCryptImportKeyPair(
            algorithm,
            ptr::null_mut(),
            BCRYPT_RSAPUBLIC_BLOB,
            &mut key,
            public.as_ptr(),
            public.len() as u32,
            0,
        );
        let padding = BCRYPT_PKCS1_PADDING_INFO {
            pszAlgId: BCRYPT_SHA256_ALGORITHM,
        };
        let valid = imported >= 0
            && signature.len() == 384
            && BCryptVerifySignature(
                key,
                (&padding as *const BCRYPT_PKCS1_PADDING_INFO).cast(),
                digest.as_ptr(),
                digest.len() as u32,
                signature.as_ptr(),
                signature.len() as u32,
                BCRYPT_PAD_PKCS1,
            ) >= 0;
        if !key.is_null() {
            BCryptDestroyKey(key);
        }
        BCryptCloseAlgorithmProvider(algorithm, 0);
        if !valid {
            return Err("更新簽章不正確，已停止；請聯絡管理者重新發布安裝包。".into());
        }
    }
    Ok(())
}

pub fn validate_artifact(config: &Config, a: &UpdateArtifact, latest: &str) -> AppResult<()> {
    let url = config.endpoint(&a.url)?;
    if url.origin() != config.base_url()?.origin()
        || a.version != latest
        || crate::service::version_number(&a.version)?
            <= crate::service::version_number(crate::service::CURRENT_VERSION)?
        || a.schema_version != 1
        || a.platform != "windows-x86_64"
        || a.kind != "nsis"
        || !(100_000..=1_073_741_824).contains(&a.size)
        || a.sha256.len() != 64
        || !a
            .sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || a.signature.len() != 768
    {
        return Err("更新檔來源、版本、平台、長度或格式不正確。".into());
    }
    verify_signature(signed_message(a).as_bytes(), &decode_hex(&a.signature)?)
}

/// 點擊同意下載後才查詢既有 download 路由；版本服務不必帶更新檔欄位。
pub fn fetch_artifact(config: &Config, latest: &str) -> AppResult<UpdateArtifact> {
    let response = crate::transport::get(&config.endpoint(crate::config::DOWNLOAD_PATH)?, None)?;
    if response.status != 200 {
        return Err(format!("更新資訊 HTTP {}；請稍後重試。", response.status));
    }
    let artifact: UpdateArtifact = serde_json::from_str(&response.body)
        .map_err(|_| "下載頁尚未提供新版 JSON 契約，請聯絡管理者。")?;
    validate_artifact(config, &artifact, latest)?;
    Ok(artifact)
}

/// 開啟中的 EXE 禁止其他程序寫入／刪除，直到 CreateProcess 完成。
pub struct ReadyUpdate {
    pub artifact: UpdateArtifact,
    pub path: PathBuf,
    _locked_file: fs::File,
}

fn verify_file(file: &mut fs::File, artifact: &UpdateArtifact) -> AppResult<()> {
    use std::io::{Seek, SeekFrom};
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() != artifact.size {
        return Err("更新檔長度不符。".into());
    }
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    if format!("{:x}", hash.finalize()) != artifact.sha256 {
        return Err("更新檔 SHA256 不符，已停止安裝。".into());
    }
    Ok(())
}

pub fn download(config: &Config, latest: &str) -> AppResult<ReadyUpdate> {
    let artifact = fetch_artifact(config, latest)?;
    let directory = crate::storage::data_dir()?
        .join("updates")
        .join(crate::jobs::new_id()?);
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let path = directory.join("LM_AI_Setup.exe");
    let result = (|| {
        crate::transport::download_file(&config.endpoint(&artifact.url)?, &path, artifact.size)?;
        let mut file = fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&path)
            .map_err(|e| e.to_string())?;
        verify_file(&mut file, &artifact)?;
        Ok(ReadyUpdate {
            artifact,
            path: path.clone(),
            _locked_file: file,
        })
    })();
    if result.is_err() {
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&directory);
    }
    result
}

/// NSIS 等指定 PID 退出、取得同一實例鎖後才替換；失敗由安裝程式顯示原生錯誤。
pub fn launch_update(ready: &ReadyUpdate, config: &Config, latest: &str) -> AppResult<()> {
    validate_artifact(config, &ready.artifact, latest)?;
    Command::new(&ready.path)
        .arg(format!("/UPDATEPID={}", std::process::id()))
        .arg("/RESTART")
        .spawn()
        .map_err(|e| format!("無法啟動 NSIS 安裝程式，主程式保持開啟：{e}"))?;
    Ok(())
}

/// 建置後以和正式更新相同的 CNG／SHA256 驗證成品；只讀檔案，絕不啟動安裝。
pub fn verify_release(manifest: &std::path::Path, installer: &std::path::Path) -> AppResult<()> {
    let bytes = fs::read(manifest).map_err(|e| e.to_string())?;
    if bytes.len() > 65_536 {
        return Err("更新資訊過大。".into());
    }
    let artifact: UpdateArtifact = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if artifact.version != crate::service::CURRENT_VERSION
        || artifact.schema_version != 1
        || artifact.platform != "windows-x86_64"
        || artifact.kind != "nsis"
    {
        return Err("發行資訊與程式版本不符。".into());
    }
    verify_signature(
        signed_message(&artifact).as_bytes(),
        &decode_hex(&artifact.signature)?,
    )?;
    let mut file = fs::File::open(installer).map_err(|e| e.to_string())?;
    verify_file(&mut file, &artifact)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pinned_signature_rejects_modified_content() {
        let signature = include_bytes!("../assets/update-signature-test.bin");
        assert!(verify_signature(b"LM_AI signature verification test", signature).is_ok());
        assert!(verify_signature(b"modified", signature).is_err());
        let mut corrupt = signature.to_vec();
        corrupt[20] ^= 1;
        assert!(verify_signature(b"LM_AI signature verification test", &corrupt).is_err());
        assert!(decode_hex("z0").is_err());
    }
    #[test]
    fn artifact_rejects_untrusted_and_incompatible_metadata() {
        let config = Config::default();
        let mut a = UpdateArtifact {
            schema_version: 1,
            version: "99.0.0".into(),
            platform: "windows-x86_64".into(),
            kind: "nsis".into(),
            url: "/lm_server/setup.exe".into(),
            size: 100_000,
            sha256: "a".repeat(64),
            signature: "a".repeat(768),
        };
        assert!(validate_artifact(&config, &a, "99.0.0")
            .unwrap_err()
            .contains("簽章"));
        a.url = "https://evil.example/setup.exe".into();
        assert!(validate_artifact(&config, &a, "99.0.0").is_err());
        a.url = "/setup.exe".into();
        a.version = "0.1.0".into();
        assert!(validate_artifact(&config, &a, "0.1.0").is_err());
    }
}
