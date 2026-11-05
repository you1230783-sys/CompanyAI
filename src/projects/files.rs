//! 固定檔案 broker：只接受專案相對路徑；逐層鎖住目錄、拒絕重新解析點與硬連結。
//! 一般成果以 create_new 發布；Python 持續草稿只更新本任務擁有且雜湊吻合的檔案。
//! 原始文件從未取得可寫 handle。

mod analysis;
pub mod batch;
mod cad;
mod cad_active;
mod cad_comparison;
mod cad_copy;
mod cad_design;
mod cad_excel;
mod cad_plan;
mod cad_preflight;
mod cad_query;
mod cad_session;
mod chart_annotations;
mod chart_preferences;
mod chart_work;
mod code_review;
mod code_sections;
mod completion;
mod deliverables;
mod diagram_sources;
pub mod diagrams;
mod dispatch;
mod documents;
mod drafts;
mod execution;
pub mod export;
mod mail_notes;
mod output;
mod state;
#[cfg(test)]
mod tests;
pub use output::conversation_folder_name;
use output::{ensure_output_folder, reserve_external_output, shared_output_folder};
mod python;
mod python_edit;
mod references;
mod report_values;
mod review;
mod semantic_progress;
mod source_code;
mod workflows;

/// 常見程式格式僅供分析結果加上類型提示，不作為文字閱讀權限。
pub(super) fn is_source_code(path: &Path) -> bool {
    source_code::supported(path)
}

pub(super) fn is_plain_text_input(path: &Path) -> bool {
    !source_code::excluded(path) && plain_text_candidate(path)
}

pub(super) fn excluded_source_path(path: &Path) -> bool {
    source_code::excluded(path)
}
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
    sync::atomic::{AtomicBool, Ordering},
};
use windows_sys::Win32::Storage::FileSystem::*;

/// 大型操作的參數可能佔滿第一頁；另附有界的結果欄位，讓錯誤原因不被參數擠走。
/// 原始紀錄與字元分頁仍完全保留，不將此摘要當成已讀完操作的證據。
pub(super) fn work_log_outcome(result: &Value) -> Value {
    let mut outcome = serde_json::Map::new();
    for key in ["ok", "error"] {
        if let Some(value) = result.get(key) {
            outcome.insert(
                key.into(),
                match value.as_str() {
                    Some(text) => Value::String(text.chars().take(2000).collect()),
                    None if value.is_boolean() => value.clone(),
                    None => Value::Null,
                },
            );
        }
    }
    for key in [
        "status",
        "recovery",
        "drawing_started",
        "completed_operations",
        "saved",
    ] {
        if let Some(value) = result.get("result").and_then(|r| r.get(key)) {
            if value.is_boolean() || value.is_number() {
                outcome.insert(key.into(), value.clone());
            } else if let Some(text) = value.as_str() {
                outcome.insert(key.into(), Value::String(text.chars().take(120).collect()));
            }
        }
    }
    if let Some(error) = result["result"]["error"].as_str() {
        outcome.insert(
            "native_error".into(),
            Value::String(error.chars().take(2000).collect()),
        );
    }
    Value::Object(outcome)
}

