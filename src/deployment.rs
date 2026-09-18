//! 使用者層級安裝與更新。PowerShell 腳本為編譯時內嵌的固定內容，參數以 JSON 傳遞。
//! 不把網址、路徑或伺服器回應插入命令字串；子程序隱藏執行。
use crate::{wide, AppResult};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
};
use windows_sys::Win32::{Security::Cryptography::*, UI::WindowsAndMessaging::*};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UpdateArtifact {
    pub url: String,
    pub sha256: String,
    pub version: String,
}

/// 對外更新位置必須同來源，且版本與版本服務一致；下載後還需驗證真正的檔案簽章。
pub fn validate_artifact(
    config: &crate::config::Config,
    artifact: &UpdateArtifact,
    latest: &str,
) -> AppResult<()> {
    let url = config.endpoint(&artifact.url)?;
    if url.origin() != config.base_url()?.origin()
        || artifact.version != latest
        || crate::service::version_number(&artifact.version)?
            <= crate::service::version_number(crate::service::CURRENT_VERSION)?
        || artifact.sha256.len() != 64
        || !artifact.sha256.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("更新檔來源、版本或 SHA256 不正確。".into());
    }
    Ok(())
}

fn encode_script(script: &str) -> AppResult<String> {
    // PS1 檔保留 UTF-8 BOM 供 Windows PowerShell 5.1 直接讀取；EncodedCommand 不帶 BOM。
    let bytes: Vec<u8> = script
        .trim_start_matches('\u{feff}')
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let mut size = 0;
    unsafe {
        if CryptBinaryToStringW(
            bytes.as_ptr(),
            bytes.len() as u32,
            CRYPT_STRING_BASE64 | CRYPT_STRING_NOCRLF,
            std::ptr::null_mut(),
            &mut size,
        ) == 0
        {
            return Err("無法編碼安裝腳本。".into());
        }
        let mut output = vec![0u16; size as usize];
        if CryptBinaryToStringW(
            bytes.as_ptr(),
            bytes.len() as u32,
            CRYPT_STRING_BASE64 | CRYPT_STRING_NOCRLF,
            output.as_mut_ptr(),
            &mut size,
        ) == 0
        {
            return Err("無法編碼安裝腳本。".into());
        }
        Ok(String::from_utf16_lossy(&output)
            .trim_end_matches('\0')
            .into())
    }
}
pub fn script_command(script: &str, plan: &Path) -> AppResult<Command> {
    let system = std::env::var_os("SystemRoot").ok_or("缺少 Windows 系統路徑。")?;
    let mut command =
        Command::new(PathBuf::from(system).join("System32/WindowsPowerShell/v1.0/powershell.exe"));
    command
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-EncodedCommand",
        ])
        .arg(encode_script(script)?)
        .env("LM_AI_PLAN", plan)
        .creation_flags(0x08000000);
    Ok(command)
}
fn run_script(script: &str, plan: &Path) -> AppResult<()> {
    let output = script_command(script, plan)?
        .output()
        .map_err(|e| format!("無法執行安裝程序：{e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "安裝／更新未完成：{}",
            String::from_utf8_lossy(&output.stderr)
                .chars()
                .take(1500)
                .collect::<String>()
        ))
    }
}
fn write_plan(directory: &Path, value: &serde_json::Value) -> AppResult<PathBuf> {
    fs::create_dir_all(directory).map_err(|e| format!("無法建立暫存目錄：{e}"))?;
    let path = directory.join("plan.json");
    fs::write(
        &path,
        serde_json::to_vec(value).map_err(|_| "無法建立安裝參數。")?,
    )
    .map_err(|e| format!("無法保存安裝參數：{e}"))?;
    Ok(path)
}
pub fn install(app: &[u8], runtime: &[u8]) -> AppResult<()> {
    if !app.starts_with(b"MZ") || !runtime.starts_with(b"MZ") {
        return Err("安裝包不完整。".into());
    }
    let local = PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("缺少使用者資料目錄。")?);
    let directory = local.join("CompanyAI/updates").join(crate::jobs::new_id()?);
    let target = local.join("Programs/LM_AI");
    if unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            wide("將安裝 LM_AI 到目前使用者的應用程式目錄，並建立開始功能表與桌面捷徑。是否繼續？")
                .as_ptr(),
            wide("LM_AI 安裝程式 · LARGAN").as_ptr(),
            MB_YESNO | MB_ICONQUESTION,
        )
    } != IDYES
    {
        return Ok(());
    }
    let plan = write_plan(
        &directory,
        &serde_json::json!({"target":target,"source":directory.join("LM_AI.exe"),"runtime":directory.join("WebView2.exe"),"version":crate::service::CURRENT_VERSION}),
    )?;
    fs::write(directory.join("LM_AI.exe"), app).map_err(|e| e.to_string())?;
    fs::write(directory.join("WebView2.exe"), runtime).map_err(|e| e.to_string())?;
    run_script(include_str!("../scripts/deployment/Install.ps1"), &plan)?;
    let _ = fs::remove_file(directory.join("WebView2.exe"));
    let _ = fs::remove_file(directory.join("LM_AI.exe"));
    let _ = fs::remove_file(plan);
    let _ = fs::remove_dir(directory);
    Ok(())
}
pub fn download(config: &crate::config::Config, artifact: &UpdateArtifact) -> AppResult<PathBuf> {
    validate_artifact(config, artifact, &artifact.version)?;
    let directory = crate::storage::data_dir()?
        .join("updates")
        .join(crate::jobs::new_id()?);
    let target = std::env::current_exe().map_err(|e| e.to_string())?;
    let plan = write_plan(
        &directory,
        &serde_json::json!({"url":config.endpoint(&artifact.url)?.as_str(),"sha256":artifact.sha256,"version":artifact.version,"target":target,"source":directory.join("LM_AI.new.exe")}),
    )?;
    run_script(include_str!("../scripts/deployment/Download.ps1"), &plan)?;
    Ok(plan)
}
pub fn launch_update(plan: &Path, restart: bool) -> AppResult<()> {
    let current = std::env::current_exe().map_err(|e| e.to_string())?;
    let directory = plan.parent().ok_or("更新目錄不正確。")?;
    let helper = directory.join("LM_AI_UpdateHelper.exe");
    fs::copy(&current, &helper).map_err(|e| format!("無法準備更新助手：{e}"))?;
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(plan).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    value["pid"] = serde_json::json!(std::process::id());
    value["restart"] = serde_json::json!(restart);
    fs::write(plan, serde_json::to_vec(&value).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    Command::new(helper)
        .arg("--apply-update")
        .arg(plan)
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|e| format!("無法啟動更新助手：{e}"))?;
    Ok(())
}
pub fn apply_update(plan: &Path) -> AppResult<()> {
    run_script(include_str!("../scripts/deployment/Apply-Update.ps1"), plan)
}
pub fn uninstall() -> AppResult<()> {
    if unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            wide("是否解除安裝 LM_AI？伺服器資料不會被刪除。").as_ptr(),
            wide("解除安裝 LM_AI").as_ptr(),
            MB_YESNO | MB_ICONQUESTION,
        )
    } != IDYES
    {
        return Ok(());
    }
    let clear = unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            wide("是否同時刪除本機設定、登入資料與聊天紀錄？選擇「否」可在重新安裝後保留資料。")
                .as_ptr(),
            wide("本機資料").as_ptr(),
            MB_YESNO | MB_DEFBUTTON2 | MB_ICONQUESTION,
        )
    } == IDYES;
    let directory =
        std::env::temp_dir().join(format!("LM_AI-uninstall-{}", crate::jobs::new_id()?));
    let plan = write_plan(
        &directory,
        &serde_json::json!({"pid":std::process::id(),"clear_data":clear}),
    )?;
    script_command(include_str!("../scripts/deployment/Uninstall.ps1"), &plan)?
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn update_metadata_rejects_cross_origin_and_old_versions() {
        let c = crate::config::Config::default();
        let mut a = UpdateArtifact {
            url: "/lm_server/desktop/LM_AI.exe".into(),
            version: "99.0.0".into(),
            sha256: "a".repeat(64),
        };
        assert!(validate_artifact(&c, &a, "99.0.0").is_ok());
        a.url = "https://other.example/app.exe".into();
        assert!(validate_artifact(&c, &a, "99.0.0").is_err());
        a.url = "/lm_server/desktop/app.exe".into();
        a.version = "0.1.0".into();
        assert!(validate_artifact(&c, &a, "0.1.0").is_err());
    }
}
