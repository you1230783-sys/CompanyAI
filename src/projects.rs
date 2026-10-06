//! 專案資料與固定工具契約。授權及副本所有權由桌面保存，不由模型文字決定。
pub(crate) mod agent;
pub mod charts;
pub mod datasets;
mod delegation;
pub mod diagnostics;
pub mod events;
pub mod files;
pub mod interaction;
pub mod logs;
pub mod mail;
pub mod memory;
mod model;
pub mod office;
mod pdf;
mod progress;
pub mod reply;
pub mod runner;
pub mod sandbox;
mod server_pdf;
mod skills;
pub mod steering;
pub mod text;
mod tool_calls;

use crate::{jobs, storage, AppResult};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub root: PathBuf,
    /// 公司加密軟體無法直接讀取時，由使用者明確匯入的明文快照。
    #[serde(default)]
    pub imports: BTreeMap<String, String>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Store {
    pub projects: Vec<Project>,
    pub conversations: BTreeMap<String, String>,
}
impl Store {
    pub fn load(root: &Path) -> AppResult<Self> {
        let path = root.join("projects.dpapi");
        if !path.exists() {
            return Ok(Self::default());
        }
        if fs::metadata(&path).map_err(|e| e.to_string())?.len() > 8_400_000 {
            return Err("專案紀錄過大，原檔保留。".into());
        }
        let data = storage::protect(&fs::read(path).map_err(|e| e.to_string())?, false)?;
        serde_json::from_slice(&data).map_err(|_| "專案紀錄無法解密或解析，原檔保留。".into())
    }
    pub fn save(&self, root: &Path) -> AppResult<()> {
        let data = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        if data.len() > 8_000_000 {
            return Err("專案紀錄已達 8 MB，請移除不需要的匯入文字。".into());
        }
        storage::atomic_write(
            &root.join("projects.dpapi"),
            &storage::protect(&data, true)?,
        )
    }
    pub fn project_for(&self, conversation: &str) -> Option<&Project> {
        let id = self.conversations.get(conversation)?;
        self.projects.iter().find(|p| &p.id == id)
    }
    pub fn add(&mut self, name: &str, root: PathBuf) -> AppResult<String> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 60 || self.projects.len() >= 50 {
            return Err("專案名稱需為 1–60 字，最多保存 50 個專案。".into());
        }
        files::validate_root(&root)?;
        let id = jobs::new_id()?;
        self.projects.push(Project {
            id: id.clone(),
            name: name.into(),
            root,
            imports: BTreeMap::new(),
        });
        Ok(id)
    }
}

