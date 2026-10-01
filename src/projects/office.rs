//! Office 的固定 COM 介面。處理 Word 正文段落、Excel 儲存格、PowerPoint 一般文字框。
//! 模型拿到區塊 ID，不可指定 COM 方法。原件以唯讀開啟，套用經核對的修改後另存。
use crate::{wide, AppResult};
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
use windows::{
    core::{GUID, PCWSTR},
    Win32::System::{Com::*, Variant::*},
};

mod authoring;
pub mod excel;
pub mod images;
pub use authoring::{render, Action, Format};
#[cfg(debug_assertions)]
mod fixtures;
#[cfg(debug_assertions)]
pub use fixtures::create as create_test_fixtures;

const MAX_BLOCKS: usize = 2000;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Block {
    pub id: String,
    pub label: String,
    pub text: String,
    /// text 與 number 可改；formula / readonly 保留原件內容。
    pub kind: String,
    /// 索引至 Snapshot.formats；相同格式共用，避免數千儲存格重複佔用上下文。
    #[serde(default)]
    pub format: serde_json::Value,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Snapshot {
    pub scope: String,
    #[serde(default)]
    pub structure: serde_json::Value,
    #[serde(default)]
    pub formats: Vec<serde_json::Value>,
    pub blocks: Vec<Block>,
}
impl Snapshot {
    pub fn serialize(&self) -> AppResult<String> {
        let value = serde_json::to_string(self).map_err(|e| e.to_string())?;
        if value.len() > super::text::MAX_TEXT {
            return Err("Office 文字超過 200 KB，請使用較小文件。".into());
        }
        Ok(value)
    }
    /// 完整區塊比對，避免索引因別人修改而指向另一個段落或儲存格。
    pub fn edit(&mut self, id: &str, expected: &str, replacement: &str) -> AppResult<()> {
        super::text::validate(replacement)?;
        let block = self
            .blocks
            .iter_mut()
            .find(|b| b.id == id)
            .ok_or("找不到此 Office 區塊。")?;
        if block.text != expected {
            return Err("Office 區塊原文已改變，請重新讀取。".into());
        }
        if block.kind == "number" {
            let number: f64 = replacement
                .parse()
                .map_err(|_| "數值儲存格只能改為有限數字。")?;
            if !number.is_finite() {
                return Err("數值不可為無限或 NaN。".into());
            }
            block.text = number.to_string();
        } else if block.kind == "text" {
            // Word 段落末端標記不交给模型修改，避免破壞表格及段落索引。
            if self.scope.starts_with("Word") && replacement.contains(['\r', '\n']) {
                return Err("本版 Word 只修訂既有段落文字，請用 office_action 新增段落。".into());
            }
            block.text = replacement.replace("\r\n", "\n").replace('\r', "\n");
        } else {
            return Err("公式、特殊欄位及受保護區塊目前僅供閱讀。".into());
        }
        Ok(())
    }
}
pub fn supported(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "docx" | "doc" | "docm" | "xlsx" | "xls" | "xlsm" | "xlsb" | "pptx" | "ppt" | "pptm"
    )
}

/// 保持來源容器格式，避免將舊版或巨集文件誤存成一般 Open XML。
fn save_format(ext: &str) -> AppResult<i32> {
    Ok(match ext {
        "doc" => 0,
        "docx" => 12,
        "docm" => 13,
        "xls" => 56,
        "xlsx" => 51,
        "xlsm" => 52,
        "xlsb" => 50,
        "ppt" => 1,
        "pptx" => 24,
        "pptm" => 25,
        _ => return Err("不支援此 Office 格式。".into()),
    })
}

