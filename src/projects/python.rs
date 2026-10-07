//! Python 分析的本機資料橋接。原檔唯讀，模型程式在無網路 AppContainer 執行。
//! 不將 COM 物件或原始路徑權限交給 Python；只傳核准的值與來源資訊。
use super::{datasets, files, sandbox::Worker, Project};
use crate::AppResult;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

pub(super) mod encoding;

pub const MAX_INPUT: usize = 32 * 1024 * 1024;
pub const MAX_OUTPUT: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub revision: Option<String>,
    /// 文字來源的明確編碼；缺省沿用依副檔名決定的自動模式。
    pub encoding: Option<String>,
    /// LOG 大檔以完整行分段；仍由原生層讀取，不開放 Python 任意路徑。
    pub log_range: Option<LogRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogRange {
    pub start_line: usize,
    pub line_count: usize,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub path: String,
    pub sha256: String,
}

#[derive(Deserialize)]
struct RuntimeFile {
    path: String,
    sha256: String,
    size: u64,
}

/// 與 EXE 分開安裝；不尋找 PATH、登錄或使用者的 Python。
pub fn runtime_root() -> AppResult<PathBuf> {
    Ok(std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("找不到安裝目錄。")?
        .join("python"))
}

/// 全部檔案都需符合編入 EXE 的清單雜湊，且在執行期間持有唯讀鎖。
/// 缺少或不一致時要求完整 NSIS 修復，不退回使用者的環境。
fn verified_runtime(root: &Path, cancel: &AtomicBool) -> AppResult<Vec<File>> {
    let mut guards =
        files::pin(root).map_err(|_| "缺少 Python 環境；請使用本版完整 NSIS 安裝包。")?;
    let manifest = read_locked(root, "runtime-manifest.json", 2 * 1024 * 1024, &mut guards)?;
    if format!("{:x}", Sha256::digest(&manifest))
        != include_str!("../../assets/python-runtime-sha256.txt").trim()
    {
        return Err("Python 環境版本不符；請重新安裝本版完整 NSIS。".into());
    }
    let entries: Vec<RuntimeFile> = serde_json::from_slice(&manifest).map_err(|e| e.to_string())?;
    // 先固定每個目錄並列出完整檔案集合，拒絕清單外模組（含舊版殘留）。
    // 每個目錄只驗證一次，避免數千個套件檔案重複開啟相同父目錄。
    let mut actual = std::collections::BTreeSet::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for item in std::fs::read_dir(&directory).map_err(|e| e.to_string())? {
            if cancel.load(Ordering::Relaxed) {
                return Err("Python 驗證已取消。".into());
            }
            let item = item.map_err(|e| e.to_string())?;
            let path = item.path();
            if item.file_type().map_err(|e| e.to_string())?.is_dir() {
                guards.extend(files::pin(&path)?);
                pending.push(path);
            } else {
                actual.insert(
                    path.strip_prefix(root)
                        .map_err(|e| e.to_string())?
                        .to_path_buf(),
                );
            }
        }
    }
    let mut expected = std::collections::BTreeSet::from([PathBuf::from("runtime-manifest.json")]);
    for entry in &entries {
        if !expected.insert(files::relative(&entry.path)?) {
            return Err("Python 清單含重複檔案。".into());
        }
    }
    if actual != expected {
        return Err("Python 目錄包含缺漏或清單外檔案；請使用完整 NSIS 修復。".into());
    }
    for entry in entries {
        if cancel.load(Ordering::Relaxed) {
            return Err("Python 驗證已取消。".into());
        }
        let bytes = read_pinned(
            &root.join(files::relative(&entry.path)?),
            64 * 1024 * 1024,
            &mut guards,
        )?;
        if bytes.len() as u64 != entry.size
            || format!("{:x}", Sha256::digest(&bytes)) != entry.sha256
        {
            return Err(format!(
                "Python 檔案損毀或版本不符：{}；請重新安裝。",
                entry.path
            ));
        }
    }
    Ok(guards)
}

fn read_locked(
    root: &Path,
    relative: &str,
    max: usize,
    guards: &mut Vec<File>,
) -> AppResult<Vec<u8>> {
    let path = root.join(files::relative(relative)?);
    guards.extend(files::pin(path.parent().ok_or("缺少資料夾。")?)?);
    read_pinned(&path, max, guards)
}

