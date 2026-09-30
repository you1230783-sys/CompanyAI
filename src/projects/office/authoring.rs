//! 有限的 Office 結構與格式操作。命令先在未儲存的文件執行並讀回，成功才接受新版本。
//! 保存的是固定操作清單，不保存 COM 物件；重建只修改新開的唯讀來源／空白文件。
use super::*;
use serde_json::{json, Value};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    /// 此項由既有 edit_office 入口轉入，沿用原文核對。
    Edit {
        block_id: String,
        expected: String,
        replacement: String,
    },
    Format {
        target: String,
        format: Format,
    },
    /// before 為正文段落 ID；省略表示文件結尾。第一版不在表格內插入結構。
    WordParagraph {
        before: Option<String>,
        text: String,
        #[serde(default)]
        format: Format,
    },
    WordTable {
        before: Option<String>,
        rows: Vec<Vec<String>>,
    },
    ExcelSheet {
        name: String,
    },
    /// sheet 是工作表的 1-based 編號；cell 只接受 A1 座標，不接受公式或命名範圍。
    ExcelWrite {
        sheet: i32,
        cell: String,
        rows: Vec<Vec<Value>>,
    },
    PptSlide {
        layout: String,
        title: String,
        body: String,
        #[serde(default)]
        right: String,
    },
}

/// 明確列出可設定的屬性。None 表示保留原格式；不提供任意屬性名稱或 COM 路徑。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Format {
    pub style: Option<String>,
    pub font: Option<String>,
    pub font_east_asia: Option<String>,
    pub size: Option<f64>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub color: Option<String>,
    pub alignment: Option<String>,
    pub space_before: Option<f64>,
    pub space_after: Option<f64>,
    /// 倍數行距；Word／PPT 適用。
    pub line_spacing: Option<f64>,
    pub bullets: Option<bool>,
    pub fill: Option<String>,
    pub borders: Option<bool>,
    pub wrap: Option<bool>,
    pub column_width: Option<f64>,
    /// 預設格式名稱：general、integer、decimal、percent、date、text。
    pub number_format: Option<String>,
}

fn color(value: &str) -> AppResult<i32> {
    let hex = value.strip_prefix('#').ok_or("顏色需為 #RRGGBB。")?;
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("顏色需為 #RRGGBB。".into());
    }
    let rgb = u32::from_str_radix(hex, 16).map_err(|e| e.to_string())?;
    Ok(((rgb & 255) << 16 | rgb & 0xff00 | rgb >> 16) as i32)
}

impl Format {
    fn validate(&self, family: &str) -> AppResult<()> {
        for (name, value, min, max) in [
            ("字級", self.size, 1., 200.),
            ("段前", self.space_before, 0., 200.),
            ("段後", self.space_after, 0., 200.),
            ("行距", self.line_spacing, 0.5, 5.),
            ("欄寬", self.column_width, 1., 100.),
        ] {
            if value.is_some_and(|v| !v.is_finite() || v < min || v > max) {
                return Err(format!("{name}超出允許範圍 {min}–{max}。"));
            }
        }
        if self
            .font
            .iter()
            .chain(self.font_east_asia.iter())
            .any(|s| s.trim().is_empty() || s.len() > 128 || s.chars().any(char::is_control))
        {
            return Err("字型名稱不正確。".into());
        }
        if family != "Documents" && self.font_east_asia.is_some() {
            return Err("東亞字型欄位目前只適用 Word。".into());
        }
        for value in [&self.color, &self.fill].into_iter().flatten() {
            color(value)?;
        }
        if self
            .alignment
            .as_deref()
            .is_some_and(|s| !matches!(s, "left" | "center" | "right" | "justify"))
        {
            return Err("對齊只接受 left／center／right／justify。".into());
        }
        if self.style.as_deref().is_some_and(|s| {
            !matches!(
                (family, s),
                ("Documents", "title" | "heading1" | "heading2" | "body")
                    | ("Workbooks", "header")
                    | ("Presentations", "title" | "body")
            )
        }) {
            return Err("此文件不支援指定的預設樣式。".into());
        }
        if family != "Workbooks"
            && (self.fill.is_some()
                || self.borders.is_some()
                || self.wrap.is_some()
                || self.column_width.is_some()
                || self.number_format.is_some())
        {
            return Err("底色、框線、換行、欄寬與數字格式只適用 Excel。".into());
        }
        if family == "Workbooks"
            && (self.space_before.is_some()
                || self.space_after.is_some()
                || self.line_spacing.is_some()
                || self.bullets.is_some())
        {
            return Err("段落間距與項目符號不適用 Excel。".into());
        }
        if self.number_format.as_deref().is_some_and(|s| {
            !matches!(
                s,
                "general" | "integer" | "decimal" | "percent" | "date" | "text"
            )
        }) {
            return Err("未知數字格式。".into());
        }
        Ok(())
    }
}