/// 回傳的 handle 保持到操作結束，拒絕目錄本身的寫入／刪除共用，
/// 避免檢查後被重新命名、替換或改成 Junction；仍可在目錄內建立成果。
pub(crate) fn pin(path: &Path) -> AppResult<Vec<File>> {
    let path = super::setup::resolve_project_path(path)?;
    let mut current = PathBuf::new();
    let mut guards = Vec::new();
    for part in path.components() {
        match part {
            Component::Prefix(prefix)
                if matches!(
                    prefix.kind(),
                    std::path::Prefix::Disk(_) | std::path::Prefix::UNC(_, _)
                ) =>
            {
                current.push(part.as_os_str())
            }
            Component::RootDir => {
                current.push(part.as_os_str());
                // 網路磁碟與 UNC 使用同一組目錄 handle／重新解析點檢查；
                // 網路失效或不支援必要檢查時直接報錯，不降級成未驗證讀寫。
            }
            Component::Normal(_) => {
                current.push(part.as_os_str());
                let file = OpenOptions::new()
                    .read(true)
                    .share_mode(FILE_SHARE_READ)
                    .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
                    .open(&current)
                    .map_err(|error| format!("無法開啟專案路徑 {}：{error}。請確認網路連線、資料夾權限與程式登入工作階段。", current.display()))?;
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
            _ => return Err("只支援磁碟或 UNC 資料夾，不接受裝置路徑或上層跳轉。".into()),
        }
    }
    Ok(guards)
}

/// 以 Windows 正規化後的 handle 路徑排除私有資料；8.3 別名亦不可繞過名稱檢查。
pub(super) fn reject_internal(file: &File) -> AppResult<()> {
    let mut name = vec![0u16; 32768];
    let mut count = unsafe {
        GetFinalPathNameByHandleW(
            file.as_raw_handle(),
            name.as_mut_ptr(),
            name.len() as u32,
            0,
        )
    } as usize;
    if count == 0 {
        // 部分網路重新導向器無法提供 DOS 磁碟名稱；NT 路徑仍要求正規化，
        // 不退回 FILE_NAME_OPENED，以免短檔名別名繞過 .lmai 檢查。
        count = unsafe {
            GetFinalPathNameByHandleW(
                file.as_raw_handle(),
                name.as_mut_ptr(),
                name.len() as u32,
                VOLUME_NAME_NT,
            )
        } as usize;
    }
    if count == 0 || count >= name.len() {
        return Err(format!(
            "無法核對文件的實際位置：{}。",
            std::io::Error::last_os_error()
        ));
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
    let resolved = super::setup::resolve_project_path(root)?;
    let root = resolved.as_path();
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
        return Err("請選擇專案子資料夾，不可授權整個磁碟或網路分享根目錄。".into());
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
    if source_code::excluded(path) {
        return Err("此路徑屬於預設排除的版本庫、產物、依賴或機密設定，未讀取內容。".into());
    }
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    Ok(ext)
}

/// 副檔名只用於分派專用讀取器；一般文字是否可讀由實際解碼結果決定。
fn readable_extension(path: &Path) -> AppResult<String> {
    extension(path)
}

/// 這些格式有自己的讀取／輸出契約，不把抽取的文字當成原封裝內容編輯。
fn plain_text_candidate(path: &Path) -> bool {
    !office::supported(path)
        && !super::vision::input::supported(path)
        && !path.extension().is_some_and(|ext| {
            ["pdf", "msg", "dwg"]
                .iter()
                .any(|value| ext.eq_ignore_ascii_case(value))
        })
}

/// 文件摘要快取與程式文字分開；像CMakeLists.txt這類已知程式檔仍不建全文快取。
fn uses_document_memory(path: &Path) -> bool {
    !plain_text_candidate(path)
        || (!source_code::supported(path)
            && path.extension().is_some_and(|ext| {
                ext.eq_ignore_ascii_case("txt") || ext.eq_ignore_ascii_case("md")
            }))
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
    let ext = readable_extension(&rel)?;
    let target = project.root.join(&rel);
    let _guards = pin(target.parent().ok_or("缺少來源資料夾。")?)?;
    let mut file = checked_file(&target)?;
    reject_internal(&file)?;
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
    if !plain_text_candidate(&rel) {
        return Err("此檔案需使用對應的 CAD 或圖片工具，不能當純文字開啟。".into());
    }
    // TXT／MD的文件摘要維持原容量；未知副檔名與程式碼則走有界文字快照。
    let limit = if matches!(ext.as_str(), "txt" | "md") && !source_code::supported(&rel) {
        text::MAX_TEXT
    } else {
        source_code::MAX_BYTES
    };
    if file.metadata().map_err(|e| e.to_string())?.len() > limit as u64 {
        return Err(format!("文件超過讀取上限 {limit} bytes，請縮小來源檔案。"));
    }
    let mut data = Vec::new();
    file.take((limit + 1) as u64)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    text::decode_with_limit(&data, limit).map_err(|error| format!(
        "無法以純文字讀取「{path}」（副檔名：{}）：{error}。目前沒有可用的其他讀取器；可能是二進位、密文或無法確認編碼。未修改原檔。",
        if ext.is_empty() { "無" } else { &ext }))
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

/// 手動儲存與AI匯出共用原生對話識別；同名成果保留舊檔並加序號。
pub fn save_user_chart(project: &Project, conversation: &str, bytes: &[u8]) -> AppResult<String> {
    super::charts::png::validate(bytes)?;
    validate_root(&project.root)?;
    let _root = pin(&project.root)?;
    let base = project.root.join("_AI_Output");
    match fs::create_dir(&base) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(e.to_string()),
    }
    let _base = pin(&base)?;
    let folder_name = conversation_folder_name(conversation)?;
    ensure_output_folder(&base, &folder_name)?;
    let folder = base.join(&folder_name);
    let _folder = pin(&folder)?;
    let (name, mut file) = reserve_output(&folder, "自訂圖表.png")?;
    let path = format!("_AI_Output/{folder_name}/{name}");
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| format!("可能部分寫入 {path}，未完成：{e}"))?;
    file.rewind().map_err(|e| e.to_string())?;
    let mut check = Vec::new();
    Read::by_ref(&mut file)
        .take((super::charts::png::MAX_BYTES + 1) as u64)
        .read_to_end(&mut check)
        .map_err(|e| e.to_string())?;
    if check != bytes {
        return Err(format!("{path} 讀回不一致，未完成。"));
    }
    Ok(path)
}
/// 預覽只記錄根目錄的檔案身分，不長時間鎖住整條路徑而妨礙其他本機保存。
pub(crate) fn directory_identity(path: &Path) -> AppResult<[u32; 3]> {
    pinned_directory_identity(&pin(path)?)
}
fn pinned_directory_identity(guards: &[File]) -> AppResult<[u32; 3]> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };
    let directory = guards.last().ok_or("無法識別專案根目錄。")?;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: 目錄handle仍由guards持有，info為完整可寫結構。
    if unsafe { GetFileInformationByHandle(directory.as_raw_handle(), &mut info) } == 0 {
        return Err("無法取得專案目錄身分，這次不保存截圖。".into());
    }
    if info.nFileIndexHigh == 0 && info.nFileIndexLow == 0 {
        return Err("此檔案系統未提供穩定目錄身分，這次不保存截圖。".into());
    }
    Ok([
        info.dwVolumeSerialNumber,
        info.nFileIndexHigh,
        info.nFileIndexLow,
    ])
}

