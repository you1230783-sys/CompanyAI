//! UltraVNC 本機設定與啟動。機台檔沿用 Python 工具格式，密碼只在原生層讀取。
use crate::{storage, AppResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::{
    collections::{BTreeMap, VecDeque},
    fs,
    io::Read,
    os::windows::{fs::MetadataExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;

mod selection;
pub mod sync;
pub use selection::{MachineKey, Selection};

#[derive(Clone, Deserialize, Serialize)]
pub struct Machine {
    pub name: String,
    pub ip: String,
    #[serde(default)]
    pub password: String,
    // 保留舊工具或公司自行增加的欄位，編輯機台時不丟失額外設定。
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

pub type Machines = BTreeMap<String, Vec<Machine>>;

#[derive(Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Options {
    pub fullscreen: bool,
    pub viewonly: bool,
    pub autoscaling: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            fullscreen: false,
            viewonly: false,
            autoscaling: true,
            extra: Map::new(),
        }
    }
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct UserConfig {
    pub vnc_path: String,
    pub options: Options,
    /// 分類的手動順序獨立保存，machines.json 仍維持原本的分類物件格式。
    pub group_order: Vec<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

pub struct Manager {
    pub path: PathBuf,
    pub machines: Machines,
    pub config: UserConfig,
    original_machines: Option<Vec<u8>>,
    original_config: Option<Vec<u8>>,
}

/// 限制檔案大小且接受 UTF-8 BOM。錯誤不可包含原始 JSON，避免顯示密碼。
fn read_optional(path: &Path) -> AppResult<Option<Vec<u8>>> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("無法讀取 {}：{error}", path.display())),
    };
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("讀取設定失敗：{error}"))?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err("VNC 設定檔超過 4 MiB，請拆分檔案。".into());
    }
    Ok(Some(bytes))
}

fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> AppResult<T> {
    serde_json::from_slice(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes)).map_err(
        |error| {
            format!(
                "VNC JSON 格式不正確（第 {} 行、第 {} 欄），原檔未變更。",
                error.line(),
                error.column()
            )
        },
    )
}

/// 新設定用四格縮排與 LF，保持 Python 工具可直接讀取。
fn encode(value: &impl Serialize) -> AppResult<Vec<u8>> {
    let mut bytes = Vec::new();
    let format = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    value
        .serialize(&mut serde_json::Serializer::with_formatter(
            &mut bytes, format,
        ))
        .map_err(|_| "無法編碼 VNC 設定。".to_string())?;
    bytes.push(b'\n');
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err("VNC 設定檔超過 4 MiB。".into());
    }
    Ok(bytes)
}

impl Manager {
    pub fn default_path() -> AppResult<PathBuf> {
        let exe = std::env::current_exe().map_err(|_| "無法取得程式位置。")?;
        Ok(exe.with_file_name("machines.json"))
    }

    /// 完整讀取成功後才能替換目前管理器；損壞的檔案不會被預設值覆寫。
    pub fn load(path: PathBuf) -> AppResult<Self> {
        if !path.is_absolute()
            || path
                .extension()
                .is_none_or(|ext| !ext.eq_ignore_ascii_case("json"))
        {
            return Err("請選擇完整路徑的 JSON 機台設定檔。".into());
        }
        if path
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("user_config.json"))
        {
            return Err("請選擇 machines.json，不是 user_config.json。".into());
        }
        let original_machines = read_optional(&path)?;
        let machines = match &original_machines {
            Some(bytes) => decode(bytes)?,
            None => Machines::new(),
        };
        let original_config = read_optional(&path.with_file_name("user_config.json"))?;
        let config = match &original_config {
            Some(bytes) => decode(bytes)?,
            None => UserConfig::default(),
        };
        Ok(Self {
            path,
            machines,
            config,
            original_machines,
            original_config,
        })
    }

    /// WebView 僅取得名稱、位址及是否有密碼，既有密碼不回傳 JavaScript。
    pub fn public_groups(&self) -> Value {
        Value::Array(
            self.ordered_groups()
                .iter()
                .map(|group| {
                    let machines = &self.machines[group];
                    json!({
                        "name": group,
                        "machines": machines.iter().enumerate().map(|(index, machine)| json!({
                            "index": index, "name": machine.name, "ip": machine.ip,
                            "has_password": !machine.password.is_empty()
                        })).collect::<Vec<_>>()
                    })
                })
                .collect(),
        )
    }

    pub fn ordered_groups(&self) -> Vec<String> {
        let mut groups = Vec::new();
        for group in self.config.group_order.iter().chain(self.machines.keys()) {
            if self.machines.contains_key(group) && !groups.contains(group) {
                groups.push(group.clone());
            }
        }
        groups
    }

    pub fn machine(&self, group: &str, index: usize) -> AppResult<&Machine> {
        self.machines
            .get(group)
            .and_then(|items| items.get(index))
            .ok_or_else(|| "機台已變更或不存在，請重新選取。".into())
    }

    pub fn ensure_unchanged(&self) -> AppResult<()> {
        if read_optional(&self.path)? != self.original_machines {
            return Err("機台檔已由其他程式修改，請先按「重新讀取」再操作。".into());
        }
        Ok(())
    }

    /// 先寫入成功再更新記憶體，避免儲存失敗卻讓畫面誤認成功。
    pub fn save_machines(&mut self, machines: Machines) -> AppResult<()> {
        self.ensure_unchanged()?;
        let bytes = encode(&machines)?;
        storage::atomic_write(&self.path, &bytes)?;
        self.original_machines = Some(bytes);
        self.machines = machines;
        Ok(())
    }

    pub fn save_config(&mut self, config: UserConfig) -> AppResult<()> {
        let path = self.path.with_file_name("user_config.json");
        if read_optional(&path)? != self.original_config {
            return Err("VNC 使用者設定已由其他程式修改，請先重新讀取。".into());
        }
        let bytes = encode(&config)?;
        storage::atomic_write(&path, &bytes)?;
        self.original_config = Some(bytes);
        self.config = config;
        Ok(())
    }
}

