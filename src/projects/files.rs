//! 固定檔案 broker：只接受專案相對路徑；逐層鎖住目錄、拒絕重新解析點與硬連結。
//! 工作副本先留在記憶體，發布只用 create_new；原始文件從未取得可寫 handle。
use super::{
    office,
    sandbox::{Edit, Worker},
    text::{self, Encoding},
    Project, Tool,
};
use crate::AppResult;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, Write},
    os::windows::{
        fs::{MetadataExt, OpenOptionsExt},
        io::AsRawHandle,
    },
    path::{Component, Path, PathBuf},
    sync::atomic::AtomicBool,
};
use windows_sys::Win32::Storage::FileSystem::*;

/// 回傳的 handle 保持到操作結束，拒絕目錄本身的寫入／刪除共用，
/// 避免檢查後被重新命名、替換或改成 Junction；仍可在目錄內建立成果。
pub(super) fn pin(path: &Path) -> AppResult<Vec<File>> {
    let mut current = PathBuf::new();
    let mut guards = Vec::new();
    for part in path.components() {
        match part {
            Component::Prefix(prefix) if matches!(prefix.kind(), std::path::Prefix::Disk(_)) => {
                current.push(part.as_os_str())
            }
            Component::RootDir => {
                current.push(part.as_os_str());
                // DRIVE_REMOTE=4；磁碟代號映射到網路分享亦拒絕，不只檢查 UNC 字串。
                if unsafe { GetDriveTypeW(crate::wide(&current.to_string_lossy()).as_ptr()) } == 4 {
                    return Err("第一版不允許網路映射磁碟。".into());
                }
            }
            Component::Normal(_) => {
                current.push(part.as_os_str());
                let file = OpenOptions::new()
                    .read(true)
                    .share_mode(FILE_SHARE_READ)
                    .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
                    .open(&current)
                    .map_err(|_| "無法開啟專案路徑；請確認資料夾存在且有權限。")?;
                if file
                    .metadata()
                    .map_err(|e| e.to_string())?
                    .file_attributes()
                    & FILE_ATTRIBUTE_REPARSE_POINT
                    != 0
                {
                    return Err("第一版不允許 Junction、符號連結或其他重新解析點。".into());
                }
                guards.push(file);
            }
            _ => {
                return Err(
                    "只支援本機磁碟的完整資料夾路徑，不接受 UNC、裝置路徑或上層跳轉。".into(),
                )
            }
        }
    }
    Ok(guards)
}

/// 以 Windows 正規化後的 handle 路徑排除私有資料；8.3 別名亦不可繞過名稱檢查。
pub(super) fn reject_internal(file: &File) -> AppResult<()> {
    let mut name = vec![0u16; 32768];
    let count = unsafe {
        GetFinalPathNameByHandleW(
            file.as_raw_handle(),
            name.as_mut_ptr(),
            name.len() as u32,
            0,
        )
    } as usize;
    if count == 0 || count >= name.len() {
        return Err("無法核對文件的實際位置。".into());
    }
    if String::from_utf16_lossy(&name[..count])
        .split(['\\', '/'])
        .any(|p| p.eq_ignore_ascii_case(".lmai"))
    {
        return Err(".lmai 是專案內部資料，不可當成一般文件讀取。".into());
    }
    Ok(())
}

pub fn validate_root(root: &Path) -> AppResult<()> {
    if root.components().any(|c| {
        c.as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(".lmai")
    }) || !root.is_absolute()
        || root
            .components()
            .filter(|p| matches!(p, Component::Normal(_)))
            .count()
            == 0
    {
        return Err("請選擇本機的專案資料夾，不可授權整個磁碟。".into());
    }
    let _guards = pin(root)?;
    if let Some(directory) = _guards.last() {
        reject_internal(directory)?;
    }
    if !root.is_dir() {
        return Err("專案根目錄必須是資料夾。".into());
    }
    Ok(())
}

pub fn relative(value: &str) -> AppResult<PathBuf> {
    if value.len() > 1000 || value.contains([':', '\0']) || value.starts_with(['/', '\\']) {
        return Err("只接受專案內的相對路徑。".into());
    }
    let path = Path::new(value);
    for component in path.components() {
        match component {
            Component::Normal(name) => {
                let name = name.to_str().ok_or("路徑文字無法辨識。")?;
                let base = name.split('.').next().unwrap_or("").to_ascii_uppercase();
                if name.eq_ignore_ascii_case(".lmai")
                    || name.ends_with(['.', ' '])
                    || name.contains(['<', '>', '"', '|', '?', '*'])
                    || name.chars().any(char::is_control)
                    || matches!(
                        base.as_str(),
                        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
                    )
                    || ((base.starts_with("COM") || base.starts_with("LPT"))
                        && base.len() == 4
                        && base.as_bytes()[3].is_ascii_digit())
                {
                    return Err("檔名包含 Windows 保留名稱或特殊字元。".into());
                }
            }
            _ => return Err("不可使用絕對路徑或 .. 跳出專案。".into()),
        }
    }
    // 由元件重建 Windows 路徑，避免 Shell 收到混用 / 與 \ 的字串。
    Ok(path.components().collect())
}
fn extension(path: &Path) -> AppResult<String> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(ext.as_str(), "txt" | "md" | "pdf" | "msg") && !office::supported(path) {
        return Err("支援 TXT、MD、PDF、MSG 及 Word／Excel／PowerPoint 文件。".into());
    }
    Ok(ext)
}

pub fn read(project: &Project, path: &str) -> AppResult<(String, Encoding)> {
    read_cancel(project, path, &AtomicBool::new(false), None, None)
}
fn read_cancel(
    project: &Project,
    path: &str,
    cancel: &AtomicBool,
    worker: Option<&mut Worker>,
    server_pdf: Option<&mut super::server_pdf::Reader>,
) -> AppResult<(String, Encoding)> {
    let rel = relative(path)?;
    let ext = extension(&rel)?;
    let target = project.root.join(&rel);
    let _guards = pin(target.parent().ok_or("缺少來源資料夾。")?)?;
    let mut file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(&target)
        .map_err(|e| super::interaction::open_error(&target, e))?;
    reject_internal(&file)?;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0
        || info.dwFileAttributes & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY) != 0
        || info.nNumberOfLinks != 1
    {
        return Err("拒絕讀取連結、目錄或無法驗證的檔案。".into());
    }
    if office::supported(&rel) {
        if file.metadata().map_err(|e| e.to_string())?.len() > 50_000_000 {
            return Err("Office 檔案上限為 50 MB。".into());
        }
        let snapshot = office::process(&target, None, None, None, cancel)?;
        return Ok((snapshot.serialize()?, Encoding::Utf8(true)));
    }
    // 匯入文字是使用者本次明確提供的快照；不宣稱即時同步原檔。
    let key = rel.to_string_lossy().replace('\\', "/");
    if let Some(text) = project.imports.get(&key) {
        text::validate(text)?;
        return Ok((text.clone(), Encoding::Utf8(true)));
    }
    if ext == "pdf" {
        if let Some(reader) = server_pdf {
            let name = rel
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or("PDF 檔名無效。")?;
            return reader
                .read(&mut file, name, cancel)
                .map(|text| (text, Encoding::Utf8(true)));
        }
    }
    if ext == "pdf" || ext == "msg" {
        let limit = if ext == "pdf" {
            super::pdf::MAX_PDF as u64
        } else {
            50_000_000
        };
        if file.metadata().map_err(|e| e.to_string())?.len() > limit {
            return Err(format!("{ext} 檔案超過 {} MB 上限。", limit / 1_000_000));
        }
        let content = if ext == "msg" {
            let (mut content, warning) = with_reader_copy(project, &mut file, "msg", |copy| {
                crate::outlook::msg::read(copy, cancel)
            })?;
            if let Some(warning) = warning {
                content.push_str(&format!("\n\n[讀取注意：{warning}]"));
            }
            text::validate(&content)?;
            content
        } else {
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            if let Some(worker) = worker {
                worker.extract_pdf(&bytes, cancel)?
            } else {
                let exe = std::env::current_exe().map_err(|e| e.to_string())?;
                Worker::start(&exe, cancel)?.extract_pdf(&bytes, cancel)?
            }
        };
        return Ok((content, Encoding::Utf8(true)));
    }
    if file.metadata().map_err(|e| e.to_string())?.len() > text::MAX_TEXT as u64 {
        return Err("文件超過第一版 200 KB 上限。".into());
    }
    let mut data = Vec::new();
    file.read_to_end(&mut data).map_err(|e| e.to_string())?;
    text::decode(&data)
}

/// 用獨立暫存副本協調 Outlook 的開檔需求，原件仍維持拒絕寫入的 handle。
/// 不放寬來源的共享模式；副本保持讀取 handle、不分享 DELETE，避免開檔前被替換。
fn with_reader_copy<T>(
    project: &Project,
    source: &mut File,
    ext: &str,
    read: impl FnOnce(&Path) -> AppResult<T>,
) -> AppResult<(T, Option<String>)> {
    let _root = pin(&project.root)?;
    let base = project.root.join("_AI_Output");
    match fs::create_dir(&base) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(format!("無法建立閱讀暫存目錄：{e}")),
    }
    let _base = pin(&base)?;
    let stage = base.join(format!(".read_{}", crate::jobs::new_id()?));
    fs::create_dir(&stage).map_err(|e| e.to_string())?;
    let guard = pin_stage(&stage)?;
    let copy = stage.join(format!("source.{ext}"));
    let result = (|| {
        let mut output = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .open(&copy)
            .map_err(|e| e.to_string())?;
        source.rewind().map_err(|e| e.to_string())?;
        std::io::copy(source, &mut output).map_err(|e| e.to_string())?;
        output.sync_all().map_err(|e| e.to_string())?;
        let _held = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(&copy)
            .map_err(|e| e.to_string())?;
        drop(output);
        read(&copy)
    })();
    // Outlook 可能暫留 MSG handle；只清除本次建立的檔案，不遞迴刪除其他內容。
    let cleanup = fs::remove_file(&copy);
    drop(guard);
    let folder_cleanup = fs::remove_dir(&stage);
    let warning = (cleanup.is_err() || folder_cleanup.is_err()).then(|| {
        format!(
            "閱讀暫存尚未能清除：{}。請關閉該信件後清理此暫存資料夾；這不是交付成果。",
            stage.display()
        )
    });
    match result {
        Ok(content) => Ok((content, warning)),
        Err(error) => Err(match warning {
            Some(warning) => format!("{error}\n{warning}"),
            None => error,
        }),
    }
}

