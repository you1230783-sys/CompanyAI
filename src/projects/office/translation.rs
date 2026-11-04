//! Word格式副本翻譯：只替換已核對的正文／表格文字範圍，段落末標記與特殊物件保留。
//! 混合字型段落按格式切分，不把整段譯文套成第一個字的格式。
use super::*;
use crate::projects::documents::Translated;
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn range(document: &IDispatch, start: i32, end: i32) -> AppResult<IDispatch> {
    obj(invoke(
        document,
        "Range",
        vec![start.into(), end.into()],
        false,
    )?)
}

fn signature(target: &IDispatch) -> AppResult<Value> {
    let font = child(target, "Font")?;
    let mut value = authoring::inspect_format(target, "Documents")?;
    // Font.Name會隨文字的語系混合而變成未定義，並不等於字型設定被改掉。
    // 比較各文字系統的字型設定，才能核對英譯中同時保留西文與東亞字型。
    if let Some(properties) = value.as_object_mut() {
        properties.remove("Name");
    }
    for name in [
        "NameAscii",
        "NameOther",
        "NameBi",
        "Underline",
        "StrikeThrough",
        "DoubleStrikeThrough",
        "Subscript",
        "Superscript",
        "SmallCaps",
        "AllCaps",
        "Hidden",
        "Spacing",
        "Scaling",
        "Position",
        "Kerning",
    ] {
        value[name] = authoring::scalar(&font, name)?;
    }
    value["HighlightColorIndex"] = authoring::scalar(target, "HighlightColorIndex")?;
    value["Style"] = json!(string(&get(&child(target, "Style")?, "NameLocal")?)?);
    Ok(value)
}

fn snapshot(session: &Session, cancel: &AtomicBool) -> AppResult<Snapshot> {
    let (base, targets) = session.snapshot(cancel)?;
    let mut result = Snapshot {
        scope: "Word格式翻譯：正文與表格的可編輯格式區段；頁首頁尾、文字方塊、註解及特殊欄位不翻譯"
            .into(),
        structure: base.structure,
        formats: vec![],
        blocks: vec![],
        calculation: vec![],
    };
    let mut format_ids = BTreeMap::new();
    let document = session.document()?;
    for (block, target) in base.blocks.iter().zip(targets) {
        if cancel.load(Ordering::Relaxed) {
            return Err("翻譯讀取已取消。".into());
        }
        if block.kind != "text" || block.text.trim().is_empty() {
            result.blocks.push(Block {
                format: Value::Null,
                ..block.clone()
            });
            continue;
        }
        let start = integer(&target, "Start")?;
        let end = integer(&target, "End")?;
        let format = signature(&target)?;
        let uniform = format
            .as_object()
            .is_some_and(|v| v.values().all(|v| !v.is_null()));
        let mut spans: Vec<(i32, i32, Value)> = vec![];
        if uniform {
            spans.push((start, end, format));
        } else {
            let characters = child(&target, "Characters")?;
            let count = integer(&characters, "Count")?;
            if count > 32_000 {
                return Err("混合格式段落超過32,000字，請先縮小文件。".into());
            }
            for i in 1..=count {
                if cancel.load(Ordering::Relaxed) {
                    return Err("格式讀取已取消。".into());
                }
                let character = item(&characters, i)?;
                let a = integer(&character, "Start")?;
                let b = integer(&character, "End")?.min(end);
                if a >= b {
                    continue;
                }
                let format = signature(&character)?;
                if let Some(last) = spans.last_mut().filter(|s| s.1 == a && s.2 == format) {
                    last.1 = b;
                } else {
                    spans.push((a, b, format));
                }
            }
        }
        for (a, b, format) in spans {
            if result.blocks.len() >= MAX_BLOCKS {
                return Err("翻譯格式區段超過2000，請拆分文件。".into());
            }
            let text = string(&get(&range(document, a, b)?, "Text")?)?;
            let key = format.to_string();
            let index = *format_ids.entry(key).or_insert_with(|| {
                let n = result.formats.len();
                result.formats.push(format);
                n
            });
            result.blocks.push(Block {
                id: format!("{}@{a}:{b}", block.id),
                label: block.label.clone(),
                text,
                kind: "text".into(),
                format: json!(index),
            });
        }
    }
    result.serialize()?;
    Ok(result)
}

