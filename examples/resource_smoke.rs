//! 原生資源驗收；只接受尚不存在的輸出目錄，不使用公司文件。
use company_ai::{
    projects::{office, python},
    AppResult,
};
fn run() -> AppResult<()> {
    let folder = std::path::PathBuf::from(std::env::args().nth(1).ok_or("需要新的驗收目錄。")?);
    std::fs::create_dir(&folder).map_err(|e| e.to_string())?;
    let report = office::verify_resource_concurrency(&folder)?;
    println!("PASS Word FIFO, independent Excel and external Word busy");
    let python = python::verify_parallel_workers()?;
    let report =
        serde_json::json!({"version":env!("CARGO_PKG_VERSION"),"office":report,"python":python});
    std::fs::write(
        folder.join("resource-verification.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    println!("PASS Office resource ownership");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
