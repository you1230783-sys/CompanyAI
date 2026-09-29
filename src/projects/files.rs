//! 固定檔案 broker：只接受專案相對路徑；逐層鎖住目錄、拒絕重新解析點與硬連結。
//! 工作副本先留在記憶體，發布只用 create_new；原始文件從未取得可寫 handle。
use super::{
    sandbox::{Edit, Worker},
    text::{self, Encoding},
    Project, Tool,
};
use crate::AppResult;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
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
fn pin(path: &Path) -> AppResult<Vec<File>> {
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

pub fn validate_root(root: &Path) -> AppResult<()> {
    if !root.is_absolute()
        || root
            .components()
            .filter(|p| matches!(p, Component::Normal(_)))
            .count()
            == 0
    {
        return Err("請選擇本機的專案資料夾，不可授權整個磁碟。".into());
    }
    let _guards = pin(root)?;
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
                if name.ends_with(['.', ' '])
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
    Ok(path.to_path_buf())
}
fn extension(path: &Path) -> AppResult<String> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(ext.as_str(), "txt" | "md") {
        return Err("第一版只支援 TXT 與 MD。".into());
    }
    Ok(ext)
}

pub fn read(project: &Project, path: &str) -> AppResult<(String, Encoding)> {
    let rel = relative(path)?;
    extension(&rel)?;
    let target = project.root.join(&rel);
    let _guards = pin(target.parent().ok_or("缺少來源資料夾。")?)?;
    let mut file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(&target)
        .map_err(|_| "文件不存在、被占用或無讀取權限。")?;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0
        || info.dwFileAttributes & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY) != 0
        || info.nNumberOfLinks != 1
    {
        return Err("拒絕讀取連結、目錄或無法驗證的檔案。".into());
    }
    // 匯入文字是使用者本次明確提供的快照；不宣稱即時同步原檔。
    let key = rel.to_string_lossy().replace('\\', "/");
    if let Some(text) = project.imports.get(&key) {
        text::validate(text)?;
        return Ok((text.clone(), Encoding::Utf8(true)));
    }
    if file.metadata().map_err(|e| e.to_string())?.len() > text::MAX_TEXT as u64 {
        return Err("文件超過第一版 200 KB 上限。".into());
    }
    let mut data = Vec::new();
    file.read_to_end(&mut data).map_err(|e| e.to_string())?;
    text::decode(&data)
}

struct Copy {
    name: String,
    text: String,
    encoding: Encoding,
    saved_revision: Option<String>,
    paths: Vec<String>,
}
pub struct Broker {
    project: Project,
    task: String,
    copies: BTreeMap<String, Copy>,
    /// 同一 operation_id 只能配對同一份工具參數，重送僅回傳已記錄的結果。
    results: BTreeMap<String, (Value, Value)>,
    published: Vec<String>,
    /// 任務接觸 TXT 後，不允許把內容混入未加密的 MD 成果。
    txt_context: bool,
}
impl Broker {
    pub fn new(project: Project, task: String) -> AppResult<Self> {
        validate_root(&project.root)?;
        Ok(Self {
            project,
            task,
            copies: BTreeMap::new(),
            results: BTreeMap::new(),
            published: Vec::new(),
            txt_context: false,
        })
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
    fn content(&mut self, path: &str) -> AppResult<String> {
        if let Some(copy) = self.copies.get(path) {
            if extension(Path::new(&copy.name))? == "txt" {
                self.txt_context = true;
            }
            return Ok(copy.text.clone());
        }
        if extension(Path::new(path))? == "txt" {
            self.txt_context = true;
        }
        read(&self.project, path).map(|(text, _)| text)
    }
    fn perform(
        &mut self,
        tool: &Tool,
        worker: &mut Worker,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        match tool {
            Tool::ListFiles { path } => {
                let target = self.project.root.join(relative(path)?);
                let _guards = pin(&target)?;
                let mut entries = Vec::new();
                let mut truncated = false;
                for (scanned, entry) in fs::read_dir(&target)
                    .map_err(|e| e.to_string())?
                    .enumerate()
                {
                    let entry = entry.map_err(|e| e.to_string())?;
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
                let content = self.content(path)?;
                let total = content.chars().count();
                if *offset > total {
                    return Err("讀取位置超過全文。".into());
                }
                let text: String = content.chars().skip(*offset).take(6000).collect();
                let next = offset + text.chars().count();
                Ok(
                    json!({"text":text,"offset":offset,"next_offset":next,"total":total,"truncated":next<total,"revision":text::revision(&content),"imported_snapshot":self.project.imports.contains_key(&path.replace('\\', "/"))}),
                )
            }
            Tool::FindText { path, text: needle } => {
                if needle.is_empty() {
                    return Err("搜尋文字不可空白。".into());
                }
                let content = self.content(path)?;
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
                let (content, encoding) = if let Some(source) = source {
                    if ext == "txt" {
                        self.txt_context = true;
                    }
                    if extension(Path::new(source))? != ext {
                        return Err("副本需保留來源 TXT／MD 格式。".into());
                    }
                    read(&self.project, source)?
                } else {
                    if ext != "txt" {
                        return Err("新的一般成果只建立 TXT；MD 僅允許既有 MD 的修訂副本。".into());
                    }
                    (String::new(), Encoding::Utf8(true))
                };
                let id = crate::jobs::new_id()?;
                let revision = text::revision(&content);
                self.copies.insert(
                    id.clone(),
                    Copy {
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
                let bytes = text::encode(&copy.text, copy.encoding)?;
                let _root = pin(&self.project.root)?;
                let base = self.project.root.join("_AI_Output");
                match fs::create_dir(&base) {
                    Ok(()) => (),
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
                    Err(e) => return Err(e.to_string()),
                }
                let _base = pin(&base)?;
                let folder = base.join(&self.task);
                match fs::create_dir(&folder) {
                    Ok(()) => (),
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
                    Err(e) => return Err(e.to_string()),
                }
                let _folder = pin(&folder)?;
                let name = format!("{}_{}", crate::jobs::new_id()?, copy.name);
                let target = folder.join(&name);
                let mut output = OpenOptions::new()
                    .write(true)
                    .read(true)
                    .create_new(true)
                    .share_mode(0)
                    .open(&target)
                    .map_err(|e| e.to_string())?;
                // 先記錄可能已建立的路徑；失敗時明確保留，絕不假裝沒有副作用或直接刪除。
                let relative = format!("_AI_Output/{}/{name}", self.task);
                self.published.push(relative.clone());
                output
                    .write_all(&bytes)
                    .and_then(|_| output.sync_all())
                    .map_err(|e| format!("可能有部分輸出 {relative}，未交付：{e}"))?;
                drop(output);
                let verified = read(&self.project, &relative).map_err(|e| format!("已建立 {relative}，但無法驗證加密後內容，尚未交付。請用記事本檢查：{e}"))?.0;
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
            if read(&self.project, path)?.0 != copy.text {
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
        assert!(relative("資料/文件.txt").is_ok());
    }
}
