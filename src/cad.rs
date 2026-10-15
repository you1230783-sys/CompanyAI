//! CAD 專用固定工作程序。與可執行模型程式的分析 Python 完全分開。
//! 固定腳本／runtime 逐檔核對，JSON 管線不提供任意 COM 名稱或 shell 指令。
use crate::AppResult;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    os::windows::{fs::MetadataExt, process::CommandExt},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    thread,
    time::{Duration, Instant},
};

// CAD 的目前文件與 COM 伺服器是共享狀態，同一應用程式只允許一個 CAD 工作。
static ACTIVE: Mutex<()> = Mutex::new(());
const MANIFEST: &str = include_str!("../assets/cad-runtime-manifest.json");
const FINGERPRINT: &str = include_str!("../assets/cad-runtime-sha256.txt");

/// 桌面對話框回覆由原生控制器核對任務／請求 ID；不把開啟程式決策交給模型。
pub type Waiter = Box<dyn FnMut(&Value, &AtomicBool, Instant) -> AppResult<String> + Send>;

pub fn execute_interactive(
    mut request: Value,
    cancel: &AtomicBool,
    mut waiter: Option<&mut Waiter>,
) -> AppResult<Value> {
    let deadline = Instant::now() + Duration::from_secs(600);
    let mut launch_if_missing = false;
    let mut wait_for_ready = false;
    loop {
        if cancel.load(Ordering::Relaxed) || Instant::now() >= deadline {
            return Ok(json!({"status":"CANCELLED", "drawing_started":false,
                "recovery":"correct_and_retry", "error":"CAD 連線等待已取消或逾時", "advice":"尚未開始繪圖。"}));
        }
        // 不先用另一個短命程序探測：啟動取得的 COM 物件必須保留至真正操作完成。
        // 只在上一輪明確尚未開始操作時重送既定計畫，不重播部分修改。
        let mut attempt = request.clone();
        attempt["launch_if_missing"] = json!(launch_if_missing);
        attempt["wait_for_ready"] = json!(wait_for_ready);
        let mut connected = execute(attempt, cancel)?;
        if let Some(progid) = connected["selected"]["progid"].as_str() {
            request["version"] = json!(progid);
        }
        if connected["step"] != "connect" || connected["drawing_started"] != false {
            if connected["connection_lost"] == true
                && connected["drawing_started"] == true
                && request["action"] != "active_edit"
            {
                if let Some(ask) = waiter.as_mut() {
                    let decision = ask(
                        &json!({"partial":true,"selected":connected["selected"],
                        "message":"CAD 連線中斷，部分圖面狀態未確認。重新開啟 CAD 後，可選擇另建新副本重做；不會接續或覆寫原圖面。"}),
                        cancel,
                        deadline,
                    )?;
                    if decision == "rebuild" {
                        connected["recovery"] = json!("user_requested_new_document");
                        connected["advice"] = json!("使用者已選擇另建新圖面重做；保留本次部分圖面，下一次呼叫建立全新副本，不能宣稱已完成。");
                    }
                }
            }
            return Ok(connected);
        }
        let Some(ask) = waiter.as_mut() else {
            return Ok(connected);
        };
        if connected["selected"].is_null() {
            return Ok(connected);
        }
        let decision = ask(
            &json!({"partial":false,"selected":connected["selected"],
            "message":"原計畫已保留，尚未開始繪圖。請開啟指定 CAD，完成啟動畫面／授權提示後按重新連線繼續；也可選擇開啟 CAD。程式會等待最多15秒取得連線。請確認兩個程式使用相同使用者與權限。", "error":connected["error"]}),
            cancel,
            deadline,
        )?;
        wait_for_ready = true;
        launch_if_missing = match decision.as_str() {
            "launch" => true,
            "retry" => false,
            _ => {
                return Ok(json!({"status":"CANCELLED","drawing_started":false,
                "recovery":"correct_and_retry","error":"使用者取消 CAD 連線","advice":"未開始繪圖，不要自動重送。"}))
            }
        };
    }
}