/// 預覽確認後保存原生截圖；固定輸出目錄、不可覆寫、讀回核對。
pub fn save_user_screenshot(
    project: &Project,
    conversation: &str,
    expected_root: [u32; 3],
    bytes: &[u8],
) -> AppResult<String> {
    if bytes.len() > super::vision::input::MAX_BYTES || !bytes.starts_with(&[255, 216]) {
        return Err("截圖必須為5 MB以內的JPEG。".into());
    }
    // 先以WIC確認實際圖片，不能只相信副檔名或JPEG開頭。
    super::vision::jpeg::convert(bytes, "jpg")?;
    validate_root(&project.root)?;
    let root_guards = pin(&project.root)?;
    if pinned_directory_identity(&root_guards)? != expected_root {
        return Err("專案根目錄已被替換，請重新開始截圖。".into());
    }
    let base = project.root.join("_AI_Output");
    match fs::create_dir(&base) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(e.to_string()),
    }
    let _base = pin(&base)?;
    let folder_name = conversation_folder_name(conversation)?;
    ensure_output_folder(&base, &folder_name)?;
    let folder = base.join(&folder_name);
    let _folder = pin(&folder)?;
    let name = format!("截圖_{}.jpg", crate::jobs::new_id()?);
    let path = format!("_AI_Output/{folder_name}/{name}");
    let mut file = OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(folder.join(&name))
        .map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| format!("可能部分寫入 {path}，未完成：{e}"))?;
    file.rewind().map_err(|e| e.to_string())?;
    let mut check = Vec::new();
    Read::by_ref(&mut file)
        .take((super::vision::input::MAX_BYTES + 1) as u64)
        .read_to_end(&mut check)
        .map_err(|e| e.to_string())?;
    if check != bytes {
        return Err(format!("{path} 讀回不一致，請核對現有圖片。"));
    }
    Ok(path)
}

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
    // 專用圖形匯出保留原本副檔名；一般文字可使用未知或沒有副檔名的檔名。
    let ext = match path.extension().and_then(|s| s.to_str()) {
        Some(ext)
            if ["png", "csv", "mmd", "svg"]
                .iter()
                .any(|value| ext.eq_ignore_ascii_case(value)) =>
        {
            ext.to_lowercase()
        }
        _ => extension(path)?,
    };
    for number in 1..=10000 {
        let candidate = if number == 1 {
            name.to_owned()
        } else if ext.is_empty() {
            format!("{stem}_{number}")
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
    // 某些企業 Shell 未註冊定位介面，固定使用系統Explorer定位成果。
    use std::os::windows::process::CommandExt;
    std::process::Command::new(system_explorer()?)
        .raw_arg(format!("/select,\"{}\"", target.display()))
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("無法開啟檔案總管：{e}；成果位置：{}", target.display()))
}

/// 路徑只取自已授權專案；開啟前再次檢查目錄，不接受前端任意路徑。
pub fn open_folder(project: &Project) -> AppResult<()> {
    validate_root(&project.root)?;
    let root = super::setup::resolve_project_path(&project.root)?;
    let _guards = pin(&root)?;
    std::process::Command::new(system_explorer()?)
        .arg(&root)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("無法開啟專案資料夾：{e}"))
}

/// 固定從Windows目錄取得Explorer，避免受PATH或檔案關聯影響。
fn system_explorer() -> AppResult<PathBuf> {
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
    Ok(PathBuf::from(String::from_utf16_lossy(&windows_dir[..length])).join("explorer.exe"))
}