/// 呼叫端已固定父目錄；檔案鎖在 Python 結束前持續保留。
fn read_pinned(path: &Path, max: usize, guards: &mut Vec<File>) -> AppResult<Vec<u8>> {
    let mut file = files::checked_file(path)?;
    files::reject_internal(&file)?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take((max + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > max {
        return Err("Python 資料超過讀取上限。".into());
    }
    guards.push(file);
    Ok(bytes)
}

/// 安裝／建置專用：只在已核對的 runtime 目錄附加 AppContainer 唯讀執行權。
/// 不修改專案、原始文件、使用者資料或 Windows 全域環境。
pub fn prepare_runtime() -> AppResult<()> {
    use windows_sys::Win32::{
        Foundation::*,
        Security::{Authorization::*, *},
    };
    let root = runtime_root()?;
    let _guards = verified_runtime(&root, &AtomicBool::new(false))?;
    unsafe {
        let mut descriptor = std::ptr::null_mut();
        let mut old_acl = std::ptr::null_mut();
        let name = crate::wide(&root.to_string_lossy());
        let status = GetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut old_acl,
            std::ptr::null_mut(),
            &mut descriptor,
        );
        if status != 0 {
            return Err(format!("無法讀取 Python 權限：{status}"));
        }
        let mut sid = std::ptr::null_mut();
        if ConvertStringSidToSidW(crate::wide("S-1-15-2-1").as_ptr(), &mut sid) == 0 {
            LocalFree(descriptor);
            return Err("無法建立 AppContainer 唯讀權限。".into());
        }
        let entry = EXPLICIT_ACCESS_W {
            grfAccessPermissions: 0x001200A9, // 檔案讀取、執行、列舉及同步；沒有寫入。
            grfAccessMode: GRANT_ACCESS,
            grfInheritance: SUB_CONTAINERS_AND_OBJECTS_INHERIT,
            Trustee: TRUSTEE_W {
                pMultipleTrustee: std::ptr::null_mut(),
                MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
                TrusteeForm: TRUSTEE_IS_SID,
                TrusteeType: TRUSTEE_IS_WELL_KNOWN_GROUP,
                ptstrName: sid.cast(),
            },
        };
        let mut new_acl = std::ptr::null_mut();
        let mut status = SetEntriesInAclW(1, &entry, old_acl, &mut new_acl);
        if status == 0 {
            status = SetNamedSecurityInfoW(
                name.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                new_acl,
                std::ptr::null_mut(),
            );
        }
        LocalFree(sid);
        LocalFree(descriptor);
        if !new_acl.is_null() {
            LocalFree(new_acl.cast());
        }
        if status != 0 {
            return Err(format!("無法設定 Python 唯讀權限：{status}"));
        }
    }
    Ok(())
}

/// 準備完整快照；不以頁面預覽或截斷文字冒充全部資料。
pub(super) fn snapshot(
    project: &Project,
    inputs: &[Input],
    cancel: &AtomicBool,
) -> AppResult<Value> {
    if inputs.len() > 8 {
        return Err("每次 Python 最多 8 個來源。".into());
    }
    let mut names = std::collections::BTreeSet::new();
    let mut output = Vec::new();
    let mut bytes_total = 0;
    for input in inputs {
        if input.kind != "text" && input.encoding.as_deref().is_some_and(|s| s != "auto") {
            return Err(
                "encoding 只適用 kind=text；CSV／資料集維持 UTF-8，XLSX 交給原格式讀取。".into(),
            );
        }
        if input.name.is_empty() || input.name.len() > 80 || !names.insert(&input.name) {
            return Err("Python 來源名稱需短且不重複。".into());
        }
        if cancel.load(Ordering::Relaxed) {
            return Err("Python 讀取已取消。".into());
        }
        let value = if let Some(range) = &input.log_range {
            if input.kind != "text" || !super::logs::supported(Path::new(&input.path)) {
                return Err("log_range 只適用 kind=text 的 LOG／OUT／ERR／JSONL。".into());
            }
            let mut value = super::logs::python_chunk(
                project,
                &input.path,
                input.revision.as_deref(),
                range.start_line,
                range.line_count,
                input.encoding.as_deref(),
                cancel,
            )?;
            value["name"] = json!(input.name);
            value
        } else if input.kind == "dataset" {
            let (table, revision) =
                datasets::inspect(project, &input.path, input.revision.as_deref(), cancel)?;
            let rows = table
                .rows
                .iter()
                .map(|row| {
                    row.values
                        .iter()
                        .zip(&row.kinds)
                        .map(|(value, kind)| match kind.as_str() {
                            "number" => serde_json::from_str::<Value>(value)
                                .unwrap_or_else(|_| json!(value)),
                            "boolean" => json!(value == "true"),
                            "blank" | "error" => Value::Null,
                            _ => json!(value),
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            json!({"name":input.name,"path":input.path,"kind":"dataset","revision":revision,
                "columns":table.columns,"rows":rows,"excel_schema":table.excel,
                "provenance":table.rows.iter().map(|r|json!({"path":r.path,"revision":r.revision,"sheet":r.sheet,"row":r.row,"kinds":r.kinds,"display":r.texts})).collect::<Vec<_>>()})
        } else {
            let rel = files::relative(&input.path)?;
            let ext = rel
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let generated_xlsx =
                input.kind == "xlsx" && ext == "xlsx" && rel.starts_with("_AI_Output");
            if !generated_xlsx
                && !(input.kind == "text"
                    && matches!(
                        ext.as_str(),
                        "log" | "txt" | "out" | "err" | "json" | "jsonl"
                    )
                    || input.kind == "csv" && ext == "csv")
            {
                return Err("Python 只直接讀取 CSV、文字／LOG，以及 _AI_Output 生成的 XLSX；公司 Excel 原檔請先由 COM 匯出資料集。".into());
            }
            let mut guards = Vec::new();
            let bytes = read_locked(&project.root, &input.path, MAX_INPUT, &mut guards)?;
            let revision = format!("sha256:{:x}", Sha256::digest(&bytes));
            if input
                .revision
                .as_ref()
                .is_some_and(|r| r != &revision && r != &revision.replace("sha256:", "log:"))
            {
                return Err("Python 來源版本已變更，請重新檢查。".into());
            }
            if generated_xlsx {
                json!({"name":input.name,"path":input.path,"kind":"xlsx","revision":revision,"hex":hex(&bytes)})
            } else {
                let decoded = encoding::decode(
                    &bytes,
                    input.kind == "text" && matches!(ext.as_str(), "log" | "out" | "err"),
                    input.encoding.as_deref(),
                )
                .map_err(|e| format!("Python 來源 {}：{e}", input.name))?;
                json!({"name":input.name,"path":input.path,"kind":input.kind,"revision":revision,
                    "text":decoded.text,"encoding":decoded.name,"encoding_ambiguous":decoded.ambiguous})
            }
        };
        bytes_total += serde_json::to_vec(&value).map_err(|e| e.to_string())?.len();
        if bytes_total > 60 * 1024 * 1024 {
            return Err("Python 來源快照合計超過 60 MiB。".into());
        }
        output.push(value);
    }
    Ok(json!(output))
}

pub fn execute(code: &str, inputs: Value, cancel: &AtomicBool) -> AppResult<Value> {
    if code.trim().is_empty() || code.len() > 32000 {
        return Err("Python 程式需為 1–32000 bytes。".into());
    }
    let root = runtime_root()?;
    let _runtime = verified_runtime(&root, cancel)?;
    let mut worker = Worker::start_python(&root.join("python.exe"), cancel)?;
    let response = worker.analyze(&json!({"code":code,"inputs":inputs}), cancel)?;
    if response["ok"] != true {
        return Err(format!(
            "Python 分析失敗：{}",
            response["error"].as_str().unwrap_or("未知錯誤")
        ));
    }
    Ok(response)
}

pub fn self_check() -> AppResult<()> {
    let result = execute("assert pd.__version__ == '2.2.3'\nassert np.__version__ == '2.2.6'\nassert openpyxl.__version__ == '3.1.5'\ndf = pd.DataFrame({'批號':['001','002'], '值':[2,4]})\nemit_excel('測試.xlsx', {'資料': df})\nresult={'sum':int(df['值'].sum())}", json!([]), &AtomicBool::new(false))?;
    if result["summary"]["sum"] != 6 || result["outputs"][0]["kind"] != "xlsx" {
        return Err("Python 分析／XLSX 自檢失敗。".into());
    }
    Ok(())
}

pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub(super) fn unhex(value: &str) -> AppResult<Vec<u8>> {
    if value.len() > MAX_OUTPUT * 2
        || !value.len().is_multiple_of(2)
        || !value.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("Python XLSX 輸出編碼或大小無效。".into());
    }
    value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| {
            u8::from_str_radix(std::str::from_utf8(b).map_err(|e| e.to_string())?, 16)
                .map_err(|e| e.to_string())
        })
        .collect()
}
