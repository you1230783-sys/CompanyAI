//! 本機差異只讀取固定快照；不加入工具清單、模型歷史或診斷匯出。
use super::{Origin, Snapshot};
use crate::{projects, AppResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const MAX_LINES: usize = 2000;
const MAX_CHANGES: usize = 200;
const MAX_BYTES: usize = 48_000;
const PAGE_SIZE: usize = 25;

/// 總覽只保存數量與狀態，不複製原文到對話訊息。
#[derive(Clone, Serialize, Deserialize)]
pub struct Entry {
    pub path: String,
    pub status: String,
    pub summary: String,
}

fn status(snapshot: &Snapshot) -> (&'static str, &'static str) {
    if matches!(snapshot.origin, Some(Origin::New)) {
        return ("new", "新建");
    }
    let ext = std::path::Path::new(&snapshot.name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if matches!(
        ext.as_str(),
        "ppt" | "pptx" | "pptm" | "pdf" | "dwg" | "dxf" | "png" | "jpg" | "jpeg"
    ) {
        return (
            "unsupported",
            "此格式未提供差異預覽；CAD 請查看既有比較報告",
        );
    }
    if snapshot.before.is_none() {
        return ("unavailable", "未保存建立時基準，無法比較");
    }
    ("available", "查看修改")
}

/// 來源變更只產生提示；比較基準永遠不使用目前磁碟內容替換。
pub fn source_notice(snapshot: &Snapshot, project: &projects::Project) -> String {
    match &snapshot.origin {
        Some(Origin::Source { path, fingerprint }) => {
            match projects::files::fingerprint(project, path) {
                Ok(current) if current == *fingerprint => {
                    "原件版本未變更；以下比較建立時與已保存成果的快照。".into()
                }
                Ok(_) => "原件已變更，差異以建立副本時的版本為準。".into(),
                Err(_) => {
                    "目前無法核對原件（可能已移動、刪除或被占用）；差異仍以建立副本時的版本為準。"
                        .into()
                }
            }
        }
        Some(Origin::New) => "新建文件，沒有原件可比較。".into(),
        None => "舊紀錄未保存來源檔案指紋，無法判斷原件目前是否變更；僅比較已保存基準。".into(),
    }
}

/// 沿用既有 Word／Excel 原生比較，不重新開啟 Office 或變更原比較工具輸出。
fn comparison(snapshot: &Snapshot) -> AppResult<Value> {
    let before = snapshot.before.as_deref().ok_or("沒有建立時基準。")?;
    if snapshot.office {
        let left = serde_json::from_str(before).map_err(|_| "建立時 Office 快照無法讀取。")?;
        let right =
            serde_json::from_str(&snapshot.after).map_err(|_| "成果 Office 快照無法讀取。")?;
        projects::documents::compare(&left, &right, None)
    } else {
        Ok(text_changes(before, &snapshot.after))
    }
}

fn summary(data: &Value) -> String {
    let count = data["total_changes"].as_u64().unwrap_or(0);
    let scope = if data["input_truncated"] == true {
        "已比較範圍"
    } else {
        "快照範圍"
    };
    format!(
        "{scope}共 {count} 處差異{}{}",
        if data["structure_changed"] == true {
            "，結構有變更"
        } else {
            ""
        },
        if data["truncated"] == true {
            "（顯示已截斷）"
        } else {
            ""
        }
    )
}

pub fn entry(snapshot: &Snapshot) -> Entry {
    let (mut state, label) = status(snapshot);
    let summary = if state == "available" {
        match comparison(snapshot) {
            Ok(data) => summary(&data),
            Err(_) => {
                state = "unavailable";
                "快照超過比較上限或無法讀取；未推定為相同。".into()
            }
        }
    } else {
        label.into()
    };
    Entry {
        path: snapshot.path.clone(),
        status: state.into(),
        summary,
    }
}

/// 一頁最多 25 處，整份比較最多 200 處／48KB；總數與截斷狀態獨立回報。
pub fn page(snapshot: &Snapshot, offset: usize) -> AppResult<Value> {
    let (state, label) = status(snapshot);
    if state != "available" {
        return Ok(
            json!({"status":state,"summary":label,"changes":[],"offset":0,"shown":0,"total_changes":0}),
        );
    }
    let mut data = comparison(snapshot)?;
    let rows = data["changes"].as_array().ok_or("比較結果缺少差異清單。")?;
    if offset > rows.len() {
        return Err("差異分頁位置已失效，請重新開啟。".into());
    }
    let shown = rows.len();
    let end = (offset + PAGE_SIZE).min(shown);
    let changes = rows[offset..end].to_vec();
    data["summary"] = json!(summary(&data));
    data["changes"] = json!(changes);
    data["status"] = json!(state);
    data["offset"] = json!(offset);
    data["shown"] = json!(shown);
    data["next_offset"] = if end < shown { json!(end) } else { Value::Null };
    data["page_size"] = json!(PAGE_SIZE);
    data["before_revision"] = json!(snapshot.before_revision);
    data["after_revision"] = json!(snapshot.after_revision);
    data["name"] = json!(snapshot.name);
    Ok(data)
}

/// 有界 LCS。相鄰錨點之間的行依序配對為修改，額外行才標為增刪。
/// 超過 2000 行只比較前段，回報兩側完整行數，絕不把未比較尾段當作相同。
fn text_changes(before: &str, after: &str) -> Value {
    let lines = |text: &str| text.lines().map(str::to_owned).collect::<Vec<_>>();
    let (left, right) = (lines(before), lines(after));
    let (a, b) = (
        &left[..left.len().min(MAX_LINES)],
        &right[..right.len().min(MAX_LINES)],
    );
    let width = b.len() + 1;
    let mut lcs = vec![0u16; (a.len() + 1) * width];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i * width + j] = if a[i] == b[j] {
                1 + lcs[(i + 1) * width + j + 1]
            } else {
                lcs[(i + 1) * width + j].max(lcs[i * width + j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut anchors = Vec::new();
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            anchors.push((i, j));
            i += 1;
            j += 1;
        } else if lcs[(i + 1) * width + j] >= lcs[i * width + j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    anchors.push((a.len(), b.len()));
    let (mut start_a, mut start_b, mut total, mut bytes) = (0, 0, 0, 0);
    let mut changes = Vec::new();
    let mut counts = [0usize; 3];
    for (end_a, end_b) in anchors {
        for n in 0..(end_a - start_a).max(end_b - start_b) {
            let ai = (start_a + n < end_a).then_some(start_a + n);
            let bi = (start_b + n < end_b).then_some(start_b + n);
            let kind = match (ai, bi) {
                (Some(_), Some(_)) => 0,
                (None, _) => 1,
                _ => 2,
            };
            counts[kind] += 1;
            let row = json!({"kind":(["modified","added","deleted"][kind]),
                "before_line":ai.map(|i|i+1),"after_line":bi.map(|j|j+1),
                "before":ai.map(|i|json!({"text":a[i]})),"after":bi.map(|j|json!({"text":b[j]}))});
            total += 1;
            let size = row.to_string().len();
            if changes.len() < MAX_CHANGES && bytes + size <= MAX_BYTES {
                bytes += size;
                changes.push(row);
            }
        }
        start_a = end_a + 1;
        start_b = end_b + 1;
    }
    let input_truncated = left.len() > MAX_LINES || right.len() > MAX_LINES;
    json!({"changes":changes,"total_changes":total,"modified":counts[0],"added":counts[1],"deleted":counts[2],
        "before_lines":left.len(),"after_lines":right.len(),"compared_before_lines":a.len(),"compared_after_lines":b.len(),
        "input_truncated":input_truncated,"truncated":input_truncated || total>changes.len(),
        "notice":"依行 LCS 比較固定內容快照；位置配對不代表語意相同。超過上限的行／差異未顯示，不能推定為相同。"})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot(name: &str, before: &str, after: &str) -> Snapshot {
        Snapshot {
            origin: Some(Origin::Source {
                path: "source.txt".into(),
                fingerprint: "old".into(),
            }),
            name: name.into(),
            path: format!("_AI_Output/{name}"),
            before_revision: Some("before".into()),
            after_revision: "after".into(),
            before: Some(before.into()),
            after: after.into(),
            office: false,
            draft: false,
        }
    }
    fn office(scope: &str, texts: &[&str]) -> projects::office::Snapshot {
        projects::office::Snapshot {
            scope: scope.into(),
            structure: json!({}),
            formats: vec![json!({})],
            calculation: vec![],
            blocks: texts
                .iter()
                .enumerate()
                .map(|(i, text)| projects::office::Block {
                    id: format!("p{i}"),
                    label: format!("Sheet!A{}", i + 1),
                    text: (*text).into(),
                    kind: "text".into(),
                    format: json!(0),
                })
                .collect(),
        }
    }
    #[test]
    fn word_insert_and_excel_formula_value_structure_reuse_native_comparison() {
        let a = office("Word", &["甲", "乙", "丙"]);
        let b = office("Word", &["甲", "新增", "乙", "丙"]);
        let mut review = snapshot("a.docx", &a.serialize().unwrap(), &b.serialize().unwrap());
        review.office = true;
        let diff = page(&review, 0).unwrap();
        assert_eq!(diff["total_changes"], 1);
        assert!(diff["changes"][0]["before"].is_null());
        let mut a = office("Excel", &["=1+1", "10"]);
        a.blocks[0].kind = "formula".into();
        a.calculation = vec![json!({"id":"p0","value":2,"display":"2"})];
        let mut b = a.clone();
        b.blocks[0].text = "=2".into();
        b.blocks[1].text = "11".into();
        b.structure = json!({"sheets":["Sheet","New"]});
        review.name = "a.xlsx".into();
        review.before = Some(a.serialize().unwrap());
        review.after = b.serialize().unwrap();
        let diff = page(&review, 0).unwrap();
        assert_eq!(diff["total_changes"], 2);
        assert_eq!(
            diff["changes"][0]["before"]["calculation"],
            diff["changes"][0]["after"]["calculation"]
        );
        assert_ne!(
            diff["changes"][0]["before"]["text"],
            diff["changes"][0]["after"]["text"]
        );
        assert_eq!(diff["changes"][1]["after"]["text"], "11");
        assert_eq!(diff["structure_changed"], true);
    }
    #[test]
    fn pages_new_unsupported_and_legacy_do_not_invent_baselines() {
        let a = (0..60).map(|n| format!("old{n}\n")).collect::<String>();
        let b = (0..60).map(|n| format!("new{n}\n")).collect::<String>();
        let mut review = snapshot("a.txt", &a, &b);
        assert_eq!(page(&review, 0).unwrap()["next_offset"], 25);
        assert_eq!(page(&review, 25).unwrap()["changes"][0]["before_line"], 26);
        assert_eq!(
            page(&review, 50).unwrap()["changes"]
                .as_array()
                .unwrap()
                .len(),
            10
        );
        assert!(page(&review, 61).is_err());
        review.origin = Some(Origin::New);
        assert_eq!(entry(&review).status, "new");
        review.origin = None;
        review.name = "slides.pptx".into();
        assert_eq!(page(&review, 0).unwrap()["status"], "unsupported");
        review.name = "a.txt".into();
        review.before = None;
        assert_eq!(entry(&review).status, "unavailable");
        let mut old = serde_json::to_value(&review).unwrap();
        old.as_object_mut().unwrap().remove("origin");
        assert!(serde_json::from_value::<Snapshot>(old)
            .unwrap()
            .origin
            .is_none());
    }
    #[test]
    fn changed_source_warns_but_keeps_the_saved_diff() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(".build")
            .join(format!("diff-{}", crate::jobs::new_id().unwrap()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("source.txt"), "old").unwrap();
        let project = projects::Project {
            id: "diff".into(),
            name: "diff".into(),
            root,
            imports: Default::default(),
        };
        let mut review = snapshot("a.txt", "old", "edited");
        review.origin = Some(Origin::Source {
            path: "source.txt".into(),
            fingerprint: projects::files::fingerprint(&project, "source.txt").unwrap(),
        });
        let initial = page(&review, 0).unwrap();
        assert!(source_notice(&review, &project).contains("未變更"));
        std::fs::write(project.root.join("source.txt"), "external change").unwrap();
        assert!(source_notice(&review, &project).contains("原件已變更"));
        assert_eq!(page(&review, 0).unwrap(), initial);
        std::fs::remove_file(project.root.join("source.txt")).unwrap();
        assert!(source_notice(&review, &project).contains("無法核對"));
    }
    #[test]
    fn three_modified_lines_and_insertions_keep_exact_line_numbers() {
        let diff = text_changes("a\nb\nc\nd\ne\n", "a\nB\nC\nD\ne\n");
        assert_eq!(diff["total_changes"], 3);
        assert_eq!(diff["modified"], 3);
        assert_eq!(diff["changes"][0]["before_line"], 2);
        assert_eq!(diff["changes"][2]["after_line"], 4);
        let diff = text_changes("a\nb\nc", "a\ninsert\nb\nc");
        assert_eq!(diff["total_changes"], 1);
        assert_eq!(diff["changes"][0]["kind"], "added");
        assert_eq!(diff["changes"][0]["after_line"], 2);
    }
    #[test]
    fn caps_expose_full_counts_and_uncompared_tail() {
        let a = (0..2100).map(|n| format!("old{n}\n")).collect::<String>();
        let b = (0..2100).map(|n| format!("new{n}\n")).collect::<String>();
        let diff = text_changes(&a, &b);
        assert_eq!(diff["before_lines"], 2100);
        assert_eq!(diff["total_changes"], 2000);
        assert_eq!(diff["input_truncated"], true);
        assert_eq!(diff["truncated"], true);
        assert!(diff["changes"].as_array().unwrap().len() <= 200);
    }
}