/// 限定 A1 單格／矩形，不允許外部活頁簿、命名範圍、整欄或聯集。
fn cell(value: &str) -> AppResult<(i32, i32)> {
    let value = value.replace('$', "").to_ascii_uppercase();
    let split = value
        .find(|c: char| c.is_ascii_digit())
        .ok_or("需填 A1 儲存格座標。")?;
    let (letters, digits) = value.split_at(split);
    if letters.is_empty()
        || letters.len() > 3
        || !letters.bytes().all(|c| c.is_ascii_uppercase())
        || !digits.bytes().all(|c| c.is_ascii_digit())
    {
        return Err("無效的 A1 座標。".into());
    }
    let col = letters
        .bytes()
        .fold(0i32, |n, c| n * 26 + i32::from(c - b'A' + 1));
    let row: i32 = digits.parse().map_err(|_| "無效列號。")?;
    // 初版範圍明確，避免一個格式要求令 UsedRange 延伸到整張試算表。
    if !(1..=1000).contains(&row) || !(1..=100).contains(&col) {
        return Err("初版操作範圍為前 1000 列、100 欄。".into());
    }
    Ok((row, col))
}

fn excel_range(session: &Session, target: &str) -> AppResult<IDispatch> {
    let (sheet, area) = target
        .split_once(':')
        .ok_or("Excel 目標格式為 s1:A1:C3。")?;
    let index: i32 = sheet
        .strip_prefix('s')
        .ok_or("工作表需使用 s1 等編號。")?
        .parse()
        .map_err(|_| "工作表編號無效。")?;
    let (first, last) = area.split_once(':').unwrap_or((area, area));
    let (r1, c1) = cell(first)?;
    let (r2, c2) = cell(last)?;
    if r2 < r1 || c2 < c1 || (r2 - r1 + 1) * (c2 - c1 + 1) > 2000 {
        return Err("範圍順序錯誤或超過 2000 格。".into());
    }
    let sheets = child(session.document()?, "Worksheets")?;
    if index < 1 || index > integer(&sheets, "Count")? {
        return Err("工作表不存在。".into());
    }
    obj(invoke(
        &item(&sheets, index)?,
        "Range",
        vec![area.into()],
        false,
    )?)
}

