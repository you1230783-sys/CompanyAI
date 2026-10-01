//! Excel 唯讀選欄工具：只枚舉要求的儲存格，不建立整張 UsedRange 的文字／格式快照。
//! 仍透過公司核准的 Excel COM 開啟文件；不自行解析可能加密的 XLSX 容器。
use super::*;
use serde_json::{json, Value};

pub const MAX_CELLS: usize = 2000;
pub fn default_sheet() -> usize {
    1
}
pub fn default_rows() -> usize {
    100
}
pub fn default_columns() -> usize {
    50
}
pub fn default_column() -> String {
    "A".into()
}

/// 欄位只接受英文字母；不接受公式、命名範圍、活頁簿／工作表限定式或整欄表達式。
pub fn column_index(value: &str) -> AppResult<usize> {
    if value.is_empty() || value.len() > 3 || !value.bytes().all(|b| b.is_ascii_alphabetic()) {
        return Err("Excel 欄位請使用 A、F、AA 等欄名。".into());
    }
    let index = value.bytes().fold(0usize, |n, b| {
        n * 26 + usize::from(b.to_ascii_uppercase() - b'A' + 1)
    });
    if index > 16384 {
        return Err("Excel 欄位不可超過 XFD。".into());
    }
    Ok(index)
}
pub fn column_name(mut index: usize) -> String {
    let mut letters = Vec::new();
    while index > 0 {
        index -= 1;
        letters.push((b'A' + (index % 26) as u8) as char);
        index /= 26;
    }
    letters.into_iter().rev().collect()
}
fn check_row(row: usize) -> AppResult<()> {
    if !(1..=1_048_576).contains(&row) {
        return Err("Excel 列號需介於 1 到 1048576。".into());
    }
    Ok(())
}
fn check_cancel(cancel: &AtomicBool) -> AppResult<()> {
    if cancel.load(Ordering::Relaxed) {
        return Err("Excel 讀取已取消。".into());
    }
    Ok(())
}

