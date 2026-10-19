//! 分段計畫只存資料。每段具識別碼與版本，最後才以同一份原生計畫建立一張DWG。
//! checkpoint保存完整參數；模型每輪只看有界目錄，按需取回單段，不靠摘要重建幾何。
use super::*;
use serde::{Deserialize, Serialize};

const PART_OPERATIONS: usize = 20;
const TOTAL_OPERATIONS: usize = 400;
const PLAN_BYTES: usize = 500_000;
const RECENT_PARTS: usize = 4;
const RECENT_DETAIL_BYTES: usize = 32_000;

/// 完整原生報告已由draw_cad寫在專案；模型只收交付事實與有界警告。
/// 不將數百圖元的前後快照再塞進最後一輪，也不隱藏量測不可用或稽核警告。
fn completion_summary(result: &Value, report_path: Value) -> Value {
    let warnings = result["audit_errors"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    json!({"status":result["status"],"path":result["path"],"sha256":result["sha256"],
        "saved":result["saved"],"file_exists":result["file_exists"],"drawing_started":result["drawing_started"],
        "completed_operations":result["completed_operations"],"units":result["units"],
        "cad_progid":result["selected"]["progid"],
        "after_reopen":{"entity_count":result["after_reopen"]["entity_count"],
            "geometry_and_colors_verified":result["after_reopen"]["geometry_and_colors_verified"],
            "dimension_measurement_unavailable_count":result["after_reopen"]["dimension_measurement_unavailable"].as_array().map_or(0,Vec::len)},
        "audit_errors_total":warnings.len(),"audit_errors":warnings.iter().take(8).map(|v|v.as_str().unwrap_or("原生稽核警告，詳完整報告").chars().take(400).collect::<String>()).collect::<Vec<_>>(),
        "report_path":report_path,"visual_check_required":true,
        "instructions":"同一張DWG已由程式交付。依實際存檔重開與警告簡短finish，artifacts=[]；外觀、字型及工程條件尚須人工核對。完整原生報告留在report_path，不需重讀全部幾何才回覆。"})
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Plan {
    id: String,
    name: String,
    version: String,
    revision: u64,
    phase: String,
    parts: Vec<Part>,
    /// 工作集只存ID，投影時取最新參數，替換區段不會殘留舊座標。
    #[serde(default)]
    recent: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Part {
    id: String,
    summary: String,
    operations: Vec<Value>,
}

impl Part {
    /// 原生參數會移除可選null；查回時還原同一預設值的傳輸形狀，
    /// 避免模型照抄已保存線段，卻因公開Schema要求line_style而再次被拒絕。
    fn model_value(&self) -> Value {
        let mut value = json!(self);
        for operation in value["operations"].as_array_mut().unwrap() {
            if matches!(
                operation["kind"].as_str(),
                Some("line" | "circle" | "curve")
            ) && operation.get("line_style").is_none()
            {
                operation["line_style"] = Value::Null;
            }
        }
        value
    }
}

impl Plan {
    pub(super) fn finished(&self) -> bool {
        self.phase != "collecting"
    }

    pub(super) fn context(&self) -> Value {
        let mut details = Vec::new();
        let mut omitted = Vec::new();
        let mut bytes = 0;
        for id in self.recent.iter().rev() {
            if let Some(part) = self.parts.iter().find(|p| &p.id == id) {
                let value = part.model_value();
                let size = value.to_string().len();
                if bytes + size <= RECENT_DETAIL_BYTES {
                    bytes += size;
                    details.push(value);
                } else {
                    omitted.push(id);
                }
            }
        }
        json!({"plan_id":self.id,"name":self.name,"version":self.version,
            "plan_revision":self.revision,"phase":self.phase,
            "operation_count":self.parts.iter().map(|p|p.operations.len()).sum::<usize>(),
            "limits":{"per_part":PART_OPERATIONS,"total":TOTAL_OPERATIONS,"parts":40},
            "parts":self.parts.iter().map(|p|json!({"part_id":p.id,"summary":p.summary,"operations":p.operations.len()})).collect::<Vec<_>>(),
            "recent_details":details,"recent_details_omitted":omitted,
            "cad_execution_submitted":self.phase != "collecting",
            "next_action":"recent_details已含最近至多4段的完整精確參數，直接合併參考牆線與家具，不要在已列出的區段間反覆inspect。只在缺少所需段時inspect；超過32KB未列入的ID在recent_details_omitted，原資料未刪除。每輪保存下一段1–20項，全部齊備才commit；暫存不代表已繪圖。"})
    }

    fn remember(&mut self, id: &str) {
        self.recent.retain(|old| old != id);
        self.recent.push(id.into());
        if self.recent.len() > RECENT_PARTS {
            self.recent.remove(0);
        }
    }

    /// 已完整常駐的段不必再查；未列入工作集的舊段仍可按需取回。
    pub(super) fn inspectable_parts(&self) -> Vec<String> {
        let context = self.context();
        self.parts
            .iter()
            .filter(|p| {
                !context["recent_details"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["id"] == p.id)
            })
            .map(|p| p.id.clone())
            .collect()
    }

    fn check(&self, id: &str, revision: u64) -> AppResult<()> {
        if id != self.id {
            return Err("不是目前任務的CAD計畫；請沿用常駐狀態的plan_id。".into());
        }
        if self.finished() {
            return Err("此計畫已送出或完成，不能重新繪圖；依最後原生結果處理。".into());
        }
        if revision != self.revision {
            return Err(format!(
                "CAD計畫版本已變更，目前revision={}；已保存內容未變，請依常駐目錄續接。",
                self.revision
            ));
        }
        Ok(())
    }

    fn put(
        &mut self,
        id: &str,
        revision: u64,
        part_id: &str,
        summary: &str,
        operations: &[Value],
    ) -> AppResult<Value> {
        // 不同operation_id重傳同一段也不能重複累加；過期版本僅容許完全相同的資料查回。
        if id == self.id
            && !self.finished()
            && self
                .parts
                .iter()
                .any(|p| p.id == part_id && p.summary == summary && p.operations == operations)
        {
            self.remember(part_id);
            return Ok(json!({"plan":self.context(),"unchanged":true,"executed":false}));
        }
        self.check(id, revision)?;
        if part_id.is_empty()
            || part_id.len() > 40
            || !part_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err("part_id需1–40個英數字、底線或連字號，例如floor1_walls。".into());
        }
        if summary.trim().is_empty() || summary.chars().count() > 160 {
            return Err("區段summary需1–160字，記視圖、構件、座標範圍及下一段接點。".into());
        }
        if operations.is_empty() || operations.len() > PART_OPERATIONS {
            return Err(
                "每段只接受1–20項operations；先提交本段，其餘留下一輪。原計畫未改、尚未繪圖。"
                    .into(),
            );
        }
        super::super::agent::validate_cad_operations(operations)?;
        let mut candidate = self.clone();
        let part = Part {
            id: part_id.into(),
            summary: summary.into(),
            operations: operations.to_vec(),
        };
        if let Some(old) = candidate.parts.iter_mut().find(|p| p.id == part_id) {
            *old = part;
        } else {
            candidate.parts.push(part);
        }
        candidate.revision += 1;
        candidate.remember(part_id);
        if candidate.parts.len() > 40
            || candidate
                .parts
                .iter()
                .map(|p| p.operations.len())
                .sum::<usize>()
                > TOTAL_OPERATIONS
            || serde_json::to_vec(&candidate)
                .map_err(|e| e.to_string())?
                .len()
                > PLAN_BYTES
        {
            return Err(
                "本圖分段計畫超過40段、400項或500KB上限；已保存內容未改，不可刪減要求後冒稱完成。"
                    .into(),
            );
        }
        *self = candidate;
        Ok(
            json!({"plan":self.context(),"saved_part":part_id,"drawing_started":false,"saved_dwg":false}),
        )
    }
}

impl Broker {
    pub(super) fn start_cad_plan(&mut self, name: &str, version: &str) -> AppResult<Value> {
        if self.cad_failed || self.cad_design.pending() {
            return Err("先完成新圖短方案，或處理上次原生失敗；尚未建立分段計畫或圖面。".into());
        }
        let name = if Path::new(name).extension().is_none() {
            format!("{name}.dwg")
        } else {
            name.into()
        };
        let rel = relative(&name)?;
        if rel.components().count() != 1
            || !name.to_ascii_lowercase().ends_with(".dwg")
            || version.is_empty()
            || version.len() > 100
            || version.eq_ignore_ascii_case("latest")
        {
            return Err("計畫需單一DWG檔名及已確認的確切CAD ProgID，不接受latest或目錄。".into());
        }
        if let Some(plan) = self.cad_design.plan.as_ref().filter(|p| !p.finished()) {
            if plan.name == name && plan.version == version {
                return Ok(json!({"plan":plan.context(),"unchanged":true,"executed":false}));
            }
            return Err(
                "已有未完成計畫；不能用新名稱或版本清掉已累積內容。請沿用目前plan_id。".into(),
            );
        }
        self.cad_design.next_plan += 1;
        let plan = Plan {
            id: format!("plan-{}", self.cad_design.next_plan),
            name,
            version: version.into(),
            revision: 0,
            phase: "collecting".into(),
            parts: vec![],
            recent: vec![],
        };
        let result = json!({"plan":plan.context(),"drawing_started":false});
        self.cad_design.plan = Some(plan);
        Ok(result)
    }

    pub(super) fn put_cad_plan_part(
        &mut self,
        id: &str,
        revision: u64,
        part: &str,
        summary: &str,
        operations: &[Value],
    ) -> AppResult<Value> {
        if self.cad_failed {
            return Err("CAD上次結果未確認，不能修改或重送計畫。".into());
        }
        self.cad_design
            .plan
            .as_mut()
            .ok_or("先cad_plan_start建立計畫。")?
            .put(id, revision, part, summary, operations)
    }

    pub(super) fn inspect_cad_plan(&mut self, id: &str, part: Option<&str>) -> AppResult<Value> {
        let plan = self
            .cad_design
            .plan
            .as_mut()
            .filter(|p| p.id == id)
            .ok_or("不是目前任務的CAD計畫。")?;
        let detail = part
            .map(|id| {
                plan.parts
                    .iter()
                    .find(|p| p.id == id)
                    .cloned()
                    .ok_or("未知part_id；已保存目錄列在常駐狀態。")
            })
            .transpose()?;
        if let Some(id) = part {
            plan.remember(id);
        }
        Ok(json!({"plan":plan.context(),"part":detail.as_ref().map(Part::model_value)}))
    }

    pub(super) fn commit_cad_plan(
        &mut self,
        id: &str,
        revision: u64,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("已取消，分段計畫保留但未送CAD。".into());
        }
        let plan = self.cad_design.plan.as_mut().ok_or("先建立分段計畫。")?;
        plan.check(id, revision)?;
        let operations: Vec<_> = plan
            .parts
            .iter()
            .flat_map(|p| p.operations.iter().cloned())
            .collect();
        if operations.is_empty() {
            return Err("計畫尚無區段，不能繪圖。".into());
        }
        // 原生指令統一移到所有幾何之後，保留各自順序；不改寫任何模型座標。
        let (mut geometry, commands): (Vec<_>, Vec<_>) = operations
            .into_iter()
            .partition(|op| op["kind"] != "command");
        geometry.extend(commands);
        let (name, version) = (plan.name.clone(), plan.version.clone());
        plan.phase = "submitted".into();
        let result = self.draw_cad_with_limit(&name, &version, &geometry, TOTAL_OPERATIONS, cancel);
        if let Some(plan) = self.cad_design.plan.as_mut() {
            plan.phase = if result.is_ok() {
                "completed"
            } else if !self.cad_failed
                && self
                    .cad_last_result
                    .as_ref()
                    .is_some_and(|r| r["drawing_started"] == false)
            {
                "collecting"
            } else {
                "blocked"
            }
            .into();
        }
        result.map(|value| {
            completion_summary(
                &value,
                self.cad_last_result
                    .as_ref()
                    .map_or(Value::Null, |r| r["report_path"].clone()),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn plan() -> Plan {
        Plan {
            id: "plan-1".into(),
            name: "test.dwg".into(),
            version: "Icad.Application.1".into(),
            revision: 0,
            phase: "collecting".into(),
            parts: vec![],
            recent: vec![],
        }
    }
    fn line(x: i32) -> Value {
        json!({"kind":"line","color":7,"start":[x,0],"end":[x,10]})
    }

    #[test]
    fn parts_survive_checkpoint_replace_in_place_and_deduplicate() {
        let mut p = plan();
        p.put("plan-1", 0, "walls", "一樓牆，x0–10", &[line(0)])
            .unwrap();
        let mut p: Plan = serde_json::from_value(serde_json::to_value(p).unwrap()).unwrap();
        p.put("plan-1", 1, "doors", "一樓門，x10", &[line(10)])
            .unwrap();
        assert_eq!(
            p.put("plan-1", 0, "walls", "一樓牆，x0–10", &[line(0)])
                .unwrap()["unchanged"],
            true
        );
        assert!(p.put("plan-1", 0, "walls", "修正牆", &[line(1)]).is_err());
        p.put("plan-1", 2, "walls", "修正牆", &[line(1)]).unwrap();
        assert_eq!(p.parts.len(), 2);
        assert_eq!(p.parts[0].operations, vec![line(1)]);
        assert_eq!(p.parts[1].operations, vec![line(10)]);
        assert!(p.context().get("operations").is_none());
    }

    #[test]
    fn bad_or_oversized_parts_are_atomic_and_finished_plans_cannot_replay() {
        let mut p = plan();
        assert!(p.put("wrong", 0, "x", "牆", &[line(0)]).is_err());
        assert!(p.put("plan-1", 0, "x", "牆", &vec![line(0); 21]).is_err());
        assert!(p
            .put("plan-1", 0, "x", "牆", &[json!({"kind":"line"})])
            .is_err());
        assert!(p.parts.is_empty());
        for i in 0..20 {
            p.put(
                "plan-1",
                i,
                &format!("p{i}"),
                "段",
                &vec![line(i as i32); 20],
            )
            .unwrap();
        }
        assert!(p.put("plan-1", 20, "overflow", "段", &[line(0)]).is_err());
        assert_eq!(p.revision, 20);
        p.phase = "completed".into();
        assert!(p.put("plan-1", 20, "x", "段", &[line(0)]).is_err());
        assert!(p.check("plan-1", 20).is_err());
    }

    #[test]
    fn native_completion_is_bounded_without_losing_warnings_or_delivery_evidence() {
        let result = json!({"status":"DATA_VERIFIED_VISUAL_PENDING","saved":true,"path":"_AI_Output/x/a.dwg",
            "sha256":"hash","completed_operations":400,"audit_errors":vec!["warning".repeat(500);20],
            "after_reopen":{"entity_count":400,"geometry_and_colors_verified":true,"dimension_measurement_unavailable":["A"]},
            "operation_entities":vec![json!({"private_geometry":"full detail"});400]});
        let summary = completion_summary(&result, json!("_AI_Output/x/cad-report.json"));
        assert_eq!(summary["saved"], true);
        assert_eq!(summary["after_reopen"]["entity_count"], 400);
        assert_eq!(
            summary["after_reopen"]["dimension_measurement_unavailable_count"],
            1
        );
        assert_eq!(summary["audit_errors_total"], 20);
        assert_eq!(summary["audit_errors"].as_array().unwrap().len(), 8);
        assert!(summary.to_string().len() < 6000);
        assert!(summary.get("operation_entities").is_none());
        assert_eq!(summary["report_path"], "_AI_Output/x/cad-report.json");
    }

    #[test]
    fn exact_recent_parts_survive_compaction_checkpoint_and_refresh_after_replacement() {
        let mut p = plan();
        for i in 0..5 {
            p.put("plan-1", i, &format!("p{i}"), "區段", &[line(i as i32)])
                .unwrap();
        }
        assert_eq!(p.context()["recent_details"].as_array().unwrap().len(), 4);
        assert_eq!(p.inspectable_parts(), vec!["p0"]);
        assert!(!p.recent.contains(&"p0".to_owned()));
        p.remember("p0");
        let mut restored: Plan = serde_json::from_value(serde_json::to_value(p).unwrap()).unwrap();
        assert_eq!(restored.context()["recent_details"][0]["id"], "p0");
        restored
            .put("plan-1", 5, "p0", "已改座標", &[line(100)])
            .unwrap();
        assert_eq!(
            restored.context()["recent_details"][0]["operations"][0]["start"],
            json!([100, 0])
        );
        assert_eq!(
            restored.context()["recent_details"][0]["operations"][0].get("line_style"),
            Some(&Value::Null)
        );
        assert!(
            restored.parts[0].operations[0].get("line_style").is_none(),
            "原生計畫仍保留合法的預設形狀"
        );
        assert_eq!(restored.parts.len(), 5, "工作集淘汰不刪除已保存區段");
    }

    #[test]
    fn oversized_recent_detail_is_explicitly_omitted_without_deleting_the_plan() {
        let mut p = plan();
        let text =
            json!({"kind":"text","color":7,"position":[0,0],"text":"字".repeat(1000),"height":3});
        p.put("plan-1", 0, "notes", "文字區", &vec![text; 20])
            .unwrap();
        let context = p.context();
        assert!(context["recent_details"].as_array().unwrap().is_empty());
        assert_eq!(context["recent_details_omitted"], json!(["notes"]));
        assert_eq!(p.parts[0].operations.len(), 20);
    }
}
