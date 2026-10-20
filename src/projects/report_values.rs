//! 報告數值只取成功計算的JSON Pointer。模型指定欄位與單位，不能手填數字。
//! 數值一致只表示引用同一計算結果，公式與資料選取仍須另外驗證。
use crate::AppResult;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub id: String,
    pub operation_id: String,
    pub pointer: String,
    pub decimals: usize,
    pub unit: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Metric {
    pub spec: Spec,
    pub value: Value,
    pub display: String,
    pub sources: Value,
    pub code_sha256: String,
}
impl Metric {
    pub fn from_result(spec: &Spec, request: &Value, result: &Value) -> AppResult<Self> {
        crate::jobs::validate_id(&spec.id)?;
        if request["tool"] != "run_python"
            || result["ok"] != true
            || !spec.pointer.starts_with("/summary/")
            || spec.pointer.len() > 400
            || spec.decimals > 10
            || spec.unit.chars().count() > 40
            || spec.unit.chars().any(char::is_control)
        {
            return Err(
                "數值需引用成功run_python的/summary/...；小數0–10位，單位最多40字。".into(),
            );
        }
        let value = result["result"]
            .pointer(&spec.pointer)
            .ok_or("找不到計算結果欄位。")?
            .clone();
        let number = value
            .as_f64()
            .filter(|n| n.is_finite())
            .ok_or("綁定值必須為有限數字，缺失值需先在計算中明確處理。")?;
        if (value.is_i64() || value.is_u64()) && number.abs() > 9_007_199_254_740_991.0 {
            return Err(
                "整數超出精確浮點顯示範圍，請改用明確字串／高精度報告流程，未四捨五入冒充原值。"
                    .into(),
            );
        }
        let display = format!("{:.*}", spec.decimals, number);
        Ok(Self {
            spec: spec.clone(),
            value,
            display,
            sources: result["result"]["sources"].clone(),
            code_sha256: result["result"]["code_sha256"]
                .as_str()
                .ok_or("計算缺少程式版本。")?
                .into(),
        })
    }
    pub fn text(&self) -> String {
        if self.spec.unit.is_empty() {
            self.display.clone()
        } else {
            format!("{} {}", self.display, self.spec.unit)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub metric_id: String,
    pub block_id: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct BoundBlock {
    pub copy_id: String,
    pub block_id: String,
    pub metric_id: String,
    pub expected: String,
    pub numeric: bool,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct State {
    pub metrics: BTreeMap<String, Metric>,
    pub blocks: Vec<BoundBlock>,
    pub datasets: Vec<Value>,
}
impl State {
    /// 模型只需要可引用數字與ID；來源明細在原生狀態／計算操作保存一次。
    pub fn context(&self) -> Value {
        serde_json::json!({"ids":self.metrics.keys().collect::<Vec<_>>(),"values":self.metrics.values().take(40).map(|m|serde_json::json!({"id":m.spec.id,"value":m.value,"display":m.display,"unit":m.spec.unit,"source_operation":m.spec.operation_id})).collect::<Vec<_>>(),"omitted":self.metrics.len().saturating_sub(40)})
    }
    /// 指定區塊後仍須核對當前快照；結構變動使舊block_id失效時拒絕交付。
    pub fn check_block(
        &self,
        bound: &BoundBlock,
        snapshot: &super::office::Snapshot,
    ) -> AppResult<()> {
        let block = snapshot
            .blocks
            .iter()
            .find(|b| b.id == bound.block_id)
            .ok_or("綁定報告區塊已移除，請重新綁定。")?;
        let metric = self
            .metrics
            .get(&bound.metric_id)
            .ok_or("綁定計算值不存在。")?;
        let valid = if bound.numeric {
            block
                .text
                .parse::<f64>()
                .ok()
                .zip(metric.display.parse::<f64>().ok())
                .is_some_and(|(a, b)| a == b)
        } else {
            block.text == metric.text()
        };
        if !valid {
            return Err(format!(
                "報告數值不一致：{} 的 {} 與計算值 {} 不符。",
                bound.copy_id, bound.block_id, bound.metric_id
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn binding_uses_calculation_and_rejects_forged_or_missing_values() {
        let mut spec = Spec {
            id: "mean".into(),
            operation_id: "calc".into(),
            pointer: "/summary/mean".into(),
            decimals: 2,
            unit: "mm".into(),
        };
        let result =
            json!({"ok":true,"result":{"summary":{"mean":12.345},"code_sha256":"a","sources":[]}});
        assert_eq!(
            Metric::from_result(&spec, &json!({"tool":"run_python"}), &result)
                .unwrap()
                .text(),
            "12.35 mm"
        );
        assert!(Metric::from_result(&spec, &json!({"tool":"read_file"}), &result).is_err());
        spec.pointer = "/summary/missing".into();
        assert!(Metric::from_result(&spec, &json!({"tool":"run_python"}), &result).is_err());
    }
}