#[derive(Clone)]
pub struct Selection {
    pub sheet: usize,
    pub columns: Vec<String>,
    pub header_row: usize,
    pub start_row: usize,
    pub row_count: usize,
}
impl Selection {
    pub fn validate(&self) -> AppResult<()> {
        self.validate_limits(1000, 100, MAX_CELLS)
    }
    /// 直接畫圖不把逐格快照送給模型，使用獨立額度；一般分批閱讀不放寬。
    pub fn validate_chart(&self) -> AppResult<()> {
        self.validate_limits(super::super::charts::MAX_POINTS, 9, 90_000)
    }
    fn validate_limits(
        &self,
        max_rows: usize,
        max_columns: usize,
        max_cells: usize,
    ) -> AppResult<()> {
        check_row(self.header_row)?;
        check_row(self.start_row)?;
        if self.sheet == 0
            || self.sheet > i32::MAX as usize
            || self.columns.is_empty()
            || self.columns.len() > max_columns
            || !(1..=max_rows).contains(&self.row_count)
            || self.columns.len() * self.row_count > max_cells
            || self.start_row + self.row_count - 1 > 1_048_576
        {
            return Err(format!("Excel 此操作限 1–{max_columns} 欄、1–{max_rows} 列，資料合計最多 {max_cells} 格；請縮小範圍或分批處理。"));
        }
        let mut seen = std::collections::BTreeSet::new();
        for column in &self.columns {
            if !seen.insert(column_index(column)?) {
                return Err("Excel 選取欄位不可重複。".into());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Cell {
    pub value: Value,
    pub text: String,
    pub kind: String,
    /// 只供核對；數值為 Excel 在本次開啟時提供的 Value2，不執行模型提供的公式。
    pub formula: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Row {
    pub row: usize,
    pub cells: Vec<Cell>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Bounds {
    pub first_row: usize,
    pub last_row: usize,
    pub first_column: String,
    pub last_column: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Page {
    pub sheet: usize,
    pub sheet_name: String,
    pub columns: Vec<String>,
    pub header_row: usize,
    pub headers: Vec<Cell>,
    pub start_row: usize,
    pub rows: Vec<Row>,
    pub next_row: Option<usize>,
    pub used_range: Bounds,
    pub date_1904: bool,
}

fn bounds(sheet: &IDispatch) -> AppResult<Bounds> {
    let used = child(sheet, "UsedRange")?;
    let first_row = integer(&used, "Row")?;
    let first_column = integer(&used, "Column")?;
    let rows = integer(&child(&used, "Rows")?, "Count")?;
    let columns = integer(&child(&used, "Columns")?, "Count")?;
    if first_row < 1
        || first_column < 1
        || rows < 1
        || columns < 1
        || first_row as i64 + rows as i64 - 1 > 1_048_576
        || first_column as i64 + columns as i64 - 1 > 16_384
    {
        return Err("Excel 使用範圍無效。".into());
    }
    Ok(Bounds {
        first_row: first_row as usize,
        last_row: (first_row + rows - 1) as usize,
        first_column: column_name(first_column as usize),
        last_column: column_name((first_column + columns - 1) as usize),
    })
}
fn worksheet(session: &Session, index: usize) -> AppResult<IDispatch> {
    let sheets = child(session.document()?, "Worksheets")?;
    if index == 0 || index > integer(&sheets, "Count")? as usize {
        return Err("Excel 工作表序號不存在。".into());
    }
    item(&sheets, index as i32)
}
fn cell(sheet: &IDispatch, column: &str, row: usize) -> AppResult<Cell> {
    // 欄名已正規化，列號已限制；Range 絕不接收模型提供的任意表達式。
    let address = format!("{column}{row}");
    let target = obj(invoke(
        sheet,
        "Range",
        vec![address.as_str().into()],
        false,
    )?)?;
    let value = get(&target, "Value2")?;
    let text = string(&get(&target, "Text")?)?;
    let formula = if bool::try_from(&get(&target, "HasFormula")?)
        .map_err(|_| "Excel 公式標記無效。")?
    {
        Some(string(&get(&target, "Formula")?)?)
    } else {
        None
    };
    let (value, mut kind) = match unsafe { value.Anonymous.Anonymous.vt } {
        VT_EMPTY | VT_NULL => (Value::Null, "blank"),
        VT_R8 | VT_I4 | VT_I2 => {
            let number = f64::try_from(&value).map_err(|_| "Excel 數值無法讀取。")?;
            if !number.is_finite() {
                return Err("Excel 數值不是有限數字。".into());
            }
            (json!(number), "number")
        }
        VT_BSTR => (json!(string(&value)?), "text"),
        VT_BOOL => (
            json!(bool::try_from(&value).map_err(|_| "Excel 布林值無效。")?),
            "boolean",
        ),
        VT_ERROR => (Value::Null, "error"),
        _ => return Err("Excel 儲存格型別不支援，請縮小範圍確認來源。".into()),
    };
    if bool::try_from(&get(&target, "MergeCells")?).map_err(|_| "Excel 合併格標記無效。")? {
        kind = "merged";
    }
    Ok(Cell {
        value,
        text,
        kind: kind.into(),
        formula,
    })
}
fn session(path: &Path) -> AppResult<Session> {
    let session = Session::open(path)?;
    if session.collection != "Workbooks" {
        return Err("選欄工具僅支援 XLS、XLSX、XLSM、XLSB。".into());
    }
    Ok(session)
}
fn bounded(value: Value) -> AppResult<Value> {
    if value.to_string().len() > super::super::text::MAX_TEXT {
        return Err("選取內容超過 200 KB，請減少欄數或列數；本輪未回傳不完整資料。".into());
    }
    Ok(value)
}

/// 邊讀邊限制內容，不先將上千個超長儲存格全部放入記憶體才拒絕。
fn account_cell(cell: &Cell, bytes: &mut usize) -> AppResult<()> {
    account_cell_limit(cell, bytes, super::super::text::MAX_TEXT)
}
fn account_cell_limit(cell: &Cell, bytes: &mut usize, limit: usize) -> AppResult<()> {
    *bytes += serde_json::to_string(cell)
        .map_err(|e| e.to_string())?
        .len();
    if *bytes > limit {
        return Err("選取文字過多，請減少列數或欄數後重讀；未截斷儲存格。".into());
    }
    Ok(())
}

/// 只列工作表名稱、所選工作表的範圍與有限表頭，UsedRange 再大也不逐格讀取。
pub fn inspect(
    path: &Path,
    sheet: usize,
    header_row: usize,
    start_column: &str,
    column_count: usize,
    cancel: &AtomicBool,
) -> AppResult<Value> {
    check_cancel(cancel)?;
    check_row(header_row)?;
    let start = column_index(start_column)?;
    if !(1..=100).contains(&column_count) || start + column_count - 1 > 16384 {
        return Err("表頭每次限 1–100 欄，且不可超出 XFD。".into());
    }
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() }.map_err(|e| e.to_string())?;
    let _apartment = Apartment;
    let session = session(path)?;
    let worksheets = child(session.document()?, "Worksheets")?;
    let total = integer(&worksheets, "Count")?;
    let mut names = vec![];
    for i in 1..=total.min(100) {
        check_cancel(cancel)?;
        names.push(json!({"sheet":i,"name":string(&get(&item(&worksheets,i)?,"Name")?)?}));
    }
    let selected = worksheet(&session, sheet)?;
    let used = bounds(&selected)?;
    let end = (start + column_count - 1).min(column_index(&used.last_column)?);
    let mut headers = vec![];
    let mut bytes = 0;
    for c in start..=end {
        check_cancel(cancel)?;
        let column = column_name(c);
        let value = cell(&selected, &column, header_row)?;
        account_cell(&value, &mut bytes)?;
        headers.push(json!({"column":column,"cell":value}));
    }
    bounded(
        json!({"sheets":names,"sheet_count":total,"sheets_truncated":total>100,"sheet":sheet,
        "sheet_name":string(&get(&selected,"Name")?)?,"used_range":used,"header_row":header_row,"headers":headers,
        "next_header_column":if end < column_index(&used.last_column)? {Some(column_name(end+1))}else{None},
        "default_row_count":100,"max_data_cells":MAX_CELLS,"scope":"僅工作表目錄與表頭；UsedRange 可能包含空白或格式儲存格，不代表已讀全文。"}),
    )
}

/// 保留原始列號與欄位順序；空白、錯誤值不刪除，避免 A／F 兩欄錯位。
pub fn read(path: &Path, selection: &Selection, cancel: &AtomicBool) -> AppResult<Page> {
    selection.validate()?;
    read_bounded(path, selection, cancel, super::super::text::MAX_TEXT)
}

/// 僅供本機畫圖使用；最多 90000 格、32 MiB 暫存資料，不回傳整批 Cell 給模型。
pub fn read_chart(path: &Path, selection: &Selection, cancel: &AtomicBool) -> AppResult<Page> {
    selection.validate_chart()?;
    read_bounded(path, selection, cancel, 32 * 1024 * 1024)
}

fn read_bounded(
    path: &Path,
    selection: &Selection,
    cancel: &AtomicBool,
    limit: usize,
) -> AppResult<Page> {
    check_cancel(cancel)?;
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() }.map_err(|e| e.to_string())?;
    let _apartment = Apartment;
    let session = session(path)?;
    let sheet = worksheet(&session, selection.sheet)?;
    let used_range = bounds(&sheet)?;
    let columns = selection
        .columns
        .iter()
        .map(|c| c.to_ascii_uppercase())
        .collect::<Vec<_>>();
    let mut headers = Vec::new();
    let mut bytes = 0;
    for column in &columns {
        check_cancel(cancel)?;
        let value = cell(&sheet, column, selection.header_row)?;
        account_cell_limit(&value, &mut bytes, limit)?;
        headers.push(value);
    }
    let end = (selection.start_row + selection.row_count - 1).min(used_range.last_row);
    let mut rows = vec![];
    for row in selection.start_row..=end {
        let mut cells = Vec::new();
        for column in &columns {
            check_cancel(cancel)?;
            let value = cell(&sheet, column, row)?;
            account_cell_limit(&value, &mut bytes, limit)?;
            cells.push(value);
        }
        rows.push(Row { row, cells });
    }
    let page = Page {
        sheet: selection.sheet,
        sheet_name: string(&get(&sheet, "Name")?)?,
        columns,
        header_row: selection.header_row,
        headers,
        start_row: selection.start_row,
        rows,
        next_row: if end < used_range.last_row {
            Some(end + 1)
        } else {
            None
        },
        used_range,
        date_1904: bool::try_from(&get(session.document()?, "Date1904")?)
            .map_err(|_| "Excel 日期系統無效。")?,
    };
    if serde_json::to_vec(&page).map_err(|e| e.to_string())?.len() > limit {
        return Err("Excel 選取資料超過本次操作容量，請縮小範圍；未自動抽樣。".into());
    }
    Ok(page)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chart_limits_are_independent_of_model_read_pages() {
        let mut selection = Selection {
            sheet: 1,
            columns: (1..=9).map(column_name).collect(),
            header_row: 1,
            start_row: 2,
            row_count: 10_000,
        };
        selection.validate_chart().unwrap();
        assert!(selection.validate().is_err());
        selection.row_count = 10_001;
        assert!(selection.validate_chart().is_err());
        selection.row_count = 3000;
        selection.columns = vec!["A".into(), "F".into()];
        selection.validate_chart().unwrap();
        assert!(selection.validate().is_err());
    }
    #[test]
    fn selected_columns_are_bounded_without_counting_intervening_columns() {
        let mut selection = Selection {
            sheet: 1,
            columns: vec!["A".into(), "F".into()],
            header_row: 1,
            start_row: 2,
            row_count: 1000,
        };
        selection.validate().unwrap();
        selection.columns.push("G".into());
        assert!(selection.validate().is_err());
        selection.row_count = 100;
        selection.columns = vec!["A".into(), "a".into()];
        assert!(selection.validate().is_err());
        for column in ["A:A", "A1", "Sheet1!A", "[x]A", "XFE", "", "=A"] {
            assert!(column_index(column).is_err());
        }
        assert_eq!(column_name(column_index("xfd").unwrap()), "XFD");
        selection.columns = vec!["A".into()];
        selection.start_row = usize::MAX;
        assert!(selection.validate().is_err());
    }
}
