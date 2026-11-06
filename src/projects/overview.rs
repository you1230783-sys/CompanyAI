//! 任務總覽由原生操作紀錄衍生；不解析模型回答來判定已讀、已寫或驗證成功。
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Overview {
    pub state: String,
    pub operations: usize,
    pub reads: Vec<Access>,
    pub outputs: Vec<String>,
    /// 本機副本預覽入口，只含數量與狀態，不含差異原文。
    #[serde(default)]
    pub previews: Vec<super::review::preview::Entry>,
    #[serde(default)]
    pub task_plan: Option<super::task_plan::State>,
    pub failures: Vec<Failure>,
    pub truncated: bool,
    pub unavailable: bool,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Access {
    pub tool: String,
    pub path: String,
    /// 只顯示工具實際回報的範圍；沒有欄位時不推定已讀全文。
    pub range: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Failure {
    pub tool: String,
    pub message: String,
}
fn short(text: &str) -> String {
    let mut chars = text.chars();
    let mut value: String = chars.by_ref().take(600).collect();
    if chars.next().is_some() {
        value.push_str("…（已截斷）");
    }
    value
}
impl Overview {
    pub fn with_task_plan(mut self, plan: &super::task_plan::State) -> Self {
        self.task_plan = plan.enabled.then(|| plan.clone());
        self
    }
    pub fn unavailable(state: &str) -> Self {
        Self {
            state: state.into(),
            unavailable: true,
            ..Self::default()
        }
    }
    /// 成果路徑必須來自原生發布清單；未保存快照的成果明示無法預覽。
    pub fn with_previews(mut self, snapshots: &[super::review::Snapshot]) -> Self {
        self.previews = self
            .outputs
            .iter()
            .map(|path| {
                snapshots
                    .iter()
                    .find(|s| s.path == *path)
                    .map(super::review::preview::entry)
                    .unwrap_or_else(|| super::review::preview::Entry {
                        path: path.clone(),
                        status: "unavailable".into(),
                        summary: "此格式未提供差異預覽或未保存副本基準；CAD 請查看既有比較報告。"
                            .into(),
                    })
            })
            .collect();
        self
    }
    /// 完整操作索引包含批次的各子項；略過批次容器以免重複計數。
    pub fn from_operations(rows: &Value, outputs: &[String], state: &str) -> Self {
        let mut summary = Self {
            state: state.into(),
            ..Self::default()
        };
        let Some(rows) = rows.as_array() else {
            return Self::unavailable(state);
        };
        for row in rows {
            let request = &row["request"];
            let result = &row["result"];
            let tool = request["tool"].as_str().unwrap_or("unknown");
            if tool == "run_batch" {
                continue;
            }
            summary.operations += 1;
            if result["ok"] != true {
                if summary.failures.len() < 100 {
                    summary.failures.push(Failure {
                        tool: short(tool),
                        message: short(
                            result["error"].as_str().unwrap_or("結果未知，未確認成功。"),
                        ),
                    });
                } else {
                    summary.truncated = true;
                }
                continue;
            }
            if tool == "document_task" {
                for key in ["path", "left", "right"] {
                    if let Some(path) = request["command"][key].as_str() {
                        if summary.reads.len() < 200 {
                            summary.reads.push(Access {
                                tool: tool.into(),
                                path: short(path),
                                range: "原生Word／Excel快照；範圍外元素未讀取，詳工具紀錄".into(),
                            });
                        } else {
                            summary.truncated = true;
                        }
                    }
                }
            }
            if matches!(
                tool,
                "read_file"
                    | "read_code_section"
                    | "read_document_section"
                    | "summarize_document"
                    | "list_files"
                    | "find_text"
                    | "search_files"
                    | "list_document_sections"
                    | "read_log"
                    | "search_logs"
                    | "inspect_excel"
                    | "read_excel_range"
                    | "inspect_dataset"
                    | "chart_dataset"
                    | "chart_excel_range"
                    | "export_excel_dataset"
                    | "export_log_dataset"
                    | "export_planned_excel"
                    | "plan_excel_analysis"
                    | "list_logs"
                    | "analyze_image"
                    | "outlook_read"
                    | "outlook_folders"
                    | "outlook_index"
                    | "outlook_headers"
                    | "outlook_compare"
                    | "cad_snapshot"
                    | "cad_query"
                    | "cad_active_read"
            ) {
                let data = &result["result"];
                let path = request["path"]
                    .as_str()
                    .or_else(|| data["path"].as_str())
                    .unwrap_or(if tool.starts_with("outlook_") {
                        "已授權的Outlook範圍（不顯示郵件識別碼）"
                    } else {
                        "未提供單一檔案路徑，詳見工具紀錄"
                    });
                let mut range = Vec::new();
                for (key, label) in [
                    ("offset", "起點"),
                    ("end", "終點"),
                    ("first_line", "起始行"),
                    ("last_line", "結束行"),
                    ("total", "總量"),
                    ("start_row", "起始列"),
                    ("end_row", "結束列"),
                    ("row_count", "列數"),
                ] {
                    if let Some(number) = data[key].as_u64() {
                        range.push(format!("{label} {number}"));
                    }
                }
                if summary.reads.len() < 200 {
                    summary.reads.push(Access {
                        tool: short(tool),
                        path: short(&match request["compare_path"].as_str() {
                            Some(compare) if tool == "analyze_image" => {
                                format!("{path}；比較圖片：{compare}")
                            }
                            _ => path.into(),
                        }),
                        range: if range.is_empty() {
                            "未回報可確認的完整範圍".into()
                        } else {
                            range.join("；")
                        },
                    });
                } else {
                    summary.truncated = true;
                }
            }
        }
        summary.outputs = outputs.iter().take(200).map(|p| short(p)).collect();
        summary.truncated |= outputs.len() > 200;
        summary
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn section_reads_and_image_pairs_keep_the_actual_source_paths() {
        let tools = [
            super::super::Tool::ReadCodeSection {
                path: "source.rs".into(),
                first_line: 3,
                last_line: 7,
            },
            super::super::Tool::AnalyzeImage {
                path: "before.png".into(),
                compare_path: Some("after.png".into()),
                focus: "差異".into(),
            },
        ];
        let rows = json!([
            {"request": tools[0], "result":{"ok":true,"result":{"first_line":3,"last_line":7}}},
            {"request":{"tool":"read_document_section","path":"report.docx"},"result":{"ok":true}},
            {"request": tools[1], "result":{"ok":true}},
        ]);
        let summary = Overview::from_operations(&rows, &[], "completed");
        assert_eq!(summary.reads.len(), 3);
        assert_eq!(summary.reads[0].path, "source.rs");
        assert!(summary.reads[0].range.contains("結束行 7"));
        assert_eq!(summary.reads[1].path, "report.docx");
        assert!(summary.reads[1].range.contains("未回報"));
        assert!(summary.reads[2].path.contains("before.png"));
        assert!(summary.reads[2].path.contains("after.png"));
    }
    #[test]
    fn failed_reads_and_model_claims_never_become_successful_outputs() {
        let rows = json!([
            {"request":{"tool":"read_file","path":"a.txt"},"result":{"ok":true,"result":{"offset":0,"end":10,"total":100}}},
            {"request":{"tool":"read_file","path":"blocked.txt"},"result":{"ok":false,"error":"denied"}},
            {"request":{"tool":"record_analysis","path":"fake.docx"},"result":{"ok":true,"result":{"claim":"all validated"}}},
            {"request":{"tool":"run_batch"},"result":{"ok":true}}
        ]);
        let summary = Overview::from_operations(&rows, &["_AI_Output/copy.docx".into()], "partial");
        assert_eq!(summary.operations, 3);
        assert_eq!(summary.reads.len(), 1);
        assert!(summary.reads[0].range.contains("終點 10"));
        assert_eq!(summary.failures.len(), 1);
        assert_eq!(summary.outputs, ["_AI_Output/copy.docx"]);
    }
    #[test]
    fn truncation_and_missing_records_are_explicit() {
        let rows = json!(vec![
            json!({"request":{"tool":"read_file","path":"x"},"result":{"ok":true}});
            205
        ]);
        let summary = Overview::from_operations(&rows, &[], "cancelled");
        assert_eq!(summary.operations, 205);
        assert_eq!(summary.reads.len(), 200);
        assert!(summary.truncated);
        assert!(Overview::from_operations(&Value::Null, &[], "unknown").unavailable);
    }
}