/// 使用本機時間命名；create_dir 本身決定是否撞名，同秒任務不共用目錄。
fn create_output_folder(base: &Path) -> AppResult<String> {
    let mut time = windows_sys::Win32::Foundation::SYSTEMTIME::default();
    unsafe { windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut time) };
    let stamp = format!(
        "{:04}{:02}{:02}_{:02}{:02}{:02}",
        time.wYear, time.wMonth, time.wDay, time.wHour, time.wMinute, time.wSecond
    );
    for number in 1..=10000 {
        let name = if number == 1 {
            stamp.clone()
        } else {
            format!("{stamp}_{number}")
        };
        match fs::create_dir(base.join(&name)) {
            Ok(()) => return Ok(name),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("同秒輸出資料夾過多。".into())
}

/// 檔名保留來源語意，只在撞名時加序號；以 create_new 防止覆寫既有成果。
fn reserve_output(folder: &Path, name: &str) -> AppResult<(String, File)> {
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("檔名無效。")?;
    // PNG／CSV 僅由專用匯出流程建立，不擴大一般文件編輯的格式。
    let ext = match path.extension().and_then(|s| s.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("png") || ext.eq_ignore_ascii_case("csv") => {
            ext.to_lowercase()
        }
        _ => extension(path)?,
    };
    for number in 1..=10000 {
        let candidate = if number == 1 {
            name.to_owned()
        } else {
            format!("{stem}_{number}.{ext}")
        };
        match OpenOptions::new()
            .write(true)
            .read(true)
            .create_new(true)
            .share_mode(0)
            .open(folder.join(&candidate))
        {
            Ok(file) => return Ok((candidate, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("同名成果過多，請更換檔名。".into())
}

/// 僅在使用者點擊後開啟 Explorer 並選取成果；不執行檔案或交給關聯程式。
pub fn reveal(project: &Project, value: &str) -> AppResult<()> {
    let rel = relative(value)?;
    if rel.components().next().map(|p| p.as_os_str()) != Some(std::ffi::OsStr::new("_AI_Output")) {
        return Err("只能定位專案成果資料夾內的檔案。".into());
    }
    let target = project.root.join(rel);
    let _guards = pin(target.parent().ok_or("缺少成果目錄。")?)?;
    let file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(&target)
        .map_err(|_| "成果已被移動、刪除或無法存取。")?;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0
        || info.dwFileAttributes & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY) != 0
        || info.nNumberOfLinks != 1
    {
        return Err("拒絕定位連結或無法驗證的成果。".into());
    }
    use windows::{
        core::PCWSTR,
        Win32::{
            System::Com::CoTaskMemFree,
            UI::Shell::{SHOpenFolderAndSelectItems, SHParseDisplayName},
        },
    };
    let selected = unsafe {
        let mut item = std::ptr::null_mut();
        match SHParseDisplayName(
            PCWSTR(crate::wide(&target.to_string_lossy()).as_ptr()),
            None,
            &mut item,
            0,
            None,
        ) {
            Ok(()) => {
                let result = SHOpenFolderAndSelectItems(item, None, 0);
                CoTaskMemFree(Some(item.cast()));
                result
            }
            Err(error) => Err(error),
        }
    };
    if selected.is_ok() {
        return Ok(());
    }
    // 某些企業 Shell 未註冊定位介面。直接啟動系統 Explorer，不依赖檔案關聯或任意命令。
    let mut windows_dir = [0u16; 32768];
    let length = unsafe {
        windows_sys::Win32::System::SystemInformation::GetWindowsDirectoryW(
            windows_dir.as_mut_ptr(),
            windows_dir.len() as u32,
        )
    } as usize;
    if length == 0 || length >= windows_dir.len() {
        return Err("無法取得 Windows 目錄。".into());
    }
    let explorer =
        PathBuf::from(String::from_utf16_lossy(&windows_dir[..length])).join("explorer.exe");
    use std::os::windows::process::CommandExt;
    std::process::Command::new(explorer)
        .raw_arg(format!("/select,\"{}\"", target.display()))
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("無法開啟檔案總管：{e}；成果位置：{}", target.display()))
}

/// 固定開啟方式同時保護原檔、發布暫存，拒絕連結及操作期間的替換。
pub(super) fn checked_file(path: &Path) -> AppResult<File> {
    let file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|e| super::interaction::open_error(path, e))?;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0
        || info.dwFileAttributes & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY) != 0
        || info.nNumberOfLinks != 1
    {
        return Err("拒絕讀取連結或無法驗證的文件。".into());
    }
    Ok(file)
}
/// Office 另存需要目錄寫入共用。先在嚴格目錄鎖下建立不分享存取的 anchor，
/// 讓目錄始終非空（Windows 不允許替非空目錄設定 reparse point），並防止過渡期間更名。
/// 再以 DELETE 存取權、不分享 DELETE 的 handle 固定目錄；不改變一般專案目錄的鎖。
pub(super) struct StageGuard {
    directory: Option<File>,
    anchor: Option<File>,
    anchor_path: PathBuf,
}
impl Drop for StageGuard {
    fn drop(&mut self) {
        self.anchor.take();
        let _ = fs::remove_file(&self.anchor_path);
        self.directory.take();
    }
}
fn pin_stage(path: &Path) -> AppResult<StageGuard> {
    pin_named_stage(path, ".anchor")
}

/// 私有目錄跨啟動存在，使用唯一 anchor，避免上次崩潰殘留的 anchor 阻擋之後存取。
pub(super) fn pin_memory_directory(path: &Path) -> AppResult<StageGuard> {
    pin_named_stage(path, &format!(".anchor_{}", crate::jobs::new_id()?))
}
fn pin_named_stage(path: &Path, anchor_name: &str) -> AppResult<StageGuard> {
    const DELETE_ACCESS: u32 = 0x0001_0000;
    let initial = pin(path)?;
    let anchor_path = path.join(anchor_name);
    let anchor = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .share_mode(0)
        .open(&anchor_path)
        .map_err(|e| e.to_string())?;
    drop(initial);
    let directory = OpenOptions::new()
        .access_mode(DELETE_ACCESS)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|e| e.to_string())?;
    let attrs = directory
        .metadata()
        .map_err(|e| e.to_string())?
        .file_attributes();
    if attrs & FILE_ATTRIBUTE_REPARSE_POINT != 0 || attrs & FILE_ATTRIBUTE_DIRECTORY == 0 {
        return Err("Office 暫存目錄不是正常資料夾。".into());
    }
    Ok(StageGuard {
        directory: Some(directory),
        anchor: Some(anchor),
        anchor_path,
    })
}

