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
fn reject_internal(file: &File) -> AppResult<()> {
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
        .map_err(|_| "文件不存在、被占用或無讀取權限。")?;
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
    let ext = extension(path)?;
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
        .map_err(|e| e.to_string())?;
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

struct OfficeCopy {
    source: String,
    fingerprint: String,
    original: office::Snapshot,
    desired: office::Snapshot,
}
struct Copy {
    office: Option<OfficeCopy>,
    name: String,
    text: String,
    encoding: Encoding,
    saved_revision: Option<String>,
    paths: Vec<String>,
}
pub struct Broker {
    server_pdf: Option<super::server_pdf::Reader>,
    memory: Option<super::memory::Memory>,
    project: Project,
    output_folder: Option<String>,
    copies: BTreeMap<String, Copy>,
    /// 同一 operation_id 只能配對同一份工具參數，重送僅回傳已記錄的結果。
    results: BTreeMap<String, (Value, Value)>,
    published: Vec<String>,
    /// 任務接觸 TXT 後，不允許把內容混入未加密的 MD 成果。
    txt_context: bool,
}
impl Broker {
    pub fn new(project: Project, _task: String) -> AppResult<Self> {
        validate_root(&project.root)?;
        Ok(Self {
            server_pdf: None,
            memory: None,
            project,
            output_folder: None,
            copies: BTreeMap::new(),
            results: BTreeMap::new(),
            published: Vec::new(),
            txt_context: false,
        })
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
    pub fn published(&self) -> &[String] {
        &self.published
    }
    pub fn execute(
        &mut self,
        id: &str,
        tool: &Tool,
        worker: &mut Worker,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        crate::jobs::validate_id(id)?;
        let request = serde_json::to_value(tool).map_err(|e| e.to_string())?;
        if let Some((old, result)) = self.results.get(id) {
            if old != &request {
                return Err("操作識別碼重複但參數不同；已停止。".into());
            }
            return Ok(result.clone());
        }
        let result = match self.perform(tool, worker, cancel) {
            Ok(value) => json!({"ok":true,"result":value}),
            Err(error) => json!({"ok":false,"error":error,"retry_same_operation":false}),
        };
        self.results.insert(id.into(), (request, result.clone()));
        Ok(result)
    }
    fn content(
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
        let content = read_cancel(
            &self.project,
            path,
            cancel,
            Some(worker),
            self.server_pdf.as_mut(),
        )
        .map(|(text, _)| text)?;
        if let (Some(memory), Some(stamp)) = (self.memory.as_mut(), stamp) {
            memory.register_document(path, &content, &stamp)?;
        }
        Ok(content)
    }
    fn perform(
        &mut self,
        tool: &Tool,
        worker: &mut Worker,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        match tool {
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
                    if metadata.is_dir() || extension(&entry.path()).is_ok() {
                        entries.push(json!({"name":entry.file_name().to_string_lossy(),"directory":metadata.is_dir()}));
                    }
                }
                Ok(json!({"entries":entries,"truncated":truncated}))
            }
            Tool::ReadFile { path, offset } => {
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
                    if ext != "txt" {
                        return Err("新的一般成果只建立 TXT；MD 僅允許既有 MD 的修訂副本。".into());
                    }
                    (String::new(), Encoding::Utf8(true))
                };
                let office = if office::supported(&rel) {
                    let source = source.as_ref().ok_or("Office 目前需由既有文件建立副本。")?;
                    let original: office::Snapshot =
                        serde_json::from_str(&content).map_err(|e| e.to_string())?;
                    Some(OfficeCopy {
                        source: source.clone(),
                        fingerprint: fingerprint(&self.project, source)?,
                        desired: original.clone(),
                        original,
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
            } => {
                let copy = self
                    .copies
                    .get_mut(copy_id)
                    .ok_or("不是本次任務的工作副本。")?;
                if text::revision(&copy.text) != *revision {
                    return Err("版本已改變，請重新讀取。".into());
                }
                let office = copy.office.as_mut().ok_or("此工具只適用 Office 副本。")?;
                let mut next = office.desired.clone();
                next.edit(block_id, expected, replacement)?;
                let serialized = next.serialize()?;
                office.desired = next;
                copy.text = serialized;
                Ok(json!({"copy_id":copy_id,"revision":text::revision(&copy.text)}))
            }
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
                    if fingerprint(&self.project, &office.source)? != office.fingerprint {
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
                    let source = self.project.root.join(self::relative(&office.source)?);
                    let _source_dirs = pin(source.parent().ok_or("缺少來源目錄。")?)?;
                    let _source_file = checked_file(&source)?;
                    if fingerprint(&self.project, &office.source)? != office.fingerprint {
                        return Err("Office 原檔已變動，請重新建立副本。".into());
                    }
                    let saved = (|| {
                        office::process(
                            &source,
                            Some(&staged),
                            Some(&office.original),
                            Some(&office.desired),
                            cancel,
                        )?;
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
        Ok(paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
