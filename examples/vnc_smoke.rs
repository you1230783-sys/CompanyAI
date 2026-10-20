//! 僅供開發機手動執行：真實 UltraVNC Viewer 連到本機合成的 RFB 畫面。
//! 使用正式 connect() 與 Python 相容設定檔；不讀取使用者機台，也不連公司網路。
//! cargo run --frozen --example vnc_smoke -- [--viewonly] [--fullscreen] [--no-autoscaling]
use company_ai::{jobs, vnc, AppResult};
use serde_json::json;
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    process::Child,
    ptr, thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::Security::Cryptography::*;

// 這是固定的虛構測試密碼，與任何使用者或機台憑證無關。
const PASSWORD: &str = "TestVnc8";
const CHALLENGE: [u8; 16] = *b"LM_AI_LOCAL_TEST";

struct Viewer(Child);
impl Drop for Viewer {
    fn drop(&mut self) {
        // 僅處理本測試啟動的 PID，不依名稱終止其他 Viewer。
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Windows CNG 計算 RFB 傳統 VNC challenge-response，不引入額外離線依賴。
fn expected_response() -> AppResult<[u8; 16]> {
    let mut algorithm = ptr::null_mut();
    let mut key = ptr::null_mut();
    let secret: Vec<u8> = PASSWORD.bytes().map(u8::reverse_bits).collect();
    let mode = company_ai::wide("ChainingModeECB");
    let mut output = [0u8; 16];
    let mut written = 0;
    // SAFETY: 所有緩衝區在同步 CNG 呼叫期間有效，handle 在任何結果下都會釋放。
    unsafe {
        if BCryptOpenAlgorithmProvider(&mut algorithm, BCRYPT_DES_ALGORITHM, ptr::null(), 0) < 0 {
            return Err("無法開啟測試用 DES provider。".into());
        }
        let result = (|| {
            if BCryptSetProperty(
                algorithm,
                BCRYPT_CHAINING_MODE,
                mode.as_ptr().cast(),
                (mode.len() * 2) as u32,
                0,
            ) < 0
                || BCryptGenerateSymmetricKey(
                    algorithm,
                    &mut key,
                    ptr::null_mut(),
                    0,
                    secret.as_ptr(),
                    secret.len() as u32,
                    0,
                ) < 0
            {
                return Err("無法建立測試用 DES key。".into());
            }
            if BCryptEncrypt(
                key,
                CHALLENGE.as_ptr(),
                16,
                ptr::null(),
                ptr::null_mut(),
                0,
                output.as_mut_ptr(),
                16,
                &mut written,
                0,
            ) < 0
                || written != 16
            {
                return Err("無法產生測試 challenge-response。".into());
            }
            Ok(output)
        })();
        if !key.is_null() {
            BCryptDestroyKey(key);
        }
        BCryptCloseAlgorithmProvider(algorithm, 0);
        result
    }
}

fn read<const N: usize>(stream: &mut TcpStream) -> AppResult<[u8; N]> {
    let mut bytes = [0; N];
    stream
        .read_exact(&mut bytes)
        .map_err(|error| format!("RFB 讀取失敗：{error}"))?;
    Ok(bytes)
}
fn write(stream: &mut TcpStream, bytes: &[u8]) -> AppResult<()> {
    stream
        .write_all(bytes)
        .map_err(|error| format!("RFB 寫入失敗：{error}"))
}

fn session(stream: &mut TcpStream) -> AppResult<()> {
    // Windows accept 可能沿用 listener 的 nonblocking 狀態，握手改用有逾時的同步讀取。
    stream.set_nonblocking(false).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
    write(stream, b"RFB 003.008\n")?;
    let version = read::<12>(stream)?;
    if version != *b"RFB 003.008\n" {
        return Err("Viewer 沒有選擇測試用 RFB 3.8。".into());
    }
    write(stream, &[1, 2])?; // 唯一允許的安全類型：VNC 密碼驗證。
    if read::<1>(stream)? != [2] {
        return Err("Viewer 未選擇 VNC 密碼驗證。".into());
    }
    write(stream, &CHALLENGE)?;
    if read::<16>(stream)? != expected_response()? {
        return Err("Viewer 傳入的密碼驗證結果不符。".into());
    }
    write(stream, &[0, 0, 0, 0])?;
    if read::<1>(stream)? != [1] {
        return Err("Viewer 未套用 /shared。".into());
    }
    // 480 × 240、32-bit little-endian true color；畫面全部由本測試合成。
    let format = [32, 24, 0, 1, 0, 255, 0, 255, 0, 255, 16, 8, 0, 0, 0, 0];
    write(stream, &480u16.to_be_bytes())?;
    write(stream, &240u16.to_be_bytes())?;
    write(stream, &format)?;
    let title = b"LM_AI - LOCAL TEST - password verified";
    write(stream, &(title.len() as u32).to_be_bytes())?;
    write(stream, title)?;
    loop {
        match read::<1>(stream)?[0] {
            0 => {
                let bytes = read::<19>(stream)?;
                if bytes[3..] != format {
                    return Err("Viewer 選擇了測試未支援的像素格式。".into());
                }
            }
            2 => {
                let header = read::<3>(stream)?;
                let length = u16::from_be_bytes([header[1], header[2]]) as usize;
                if length > 256 {
                    return Err("Viewer encoding 清單過長。".into());
                }
                let mut encodings = vec![0; length * 4];
                stream
                    .read_exact(&mut encodings)
                    .map_err(|e| e.to_string())?;
            }
            3 => {
                read::<9>(stream)?;
                // 一個 raw rectangle，僅顯示三塊合成色塊，完全不擷取桌面。
                write(stream, &[0, 0, 0, 1, 0, 0, 0, 0])?;
                write(stream, &480u16.to_be_bytes())?;
                write(stream, &240u16.to_be_bytes())?;
                write(stream, &[0, 0, 0, 0])?;
                let pixels: Vec<u8> = (0..240)
                    .flat_map(|_| {
                        (0..480).flat_map(|x| {
                            if x < 160 {
                                [105, 121, 82, 0]
                            } else if x < 320 {
                                [150, 180, 140, 0]
                            } else {
                                [220, 235, 225, 0]
                            }
                        })
                    })
                    .collect();
                write(stream, &pixels)?;
                println!("PASS: real Viewer connected to loopback, authenticated the configured password, enabled shared mode, and requested a framebuffer.");
                break;
            }
            other => return Err(format!("初始化收到未支援的 RFB message {other}。")),
        }
    }
    // 留下短暫觀察時間供 Computer Use 擷取真實 Viewer 畫面；最長 60 秒。
    let hold = std::env::var("LM_VNC_SMOKE_HOLD_SECONDS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(10)
        .min(60);
    thread::sleep(Duration::from_secs(hold));
    Ok(())
}

fn main() -> AppResult<()> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.iter().any(|argument| {
        !matches!(
            argument.as_str(),
            "--viewonly" | "--fullscreen" | "--no-autoscaling"
        )
    }) {
        return Err("只接受 --viewonly、--fullscreen、--no-autoscaling。".into());
    }
    let viewer = vnc::find_viewer()?;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".build/vnc-smoke")
        .join(jobs::new_id()?);
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let machines = root.join("machines.json");
    fs::write(&machines, json!({"local-test":[{"name":"Local synthetic desktop","ip":format!("127.0.0.1::{port}"),"password":PASSWORD}]}).to_string()).map_err(|e| e.to_string())?;
    fs::write(
        root.join("user_config.json"),
        json!({"vnc_path":viewer,"options":{
            "fullscreen":arguments.iter().any(|a| a == "--fullscreen"),
            "viewonly":arguments.iter().any(|a| a == "--viewonly"),
            "autoscaling":!arguments.iter().any(|a| a == "--no-autoscaling")
        }})
        .to_string(),
    )
    .map_err(|e| e.to_string())?;
    let manager = vnc::Manager::load(machines)?;
    let child = Viewer(vnc::connect(&manager, "local-test", 0)?);
    println!(
        "Viewer PID={} path={} local-port={port}",
        child.0.id(),
        viewer.display()
    );
    let start = Instant::now();
    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                session(&mut stream)?;
                break;
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    && start.elapsed() < Duration::from_secs(25) =>
            {
                thread::sleep(Duration::from_millis(50))
            }
            Err(error) => return Err(format!("Viewer 未連線到本機測試端：{error}")),
        }
    }
    let report = json!({"result":"PASS","app_version":env!("CARGO_PKG_VERSION"),"viewer":viewer,"options":manager.config.options,
        "checks":["native connect() launch","loopback RFB 3.8","VNC password challenge-response","shared flag","framebuffer request"],
        "company_machine_tested":false});
    fs::write(
        root.join("verification.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    // 建置腳本可指定報告位置；只複製不含測試密碼與本機 port 的驗證結果。
    if let Some(destination) = std::env::var_os("LM_VNC_SMOKE_REPORT") {
        fs::copy(root.join("verification.json"), destination).map_err(|e| e.to_string())?;
    }
    println!("REPORT={}", root.join("verification.json").display());
    Ok(())
}