/// 每次只允許一個操作；避免一批指令部分成功後整批重播。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "tool", rename_all = "snake_case", deny_unknown_fields)]
pub enum Tool {
    OutlookFolders {
        scope: String,
        parent_id: Option<String>,
        #[serde(default)]
        offset: usize,
    },
    OutlookHeaders {
        folder_id: String,
        start_date: String,
        end_date: String,
        cursor: Option<String>,
    },
    OutlookCompare {
        mail_ids: Vec<String>,
        #[serde(default)]
        offset: usize,
    },
    OutlookRead {
        mail_id: String,
        #[serde(default)]
        offset: usize,
    },
    ListLogs {
        path: String,
        date: Option<String>,
        category: Option<String>,
        station: Option<String>,
        #[serde(default)]
        offset: usize,
    },
    ReadLog {
        path: String,
        revision: Option<String>,
        #[serde(default = "logs::first_line")]
        start_line: usize,
        #[serde(default)]
        start_column: usize,
        #[serde(default = "logs::default_lines")]
        line_count: usize,
    },
    SearchLogs {
        query: logs::Query,
        cursor: Option<String>,
    },
    InspectExcel {
        path: String,
        #[serde(default = "office::excel::default_sheet")]
        sheet: usize,
        #[serde(default = "office::excel::default_sheet")]
        header_row: usize,
        #[serde(default = "office::excel::default_column")]
        start_column: String,
        #[serde(default = "office::excel::default_columns")]
        column_count: usize,
    },
    ReadExcelRange {
        path: String,
        revision: String,
        sheet: usize,
        columns: Vec<String>,
        #[serde(default = "office::excel::default_sheet")]
        header_row: usize,
        start_row: usize,
        #[serde(default = "office::excel::default_rows")]
        row_count: usize,
    },
    ChartExcelRange {
        path: String,
        revision: String,
        sheet: usize,
        x_column: String,
        y_columns: Vec<String>,
        #[serde(default = "office::excel::default_sheet")]
        header_row: usize,
        start_row: usize,
        #[serde(default = "office::excel::default_rows")]
        row_count: usize,
        kind: String,
        title: String,
        x_label: String,
        y_label: String,
    },
    /// 保存交接筆記後移出已封存的工具原文；不刪除證據或重設任務上限。
    CompactContext {
        working_note: String,
        superseded: Vec<String>,
        next_step: String,
    },
    ExportLogDataset {
        query: logs::Query,
        revisions: Vec<String>,
        fields: Vec<datasets::Field>,
        name: String,
    },
    ExportExcelDataset {
        path: String,
        revision: String,
        sheet: usize,
        columns: Vec<String>,
        header_row: usize,
        start_row: usize,
        row_count: usize,
        name: String,
    },
    InspectDataset {
        path: String,
        revision: Option<String>,
    },
    ChartDataset {
        path: String,
        revision: String,
        x_column: String,
        y_columns: Vec<String>,
        start_row: usize,
        row_count: usize,
        kind: String,
        title: String,
        x_label: String,
        y_label: String,
    },
    LoadSkill {
        id: String,
    },
    SearchFiles {
        paths: Vec<String>,
        query: String,
    },
    OfficeBatch {
        copy_id: String,
        revision: String,
        operations: Vec<office::Action>,
    },
    CreateChart {
        chart: charts::Chart,
    },
    ExportChartPng {
        chart_index: usize,
        name: String,
    },
    ChartFromExcel {
        path: String,
        revision: String,
        sheet: usize,
        range: String,
        kind: String,
        title: String,
        x_label: String,
        y_label: String,
    },
    SummarizeDocument {
        path: String,
        focus: String,
    },
    ListFiles {
        path: String,
    },
    ReadFile {
        path: String,
        #[serde(default)]
        offset: usize,
    },
    FindText {
        path: String,
        text: String,
    },
    CreateWorkingCopy {
        source: Option<String>,
        name: String,
    },
    EditText {
        copy_id: String,
        revision: String,
        start: usize,
        expected: String,
        replacement: String,
    },
    EditOffice {
        copy_id: String,
        revision: String,
        block_id: String,
        expected: String,
        replacement: String,
    },
    OfficeAction {
        copy_id: String,
        revision: String,
        operation: Box<office::Action>,
    },
    SaveCopy {
        copy_id: String,
        revision: String,
    },
    DeleteCopy {
        copy_id: String,
    },
    ListNotes {
        #[serde(default)]
        query: String,
    },
    ReadNote {
        id: String,
    },
    CreateNote {
        scope: String,
        title: String,
        body: String,
    },
    UpdateNote {
        id: String,
        revision: String,
        title: String,
        body: String,
    },
    DeleteNote {
        id: String,
        revision: String,
    },
    RestoreNote {
        id: String,
        revision: String,
    },
    ListDocumentSections {
        path: String,
        #[serde(default)]
        offset: usize,
    },
    ReadDocumentSection {
        path: String,
        revision: String,
        section_id: String,
    },
    UpdateDocumentNote {
        path: String,
        revision: String,
        note_revision: String,
        section_id: Option<String>,
        summary: String,
    },
    ReadWorkLog {
        operation_id: Option<String>,
        #[serde(default)]
        offset: usize,
    },
    ReadTaskResult {
        task_id: String,
        #[serde(default = "result_field")]
        field: String,
        #[serde(default)]
        offset: usize,
    },
}