fn apply_format(target: &IDispatch, family: &str, format: &Format) -> AppResult<()> {
    format.validate(family)?;
    let font = child(target, "Font")?;
    if let Some(style) = &format.style {
        match family {
            "Documents" => set(
                target,
                "Style",
                match style.as_str() {
                    "title" => -63i32,
                    "heading1" => -2,
                    "heading2" => -3,
                    _ => -1,
                }
                .into(),
            )?,
            "Workbooks" => {
                set(&font, "Bold", true.into())?;
                set(
                    &child(target, "Interior")?,
                    "Color",
                    color("#E8EEF7")?.into(),
                )?;
            }
            _ => {
                set(
                    &font,
                    "Size",
                    if style == "title" { 32f64 } else { 20f64 }.into(),
                )?;
            }
        }
    }
    if let Some(value) = &format.font {
        set(&font, "Name", value.as_str().into())?;
    }
    if let Some(value) = &format.font_east_asia {
        if family != "Documents" {
            return Err("東亞字型欄位目前只適用 Word。".into());
        }
        set(&font, "NameFarEast", value.as_str().into())?;
    }
    if let Some(value) = format.size {
        set(&font, "Size", value.into())?;
    }
    for (name, value) in [("Bold", format.bold), ("Italic", format.italic)] {
        if let Some(value) = value {
            set(
                &font,
                name,
                if family == "Workbooks" {
                    value.into()
                } else {
                    if value { -1i32 } else { 0i32 }.into()
                },
            )?;
        }
    }
    if let Some(value) = &format.color {
        if family == "Presentations" {
            set(&child(&font, "Color")?, "RGB", color(value)?.into())?;
        } else {
            set(&font, "Color", color(value)?.into())?;
        }
    }
    if family == "Workbooks" {
        if let Some(value) = &format.alignment {
            set(
                target,
                "HorizontalAlignment",
                match value.as_str() {
                    "center" => -4108i32,
                    "right" => -4152,
                    "justify" => -4130,
                    _ => -4131,
                }
                .into(),
            )?;
        }
        if let Some(value) = &format.fill {
            set(&child(target, "Interior")?, "Color", color(value)?.into())?;
        }
        if let Some(value) = format.borders {
            // 外框與內部格線；不一併畫上對角線（索引 5、6）。
            let borders = child(target, "Borders")?;
            for edge in 7..=12 {
                set(
                    &item(&borders, edge)?,
                    "LineStyle",
                    if value { 1i32 } else { -4142i32 }.into(),
                )?;
            }
        }
        if let Some(value) = format.wrap {
            set(target, "WrapText", value.into())?;
        }
        if let Some(value) = format.column_width {
            set(target, "ColumnWidth", value.into())?;
        }
        if let Some(value) = &format.number_format {
            set(
                target,
                "NumberFormat",
                match value.as_str() {
                    "integer" => "0",
                    "decimal" => "0.00",
                    "percent" => "0.00%",
                    "date" => "yyyy-mm-dd",
                    "text" => "@",
                    _ => "General",
                }
                .into(),
            )?;
        }
    } else {
        let paragraph = child(target, "ParagraphFormat")?;
        if let Some(value) = &format.alignment {
            let n = match value.as_str() {
                "center" => 1i32,
                "right" => 2,
                "justify" => 3,
                _ => 0,
            } + i32::from(family == "Presentations");
            set(&paragraph, "Alignment", n.into())?;
        }
        for (name, value) in [
            ("SpaceBefore", format.space_before),
            ("SpaceAfter", format.space_after),
        ] {
            if let Some(value) = value {
                set(&paragraph, name, value.into())?;
            }
        }
        if let Some(value) = format.line_spacing {
            if family == "Documents" {
                set(&paragraph, "LineSpacingRule", 5i32.into())?;
                set(&paragraph, "LineSpacing", (value * 12.).into())?;
            } else {
                set(&paragraph, "LineRuleWithin", (-1i32).into())?;
                set(&paragraph, "SpaceWithin", value.into())?;
            }
        }
        if let Some(value) = format.bullets {
            if family == "Documents" {
                invoke(
                    &child(target, "ListFormat")?,
                    if value {
                        "ApplyBulletDefault"
                    } else {
                        "RemoveNumbers"
                    },
                    vec![],
                    false,
                )?;
            } else {
                set(
                    &child(&paragraph, "Bullet")?,
                    "Visible",
                    if value { -1i32 } else { 0i32 }.into(),
                )?;
            }
        }
    }
    Ok(())
}

/// 可重讀的格式快照；Office 的混合值／Null 保留為 null，不臆測。
fn scalar(object: &IDispatch, property: &str) -> AppResult<Value> {
    let v = get(object, property)?;
    let vt = unsafe { v.Anonymous.Anonymous.vt };
    Ok(match vt {
        VT_NULL | VT_EMPTY => Value::Null,
        VT_BSTR => json!(string(&v)?),
        VT_BOOL => json!(bool::try_from(&v).map_err(|e| e.to_string())?),
        _ => {
            let n = f64::try_from(&v).map_err(|e| e.to_string())?;
            if n == 9999999. || n == -2. {
                Value::Null
            } else {
                // Office 的 Single 轉 Double 後帶二進位尾差；正規化至千分之一點後再比較。
                serde_json::from_str(&format!("{n:.3}")).map_err(|e| e.to_string())?
            }
        }
    })
}

