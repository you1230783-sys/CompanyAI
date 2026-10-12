use super::*;
use windows::Win32::System::Ole::{SafeArrayCreate, SafeArrayPutElement};

/// 建立原生陣列，方向直接依公司診斷輸出的 D1=列、D2=欄，不共用正式解析器。
fn native_array(lengths: &[u32], lower: &[i32]) -> VARIANT {
    assert_eq!(lengths.len(), lower.len());
    let bounds: Vec<_> = lengths
        .iter()
        .zip(lower)
        .map(|(&c_elements, &l_lbound)| SAFEARRAYBOUND {
            cElements: c_elements,
            lLbound: l_lbound,
        })
        .collect();
    let array = unsafe { SafeArrayCreate(VT_VARIANT, bounds.len() as u32, bounds.as_ptr()) };
    assert!(!array.is_null());
    let mut value = VARIANT::default();
    // VARIANT 接手所有權；離開測試時連同各儲存格一起釋放。
    unsafe {
        (*value.Anonymous.Anonymous).vt = VARENUM(VT_ARRAY.0 | VT_VARIANT.0);
        (*value.Anonymous.Anonymous).Anonymous.parray = array;
    }
    value
}

fn numbered_cells(rows: u32, columns: u32, lower: [i32; 2]) -> VARIANT {
    let value = native_array(&[rows, columns], &lower);
    let array = unsafe { value.Anonymous.Anonymous.Anonymous.parray };
    for row in 0..rows as i32 {
        for column in 0..columns as i32 {
            let cell = VARIANT::from(row * 1000 + column);
            unsafe {
                SafeArrayPutElement(
                    array,
                    [lower[0] + row, lower[1] + column].as_ptr(),
                    (&cell as *const VARIANT).cast(),
                )
            }
            .unwrap();
        }
    }
    value
}

#[test]
fn native_rows_first_batches_preserve_every_cell_and_nonzero_bounds() {
    // 包含公司實測的1×5、100×5、1×9、100×9，以及支援ConversationID時的10欄。
    for width in [5, 9, 10] {
        for count in [1, 2, 37, 100] {
            for lower in [[0, 0], [-3, 7]] {
                let input = numbered_cells(count, width, lower);
                let rows = cells(&input, width as usize).unwrap();
                assert_eq!(rows.len(), count as usize);
                for (row_index, row) in rows.iter().enumerate() {
                    assert_eq!(row.len(), width as usize);
                    for (column, cell) in row.iter().enumerate() {
                        assert_eq!(
                            i32::try_from(cell).unwrap(),
                            (row_index * 1000 + column) as i32
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn square_batches_do_not_silently_transpose_headers() {
    for width in [5, 9, 10] {
        let input = numbered_cells(width, width, [0, 0]);
        let rows = cells(&input, width as usize).unwrap();
        // 正方形陣列即使維度判反也不會報錯，必須核對非對角的真實內容。
        assert_eq!(i32::try_from(&rows[1][2]).unwrap(), 1002);
        assert_eq!(i32::try_from(&rows[2][1]).unwrap(), 2001);
    }
}

#[test]
fn empty_batches_keep_supported_native_representations() {
    assert!(cells(&VARIANT::default(), 9).unwrap().is_empty());
    assert!(cells(&native_array(&[0], &[0]), 9).unwrap().is_empty());
    assert!(cells(&native_array(&[0, 9], &[0, 0]), 9)
        .unwrap()
        .is_empty());
    // 空列不能掩蓋錯誤欄數。
    assert!(cells(&native_array(&[0, 8], &[0, 0]), 9).is_err());
}

#[test]
fn wrong_width_and_oversized_batches_report_actual_dimensions() {
    for (rows, columns) in [(100, 8), (100, 10), (101, 9), (9, 100)] {
        let error = cells(&native_array(&[rows, columns], &[0, 0]), 9)
            .expect_err("不可默默丟棄欄位、轉置或截短超量批次");
        assert!(
            error.contains(&format!("實際{rows}列×{columns}欄")),
            "{error}"
        );
        assert!(error.contains("預期9欄、最多100列"), "{error}");
    }
}

#[test]
fn unexpected_variant_or_array_rank_is_not_empty_mail() {
    assert!(cells(&VARIANT::from(7i32), 9).is_err());
    for lengths in [vec![9], vec![1, 9, 1]] {
        let input = native_array(&lengths, &vec![0; lengths.len()]);
        let error = cells(&input, 9).err().unwrap();
        assert!(
            error.contains(&format!("實際{}維", lengths.len())),
            "{error}"
        );
    }
}

#[test]
fn identity_preserves_subject_and_compares_whole_seconds() {
    let date = NaiveDate::from_ymd_opt(2026, 9, 30)
        .unwrap()
        .and_hms_milli_opt(23, 59, 59, 100)
        .unwrap();
    assert_eq!(
        key("主題", date),
        key("主題", date + chrono::Duration::milliseconds(700))
    );
    assert_ne!(key("主題", date), key("RE: 主題", date));
    assert_ne!(
        key("主題", date),
        key("主題", date + chrono::Duration::seconds(1))
    );
}
