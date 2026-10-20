//! Excel 座標在本機讀取、轉換及產生固定 CAD 操作；模型只收到預覽與計畫識別碼。
use super::*;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    sheet: usize,
    header_row: usize,
    start_row: usize,
    row_count: usize,
    x_column: String,
    y_column: String,
    radius_column: Option<String>,
    label_column: Option<String>,
    kind: String,
    radius: f64,
    scale: f64,
    origin: [f64; 2],
    color: u16,
    height: f64,
    closed: bool,
    units_code: u32,
}

/// 一份預覽最多 100 個操作；無效／空白列整批拒絕，不暗中跳過或截斷。
fn operations(page: &office::excel::Page, settings: &Settings) -> AppResult<Vec<Value>> {
    let s = settings;
    if !matches!(s.kind.as_str(), "circles" | "lines")
        || !(1..=100).contains(&s.row_count)
        || !(1..=255).contains(&s.color)
        || ![1, 4, 5, 6].contains(&s.units_code)
        || !s.scale.is_finite()
        || s.scale <= 0.0
        || !s.height.is_finite()
        || s.height <= 0.0
        || s.height > 1_000_000.0
        || s.origin.iter().any(|n| !n.is_finite())
        || (s.kind == "lines" && s.radius_column.is_some())
        || (s.kind == "circles" && s.closed)
    {
        return Err("座標計畫格式不符：circles/lines、1–100 列、正縮放、合法色碼與單位（1英吋/4mm/5cm/6m）。".into());
    }
    if page.rows.len() != s.row_count {
        return Err("Excel 未回傳完整指定列數；請修正範圍，不產生部分計畫。".into());
    }
    let index = |column: &str| {
        page.columns
            .iter()
            .position(|c| c.eq_ignore_ascii_case(column))
            .ok_or("Excel 欄位不在本次讀取範圍。".to_owned())
    };
    let xi = index(&s.x_column)?;
    let yi = index(&s.y_column)?;
    let ri = s.radius_column.as_deref().map(index).transpose()?;
    let li = s.label_column.as_deref().map(index).transpose()?;
    let mut result = vec![];
    let mut points = vec![];
    for row in &page.rows {
        let numeric = |i: usize| -> AppResult<f64> {
            let cell = row.cells.get(i).ok_or("Excel 儲存格缺漏。")?;
            let value = cell.value.as_f64().filter(|v| v.is_finite());
            if cell.kind != "number" || value.is_none() {
                return Err(format!(
                    "Excel 第 {} 列 {} 欄不是有限數值；未跳過此列。",
                    row.row, page.columns[i]
                ));
            }
            Ok(value.unwrap())
        };
        let point = [
            numeric(xi)? * s.scale + s.origin[0],
            numeric(yi)? * s.scale + s.origin[1],
        ];
        if point
            .iter()
            .any(|n| !n.is_finite() || n.abs() > 1_000_000.0)
        {
            return Err(format!("第 {} 列換算座標超出範圍。", row.row));
        }
        if s.kind == "circles" {
            let radius = ri.map(numeric).transpose()?.unwrap_or(s.radius) * s.scale;
            if !radius.is_finite() || radius <= 0.0 || radius > 1_000_000.0 {
                return Err(format!("第 {} 列半徑必須為正且不超過一百萬。", row.row));
            }
            result.push(json!({"kind":"circle","center":point,"radius":radius,"color":s.color}));
        }
        if let Some(i) = li {
            let label = &row.cells.get(i).ok_or("標籤欄缺漏。")?.text;
            if label.is_empty() || label.chars().count() > 1000 || label.contains('\0') {
                return Err(format!("第 {} 列標籤為空或過長。", row.row));
            }
            result.push(json!({"kind":"text","position":point,"text":label,"height":s.height,"color":s.color}));
        }
        points.push(point);
    }
    if s.kind == "lines" {
        if points.len() < 2 || (s.closed && points.len() < 3) {
            return Err("連線至少兩點，閉合至少三點。".into());
        }
        if s.closed {
            points.push(points[0]);
        }
        for pair in points.windows(2) {
            if pair[0] == pair[1] {
                return Err("連續座標相同會產生零長度線，請修正來源。".into());
            }
            result.push(json!({"kind":"line","start":pair[0],"end":pair[1],"color":s.color}));
        }
    }
    if result.is_empty() || result.len() > 100 {
        return Err("計畫超過單次 100 個 CAD 操作，請縮小 Excel 範圍。".into());
    }
    Ok(result)
}