pub(super) fn inspect_format(target: &IDispatch, family: &str) -> AppResult<Value> {
    let font = child(target, "Font")?;
    let mut value = json!({});
    for name in ["Name", "Size", "Bold", "Italic"] {
        value[name] = scalar(&font, name)?;
    }
    value["Color"] = if family == "Presentations" {
        scalar(&child(&font, "Color")?, "RGB")?
    } else {
        scalar(&font, "Color")?
    };
    if family == "Workbooks" {
        for name in [
            "NumberFormat",
            "HorizontalAlignment",
            "WrapText",
            "ColumnWidth",
        ] {
            value[name] = scalar(target, name)?;
        }
        value["Fill"] = scalar(&child(target, "Interior")?, "Color")?;
        let borders = child(target, "Borders")?;
        value["Borders"] = Value::Array(
            (7..=10)
                .map(|i| scalar(&item(&borders, i)?, "LineStyle"))
                .collect::<AppResult<Vec<_>>>()?,
        );
    } else {
        let paragraph = child(target, "ParagraphFormat")?;
        for name in ["Alignment", "SpaceBefore", "SpaceAfter"] {
            value[name] = scalar(&paragraph, name)?;
        }
        if family == "Documents" {
            for name in ["LineSpacingRule", "LineSpacing"] {
                value[name] = scalar(&paragraph, name)?;
            }
            value["NameFarEast"] = scalar(&font, "NameFarEast")?;
            value["ListType"] = scalar(&child(target, "ListFormat")?, "ListType")?;
        } else {
            for name in ["LineRuleWithin", "SpaceWithin"] {
                value[name] = scalar(&paragraph, name)?;
            }
            value["Bullet"] = scalar(&child(&paragraph, "Bullet")?, "Visible")?;
        }
    }
    Ok(value)
}

pub(super) fn structure(session: &Session) -> AppResult<Value> {
    let doc = session.document()?;
    let (collection, property) = match session.collection {
        "Documents" => ("Tables", "Rows"),
        "Workbooks" => ("Worksheets", "Name"),
        _ => ("Slides", "Layout"),
    };
    let items = child(doc, collection)?;
    let count = integer(&items, "Count")?;
    if count > 2000 {
        return Err("Office 結構超過上限。".into());
    }
    let mut result = Vec::new();
    for i in 1..=count {
        let object = item(&items, i)?;
        result.push(if session.collection == "Documents" {
            // 合併表格可能不提供 Rows/Columns，保留文字支援，標示無法取得維度。
            json!({"rows":child(&object, property).and_then(|r| integer(&r,"Count")).ok(),"columns":child(&object,"Columns").and_then(|r| integer(&r,"Count")).ok()})
        } else { scalar(&object, property)? });
    }
    Ok(json!({collection:result}))
}

fn rectangular<T>(rows: &[Vec<T>]) -> AppResult<usize> {
    let width = rows.first().map_or(0, Vec::len);
    if width == 0
        || rows.len() > 1000
        || width > 100
        || rows.len() * width > 2000
        || rows.iter().any(|r| r.len() != width)
    {
        return Err("資料需為等寬矩形，最多 2000 格。".into());
    }
    Ok(width)
}

impl Session {
    fn word_insertion(&self, before: &Option<String>, cancel: &AtomicBool) -> AppResult<IDispatch> {
        let doc = self.document()?;
        let position = if let Some(id) = before {
            let (snapshot, targets) = self.snapshot(cancel)?;
            let index = snapshot
                .blocks
                .iter()
                .position(|b| &b.id == id)
                .ok_or("插入位置不存在。")?;
            let target = &targets[index];
            if bool::try_from(&invoke(target, "Information", vec![12i32.into()], false)?)
                .map_err(|e| e.to_string())?
            {
                return Err("初版只在正文段落前插入，不在表格內新增結構。".into());
            }
            integer(target, "Start")?
        } else {
            integer(&child(doc, "Content")?, "End")? - 1
        };
        obj(invoke(
            doc,
            "Range",
            vec![position.into(), position.into()],
            false,
        )?)
    }

