//! 僅載入隨 EXE 發行的工作方法，不執行專案資料夾內的腳本或設定。
use crate::AppResult;
use serde_json::{json, Value};
const SKILLS: &[(&str, &str, &str)] = &[
    (
        "outlook-research",
        "同意後挑選郵件、去重及整理週報",
        include_str!("skills/outlook-research.md"),
    ),
    (
        "log-analysis",
        "大型 LOG 分頁、時間及多關鍵字搜尋",
        include_str!("skills/log-analysis.md"),
    ),
    (
        "text-edit",
        "搜尋、建立與修訂 TXT／MD 副本",
        include_str!("skills/text-edit.md"),
    ),
    (
        "office-edit",
        "建立、編輯與排版 Office，插入 PNG",
        include_str!("skills/office-edit.md"),
    ),
    (
        "excel-read",
        "Excel 表頭檢視、選欄與分批讀取",
        include_str!("skills/excel-read.md"),
    ),
    (
        "research",
        "跨文件搜尋、分段閱讀與快速摘要",
        include_str!("skills/research.md"),
    ),
    (
        "notes",
        "保存及修訂文件摘要、任務筆記",
        include_str!("skills/notes.md"),
    ),
    (
        "charts",
        "Excel 趨勢圖、異常值處理與 PNG",
        include_str!("skills/charts.md"),
    ),
    (
        "paper-evidence",
        "論文閱讀、數據核對與證據整理",
        include_str!("skills/paper-evidence.md"),
    ),
    (
        "weekly-update",
        "保留舊週報結構，更新本週內容",
        include_str!("skills/weekly-update.md"),
    ),
    (
        "multi-file-excel",
        "抽取多文件欄位，建立 Excel 比較表",
        include_str!("skills/multi-file-excel.md"),
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

/// 高階工作技能載入所需基本組；同一份說明在 system 只出現一次。
pub fn activate(ids: &mut Vec<String>, id: &str) -> AppResult<()> {
    load(id)?;
    let dependencies: &[&str] = match id {
        "paper-evidence" => &["research", "notes"],
        "weekly-update" => &["research", "notes", "office-edit"],
        "multi-file-excel" => &["research", "office-edit", "excel-read"],
        "charts" => &["excel-read"],
        _ => &[],
    };
    for next in dependencies.iter().copied().chain(std::iter::once(id)) {
        if !ids.iter().any(|i| i == next) {
            ids.push(next.into());
        }
    }
    Ok(())
}
/// 技能只決定可公告的工具群組；權限、格式與可用副本仍由 broker 驗證。
pub fn enabled(tool: &str, ids: &[String]) -> bool {
    let has = |id: &str| ids.iter().any(|i| i == id);
    match tool {
        "list_files" | "read_file" | "load_skill" | "ask_user" | "finish" | "read_work_log"
        | "read_task_result" => true,
        "inspect_excel" | "read_excel_range" => has("excel-read"),
        "list_logs" | "read_log" | "search_logs" => has("log-analysis"),
        "outlook_folders" | "outlook_headers" | "outlook_read" => has("outlook-research"),
        "create_chart" | "chart_from_excel" | "chart_excel_range" | "export_chart_png" => {
            has("charts")
        }
        "create_working_copy" | "save_copy" | "delete_copy" => {
            has("text-edit") || has("office-edit")
        }
        "find_text" | "edit_text" => has("text-edit"),
        "edit_office" | "office_action" | "office_batch" => has("office-edit"),
        "search_files" => has("research") || has("text-edit"),
        "list_document_sections" | "read_document_section" | "summarize_document" => {
            has("research")
        }
        "list_notes"
        | "read_note"
        | "create_note"
        | "update_note"
        | "delete_note"
        | "restore_note"
        | "update_document_note" => has("notes"),
        _ => false,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_advertised_skill_can_be_loaded_through_tool_schema() {
        let tools: Value = serde_json::from_str(include_str!("tools.json")).unwrap();
        let load = tools["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["function"]["name"] == "load_skill")
            .unwrap();
        let allowed = load["function"]["parameters"]["properties"]["id"]["enum"]
            .as_array()
            .unwrap();
        let catalog = catalog();
        let entries = catalog.as_array().unwrap();
        assert_eq!(entries.len(), allowed.len());
        for entry in entries {
            assert!(allowed.contains(&entry["id"]), "{}", entry["id"]);
        }
    }
    #[test]
    fn catalog_is_short_and_groups_load_once() {
        assert!(SKILLS.iter().all(|(_, d, _)| d.chars().count() <= 30));
        let mut ids = vec![];
        assert!(!enabled("office_action", &ids));
        activate(&mut ids, "weekly-update").unwrap();
        assert!(enabled("office_action", &ids));
        let before = context(&ids).unwrap();
        activate(&mut ids, "weekly-update").unwrap();
        assert_eq!(before, context(&ids).unwrap());
        assert!(!enabled("create_chart", &ids));
        assert!(activate(&mut ids, "../../bad").is_err());
    }
}
