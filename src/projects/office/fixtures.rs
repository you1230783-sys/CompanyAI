//! 僅 debug 驗收程式使用的固定資料建立器；不提供給模型，release 不包含此入口。
use super::*;

pub fn create(folder: &Path) -> AppResult<()> {
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() }.map_err(|e| e.to_string())?;
    let _apartment = Apartment;
    for (kind, formats) in [
        ("docx", vec!["docx", "doc", "docm"]),
        ("xlsx", vec!["xlsx", "xls", "xlsm", "xlsb"]),
        ("pptx", vec!["pptx", "ppt", "pptm"]),
    ] {
        let session = Session::start(Path::new(&format!("source.{kind}")), true)?;
        let doc = session.document()?;
        match kind {
            "docx" => {
                set(&child(doc, "Content")?, "Text", "原始文字\r第二段".into())?;
                let range = child(&item(&child(doc, "Paragraphs")?, 1)?, "Range")?;
                set(&child(&range, "Font")?, "Bold", (-1i32).into())?;
            }
            "xlsx" => {
                let sheet = item(&child(doc, "Worksheets")?, 1)?;
                for (address, property, value) in [
                    ("A1", "Value2", "原始文字".into()),
                    ("B1", "Value2", 12f64.into()),
                    ("C1", "Formula", "=B1*2".into()),
                ] {
                    let cell = obj(invoke(&sheet, "Range", vec![address.into()], false)?)?;
                    set(&cell, property, value)?;
                    if address == "A1" {
                        set(&child(&cell, "Font")?, "Bold", true.into())?;
                    }
                }
            }
            _ => {
                let slide = obj(invoke(
                    &child(doc, "Slides")?,
                    "Add",
                    vec![1i32.into(), 1i32.into()],
                    false,
                )?)?;
                let shapes = child(&slide, "Shapes")?;
                for (index, text) in [(1, "原始文字"), (2, "第二區塊")] {
                    set(
                        &child(&child(&item(&shapes, index)?, "TextFrame")?, "TextRange")?,
                        "Text",
                        text.into(),
                    )?;
                }
            }
        }
        for ext in formats {
            let path = folder.join(format!("source.{ext}"));
            invoke(
                doc,
                if kind == "docx" { "SaveAs2" } else { "SaveAs" },
                vec![
                    path.to_string_lossy().as_ref().into(),
                    save_format(ext)?.into(),
                ],
                false,
            )?;
        }
        if kind == "docx" {
            invoke(
                doc,
                "ExportAsFixedFormat",
                vec![
                    folder.join("source.pdf").to_string_lossy().as_ref().into(),
                    17i32.into(),
                ],
                false,
            )?;
        }
    }
    create_large_excel(folder)?;
    Ok(())
}

/// 大表使用整段固定公式建立數值，避免測試資料建立器本身逐格呼叫 COM。
/// 中間欄含未選文字、F3 留白、H2 為錯誤；用來核對選欄與缺值不位移。
fn create_large_excel(folder: &Path) -> AppResult<()> {
    let session = Session::start(Path::new("large.xlsx"), true)?;
    let doc = session.document()?;
    let sheets = child(doc, "Worksheets")?;
    let sheet = item(&sheets, 1)?;
    set(&sheet, "Name", "大量資料".into())?;
    for (address, property, value) in [
        ("A1", "Value2", "序號".into()),
        ("F1", "Value2", "量測".into()),
        ("A2:A10001", "Formula", "=ROW()-1".into()),
        ("F2:F10001", "Formula", "=(ROW()-1)*2".into()),
        ("B2:E10001", "Value2", "不應讀到的中間欄".into()),
        ("H2", "Formula", "=1/0".into()),
        ("H3", "NumberFormat", "@".into()),
        ("H3", "Value2", "339".into()),
        ("H4", "Value2", "NG".into()),
        ("H6", "Value2", "12 mm".into()),
    ] {
        let range = obj(invoke(&sheet, "Range", vec![address.into()], false)?)?;
        set(&range, property, value)?;
    }
    let blank = obj(invoke(&sheet, "Range", vec!["F3".into()], false)?)?;
    invoke(&blank, "ClearContents", vec![], false)?;
    let merged = obj(invoke(&sheet, "Range", vec!["H7:H8".into()], false)?)?;
    invoke(&merged, "Merge", vec![], false)?;
    // 新工作表位於前面，刻意讓測試先依工作表名稱找到序號。
    let offset = obj(invoke(&sheets, "Add", vec![], false)?)?;
    set(&offset, "Name", "偏移表頭".into())?;
    for (address, value) in [("D5", "標題"), ("F1105", "末端")] {
        let range = obj(invoke(&offset, "Range", vec![address.into()], false)?)?;
        set(&range, "Value2", value.into())?;
    }
    for ext in ["xlsx", "xls"] {
        invoke(
            doc,
            "SaveAs",
            vec![
                folder
                    .join(format!("large.{ext}"))
                    .to_string_lossy()
                    .as_ref()
                    .into(),
                save_format(ext)?.into(),
            ],
            false,
        )?;
    }
    Ok(())
}