    fn apply(&self, action: &Action, cancel: &AtomicBool) -> AppResult<()> {
        if cancel.load(Ordering::Relaxed) {
            return Err("Office 操作已取消。".into());
        }
        let doc = self.document()?;
        match action {
            Action::Edit {
                block_id,
                expected,
                replacement,
            } => {
                let (mut snapshot, targets) = self.snapshot(cancel)?;
                snapshot.edit(block_id, expected, replacement)?;
                let index = snapshot
                    .blocks
                    .iter()
                    .position(|b| &b.id == block_id)
                    .ok_or("區塊不存在。")?;
                let block = &snapshot.blocks[index];
                let target = &targets[index];
                if self.collection == "Workbooks" {
                    if block.kind == "number" {
                        set(
                            target,
                            "Value2",
                            block.text.parse::<f64>().map_err(|e| e.to_string())?.into(),
                        )?;
                    } else {
                        set(target, "Value2", format!("'{}", block.text).as_str().into())?;
                    }
                } else {
                    // 只取代變動區間，保留前後未變文字各自的格式（索引以 UTF-16 計）。
                    let (start, old_len, text) = difference(expected, &block.text);
                    if old_len > 0 || !text.is_empty() {
                        let changed = if self.collection == "Documents" {
                            let range = child(target, "Duplicate")?;
                            let offset = integer(target, "Start")?;
                            invoke(
                                &range,
                                "SetRange",
                                vec![(offset + start).into(), (offset + start + old_len).into()],
                                false,
                            )?;
                            range
                        } else {
                            obj(invoke(
                                target,
                                "Characters",
                                vec![(start + 1).into(), old_len.into()],
                                false,
                            )?)?
                        };
                        set(&changed, "Text", text.replace('\n', "\r").as_str().into())?;
                    }
                }
                // 文字修改仍須逐區塊核對，不因格式快照擴充而放寬。
                let actual = self.snapshot(cancel)?.0;
                if actual
                    .blocks
                    .iter()
                    .map(|b| (&b.id, &b.text))
                    .ne(snapshot.blocks.iter().map(|b| (&b.id, &b.text)))
                {
                    return Err("Office 文字修改結果不一致。".into());
                }
            }
            Action::Format { target, format } => {
                let object = if self.collection == "Workbooks" {
                    excel_range(self, target)?
                } else {
                    let (snapshot, targets) = self.snapshot(cancel)?;
                    let index = snapshot
                        .blocks
                        .iter()
                        .position(|b| &b.id == target)
                        .ok_or("格式目標不存在。")?;
                    if snapshot.blocks[index].kind == "readonly" {
                        return Err("此特殊區塊不可修改格式。".into());
                    }
                    targets[index].clone()
                };
                apply_format(&object, self.collection, format)?;
            }
            Action::WordParagraph {
                before,
                text,
                format,
            } if self.collection == "Documents" => {
                plain(text)?;
                format.validate(self.collection)?;
                let range = self.word_insertion(before, cancel)?;
                let start = integer(&range, "Start")?;
                set(&range, "Text", format!("{text}\r").as_str().into())?;
                let range = obj(invoke(
                    doc,
                    "Range",
                    vec![
                        start.into(),
                        (start + text.encode_utf16().count() as i32).into(),
                    ],
                    false,
                )?)?;
                // 新段落不沿用前一段的標題／條列設定；既有段落的格式工具仍只改指定屬性。
                apply_format(
                    &range,
                    self.collection,
                    &Format {
                        style: Some("body".into()),
                        bullets: Some(false),
                        ..Format::default()
                    },
                )?;
                apply_format(&range, self.collection, format)?;
            }
            Action::WordTable { before, rows } if self.collection == "Documents" => {
                let width = rectangular(rows)?;
                for text in rows.iter().flatten() {
                    plain(text)?;
                }
                let range = self.word_insertion(before, cancel)?;
                let table = obj(invoke(
                    &child(doc, "Tables")?,
                    "Add",
                    vec![
                        range.into(),
                        (rows.len() as i32).into(),
                        (width as i32).into(),
                    ],
                    false,
                )?)?;
                apply_format(
                    &child(&table, "Range")?,
                    self.collection,
                    &Format {
                        style: Some("body".into()),
                        bullets: Some(false),
                        ..Format::default()
                    },
                )?;
                for (r, row) in rows.iter().enumerate() {
                    for (c, text) in row.iter().enumerate() {
                        let cell = obj(invoke(
                            &table,
                            "Cell",
                            vec![(r as i32 + 1).into(), (c as i32 + 1).into()],
                            false,
                        )?)?;
                        set(&child(&cell, "Range")?, "Text", text.as_str().into())?;
                    }
                }
                set(&child(&table, "Borders")?, "Enable", (-1i32).into())?;
            }
            Action::ExcelSheet { name } if self.collection == "Workbooks" => {
                if name.trim().is_empty()
                    || name.chars().count() > 31
                    || name.contains(['[', ']', ':', '*', '?', '/', '\\', '\''])
                    || name.chars().any(char::is_control)
                {
                    return Err("工作表名稱不合法。".into());
                }
                let sheets = child(doc, "Worksheets")?;
                let count = integer(&sheets, "Count")?;
                if count >= 50 {
                    return Err("最多 50 張工作表。".into());
                }
                let sheet = obj(invoke(
                    &sheets,
                    "Add",
                    vec![missing(), item(&sheets, count)?.into()],
                    false,
                )?)?;
                set(&sheet, "Name", name.as_str().into())?;
            }
            Action::ExcelWrite {
                sheet,
                cell: address,
                rows,
            } if self.collection == "Workbooks" => {
                let width = rectangular(rows)?;
                let (row, col) = cell(address)?;
                if row + rows.len() as i32 - 1 > 1000 || col + width as i32 - 1 > 100 {
                    return Err("寫入範圍超出上限。".into());
                }
                let sheets = child(doc, "Worksheets")?;
                if *sheet < 1 || *sheet > integer(&sheets, "Count")? {
                    return Err("工作表不存在。".into());
                }
                let cells = child(&item(&sheets, *sheet)?, "Cells")?;
                for (r, values) in rows.iter().enumerate() {
                    for (c, value) in values.iter().enumerate() {
                        let target = obj(invoke(
                            &cells,
                            "Item",
                            vec![(row + r as i32).into(), (col + c as i32).into()],
                            false,
                        )?)?;
                        if bool::try_from(&get(&target, "HasFormula")?).unwrap_or(true)
                            || bool::try_from(&get(&target, "MergeCells")?).unwrap_or(true)
                        {
                            return Err("不可覆寫公式或合併儲存格。".into());
                        }
                        match value {
                            Value::String(s) => {
                                super::super::text::validate(s)?;
                                set(&target, "Value2", format!("'{s}").as_str().into())?;
                            }
                            Value::Number(n) => set(
                                &target,
                                "Value2",
                                n.as_f64()
                                    .filter(|n| n.is_finite())
                                    .ok_or("數字無效。")?
                                    .into(),
                            )?,
                            Value::Null => {
                                invoke(&target, "ClearContents", vec![], false)?;
                            }
                            _ => return Err("儲存格只接受字串、有限數字或 null。".into()),
                        }
                    }
                }
            }
            Action::PptSlide {
                layout,
                title,
                body,
                right,
            } if self.collection == "Presentations" => {
                let layout_id = match layout.as_str() {
                    "title" => 1i32,
                    "content" => 2,
                    "two_column" => 3,
                    _ => return Err("版面只接受 title／content／two_column。".into()),
                };
                if layout_id != 3 && !right.is_empty() {
                    return Err("right 僅適用雙欄版面。".into());
                }
                for t in [title, body, right] {
                    super::super::text::validate(t)?;
                }
                let slides = child(doc, "Slides")?;
                let count = integer(&slides, "Count")?;
                if count >= 200 {
                    return Err("最多 200 張投影片。".into());
                }
                let slide = obj(invoke(
                    &slides,
                    "Add",
                    vec![(count + 1).into(), layout_id.into()],
                    false,
                )?)?;
                let shapes = child(&child(&slide, "Shapes")?, "Placeholders")?;
                for (i, text) in [title, body, right]
                    .into_iter()
                    .take(if layout_id == 3 { 3 } else { 2 })
                    .enumerate()
                {
                    let range = child(
                        &child(&item(&shapes, i as i32 + 1)?, "TextFrame")?,
                        "TextRange",
                    )?;
                    set(&range, "Text", text.replace('\n', "\r").as_str().into())?;
                }
            }
            _ => return Err("此操作與 Office 文件種類不符。".into()),
        }
        Ok(())
    }
}

