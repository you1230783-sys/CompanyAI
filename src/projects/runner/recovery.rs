//! 專案代理的recovery流程；原生操作順序與恢復契約維持不變。
use super::*;

/// 重啟後只恢復顯示紀錄，不執行任何原有操作。
pub fn recover_activity(root: &Path, id: &str) -> AppResult<Vec<Activity>> {
    jobs::validate_id(id)?;
    let path = root.join("project-runs").join(format!("{id}.dpapi"));
    if !path.exists() {
        return Ok(Vec::new());
    }
    if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 64_000_000 {
        return Err("上次任務紀錄過大。".into());
    }
    let bytes = storage::protect(&std::fs::read(path).map_err(|e| e.to_string())?, false)?;
    let record: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    Ok(record["activity"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|value| serde_json::from_value::<Activity>(value.clone()).ok())
        .collect())
}

/// 重啟只恢復可見結果／中斷說明，不恢復檔案授權，也不重播工具。
pub fn recover(root: &Path, id: &str) -> AppResult<String> {
    jobs::validate_id(id)?;
    let path = root.join("project-runs").join(format!("{id}.dpapi"));
    if !path.exists() {
        return Ok("上次專案任務在開始前中斷；未自動重新執行，請重新送出需求。".into());
    }
    if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 64_000_000 {
        return Err("上次任務紀錄過大，原檔保留。".into());
    }
    let bytes = storage::protect(&std::fs::read(path).map_err(|e| e.to_string())?, false)?;
    let record: Value = serde_json::from_slice(&bytes).map_err(|_| "上次專案任務紀錄無法解析。")?;
    if let Some(text) = record["result"]["Ok"].as_str() {
        return Ok(text.into());
    }
    if paused_available(root, id) {
        return Ok("上次任務中斷，已找到安全 checkpoint。按「繼續」後會核對來源與帳號，再接續原請求；不會自動重播未知寫入。".into());
    }
    let reason = record["result"]["Err"].as_str().unwrap_or(
        "應用程式已退出，先前的本機操作未完成；不會自動重播。尚未發布的記憶體副本已釋放。",
    );
    let outputs: Vec<_> = record["outputs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .collect();
    Ok(format!(
        "上次專案任務未完成：{reason}\n\n可能已建立的檔案（需重新確認）：\n{}",
        outputs.join("\n")
    ))
}

/// 歷史對話與異常復原共用已保存的結構化圖表，不重新呼叫模型。
pub fn recover_charts(root: &Path, id: &str) -> Vec<crate::projects::charts::Chart> {
    let read = || -> AppResult<Vec<crate::projects::charts::Chart>> {
        jobs::validate_id(id)?;
        let bytes = std::fs::read(root.join("project-runs").join(format!("{id}.dpapi")))
            .map_err(|e| e.to_string())?;
        let data: Value =
            serde_json::from_slice(&storage::protect(&bytes, false)?).map_err(|e| e.to_string())?;
        let charts: Vec<crate::projects::charts::Chart> =
            serde_json::from_value(data.get("charts").cloned().unwrap_or(json!([])))
                .map_err(|e| e.to_string())?;
        for chart in &charts {
            chart.validate()?;
        }
        Ok(charts)
    };
    read().unwrap_or_default()
}

/// 從加密工作日誌還原 Mermaid，不重新呼叫模型。
pub fn recover_diagrams(root: &Path, id: &str) -> Vec<crate::projects::diagrams::Diagram> {
    let read = || -> AppResult<Vec<crate::projects::diagrams::Diagram>> {
        jobs::validate_id(id)?;
        let bytes = std::fs::read(root.join("project-runs").join(format!("{id}.dpapi")))
            .map_err(|e| e.to_string())?;
        let data: Value =
            serde_json::from_slice(&storage::protect(&bytes, false)?).map_err(|e| e.to_string())?;
        let diagrams: Vec<crate::projects::diagrams::Diagram> =
            serde_json::from_value(data.get("diagrams").cloned().unwrap_or(json!([])))
                .map_err(|e| e.to_string())?;
        if diagrams.len() > crate::projects::diagrams::MAX_DIAGRAMS {
            return Err("圖形數量超限。".into());
        }
        for diagram in &diagrams {
            diagram.validate()?;
        }
        Ok(diagrams)
    };
    read().unwrap_or_default()
}

/// 只列明確唯讀／記憶體操作；新增工具預設不允許崩潰重播。
pub(super) fn replay_safe(tool: &crate::projects::Tool) -> bool {
    use crate::projects::Tool::*;
    matches!(
        tool,
        InspectMermaid { .. }
            | ReadReadingNotes { .. }
            | ReadWorkflow { .. }
            | RecordReadingNotes { .. }
            | RegisterReportValues { .. }
            | LinkDiagramSources { .. }
            | ListFiles { .. }
            | ReadMailNotes { .. }
            | SetWorkStage { .. }
            | AnalyzeImage { .. }
            | ReadFile { .. }
            | FindText { .. }
            | SearchFiles { .. }
            | ListLogs { .. }
            | ReadLog { .. }
            | SearchLogs { .. }
            | LoadSkill { .. }
            | ReadWorkLog { .. }
            | ListNotes { .. }
            | ReadNote { .. }
            | ReadTaskResult { .. }
            | InspectExcel { .. }
            | PlanExcelAnalysis { .. }
            | ReadExcelRange { .. }
            | OutlookFolders { .. }
            | OutlookHeaders { .. }
            | OutlookCompare { .. }
            | OutlookRead { .. }
            | OutlookIndex { .. }
            | ReadSkillGuide { .. }
            | AskPreference { .. }
    )
}

pub(super) fn invalidate_pause(run: &Run) -> AppResult<()> {
    checkpoint(
        &pause_path(&run.root, &run.id)?,
        &json!({"version":1,"available":false}),
    )
}

/// 診斷只保留近期完整請求；操作原文另在加密操作簿，避免每日重複累積整份 context。
pub(super) fn trim_journal(record: &mut Value) {
    for (key, limit) in [("requests", 8), ("operations", 80)] {
        if let Some(rows) = record[key].as_array_mut() {
            if rows.len() > limit {
                rows.drain(..rows.len() - limit);
            }
        }
    }
}

/// 只還原選定成果的固定比較快照，不重新讀取現在的原件。
pub fn recover_review(
    root: &Path,
    id: &str,
    path: &str,
) -> AppResult<crate::projects::review::Snapshot> {
    jobs::validate_id(id)?;
    let file = root.join("project-runs").join(format!("{id}.dpapi"));
    if std::fs::metadata(&file).map_err(|e| e.to_string())?.len() > 64_000_000 {
        return Err("任務紀錄過大，無法開啟比較。".into());
    }
    let bytes = storage::protect(&std::fs::read(file).map_err(|e| e.to_string())?, false)?;
    let record: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let snapshots: Vec<crate::projects::review::Snapshot> =
        serde_json::from_value(record["reviews"].clone())
            .map_err(|_| "此舊成果沒有保存比較基準，無法還原修改前內容。")?;
    snapshots
        .into_iter()
        .find(|s| s.path.replace('\\', "/") == path.replace('\\', "/"))
        .ok_or("此成果沒有可用的比較快照。".into())
}

pub fn recover_analysis(root: &Path, id: &str) -> Option<crate::projects::analysis::State> {
    jobs::validate_id(id).ok()?;
    let path = root.join("project-runs").join(format!("{id}.dpapi"));
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() > 64_000_000 {
        return None;
    }
    let record: Value = serde_json::from_slice(&storage::protect(&bytes, false).ok()?).ok()?;
    serde_json::from_value(record["analysis"].clone()).ok()
}

/// 舊紀錄或未寫完的終態不猜測結果，由畫面顯示「沒有總覽紀錄」。
pub fn recover_overview(root: &Path, id: &str) -> Option<crate::projects::overview::Overview> {
    jobs::validate_id(id).ok()?;
    let path = root.join("project-runs").join(format!("{id}.dpapi"));
    if std::fs::metadata(&path).ok()?.len() > 64_000_000 {
        return None;
    }
    let data = storage::protect(&std::fs::read(path).ok()?, false).ok()?;
    let record: Value = serde_json::from_slice(&data).ok()?;
    serde_json::from_value(record["overview"].clone()).ok()
}

/// 只有舊任務實際保存的時間可恢復；沒有欄位的版本顯示空白。
pub fn recover_local_time(root: &Path, id: &str) -> Option<String> {
    let read = || -> AppResult<Option<String>> {
        jobs::validate_id(id)?;
        let path = root.join("project-runs").join(format!("{id}.dpapi"));
        if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 64_000_000 {
            return Ok(None);
        }
        let bytes = storage::protect(&std::fs::read(path).map_err(|e| e.to_string())?, false)?;
        let record: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        Ok(record["finished_local"].as_str().map(str::to_owned))
    };
    read().ok().flatten()
}
