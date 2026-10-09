//! 僅載入隨 EXE 發行的工作方法，不執行專案資料夾內的腳本或設定。
use crate::AppResult;
use serde_json::{json, Value};
mod core;
const SKILLS: &[(&str, &str, &str)] = &[
    (
        "python-edit",
        "Python分段修改、需求核對與隔離功能測試",
        include_str!("skills/python-edit.md"),
    ),
    (
        "chart-edit",
        "編輯既有圖表、配色、排版與圖中文字",
        include_str!("skills/chart-edit.md"),
    ),
    (
        "python-analysis",
        "多檔關聯、統計、LOG配對與XLSX，主動選用Python",
        include_str!("skills/python-analysis.md"),
    ),
    (
        "outlook-coverage",
        "本機比對郵件前文與建議閱讀",
        include_str!("skills/outlook-coverage.md"),
    ),
    (
        "dataset-charts",
        "大量資料存 CSV、預覽與本地直接畫圖",
        include_str!("skills/dataset-charts.md"),
    ),
    (
        "context-management",
        "更正方向、交接筆記與主動精簡上下文",
        include_str!("skills/context-management.md"),
    ),
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
    catalog_for("quality")
}
pub fn catalog_for(_model: &str) -> Value {
    json!(SKILLS
        .iter()
        .map(|(id, description, _)| {
            let category = match *id {
                "python-analysis" | "python-edit" | "log-analysis" | "excel-read"
                | "multi-file-excel" => "資料分析",
                "charts" | "dataset-charts" | "chart-edit" => "圖表",
                _ => "文件與郵件",
            };
            json!({"category":category,"id":id,"description":description})
        })
        .collect::<Vec<_>>())
}
pub fn load(id: &str) -> AppResult<&'static str> {
    // 僅相容舊 checkpoint 尚待完成的載入；新目錄不公告圖片技能，基本閱讀已內建。
    if id == "image-read" {
        return Ok("");
    }
    SKILLS
        .iter()
        .find(|s| s.0 == id)
        .map(|s| s.2)
        .ok_or("未知技能；請依技能目錄指定 id。".into())
}
pub fn context(ids: &[String]) -> AppResult<String> {
    for id in ids {
        load(id)?;
    }
    Ok(ids
        .iter()
        .map(|id| core::guide(id))
        .collect::<Vec<_>>()
        .join("\n"))
}

pub fn context_for(ids: &[String], _model: &str) -> AppResult<String> {
    let available: Vec<_> = ids
        .iter()
        .filter(|id| id.as_str() != "image-read")
        .cloned()
        .collect();
    context(&available)
}

/// 圖片併入基本檔案閱讀，依模型能力提供方法，不需要多一輪載入技能。
pub fn image_context_for(model: &str) -> &'static str {
    if super::vision::input::model_supported(model) {
        "圖片：analyze_image(path,compare_path,focus)讀專案JPG/PNG/BMP/TIF/TIFF/GIF；第二路徑null為單圖。先本機轉JPG，每張5MB／雙圖合計8MB，每任務100張。TIFF多頁或動畫只讀第一幀並明示範圍。只回文字重點，關鍵數字需核對；不讀Outlook附件或掃描PDF。"
    } else {
        super::vision::input::UNSUPPORTED_MODEL
    }
}

/// 高階工作技能載入所需基本組；同一份說明在 system 只出現一次。
pub fn activate(ids: &mut Vec<String>, id: &str) -> AppResult<()> {
    load(id)?;
    // 技能代表當前階段；切換時收回舊詳細說明與工具，資料與成果仍由 broker 保存。
    ids.clear();
    let dependencies: &[&str] = match id {
        "outlook-research" => &["outlook-coverage"],
        "outlook-coverage" => &["outlook-research"],
        "paper-evidence" => &["research"],
        "multi-file-excel" => &["excel-read"],
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
        "run_python" => has("python-analysis"),
        "analyze_image" | "read_mail_notes" | "set_work_stage" | "run_batch" => true,
        "plan_excel_analysis" | "export_planned_excel" => has("excel-read"),
        "outlook_compare" => has("outlook-coverage"),
        "list_files" | "read_file" | "load_skill" | "read_skill_guide" | "ask_preference"
        | "ask_user" | "finish" | "read_work_log" | "read_task_result" | "compact_context" => true,
        "record_analysis" => {
            has("log-analysis")
                || has("python-analysis")
                || has("research")
                || has("notes")
                || has("excel-read")
        }
        "export_log_dataset" | "export_excel_dataset" | "inspect_dataset" | "chart_dataset" => {
            has("dataset-charts") || has("python-analysis") || has("charts")
        }
        "inspect_excel" | "read_excel_range" => has("excel-read") || has("dataset-charts"),
        "list_logs" | "read_log" | "search_logs" => has("log-analysis"),
        "outlook_folders" | "outlook_headers" | "outlook_read" | "outlook_index" => {
            has("outlook-research")
        }
        "plan_code_change" | "test_python" | "review_code_change" | "check_python"
        | "read_code_section" | "edit_code_section" => has("python-edit"),
        "edit_chart" => has("chart-edit"),
        "inspect_chart" | "export_chart_png" => {
            has("chart-edit") || has("charts") || has("dataset-charts")
        }
        "create_chart"
        | "chart_from_excel"
        | "chart_excel_range"
        | "transform_chart"
        | "set_chart_policy"
        | "set_chart_reference_lines" => has("charts") || has("dataset-charts"),
        "create_working_copy" | "save_copy" | "delete_copy" => {
            has("text-edit") || has("office-edit") || has("python-edit")
        }
        "find_text" | "edit_text" => has("text-edit") || has("python-edit"),
        "edit_office" | "office_action" | "office_batch" => has("office-edit"),
        "search_files" => has("research") || has("text-edit") || has("python-edit"),
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
        assert!(enabled("analyze_image", &ids));
        assert!(!catalog().to_string().contains("image-read"));
        assert!(!enabled("office_action", &ids));
        activate(&mut ids, "weekly-update").unwrap();
        assert!(!enabled("office_action", &ids));
        activate(&mut ids, "office-edit").unwrap();
        assert!(enabled("office_action", &ids));
        activate(&mut ids, "python-analysis").unwrap();
        assert!(!enabled("office_action", &ids));
        assert!(enabled("run_python", &ids));
        assert!(context(&ids).unwrap().chars().count() < 1500);
        activate(&mut ids, "weekly-update").unwrap();
        let before = context(&ids).unwrap();
        activate(&mut ids, "weekly-update").unwrap();
        assert_eq!(before, context(&ids).unwrap());
        assert!(!enabled("create_chart", &ids));
        assert!(activate(&mut ids, "../../bad").is_err());
    }
}
