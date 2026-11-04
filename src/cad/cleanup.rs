//! 安裝成功後清理已知舊CAD環境；不依資料夾名稱或副檔名直接遞迴刪除。
//! 先取得整棵樹的固定handle並核對內容，之後只刪除這些已驗證的物件。
use crate::{projects::files, AppResult};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::Read,
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
    path::{Component, Path, PathBuf},
};
use windows_sys::Win32::{
    Foundation::{GENERIC_READ, GENERIC_WRITE},
    Storage::FileSystem::*,
};

const HISTORY: &str = include_str!("../../assets/cad-runtime-history.json");
const DELETE_ACCESS: u32 = 0x0001_0000;

#[derive(Default, Serialize)]
pub struct Report {
    pub removed: Vec<String>,
    pub kept: Vec<Kept>,
}
#[derive(Serialize)]
pub struct Kept {
    pub directory: String,
    pub reason: String,
}

/// 僅處理執行此EXE所在目錄，不接受外部指定清理位置。
/// 新版環境必須先完整核對；安裝回復路徑不會呼叫此入口。
pub fn obsolete_runtimes() -> AppResult<Report> {
    let (current, _runtime_guards) = super::runtime()?;
    let root = current.parent().ok_or("找不到CAD安裝目錄。")?;
    let trusted: BTreeSet<String> = serde_json::from_str(HISTORY).map_err(|e| e.to_string())?;
    clean_root(root, super::FINGERPRINT.trim(), &trusted)
}

fn runtime_name(name: &str) -> bool {
    name.strip_prefix("cad-")
        .is_some_and(|suffix| suffix.len() == 16 && suffix.bytes().all(|c| c.is_ascii_hexdigit()))
}

fn clean_root(root: &Path, current: &str, trusted: &BTreeSet<String>) -> AppResult<Report> {
    // 安裝目錄及祖先固定後，候選目錄只能是同一層的直接子目錄。
    let _parents = files::pin(root)?;
    let current_name = format!("cad-{}", current.get(..16).ok_or("目前CAD指紋不合法。")?);
    let mut report = Report::default();
    for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !runtime_name(&name) || name.eq_ignore_ascii_case(&current_name) {
            continue;
        }
        match remove_verified_tree(&entry.path(), &name, trusted) {
            Ok(()) => report.removed.push(name),
            Err(reason) => report.kept.push(Kept {
                directory: name,
                reason,
            }),
        }
    }
    Ok(report)
}

/// 所有目錄拒絕重解析點；檔案另拒絕硬連結、唯讀及占用。
/// 檔案要求可寫入但不實際寫入；Windows會拒絕仍被載入為image的EXE／DLL。
/// 只要求READ／DELETE無法偵測已關閉檔案handle但仍載入中的DLL，可能造成部分刪除。
fn open_locked(path: &Path, directory: bool) -> AppResult<File> {
    let file = OpenOptions::new()
        .access_mode(GENERIC_READ | DELETE_ACCESS | if directory { 0 } else { GENERIC_WRITE })
        .share_mode(if directory { FILE_SHARE_READ } else { 0 })
        .custom_flags(
            FILE_FLAG_OPEN_REPARSE_POINT
                | if directory {
                    FILE_FLAG_BACKUP_SEMANTICS
                } else {
                    0
                },
        )
        .open(path)
        .map_err(|e| format!("檔案使用中或無法取得清理權限：{e}"))?;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0
        || info.dwFileAttributes & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_READONLY) != 0
        || (info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0) != directory
        || (!directory && info.nNumberOfLinks != 1)
    {
        return Err("環境包含連結、唯讀或無法驗證的項目，保留原位。".into());
    }
    Ok(file)
}

fn relative(name: &str) -> AppResult<PathBuf> {
    let path = PathBuf::from(name);
    if name.is_empty()
        || name.contains(':')
        || name.contains('\\')
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err("舊環境清單包含不合法相對路徑。".into());
    }
    Ok(path)
}

