//! 英文代號依字母、數字段依數值比較，避免 A10 排在 A2 前面。
use std::cmp::Ordering;

pub fn group_cmp(a: &str, b: &str) -> Ordering {
    (a == "未分類")
        .cmp(&(b == "未分類"))
        .then_with(|| natural_cmp(a, b))
}

/// 逐段比較 ASCII 數字，使用字串長度而非整數轉換，長代號也不會溢位。
/// 同一位置的數字排在字母前；大小寫不影響主要順序，最後以原字串穩定排序。
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let left = a.to_ascii_uppercase();
    let right = b.to_ascii_uppercase();
    let (mut x, mut y) = (left.as_bytes(), right.as_bytes());
    while !x.is_empty() && !y.is_empty() {
        let order;
        if x[0].is_ascii_digit() && y[0].is_ascii_digit() {
            let nx = x.iter().take_while(|v| v.is_ascii_digit()).count();
            let ny = y.iter().take_while(|v| v.is_ascii_digit()).count();
            let ax = &x[..nx];
            let by = &y[..ny];
            let ax = &ax[ax.iter().take_while(|v| **v == b'0').count()..];
            let by = &by[by.iter().take_while(|v| **v == b'0').count()..];
            order = ax.len().cmp(&by.len()).then_with(|| ax.cmp(by));
            x = &x[nx..];
            y = &y[ny..];
        } else {
            order = x[0].cmp(&y[0]);
            x = &x[1..];
            y = &y[1..];
        }
        if order != Ordering::Equal {
            return order;
        }
    }
    x.len().cmp(&y.len()).then_with(|| a.cmp(b))
}