fn check_source(source: &Path) -> AppResult<()> {
    if source
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("docx"))
    {
        Ok(())
    } else {
        Err("保留樣式的翻譯目前限DOCX；DOC／DOCM請先另存DOCX，或使用新文件翻譯。".into())
    }
}

pub fn inspect(source: &Path, cancel: &AtomicBool) -> AppResult<Snapshot> {
    check_source(source)?;
    // SAFETY: 本次背景執行緒獨立初始化STA，所有COM物件在Apartment之前釋放。
    let _apartment = Apartment::sta().map_err(|e| e.to_string())?;
    // 明確綁定Session，確保它先於Apartment釋放；不能把COM暫存物件放在尾端運算式。
    let session = Session::open(source)?;
    snapshot(&session, cancel)
}

/// 來源重新核對後，由後向前替換範圍；輸出只能是呼叫端準備的新檔案位置。
pub(super) fn apply(
    session: &Session,
    expected: &Snapshot,
    items: &[Translated],
    cancel: &AtomicBool,
) -> AppResult<()> {
    if snapshot(session, cancel)? != *expected {
        return Err("Word來源或格式已改變，未套用舊譯文。".into());
    }
    let mut replacements = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for translated in items {
        let block = expected
            .blocks
            .iter()
            .find(|b| b.id == translated.id && b.kind == "text" && b.text == translated.source)
            .ok_or("翻譯區段與原文不相符。")?;
        if !seen.insert(&translated.id)
            || translated.translation.contains(['\r', '\n'])
            || translated.translation.trim().is_empty()
        {
            return Err("翻譯區段重複、空白或改變段落結構。".into());
        }
        super::super::text::validate(&translated.translation)?;
        let (a, b) = block
            .id
            .rsplit_once('@')
            .and_then(|(_, v)| v.split_once(':'))
            .ok_or("格式區段座標錯誤。")?;
        let start = a.parse::<i32>().map_err(|_| "區段起點錯誤")?;
        let end = b.parse::<i32>().map_err(|_| "區段終點錯誤")?;
        replacements.push((start, end, translated, block));
    }
    if expected
        .blocks
        .iter()
        .filter(|b| b.kind == "text" && !b.text.trim().is_empty())
        .any(|b| !seen.contains(&b.id))
    {
        return Err("仍有未翻譯區段，不能發布。".into());
    }
    replacements.sort_by_key(|r| std::cmp::Reverse(r.0));
    for (start, end, translated, block) in replacements {
        if cancel.load(Ordering::Relaxed) {
            return Err("翻譯已取消，未發布。".into());
        }
        let target = range(session.document()?, start, end)?;
        let font = child(&child(&target, "Font")?, "Duplicate")?;
        set(&target, "Text", translated.translation.as_str().into())?;
        let replacement = range(
            session.document()?,
            start,
            start + translated.translation.encode_utf16().count() as i32,
        )?;
        // 不重設段落樣式：那會連帶清除同段其他區段的直接格式。
        // 段落末標記未被替換，原樣式應自然保留，再由signature核對。
        set(&replacement, "Font", VARIANT::from(font))?;
        let expected_format = block
            .format
            .as_u64()
            .and_then(|n| expected.formats.get(n as usize))
            .ok_or("原格式遺失。")?;
        let text_matches = string(&get(&replacement, "Text")?)? == translated.translation;
        let actual_format = signature(&replacement)?;
        if !text_matches || actual_format != *expected_format {
            let changed: Vec<_> = expected_format
                .as_object()
                .into_iter()
                .flat_map(|v| v.iter())
                .filter(|(key, value)| actual_format[*key] != **value)
                .map(|(key, _)| key.as_str())
                .collect();
            return Err(format!(
                "Word區段{}譯文／格式讀回不符（文字相符：{text_matches}；格式欄位：{}），未發布。",
                block.id,
                changed.join("、")
            ));
        }
    }
    Ok(())
}