fn remove_verified_tree(root: &Path, name: &str, trusted: &BTreeSet<String>) -> AppResult<()> {
    let directory = open_locked(root, true)?;
    let mut manifest = open_locked(&root.join("manifest.json"), false)?;
    let mut raw = Vec::new();
    Read::by_ref(&mut manifest)
        .take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut raw)
        .map_err(|e| e.to_string())?;
    let fingerprint = format!("{:x}", Sha256::digest(&raw));
    if raw.len() > 2 * 1024 * 1024
        || !trusted.contains(&fingerprint)
        || !name.eq_ignore_ascii_case(&format!("cad-{}", &fingerprint[..16]))
    {
        return Err("不是已知發行版本的CAD環境，未刪除。".into());
    }
    let entries: BTreeMap<String, String> =
        serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    if entries.is_empty() || entries.len() > 4096 {
        return Err("舊環境清單不完整。".into());
    }
    let mut expected = BTreeMap::new();
    let mut allowed_dirs = BTreeSet::new();
    for (name, hash) in entries {
        let path = relative(&name)?;
        let mut parent = path.parent();
        while let Some(p) = parent.filter(|p| !p.as_os_str().is_empty()) {
            allowed_dirs.insert(p.to_path_buf());
            parent = p.parent();
        }
        expected.insert(path, hash);
    }
    let mut directories = vec![(PathBuf::new(), directory)];
    let mut locked_files = Vec::new();
    let mut cursor = 0;
    while cursor < directories.len() {
        let base = directories[cursor].0.clone();
        for item in fs::read_dir(root.join(&base)).map_err(|e| e.to_string())? {
            let item = item.map_err(|e| e.to_string())?;
            let rel = base.join(item.file_name());
            if rel == Path::new("manifest.json") {
                continue;
            }
            if item.file_type().map_err(|e| e.to_string())?.is_dir() {
                if !allowed_dirs.contains(&rel) {
                    return Err("包含清單以外的資料夾，保留原位。".into());
                }
                directories.push((rel, open_locked(&item.path(), true)?));
            } else {
                let hash = expected.get(&rel).ok_or("包含清單以外的檔案，保留原位。")?;
                let mut file = open_locked(&item.path(), false)?;
                let mut digest = Sha256::new();
                std::io::copy(&mut file, &mut digest).map_err(|e| e.to_string())?;
                if format!("{:x}", digest.finalize()) != *hash {
                    return Err("環境內有被修改的檔案，保留原位。".into());
                }
                locked_files.push(file);
            }
        }
        cursor += 1;
    }
    // 到這裡才開始刪除：未知檔案、內容變更或任何占用都不會造成先刪一半。
    // 已缺少的已知項目允許重試；manifest最後才刪，支援中斷後再次清理。
    for file in locked_files {
        delete_handle(file)?;
    }
    // breadth-first收集的子目錄逆序處理，永遠先移除最深層；不使用remove_dir_all。
    while directories.len() > 1 {
        let (_, file) = directories.pop().ok_or("清理目錄索引失效。")?;
        delete_handle(file)?;
    }
    delete_handle(manifest)?;
    let (_, directory) = directories.pop().ok_or("缺少舊環境目錄。")?;
    delete_handle(directory)
}

