//! 編譯時能力是原生權限邊界；網頁狀態僅用來呈現，不能重新啟用模組。
use serde_json::{json, Value};

/// 回報本成品實際包含的選用模組；預設發行四項皆啟用。
pub fn state() -> Value {
    json!({"cad":cfg!(feature="cad"),"pet":cfg!(feature="pet"),
        "tarot":cfg!(feature="tarot"),"vnc":cfg!(feature="vnc")})
}

/// 停用CAD時仍可讀舊專案資料，但不能公布、執行或重播CAD工具成果。
pub(crate) fn tool_available(name: &str) -> bool {
    cfg!(feature = "cad") || !(name.starts_with("cad_") || name == "export_cad_comparison")
}
