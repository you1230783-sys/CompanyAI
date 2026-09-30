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
    Ok(())
}
