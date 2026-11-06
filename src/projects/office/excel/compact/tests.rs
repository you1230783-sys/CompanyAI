use super::*;
use crate::projects::office::excel::Cell;

/// 測試端依公開欄位還原，不能呼叫轉換端的 default_text，避免同一錯誤同時存在於編解碼兩端。
fn restore(data: &Value) -> Value {
    let columns = data["columns"].as_array().unwrap();
    let mut headers: Vec<Value> = data["headers"].as_array().unwrap().iter()
        .map(|text| json!({"value":text,"text":text,"kind":"text","number_format":"General","formula":null})).collect();
    for exception in data["exceptions"].as_array().unwrap() {
        if exception["header"] == true {
            let index = columns
                .iter()
                .position(|c| c == &exception["column"])
                .unwrap();
            headers[index] = exception["cell"].clone();
        }
    }
    let rows: Vec<_> = data["rows"].as_array().unwrap().iter().map(|row| {
        let cells: Vec<_> = columns.iter().enumerate().map(|(index, column)| {
            let value = &row[index+1];
            let text = match value {
                Value::String(text) => text.clone(),
                Value::Bool(true) => "TRUE".into(),
                Value::Bool(false) => "FALSE".into(),
                Value::Null => String::new(),
                Value::Number(n) if n.is_f64() => format!("{}", n.as_f64().unwrap()),
                _ => value.to_string(),
            };
            let key = column.as_str().unwrap();
            let mut cell = json!({"value":value,"text":text,"kind":data["column_kinds"][key],"number_format":data["column_formats"][key],"formula":null});
            for field in ["exceptions","cell_texts","formulas"] {
                for item in data[field].as_array().unwrap() {
                    if item["header"] != true && item["row"] == row[0] && &item["column"] == column {
                        for name in ["kind","number_format","text","formula"] {
                            if let Some(value) = item.get(name) { cell[name] = value.clone(); }
                        }
                    }
                }
            }
            // 由 Cell serde 還原原本「未知格式不序列化空字串」的行為。
            serde_json::to_value(serde_json::from_value::<Cell>(cell).unwrap()).unwrap()
        }).collect();
        json!({"row":row[0],"cells":cells})
    }).collect();
    json!({"sheet":data["sheet"],"sheet_name":data["sheet_name"],"columns":columns,
        "header_row":data["header_row"],"headers":headers,"start_row":data["start_row"],
        "rows":rows,"next_row":data["next_row"],"used_range":data["used_range"],"date_1904":data["date_1904"]})
}

fn cell(value: Value, text: &str, kind: &str, format: &str, formula: Option<&str>) -> Value {
    json!({"value":value,"text":text,"kind":kind,"number_format":format,"formula":formula})
}
fn page(headers: Vec<Value>, rows: Vec<Value>) -> Page {
    serde_json::from_value(json!({"sheet":1,"sheet_name":"資料","columns":["A","F"],"header_row":2,
        "headers":headers,"start_row":2,"rows":rows,"next_row":10,
        "used_range":{"first_row":2,"last_row":10000,"first_column":"A","last_column":"F"},"date_1904":true})).unwrap()
}

#[test]
fn compact_roundtrip_preserves_every_cell_and_special_headers() {
    let cells = vec![
        cell(json!(1.0), "1", "number", "General", None),
        cell(json!(1.0), "1.00", "number", "0.00", None),
        cell(
            json!(45964.333),
            "2025/11/3 08:00",
            "number",
            "yyyy/m/d h:mm",
            None,
        ),
        cell(json!(0.5), "12:00", "number", "h:mm", None),
        cell(json!(true), "TRUE", "boolean", "General", None),
        cell(json!(false), "假", "boolean", "General", None),
        cell(Value::Null, "", "blank", "General", None),
        cell(Value::Null, "#DIV/0!", "error", "General", Some("=1/0")),
        cell(
            json!(12.34),
            "12.34 元",
            "number",
            "0.00 元",
            Some("=SUM(A1:A2)"),
        ),
        cell(json!("00123"), "00123", "text", "@", None),
        cell(json!("保留\n換行"), "保留\n換行", "merged", "General", None),
        cell(json!(1.0), "1", "number", "", None),
    ];
    let rows = cells
        .chunks(2)
        .enumerate()
        .map(|(i, c)| json!({"row":i+2,"cells":c}))
        .collect();
    let page = page(
        vec![
            cell(json!("名稱"), "名稱", "text", "General", None),
            cell(Value::Null, "#NAME?", "error", "@", Some("=missing")),
        ],
        rows,
    );
    let expected = serde_json::to_value(&page).unwrap();
    let compact = compact(&page);
    assert_eq!(restore(&compact), expected);
    assert_eq!(
        serde_json::to_value(&page).unwrap(),
        expected,
        "不可改動內部 Page"
    );
    assert_eq!(compact["row_fields"], json!(["row", "A", "F"]));
    assert_eq!(compact["rows"][0], json!([2, 1.0, 1.0]));
    assert!(compact["cell_texts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["text"] == "1.00"));
}

#[test]
fn compact_empty_page_and_numeric_headers_roundtrip() {
    let page = page(
        vec![
            cell(json!(4.0), "4.00", "number", "0.00", None),
            cell(Value::Null, "", "blank", "General", None),
        ],
        vec![],
    );
    assert_eq!(
        restore(&compact(&page)),
        serde_json::to_value(page).unwrap()
    );
}

#[test]
fn compact_numeric_100_by_9_is_under_forty_percent() {
    let columns: Vec<_> = (1..=9)
        .map(crate::projects::office::excel::column_name)
        .collect();
    let headers: Vec<_> = columns
        .iter()
        .map(|c| cell(json!(c), c, "text", "General", None))
        .collect();
    let rows: Vec<_> = (2..102)
        .map(|r| {
            json!({"row":r,"cells":(0..9).map(|c| {
        let n = f64::from(r * 9+c) / 10.0;
        cell(json!(n),&n.to_string(),"number","General",None)
    }).collect::<Vec<_>>()})
        })
        .collect();
    let mut value = serde_json::to_value(page(vec![], vec![])).unwrap();
    value["columns"] = json!(columns);
    value["headers"] = json!(headers);
    value["rows"] = json!(rows);
    let page: Page = serde_json::from_value(value).unwrap();
    let old = serde_json::to_value(&page).unwrap();
    let new = compact(&page);
    let old_chars = old.to_string().chars().count();
    let new_chars = new.to_string().chars().count();
    println!(
        "COMPACT_SIZE old={old_chars} new={new_chars} ratio={:.4}",
        new_chars as f64 / old_chars as f64
    );
    assert!(new_chars * 100 < old_chars * 40);
    assert_eq!(restore(&new), old);
}
