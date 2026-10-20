//! 圖形節點來源是獨立結構，不把任意click、URL或磁碟命令寫入Mermaid。
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Link {
    pub node_id: String,
    pub operation_id: String,
    pub first_line: Option<usize>,
    pub last_line: Option<usize>,
    pub block_id: Option<String>,
    pub detail_diagram_index: Option<usize>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Origin {
    pub node_id: String,
    pub operation_id: String,
    pub path: String,
    pub revision: String,
    pub source_sha256: String,
    pub locator: String,
    pub excerpt: String,
    pub first_line: Option<usize>,
    pub last_line: Option<usize>,
    pub detail_diagram_index: Option<usize>,
}