impl Broker {
    pub(super) fn plan_cad_excel(
        &mut self,
        path: &str,
        revision: &str,
        settings: &Value,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        let s: Settings = serde_json::from_value(settings.clone()).map_err(|e| e.to_string())?;
        if !(1..=100).contains(&s.row_count) {
            return Err("每次最多 100 列。".into());
        }
        let mut columns = vec![s.x_column.clone(), s.y_column.clone()];
        for column in [&s.radius_column, &s.label_column].into_iter().flatten() {
            if !columns.iter().any(|c| c.eq_ignore_ascii_case(column)) {
                columns.push(column.clone());
            }
        }
        let selection = office::excel::Selection {
            sheet: s.sheet,
            header_row: s.header_row,
            start_row: s.start_row,
            row_count: s.row_count,
            columns,
        };
        let (page, _) = self.with_excel(path, Some(revision), |target| {
            office::excel::read(target, &selection, cancel)
        })?;
        let ops = operations(&page, &s)?;
        if self
            .cad_reads
            .keys()
            .filter(|k| k.starts_with("excel-plan:"))
            .count()
            >= 20
        {
            return Err("本任務最多 20 份座標計畫。".into());
        }
        let id = format!("excel-plan:{}", crate::jobs::new_id()?);
        self.cad_reads.insert(
            id.clone(),
            json!({"source":path,"revision":revision,"settings":settings,"operations":ops}),
        );
        Ok(
            json!({"plan_id":id,"source":path,"revision":revision,"settings":settings,"rows":page.rows.len(),
            "operation_count":ops.len(),"preview":ops.iter().take(6).collect::<Vec<_>>(),
            "instructions":"先確認欄位、單位、原點與 scale；cad_excel_apply 使用本計畫，不手抄數列。circles 半徑乘 scale；文字高度為目標圖面單位，標籤放在座標點。lines 依列順序連線。"}),
        )
    }

