//! 僅載入隨 EXE 發行的工作方法，不執行專案資料夾內的腳本或設定。
use crate::AppResult;
use serde_json::{json, Value};
const SKILLS: &[(&str, &str, &str)] = &[
    (
        "paper-evidence",
        "論文閱讀與證據整理：讀取論文、比較證據、保留數字單位與來源區段。",
        include_str!("skills/paper-evidence.md"),
    ),
    (
        "weekly-update",
        "週報增量更新：延續既有週報，核對本週新增事項並保留其餘內容。",
        include_str!("skills/weekly-update.md"),
    ),
    (
        "multi-file-excel",
        "多文件抽取成 Excel：依共同欄位抽取多份資料並建立可追溯的比較表。",
        include_str!("skills/multi-file-excel.md"),
    ),
    (
        "charts",
        "資料圖表：使用折線、長條、散佈圖呈現可核對資料。",
        include_str!("skills/charts.md"),
    ),
];
pub fn catalog() -> Value {
    json!(SKILLS
        .iter()
        .map(|(id, description, _)| json!({"id":id,"description":description}))
        .collect::<Vec<_>>())
}
pub fn load(id: &str) -> AppResult<&'static str> {
    SKILLS
        .iter()
        .find(|s| s.0 == id)
        .map(|s| s.2)
        .ok_or("未知技能；請依技能目錄指定 id。".into())
}
pub fn context(ids: &[String]) -> AppResult<String> {
    ids.iter()
        .map(|id| load(id))
        .collect::<AppResult<Vec<_>>>()
        .map(|v| v.join("\n\n"))
}
