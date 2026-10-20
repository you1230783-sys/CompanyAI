#![windows_subsystem = "windows"]
//! 在全新本機設定目錄顯示已完成的公開專案測試，供真正桌面目視核對。
//! 只複製本輪測試的歷史與診斷，不讀取一般使用者設定、登入或WebView profile。
use company_ai::{local_test::LocalTest, ui, AppResult};
use serde::Deserialize;
use std::{
    fs,
    os::windows::fs::MetadataExt,
    path::{Path, PathBuf},
};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Review {
    connection: PathBuf,
    source: PathBuf,
    destination: PathBuf,
}
fn copy_directory(source: &Path, target: &Path) -> AppResult<()> {
    fs::create_dir(target).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let metadata = fs::symlink_metadata(entry.path()).map_err(|e| e.to_string())?;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("測試紀錄不可包含重新解析點。".into());
        }
        if metadata.is_dir() {
            copy_directory(&entry.path(), &target.join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), target.join(entry.file_name())).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
fn run() -> AppResult<()> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".build")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let request: Review = serde_json::from_slice(
        &fs::read(base.join("ui074-review-config.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let source = request.source.canonicalize().map_err(|e| e.to_string())?;
    let destination_parent = request
        .destination
        .parent()
        .ok_or("缺少測試父目錄。")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !source.starts_with(&base) || !destination_parent.starts_with(&base) {
        return Err("目視驗證只接受本專案.build測試目錄。".into());
    }
    let mut local = LocalTest::load(&request.connection, &request.destination)?;
    local.config.notification_popups = false;
    for name in ["history.dpapi", "projects.dpapi"] {
        fs::copy(source.join(name), request.destination.join(name)).map_err(|e| e.to_string())?;
    }
    copy_directory(
        &source.join("project-runs"),
        &request.destination.join("project-runs"),
    )?;
    ui::run_local(local)
}
fn main() {
    if let Err(error) = run() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".build/ui074-review-error.txt");
        let _ = fs::write(path, error);
        std::process::exit(1);
    }
}
