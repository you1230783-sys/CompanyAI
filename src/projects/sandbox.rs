//! 無網路能力的 AppContainer 文字執行器。只繼承 IPC 管線，不繼承文件或登入控制代碼。
//! 檔案 broker 留在主程序；worker 只接受固定文字修改，沒有 Shell 或任意程式入口。
use crate::{wide, AppResult};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{Read, Write},
    os::windows::io::{AsRawHandle, FromRawHandle},
    ptr,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    Security::{Isolation::*, *},
    System::{JobObjects::*, Pipes::*, Threading::*},
};

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}
struct Profile {
    name: Vec<u16>,
    sid: PSID,
}
impl Drop for Profile {
    fn drop(&mut self) {
        unsafe {
            FreeSid(self.sid);
            DeleteAppContainerProfile(self.name.as_ptr());
        }
    }
}
struct Attributes {
    data: Vec<usize>,
    initialized: bool,
}
impl Attributes {
    fn pointer(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.data.as_mut_ptr().cast()
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        if self.initialized {
            unsafe {
                DeleteProcThreadAttributeList(self.pointer());
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Edit {
    pub content: String,
    pub revision: String,
    pub start: usize,
    pub expected: String,
    pub replacement: String,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Edit {
        edit: Edit,
    },
    /// 只供本機驗收；模型工具表沒有此入口。回傳是否拒絕，不回傳檔案內容。
    InspectIsolation {
        path: String,
        port: u16,
    },
}

pub struct Worker {
    // 順序很重要：先終止程序，關閉 IPC，再刪除 AppContainer 設定。
    process: Handle,
    job: Handle,
    input: File,
    output: File,
    _profile: Profile,
}
impl Drop for Worker {
    fn drop(&mut self) {
        unsafe {
            TerminateJobObject(self.job.0, 0);
            WaitForSingleObject(self.process.0, 3000);
        }
    }
}

fn error(label: &str) -> String {
    format!(
        "{label}（Windows {}）；已停止，未退回一般權限。",
        unsafe { GetLastError() }
    )
}
fn pipe() -> AppResult<(File, File)> {
    let mut read = ptr::null_mut();
    let mut write = ptr::null_mut();
    let sa = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        bInheritHandle: 1,
        lpSecurityDescriptor: ptr::null_mut(),
    };
    // 容量可容納一般文字訊息；大型 PDF 由已就緒的子程序持續讀取輸入後才解析。
    if unsafe { CreatePipe(&mut read, &mut write, &sa, 1_048_576) } == 0 {
        return Err(error("無法建立隔離管線"));
    }
    Ok(unsafe { (File::from_raw_handle(read), File::from_raw_handle(write)) })
}

impl Worker {
    pub fn start(exe: &std::path::Path, cancel: &AtomicBool) -> AppResult<Self> {
        let name = wide(&format!("CompanyAI.Text.{}", crate::jobs::new_id()?));
        let mut sid = ptr::null_mut();
        let hr = unsafe {
            CreateAppContainerProfile(
                name.as_ptr(),
                name.as_ptr(),
                name.as_ptr(),
                ptr::null(),
                0,
                &mut sid,
            )
        };
        if hr < 0 {
            return Err(format!("無法建立 AppContainer（{hr:#x}），本次任務停止。"));
        }
        let profile = Profile { name, sid };
        let (child_input, input) = pipe()?;
        let (output, child_output) = pipe()?;
        unsafe {
            SetHandleInformation(input.as_raw_handle(), HANDLE_FLAG_INHERIT, 0);
            SetHandleInformation(output.as_raw_handle(), HANDLE_FLAG_INHERIT, 0);
        }
        let mut size = 0;
        unsafe {
            InitializeProcThreadAttributeList(ptr::null_mut(), 2, 0, &mut size);
        }
        let mut attributes = Attributes {
            data: vec![0; size.div_ceil(size_of::<usize>())],
            initialized: false,
        };
        if unsafe { InitializeProcThreadAttributeList(attributes.pointer(), 2, 0, &mut size) } == 0
        {
            return Err(error("無法設定程序屬性"));
        }
        attributes.initialized = true;
        let capabilities = SECURITY_CAPABILITIES {
            AppContainerSid: profile.sid,
            Capabilities: ptr::null_mut(),
            CapabilityCount: 0,
            Reserved: 0,
        };
        let handles = [child_input.as_raw_handle(), child_output.as_raw_handle()];
        for (attribute, value, bytes) in [
            (
                PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
                (&capabilities as *const SECURITY_CAPABILITIES).cast(),
                size_of::<SECURITY_CAPABILITIES>(),
            ),
            (
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                handles.as_ptr().cast(),
                size_of_val(&handles),
            ),
        ] {
            if unsafe {
                UpdateProcThreadAttribute(
                    attributes.pointer(),
                    0,
                    attribute,
                    value,
                    bytes,
                    ptr::null_mut(),
                    ptr::null(),
                )
            } == 0
            {
                return Err(error("無法設定隔離權限"));
            }
        }
        let startup = STARTUPINFOEXW {
            StartupInfo: STARTUPINFOW {
                cb: size_of::<STARTUPINFOEXW>() as u32,
                dwFlags: STARTF_USESTDHANDLES | STARTF_USESHOWWINDOW,
                wShowWindow: 0,
                hStdInput: child_input.as_raw_handle(),
                hStdOutput: child_output.as_raw_handle(),
                hStdError: child_output.as_raw_handle(),
                ..Default::default()
            },
            lpAttributeList: attributes.pointer(),
        };
        let system = std::env::var("SystemRoot").map_err(|_| "找不到 Windows 系統目錄。")?;
        // Windows 建立 AppContainer 時仍需要標準目錄變數以建立隔離後的環境。
        // 精確白名單不傳 Token、代理伺服器、PATH 或其他使用者自訂機密。
        let mut entries = Vec::new();
        for key in [
            "SystemRoot",
            "SystemDrive",
            "WINDIR",
            "USERPROFILE",
            "LOCALAPPDATA",
            "APPDATA",
            "TEMP",
            "TMP",
            "ProgramData",
            "ALLUSERSPROFILE",
            "ProgramFiles",
            "ProgramFiles(x86)",
            "CommonProgramFiles",
            "CommonProgramFiles(x86)",
        ] {
            if let Some(value) = std::env::var_os(key) {
                entries.push(format!("{key}={}", value.to_string_lossy()));
            }
        }
        entries.sort_by_key(|s| s.to_ascii_uppercase());
        let mut env: Vec<u16> = format!("{}\0\0", entries.join("\0"))
            .encode_utf16()
            .collect();
        let executable = wide(&exe.to_string_lossy());
        let mut command = wide(&format!("\"{}\" --project-worker", exe.display()));
        let mut process = PROCESS_INFORMATION::default();
        let job = Handle(unsafe { CreateJobObjectW(ptr::null(), ptr::null()) });
        if job.0.is_null() {
            return Err(error("無法建立程序群組"));
        }
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
            | JOB_OBJECT_LIMIT_PROCESS_MEMORY;
        limits.BasicLimitInformation.ActiveProcessLimit = 1;
        limits.ProcessMemoryLimit = 128 * 1024 * 1024;
        if unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of_val(&limits) as u32,
            )
        } == 0
        {
            return Err(error("無法設定程序資源限制"));
        }
        // 暫停建立，先納入 Job 再執行，避免主程序退出時遺留不受管理的 worker。
        if unsafe {
            CreateProcessW(
                executable.as_ptr(),
                command.as_mut_ptr(),
                ptr::null(),
                ptr::null(),
                1,
                EXTENDED_STARTUPINFO_PRESENT
                    | CREATE_SUSPENDED
                    | CREATE_NO_WINDOW
                    | CREATE_UNICODE_ENVIRONMENT,
                env.as_mut_ptr().cast(),
                wide(&system).as_ptr(),
                &startup.StartupInfo,
                &mut process,
            )
        } == 0
        {
            return Err(error("無法啟動受限制子程序"));
        }
        let thread = Handle(process.hThread);
        let process = Handle(process.hProcess);
        if unsafe { AssignProcessToJobObject(job.0, process.0) } == 0 {
            unsafe {
                TerminateProcess(process.0, 1);
            }
            return Err(error("無法管理子程序"));
        }
        let mut worker = Self {
            process,
            job,
            input,
            output,
            _profile: profile,
        };
        if unsafe { ResumeThread(thread.0) } == u32::MAX {
            return Err(error("無法啟動文字執行器"));
        }
        drop(child_input);
        drop(child_output);
        let handshake = worker.receive(cancel)?;
        if handshake != b"appcontainer-ready" {
            return Err("子程序隔離驗證失敗。".into());
        }
        Ok(worker)
    }

    pub fn edit(&mut self, edit: &Edit, cancel: &AtomicBool) -> AppResult<String> {
        let data = serde_json::to_vec(&Request::Edit {
            edit: Edit {
                content: edit.content.clone(),
                revision: edit.revision.clone(),
                start: edit.start,
                expected: edit.expected.clone(),
                replacement: edit.replacement.clone(),
            },
        })
        .map_err(|e| e.to_string())?;
        if data.len() > 800_000 {
            return Err("本次文字修改過大，請分段操作。".into());
        }
        if cancel.load(Ordering::Relaxed) {
            return Err("任務已取消。".into());
        }
        write_frame(&mut self.input, &data)?;
        serde_json::from_slice::<AppResult<String>>(&self.receive(cancel)?)
            .map_err(|_| "子程序回覆格式不正確。".to_string())?
    }

    /// PDF 使用固定二進位訊息，避免 JSON 數字陣列膨脹；解析仍在無檔案權限的子程序。
    pub fn extract_pdf(&mut self, bytes: &[u8], cancel: &AtomicBool) -> AppResult<String> {
        if bytes.len() > super::pdf::MAX_PDF || cancel.load(Ordering::Relaxed) {
            return Err("PDF 過大或任務已取消。".into());
        }
        let mut data = Vec::with_capacity(bytes.len() + 4);
        data.extend_from_slice(b"PDF\0");
        data.extend_from_slice(bytes);
        write_frame(&mut self.input, &data)?;
        serde_json::from_slice::<AppResult<String>>(&self.receive(cancel)?)
            .map_err(|_| "PDF 子程序回覆格式不正確。".to_string())?
    }

    /// 以真實外部檔案與本機監聽埠驗證 OS 邊界，供建置驗收程式使用。
    pub fn inspect_isolation(
        &mut self,
        path: &std::path::Path,
        port: u16,
        cancel: &AtomicBool,
    ) -> AppResult<serde_json::Value> {
        let data = serde_json::to_vec(&Request::InspectIsolation {
            path: path.to_string_lossy().into_owned(),
            port,
        })
        .map_err(|e| e.to_string())?;
        write_frame(&mut self.input, &data)?;
        let response = serde_json::from_slice::<AppResult<String>>(&self.receive(cancel)?)
            .map_err(|e| e.to_string())??;
        serde_json::from_str(&response).map_err(|e| e.to_string())
    }

    fn receive(&mut self, cancel: &AtomicBool) -> AppResult<Vec<u8>> {
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut received = Vec::new();
        loop {
            if cancel.load(Ordering::Relaxed) || Instant::now() >= deadline {
                // 終止後不再接受遲到回覆，避免將前一操作的結果誤配給下一操作。
                unsafe {
                    TerminateJobObject(self.job.0, 1);
                }
                return Err("子程序已取消或回應逾時。".into());
            }
            let mut available = 0;
            if unsafe {
                PeekNamedPipe(
                    self.output.as_raw_handle(),
                    ptr::null_mut(),
                    0,
                    ptr::null_mut(),
                    &mut available,
                    ptr::null_mut(),
                )
            } == 0
            {
                return Err(error("子程序已中斷"));
            }
            if available > 0 {
                let mut buffer = vec![0; (available as usize).min(16_384)];
                let count = self.output.read(&mut buffer).map_err(|e| e.to_string())?;
                if count == 0 {
                    return Err("子程序已結束。".into());
                }
                received.extend_from_slice(&buffer[..count]);
                if received.len() >= 4 {
                    let length = u32::from_le_bytes(
                        received[..4].try_into().map_err(|_| "管線資料不完整。")?,
                    ) as usize;
                    if length > 800_000 || received.len() > length + 4 {
                        return Err("子程序回覆超出限制。".into());
                    }
                    if received.len() == length + 4 {
                        return Ok(received.split_off(4));
                    }
                }
            } else {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

fn write_frame(output: &mut impl Write, data: &[u8]) -> AppResult<()> {
    output
        .write_all(&(data.len() as u32).to_le_bytes())
        .and_then(|_| output.write_all(data))
        .and_then(|_| output.flush())
        .map_err(|e| e.to_string())
}

/// 隱藏執行入口：實際 TokenIsAppContainer 不成立即拒絕，不可手動以一般權限執行。
pub fn run_worker() -> AppResult<()> {
    let mut token = ptr::null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(error("無法驗證隔離身分"));
    }
    let token = Handle(token);
    let mut is_container = 0u32;
    let mut size = 0;
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenIsAppContainer,
            (&mut is_container as *mut u32).cast(),
            4,
            &mut size,
        )
    } == 0
        || is_container != 1
    {
        return Err("文字執行器必須在 AppContainer 內啟動。".into());
    }
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    write_frame(&mut output, b"appcontainer-ready")?;
    loop {
        let mut header = [0; 4];
        if input.read_exact(&mut header).is_err() {
            return Ok(());
        }
        let length = u32::from_le_bytes(header) as usize;
        if length > super::pdf::MAX_PDF + 4 {
            return Err("操作資料過大。".into());
        }
        let mut bytes = vec![0; length];
        input.read_exact(&mut bytes).map_err(|e| e.to_string())?;
        let result: AppResult<String> = if let Some(pdf) = bytes.strip_prefix(b"PDF\0") {
            super::pdf::extract(pdf)
        } else if bytes.len() > 800_000 {
            Err("文字操作資料過大。".into())
        } else {
            serde_json::from_slice::<Request>(&bytes)
            .map_err(|_| "未知文字操作。".into())
            .and_then(|request| match request {
                Request::Edit { edit: e } => super::text::edit(&e.content, &e.revision, e.start, &e.expected, &e.replacement),
                Request::InspectIsolation { path, port } => {
                    let denied_read = std::fs::File::open(&path).is_err();
                    let denied_write = std::fs::OpenOptions::new().write(true).open(&path).is_err();
                    let denied_network = std::net::TcpStream::connect_timeout(&std::net::SocketAddr::from(([127,0,0,1], port)), Duration::from_secs(2)).is_err();
                    Ok(serde_json::json!({"read_denied":denied_read,"write_denied":denied_write,"network_denied":denied_network}).to_string())
                }
            })
        };
        write_frame(
            &mut output,
            &serde_json::to_vec(&result).map_err(|e| e.to_string())?,
        )?;
    }
}