    pub(super) fn apply_cad_excel(&mut self, tool: &Tool, cancel: &AtomicBool) -> AppResult<Value> {
        let Tool::CadExcelApply {
            plan_id,
            target,
            path,
            revision,
            name,
            version,
        } = tool
        else {
            return Err("不是座標計畫。".into());
        };
        let plan = self
            .cad_reads
            .get(plan_id)
            .filter(|_| plan_id.starts_with("excel-plan:"))
            .cloned()
            .ok_or("座標計畫不存在，請重新預覽。")?;
        self.with_excel(
            plan["source"].as_str().ok_or("缺少來源。")?,
            plan["revision"].as_str(),
            |_| Ok(()),
        )?;
        let ops = plan["operations"].as_array().ok_or("缺少座標計畫操作。")?;
        let expected_units = &plan["settings"]["units_code"];
        let actual_units = match target.as_str() {
            "new" => json!(4),
            "active" => self.cad_active.as_ref().ok_or("請先讀取目前圖面。")?["units_code"].clone(),
            "copy" => self
                .cad_reads
                .get(path.as_deref().ok_or("缺少 DWG 路徑。")?)
                .ok_or("請先讀取 DWG。")?["units_code"]
                .clone(),
            _ => return Err("target 只接受 new/active/copy。".into()),
        };
        if actual_units != *expected_units {
            return Err("圖面單位與座標計畫不符或未知，請先確認 CAD 單位並重新預覽。".into());
        }
        // 消耗計畫後才執行，未知結果不可重送同一份座標計畫。
        self.cad_reads.remove(plan_id);
        match target.as_str() {
            "new" => self.draw_cad(name, version, ops, cancel),
            "active" => self.edit_active_cad(
                revision.as_deref().ok_or("缺少目前圖面 revision。")?,
                &[],
                ops,
                cancel,
            ),
            _ => self.edit_cad(
                &Tool::CadEdit {
                    path: path.clone().ok_or("缺少 DWG 路徑。")?,
                    revision: revision.clone().ok_or("缺少 DWG revision。")?,
                    name: name.clone(),
                    edits: vec![],
                    operations: ops.clone(),
                },
                cancel,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (office::excel::Page, Settings) {
        let page = serde_json::from_value(
            json!({"sheet":1,"sheet_name":"coordinates","columns":["A","B","C","D"],
            "header_row":1,"headers":[],"start_row":2,"next_row":null,"date_1904":false,
            "used_range":{"first_row":1,"last_row":3,"first_column":"A","last_column":"D"},
            "rows":[{"row":2,"cells":[
                {"value":10,"text":"10","kind":"number","formula":null},
                {"value":20,"text":"20","kind":"number","formula":null},
                {"value":2,"text":"2","kind":"number","formula":null},
                {"value":"孔一","text":"孔一","kind":"text","formula":null}]},
                {"row":3,"cells":[
                {"value":30,"text":"30","kind":"number","formula":null},
                {"value":40,"text":"40","kind":"number","formula":null},
                {"value":3,"text":"3","kind":"number","formula":null},
                {"value":"孔二","text":"孔二","kind":"text","formula":null}]}]}),
        )
        .unwrap();
        let settings = serde_json::from_value(json!({"sheet":1,"header_row":1,"start_row":2,"row_count":2,
            "x_column":"A","y_column":"B","radius_column":"C","label_column":"D","kind":"circles",
            "radius":5,"scale":2,"origin":[1,-1],"color":3,"height":4,"closed":false,"units_code":4})).unwrap();
        (page, settings)
    }

    #[test]
    fn coordinates_preserve_rows_labels_and_apply_scale() {
        let (page, settings) = fixture();
        let ops = operations(&page, &settings).unwrap();
        assert_eq!(ops.len(), 4);
        assert_eq!(ops[0]["center"], json!([21.0, 39.0]));
        assert_eq!(ops[0]["radius"], 4.0);
        assert_eq!(ops[1]["text"], "孔一");
        assert_eq!(ops[1]["height"], 4.0);
    }

    #[test]
    fn coordinates_reject_blank_negative_radius_and_missing_rows() {
        let (mut page, settings) = fixture();
        page.rows[0].cells[0].value = Value::Null;
        assert!(operations(&page, &settings).is_err());
        let (mut page, settings) = fixture();
        page.rows[0].cells[2].value = json!(-1);
        assert!(operations(&page, &settings).is_err());
        page.rows.clear();
        assert!(operations(&page, &settings).is_err());
    }

    #[test]
    fn coordinates_connect_in_source_order_without_implicit_closure() {
        let (page, mut settings) = fixture();
        settings.kind = "lines".into();
        settings.radius_column = None;
        settings.label_column = None;
        let ops = operations(&page, &settings).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0]["start"], json!([21.0, 39.0]));
        assert_eq!(ops[0]["end"], json!([61.0, 79.0]));
        settings.closed = true;
        assert!(operations(&page, &settings).is_err());
    }

    #[test]
    fn coordinates_reject_unknown_units_and_nonfinite_transform() {
        let (page, mut settings) = fixture();
        settings.units_code = 0;
        assert!(operations(&page, &settings).is_err());
        settings.units_code = 4;
        settings.scale = f64::NAN;
        assert!(operations(&page, &settings).is_err());
    }
}