/// 固定開啟方式同時保護原檔、發布暫存，拒絕連結及操作期間的替換。
pub(crate) fn checked_file(path: &Path) -> AppResult<File> {
    // Windows以一般檔案旗標開啟資料夾會得到模糊的os error 5。
    // 這裡僅改善誤用訊息，不能取代下方handle層的連結／替換驗證。
    if fs::symlink_metadata(path).is_ok_and(|metadata| {
        metadata.is_dir() && metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0
    }) {
        return Err(format!(
            "「{}」是資料夾，不能使用檔案讀取工具。請改用list_files列出此路徑，再讀取清單中的檔案；這不是副檔名不支援，請勿據此判定目錄權限不足。",
            path.file_name().unwrap_or_default().to_string_lossy()
        ));
    }
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
    // 記憶檔替換需要重新開啟父目錄；不要求目錄 DELETE 存取權，避免與
    // 未分享 DELETE 的讀取 handle 衝突。仍以不分享 DELETE 的目錄 handle
    // 防止更名，並用獨占 anchor 防止清空目錄後改為重新解析點。
    pin_named_stage_access(
        path,
        &format!(".anchor_{}", crate::jobs::new_id()?),
        FILE_READ_ATTRIBUTES,
    )
}
/// 共用成果目錄可能同時是CAD來源的父目錄，必須與既有唯讀pin相容。
/// READ_ATTRIBUTES不要求DELETE存取；不分享DELETE的目錄handle仍防止更名，
/// 不分享存取的唯一anchor維持目錄非空，避免被改成重新解析點。
fn pin_output_directory(path: &Path) -> AppResult<StageGuard> {
    pin_named_stage_access(
        path,
        &format!(".anchor_{}", crate::jobs::new_id()?),
        FILE_READ_ATTRIBUTES,
    )
}
fn pin_named_stage(path: &Path, anchor_name: &str) -> AppResult<StageGuard> {
    const DELETE_ACCESS: u32 = 0x0001_0000;
    pin_named_stage_access(path, anchor_name, DELETE_ACCESS)
}
fn pin_named_stage_access(path: &Path, anchor_name: &str, access: u32) -> AppResult<StageGuard> {
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
        .access_mode(access)
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

pub(crate) fn fingerprint(project: &Project, source: &str) -> AppResult<String> {
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
    /// 建立副本時的固定基準；舊紀錄缺少時維持 None。
    #[serde(default)]
    baseline: Option<String>,
    #[serde(default)]
    reviews: BTreeMap<String, super::review::Snapshot>,
    office: Option<OfficeCopy>,
    name: String,
    text: String,
    encoding: Encoding,
    saved_revision: Option<String>,
    /// 只對目前內容版本有效；任何文字修改都需重新語法檢查。
    #[serde(default)]
    python_checked_revision: Option<String>,
    #[serde(default)]
    code_review: super::code_review::State,
    #[serde(default)]
    draft: Option<drafts::Draft>,
    /// 目前正在編輯／驗證的小段；與副本一起加密保存，避免查範例後遺失原文。
    #[serde(default)]
    code_sections: Vec<code_sections::Section>,
    paths: Vec<String>,
}
#[derive(serde::Serialize, serde::Deserialize)]
struct ChartExport {
    chart_index: usize,
    name: String,
    path: String,
    sha256: String,
    /// 圖表轉換後，同名 PNG 不可誤用舊座標成果；舊紀錄仍可驗證，但不重用。
    #[serde(default)]
    chart_revision: String,
}
/// 只保存資料，不保存授權、Token、COM 物件或執行中的程序。
#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct SavedBroker {
    #[serde(default)]
    translations: BTreeMap<String, super::documents::Translation>,
    #[serde(default)]
    delivery_plan: super::deliverables::Plan,
    #[serde(default)]
    work_context: Option<super::work_settings::Snapshot>,
    #[serde(default)]
    comparison_records: BTreeMap<String, Value>,
    #[serde(default)]
    comparison_artifacts: Vec<super::python::Artifact>,
    #[serde(default)]
    source_versions: BTreeMap<String, String>,
    #[serde(default)]
    workflow: Option<super::workflows::Active>,
    #[serde(default)]
    report_values: super::report_values::State,
    #[serde(default)]
    reading: super::reading::Notebook,
    #[serde(default)]
    analysis: super::analysis::State,
    #[serde(default)]
    excel_plans: BTreeMap<String, super::excel_plan::Plan>,
    #[serde(default)]
    outlook: super::mail::Saved,
    #[serde(default)]
    mail_notes: super::mail_notes::State,
    #[serde(default)]
    mail_draft: Option<drafts::Draft>,
    #[serde(default)]
    log_cursors: BTreeMap<String, super::logs::Cursor>,
    output_folder: Option<String>,
    copies: BTreeMap<String, Copy>,
    results: BTreeMap<String, (Value, Value)>,
    #[serde(default)]
    archived_results: BTreeMap<String, Value>,
    #[serde(default)]
    work_log_index: Option<Vec<Value>>,
    #[serde(default)]
    references: references::References,
    published: Vec<String>,
    txt_context: bool,
    #[serde(default)]
    loaded_skills: Vec<String>,
    #[serde(default)]
    charts: Vec<super::charts::Chart>,
    #[serde(default)]
    diagrams: Vec<super::diagrams::Diagram>,
    #[serde(default)]
    diagram_sources: Vec<diagrams::SourceSlice>,
    #[serde(default)]
    diagram_exports: Vec<super::diagrams::Export>,
    #[serde(default)]
    chart_exports: Vec<ChartExport>,
    #[serde(default)]
    datasets: Vec<super::datasets::Reference>,
    #[serde(default)]
    python_artifacts: Vec<super::python::Artifact>,
    #[serde(default)]
    cad_artifacts: Vec<super::python::Artifact>,
    #[serde(default)]
    cad_failed: bool,
    #[serde(default)]
    cad_last_result: Option<Value>,
    #[serde(default)]
    cad_design: cad_design::Design,
    #[serde(default)]
    cad_attempts: u32,
    #[serde(default)]
    cad_partial_plans: Vec<String>,
    #[serde(default)]
    cad_reads: BTreeMap<String, Value>,
    #[serde(default)]
    cad_copy_version: Option<String>,
    #[serde(default)]
    cad_previews: BTreeMap<String, Value>,
}
pub struct Broker {
    translations: BTreeMap<String, super::documents::Translation>,
    delivery_plan: super::deliverables::Plan,
    work_context: Option<super::work_settings::Snapshot>,
    comparison_records: BTreeMap<String, Value>,
    comparison_artifacts: Vec<super::python::Artifact>,
    source_versions: BTreeMap<String, String>,
    pub(crate) workflow: Option<super::workflows::Active>,
    pub(crate) report_values: super::report_values::State,
    pub(crate) reading: super::reading::Notebook,
    model: String,
    task_id: String,
    pub(super) analysis: super::analysis::State,
    excel_plans: BTreeMap<String, super::excel_plan::Plan>,
    file_waiter: Option<super::interaction::FileWaiter>,
    cad_waiter: Option<crate::cad::Waiter>,
    cad_progress: Option<crate::cad::Progress>,
    cad_active: Option<Value>,
    cad_session: cad_session::ActiveSession,
    // 完整快照只活在本次 Broker；重啟／續接須重讀，不恢復作用中文件寫入授權。
    cad_snapshot: Option<super::cad_index::Snapshot>,
    // 不支援介面的錯誤不能因模型遺忘歷史而持續送往 COM。
    cad_active_unsupported: Option<String>,
    outlook: super::mail::Session,
    outlook_process: Option<crate::outlook::process::Process>,
    pub(super) mail_notes: super::mail_notes::State,
    mail_draft: Option<drafts::Draft>,
    log_cursors: BTreeMap<String, super::logs::Cursor>,
    server_pdf: Option<super::server_pdf::Reader>,
    memory: Option<super::memory::Memory>,
    project: Project,
    /// 僅由原生runner的對話ID設定，模型不能指定；重啟後可推導相同資料夾。
    output_conversation: Option<String>,
    output_folder: Option<String>,
    copies: BTreeMap<String, Copy>,
    /// 同一 operation_id 只能配對同一份工具參數，重送僅回傳已記錄的結果。
    results: BTreeMap<String, (Value, Value)>,
    archived_results: BTreeMap<String, Value>,
    work_log_index: Option<Vec<Value>>,
    references: references::References,
    published: Vec<String>,
    /// 任務接觸 TXT 後，不允許把內容混入未加密的 MD 成果。
    txt_context: bool,
    loaded_skills: Vec<String>,
    charts: Vec<super::charts::Chart>,
    diagrams: Vec<super::diagrams::Diagram>,
    diagram_sources: Vec<diagrams::SourceSlice>,
    diagram_renderer: Option<super::diagrams::Renderer>,
    diagram_exports: Vec<super::diagrams::Export>,
    previous_diagrams: BTreeMap<usize, Vec<super::diagrams::Diagram>>,
    chart_exports: Vec<ChartExport>,
    datasets: Vec<super::datasets::Reference>,
    python_artifacts: Vec<super::python::Artifact>,
    cad_artifacts: Vec<super::python::Artifact>,
    cad_failed: bool,
    /// 新建圖面的最後原生結果摘要；不保存完整圖元或恢復目前文件寫入授權。
    cad_last_result: Option<Value>,
    cad_design: cad_design::Design,
    cad_attempts: u32,
    cad_partial_plans: Vec<String>,
    cad_reads: BTreeMap<String, Value>,
    cad_copy_version: Option<String>,
    cad_previews: BTreeMap<String, Value>,
    png_renderer: Option<super::charts::png::Renderer>,
    chart_chooser: Option<super::charts::quality::Chooser>,
    preferences: Option<super::steering::Inbox>,
    chart_deadline: std::time::Instant,
    /// 只來自同一對話的歷史，按需查看樣式；不把完整數列重送模型。
    previous_charts: BTreeMap<usize, Vec<super::charts::Chart>>,
    chart_palette: Vec<String>,
}
impl Broker {
    /// 新任務啟用批後閱讀成果契約；舊checkpoint不隱式變更協定。
    pub fn enable_reading_notes(&mut self) {
        self.reading.enabled = true;
    }
    /// 來源筆記涵蓋原件文字、Office、PDF與圖片；副本驗證及郵件仍用各自契約。
    fn observe_reading(&mut self, id: &str, tool: &Tool, result: &Value) -> AppResult<()> {
        self.references.register(id);
        if let Tool::AnalyzeImage { .. } = tool {
            return self.reading.observe_image(id, result);
        }
        if let Tool::ListFiles { path, offset } = tool {
            self.reading.remember_directory(path, *offset, result);
            return Ok(());
        }
        let path = match tool {
            Tool::ReadCodeSection { path, .. } => path,
            Tool::ReadFile { path, .. } | Tool::ReadDocumentSection { path, .. } => path,
            _ => return Ok(()),
        };
        if self.code_copy_id(path).is_some() {
            return Ok(());
        }
        self.reading.observe(id, result)
    }