fn result_field() -> String {
    "result".into()
}

impl Tool {
    /// 使用者可讀的操作名稱，避免將 JSON 或文件全文當進度訊息。
    pub fn label(&self) -> &'static str {
        match self {
            Self::OutlookFolders { .. } => "列出授權的 Outlook 資料夾",
            Self::OutlookHeaders { .. } => "讀取選定資料夾的郵件標題",
            Self::OutlookCompare { .. } => "本機比對郵件前文並建議閱讀",
            Self::OutlookRead { .. } => "讀取選定的重要郵件內文",
            Self::ListLogs { .. } => "依日期與機台篩選 LOG",
            Self::ReadLog { .. } => "分批讀取 LOG",
            Self::SearchLogs { .. } => "搜尋 LOG 時間與文字",
            Self::InspectExcel { .. } => "查看 Excel 表頭",
            Self::ReadExcelRange { .. } => "分批讀取 Excel",
            Self::ChartExcelRange { .. } => "依選取欄位建立圖表",
            Self::CompactContext { .. } => "保存筆記並精簡上下文",
            Self::ExportLogDataset { .. } => "將 LOG 擷取為本地 CSV",
            Self::ExportExcelDataset { .. } => "將 Excel 選欄保存為本地 CSV",
            Self::InspectDataset { .. } => "查看 CSV 欄位、統計與首尾預覽",
            Self::ChartDataset { .. } => "直接以本地 CSV 建立圖表",
            Self::LoadSkill { .. } => "載入工作技能",
            Self::SearchFiles { .. } => "跨文件搜尋",
            Self::OfficeBatch { .. } => "批次編輯 Office",
            Self::CreateChart { .. } | Self::ChartFromExcel { .. } => "建立圖表",
            Self::ExportChartPng { .. } => "儲存圖表 PNG",
            Self::SummarizeDocument { .. } => "快速模型摘要",
            Self::ListFiles { .. } => "讀取檔案清單",
            Self::ReadFile { .. } => "閱讀檔案",
            Self::FindText { .. } => "尋找文字",
            Self::CreateWorkingCopy { source: None, .. } => "建立新檔案",
            Self::CreateWorkingCopy { .. } => "建立副本",
            Self::OfficeAction { .. } => "編輯 Office 結構與格式",
            Self::EditOffice { .. } => "編輯 Office 文字",
            Self::EditText { .. } => "編輯文字",
            Self::SaveCopy { .. } => "儲存副本",
            Self::DeleteCopy { .. } => "刪除工作副本",
            Self::ListNotes { .. } => "查詢專案筆記",
            Self::ReadNote { .. } => "閱讀筆記",
            Self::CreateNote { .. } => "新增筆記",
            Self::UpdateNote { .. } | Self::UpdateDocumentNote { .. } => "更新筆記",
            Self::DeleteNote { .. } => "刪除筆記",
            Self::RestoreNote { .. } => "復原筆記",
            Self::ListDocumentSections { .. } => "查看文件分段摘要",
            Self::ReadDocumentSection { .. } => "閱讀文件區段",
            Self::ReadWorkLog { .. } => "查閱本次操作紀錄",
            Self::ReadTaskResult { .. } => "讀取先前任務結果",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Decision {
    Tool {
        operation_id: String,
        request: Tool,
    },
    Finish {
        message: String,
        artifacts: Vec<String>,
    },
    AskUser {
        message: String,
    },
}

pub const SKILL: &str = include_str!("projects/skill.md");

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_tools_and_ambiguous_fields_are_rejected() {
        assert!(serde_json::from_str::<Decision>(r#"{"action":"tool","operation_id":"1","request":{"tool":"powershell","command":"dir"}}"#).is_err());
        assert!(serde_json::from_str::<Decision>(
            r#"{"action":"finish","message":"ok","artifacts":[],"request":{}}"#
        )
        .is_err());
    }
}