/// 呼叫名稱都在此模組內固定，參數在此統一反轉成 IDispatch 規則。
fn invoke(object: &IDispatch, name: &str, mut args: Vec<VARIANT>, put: bool) -> AppResult<VARIANT> {
    let key = wide(name);
    let mut id = 0;
    let mut result = VARIANT::default();
    let mut exception = crate::com_error::DispatchException::default();
    args.reverse();
    let mut property = -3; // DISPID_PROPERTYPUT
    let params = DISPPARAMS {
        rgvarg: args.as_mut_ptr(),
        cArgs: args.len() as u32,
        rgdispidNamedArgs: if put {
            &mut property
        } else {
            std::ptr::null_mut()
        },
        cNamedArgs: u32::from(put),
    };
    unsafe {
        object
            .GetIDsOfNames(&GUID::zeroed(), &PCWSTR(key.as_ptr()), 1, 0, &mut id)
            .and_then(|()| {
                object.Invoke(
                    id,
                    &GUID::zeroed(),
                    0,
                    if put {
                        DISPATCH_PROPERTYPUT
                    } else {
                        DISPATCH_METHOD | DISPATCH_PROPERTYGET
                    },
                    &params,
                    Some(&mut result),
                    Some(&mut exception.0),
                    None,
                )
            })
            .map_err(|e| exception.describe(&format!("Office 操作 {name} 失敗"), &e))?;
    }
    Ok(result)
}
fn get(o: &IDispatch, name: &str) -> AppResult<VARIANT> {
    invoke(o, name, vec![], false)
}
fn obj(value: VARIANT) -> AppResult<IDispatch> {
    IDispatch::try_from(&value).map_err(|_| "Office 未回傳文件物件。".into())
}
fn child(o: &IDispatch, name: &str) -> AppResult<IDispatch> {
    obj(get(o, name)?)
}
fn item(o: &IDispatch, index: i32) -> AppResult<IDispatch> {
    obj(invoke(o, "Item", vec![index.into()], false)?)
}
fn set(o: &IDispatch, name: &str, value: VARIANT) -> AppResult<()> {
    invoke(o, name, vec![value], true).map(|_| ())
}
fn integer(o: &IDispatch, name: &str) -> AppResult<i32> {
    i32::try_from(&get(o, name)?).map_err(|_| format!("Office {name} 不是整數。"))
}
fn string(value: &VARIANT) -> AppResult<String> {
    if unsafe { value.Anonymous.Anonymous.vt } == VT_EMPTY {
        return Ok(String::new());
    }
    let mut output = VARIANT::default();
    unsafe { VariantChangeType(&mut output, value, VAR_CHANGE_FLAGS(0), VT_BSTR) }
        .map_err(|_| "Office 欄位無法轉為文字。")?;
    Ok(unsafe { output.Anonymous.Anonymous.Anonymous.bstrVal.to_string() })
}
fn missing() -> VARIANT {
    let mut value = VARIANT::default();
    unsafe {
        (*value.Anonymous.Anonymous).vt = VT_ERROR;
        (*value.Anonymous.Anonymous).Anonymous.scode = 0x80020004u32 as i32;
    }
    value
}
struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