/// 保留檔案唯讀 handle 至子程序結束，避免核對後被替換。
fn runtime() -> AppResult<(PathBuf, Vec<fs::File>)> {
    let base = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("找不到應用程式目錄。")?
        .join(format!("cad-{}", &FINGERPRINT.trim()[..16]));
    let files: BTreeMap<String, String> =
        serde_json::from_str(MANIFEST).map_err(|e| e.to_string())?;
    let mut guards = Vec::new();
    for (name, expected) in files {
        let target = base.join(name);
        // 清單由建置時固定；仍拒絕 runtime 及子目錄重解析點。
        let mut parent = target.parent();
        while let Some(path) = parent {
            let metadata = fs::symlink_metadata(path)
                .map_err(|e| format!("CAD runtime 缺少或無法讀取：{e}"))?;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err("CAD runtime 不能使用連結目錄。".into());
            }
            if path == base {
                break;
            }
            parent = path.parent();
        }
        let mut file = crate::projects::files::checked_file(&target)?;
        let mut hash = Sha256::new();
        std::io::copy(&mut file, &mut hash).map_err(|e| e.to_string())?;
        if format!("{:x}", hash.finalize()) != expected {
            return Err("CAD runtime 完整性檢查失敗，請重新安裝本版。".into());
        }
        guards.push(file);
    }
    Ok((base, guards))
}

/// 安裝／基礎驗證核對 runtime、載入模組並驗證唯讀登錄查詢管線，不啟動 CAD。
pub fn self_check() -> AppResult<()> {
    let (base, _guards) = runtime()?;
    let status = Command::new(base.join("runtime/python.exe"))
        .args([
            "-I",
            "-B",
            "-c",
            "import worker; assert worker.Formula('sin(t)').at(0) == 0",
        ])
        .creation_flags(0x08000000)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("CAD runtime 載入失敗。".into());
    }
    let result = inventory(&AtomicBool::new(false))?;
    if result["status"] != "INVENTORY" || !result["versions"].is_array() {
        return Err(format!("CAD 登錄查詢管線失敗：{result}"));
    }
    Ok(())
}

/// 取消／逾時只終止自己的 Python；CAD 可能仍有部分圖面，因此不自動重試。
pub fn execute(request: Value, cancel: &AtomicBool) -> AppResult<Value> {
    let _active = ACTIVE
        .try_lock()
        .map_err(|_| "另一個 CAD 工作仍在執行，請稍後手動重試。")?;
    let input = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
    if input.len() > 900_000 {
        return Err("CAD 計畫超過大小限制。".into());
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("CAD 工作已取消。".into());
    }
    let (base, _guards) = runtime()?;
    let mut child = Command::new(base.join("runtime/python.exe"))
        .args(["-I", "-B"])
        .arg(base.join("worker.py"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|e| format!("無法啟動 CAD 工作程序：{e}"))?;
    let mut stdin = child.stdin.take().ok_or("CAD 輸入管線遺失。")?;
    let stdout = child.stdout.take().ok_or("CAD 輸出管線遺失。")?;
    // 同時讀寫管線，避免大量幾何資料塞滿管線而阻塞主程序的取消監控。
    let writer = thread::spawn(move || stdin.write_all(&input));
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .take(2_000_001)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let deadline = Instant::now() + Duration::from_secs(240);
    loop {
        if cancel.load(Ordering::Relaxed) || Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(
                "CAD 工作取消或逾時，結果未知；請檢查 CAD 與成果資料夾，勿自動重送。".into(),
            );
        }
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => thread::sleep(Duration::from_millis(100)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.to_string());
            }
        }
    }
    writer
        .join()
        .map_err(|_| "CAD 輸入執行緒失敗。")?
        .map_err(|e| e.to_string())?;
    let bytes = reader
        .join()
        .map_err(|_| "CAD 輸出執行緒失敗。")?
        .map_err(|e| e.to_string())?;
    if bytes.len() > 2_000_000 {
        return Err("CAD 結果過大。".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| format!("CAD 回應無法解析：{e}"))
}

pub fn inventory(cancel: &AtomicBool) -> AppResult<Value> {
    execute(json!({"action":"inventory"}), cancel)
}