pub(super) fn fingerprint(project: &Project, source: &str) -> AppResult<String> {
    use sha2::{Digest, Sha256};
    let target = project.root.join(relative(source)?);
    let _pins = pin(target.parent().ok_or("缺少來源資料夾。")?)?;
    let mut file = checked_file(&target)?;
    reject_internal(&file)?;
    if file.metadata().map_err(|e| e.to_string())?.len() > 52_428_800 {
        return Err("來源檔案超過 50 MiB 上限。".into());
    }
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let size = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if size == 0 {
            break;
        }
        hash.update(&buffer[..size]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct OfficeCopy {
    source: Option<String>,
    fingerprint: String,
    original: Option<office::Snapshot>,
    actions: Vec<office::Action>,
    desired: office::Snapshot,
}

/// 在同一組目錄／檔案鎖下驗證 PNG 並計算版本；鎖需保持至 Office 嵌入完成。
fn locked_image(project: &Project, value: &str) -> AppResult<(PathBuf, Vec<File>, String)> {
    let rel = relative(value)?;
    if !rel
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("png"))
    {
        return Err("圖片插入目前只接受專案內的 PNG。".into());
    }
    let path = project.root.join(rel);
    let mut locks = pin(path.parent().ok_or("缺少圖片目錄。")?)?;
    let mut file = checked_file(&path)?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take((super::charts::png::MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    super::charts::png::dimensions(&bytes)?;
    use sha2::{Digest, Sha256};
    let hash = format!("{:x}", Sha256::digest(&bytes));
    locks.push(file);
    Ok((path, locks, hash))
}
/// 原件的路徑、內容與格式均鎖定核對；空白文件不需要假造來源檔案。
fn render_office(
    project: &Project,
    name: &str,
    copy: &OfficeCopy,
    output: Option<&Path>,
    cancel: &AtomicBool,
) -> AppResult<office::Snapshot> {
    let source = copy
        .source
        .as_ref()
        .map(|s| relative(s).map(|p| project.root.join(p)))
        .transpose()?;
    let _dirs = source
        .as_ref()
        .map(|p| pin(p.parent().ok_or("缺少來源目錄。")?))
        .transpose()?;
    let _file = source.as_ref().map(|p| checked_file(p)).transpose()?;
    if let Some(path) = &copy.source {
        if fingerprint(project, path)? != copy.fingerprint {
            return Err("Office 原檔已變動，請重新建立副本。".into());
        }
    }
    let mut actions = copy.actions.clone();
    let mut image_locks = Vec::new();
    for action in &mut actions {
        if let office::Action::InsertImage { path, sha256, .. } = action {
            let (absolute, locks, hash) = locked_image(project, path)?;
            if sha256.as_deref() != Some(hash.as_str()) {
                return Err("圖片來源已變更，請重新建立工作副本，避免替換已確認的圖像。".into());
            }
            image_locks.extend(locks);
            *path = absolute.to_string_lossy().into_owned();
        }
    }
    office::render(
        source.as_deref(),
        Path::new(name),
        copy.original.as_ref(),
        &actions,
        output,
        cancel,
    )
}
#[derive(serde::Serialize, serde::Deserialize)]
struct Copy {
    office: Option<OfficeCopy>,
    name: String,
    text: String,
    encoding: Encoding,
    saved_revision: Option<String>,
    paths: Vec<String>,
}
#[derive(serde::Serialize, serde::Deserialize)]
struct ChartExport {
    chart_index: usize,
    name: String,
    path: String,
    sha256: String,
}
/// 只保存資料，不保存授權、Token、COM 物件或執行中的程序。
#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct SavedBroker {
    #[serde(default)]
    outlook: super::mail::Saved,
    #[serde(default)]
    log_cursors: BTreeMap<String, super::logs::Cursor>,
    output_folder: Option<String>,
    copies: BTreeMap<String, Copy>,
    results: BTreeMap<String, (Value, Value)>,
    #[serde(default)]
    archived_results: BTreeMap<String, Value>,
    published: Vec<String>,
    txt_context: bool,
    #[serde(default)]
    loaded_skills: Vec<String>,
    #[serde(default)]
    charts: Vec<super::charts::Chart>,
    #[serde(default)]
    chart_exports: Vec<ChartExport>,
    #[serde(default)]
    datasets: Vec<super::datasets::Reference>,
}
pub struct Broker {
    file_waiter: Option<super::interaction::FileWaiter>,
    outlook: super::mail::Session,
    log_cursors: BTreeMap<String, super::logs::Cursor>,
    server_pdf: Option<super::server_pdf::Reader>,
    memory: Option<super::memory::Memory>,
    project: Project,
    output_folder: Option<String>,
    copies: BTreeMap<String, Copy>,
    /// 同一 operation_id 只能配對同一份工具參數，重送僅回傳已記錄的結果。
    results: BTreeMap<String, (Value, Value)>,
    archived_results: BTreeMap<String, Value>,
    published: Vec<String>,
    /// 任務接觸 TXT 後，不允許把內容混入未加密的 MD 成果。
    txt_context: bool,
    loaded_skills: Vec<String>,
    charts: Vec<super::charts::Chart>,
    chart_exports: Vec<ChartExport>,
    datasets: Vec<super::datasets::Reference>,
    png_renderer: Option<super::charts::png::Renderer>,
    chart_chooser: Option<super::charts::quality::Chooser>,
    chart_deadline: std::time::Instant,
}
impl Broker {
    pub fn new(project: Project, _task: String) -> AppResult<Self> {
        validate_root(&project.root)?;
        Ok(Self {
            file_waiter: None,
            outlook: super::mail::Session::default(),
            log_cursors: BTreeMap::new(),
            server_pdf: None,
            memory: None,
            project,
            output_folder: None,
            copies: BTreeMap::new(),
            results: BTreeMap::new(),
            archived_results: BTreeMap::new(),
            published: Vec::new(),
            txt_context: false,
            loaded_skills: vec![],
            charts: vec![],
            chart_exports: vec![],
            datasets: vec![],
            png_renderer: None,
            chart_chooser: None,
            chart_deadline: std::time::Instant::now(),
        })
    }
    pub(super) fn saved(&self) -> AppResult<SavedBroker> {
        // 經序列化建立不含程序資源的快照；授權與執行中的 COM 物件不保存。
        serde_json::from_value(
            json!({"outlook":self.outlook.saved,"log_cursors":self.log_cursors,"output_folder":self.output_folder,"copies":self.copies,
            "results":self.results,"archived_results":self.archived_results,"published":self.published,"txt_context":self.txt_context,"loaded_skills":self.loaded_skills,"charts":self.charts,"chart_exports":self.chart_exports,"datasets":self.datasets}),
        )
        .map_err(|e| e.to_string())
    }
    pub(super) fn restore(&mut self, state: SavedBroker, cancel: &AtomicBool) -> AppResult<()> {
        self.log_cursors = state.log_cursors;
        self.outlook.saved = state.outlook;
        if state.copies.len() > 20 {
            return Err("暫存工作副本數不合法。".into());
        }
        if let Some(folder) = &state.output_folder {
            if relative(folder)?.components().count() != 1 {
                return Err("暫存輸出目錄不合法。".into());
            }
            let _guards = pin(&self.project.root.join("_AI_Output").join(folder))?;
        }
        for (id, copy) in &state.copies {
            crate::jobs::validate_id(id)?;
            if relative(&copy.name)?.components().count() != 1 {
                return Err("暫存檔名不合法。".into());
            }
            if let Some(office) = &copy.office {
                if let Some(source) = &office.source {
                    if fingerprint(&self.project, source)? != office.fingerprint {
                        return Err(format!(
                            "來源 {source} 已變更，未恢復舊版修改；請重新提出任務。"
                        ));
                    }
                }
                if office.desired.serialize()? != copy.text {
                    return Err("暫存 Office 版本不一致。".into());
                }
            }
            if copy.saved_revision.as_deref() == Some(text::revision(&copy.text).as_str()) {
                let path = copy.paths.last().ok_or("暫存成果缺少路徑。")?;
                if read_cancel(&self.project, path, cancel, None, None)?.0 != copy.text {
                    return Err(format!("成果 {path} 已變更，未繼續舊任務。"));
                }
            }
        }
        super::skills::context(&state.loaded_skills)?;
        for chart in &state.charts {
            chart.validate()?;
        }
        for export in &state.chart_exports {
            self.verify_chart_export(export)?;
        }
        for dataset in &state.datasets {
            super::datasets::load(&self.project, &dataset.path, &dataset.revision, cancel)?;
        }
        self.datasets = state.datasets;
        self.chart_exports = state.chart_exports;
        self.loaded_skills.clear();
        for id in state.loaded_skills {
            super::skills::activate(&mut self.loaded_skills, &id)?;
        }
        self.charts = state.charts;
        self.output_folder = state.output_folder;
        self.copies = state.copies;
        self.results = state.results;
        self.archived_results = state.archived_results;
        self.published = state.published;
        self.txt_context = state.txt_context;
        Ok(())
    }
    /// 只有專案代理啟用跨任務記憶；一般聊天沒有此能力。
    pub fn enable_memory(&mut self, conversation: &str) -> AppResult<()> {
        self.memory = Some(super::memory::Memory::open(
            self.project.clone(),
            conversation,
        )?);
        self.txt_context |= self.memory()?.has_protected_documents()?;
        Ok(())
    }
    pub fn memory(&self) -> AppResult<&super::memory::Memory> {
        self.memory
            .as_ref()
            .ok_or("本次任務未啟用專案記憶。".into())
    }
    /// 正式代理任務啟用伺服器 PDF；本機診斷測試仍可使用原生解析器。
    pub fn enable_server_pdf(
        &mut self,
        config: crate::config::Config,
        session: crate::storage::Session,
    ) -> AppResult<()> {
        self.server_pdf = Some(super::server_pdf::Reader::new(
            config,
            session,
            self.project.root.clone(),
        )?);
        Ok(())
    }
    /// 續接只能引用實際存在的工作副本與版本，不使用 AI 筆記重建檔案狀態。
    pub fn progress_snapshot(&self) -> Value {
        json!(self
            .copies
            .iter()
            .map(|(id, copy)| json!({
                "copy_id":id,"name":copy.name,"revision":text::revision(&copy.text),
                "saved_revision":copy.saved_revision,"paths":copy.paths
            }))
            .collect::<Vec<_>>())
    }
    /// 可重用資料集只附路徑與結構，不在續接快照重送所有資料列。
    pub(super) fn dataset_index(&self) -> Value {
        json!(self.datasets)
    }
    pub(super) fn skill_context(&self) -> AppResult<String> {
        super::skills::context(&self.loaded_skills)
    }
    pub fn charts(&self) -> &[super::charts::Chart] {
        &self.charts
    }
    /// 新一輪只公告目前有實際操作對象的工具；不改寫已提交／待查回的請求。
    pub(super) fn restrict_tools(&self, request: &mut Value) {
        if let Some(tools) = request["tools"].as_array_mut() {
            tools.retain(|tool| {
                let name = tool["function"]["name"].as_str().unwrap_or("");
                if !super::skills::enabled(name, &self.loaded_skills) {
                    return false;
                }
                match name {
                    "export_chart_png" => !self.charts.is_empty(),
                    "edit_text" => self.copies.values().any(|c| c.office.is_none()),
                    "edit_office" | "office_action" | "office_batch" => {
                        self.copies.values().any(|c| c.office.is_some())
                    }
                    "save_copy" | "delete_copy" => !self.copies.is_empty(),
                    _ => true,
                }
            });
        }
    }
    pub fn set_png_renderer(&mut self, renderer: super::charts::png::Renderer) {
        self.png_renderer = Some(renderer);
    }
    pub(super) fn set_outlook_root(&mut self, root: &Path) {
        self.outlook.policy_root = Some(root.into());
    }
    pub fn set_outlook_consent(&mut self, consent: Option<super::mail::Consent>) {
        self.outlook.set_consent(consent);
    }
    /// 恢復模型歷史前先重新授權；拒絕或範圍改變時不能先送出舊郵件結果。
    pub(super) fn reauthorize_outlook(
        &mut self,
        cancel: &AtomicBool,
        deadline: std::time::Instant,
    ) -> AppResult<()> {
        if self.outlook.has_snapshot() && !self.outlook.authorize(cancel, deadline)? {
            return Err("未確認 Outlook 範圍，未恢復舊任務。".into());
        }
        self.outlook.check_policy()
    }
    pub fn set_file_waiter(&mut self, waiter: Option<super::interaction::FileWaiter>) {
        self.file_waiter = waiter;
    }
    fn wait_for_file(&mut self, error: &str, cancel: &AtomicBool) -> AppResult<bool> {
        match self.file_waiter.as_mut() {
            Some(waiter) => waiter(
                &error.replace(super::interaction::BUSY, ""),
                cancel,
                self.chart_deadline,
            ),
            None => Ok(false),
        }
    }
    pub fn set_chart_chooser(
        &mut self,
        chooser: Option<super::charts::quality::Chooser>,
        deadline: std::time::Instant,
    ) {
        self.chart_chooser = chooser;
        self.chart_deadline = deadline;
    }
    fn review_chart(
        &mut self,
        prepared: super::charts::quality::Prepared,
        cancel: &AtomicBool,
    ) -> AppResult<Option<super::charts::Chart>> {
        let choices = if prepared.review.groups.is_empty() {
            vec![]
        } else {
            let Some(chooser) = self.chart_chooser.as_mut() else {
                return Ok(None);
            };
            let Some(choices) = chooser(&prepared.review, cancel, self.chart_deadline)? else {
                return Ok(None);
            };
            choices
        };
        prepared.apply(&choices).map(Some)
    }
    fn add_chart(&mut self, chart: super::charts::Chart) -> AppResult<Value> {
        chart.validate()?;
        if self.charts.len() >= 12 {
            return Err("每次任務最多 12 張圖表。".into());
        }
        if serde_json::to_vec(&self.charts)
            .map_err(|e| e.to_string())?
            .len()
            + serde_json::to_vec(&chart).map_err(|e| e.to_string())?.len()
            > 16 * 1024 * 1024
        {
            return Err("本次任務的圖表資料合計超過 16 MiB，請分成另一個任務。".into());
        }
        let data_note = chart.data_note.clone();
        self.charts.push(chart);
        Ok(
            json!({"chart_index":self.charts.len()-1,"displayed_in_conversation":true,"data_note":data_note}),
        )
    }
    fn verify_chart_export(&self, export: &ChartExport) -> AppResult<()> {
        let path = relative(&export.path)?;
        if path.components().count() != 3
            || !path.starts_with("_AI_Output")
            || !path
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.eq_ignore_ascii_case("png"))
        {
            return Err("PNG 成果路徑不合法。".into());
        }
        let target = self.project.root.join(path);
        let _guards = pin(target.parent().ok_or("PNG 目錄不存在。")?)?;
        let mut file = checked_file(&target)?;
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take((super::charts::png::MAX_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        super::charts::png::validate(&bytes)?;
        use sha2::{Digest, Sha256};
        if format!("{:x}", Sha256::digest(&bytes)) != export.sha256 {
            return Err("PNG 成果已變更，請確認後重新匯出。".into());
        }
        Ok(())
    }

    /// CSV 只建立新成果，讀回核對成功才加入可引用索引；不覆寫原始資料。
    fn save_dataset(
        &mut self,
        name: &str,
        table: &super::datasets::Table,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        let rel = relative(name)?;
        if rel.components().count() != 1
            || rel
                .extension()
                .is_none_or(|s| !s.eq_ignore_ascii_case("csv"))
        {
            return Err("資料集名稱需為單一 CSV 檔名。".into());
        }
        if self.datasets.len() >= 30 {
            return Err("每個任務最多 30 份 CSV 資料集。".into());
        }
        let csv = table.csv()?;
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("CSV 匯出已取消。".into());
        }
        let _root = pin(&self.project.root)?;
        let base = self.project.root.join("_AI_Output");
        match fs::create_dir(&base) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e.to_string()),
        }
        let _base = pin(&base)?;
        if self.output_folder.is_none() {
            self.output_folder = Some(create_output_folder(&base)?);
        }
        let folder_name = self
            .output_folder
            .as_deref()
            .ok_or("CSV 輸出資料夾不存在。")?;
        let folder = base.join(folder_name);
        let _folder = pin(&folder)?;
        let (actual_name, mut file) = reserve_output(&folder, name)?;
        let path = format!("_AI_Output/{folder_name}/{actual_name}");
        self.published.push(path.clone());
        file.write_all(csv.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|e| format!("CSV 可能已部分寫入 {path}，尚未交付：{e}"))?;
        file.rewind().map_err(|e| e.to_string())?;
        let mut readback = String::new();
        Read::by_ref(&mut file)
            .take((super::datasets::MAX_BYTES + 1) as u64)
            .read_to_string(&mut readback)
            .map_err(|e| e.to_string())?;
        if readback != csv {
            return Err(format!("CSV {path} 讀回不一致，尚未交付。"));
        }
        let reference = super::datasets::Reference {
            path,
            revision: text::revision(&csv),
            rows: table.rows.len(),
            columns: table.columns.clone(),
        };
        let result = table.summary(&reference);
        self.datasets.push(reference);
        Ok(result)
    }

    fn export_chart_png(
        &mut self,
        index: usize,
        name: &str,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        let relative_name = relative(name)?;
        if relative_name.components().count() != 1
            || !relative_name
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.eq_ignore_ascii_case("png"))
        {
            return Err("請提供單一 PNG 檔名，例如「亮度趨勢.png」，不要包含資料夾。".into());
        }
        let chart = self
            .charts
            .get(index)
            .ok_or("找不到本次任務的圖表編號，請先建立圖表。")?;
        chart.validate()?;
        // 同圖、同檔名的再次要求沿用已驗證成果；任務暫停後亦保留此記錄。
        if let Some(export) = self
            .chart_exports
            .iter()
            .find(|e| e.chart_index == index && e.name == name)
        {
            self.verify_chart_export(export)?;
            return Ok(
                json!({"chart_index":index,"path":export.path,"verified":true,"reused":true}),
            );
        }
        let bytes = self
            .png_renderer
            .as_mut()
            .ok_or("目前沒有可用的桌面圖表匯出器。")?(chart, cancel)?;
        super::charts::png::validate(&bytes)?;
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("PNG 匯出已取消，未寫入檔案。".into());
        }
        let _root = pin(&self.project.root)?;
        let base = self.project.root.join("_AI_Output");
        match fs::create_dir(&base) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e.to_string()),
        }
        let _base = pin(&base)?;
        if self.output_folder.is_none() {
            self.output_folder = Some(create_output_folder(&base)?);
        }
        let folder_name = self
            .output_folder
            .as_deref()
            .ok_or("PNG 輸出資料夾不存在。")?;
        let folder = base.join(folder_name);
        let _folder = pin(&folder)?;
        let (actual_name, mut file) = reserve_output(&folder, name)?;
        let path = format!("_AI_Output/{folder_name}/{actual_name}");
        // 保留可能部分寫入的路徑；失敗時明確回報，不將檔案登記為成功匯出。
        self.published.push(path.clone());
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| format!("PNG 可能已部分寫入 {path}，尚未交付：{e}"))?;
        file.rewind().map_err(|e| e.to_string())?;
        let mut verified = Vec::new();
        Read::by_ref(&mut file)
            .take((super::charts::png::MAX_BYTES + 1) as u64)
            .read_to_end(&mut verified)
            .map_err(|e| e.to_string())?;
        if verified != bytes {
            return Err(format!("PNG {path} 讀回不一致，尚未交付。"));
        }
        use sha2::{Digest, Sha256};
        self.chart_exports.push(ChartExport {
            chart_index: index,
            name: name.into(),
            path: path.clone(),
            sha256: format!("{:x}", Sha256::digest(&bytes)),
        });
        Ok(
            json!({"chart_index":index,"path":path,"verified":true,"width":super::charts::png::WIDTH,"height":super::charts::png::HEIGHT}),
        )
    }
    /// 明確指定檔案，逐檔使用與 read_file 相同的存取邊界；不掃描未知目錄。
    fn search_files(
        &mut self,
        paths: &[String],
        query: &str,
        worker: &mut Worker,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        if paths.is_empty()
            || paths.len() > 20
            || query.trim().is_empty()
            || query.chars().count() > 200
        {
            return Err("搜尋需指定 1–20 份文件與 1–200 字查詢。".into());
        }
        let mut matches = Vec::new();
        let mut errors = Vec::new();
        let mut truncated = false;
        for path in paths {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err("操作已取消。".into());
            }
            match self.content(path, cancel, worker) {
                Ok(content) => {
                    let revision = text::revision(&content);
                    for (byte, _) in content.match_indices(query) {
                        if matches.len() >= 60 {
                            truncated = true;
                            break;
                        }
                        let offset = content[..byte].chars().count();
                        let start = offset.saturating_sub(100);
                        matches.push(json!({"path":path,"revision":revision,"offset":offset,"excerpt":content.chars().skip(start).take(300).collect::<String>()}));
                    }
                }
                Err(error) if error.contains(super::interaction::DEFERRED) => return Err(error),
                Err(error) => errors.push(json!({"path":path,"error":error})),
            }
        }
        Ok(
            json!({"matches":matches,"errors":errors,"truncated":truncated,"complete":errors.is_empty()&&!truncated}),
        )
    }
    pub fn published(&self) -> &[String] {
        &self.published
    }
    /// 每批結束先封存原文，再縮小快照。索引保留全部 ID，重送不會重做修改。
    pub(super) fn archive_results(&mut self) -> AppResult<()> {
        for (id, (request, result)) in &self.results {
            let hash = self
                .memory()?
                .archive_operation(&json!([request, result]))?;
            self.archived_results.insert(
                id.clone(),
                json!({"hash":hash,"tool":request["tool"],"ok":result["ok"]}),
            );
        }
        self.results.clear();
        Ok(())
    }
    fn recorded_operation(&self, id: &str) -> AppResult<Option<(Value, Value)>> {
        if let Some(pair) = self.results.get(id) {
            return Ok(Some(pair.clone()));
        }
        let Some(entry) = self.archived_results.get(id) else {
            return Ok(None);
        };
        let value = self
            .memory()?
            .archived_operation(entry["hash"].as_str().ok_or("操作索引無效。")?)?;
        serde_json::from_value(value)
            .map(Some)
            .map_err(|_| "操作原文格式無效。".into())
    }
    pub(super) fn operation_history(&self) -> AppResult<Value> {
        let mut rows = Vec::new();
        for id in self
            .archived_results
            .keys()
            .chain(self.results.keys())
            .collect::<std::collections::BTreeSet<_>>()
        {
            let (request, result) = self.recorded_operation(id)?.ok_or("操作紀錄遺失。")?;
            rows.push(json!({"operation_id":id,"request":request,"result":result}));
        }
        Ok(json!(rows))
    }
    /// 非同步委派也共用操作去重表；不能與一般工具重複使用不同參數的 ID。
    pub(super) fn cached_result(&self, id: &str, tool: &Tool) -> AppResult<Option<Value>> {
        self.outlook.check_policy()?;
        crate::jobs::validate_id(id)?;
        let request = serde_json::to_value(tool).map_err(|e| e.to_string())?;
        if let Some((previous, result)) = self.recorded_operation(id)? {
            if previous != request {
                return Err("操作識別碼重複但參數不同；已停止。".into());
            }
            if matches!(
                tool,
                Tool::OutlookFolders { .. }
                    | Tool::OutlookHeaders { .. }
                    | Tool::OutlookCompare { .. }
                    | Tool::OutlookRead { .. }
            ) && !self.outlook.is_allowed()
            {
                return Ok(None);
            }
            return Ok(Some(result.clone()));
        }
        Ok(None)
    }
    pub(super) fn remember_result(
        &mut self,
        id: &str,
        tool: &Tool,
        result: &Value,
    ) -> AppResult<()> {
        self.cached_result(id, tool)?;
        self.results.insert(
            id.into(),
            (
                serde_json::to_value(tool).map_err(|e| e.to_string())?,
                result.clone(),
            ),
        );
        Ok(())
    }
    pub fn execute(
        &mut self,
        id: &str,
        tool: &Tool,
        worker: &mut Worker,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        self.reauthorize_outlook(cancel, self.chart_deadline)?;
        crate::jobs::validate_id(id)?;
        // 連重播舊工具結果都先經本次同意；授權不隨 checkpoint 恢復。
        if matches!(
            tool,
            Tool::OutlookFolders { .. }
                | Tool::OutlookHeaders { .. }
                | Tool::OutlookCompare { .. }
                | Tool::OutlookRead { .. }
        ) && !self.outlook.authorize(cancel, self.chart_deadline)?
        {
            return Ok(
                json!({"ok":true,"result":{"declined":true,"executed":false,"message":"使用者未確認 Outlook 使用範圍；資料夾名稱僅在本機預覽，未讀取郵件或交給 AI。"}}),
            );
        }
        let request = serde_json::to_value(tool).map_err(|e| e.to_string())?;
        if let Some((old, result)) = self.recorded_operation(id)? {
            if old != request {
                return Err("操作識別碼重複但參數不同；已停止。".into());
            }
            return Ok(result.clone());
        }
        let outcome = loop {
            let result = self.perform(tool, worker, cancel);
            let Err(error) = &result else {
                break result;
            };
            let retryable = matches!(
                tool,
                Tool::ReadFile { .. }
                    | Tool::CreateWorkingCopy {
                        source: Some(_),
                        ..
                    }
                    | Tool::FindText { .. }
                    | Tool::SearchFiles { .. }
                    | Tool::ListDocumentSections { .. }
                    | Tool::ReadLog { .. }
                    | Tool::SearchLogs { .. }
                    | Tool::InspectExcel { .. }
                    | Tool::ReadExcelRange { .. }
                    | Tool::ChartExcelRange { .. }
                    | Tool::ExportExcelDataset { .. }
                    | Tool::ExportLogDataset { .. }
                    | Tool::InspectDataset { .. }
                    | Tool::ChartDataset { .. }
                    | Tool::ChartFromExcel { .. }
            );
            if error.contains(super::interaction::DEFERRED)
                || (retryable && error.contains(super::interaction::BUSY))
            {
                if !error.contains(super::interaction::DEFERRED)
                    && self.wait_for_file(error, cancel)?
                {
                    continue;
                }
                return Ok(
                    json!({"ok":true,"result":{"waiting_for_user":true,"wait_reason":"等待關閉占用檔案後繼續讀取","executed":false}}),
                );
            }
            break result;
        };
        let result = match outcome {
            Ok(value) => json!({"ok":true,"result":value}),
            Err(error) => json!({"ok":false,"error":error,"retry_same_operation":false}),
        };
        if result["result"]["waiting_for_user"] != true {
            self.results.insert(id.into(), (request, result.clone()));
        }
        Ok(result)
    }
    pub(super) fn content(
        &mut self,
        path: &str,
        cancel: &AtomicBool,
        worker: &mut Worker,
    ) -> AppResult<String> {
        if let Some(copy) = self.copies.get(path) {
            if extension(Path::new(&copy.name))? != "md" {
                self.txt_context = true;
            }
            return Ok(copy.text.clone());
        }
        if extension(Path::new(path))? != "md" {
            self.txt_context = true;
        }
        let stamp = self
            .memory
            .as_ref()
            .map(|m| m.source_stamp(path))
            .transpose()?;
        let content = loop {
            let result = read_cancel(
                &self.project,
                path,
                cancel,
                Some(worker),
                self.server_pdf.as_mut(),
            )
            .map(|(text, _)| text);
            match result {
                Err(error) if error.contains(super::interaction::BUSY) => {
                    if !self.wait_for_file(&error, cancel)? {
                        return Err(format!("{} 等待關閉檔案。", super::interaction::DEFERRED));
                    }
                }
                other => break other?,
            }
        };
        if let (Some(memory), Some(stamp)) = (self.memory.as_mut(), stamp) {
            memory.register_document(path, &content, &stamp)?;
        }
        Ok(content)
    }
    /// 選欄讀取不呼叫 content()，避免先建立受 2000 格限制的完整編輯快照。
    /// 來源及上層目錄在整次 COM 操作期間禁止替換；每批使用原檔 bytes 的版本指紋。
    fn with_excel<T>(
        &self,
        path: &str,
        expected: Option<&str>,
        read: impl FnOnce(&Path) -> AppResult<T>,
    ) -> AppResult<(T, String)> {
        let relative = relative(path)?;
        if !matches!(
            extension(&relative)?.as_str(),
            "xlsx" | "xls" | "xlsm" | "xlsb"
        ) {
            return Err(
                "Excel 選欄工具需要專案內已儲存的 XLS/XLSX/XLSM/XLSB 路徑；工作副本請先儲存。"
                    .into(),
            );
        }
        let target = self.project.root.join(relative);
        let _directories = pin(target.parent().ok_or("缺少 Excel 來源目錄。")?)?;
        let source = checked_file(&target)?;
        reject_internal(&source)?;
        let revision = format!("excel:{}", fingerprint(&self.project, path)?);
        if expected.is_some_and(|expected| expected != revision) {
            return Err("Excel 原檔版本已變更，或提供的不是選欄讀取版本；請重新 inspect_excel 後再讀取，不可混用兩個版本的列資料。".into());
        }
        let value = read(&target)?;
        if revision != format!("excel:{}", fingerprint(&self.project, path)?) {
            return Err("Excel 在讀取期間變更，本輪資料未接受，請重新取得表頭與版本。".into());
        }
        Ok((value, revision))
    }

    fn perform(
        &mut self,
        tool: &Tool,
        worker: &mut Worker,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        match tool {
            Tool::OutlookFolders {
                scope,
                parent_id,
                offset,
            } => self.outlook.folders(
                &mut crate::outlook::project::Reader,
                scope,
                parent_id.as_deref(),
                *offset,
                cancel,
            ),
            Tool::OutlookHeaders {
                folder_id,
                start_date,
                end_date,
                cursor,
            } => self.outlook.headers(
                &mut crate::outlook::project::Reader,
                folder_id,
                start_date,
                end_date,
                cursor.as_deref(),
                cancel,
            ),
            Tool::OutlookCompare { mail_ids, offset } => self.outlook.compare(
                &mut crate::outlook::project::Reader,
                mail_ids,
                *offset,
                cancel,
            ),
            Tool::OutlookRead { mail_id, offset } => {
                self.txt_context = true;
                self.outlook.body(
                    &mut crate::outlook::project::Reader,
                    mail_id,
                    *offset,
                    cancel,
                )
            }
            Tool::ListLogs {
                path,
                date,
                category,
                station,
                offset,
            } => super::logs::list(
                &self.project,
                path,
                date.as_deref(),
                category.as_deref(),
                station.as_deref(),
                *offset,
                cancel,
            ),
            Tool::ReadLog {
                path,
                revision,
                start_line,
                start_column,
                line_count,
            } => {
                self.txt_context = true;
                super::logs::read(
                    &self.project,
                    path,
                    revision.as_deref(),
                    *start_line,
                    *start_column,
                    *line_count,
                    cancel,
                )
            }
            Tool::SearchLogs { query, cursor } => {
                self.txt_context = true;
                let previous = cursor
                    .as_ref()
                    .map(|id| {
                        self.log_cursors
                            .get(id)
                            .ok_or("LOG 游標已失效，請從第一頁重新搜尋。")
                    })
                    .transpose()?;
                let (mut result, next) =
                    super::logs::search(&self.project, query, previous, cancel)?;
                if let Some(next) = next {
                    if self.log_cursors.len() >= 240 {
                        return Err("LOG 續頁已達本次任務上限，請縮小查詢範圍。".into());
                    }
                    // 同一查詢／來源／位置取得同一個游標，避免重查製造假進度。
                    let id =
                        text::revision(&serde_json::to_string(&next).map_err(|e| e.to_string())?);
                    self.log_cursors.insert(id.clone(), next);
                    result["next_cursor"] = json!(id);
                } else {
                    result["next_cursor"] = Value::Null;
                }
                Ok(result)
            }
            Tool::InspectExcel {
                path,
                sheet,
                header_row,
                start_column,
                column_count,
            } => {
                self.txt_context = true;
                let (mut value, revision) = self.with_excel(path, None, |target| {
                    office::excel::inspect(
                        target,
                        *sheet,
                        *header_row,
                        start_column,
                        *column_count,
                        cancel,
                    )
                })?;
                value["path"] = json!(path);
                value["revision"] = json!(revision);
                Ok(value)
            }
            Tool::ReadExcelRange {
                path,
                revision,
                sheet,
                columns,
                header_row,
                start_row,
                row_count,
            } => {
                self.txt_context = true;
                let selection = office::excel::Selection {
                    sheet: *sheet,
                    columns: columns.clone(),
                    header_row: *header_row,
                    start_row: *start_row,
                    row_count: *row_count,
                };
                selection.validate()?;
                let (page, _) = self.with_excel(path, Some(revision), |target| {
                    office::excel::read(target, &selection, cancel)
                })?;
                let mut value = serde_json::to_value(&page).map_err(|e| e.to_string())?;
                value["path"] = json!(path);
                value["revision"] = json!(revision);
                value["row_count"] = json!(page.rows.len());
                value["data_cells"] = json!(page.rows.len() * page.columns.len());
                value["scope"] = json!("僅所列欄位與列號；next_row 為所選欄位下一批位置，不代表整份文件已讀完。value 為 Excel Value2；日期是序號，text 為顯示文字。公式值為 Excel 本次開啟時提供，未主動更新外部連結。");
                Ok(value)
            }
            Tool::ChartExcelRange {
                path,
                revision,
                sheet,
                x_column,
                y_columns,
                header_row,
                start_row,
                row_count,
                kind,
                title,
                x_label,
                y_label,
            } => {
                self.txt_context = true;
                if y_columns.is_empty() || y_columns.len() > 8 {
                    return Err("圖表需選擇 1–8 個縱軸欄位。".into());
                }
                let columns = std::iter::once(x_column.clone())
                    .chain(y_columns.iter().cloned())
                    .collect();
                let selection = office::excel::Selection {
                    sheet: *sheet,
                    columns,
                    header_row: *header_row,
                    start_row: *start_row,
                    row_count: *row_count,
                };
                selection.validate_chart()?;
                let (page, _) = self.with_excel(path, Some(revision), |target| {
                    office::excel::read_chart(target, &selection, cancel)
                })?;
                let prepared = super::charts::prepare_page(
                    &page, kind, title, x_label, y_label, path, revision,
                )?;
                let Some(chart) = self.review_chart(prepared, cancel)? else {
                    return Ok(json!({"waiting_for_user":true}));
                };
                // 決策期間不佔用 Excel；接受前重新核對來源版本，舊決策不套到新資料。
                self.with_excel(path, Some(revision), |_| Ok(()))?;
                self.add_chart(chart)
            }
            Tool::CompactContext {
                working_note,
                superseded,
                next_step,
            } => {
                super::progress::validate_handoff(working_note, superseded, next_step)?;
                // runner 在本結果封存成功後才套用；此處不可自行丟棄正在執行的對話。
                self.archive_results()?;
                Ok(
                    json!({"handoff_validated":true,"apply_at":"本工具結果封存後、下一次模型請求前","raw_history":"read_work_log"}),
                )
            }
            Tool::ExportLogDataset {
                query,
                revisions,
                fields,
                name,
            } => {
                self.txt_context = true;
                let (table, extraction) =
                    super::logs::dataset(&self.project, query, revisions, fields, cancel)?;
                let mut result = self.save_dataset(name, &table, cancel)?;
                result["extraction"] = extraction;
                result["field_rules"] = json!(fields);
                Ok(result)
            }
            Tool::ExportExcelDataset {
                path,
                revision,
                sheet,
                columns,
                header_row,
                start_row,
                row_count,
                name,
            } => {
                self.txt_context = true;
                let selection = office::excel::Selection {
                    sheet: *sheet,
                    columns: columns.clone(),
                    header_row: *header_row,
                    start_row: *start_row,
                    row_count: *row_count,
                };
                selection.validate_chart()?;
                let (page, _) = self.with_excel(path, Some(revision), |target| {
                    office::excel::read_chart(target, &selection, cancel)
                })?;
                let table = super::datasets::Table::from_excel(&page, path, revision)?;
                let mut result = self.save_dataset(name, &table, cancel)?;
                result["headers"] = json!(page
                    .headers
                    .iter()
                    .map(|h| h.text.chars().take(100).collect::<String>())
                    .collect::<Vec<_>>());
                result["next_source_row"] = json!(page.next_row);
                result["date_1904"] = json!(page.date_1904);
                Ok(result)
            }
            Tool::InspectDataset { path, revision } => {
                let (table, actual_revision) =
                    super::datasets::inspect(&self.project, path, revision.as_deref(), cancel)?;
                let reference = super::datasets::Reference {
                    path: path.clone(),
                    revision: actual_revision,
                    rows: table.rows.len(),
                    columns: table.columns.clone(),
                };
                Ok(table.summary(&reference))
            }
            Tool::ChartDataset {
                path,
                revision,
                x_column,
                y_columns,
                start_row,
                row_count,
                kind,
                title,
                x_label,
                y_label,
            } => {
                let table = super::datasets::load(&self.project, path, revision, cancel)?;
                let page = table.page(x_column, y_columns, *start_row, *row_count)?;
                let prepared = super::charts::prepare_page(
                    &page, kind, title, x_label, y_label, path, revision,
                )?;
                let Some(mut chart) = self.review_chart(prepared, cancel)? else {
                    return Ok(json!({"waiting_for_user":true}));
                };
                // 使用者選擇異常值處理期間，CSV 也可能被外部程式改動。
                super::datasets::load(&self.project, path, revision, cancel)?;
                table.annotate(&mut chart);
                self.add_chart(chart)
            }
            Tool::LoadSkill { id } => {
                super::skills::activate(&mut self.loaded_skills, id)?;
                Ok(
                    json!({"id":id,"loaded":self.loaded_skills,"instructions_location":"下一輪 system，工具定義位於 tools；無需重複載入。"}),
                )
            }
            Tool::SearchFiles { paths, query } => self.search_files(paths, query, worker, cancel),
            Tool::OfficeBatch {
                copy_id,
                revision,
                operations,
            } => self.office_batch(copy_id, revision, operations.clone(), cancel),
            Tool::CreateChart { chart } => {
                if !chart.data_note.is_empty()
                    || !chart.data_issues.is_empty()
                    || chart.series.iter().any(|s| !s.skip_indices.is_empty())
                {
                    return Err("圖表的使用者處理紀錄只能由桌面預檢建立。".into());
                }
                self.add_chart(chart.clone())
            }
            Tool::ExportChartPng { chart_index, name } => {
                self.export_chart_png(*chart_index, name, cancel)
            }
            Tool::ChartFromExcel {
                path,
                revision,
                sheet,
                range,
                kind,
                title,
                x_label,
                y_label,
            } => {
                let content = self.content(path, cancel, worker)?;
                if text::revision(&content) != *revision {
                    return Err("文件版本已變更，請重新讀取。".into());
                }
                let prepared = super::charts::prepare_excel(
                    &content, *sheet, range, kind, title, x_label, y_label, path, revision,
                )?;
                let Some(chart) = self.review_chart(prepared, cancel)? else {
                    return Ok(json!({"waiting_for_user":true}));
                };
                if text::revision(&self.content(path, cancel, worker)?) != *revision {
                    return Err("文件版本已變更，請重新讀取。".into());
                }
                self.add_chart(chart)
            }
            Tool::SummarizeDocument { .. } => Err("摘要委派需由專案任務協調器執行。".into()),
            Tool::ReadWorkLog {
                operation_id,
                offset,
            } => {
                if let Some(id) = operation_id {
                    let (request, result) = self
                        .recorded_operation(id)?
                        .ok_or("找不到此操作的已保存結果。")?;
                    let log =
                        json!({"operation_id":id,"request":request,"result":result}).to_string();
                    let total = log.chars().count();
                    if *offset > total {
                        return Err("操作紀錄位置超過尾端。".into());
                    }
                    let part: String = log.chars().skip(*offset).take(6000).collect();
                    return Ok(
                        json!({"text":part,"offset":offset,"next_offset":offset+part.chars().count(),"total":total,"operation_id":id}),
                    );
                }
                // 預設只列索引；指定 operation_id 才讀原文，避免每次重建全天紀錄。
                let mut index = self.archived_results.clone();
                for (id, (request, result)) in &self.results {
                    index.insert(
                        id.clone(),
                        json!({"tool":request["tool"],"ok":result["ok"]}),
                    );
                }
                let log =
                    json!({"operations":index,"details":"以 operation_id 及 offset 查回原文"})
                        .to_string();
                let total = log.chars().count();
                if *offset > total {
                    return Err("紀錄讀取位置超出範圍。".into());
                }
                let part: String = log.chars().skip(*offset).take(6000).collect();
                Ok(
                    json!({"text":part,"offset":offset,"next_offset":offset+part.chars().count(),"total":total,"order":"operation_id；非時間順序"}),
                )
            }
            Tool::ListNotes { query } => self.memory()?.list_notes(query),
            Tool::ReadNote { id } => self.memory()?.read_note(id),
            Tool::CreateNote { scope, title, body } => {
                self.memory()?.create_note(scope, title, body)
            }
            Tool::UpdateNote {
                id,
                revision,
                title,
                body,
            } => self
                .memory()?
                .change_note(id, revision, Some((title, body)), false),
            Tool::DeleteNote { id, revision } => {
                self.memory()?.change_note(id, revision, None, false)
            }
            Tool::RestoreNote { id, revision } => {
                self.memory()?.change_note(id, revision, None, true)
            }
            Tool::ListDocumentSections { path, offset } => {
                self.content(path, cancel, worker)?;
                self.memory()?.document_info(path, *offset)
            }
            Tool::ReadDocumentSection {
                path,
                revision,
                section_id,
            } => {
                self.txt_context = true;
                self.memory
                    .as_mut()
                    .ok_or("未啟用專案記憶。")?
                    .read_section(path, revision, section_id)
            }
            Tool::UpdateDocumentNote {
                path,
                revision,
                note_revision,
                section_id,
                summary,
            } => self.memory()?.update_document_note(
                path,
                revision,
                note_revision,
                section_id.as_deref(),
                summary,
            ),
            Tool::ReadTaskResult {
                task_id,
                field,
                offset,
            } => self.memory()?.read_task_result(task_id, field, *offset),
            Tool::ListFiles { path } => {
                let target = self.project.root.join(relative(path)?);
                let _guards = pin(&target)?;
                if let Some(directory) = _guards.last() {
                    reject_internal(directory)?;
                }
                let mut entries = Vec::new();
                let mut truncated = false;
                for (scanned, entry) in fs::read_dir(&target)
                    .map_err(|e| e.to_string())?
                    .enumerate()
                {
                    let entry = entry.map_err(|e| e.to_string())?;
                    if entry
                        .file_name()
                        .to_string_lossy()
                        .eq_ignore_ascii_case(".lmai")
                    {
                        continue;
                    }
                    if entries.len() == 200 || scanned >= 500 {
                        truncated = true;
                        break;
                    }
                    let metadata = fs::symlink_metadata(entry.path()).map_err(|e| e.to_string())?;
                    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                        continue;
                    }
                    if metadata.is_dir()
                        || extension(&entry.path()).is_ok()
                        || super::logs::supported(&entry.path())
                    {
                        entries.push(json!({"name":entry.file_name().to_string_lossy(),"directory":metadata.is_dir()}));
                    }
                }
                Ok(json!({"entries":entries,"truncated":truncated}))
            }
            Tool::ReadFile { path, offset } => {
                if super::logs::supported(Path::new(path)) {
                    if *offset != 0 {
                        return Err(
                            "LOG 請載入 log-analysis，使用 read_log 的行號與 revision 續讀。"
                                .into(),
                        );
                    }
                    self.txt_context = true;
                    let mut result =
                        super::logs::read(&self.project, path, None, 1, 0, 30, cancel)?;
                    result["guidance"] = json!("這是 LOG 首頁。載入 log-analysis，使用 read_log 續讀或 search_logs 搜尋；不可使用全文摘要工具。");
                    return Ok(result);
                }
                let content = self.content(path, cancel, worker)?;
                let total = content.chars().count();
                if *offset > total {
                    return Err("讀取位置超過全文。".into());
                }
                let text: String = content.chars().skip(*offset).take(6000).collect();
                let next = offset + text.chars().count();
                let document = if self.copies.contains_key(path) {
                    Value::Null
                } else if let Some(memory) = self.memory.as_mut() {
                    memory.record_read(path, *offset, next);
                    memory.read_info(path, *offset, next)?
                } else {
                    Value::Null
                };
                Ok(
                    json!({"text":text,"offset":offset,"next_offset":next,"total":total,"truncated":next<total,"revision":text::revision(&content),"document":document,"imported_snapshot":self.project.imports.contains_key(&path.replace('\\', "/"))}),
                )
            }
            Tool::FindText { path, text: needle } => {
                if needle.is_empty() {
                    return Err("搜尋文字不可空白。".into());
                }
                let content = self.content(path, cancel, worker)?;
                let positions: Vec<_> = content
                    .match_indices(needle)
                    .take(101)
                    .map(|(index, _)| content[..index].chars().count())
                    .collect();
                Ok(
                    json!({"positions":positions.iter().take(100).collect::<Vec<_>>(),"truncated":positions.len()>100,"revision":text::revision(&content)}),
                )
            }
            Tool::CreateWorkingCopy { source, name } => {
                if self.copies.len() >= 20 {
                    return Err("本次工作副本已達 20 份。".into());
                }
                let rel = relative(name)?;
                if rel.components().count() != 1 {
                    return Err("工作副本只填檔名，不含資料夾。".into());
                }
                let ext = extension(&rel)?;
                if matches!(ext.as_str(), "pdf" | "msg") {
                    return Err("PDF／MSG 只支援 TXT 文字副本。".into());
                }
                let (content, encoding) = if let Some(source) = source {
                    if ext != "md" {
                        self.txt_context = true;
                    }
                    let source_ext = extension(Path::new(source))?;
                    let expected_ext = if matches!(source_ext.as_str(), "pdf" | "msg") {
                        "txt"
                    } else {
                        &source_ext
                    };
                    if ext != expected_ext {
                        return Err("副本需保留來源格式；PDF／MSG 只建立 TXT 文字副本。".into());
                    }
                    read_cancel(
                        &self.project,
                        source,
                        cancel,
                        Some(worker),
                        self.server_pdf.as_mut(),
                    )?
                } else {
                    if matches!(ext.as_str(), "docx" | "xlsx" | "pptx") {
                        self.txt_context = true;
                        (
                            office::render(None, &rel, None, &[], None, cancel)?.serialize()?,
                            Encoding::Utf8(true),
                        )
                    } else if ext == "txt" {
                        (String::new(), Encoding::Utf8(true))
                    } else {
                        return Err(
                            "新建只支援 TXT、DOCX、XLSX、PPTX；舊格式與 MD 請從來源建立副本。"
                                .into(),
                        );
                    }
                };
                let office = if office::supported(&rel) {
                    let original: office::Snapshot =
                        serde_json::from_str(&content).map_err(|e| e.to_string())?;
                    Some(OfficeCopy {
                        source: source.clone(),
                        fingerprint: source
                            .as_ref()
                            .map(|s| fingerprint(&self.project, s))
                            .transpose()?
                            .unwrap_or_default(),
                        desired: original.clone(),
                        original: source.as_ref().map(|_| original),
                        actions: Vec::new(),
                    })
                } else {
                    None
                };
                let id = crate::jobs::new_id()?;
                let revision = text::revision(&content);
                self.copies.insert(
                    id.clone(),
                    Copy {
                        office,
                        name: name.clone(),
                        text: content,
                        encoding,
                        saved_revision: None,
                        paths: Vec::new(),
                    },
                );
                Ok(json!({"copy_id":id,"revision":revision}))
            }
            Tool::EditText {
                copy_id,
                revision,
                start,
                expected,
                replacement,
            } => {
                let copy = self
                    .copies
                    .get_mut(copy_id)
                    .ok_or("不是本次任務的工作副本。")?;
                if copy.office.is_some() {
                    return Err(
                        "Office 副本請使用 edit_office 修改區塊，不能用純文字索引修改封裝。".into(),
                    );
                }
                let next = worker.edit(
                    &Edit {
                        content: copy.text.clone(),
                        revision: revision.clone(),
                        start: *start,
                        expected: expected.clone(),
                        replacement: replacement.clone(),
                    },
                    cancel,
                )?;
                // 即使子程序出錯，broker 也只接受相同固定操作的結果。
                if next != text::edit(&copy.text, revision, *start, expected, replacement)? {
                    return Err("子程序修改結果不一致。".into());
                }
                copy.text = next;
                Ok(json!({"copy_id":copy_id,"revision":text::revision(&copy.text)}))
            }
            Tool::EditOffice {
                copy_id,
                revision,
                block_id,
                expected,
                replacement,
            } => self.office_action(
                copy_id,
                revision,
                office::Action::Edit {
                    block_id: block_id.clone(),
                    expected: expected.clone(),
                    replacement: replacement.clone(),
                },
                cancel,
            ),
            Tool::OfficeAction {
                copy_id,
                revision,
                operation,
            } => self.office_action(copy_id, revision, *operation.clone(), cancel),
            Tool::SaveCopy { copy_id, revision } => {
                let copy = self
                    .copies
                    .get_mut(copy_id)
                    .ok_or("不是本次任務的工作副本。")?;
                if text::revision(&copy.text) != *revision {
                    return Err("版本已改變，請重新讀取。".into());
                }
                if copy.saved_revision.as_ref() == Some(revision) {
                    return Ok(
                        json!({"copy_id":copy_id,"path":copy.paths.last(),"revision":revision}),
                    );
                }
                if self.txt_context && extension(Path::new(&copy.name))? == "md" {
                    return Err(
                        "本次任務讀取過 TXT，禁止輸出至未加密的 MD。請建立 TXT 成果。".into(),
                    );
                }
                // 原檔版本與內容都核對，格式或其他非文字部分變動也不能沿用舊副本。
                if let Some(office) = &copy.office {
                    if office
                        .source
                        .as_ref()
                        .map(|s| fingerprint(&self.project, s))
                        .transpose()?
                        .is_some_and(|stamp| stamp != office.fingerprint)
                    {
                        return Err("Office 原檔已變動，請重新建立副本。".into());
                    }
                }
                let _root = pin(&self.project.root)?;
                let base = self.project.root.join("_AI_Output");
                match fs::create_dir(&base) {
                    Ok(()) => (),
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
                    Err(e) => return Err(e.to_string()),
                }
                let _base = pin(&base)?;
                if self.output_folder.is_none() {
                    self.output_folder = Some(create_output_folder(&base)?);
                }
                let folder_name = self.output_folder.as_deref().ok_or("缺少輸出資料夾。")?;
                let folder = base.join(folder_name);
                let _folder = pin(&folder)?;
                let (name, mut output) = reserve_output(&folder, &copy.name)?;
                // 路徑先記錄，失敗時也能告知使用者可能已建立的檔案。
                let relative = format!("_AI_Output/{folder_name}/{name}");
                self.published.push(relative.clone());
                if let Some(office) = &copy.office {
                    // Office 先寫入本次獨立暫存目錄；發布仍以已保留的 create_new handle 寫入，
                    // 不讓 Office 的 SaveAs 覆寫使用者檔案。公司加密是否允許此複製，最後由 Office 讀回確認。
                    let stage = folder.join(format!(".office_{}", crate::jobs::new_id()?));
                    fs::create_dir(&stage).map_err(|e| e.to_string())?;
                    let stage_guard = pin_stage(&stage)?;
                    let staged = stage.join(&copy.name);
                    let saved = (|| {
                        let snapshot = render_office(
                            &self.project,
                            &copy.name,
                            office,
                            Some(&staged),
                            cancel,
                        )?;
                        if snapshot != office.desired {
                            return Err("Office 重建結果與工作版本不一致。".into());
                        }
                        let mut input = checked_file(&staged)?;
                        if input.metadata().map_err(|e| e.to_string())?.len() > 50_000_000 {
                            return Err("Office 成果超過 50 MB。".into());
                        }
                        std::io::copy(&mut input, &mut output).map_err(|e| e.to_string())?;
                        output.sync_all().map_err(|e| e.to_string())
                    })();
                    // 暫存位置只由程式產生，不遞迴刪除、不跟隨模型提供的路徑。
                    let _ = fs::remove_file(&staged);
                    drop(stage_guard);
                    let _ = fs::remove_dir(&stage);
                    saved.map_err(|e: String| format!("可能已建立 {relative}，尚未交付：{e}"))?;
                } else {
                    let bytes = text::encode(&copy.text, copy.encoding)?;
                    output
                        .write_all(&bytes)
                        .and_then(|_| output.sync_all())
                        .map_err(|e| format!("可能有部分輸出 {relative}，未交付：{e}"))?;
                }
                drop(output);
                let verified = read_cancel(&self.project, &relative, cancel, Some(worker), None).map_err(|e| format!("已建立 {relative}，但無法驗證加密後內容，尚未交付。請用對應應用程式檢查：{e}"))?.0;
                if verified != copy.text {
                    return Err(format!("已建立 {relative}，讀回內容不一致，尚未交付。"));
                }
                copy.saved_revision = Some(revision.clone());
                copy.paths.push(relative.clone());
                Ok(json!({"copy_id":copy_id,"path":relative,"revision":revision,"verified":true}))
            }
            Tool::DeleteCopy { copy_id } => {
                let copy = self.copies.get(copy_id).ok_or("不是本次任務的工作副本。")?;
                if !copy.paths.is_empty() {
                    return Err("已發布的成果不可刪除；請由使用者管理。".into());
                }
                self.copies.remove(copy_id);
                Ok(json!({"discarded":copy_id}))
            }
        }
    }
    /// 驗證通過才替換記憶體版本；失敗的格式／結構操作不會污染先前成功的工作。
    fn office_action(
        &mut self,
        id: &str,
        revision: &str,
        action: office::Action,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        self.office_batch(id, revision, vec![action], cancel)
    }
    /// 整批在候選文件重建成功後才提交；失敗不修改目前副本，且只啟動一次 Office。
    fn office_batch(
        &mut self,
        id: &str,
        revision: &str,
        mut actions: Vec<office::Action>,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        if actions.is_empty() || actions.len() > 20 {
            return Err("每批需為 1–20 個 Office 操作。".into());
        }
        for action in &mut actions {
            if let office::Action::InsertImage { path, sha256, .. } = action {
                if sha256.is_some() {
                    return Err("圖片雜湊由程式管理，不接受模型提供的 sha256。".into());
                }
                *sha256 = Some(locked_image(&self.project, path)?.2);
            }
        }
        let copy = self.copies.get_mut(id).ok_or("不是本次任務的工作副本。")?;
        if text::revision(&copy.text) != revision {
            return Err("版本已改變，請重新讀取。".into());
        }
        let office = copy.office.as_mut().ok_or("此工具只適用 Office 副本。")?;
        let mut candidate = office.clone();
        candidate.actions.extend(actions);
        if candidate.actions.len() > 200 {
            return Err("單一文件最多 200 次修改；請發布後建立新副本。".into());
        }
        let snapshot = render_office(&self.project, &copy.name, &candidate, None, cancel)?;
        let content = snapshot.serialize()?;
        let result = json!({"copy_id":id,"revision":text::revision(&content),"structure":snapshot.structure,
            "blocks_tail":snapshot.blocks.iter().rev().take(12).map(|b| json!({"id":b.id,"label":b.label,"kind":b.kind,"format":b.format,"preview":b.text.chars().take(120).collect::<String>()})).collect::<Vec<_>>(),
            "hint":"結構改變後使用新版本與區塊 ID；完整內容可用 read_file(copy_id) 讀取。"});
        candidate.desired = snapshot;
        *office = candidate;
        copy.text = content;
        Ok(result)
    }
    pub fn finish(&self, artifacts: &[String]) -> AppResult<Vec<String>> {
        self.finish_cancellable(artifacts, &AtomicBool::new(false))
    }
    /// Office 交付讀回也要遵守取消，不能在使用者取消後繼續逐檔啟動 Office。
    pub fn finish_cancellable(
        &self,
        artifacts: &[String],
        cancel: &AtomicBool,
    ) -> AppResult<Vec<String>> {
        let unique: std::collections::BTreeSet<_> = artifacts.iter().collect();
        if unique.len() != artifacts.len() || artifacts.len() != self.copies.len() {
            return Err(
                "交付清單必須列出本次所有工作副本且不可重複；不需要的未發布副本請先捨棄。".into(),
            );
        }
        let mut paths = Vec::new();
        for id in artifacts {
            let copy = self.copies.get(id).ok_or("完成清單含未知副本。")?;
            if copy.saved_revision.as_deref() != Some(text::revision(&copy.text).as_str()) {
                return Err("成果尚未成功儲存最新版本，不能交付。".into());
            }
            let path = copy.paths.last().ok_or("成果尚未儲存。")?;
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err("成果檢查已取消。".into());
            }
            if read_cancel(&self.project, path, cancel, None, None)?.0 != copy.text {
                return Err("成果交付前已變更，請重新確認。".into());
            }
            paths.push(path.clone());
        }
        if self
            .copies
            .values()
            .any(|copy| copy.saved_revision.as_deref() != Some(text::revision(&copy.text).as_str()))
        {
            return Err("仍有未儲存的工作副本，請先儲存或捨棄。".into());
        }
        for dataset in &self.datasets {
            super::datasets::load(&self.project, &dataset.path, &dataset.revision, cancel)?;
            paths.push(dataset.path.clone());
        }
        // PNG 已由匯出工具發布，不是文字工作副本；自動併入交付清單並重新核對。
        for export in &self.chart_exports {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err("成果檢查已取消。".into());
            }
            self.verify_chart_export(export)?;
            paths.push(export.path.clone());
        }
        Ok(paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dataset_checkpoint_reopens_and_rejects_modified_csv_without_touching_sources() {
        let root = std::env::temp_dir().join(format!(
            "dataset-checkpoint-{}",
            crate::jobs::new_id().unwrap()
        ));
        fs::create_dir(&root).unwrap();
        let project = Project {
            id: "dataset-test".into(),
            name: "CSV".into(),
            root: root.clone(),
            imports: Default::default(),
        };
        let cancel = AtomicBool::new(false);
        let mut broker = Broker::new(project.clone(), "task".into()).unwrap();
        let mut table = super::super::datasets::Table::new(vec!["x".into(), "y".into()]).unwrap();
        table
            .push(super::super::datasets::Row {
                path: "source.log".into(),
                revision: "r".into(),
                sheet: 0,
                row: 123,
                values: vec!["1".into(), "2".into()],
                kinds: vec![],
                texts: vec![],
            })
            .unwrap();
        let result = broker.save_dataset("資料.csv", &table, &cancel).unwrap();
        let mut restored = Broker::new(project.clone(), "task".into()).unwrap();
        restored.restore(broker.saved().unwrap(), &cancel).unwrap();
        assert_eq!(restored.dataset_index()[0]["rows"], 1);
        let path = result["dataset"]["path"].as_str().unwrap();
        let (_, revision) = super::super::datasets::inspect(&project, path, None, &cancel).unwrap();
        assert_eq!(revision, result["dataset"]["revision"].as_str().unwrap());
        assert_eq!(restored.finish(&[]).unwrap(), vec![path]);
        fs::write(root.join(path), "externally changed").unwrap();
        assert!(restored.finish(&[]).is_err());
        assert!(Broker::new(project, "task".into())
            .unwrap()
            .restore(broker.saved().unwrap(), &cancel)
            .is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn archived_results_keep_operation_identity_and_survive_restore() {
        let root =
            std::env::temp_dir().join(format!("lmai-archive-{}", crate::jobs::new_id().unwrap()));
        fs::create_dir_all(&root).unwrap();
        let project = Project {
            id: "archive".into(),
            name: "archive".into(),
            root,
            imports: BTreeMap::new(),
        };
        let mut broker = Broker::new(project.clone(), "run".into()).unwrap();
        broker.enable_memory("chat").unwrap();
        let tool = Tool::ReadFile {
            path: "source.txt".into(),
            offset: 0,
        };
        let result = json!({"ok":true,"result":{"text":"已確認證據","revision":"v1"}});
        broker.remember_result("operation", &tool, &result).unwrap();
        broker.archive_results().unwrap();
        assert!(broker.results.is_empty());
        assert_eq!(
            broker.cached_result("operation", &tool).unwrap(),
            Some(result.clone())
        );
        let mut restored = Broker::new(project, "run".into()).unwrap();
        restored.enable_memory("chat").unwrap();
        restored
            .restore(broker.saved().unwrap(), &AtomicBool::new(false))
            .unwrap();
        assert_eq!(
            restored.cached_result("operation", &tool).unwrap(),
            Some(result)
        );
        assert!(restored
            .cached_result(
                "operation",
                &Tool::ReadFile {
                    path: "other.txt".into(),
                    offset: 0
                }
            )
            .is_err());
    }
    #[test]
    fn tools_require_existing_edit_and_export_targets() {
        let mut broker = Broker::new(
            Project {
                id: "p".into(),
                name: "p".into(),
                root: std::env::current_dir().unwrap(),
                imports: BTreeMap::new(),
            },
            "r".into(),
        )
        .unwrap();
        let definitions: Value = serde_json::from_str(include_str!("tools.json")).unwrap();
        let mut request = definitions.clone();
        broker.restrict_tools(&mut request);
        let has = |r: &Value, name: &str| {
            r["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t["function"]["name"] == name)
        };
        for name in [
            "edit_text",
            "office_action",
            "office_batch",
            "save_copy",
            "delete_copy",
            "export_chart_png",
        ] {
            assert!(!has(&request, name));
        }
        assert!(!has(&request, "create_working_copy") && has(&request, "finish"));
        super::super::skills::activate(&mut broker.loaded_skills, "text-edit").unwrap();
        broker.copies.insert(
            "c".into(),
            Copy {
                office: None,
                name: "a.txt".into(),
                text: String::new(),
                encoding: Encoding::Utf8(false),
                saved_revision: None,
                paths: vec![],
            },
        );
        let mut request = definitions;
        broker.restrict_tools(&mut request);
        assert!(has(&request, "edit_text") && has(&request, "save_copy"));
        assert!(!has(&request, "office_action") && !has(&request, "export_chart_png"));
    }
    #[test]
    fn png_exports_preserve_names_reuse_verified_files_and_survive_restore() {
        let root = std::env::current_dir()
            .unwrap()
            .join(".build")
            .join(format!("png-test-{}", crate::jobs::new_id().unwrap()));
        fs::create_dir_all(&root).unwrap();
        let project = Project {
            id: "png".into(),
            name: "png".into(),
            root: root.clone(),
            imports: BTreeMap::new(),
        };
        let mut broker = Broker::new(project.clone(), "run".into()).unwrap();
        let chart = super::super::charts::Chart {
            kind: "line".into(),
            title: "圖".into(),
            x_label: "x".into(),
            y_label: "y".into(),
            x: vec![json!(1)],
            series: vec![super::super::charts::Series {
                skip_indices: vec![],
                name: "A".into(),
                values: vec![Some(2.0)],
            }],
            source: "測試".into(),
            data_note: String::new(),
            data_issues: vec![],
        };
        broker.add_chart(chart.clone()).unwrap();
        broker.add_chart(chart).unwrap();
        broker.set_png_renderer(Box::new(|_, _| Ok(super::super::charts::png::fixture())));
        let cancel = AtomicBool::new(false);
        assert!(broker.export_chart_png(0, "../bad.png", &cancel).is_err());
        assert!(broker.export_chart_png(99, "bad.png", &cancel).is_err());
        let first = broker.export_chart_png(0, "趨勢.png", &cancel).unwrap();
        let again = broker.export_chart_png(0, "趨勢.png", &cancel).unwrap();
        assert_eq!(first["path"], again["path"]);
        assert_eq!(again["reused"], true);
        let second = broker.export_chart_png(1, "趨勢.png", &cancel).unwrap();
        assert!(second["path"].as_str().unwrap().ends_with("趨勢_2.png"));
        let mut restored = Broker::new(project, "run".into()).unwrap();
        restored.restore(broker.saved().unwrap(), &cancel).unwrap();
        let paths = restored.finish(&[]).unwrap();
        assert_eq!(paths.len(), 2);
        fs::write(root.join(&paths[0]), b"changed").unwrap();
        assert!(restored.finish(&[]).is_err());
        for path in &paths {
            fs::remove_file(root.join(path)).unwrap();
        }
        fs::remove_dir(root.join(paths[0].rsplit_once('/').unwrap().0)).unwrap();
        fs::remove_dir(root.join("_AI_Output")).unwrap();
        fs::remove_dir(root).unwrap();
    }
    #[test]
    fn reader_copy_allows_reader_write_access_without_unlocking_original() {
        let root = std::env::current_dir()
            .unwrap()
            .join(".build")
            .join(format!("reader-copy-{}", crate::jobs::new_id().unwrap()));
        fs::create_dir_all(&root).unwrap();
        let original = root.join("source.msg");
        fs::write(&original, b"original bytes").unwrap();
        let mut source = checked_file(&original).unwrap();
        let project = Project {
            id: "copy-test".into(),
            name: "copy-test".into(),
            root: root.clone(),
            imports: BTreeMap::new(),
        };
        let (result, warning) = with_reader_copy(&project, &mut source, "msg", |copy| {
            assert!(OpenOptions::new().write(true).open(&original).is_err());
            assert!(fs::rename(copy, copy.with_extension("moved")).is_err());
            let mut writer = OpenOptions::new()
                .read(true)
                .write(true)
                .share_mode(FILE_SHARE_READ)
                .open(copy)
                .map_err(|e| e.to_string())?;
            writer.write_all(b"updated").map_err(|e| e.to_string())?;
            Ok(copy.to_path_buf())
        })
        .unwrap();
        assert!(warning.is_none());
        assert!(!result.exists());
        assert_eq!(fs::read(&original).unwrap(), b"original bytes");
        let error = with_reader_copy(&project, &mut source, "msg", |_| {
            Err::<(), _>("reader failed".into())
        })
        .unwrap_err();
        assert_eq!(error, "reader failed");
        // 模擬 Outlook 在讀取成功後暫留 handle：回傳正文與提醒，不誤報整次失敗。
        let ((held, copy), warning) = with_reader_copy(&project, &mut source, "msg", |copy| {
            let held = OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
                .open(copy)
                .map_err(|e| e.to_string())?;
            Ok((held, copy.to_path_buf()))
        })
        .unwrap();
        assert!(warning.unwrap().contains("閱讀暫存尚未能清除"));
        assert!(copy.exists());
        drop(held);
        fs::remove_file(&copy).unwrap();
        fs::remove_dir(copy.parent().unwrap()).unwrap();
        assert_eq!(fs::read_dir(root.join("_AI_Output")).unwrap().count(), 0);
        drop(source);
        fs::remove_file(original).unwrap();
        fs::remove_dir(root.join("_AI_Output")).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn names_collide_without_overwrite_and_directory_pins_still_block_changes() {
        let root = std::env::current_dir()
            .unwrap()
            .join(".build")
            .join(format!("name-test-{}", crate::jobs::new_id().unwrap()));
        fs::create_dir_all(&root).unwrap();
        let first = create_output_folder(&root).unwrap();
        let second = create_output_folder(&root).unwrap();
        assert_ne!(first, second);
        assert_eq!(&first[8..9], "_");
        assert!(first[..8].bytes().all(|c| c.is_ascii_digit()));
        let folder = root.join(&first);
        let pins = pin(&folder).unwrap();
        assert!(OpenOptions::new()
            .access_mode(windows_sys::Win32::Foundation::GENERIC_WRITE)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(&folder)
            .is_err());
        assert!(fs::rename(&folder, root.join("moved")).is_err());
        let (name, mut output) = reserve_output(&folder, "報告.txt").unwrap();
        output.write_all(b"original").unwrap();
        drop(output);
        let (next, output) = reserve_output(&folder, "報告.txt").unwrap();
        drop(output);
        assert_eq!(name, "報告.txt");
        assert_eq!(next, "報告_2.txt");
        assert_eq!(fs::read(folder.join(&name)).unwrap(), b"original");
        // 換鎖時 anchor 單獨持有，Windows 也必須拒絕更名父目錄。
        let transition = folder.join("transition");
        fs::create_dir(&transition).unwrap();
        let anchor = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .share_mode(0)
            .open(transition.join(".anchor"))
            .unwrap();
        assert!(fs::rename(&transition, folder.join("transition-moved")).is_err());
        drop(anchor);
        fs::remove_file(transition.join(".anchor")).unwrap();
        fs::remove_dir(transition).unwrap();
        let stage = folder.join("stage");
        fs::create_dir(&stage).unwrap();
        let stage_pin = pin_stage(&stage).unwrap();
        // 直接嘗試設 Junction，確保「非空目錄」防線真正在 OS 層生效。
        let junction = OpenOptions::new()
            .access_mode(windows_sys::Win32::Foundation::GENERIC_WRITE)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(&stage)
            .unwrap();
        let substitute: Vec<u16> = format!("\\??\\{}", root.display()).encode_utf16().collect();
        let display: Vec<u16> = root.to_string_lossy().encode_utf16().collect();
        let mut reparse = Vec::new();
        reparse.extend_from_slice(&0xa0000003u32.to_le_bytes());
        reparse.extend_from_slice(
            &((8 + (substitute.len() + display.len() + 2) * 2) as u16).to_le_bytes(),
        );
        for value in [
            0,
            0,
            (substitute.len() * 2) as u16,
            ((substitute.len() + 1) * 2) as u16,
            (display.len() * 2) as u16,
        ] {
            reparse.extend_from_slice(&value.to_le_bytes());
        }
        for value in substitute.into_iter().chain([0]).chain(display).chain([0]) {
            reparse.extend_from_slice(&value.to_le_bytes());
        }
        let mut returned = 0;
        let result = unsafe {
            windows_sys::Win32::System::IO::DeviceIoControl(
                junction.as_raw_handle(),
                0x000900a4,
                reparse.as_ptr().cast(),
                reparse.len() as u32,
                std::ptr::null_mut(),
                0,
                &mut returned,
                std::ptr::null_mut(),
            )
        };
        assert_eq!(result, 0);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(145),
            "必須因目錄非空拒絕，而不是測試資料格式錯誤"
        );
        drop(junction);
        assert!(fs::remove_file(stage.join(".anchor")).is_err());
        assert!(fs::rename(&stage, folder.join("moved-stage")).is_err());
        drop(stage_pin);
        fs::remove_dir(stage).unwrap();
        fs::remove_file(folder.join(name)).unwrap();
        fs::remove_file(folder.join(next)).unwrap();
        drop(pins);
        fs::remove_dir(folder).unwrap();
        fs::remove_dir(root.join(second)).unwrap();
        fs::remove_dir(root).unwrap();
    }
    #[test]
    fn reject_windows_escape_and_device_paths() {
        for value in [
            "../secret.txt",
            "C:\\secret.txt",
            "\\\\host\\share",
            "file.txt:secret",
            "NUL.txt",
            "a/../b",
            "a.",
        ] {
            assert!(relative(value).is_err(), "{value}");
        }
        assert_eq!(
            relative("資料/文件.txt").unwrap().to_str().unwrap(),
            r"資料\文件.txt"
        );
    }
}