/// 只刪除已開啟、已核對身分的物件，避免核對與刪除之間路徑被替換。
fn delete_handle(file: File) -> AppResult<()> {
    let info = FILE_DISPOSITION_INFO { DeleteFile: true };
    if unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            FileDispositionInfo,
            (&info as *const FILE_DISPOSITION_INFO).cast(),
            std::mem::size_of_val(&info) as u32,
        )
    } == 0
    {
        return Err(format!(
            "舊環境清理未完成，保留其餘項目：{}",
            std::io::Error::last_os_error()
        ));
    }
    drop(file);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        root: PathBuf,
        old: PathBuf,
        trusted: BTreeSet<String>,
        current: String,
    }
    impl Fixture {
        fn new() -> Self {
            Self::with_library(b"library")
        }
        fn with_library(library: &[u8]) -> Self {
            let root = std::env::temp_dir().join(format!(
                "lmai-cad-cleanup-{}",
                crate::jobs::new_id().unwrap()
            ));
            fs::create_dir(&root).unwrap();
            let entries = BTreeMap::from([
                ("worker.py", format!("{:x}", Sha256::digest(b"worker"))),
                (
                    "runtime/library.py",
                    format!("{:x}", Sha256::digest(library)),
                ),
            ]);
            let raw = serde_json::to_vec(&entries).unwrap();
            let hash = format!("{:x}", Sha256::digest(&raw));
            let old = root.join(format!("cad-{}", &hash[..16]));
            fs::create_dir_all(old.join("runtime")).unwrap();
            fs::write(old.join("manifest.json"), raw).unwrap();
            fs::write(old.join("worker.py"), b"worker").unwrap();
            fs::write(old.join("runtime/library.py"), library).unwrap();
            let current = "c".repeat(64);
            let current_dir = root.join(format!("cad-{}", &current[..16]));
            fs::create_dir(&current_dir).unwrap();
            fs::write(current_dir.join("keep.txt"), b"current runtime").unwrap();
            Self {
                root,
                old,
                trusted: BTreeSet::from([hash]),
                current,
            }
        }
        fn clean(&self) -> Report {
            clean_root(&self.root, &self.current, &self.trusted).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            // 測試只清理自行create_new的新目錄；不接受使用者或安裝路徑。
            assert!(self.root.is_absolute() && self.root.starts_with(std::env::temp_dir()));
            assert!(self
                .root
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("lmai-cad-cleanup-"));
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    #[test]
    fn removes_known_obsolete_tree_and_keeps_current_runtime() {
        let fixture = Fixture::new();
        assert_eq!(fixture.clean().removed.len(), 1);
        assert!(!fixture.old.exists());
        assert_eq!(
            fs::read(
                fixture
                    .root
                    .join(format!("cad-{}/keep.txt", &fixture.current[..16]))
            )
            .unwrap(),
            b"current runtime"
        );
    }

    #[test]
    fn unknown_or_modified_content_is_kept_without_partial_deletion() {
        for case in [
            "unknown_manifest",
            "user_file",
            "modified_file",
            "unknown_directory",
        ] {
            let fixture = Fixture::new();
            match case {
                "unknown_manifest" => fs::write(fixture.old.join("manifest.json"), b"{}").unwrap(),
                "user_file" => fs::write(fixture.old.join("my-notes.txt"), b"personal").unwrap(),
                "modified_file" => {
                    fs::write(fixture.old.join("runtime/library.py"), b"edited").unwrap()
                }
                _ => fs::create_dir(fixture.old.join("personal")).unwrap(),
            }
            let report = fixture.clean();
            assert!(report.removed.is_empty());
            assert_eq!(report.kept.len(), 1);
            assert_eq!(fs::read(fixture.old.join("worker.py")).unwrap(), b"worker");
            assert!(fixture.old.join("runtime/library.py").exists());
        }
    }

    #[test]
    fn in_use_file_preserves_whole_runtime_until_next_attempt() {
        let fixture = Fixture::new();
        let held = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(fixture.old.join("runtime/library.py"))
            .unwrap();
        assert_eq!(fixture.clean().kept.len(), 1);
        assert!(fixture.old.join("worker.py").exists());
        drop(held);
        assert_eq!(fixture.clean().removed.len(), 1);
    }

    #[test]
    fn loaded_dll_preserves_whole_tree_even_after_loader_closed_file_handles() {
        // DLL loader狀態是程序共用的；WIC／COM平行測試也會載入系統DLL。
        // 使用獨立測試程序，並核對實際載入的路徑，避免誤測另一個已載入模組。
        const CHILD: &str = "LM_AI_DLL_CLEANUP_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "cad::cleanup::tests::loaded_dll_preserves_whole_tree_even_after_loader_closed_file_handles", "--nocapture"])
                .env(CHILD, "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        use windows_sys::Win32::{
            Foundation::FreeLibrary,
            System::LibraryLoader::{
                GetModuleFileNameW, LoadLibraryExW, DONT_RESOLVE_DLL_REFERENCES,
            },
        };
        let system =
            PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/version.dll");
        let fixture = Fixture::with_library(&fs::read(system).unwrap());
        // 載入測試副本的PE image，不呼叫DllMain或解析匯入；不修改Windows原檔。
        let module = unsafe {
            LoadLibraryExW(
                crate::wide(&fixture.old.join("runtime/library.py").to_string_lossy()).as_ptr(),
                std::ptr::null_mut(),
                DONT_RESOLVE_DLL_REFERENCES,
            )
        };
        assert!(!module.is_null());
        let mut name = vec![0u16; 32768];
        let length = unsafe { GetModuleFileNameW(module, name.as_mut_ptr(), name.len() as u32) };
        assert!(length > 0 && (length as usize) < name.len());
        let actual = PathBuf::from(String::from_utf16(&name[..length as usize]).unwrap());
        assert_eq!(
            fs::canonicalize(actual).unwrap(),
            fs::canonicalize(fixture.old.join("runtime/library.py")).unwrap()
        );
        let report = fixture.clean();
        unsafe {
            FreeLibrary(module);
        }
        assert_eq!(report.kept.len(), 1);
        assert!(report.removed.is_empty());
        assert_eq!(fs::read(fixture.old.join("worker.py")).unwrap(), b"worker");
        assert_eq!(fixture.clean().removed.len(), 1);
    }

    #[test]
    fn linked_file_and_external_source_are_preserved() {
        let fixture = Fixture::new();
        let external = fixture.root.join("external.py");
        fs::write(&external, b"library").unwrap();
        fs::remove_file(fixture.old.join("runtime/library.py")).unwrap();
        fs::hard_link(&external, fixture.old.join("runtime/library.py")).unwrap();
        assert_eq!(fixture.clean().kept.len(), 1);
        assert_eq!(fs::read(&external).unwrap(), b"library");
        assert!(fixture.old.join("worker.py").exists());
    }

    #[test]
    fn interrupted_cleanup_can_remove_remaining_known_files() {
        let fixture = Fixture::new();
        fs::remove_file(fixture.old.join("worker.py")).unwrap();
        assert_eq!(fixture.clean().removed.len(), 1);
    }

    #[test]
    fn junction_is_rejected_without_touching_its_target() {
        let fixture = Fixture::new();
        let target = fixture.root.join("external-directory");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("library.py"), b"library").unwrap();
        let junction_path = fixture.old.join("runtime");
        fs::remove_file(junction_path.join("library.py")).unwrap();
        let handle = OpenOptions::new()
            .access_mode(windows_sys::Win32::Foundation::GENERIC_WRITE)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(&junction_path)
            .unwrap();
        let substitute: Vec<u16> = format!("\\??\\{}", target.display())
            .encode_utf16()
            .collect();
        let display: Vec<u16> = target.to_string_lossy().encode_utf16().collect();
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&0xa0000003u32.to_le_bytes());
        buffer.extend_from_slice(
            &((8 + (substitute.len() + display.len() + 2) * 2) as u16).to_le_bytes(),
        );
        for value in [
            0,
            0,
            (substitute.len() * 2) as u16,
            ((substitute.len() + 1) * 2) as u16,
            (display.len() * 2) as u16,
        ] {
            buffer.extend_from_slice(&value.to_le_bytes());
        }
        for value in substitute.into_iter().chain([0]).chain(display).chain([0]) {
            buffer.extend_from_slice(&value.to_le_bytes());
        }
        let mut returned = 0;
        let result = unsafe {
            windows_sys::Win32::System::IO::DeviceIoControl(
                handle.as_raw_handle(),
                0x000900a4,
                buffer.as_ptr().cast(),
                buffer.len() as u32,
                std::ptr::null_mut(),
                0,
                &mut returned,
                std::ptr::null_mut(),
            )
        };
        assert_ne!(
            result,
            0,
            "建立Junction反例失敗：{}",
            std::io::Error::last_os_error()
        );
        drop(handle);
        assert_eq!(fixture.clean().kept.len(), 1);
        assert_eq!(fs::read(target.join("library.py")).unwrap(), b"library");
        assert!(fixture.old.join("worker.py").exists());
        fs::remove_dir(junction_path).unwrap();
    }
}
