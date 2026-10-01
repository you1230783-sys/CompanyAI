//! PNG 嵌入固定操作；圖片路徑由 broker 鎖定及核對，COM 不接受網址或任意方法。
use super::*;
use serde_json::{json, Value};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Placement {
    Word { before: Option<String> },
    Excel { sheet: i32, cell: String },
    Ppt { slide: i32, left: f64, top: f64 },
}

fn number(object: &IDispatch, property: &str) -> AppResult<f64> {
    f64::try_from(&get(object, property)?).map_err(|e| e.to_string())
}

pub(super) fn insert(
    session: &Session,
    path: &Path,
    target: &Placement,
    width: f64,
    hash: &str,
    cancel: &AtomicBool,
) -> AppResult<()> {
    if !width.is_finite() || !(12.0..=1200.0).contains(&width) {
        return Err("圖片寬度需為 12–1200 點（72 點＝1 英吋）。".into());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let (pixels_x, pixels_y) = super::super::charts::png::dimensions(&bytes)?;
    let height = width * f64::from(pixels_y) / f64::from(pixels_x);
    if height > 1200.0 {
        return Err("等比例圖片高度超過 1200 點，請縮小寬度。".into());
    }
    let doc = session.document()?;
    let picture = match target {
        Placement::Word { before } if session.collection == "Documents" => {
            let range = session.word_insertion(before, cancel)?;
            let page = child(&range, "PageSetup")?;
            let available_width = number(&page, "PageWidth")?
                - number(&page, "LeftMargin")?
                - number(&page, "RightMargin")?;
            let available_height = number(&page, "PageHeight")?
                - number(&page, "TopMargin")?
                - number(&page, "BottomMargin")?;
            if width > available_width || height > available_height {
                return Err("圖片超過 Word 頁面可用範圍，請縮小寬度。".into());
            }
            let start = integer(&range, "Start")?;
            // 使用独立段落中的行內圖片，避免覆蓋既有文字或加入不穩定的浮動錨點。
            set(&range, "Text", "\r".into())?;
            invoke(&range, "SetRange", vec![start.into(), start.into()], false)?;
            let picture = obj(invoke(
                &child(doc, "InlineShapes")?,
                "AddPicture",
                vec![
                    path.to_string_lossy().as_ref().into(),
                    false.into(),
                    true.into(),
                    range.into(),
                ],
                false,
            )?)?;
            set(&picture, "LockAspectRatio", 0i32.into())?;
            set(&picture, "Width", width.into())?;
            set(&picture, "Height", height.into())?;
            picture
        }
        Placement::Excel { sheet, cell } if session.collection == "Workbooks" => {
            authoring::cell(cell)?;
            let sheets = child(doc, "Worksheets")?;
            if *sheet < 1 || *sheet > integer(&sheets, "Count")? {
                return Err("圖片目標工作表不存在。".into());
            }
            let sheet = item(&sheets, *sheet)?;
            let anchor = obj(invoke(&sheet, "Range", vec![cell.as_str().into()], false)?)?;
            let picture = add_picture(
                &child(&sheet, "Shapes")?,
                path,
                number(&anchor, "Left")?,
                number(&anchor, "Top")?,
                width,
                height,
            )?;
            // 圖片隨儲存格移動，但不因列高欄寬改變而被壓縮。
            set(&picture, "Placement", 2i32.into())?;
            picture
        }
        Placement::Ppt { slide, left, top } if session.collection == "Presentations" => {
            let slides = child(doc, "Slides")?;
            if *slide < 1 || *slide > integer(&slides, "Count")? {
                return Err("圖片目標投影片不存在；請先建立投影片。".into());
            }
            let page = child(doc, "PageSetup")?;
            if !left.is_finite()
                || !top.is_finite()
                || *left < 0.0
                || *top < 0.0
                || left + width > number(&page, "SlideWidth")?
                || top + height > number(&page, "SlideHeight")?
            {
                return Err("圖片超出投影片邊界，請調整位置或縮小寬度。".into());
            }
            add_picture(
                &child(&item(&slides, *slide)?, "Shapes")?,
                path,
                *left,
                *top,
                width,
                height,
            )?
        }
        _ => return Err("圖片位置種類與 Office 文件不符。".into()),
    };
    set(&picture, "LockAspectRatio", (-1i32).into())?;
    set(
        &picture,
        "AlternativeText",
        format!("LM_AI image SHA256:{hash}").as_str().into(),
    )?;
    if (number(&picture, "Width")? - width).abs() > 0.1
        || (number(&picture, "Height")? - height).abs() > 0.1
    {
        return Err("Office 圖片尺寸讀回不符，未接受本次修改。".into());
    }
    Ok(())
}

fn add_picture(
    shapes: &IDispatch,
    path: &Path,
    left: f64,
    top: f64,
    width: f64,
    height: f64,
) -> AppResult<IDispatch> {
    obj(invoke(
        shapes,
        "AddPicture",
        vec![
            path.to_string_lossy().as_ref().into(),
            0i32.into(),
            (-1i32).into(),
            left.into(),
            top.into(),
            width.into(),
            height.into(),
        ],
        false,
    )?)
}

/// 將本工具嵌入的圖像納入版本與存檔讀回核對，避免「文字没變所以圖片修改没發生」。
pub(super) fn inventory(session: &Session) -> AppResult<Vec<Value>> {
    let doc = session.document()?;
    let mut result = Vec::new();
    let mut collections = Vec::new();
    if session.collection == "Documents" {
        collections.push((0, child(doc, "InlineShapes")?));
    } else {
        let pages = child(
            doc,
            if session.collection == "Workbooks" {
                "Worksheets"
            } else {
                "Slides"
            },
        )?;
        for index in 1..=integer(&pages, "Count")? {
            collections.push((index, child(&item(&pages, index)?, "Shapes")?));
        }
    }
    let mut scanned = 0;
    for (page, shapes) in collections {
        for index in 1..=integer(&shapes, "Count")? {
            scanned += 1;
            if scanned > 2000 {
                return Err("Office 圖形超過 2000 個。".into());
            }
            let shape = item(&shapes, index)?;
            let tag = string(&get(&shape, "AlternativeText")?)?;
            if !tag.starts_with("LM_AI image SHA256:") {
                continue;
            }
            let mut image = json!({"page":page,"index":index,"tag":tag});
            for property in ["Width", "Height"] {
                image[property] = json!((number(&shape, property)? * 1000.0).round() / 1000.0);
            }
            if session.collection == "Documents" {
                image["start"] = json!(integer(&child(&shape, "Range")?, "Start")?);
            } else {
                for property in ["Left", "Top"] {
                    image[property] = json!((number(&shape, property)? * 1000.0).round() / 1000.0);
                }
            }
            result.push(image);
        }
    }
    Ok(result)
}