    /// 只追蹤原生工具確實讀取的來源，採任務範圍保守關聯，避免猜測單一數字因果。
    fn observe_source_versions(&mut self, tool: &Tool, result: &Value) {
        if result["ok"] != true {
            return;
        }
        let mut paths = Vec::new();
        match tool {
            Tool::ReadFile { path, .. }
            | Tool::ReadCodeSection { path, .. }
            | Tool::ReadExcelRange { path, .. }
            | Tool::InspectExcel { path, .. }
            | Tool::ReadDocumentSection { path, .. } => paths.push(path.clone()),
            Tool::RunPython { inputs, .. } => paths.extend(inputs.iter().map(|s| s.path.clone())),
            _ => (),
        }
        for path in paths {
            if !self.copies.contains_key(&path) {
                if let Ok(version) = fingerprint(&self.project, &path) {
                    self.source_versions.insert(path, version);
                }
            }
        }
    }
    pub(crate) fn register_artifacts(&self, paths: &[String]) -> AppResult<()> {
        if self.output_conversation.is_none() {
            return Ok(());
        }
        let conversation = self.output_conversation.as_deref().unwrap();
        let mut entries = Vec::new();
        for path in paths {
            entries.push(super::artifacts::entry(
                &self.project,
                path,
                &self.task_id,
                conversation,
                self.artifact_sources(path),
                false,
            )?);
        }
        self.memory()?.store_artifacts(&entries)
    }

    /// 匯出舊圖時沿用該圖原生綁定的來源版本，避免新任務沒有讀檔便遺失追溯。
    /// 其餘來源仍採本任務的保守集合，不宣稱已證明每一條因果關係。
    fn artifact_sources(&self, path: &str) -> BTreeMap<String, String> {
        let mut sources = self.source_versions.clone();
        let diagram = self.diagrams.iter().find(|d| d.path == path).or_else(|| {
            let source = self
                .diagram_exports
                .iter()
                .find(|e| e.path == path)?
                .source
                .as_ref()?;
            let diagram = match source.message_index {
                Some(message) => self
                    .previous_diagrams
                    .get(&message)?
                    .get(source.diagram_index),
                None => self.diagrams.get(source.diagram_index),
            }?;
            (diagram.revision == source.revision).then_some(diagram)
        });
        if let Some(diagram) = diagram {
            for origin in &diagram.node_sources {
                sources.insert(origin.path.clone(), origin.source_sha256.clone());
            }
        }
        sources
    }