fn plain(text: &str) -> AppResult<()> {
    super::super::text::validate(text)?;
    if text.chars().any(|c| c.is_control() && c != '\t') {
        return Err("單一段落／表格欄位不可含換行或結構控制字元。".into());
    }
    Ok(())
}

/// 找出真正變動範圍；不切斷 surrogate pair，前後重複字元亦不重疊。
fn difference(old: &str, new: &str) -> (i32, i32, String) {
    let a: Vec<_> = old.chars().collect();
    let b: Vec<_> = new.chars().collect();
    let prefix = a.iter().zip(&b).take_while(|(a, b)| a == b).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    (
        a[..prefix].iter().map(|c| c.len_utf16() as i32).sum(),
        a[prefix..a.len() - suffix]
            .iter()
            .map(|c| c.len_utf16() as i32)
            .sum(),
        b[prefix..b.len() - suffix].iter().collect(),
    )
}

/// 每次從相同來源重建獨立文件，失敗的操作不會改動已接受的工作版本或原檔。
pub fn render(
    source: Option<&Path>,
    name: &Path,
    original: Option<&Snapshot>,
    actions: &[Action],
    output: Option<&Path>,
    cancel: &AtomicBool,
) -> AppResult<Snapshot> {
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() }.map_err(|e| e.to_string())?;
    let _apartment = Apartment;
    let session = Session::start(source.unwrap_or(name), source.is_none())?;
    if let Some(expected) = original {
        let actual = session.snapshot(cancel)?.0;
        if actual != *expected {
            return Err("Office 來源已改變，請重新建立副本。".into());
        }
    }
    for action in actions {
        session.apply(action, cancel)?;
    }
    let snapshot = session.snapshot(cancel)?.0;
    if let Some(output) = output {
        if cancel.load(Ordering::Relaxed) {
            return Err("Office 儲存已取消。".into());
        }
        let method = if session.collection == "Documents" {
            "SaveAs2"
        } else if session.collection == "Workbooks" {
            "SaveAs"
        } else {
            "SaveCopyAs"
        };
        invoke(
            session.document()?,
            method,
            vec![
                output.to_string_lossy().as_ref().into(),
                save_format(&session.ext)?.into(),
            ],
            false,
        )?;
    }
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ranges_and_formats_cannot_escape_fixed_operations() {
        for s in ["[book]A1", "A:A", "A1,B2", "A0", "A1001", "CW1", "A1!B2"] {
            assert!(cell(s).is_err(), "{s}");
        }
        assert_eq!(cell("$CV$1000").unwrap(), (1000, 100));
        assert_eq!(color("#FF0000").unwrap(), 255);
        assert!(color("#FF").is_err());
        assert!(Format {
            size: Some(f64::NAN),
            ..Format::default()
        }
        .validate("Documents")
        .is_err());
        assert!(Format {
            number_format: Some("date".into()),
            ..Format::default()
        }
        .validate("Documents")
        .is_err());
        assert!(rectangular(&[vec![1], vec![2, 3]]).is_err());
    }
    #[test]
    fn edits_preserve_unmodified_runs_and_unicode_boundaries() {
        assert_eq!(difference("a😀 old z", "a😀 new z"), (4, 3, "new".into()));
        assert_eq!(difference("abc", "abcd"), (3, 0, "d".into()));
        assert_eq!(difference("same", "same"), (4, 0, String::new()));
        assert!(plain("text\u{7}").is_err());
    }
}
