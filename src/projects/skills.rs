//! 僅載入隨 EXE 發行的工作方法，不執行專案資料夾內的腳本或設定。
use crate::AppResult;
use serde_json::{json, Value};
mod core;
const SKILLS: &[(&str, &str, &str)] = &[
    ("cad-new", "CAD新建圖面", include_str!("skills/cad-new.md")),
    (
        "cad-active",
        "CAD目前圖面閱讀與修改，不儲存",
        include_str!("skills/cad-active.md"),
    ),
    (
        "cad-copy",
        "CAD既有DWG副本閱讀與修改",
        include_str!("skills/cad-copy.md"),
    ),
    (
        "cad-excel",
        "Excel座標轉CAD圖面",
        include_str!("skills/cad-excel.md"),
    ),
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
                "cad-new" | "cad-active" | "cad-copy" | "cad-excel" => "CAD",
                _ => "文件與郵件",
            };
            json!({"category":category,"id":id,"description":description})
        })
        .collect::<Vec<_>>())
}
pub fn load(id: &str) -> AppResult<&'static str> {
    if id == "cad-drawing" {
        return Ok(include_str!("skills/cad-drawing.md"));
    }
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
/// 參考頁只提供方法，不啟用工具或切換工作階段。
pub fn read_guide(id: &str) -> AppResult<&'static str> {
    match id {
        "cad-geometry" => Ok(include_str!("skills/cad-geometry.md")),
        "cad-edits" => Ok(include_str!("skills/cad-edits.md")),
        "cad-commands" => Ok(include_str!("skills/cad-commands.md")),
        "python-tables" => Ok(include_str!("skills/python-tables.md")),
        "python-logs" => Ok(include_str!("skills/python-logs.md")),
        "python-source" => Ok(include_str!("skills/python-source.md")),
        "log-time" => Ok(include_str!("skills/log-time.md")),
        "log-events" => Ok(include_str!("skills/log-events.md")),
        "chart-details" => Ok(include_str!("skills/chart-details.md")),
        _ => load(id),
    }
}
pub fn context(ids: &[String]) -> AppResult<String> {
    for id in ids {
        load(id)?;
    }
    let mut guides = Vec::new();
    if ids.iter().any(|id| id.starts_with("cad-")) {
        guides.push(include_str!("skills/cad-common.md"));
    }
    for id in ids {
        let guide = core::guide(id);
        if !guide.is_empty() && !guides.contains(&guide) {
            guides.push(guide);
        }
    }
    Ok(guides.join("\n"))
}

/// 快速模型連詳細參考頁也不收到圖片操作指示；CAD使用者預覽說明仍可保留。
pub fn read_guide_for(id: &str, model: &str) -> AppResult<String> {
    let guide = read_guide(id)?;
    if super::vision::input::model_supported(model) {
        return Ok(guide.into());
    }
    Ok(guide
        .lines()
        .filter(|line| !line.contains("analyze_image") && !line.contains("品質模型圖片輔助"))
        .collect::<Vec<_>>()
        .join("\n"))
}