struct Session {
    app: IDispatch,
    document: Option<IDispatch>,
    ext: String,
    collection: &'static str,
    restore: Vec<(IDispatch, &'static str, VARIANT)>,
}
impl Drop for Session {
    fn drop(&mut self) {
        if let Some(doc) = self.document.take() {
            if self.collection == "Presentations" {
                let _ = set(&doc, "Saved", (-1i32).into());
            }
            let _ = invoke(
                &doc,
                "Close",
                if self.collection == "Presentations" {
                    vec![]
                } else {
                    vec![false.into()]
                },
                false,
            );
        }
        for (object, name, value) in self.restore.drain(..).rev() {
            let _ = set(&object, name, value);
        }
        // 只結束沒有其他文件的本次 Office 程序，絕不關閉使用者後來開啟的文件。
        if child(&self.app, self.collection)
            .and_then(|c| integer(&c, "Count"))
            .ok()
            == Some(0)
        {
            let _ = invoke(&self.app, "Quit", vec![], false);
        }
    }
}
impl Session {
    fn open(path: &Path) -> AppResult<Self> {
        Self::start(path, false)
    }
    fn start(path: &Path, create: bool) -> AppResult<Self> {
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let (prog, collection) = match ext.as_str() {
            "docx" | "doc" | "docm" => ("Word.Application", "Documents"),
            "xlsx" | "xls" | "xlsm" | "xlsb" => ("Excel.Application", "Workbooks"),
            "pptx" | "ppt" | "pptm" => ("PowerPoint.Application", "Presentations"),
            _ => return Err("不支援此 Office 副檔名。".into()),
        };
        let class = unsafe { CLSIDFromProgID(PCWSTR(wide(prog).as_ptr())) }
            .map_err(|_| format!("請先安裝 {prog} 對應的桌面版 Office。"))?;
        // PowerPoint 是單一實例；避免改變使用者現有簡報或共用的巨集設定。
        if collection == "Presentations" {
            let mut active = None;
            if unsafe { windows::Win32::System::Ole::GetActiveObject(&class, None, &mut active) }
                .is_ok()
            {
                return Err("請先儲存並關閉 PowerPoint，再執行簡報工具。".into());
            }
        }
        let app: IDispatch = unsafe { CoCreateInstance(&class, None, CLSCTX_LOCAL_SERVER) }
            .map_err(|e| format!("無法啟動 {prog}：{e}"))?;
        let mut session = Self {
            app,
            document: None,
            ext,
            collection,
            restore: Vec::new(),
        };
        if integer(&child(&session.app, collection)?, "Count")? != 0 {
            return Err("Office 正在使用中，請先儲存並關閉相關視窗後重試。".into());
        }
        session.setting(session.app.clone(), "AutomationSecurity", 3i32.into())?; // msoAutomationSecurityForceDisable
        session.setting(
            session.app.clone(),
            "DisplayAlerts",
            if session.collection == "Presentations" {
                1i32.into()
            } else {
                0i32.into()
            },
        )?;
        if session.collection == "Documents" {
            session.setting(
                child(&session.app, "Options")?,
                "UpdateLinksAtOpen",
                false.into(),
            )?;
        } else if session.collection == "Workbooks" {
            session.setting(session.app.clone(), "EnableEvents", false.into())?;
            session.setting(session.app.clone(), "AskToUpdateLinks", false.into())?;
        }
        if create {
            if !matches!(session.ext.as_str(), "docx" | "xlsx" | "pptx") {
                return Err("新建 Office 僅支援 DOCX、XLSX、PPTX。".into());
            }
            let args = match collection {
                // Word 圖片的比例／替代文字屬性需要文件視窗物件。
                // Application 本身保持隱藏；Visible=true 不把背景 Word 顯示給使用者。
                "Documents" => vec![missing(), false.into(), 0i32.into(), true.into()],
                "Workbooks" => vec![(-4167i32).into()], // xlWBATWorksheet：固定一張工作表
                _ => vec![0i32.into()],
            };
            session.document = Some(obj(invoke(
                &child(&session.app, collection)?,
                "Add",
                args,
                false,
            )?)?);
            return Ok(session);
        }
        let path = VARIANT::from(path.to_string_lossy().as_ref());
        let args = match session.ext.as_str() {
            "docx" | "doc" | "docm" => vec![
                path,
                false.into(),
                true.into(),
                false.into(),
                "".into(),
                "".into(),
                false.into(),
                "".into(),
                "".into(),
                missing(),
                missing(),
                true.into(), // 文件視窗物件供圖片 COM 屬性使用；Application 仍隱藏。
            ],
            "xlsx" | "xls" | "xlsm" | "xlsb" => vec![
                path,
                0i32.into(),
                true.into(),
                missing(),
                "".into(),
                "".into(),
                true.into(),
                missing(),
                missing(),
                false.into(),
                false.into(),
                missing(),
                false.into(),
            ],
            _ => vec![path, (-1i32).into(), 0i32.into(), 0i32.into()],
        };
        session.document = Some(obj(invoke(
            &child(&session.app, collection)?,
            "Open",
            args,
            false,
        )?)?);
        if session.collection != "Presentations" {
            let actual = integer(
                session.document()?,
                if session.collection == "Documents" {
                    "SaveFormat"
                } else {
                    "FileFormat"
                },
            )?;
            let required = save_format(&session.ext)?;
            if actual != required {
                return Err(
                    "文件實際格式與副檔名不同，請先用 Office 轉存與副檔名相符的文件。".into(),
                );
            }
        }
        Ok(session)
    }
    /// 設定只在本次操作有效，包含錯誤路徑都恢复原值。
    fn setting(&mut self, object: IDispatch, name: &'static str, value: VARIANT) -> AppResult<()> {
        let old = get(&object, name)?;
        self.restore.push((object.clone(), name, old));
        set(&object, name, value)
    }
    fn document(&self) -> AppResult<&IDispatch> {
        self.document.as_ref().ok_or("Office 文件未開啟。".into())
    }
    fn snapshot(&self, cancel: &AtomicBool) -> AppResult<(Snapshot, Vec<IDispatch>)> {
        let document = self.document()?;
        let mut blocks = Vec::new();
        let mut formats = Vec::new();
        let mut format_ids = std::collections::BTreeMap::new();
        let mut targets = Vec::new();
        let mut text_bytes = 0usize;
        let mut push = |object: IDispatch,
                        id: String,
                        label: String,
                        text: String,
                        kind: &str|
         -> AppResult<()> {
            if cancel.load(Ordering::Relaxed) {
                return Err("Office 操作已取消。".into());
            }
            if blocks.len() >= MAX_BLOCKS {
                return Err("Office 文件超過 2000 個文字區塊／使用中儲存格，請縮小文件。".into());
            }
            text_bytes += text.len();
            if text_bytes > super::text::MAX_TEXT {
                return Err("Office 文字超過 200 KB，請縮小文件。".into());
            }
            let attributes = authoring::inspect_format(&object, self.collection)?;
            let key = attributes.to_string();
            let index = *format_ids.entry(key).or_insert_with(|| {
                let index = formats.len();
                formats.push(attributes);
                index
            });
            let format = serde_json::json!(index);
            blocks.push(Block {
                id,
                label,
                text,
                kind: kind.into(),
                format,
            });
            targets.push(object);
            Ok(())
        };
        let scope = match self.ext.as_str() {
            "docx" | "doc" | "docm" => {
                let paragraphs = child(document, "Paragraphs")?;
                for i in 1..=integer(&paragraphs, "Count")? {
                    let range = child(&item(&paragraphs, i)?, "Range")?;
                    let raw = string(&get(&range, "Text")?)?;
                    let content = raw.trim_end_matches(['\r', '\u{7}']);
                    // 只縮短尾端標記，不取代整個段落容器；保留表格結構。
                    let end = integer(&range, "End")?
                        - raw[content.len()..].encode_utf16().count() as i32;
                    set(&range, "End", end.into())?;
                    let plain = integer(&child(&range, "Fields")?, "Count")? == 0
                        && integer(&child(&range, "InlineShapes")?, "Count")? == 0;
                    push(
                        range,
                        format!("p{i}"),
                        format!("正文段落 {i}"),
                        content.into(),
                        if plain { "text" } else { "readonly" },
                    )?;
                }
                "Word 正文段落（含表格文字）；不含頁首頁尾、文字方塊、註解；特殊欄位唯讀。"
            }
            "xlsx" | "xls" | "xlsm" | "xlsb" => {
                let sheets = child(document, "Worksheets")?;
                for s in 1..=integer(&sheets, "Count")? {
                    let sheet = item(&sheets, s)?;
                    let name = string(&get(&sheet, "Name")?)?;
                    let cells = child(&child(&sheet, "UsedRange")?, "Cells")?;
                    let count = integer(&cells, "Count")?;
                    if count < 0 || count as usize > MAX_BLOCKS {
                        return Err("Excel 完整編輯快照超過 2000 格；閱讀或畫圖請先用 inspect_excel，再用 read_excel_range／chart_excel_range 選欄取值。".into());
                    }
                    for n in 1..=count {
                        let cell = item(&cells, n)?;
                        let address = string(&get(&cell, "Address")?)?;
                        let formula = bool::try_from(&get(&cell, "HasFormula")?).unwrap_or(true);
                        let merged = bool::try_from(&get(&cell, "MergeCells")?).unwrap_or(true);
                        let value = get(&cell, if formula { "Formula" } else { "Value2" })?;
                        let kind = if formula {
                            "formula"
                        } else if merged {
                            "readonly"
                        } else {
                            match unsafe { value.Anonymous.Anonymous.vt } {
                                VT_R8 | VT_I4 => "number",
                                VT_BSTR | VT_EMPTY => "text",
                                _ => "readonly",
                            }
                        };
                        let text = if unsafe { value.Anonymous.Anonymous.vt } == VT_ERROR {
                            "（Excel 錯誤值）".into()
                        } else {
                            string(&value)?
                        };
                        push(
                            cell,
                            format!("s{s}:{address}"),
                            format!("{name}!{address}"),
                            text,
                            kind,
                        )?;
                    }
                }
                "Excel 工作表 UsedRange（最多 2000 格）；公式顯示公式文字且不可修改，合併格及特殊值唯讀；不含圖表、註解、資料連線。"
            }
            _ => {
                let slides = child(document, "Slides")?;
                let count = integer(&slides, "Count")?;
                if count > 200 {
                    return Err("PowerPoint 上限為 200 張投影片。".into());
                }
                let mut scanned = 0;
                for s in 1..=count {
                    let shapes = child(&item(&slides, s)?, "Shapes")?;
                    for n in 1..=integer(&shapes, "Count")? {
                        scanned += 1;
                        if scanned > MAX_BLOCKS || cancel.load(Ordering::Relaxed) {
                            return Err("PowerPoint 操作已取消或圖形超過 2000 個。".into());
                        }
                        let shape = item(&shapes, n)?;
                        if integer(&shape, "HasTextFrame")? != 0 {
                            let range = child(&child(&shape, "TextFrame")?, "TextRange")?;
                            let content = string(&get(&range, "Text")?)?.replace('\r', "\n");
                            push(
                                range,
                                format!("s{s}:shape{n}"),
                                format!("投影片 {s} / 文字框 {n}"),
                                content,
                                "text",
                            )?;
                        }
                    }
                }
                "PowerPoint 一般文字框及標題；不含群組、SmartArt、圖表、表格、備忘稿或圖片文字。"
            }
        };
        let snapshot = Snapshot {
            scope: scope.into(),
            structure: authoring::structure(self)?,
            formats,
            blocks,
        };
        snapshot.serialize()?;
        Ok((snapshot, targets))
    }
}

/// COM 物件只存在本次背景操作內，成功或錯誤均由 Drop 關閉文件。
/// Office 是受信任的桌面程式，並非文字 AppContainer 的一部分。
pub fn process(
    source: &Path,
    output: Option<&Path>,
    expected: Option<&Snapshot>,
    desired: Option<&Snapshot>,
    cancel: &AtomicBool,
) -> AppResult<Snapshot> {
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() }.map_err(|e| e.to_string())?;
    let _apartment = Apartment;
    let session = Session::open(source)?;
    let (current, targets) = session.snapshot(cancel)?;
    if let Some(expected) = expected {
        if &current != expected {
            return Err("來源 Office 文件已變更，請重新建立工作副本。".into());
        }
    }
    if let Some(desired) = desired {
        if desired.blocks.len() != current.blocks.len() {
            return Err("Office 區塊結構不一致。".into());
        }
        for ((old, new), target) in current.blocks.iter().zip(&desired.blocks).zip(&targets) {
            if cancel.load(Ordering::Relaxed) {
                return Err("Office 操作已取消，未發布。".into());
            }
            if old.text == new.text {
                continue;
            }
            if old.id != new.id || old.kind != new.kind {
                return Err("Office 區塊身分不符。".into());
            }
            match (session.ext.as_str(), old.kind.as_str()) {
                ("xlsx" | "xls" | "xlsm" | "xlsb", "number") => set(
                    target,
                    "Value2",
                    new.text
                        .parse::<f64>()
                        .map_err(|_| "數字格式不正確。")?
                        .into(),
                )?,
                ("xlsx" | "xls" | "xlsm" | "xlsb", "text") => {
                    // 前置單引號要求 Excel 儲存字面文字，不能把 =cmd 等內容當成公式。
                    set(
                        target,
                        "Value2",
                        VARIANT::from(format!("'{}", new.text).as_str()),
                    )?;
                }
                (_, "text") => set(
                    target,
                    "Text",
                    VARIANT::from(new.text.replace('\n', "\r").as_str()),
                )?,
                _ => return Err("拒絕修改唯讀 Office 區塊。".into()),
            }
        }
        if session.snapshot(cancel)?.0 != *desired {
            return Err("Office 修改結果與要求不一致，未發布。".into());
        }
    }
    if let Some(output) = output {
        if cancel.load(Ordering::Relaxed) {
            return Err("Office 儲存已取消。".into());
        }
        let method = match session.ext.as_str() {
            "docx" | "doc" | "docm" => "SaveAs2",
            "xlsx" | "xls" | "xlsm" | "xlsb" => "SaveAs",
            _ => "SaveCopyAs",
        };
        invoke(
            session.document()?,
            method,
            vec![
                VARIANT::from(output.to_string_lossy().as_ref()),
                save_format(&session.ext)?.into(),
            ],
            false,
        )?;
    }
    Ok(desired.unwrap_or(&current).clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample(kind: &str) -> Snapshot {
        Snapshot {
            scope: "Excel".into(),
            structure: serde_json::Value::Null,
            formats: Vec::new(),
            blocks: vec![Block {
                id: "s1:$A$1".into(),
                label: "A1".into(),
                text: "12".into(),
                kind: kind.into(),
                format: serde_json::Value::Null,
            }],
        }
    }
    #[test]
    fn edits_reject_formula_wrong_original_and_nonfinite_numbers() {
        assert!(sample("formula").edit("s1:$A$1", "12", "99").is_err());
        assert!(sample("text").edit("s1:$A$1", "wrong", "99").is_err());
        assert!(sample("number").edit("s1:$A$1", "12", "NaN").is_err());
        let mut value = sample("number");
        value.edit("s1:$A$1", "12", "24.0").unwrap();
        assert_eq!(value.blocks[0].text, "24");
    }
    #[test]
    fn word_structure_is_preserved_and_legacy_formats_are_recognized() {
        let mut value = sample("text");
        value.scope = "Word 正文".into();
        assert!(value.edit("s1:$A$1", "12", "first\nsecond").is_err());
        for ext in ["docm", "xlsm", "pptm", "doc", "xls", "ppt"] {
            assert!(supported(Path::new(&format!("file.{ext}"))));
        }
    }
}