/// server 只接受主機位址，支援 host:display、host::port 與括號 IPv6；不得夾帶選項。
pub fn validate_machine(group: &str, name: &str, server: &str, password: &str) -> AppResult<()> {
    if [group, name].iter().any(|text| {
        text.trim().is_empty() || text.len() > 256 || text.chars().any(char::is_control)
    }) {
        return Err("分類與機台名稱必須填寫，且不可含控制字元或超過 256 bytes。".into());
    }
    if server.len() > 512
        || server.starts_with('-')
        || !server
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b".-_:[]".contains(&byte))
    {
        return Err(
            "IP / Server 請填入主機名稱或位址，可加 :display 或 ::port，不可包含空白或命令參數。"
                .into(),
        );
    }
    if password.len() > 1024 || password.chars().any(char::is_control) {
        return Err("VNC 密碼不可含控制字元或超過 1024 bytes。".into());
    }
    Ok(())
}

pub fn validate_viewer(path: &Path) -> AppResult<()> {
    if !path.is_absolute()
        || !path.is_file()
        || !path
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("vncviewer.exe"))
    {
        return Err("請指定已安裝的 UltraVNC vncviewer.exe。".into());
    }
    Ok(())
}

/// 沿用原工具參數及順序；逐個傳遞參數，不組 shell 字串，也不輸出密碼。
pub fn connection_args(machine: &Machine, options: &Options) -> AppResult<Vec<String>> {
    let server = machine.ip.trim();
    if server.is_empty() {
        return Err("此機台尚未設定 IP，請先在機台管理補上位址。".into());
    }
    validate_machine("連線", &machine.name, server, &machine.password)?;
    let mut args = Vec::new();
    if !machine.password.is_empty() {
        args.extend(["/password".into(), machine.password.clone()]);
    }
    args.push("/shared".into());
    if options.fullscreen {
        args.push("/fullscreen".into());
    }
    if options.viewonly {
        args.push("/viewonly".into());
    }
    if options.autoscaling {
        args.push("/autoscaling".into());
    }
    args.extend(
        [
            "/autoreconnect",
            "5",
            "/reconnectcounter",
            "3",
            "/quickoption",
            "7",
        ]
        .map(String::from),
    );
    args.push(server.into());
    Ok(args)
}

/// 啟動 Viewer 並回傳子程序；一般介面讓它獨立運作，整合測試可精確清理自己的程序。
pub fn connect(manager: &Manager, group: &str, index: usize) -> AppResult<std::process::Child> {
    manager.ensure_unchanged()?;
    let path = Path::new(&manager.config.vnc_path);
    validate_viewer(path)?;
    let args = connection_args(manager.machine(group, index)?, &manager.config.options)?;
    Command::new(path)
        .args(args)
        .current_dir(path.parent().ok_or("Viewer 目錄無效。")?)
        .creation_flags(0x08000000) // CREATE_NO_WINDOW：隱藏主控台，不隱藏 Viewer 視窗。
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("無法啟動 UltraVNC Viewer：{error}"))
}

/// 背景搜尋標準安裝目錄；略過 reparse point，限制時間及目錄數，不呼叫 where/CMD。
pub fn find_viewer() -> AppResult<PathBuf> {
    let roots: Vec<PathBuf> = ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"]
        .iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .collect();
    let mut queue = VecDeque::new();
    for root in &roots {
        for relative in ["uvnc bvba/UltraVNC/vncviewer.exe", "UltraVNC/vncviewer.exe"] {
            let candidate = root.join(relative);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
        queue.push_back(root.clone());
    }
    let start = Instant::now();
    let mut count = 0;
    while let Some(directory) = queue.pop_front() {
        count += 1;
        if count > 20_000 || start.elapsed() > Duration::from_secs(30) {
            return Err("搜尋已達時間或目錄上限，請手動指定 vncviewer.exe。".into());
        }
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            if start.elapsed() > Duration::from_secs(30) {
                return Err("搜尋已達時間上限，請手動指定 vncviewer.exe。".into());
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            if metadata.file_attributes() & 0x400 != 0 {
                continue;
            }
            if metadata.is_dir() {
                queue.push_back(entry.path());
            } else if entry.file_name().eq_ignore_ascii_case("vncviewer.exe") {
                return Ok(entry.path());
            }
        }
    }
    Err("找不到 vncviewer.exe，請先安裝 UltraVNC Viewer，再按「指定 VNC」。".into())
}

#[cfg(test)]
mod tests;