pub fn context_for(ids: &[String], model: &str) -> AppResult<String> {
    let available: Vec<_> = ids
        .iter()
        .filter(|id| id.as_str() != "image-read")
        .cloned()
        .collect();
    let mut result = context(&available)?;
    if ids.iter().any(|id| id.starts_with("cad-")) && super::vision::input::model_supported(model) {
        result.push_str("\n品質模型圖片輔助：cad_preview(show_user=false)只產生局部JPG；show_user=true交給使用者指認。產生的局部JPG可按需求用analyze_image(path,focus)判讀，並核對快照版本。此為幾何示意，不是CAD截圖；尺寸與修改目標以cad_query精確資料為準。不得從省略圖元的預覽推斷不存在。圖片不會隨預覽自動上傳。\n");
    }
    Ok(result)
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
        // 舊 checkpoint 保留原工具權限；新任務只公告與目標文件相關的工具。
        "cad_inventory" => ids.iter().any(|id| id.starts_with("cad-")),
        "cad_snapshot" | "cad_query" | "cad_preview" => {
            has("cad-active") || has("cad-copy") || has("cad-excel") || has("cad-drawing")
        }
        "cad_draw" => has("cad-new") || has("cad-drawing"),
        "cad_active_read" => has("cad-active") || has("cad-excel") || has("cad-drawing"),
        "cad_active_edit" => has("cad-active") || has("cad-drawing"),
        "cad_read" => has("cad-copy") || has("cad-excel") || has("cad-drawing"),
        "cad_edit" => has("cad-copy") || has("cad-drawing"),
        "cad_excel_plan" | "cad_excel_apply" => has("cad-excel") || has("cad-drawing"),
        "record_log_events" => has("log-analysis") || has("python-analysis"),
        "run_python" => has("python-analysis"),
        "analyze_image" | "read_mail_notes" | "set_work_stage" | "run_batch" => true,
        "plan_excel_analysis" | "export_planned_excel" => has("excel-read"),
        "outlook_compare" => has("outlook-coverage"),
        "list_files" | "read_file" | "load_skill" | "read_skill_guide" | "ask_preference"
        | "ask_user" | "finish" | "read_work_log" | "read_task_result" | "compact_context"
        | "read_code_section" | "find_text" => true,
        "record_analysis" => {
            has("log-analysis")
                || has("python-analysis")
                || has("research")
                || has("notes")
                || has("excel-read")
        }
        "export_log_dataset" | "export_excel_dataset" | "inspect_dataset" | "chart_dataset" => {
            has("dataset-charts")
        }
        "inspect_excel" | "read_excel_range" => {
            has("excel-read") || has("dataset-charts") || has("cad-excel")
        }
        "list_logs" | "read_log" | "search_logs" => has("log-analysis"),
        "outlook_folders" | "outlook_headers" | "outlook_read" | "outlook_index" => {
            has("outlook-research")
        }
        "plan_code_change" | "test_python" | "review_code_change" | "check_python"
        | "edit_code_section" => has("python-edit"),
        "edit_chart" => has("chart-edit"),
        "inspect_chart" | "export_chart_png" => {
            has("chart-edit") || has("charts") || has("dataset-charts")
        }
        "create_chart"
        | "chart_from_excel"
        | "chart_excel_range"
        | "transform_chart"
        | "set_chart_policy"
        | "set_chart_reference_lines" => has("charts"),
        "create_working_copy" | "save_copy" | "delete_copy" => {
            has("text-edit") || has("office-edit") || has("python-edit")
        }
        "edit_text" => has("text-edit") || has("python-edit"),
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
    fn cad_stages_and_reference_pages_do_not_load_unrelated_tools() {
        let mut ids = vec![];
        activate(&mut ids, "cad-new").unwrap();
        assert!(enabled("cad_draw", &ids));
        assert!(!enabled("cad_edit", &ids));
        assert!(!enabled("cad_excel_apply", &ids));
        assert!(context(&ids).unwrap().len() < 5000);
        activate(&mut ids, "cad-active").unwrap();
        assert!(enabled("cad_active_edit", &ids));
        assert!(!enabled("cad_draw", &ids));
        assert!(read_guide("cad-geometry").is_ok());
        assert!(activate(&mut ids, "cad-geometry").is_err());
        assert!(enabled("cad_active_edit", &ids));
        activate(&mut ids, "cad-excel").unwrap();
        assert!(enabled("inspect_excel", &ids));
        assert!(enabled("cad_excel_apply", &ids));
        assert!(!enabled("cad_active_edit", &ids));
        activate(&mut ids, "outlook-research").unwrap();
        assert!(!enabled("outlook_compare", &ids));
        activate(&mut ids, "outlook-coverage").unwrap();
        assert!(enabled("outlook_compare", &ids));
        assert!(!enabled("outlook_read", &ids));
    }
    #[test]
    fn shared_core_guides_are_not_repeated() {
        let mut ids = vec![];
        activate(&mut ids, "paper-evidence").unwrap();
        assert_eq!(context(&ids).unwrap(), core::guide("research"));
    }
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
