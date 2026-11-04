//! 精簡建置的CAD拒絕入口。保留工具資料型別與舊專案相容性，不編入CAD工作程序。
use crate::AppResult;
use serde_json::Value;
use std::{sync::atomic::AtomicBool, time::Instant};

/// 精簡版不具備完整runtime核對能力，因此也不允許刪除既有CAD環境。
pub mod cleanup {
    /// 保留命令列回報型別；停用版不會產生成功的清理報告。
    #[derive(serde::Serialize)]
    pub struct Report {
        pub kept: Vec<String>,
    }

    /// 不載入或刪除任何CAD runtime。
    pub fn obsolete_runtimes() -> crate::AppResult<Report> {
        Err("此建置未包含CAD，未執行runtime清理，既有環境全部保留。".into())
    }
}

/// 與完整版共用的進度回呼型別；停用版不會呼叫它。
pub type Progress = Box<dyn FnMut(Value) + Send>;
/// 與完整版共用的連線等待回呼型別；停用版不會啟動連線。
pub type Waiter = Box<dyn FnMut(&Value, &AtomicBool, Instant) -> AppResult<String> + Send>;

const UNAVAILABLE: &str = "此建置未包含CAD功能；請使用包含cad feature的版本。尚未執行CAD操作。";

/// 保留呼叫介面並立即拒絕，不啟動CAD程序或等待互動。
pub fn execute_interactive(
    _request: Value,
    _cancel: &AtomicBool,
    _waiter: Option<&mut Waiter>,
    _progress: Option<&mut Progress>,
) -> AppResult<Value> {
    Err(UNAVAILABLE.into())
}
/// 拒絕任何CAD請求，不解析或執行外部內容。
pub fn execute(_request: Value, _cancel: &AtomicBool) -> AppResult<Value> {
    Err(UNAVAILABLE.into())
}
/// 停用版不掃描已安裝的CAD環境。
pub fn inventory(_cancel: &AtomicBool) -> AppResult<Value> {
    Err(UNAVAILABLE.into())
}
/// 未編入的CAD不能回報自檢成功。
pub fn self_check() -> AppResult<()> {
    Err(UNAVAILABLE.into())
}
pub mod offline {
    /// 未包含CAD的版本不能把未執行的圖面驗收回報為通過。
    pub fn run(_base: &std::path::Path, _version: &str) -> crate::AppResult<()> {
        Err(super::UNAVAILABLE.into())
    }
}
