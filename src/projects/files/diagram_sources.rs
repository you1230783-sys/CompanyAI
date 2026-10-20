//! 只接受實際成功閱讀操作作為節點依據；對話描述沒有來源操作，維持一般圖形。
use super::*;
use crate::projects::diagram_sources::{Link, Origin};
impl Broker {
    pub(super) fn link_diagram_sources(
        &mut self,
        index: usize,
        revision: &str,
        links: &[Link],
        worker: &mut Worker,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        let diagram = self
            .diagrams
            .get(index)
            .filter(|d| d.revision == revision)
            .ok_or("圖形索引或版本不符。")?;
        let source = diagram.source.clone();
        if !source.trim_start().starts_with("flowchart")
            && !source.trim_start().starts_with("graph")
        {
            return Err("本版節點導覽適用flowchart／graph；其他圖形保留一般檢視。".into());
        }
        if links.is_empty() || links.len() > 80 {
            return Err("每張圖綁定1–80個來源節點。".into());
        }
        let mut origins = Vec::new();
        let mut used = std::collections::BTreeSet::new();
        for link in links {
            if !used.insert(&link.node_id)
                || link.node_id.is_empty()
                || link.node_id.len() > 80
                || !link
                    .node_id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
            {
                return Err("節點ID需唯一且只有英數、底線或連字號。".into());
            }
            if !source
                .split(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
                .any(|token| token == link.node_id)
            {
                return Err("圖形原碼找不到指定節點ID。".into());
            }
            if link
                .detail_diagram_index
                .is_some_and(|i| i == index || i >= self.diagrams.len())
            {
                return Err("詳細圖需引用本任務另一張實際圖形。".into());
            }
            let (request, result) = self
                .recorded_operation(&link.operation_id)?
                .ok_or("來源閱讀操作不存在。")?;
            if result["ok"] != true
                || !matches!(
                    request["tool"].as_str(),
                    Some(
                        "read_file"
                            | "read_code_section"
                            | "read_document_section"
                            | "read_excel_range"
                    )
                )
            {
                return Err(
                    "節點依據只接受成功的檔案／文件／Excel閱讀操作，不能引用進度筆記或對話描述。"
                        .into(),
                );
            }
            let data = &result["result"];
            let path = data["path"]
                .as_str()
                .or_else(|| request["path"].as_str())
                .ok_or("來源缺少檔案位置。")?
                .to_owned();
            if self.code_copy_id(&path).is_some() {
                return Err("本版來源導覽只連結原始檔案，工作副本請先發布後讀回。".into());
            }
            let raw = fingerprint(&self.project, &path)?;
            let version = data["revision"]
                .as_str()
                .ok_or("來源缺少版本。")?
                .to_owned();
            let (locator, excerpt, first, last) = if request["tool"] == "read_excel_range" {
                if version != format!("excel:{raw}") {
                    return Err("Excel來源已變更，需重新讀取再綁定。".into());
                }
                if link.first_line.is_some() || link.last_line.is_some() || link.block_id.is_some()
                {
                    return Err("Excel來源位置由已讀工作表／列欄範圍產生，不接受文字行號。".into());
                }
                (
                    format!(
                        "工作表 {}；欄 {}；起始列 {}，實際已讀 {} 列",
                        data["sheet_name"], data["columns"], data["start_row"], data["row_count"]
                    ),
                    serde_json::to_string(&json!({"headers":data["headers"],"rows":data["rows"]}))
                        .map_err(|e| e.to_string())?
                        .chars()
                        .take(500)
                        .collect(),
                    None,
                    None,
                )
            } else {
                let content = self.content(&path, cancel, worker)?;
                if text::revision(&content) != version {
                    return Err("來源已變更，不能把舊節點導向新檔案同一行。".into());
                }
                let delivered = data["text"]
                    .as_str()
                    .ok_or("來源只有筆記，請用實際原文操作ID綁定。")?;
                if let Some(id) = &link.block_id {
                    let snapshot: office::Snapshot =
                        serde_json::from_str(&content).map_err(|_| "此來源不是Office結構快照。")?;
                    let block = snapshot
                        .blocks
                        .iter()
                        .find(|b| b.id == *id)
                        .ok_or("Office段落／表格區塊不存在。")?;
                    let encoded = serde_json::to_string(block).map_err(|e| e.to_string())?;
                    if !delivered.contains(&encoded) {
                        return Err("指定Office區塊未完整交付，請先定向讀取。".into());
                    }
                    (
                        format!("{}（區塊{}）", block.label, block.id),
                        block.text.chars().take(500).collect(),
                        None,
                        None,
                    )
                } else if let Some(start) = data["first_line"].as_u64() {
                    let a = link.first_line.unwrap_or(start as usize);
                    let b = link
                        .last_line
                        .unwrap_or(data["last_line"].as_u64().ok_or("來源缺少尾行。")? as usize);
                    if a < start as usize
                        || b < a
                        || b > data["last_line"].as_u64().unwrap_or(0) as usize
                    {
                        return Err("節點行號必須位於實際已讀範圍。".into());
                    }
                    let excerpt = content
                        .split_inclusive('\n')
                        .skip(a - 1)
                        .take(b - a + 1)
                        .collect::<String>()
                        .chars()
                        .take(500)
                        .collect();
                    (format!("第{a}–{b}行"), excerpt, Some(a), Some(b))
                } else {
                    if link.first_line.is_some() || link.last_line.is_some() {
                        return Err(format!("節點{}的來源「{path}」以字元範圍交付。將此節點first_line與last_line設為null即可沿用原操作綁定字元位置，不必重讀。只有確實需要行號才read_code_section定向核對；其他節點參數保留。",link.node_id));
                    }
                    (
                        format!("字元 {}–{}", data["offset"], data["next_offset"]),
                        delivered.chars().take(500).collect(),
                        None,
                        None,
                    )
                }
            };
            origins.push(Origin {
                node_id: link.node_id.clone(),
                operation_id: link.operation_id.clone(),
                path,
                revision: version,
                source_sha256: raw,
                locator,
                excerpt,
                first_line: first,
                last_line: last,
                detail_diagram_index: link.detail_diagram_index,
            });
        }
        let mut candidate = self.diagrams[index].clone();
        candidate.node_sources = origins;
        candidate.validate()?;
        self.diagrams[index] = candidate;
        Ok(
            // 完整摘錄只留原生圖形／checkpoint，避免多個節點引用同段原文時重送數十KB。
            json!({"diagram_index":index,"revision":revision,"linked_nodes":self.diagrams[index].node_sources.iter().map(|origin|&origin.node_id).collect::<Vec<_>>(),"linked_count":self.diagrams[index].node_sources.len(),"notice":"完整來源與摘錄已保留在圖形，可點節點查看；模型僅回傳成功節點索引。來源不代表語意正確性；描述節點不綁檔案。"}),
        )
    }
}
