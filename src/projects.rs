//! 專案資料與固定工具契約。授權及副本所有權由桌面保存，不由模型文字決定。
pub mod files;
pub mod office;
mod pdf;
pub mod reply;
pub mod runner;
pub mod sandbox;
mod server_pdf;
pub mod text;

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
    SaveCopy {
        copy_id: String,
        revision: String,
    },
    DeleteCopy {
        copy_id: String,
    },
}

impl Tool {
    /// 使用者可讀的操作名稱，避免將 JSON 或文件全文當進度訊息。
    pub fn label(&self) -> &'static str {
        match self {
            Self::ListFiles { .. } => "讀取檔案清單",
            Self::ReadFile { .. } => "閱讀檔案",
            Self::FindText { .. } => "尋找文字",
            Self::CreateWorkingCopy { source: None, .. } => "建立新檔案",
            Self::CreateWorkingCopy { .. } => "建立副本",
            Self::EditOffice { .. } => "編輯 Office 文字",
            Self::EditText { .. } => "編輯文字",
            Self::SaveCopy { .. } => "儲存副本",
            Self::DeleteCopy { .. } => "刪除工作副本",
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
