//! 對話的輸出位置只取自原生識別，不採用模型或網頁提供的資料夾。
//! 寫檔者仍須在整段操作持有原有專案／父目錄鎖，並用create_new保留舊成果。
use super::*;
use sha2::{Digest, Sha256};

/// 不受對話改名、任務輪次或程式重啟影響，也不將對話內容放進路徑。
pub fn conversation_folder_name(conversation: &str) -> AppResult<String> {
    crate::jobs::validate_id(conversation)?;
    Ok(format!(
        "對話_{:x}",
        Sha256::digest(conversation.as_bytes())
    ))
}

pub(super) fn ensure_output_folder(base: &Path, name: &str) -> AppResult<()> {
    if relative(name)?.components().count() != 1 {
        return Err("輸出資料夾必須是專案成果目錄下的單一名稱。".into());
    }
    let folder = base.join(name);
    match fs::create_dir(&folder) {
        Ok(()) => (),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(error) => return Err(format!("無法建立對話成果資料夾：{error}")),
    }
    // 同名路徑必須仍是正常目錄，不能重用連結或一般檔案。
    let guard = pin(&folder)?;
    if !guard
        .last()
        .ok_or("缺少成果目錄handle。")?
        .metadata()
        .map_err(|e| e.to_string())?
        .is_dir()
    {
        return Err("同名成果位置不是資料夾，未建立或改寫任何成果。".into());
    }
    Ok(())
}

/// 只有第一次實際輸出才建目錄；續接保留已驗證的舊目錄與既有草稿位置。
/// 未啟用對話的內部工具／契約範例，仍以同一Broker的快取共用一次建立的目錄。
pub(super) fn shared_output_folder(
    base: &Path,
    selected: &mut Option<String>,
    conversation: Option<&str>,
) -> AppResult<String> {
    let name = match selected.as_ref() {
        Some(name) => name.clone(),
        None => match conversation {
            Some(conversation) => conversation_folder_name(conversation)?,
            None => create_output_folder(base)?,
        },
    };
    ensure_output_folder(base, &name)?;
    *selected = Some(name.clone());
    Ok(name)
}

struct Reservation {
    file: Option<File>,
    path: PathBuf,
}
impl Drop for Reservation {
    fn drop(&mut self) {
        self.file.take();
        let _ = fs::remove_file(&self.path);
    }
}

pub(super) struct ExternalOutput {
    pub name: String,
    pub path: PathBuf,
    _reservation: Reservation,
}

/// CAD SaveAs要求目的檔尚不存在，因此用旁邊的create_new保留檔協調同名輸出。
/// 保留檔活到原生操作結束；CAD本身仍再次拒絕已存在的DWG，絕不授權覆寫。
pub(super) fn reserve_external_output(folder: &Path, name: &str) -> AppResult<ExternalOutput> {
    if relative(name)?.components().count() != 1 {
        return Err("CAD成果只接受單一檔名。".into());
    }
    let stem = Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("檔名無法辨識。")?;
    let extension = Path::new(name)
        .extension()
        .and_then(|s| s.to_str())
        .ok_or("成果缺少副檔名。")?;
    for number in 1..=10_000 {
        let name = if number == 1 {
            name.to_owned()
        } else {
            format!("{stem}_{number}.{extension}")
        };
        let path = folder.join(&name);
        let reservation_path = folder.join(format!(
            ".cad-reserved-{:x}",
            Sha256::digest(name.to_lowercase().as_bytes())
        ));
        let file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .share_mode(0)
            .open(&reservation_path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.to_string()),
        };
        let reservation = Reservation {
            file: Some(file),
            path: reservation_path,
        };
        match fs::symlink_metadata(&path) {
            Ok(_) => continue,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ExternalOutput {
                    name,
                    path,
                    _reservation: reservation,
                });
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    Err("同名CAD成果過多，請改用其他檔名。".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 只建立本測試的獨立目錄；不碰使用者對話或舊成果。
    fn folder() -> PathBuf {
        let path = std::env::current_dir()
            .unwrap()
            .join(".build")
            .join(format!(
                "output-contract-{}",
                crate::jobs::new_id().unwrap()
            ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn a_conversation_reuses_one_folder_across_brokers_and_preserves_old_tasks() {
        let base = folder();
        let mut first = None;
        let shared = shared_output_folder(&base, &mut first, Some("conversation_a")).unwrap();
        fs::write(
            base.join(&shared).join("diagram.mmd"),
            "flowchart TD\nA-->B",
        )
        .unwrap();
        assert_eq!(
            shared_output_folder(&base, &mut None, Some("conversation_a")).unwrap(),
            shared
        );
        assert_ne!(
            shared_output_folder(&base, &mut None, Some("conversation_b")).unwrap(),
            shared
        );
        let mut old = Some("20261019_150054".into());
        assert_eq!(
            shared_output_folder(&base, &mut old, Some("conversation_a")).unwrap(),
            "20261019_150054"
        );
        assert!(base.join(shared).join("diagram.mmd").exists());
        assert!(conversation_folder_name("../outside").is_err());
        assert!(ensure_output_folder(&base, "../outside").is_err());
    }

    #[test]
    fn cad_reservations_do_not_overwrite_or_claim_the_same_filename() {
        let base = folder();
        fs::write(base.join("drawing.dwg"), b"original").unwrap();
        let second = reserve_external_output(&base, "drawing.dwg").unwrap();
        let third = reserve_external_output(&base, "DRAWING.dwg").unwrap();
        assert_eq!(second.name, "drawing_2.dwg");
        assert_eq!(third.name, "DRAWING_3.dwg");
        assert!(!second.path.exists());
        drop(second);
        drop(third);
        assert_eq!(
            reserve_external_output(&base, "drawing.dwg").unwrap().name,
            "drawing_2.dwg"
        );
        assert_eq!(fs::read(base.join("drawing.dwg")).unwrap(), b"original");
    }

    #[test]
    fn shared_stage_lock_can_coexist_with_readonly_source_parent() {
        let base = folder();
        fs::write(base.join("source.dwg"), b"source").unwrap();
        let source_dirs = pin(&base).unwrap();
        let source = checked_file(&base.join("source.dwg")).unwrap();
        let stage = pin_output_directory(&base).unwrap();
        let (_, file) = reserve_output(&base, "new.dwg").unwrap();
        drop(file);
        assert!(OpenOptions::new()
            .write(true)
            .open(base.join("source.dwg"))
            .is_err());
        drop(stage);
        drop(source);
        drop(source_dirs);
        assert!(base.join("new.dwg").exists());
    }
}
