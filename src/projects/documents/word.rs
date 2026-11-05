//! Word 依原文順序對齊；位置僅作來源引用，不當成段落身分。
use super::{block_value, office};
use crate::AppResult;
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// 快照上限 2000 段，LCS 最多約 400 萬格（u16 約 8 MB）。
/// 相同文字是錨點；單一間隙只能標為候選替換，不宣稱語意相同。
pub(super) fn changes(left: &office::Snapshot, right: &office::Snapshot) -> AppResult<Value> {
    let (a, b) = (&left.blocks, &right.blocks);
    if a.len() > 2000 || b.len() > 2000 {
        return Err("Word 比較最多接受每份 2000 段，請縮小文件。".into());
    }
    let identity = |snapshot: &office::Snapshot, block: &office::Block| {
        let table = block
            .format
            .as_u64()
            .and_then(|index| snapshot.formats.get(index as usize))
            .and_then(|format| format.get("WordInTable"))
            .and_then(Value::as_bool);
        // 舊快照缺少表格資訊時保持 unknown，不猜成正文。
        (block.kind.clone(), block.text.clone(), table)
    };
    let ka: Vec<_> = a.iter().map(|block| identity(left, block)).collect();
    let kb: Vec<_> = b.iter().map(|block| identity(right, block)).collect();
    let mut counts = BTreeMap::new();
    for key in &ka {
        counts.entry(key).or_insert((0usize, 0usize)).0 += 1;
    }
    for key in &kb {
        counts.entry(key).or_insert((0usize, 0usize)).1 += 1;
    }
    let ambiguous = counts
        .values()
        .any(|&(x, y)| x > 0 && y > 0 && (x > 1 || y > 1));
    let width = b.len() + 1;
    let mut lengths = vec![0u16; (a.len() + 1) * width];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lengths[i * width + j] = if ka[i] == kb[j] {
                lengths[(i + 1) * width + j + 1] + 1
            } else {
                lengths[(i + 1) * width + j].max(lengths[i * width + j + 1])
            };
        }
    }
    let mut anchors = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if ka[i] == kb[j] {
            anchors.push((i, j));
            i += 1;
            j += 1;
        } else if lengths[(i + 1) * width + j] >= lengths[i * width + j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    anchors.push((a.len(), b.len()));
    let mut changes = Vec::new();
    let (mut total, mut bytes) = (0, 0);
    let mut add = |before: Option<usize>, after: Option<usize>, alignment: &str| {
        let av = before.map(|index| block_value(left, &a[index]));
        let bv = after.map(|index| block_value(right, &b[index]));
        if av == bv {
            return;
        }
        total += 1;
        let source = |blocks: &[office::Block], index: Option<usize>| {
            index.map(|index| json!({"id":blocks[index].id,"label":blocks[index].label}))
        };
        let change = json!({
            "location":format!("{} → {}", before.map(|n| a[n].id.as_str()).unwrap_or("新增"), after.map(|n| b[n].id.as_str()).unwrap_or("刪除")),
            "before_location":source(a,before),"after_location":source(b,after),
            "before":av,"after":bv,"alignment":alignment
        });
        let size = change.to_string().len();
        if changes.len() < 200 && bytes + size <= 48_000 {
            bytes += size;
            changes.push(change);
        }
    };
    let (mut start_a, mut start_b) = (0, 0);
    for (end_a, end_b) in anchors {
        // 只有同類／同表格範圍的一對間隙才列候選替換，其餘保守列增刪。
        if end_a - start_a == 1
            && end_b - start_b == 1
            && ka[start_a].0 == kb[start_b].0
            && ka[start_a].2 == kb[start_b].2
        {
            add(
                Some(start_a),
                Some(start_b),
                "相鄰錨點之間的單段候選替換；需核對",
            );
        } else {
            for index in start_a..end_a {
                add(Some(index), None, "刪除或移動來源；未推論語意");
            }
            for index in start_b..end_b {
                add(None, Some(index), "新增或移動目的；未推論語意");
            }
        }
        if end_a < a.len() && end_b < b.len() {
            add(Some(end_a), Some(end_b), "原文與種類相同，依順序對齊");
        }
        start_a = end_a + 1;
        start_b = end_b + 1;
    }
    Ok(
        json!({"changes":changes,"total_changes":total,"truncated":total>changes.len(),
        "ambiguous_alignment":ambiguous,
        "alignment":"依原文順序對齊正文／表格段落；重複文字採固定順序，不推論移動或語意",
        "notice":"只比較原生快照。重複段落可能有多種對應；before_location／after_location 分別引用兩份來源。單段替換是候選，表格內文字仍按文件順序比較，不宣稱儲存格或表格身分。頁首頁尾等範圍外元素未比較；數量不是語意變更數。"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(texts: &[&str]) -> office::Snapshot {
        office::Snapshot {
            scope: "Word".into(),
            structure: json!({}),
            calculation: vec![],
            formats: vec![json!({"Bold":false,"WordInTable":false})],
            blocks: texts
                .iter()
                .enumerate()
                .map(|(i, text)| office::Block {
                    id: format!("p{}", i + 1),
                    label: format!("段落 {}", i + 1),
                    text: (*text).into(),
                    kind: "text".into(),
                    format: json!(0),
                })
                .collect(),
        }
    }

    #[test]
    fn inserted_and_deleted_paragraphs_do_not_cascade() {
        let a = snapshot(&["甲", "乙", "丙"]);
        let b = snapshot(&["新增", "甲", "乙", "丙"]);
        let forward = changes(&a, &b).unwrap();
        assert_eq!(forward["total_changes"], 1);
        assert!(forward["changes"][0]["before_location"].is_null());
        assert_eq!(forward["changes"][0]["after_location"]["id"], "p1");
        assert_eq!(changes(&b, &a).unwrap()["total_changes"], 1);
    }

    #[test]
    fn repeated_text_is_explicitly_ambiguous_and_format_change_uses_both_locations() {
        let a = snapshot(&["重複", "重複", "結尾"]);
        let mut b = snapshot(&["前言", "重複", "重複", "結尾"]);
        b.formats.push(json!({"Bold":true,"WordInTable":false}));
        b.blocks[2].format = json!(1);
        let result = changes(&a, &b).unwrap();
        assert_eq!(result["ambiguous_alignment"], true);
        assert_eq!(result["total_changes"], 2);
        assert_eq!(result["changes"][1]["before_location"]["id"], "p2");
        assert_eq!(result["changes"][1]["after_location"]["id"], "p3");
    }

    #[test]
    fn table_text_does_not_match_body_and_empty_documents_are_supported() {
        let a = snapshot(&["相同文字"]);
        let mut b = a.clone();
        b.formats[0]["WordInTable"] = json!(true);
        assert_eq!(changes(&a, &b).unwrap()["total_changes"], 2);
        assert_eq!(
            changes(&snapshot(&[]), &snapshot(&[])).unwrap()["total_changes"],
            0
        );
        assert_eq!(
            changes(&snapshot(&["舊"]), &snapshot(&["新"])).unwrap()["total_changes"],
            1
        );
    }

    #[test]
    fn output_and_work_are_bounded_without_hiding_total() {
        let a = snapshot(&vec!["舊"; 201]);
        let b = snapshot(&[]);
        let result = changes(&a, &b).unwrap();
        assert_eq!(result["total_changes"], 201);
        assert_eq!(result["truncated"], true);
        assert!(result["changes"].as_array().unwrap().len() <= 200);
        assert!(changes(&snapshot(&vec!["過大"; 2001]), &b).is_err());
    }
}