    /// 正式代理任務啟用伺服器 PDF；本機診斷測試仍可使用原生解析器。
    pub fn enable_server_pdf(
        &mut self,
        config: crate::config::Config,
        session: crate::storage::Session,
    ) -> AppResult<()> {
        self.model = config.model.clone();
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
                "code_state":copy.name.to_ascii_lowercase().ends_with(".py").then(||code_sections::code_state(copy)),
                "saved_revision":copy.saved_revision,"paths":copy.paths,"draft_path":copy.draft.as_ref().map(|d|&d.path),"syntax_checked":copy.python_checked_revision.as_ref()==Some(&text::revision(&copy.text)),"requirements":copy.code_review.requirements.iter().map(|r|json!({"id":r.id,"origin":r.origin,"description":r.description.chars().take(100).collect::<String>()})).collect::<Vec<_>>(),"review_current":copy.code_review.reviewed_revision.as_ref()==Some(&text::revision(&copy.text)),"test_status":copy.code_review.tests.iter().map(|t|json!({"id":t["id"],"status":t["status"]})).collect::<Vec<_>>()
            }))
            .collect::<Vec<_>>())
    }
    /// 可重用資料集只附路徑與結構，不在續接快照重送所有資料列。
    pub(super) fn dataset_index(&self) -> Value {
        json!(self.datasets)
    }
    /// 生成的 XLSX 不是工作副本；精簡／續接仍需提供其確定的路徑。
    pub(super) fn python_artifact_index(&self) -> Value {
        json!(self.python_artifacts)
    }
    pub(super) fn cad_artifact_index(&self) -> Value {
        json!({"artifacts":self.cad_artifacts,"blocked_after_error":self.cad_failed,
            "design_planning":self.cad_design.context(),
            "last_execution":self.cad_last_result,"visual_check_required":true,
            "next_action":if self.cad_failed {"finish回報最後原生結果及實際存檔狀態；不重畫、不換CAD或改用ASCII重試。"} else {"依原要求及原生結果繼續；新設計不用讀無關的目前圖面。"}})
    }
    /// 精簡上下文後仍提供確定的欄位角色，不附原始資料。
    pub(super) fn excel_plan_index(&self) -> Value {
        // 每輪只带識別碼及欄位角色；原始要求／理由已在規劃與操作簿保存，
        // 不隨檔案數量反覆塞回上下文。
        json!(self
            .excel_plans
            .values()
            .map(|p| json!({
                "plan_id":p.id,"path":p.proposal.path,"revision":p.proposal.revision,
                "sheet":p.proposal.sheet,"header_row":p.proposal.header_row,
                "x":p.proposal.x,"y":p.proposal.y,"time":p.proposal.time,
                "time_mode":p.proposal.time_mode,"y_kind":p.proposal.y_kind
            }))
            .collect::<Vec<_>>())
    }
    pub(super) fn skill_context(&self, model: &str) -> AppResult<String> {
        let mut context = super::skills::context_for(&self.loaded_skills, model)?;
        if self.loaded_skills.iter().any(|id| id == "office-edit")
            && !self.copies.values().any(|copy| copy.office.is_some())
        {
            context.push_str("\n目前尚未建立Office工作副本。下一步可直接create_working_copy(source=null,name=\"報告.docx\")建立空白Word；Excel用.xlsx，PowerPoint用.pptx。建立成功後會自動提供office_action／office_batch與save_copy。JSON或TXT也用create_working_copy(source=null,name=\"摘要.json\")建立，再以edit_text寫入；不需反覆load_skill或等待新的工具。交付清單只是待辦登記，必須實際建立／編輯／保存檔案。");
        }
        if self.cad_design.pending() && self.loaded_skills.iter().any(|id| id == "cad-new") {
            context.push_str("\n目前為新圖規劃階段，暫不提供cad_draw及其完整參數；不是缺少CAD能力。必要尺寸未齊先ask_user，條件足夠就compact_context保存短方案，下一輪才提供cad_plan_start分段工具。這一輪不展開全部座標。簡單新圖只需1–2句方案。不要重載技能找繪圖工具。");
        }
        if self.loaded_skills.iter().any(|id| id == "mermaid") {
            context.push_str(&format!(
                "\n本對話圖形索引（改圖先讀完整原始碼）：{}",
                self.inspect_mermaid(None, None)?
            ));
        }
        Ok(context)
    }

    /// 只在真正切入新圖技能時開始規劃；重複載入不清除已保存方案。
    fn activate_skill(&mut self, id: &str) -> AppResult<()> {
        let already_loaded = self.loaded_skills.iter().any(|loaded| loaded == id);
        super::skills::activate(&mut self.loaded_skills, id)?;
        if id == "cad-new" && !already_loaded {
            self.cad_design.begin();
        }
        Ok(())
    }
    pub fn charts(&self) -> &[super::charts::Chart] {
        &self.charts
    }
    /// 新一輪只公告目前有實際操作對象的工具；不改寫已提交／待查回的請求。
    pub(super) fn restrict_tools(&self, request: &mut Value) {
        if let Some(tools) = request["tools"].as_array_mut() {
            tools.retain(|tool| {
                let name = tool["function"]["name"].as_str().unwrap_or("");
                if self.reading.enabled
                    && !self.reading.pending.is_empty()
                    && !matches!(name, "record_reading_notes" | "read_reading_notes")
                {
                    return false;
                }
                if !super::skills::enabled(name, &self.loaded_skills) {
                    return false;
                }
                if name == "cad_draw"
                    && (self.cad_design.pending()
                        || self.cad_design.staged()
                        || self.cad_design.plan.is_some())
                {
                    return false;
                }
                if name.starts_with("cad_plan_") {
                    if self.cad_design.pending() {
                        return false;
                    }
                    if name != "cad_plan_start" && self.cad_design.plan.is_none() {
                        return false;
                    }
                    if name == "cad_plan_inspect"
                        && self
                            .cad_design
                            .plan
                            .as_ref()
                            .is_some_and(|p| p.inspectable_parts().is_empty())
                    {
                        return false;
                    }
                    if name == "cad_plan_start"
                        && self.cad_design.plan.as_ref().is_some_and(|p| !p.finished())
                    {
                        return false;
                    }
                    if matches!(name, "cad_plan_part" | "cad_plan_commit")
                        && self.cad_design.plan.as_ref().is_some_and(|p| p.finished())
                    {
                        return false;
                    }
                }
                // 新模型只看到本機索引流程；舊入口仍供已提交請求／歷史相容。
                if matches!(name, "cad_active_read" | "cad_read") {
                    return false;
                }
                if (self.cad_session.stopped() || self.cad_failed)
                    && name.starts_with("cad_")
                    && name != "cad_inventory"
                {
                    return false;
                }
                // 修改後舊快照已撤銷；不要再公告必須持有快照／授權的工具，
                // 避免模型為額外核對而憑空產生 snapshot_id。需要新量測仍可先讀快照。
                if self.cad_snapshot.is_none() && matches!(name, "cad_query" | "cad_preview") {
                    return false;
                }
                if self.cad_active.is_none() && name == "cad_active_edit" {
                    return false;
                }
                match name {
                    "read_mail_notes" => {
                        !self.mail_notes.notes.is_empty() || self.mail_notes.pending.is_some()
                    }
                    "inspect_chart" | "edit_chart" => {
                        !self.charts.is_empty() || !self.previous_charts.is_empty()
                    }
                    "export_chart_png"
                    | "transform_chart"
                    | "set_chart_policy"
                    | "set_chart_reference_lines" => !self.charts.is_empty(),
                    "edit_text" | "edit_code_section" => {
                        self.copies.values().any(|c| c.office.is_none())
                    }
                    "edit_office" | "office_action" | "office_batch" => {
                        self.copies.values().any(|c| c.office.is_some())
                    }
                    "save_copy" | "delete_copy" => !self.copies.is_empty(),
                    _ => true,
                }
            });
            if let Some(plan) = &self.cad_design.plan {
                let mut missing: Vec<Value> = plan
                    .inspectable_parts()
                    .into_iter()
                    .map(Value::String)
                    .collect();
                missing.push(Value::Null);
                for tool in tools
                    .iter_mut()
                    .filter(|t| t["function"]["name"] == "cad_plan_inspect")
                {
                    tool["function"]["parameters"]["properties"]["part_id"]["enum"] =
                        json!(missing);
                }
            }
            if self.cad_failed {
                for tool in tools.iter_mut() {
                    if tool["function"]["name"] == "finish" {
                        tool["function"]["parameters"]["properties"]["message"]["description"] = json!("依cad_artifacts.last_execution簡短回報實際失敗步驟。drawing_started=true或completed_operations>0表示已建立圖面或曾執行操作；saved=false/file_exists=false只代表未存檔，不能說未建圖。說明部分圖面待人工檢查，不推測未證實原因，不建議ASCII、換CAD或重畫繞過manual_check。");
                    }
                }
            }
            if self.loaded_skills.iter().any(|id| id == "cad-active") {
                let edited = self.cad_session.context()["last_edit"]["status"] == "EDITED_UNSAVED";
                for tool in tools {
                    let note = &mut tool["function"]["parameters"]["properties"]["progress_note"];
                    if note.is_object() {
                        note["description"] = json!("簡短80–150字記本輪新增進展及下一步；精確ID／圖元資料由CAD常駐狀態提供，不反覆抄寫，不把待執行說成完成。");
                    }
                    if edited && tool["function"]["name"] == "finish" {
                        // 完成狀態靠原生證據；避免小模型在結尾重述未要求的外框、孔位而猜錯。
                        tool["function"]["parameters"]["properties"]["message"]["description"] = json!("依原生修改結果簡短交付。單一圖元修改用2–3句，只說目標、實際舊值→新值與未儲存／未關閉狀態；必要限制才補充。不重述外框、其他孔、未核對的圖元類型、輪廓編號或內部驗證欄位。輪廓／候選編號不是Handle。");
                    }
                }
            }
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
    pub fn set_cad_waiter(&mut self, waiter: Option<crate::cad::Waiter>) {
        self.cad_waiter = waiter;
    }

    /// 進度只供桌面顯示，不放入模型上下文，也不建立任何寫入授權。
    pub fn set_cad_progress(&mut self, progress: Option<crate::cad::Progress>) {
        self.cad_progress = progress;
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

    /// 明確指定檔案，逐檔使用與 read_file 相同的存取邊界；不掃描未知目錄。
    fn search_files(
        &mut self,
        paths: &[String],
        query: &str,
        worker: &mut Worker,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        // 程式碼以純文字循序掃描，不啟動 Office／PDF；允許較大批次但不擴大結果上限。
        let source_only = paths
            .iter()
            .all(|path| is_plain_text_input(Path::new(path)));
        let max_files = if source_only { 100 } else { 20 };
        if paths.is_empty()
            || paths.len() > max_files
            || query.trim().is_empty()
            || query.chars().count() > 200
        {
            return Err(format!("搜尋需指定1–{max_files}個明確檔案及1–200字查詢；全部為支援的程式碼時最多100檔，含一般文件時最多20檔。更多檔案請拆批，不接受資料夾或萬用字元，勿原樣重試。"));
        }
        let mut matches = Vec::new();
        let mut errors = Vec::new();
        let mut coverage = Vec::new();
        let mut truncated = false;
        for path in paths {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err("操作已取消。".into());
            }
            if path.contains('*') || path.contains('?') || path.ends_with(['/', '\\']) {
                let error = "搜尋只接受明確檔名；請先 list_files 列出資料夾，全程式碼每批最多100檔、混合文件最多20檔，不支援萬用字元。";
                coverage.push(json!({"path":path,"status":"failed","matches":null,"detail":error}));
                errors.push(json!({"path":path,"error":error}));
                continue;
            }
            match self.content(path, cancel, worker) {
                Ok(content) => {
                    let revision = text::revision(&content);
                    let blocks: Vec<(String, String, Option<String>)> =
                        if let Ok(snapshot) = serde_json::from_str::<office::Snapshot>(&content) {
                            snapshot
                                .blocks
                                .into_iter()
                                .map(|b| (b.label, b.text, Some(b.id)))
                                .collect()
                        } else {
                            vec![("文件原文".into(), content.clone(), None)]
                        };
                    let mut total = 0usize;
                    let mut omitted = 0usize;
                    for (label, body, block_id) in blocks {
                        let body_chars = body.chars().count();
                        for (byte, _) in body.match_indices(query) {
                            total += 1;
                            if total.is_multiple_of(4096)
                                && cancel.load(std::sync::atomic::Ordering::Relaxed)
                            {
                                return Err("操作已取消。".into());
                            }
                            if matches.len() >= 60 {
                                truncated = true;
                                omitted += 1;
                                continue;
                            }
                            let offset = body[..byte].chars().count();
                            let start = offset.saturating_sub(500);
                            let context: String = body.chars().skip(start).take(2000).collect();
                            matches.push(json!({"path":path,"revision":revision,"offset":offset,"label":label,
                                "context_start":start,"excerpt":context,"query":query,"block_id":block_id,
                                "offset_basis":if block_id.is_some(){"block_text"}else{"document_text"},
                                "excerpt_truncated":start>0 || body_chars>start+2000,
                                "line":body[..byte].bytes().filter(|b|*b==b'\n').count()+1}));
                        }
                    }
                    coverage.push(json!({"path":path,"revision":revision,"matches":total,"omitted":omitted,
                        "status":if omitted>0 {"partial"}else{"complete"},"detail":if omitted>0 {"已掃描，但部分命中未列出；縮小搜尋範圍可查回。"}else{"此查詢掃描完成；零命中不代表其他用詞不存在。"}}));
                }
                Err(error) if error.contains(super::interaction::DEFERRED) => return Err(error),
                Err(error) => {
                    coverage
                        .push(json!({"path":path,"status":"failed","matches":null,"detail":error}));
                    errors.push(json!({"path":path,"error":error}));
                }
            }
        }
        Ok(
            json!({"matches":matches,"coverage":coverage,"errors":errors,"truncated":truncated,"complete":errors.is_empty()&&!truncated,
                "application_version":env!("CARGO_PKG_VERSION"),"source_code_supported":true,"max_files":max_files}),
        )
    }
    pub fn published(&self) -> &[String] {
        &self.published
    }

    pub(super) fn content(
        &mut self,
        path: &str,
        cancel: &AtomicBool,
        worker: &mut Worker,
    ) -> AppResult<String> {
        let canonical = self.code_copy_id(path).unwrap_or_else(|| path.into());
        let path = canonical.as_str();
        if let Some(copy) = self.copies.get(path) {
            if extension(Path::new(&copy.name))? != "md" {
                self.txt_context = true;
            }
            return Ok(copy.text.clone());
        }
        if readable_extension(Path::new(path))? != "md" {
            self.txt_context = true;
        }
        // 程式檔以分頁原文及 revision 查閱，不把整個程式庫複製到200份文件摘要快取。
        let cache_document = uses_document_memory(Path::new(path));
        let stamp = self
            .memory
            .as_ref()
            .filter(|_| cache_document)
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

    /// 舊工具保留給既有任務；已規劃的來源必須走帶欄位鎖的匯出流程。
    fn require_planned_export(&self, path: &str, sheet: usize) -> AppResult<()> {
        let canonical = relative(path)?.to_string_lossy().to_lowercase();
        if self.excel_plans.values().any(|p| {
            p.proposal.sheet == sheet
                && relative(&p.proposal.path)
                    .is_ok_and(|v| v.to_string_lossy().to_lowercase() == canonical)
        }) {
            return Err("此Excel已建立欄位規劃，請用export_planned_excel，之後chart_dataset會核對鎖定的X/Y；不要改走未帶規劃的舊工具。".into());
        }
        Ok(())
    }
    fn outlook_tool(
        &mut self,
        tool: &Tool,
        executable: &Path,
        cancel: &AtomicBool,
        notify: &mut dyn FnMut(&str),
    ) -> AppResult<Value> {
        if let Tool::OutlookRead { mail_id, offset } = tool {
            self.txt_context = true;
            if let Some(note) = self.reuse_mail_note(mail_id, *offset)? {
                return Ok(note);
            }
        }
        let mut reader = crate::outlook::process::Reader {
            process: &mut self.outlook_process,
            executable,
            progress: notify,
            deadline: crate::outlook::process::Reader::deadline(),
        };
        match tool {
            Tool::OutlookFolders {
                scope,
                parent_id,
                offset,
            } => self
                .outlook
                .folders(&mut reader, scope, parent_id.as_deref(), *offset, cancel),
            Tool::OutlookHeaders {
                folder_id,
                start_date,
                end_date,
                cursor,
            } => self.outlook.headers(
                &mut reader,
                folder_id,
                start_date,
                end_date,
                cursor.as_deref(),
                cancel,
            ),
            Tool::OutlookCompare { mail_ids, offset } => {
                self.outlook.compare(&mut reader, mail_ids, *offset, cancel)
            }
            Tool::OutlookRead { mail_id, offset } => {
                self.outlook.body(&mut reader, mail_id, *offset, cancel)
            }
            _ => Err("不是Outlook唯讀工具。".into()),
        }
    }
}
