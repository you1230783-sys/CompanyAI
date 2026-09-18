//! EXE／NSIS 共用下載與簽章驗證；EXE 交由使用者手動更換，NSIS 才能啟動安裝。
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

impl UpdateArtifact {
    /// 固定本機檔名，不將伺服器提供的路徑用於本機寫檔。
    fn file_name(&self) -> AppResult<&'static str> {
        match self.kind.as_str() {
            "exe" => Ok("LM_AI.exe"),
            "nsis" => Ok("LM_AI_Setup.exe"),
            _ => Err("不支援的更新檔格式。".into()),
        }
    }
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

/// 先驗證每份候選清單，之後才比較版本，避免偽造高版本遮蔽合法更新。
fn validate_manifest(config: &Config, a: &UpdateArtifact) -> AppResult<()> {
    let url = config.endpoint(&a.url)?;
    crate::service::version_number(&a.version)?;
    if url.origin() != config.base_url()?.origin()
        || a.schema_version != 1
        || a.platform != "windows-x86_64"
        || !matches!(a.kind.as_str(), "exe" | "nsis")
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

pub fn validate_artifact(config: &Config, a: &UpdateArtifact, latest: &str) -> AppResult<()> {
    validate_manifest(config, a)?;
    let version = crate::service::version_number(&a.version)?;
    if version < crate::service::version_number(latest)?
        || version <= crate::service::version_number(crate::service::CURRENT_VERSION)?
    {
        return Err("更新檔低於已知最新版本，或不是比目前程式更新的版本。".into());
    }
    Ok(())
}

/// 數字版本優先；同版優先 EXE，方便現階段手動測試，未來較新 NSIS 仍會勝出。
fn preference(a: &UpdateArtifact) -> AppResult<([u32; 3], bool)> {
    Ok((crate::service::version_number(&a.version)?, a.kind == "exe"))
}

/// 各來源可回單份清單或最多 16 份的 JSON 陣列；每一份獨立驗證。
fn parse_manifests(body: &str) -> AppResult<Vec<UpdateArtifact>> {
    if body.len() > 65_536 {
        return Err("更新清單過大。".into());
    }
    let value: serde_json::Value = serde_json::from_str(body).map_err(|_| "更新清單不是 JSON。")?;
    let values = match value {
        serde_json::Value::Array(values) if values.len() <= 16 => values,
        value @ serde_json::Value::Object(_) => vec![value],
        _ => return Err("更新清單格式或數量不正確。".into()),
    };
    Ok(values
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect())
}

fn select_latest(config: &Config, candidates: Vec<UpdateArtifact>) -> AppResult<UpdateArtifact> {
    let mut best: Option<UpdateArtifact> = None;
    for candidate in candidates {
        if validate_manifest(config, &candidate).is_err() {
            continue;
        }
        let replace = match &best {
            Some(previous) => preference(&candidate)? > preference(previous)?,
            None => true,
        };
        if replace {
            best = Some(candidate);
        }
    }
    best.ok_or_else(|| "找不到通過簽章驗證的 EXE 或 NSIS 更新清單；請檢查網站發行檔。".into())
}

/// 只讀小型清單，不下載程式。不存在、舊 HTML 頁或損毀的單一來源不遮蔽其他合法來源。
pub fn discover_latest(config: &Config) -> AppResult<UpdateArtifact> {
    let mut candidates = Vec::new();
    for path in crate::config::UPDATE_MANIFEST_PATHS {
        let response = crate::transport::get(&config.endpoint(path)?, None);
        if let Ok(response) = response {
            if response.status == 200 {
                if let Ok(mut entries) = parse_manifests(&response.body) {
                    candidates.append(&mut entries);
                }
            }
        }
    }
    select_latest(config, candidates)
}

/// 使用者同意後重新選最新版；不得下載比已顯示版本更舊的檔案。
pub fn fetch_artifact(config: &Config, latest: &str) -> AppResult<UpdateArtifact> {
    let artifact = discover_latest(config)?;
    validate_artifact(config, &artifact, latest)?;
    Ok(artifact)
}

/// 開啟中的 EXE 禁止其他程序寫入／刪除，直到 CreateProcess 完成。
pub struct ReadyUpdate {
    pub artifact: UpdateArtifact,
    pub path: PathBuf,
    _locked_file: fs::File,
}

/// 以 Shell 原生 API 在檔案總管選取已驗證的 EXE；不組合命令列，也不執行更新檔。
pub fn show_download(ready: &ReadyUpdate) -> AppResult<()> {
    use windows::{
        core::PCWSTR,
        Win32::{System::Com::CoTaskMemFree, UI::Shell::*},
    };
    if ready.artifact.kind != "exe" {
        return Err("只有獨立 EXE 使用手動更換流程。".into());
    }
    let path = crate::wide(&ready.path.to_string_lossy());
    unsafe {
        let mut item = ptr::null_mut();
        SHParseDisplayName(PCWSTR(path.as_ptr()), None, &mut item, 0, None)
            .map_err(|e| format!("無法定位下載檔：{e}"))?;
        let result = SHOpenFolderAndSelectItems(item, None, 0);
        CoTaskMemFree(Some(item.cast()));
        result.map_err(|e| format!("無法開啟下載資料夾：{e}"))
    }
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
    let path = directory.join(artifact.file_name()?);
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
    if ready.artifact.kind != "nsis" {
        return Err("獨立 EXE 需手動更換，不能當作安裝程式執行。".into());
    }
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
        || !matches!(artifact.kind.as_str(), "exe" | "nsis")
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
mod update_tests;

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
