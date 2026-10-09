//! 可跨語言沿用的需求／證據資料。程式執行目前只接 Python，模型核對不冒充測試。
use crate::AppResult;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Requirement {
    pub id: String,
    pub description: String,
    pub origin: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Test {
    pub id: String,
    pub requirement_ids: Vec<String>,
    pub code: String,
    pub mocked_dependencies: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Check {
    pub requirement_id: String,
    pub status: String,
    pub evidence: String,
    pub first_line: usize,
    pub last_line: usize,
    pub test_ids: Vec<String>,
}
#[derive(Default, Serialize, Deserialize)]
pub struct State {
    pub requirements: Vec<Requirement>,
    pub baseline: Option<String>,
    pub inspected_revision: Option<String>,
    pub reviewed_revision: Option<String>,
    pub checks: Vec<Check>,
    pub tested_revision: Option<String>,
    pub tests: Vec<Value>,
}
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 40
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}
impl State {
    pub fn plan(&mut self, requirements: &[Requirement], source: &str) -> AppResult<()> {
        let mut ids = BTreeSet::new();
        if requirements.is_empty()
            || requirements.len() > 16
            || requirements.iter().any(|r| {
                !valid_id(&r.id)
                    || !ids.insert(&r.id)
                    || r.description.trim().is_empty()
                    || r.description.chars().count() > 500
                    || !matches!(r.origin.as_str(), "user" | "preserve" | "added")
            })
        {
            return Err(
                "需求清單需1–16項、唯一英數id、500字內說明及user/preserve/added來源。".into(),
            );
        }
        self.baseline.get_or_insert_with(|| source.into());
        self.requirements = requirements.to_vec();
        self.invalidate();
        Ok(())
    }
    pub fn invalidate(&mut self) {
        self.inspected_revision = None;
        self.reviewed_revision = None;
        self.checks.clear();
        self.tested_revision = None;
        self.tests.clear();
    }
    pub fn review(&mut self, revision: &str, line_count: usize, checks: &[Check]) -> AppResult<()> {
        if self.requirements.is_empty() || self.inspected_revision.as_deref() != Some(revision) {
            return Err(
                "先建立需求清單，再用review_code_change的空checks檢查目前副本差異及需求。".into(),
            );
        }
        let mut ids = BTreeSet::new();
        if checks.len() != self.requirements.len() {
            return Err("核對須涵蓋每一項需求。".into());
        }
        for check in checks {
            if !ids.insert(&check.requirement_id)
                || !self
                    .requirements
                    .iter()
                    .any(|r| r.id == check.requirement_id)
                || !matches!(
                    check.status.as_str(),
                    "tested" | "reviewed" | "unverified" | "incomplete"
                )
                || check.evidence.trim().is_empty()
                || check.evidence.chars().count() > 600
                || check.first_line == 0
                || check.last_line < check.first_line
                || check.last_line > line_count.max(1)
                || check.test_ids.len() > 8
            {
                return Err("核對項目、狀態、證據或目前副本行號無效。".into());
            }
            if check.status == "tested" {
                if self.tested_revision.as_deref() != Some(revision) || check.test_ids.is_empty() {
                    return Err("tested必須引用目前revision的實際測試，不能以模型說明代替。".into());
                }
                for id in &check.test_ids {
                    if !self.tests.iter().any(|t| {
                        t["id"] == *id
                            && t["status"] == "passed"
                            && t["requirement_ids"]
                                .as_array()
                                .is_some_and(|rs| rs.iter().any(|r| r == &check.requirement_id))
                    }) {
                        return Err("引用的測試未通過，或沒有對應這項需求。".into());
                    }
                }
            }
            if self.tested_revision.as_deref() == Some(revision)
                && self.tests.iter().any(|t| {
                    t["status"] == "failed"
                        && t["requirement_ids"]
                            .as_array()
                            .is_some_and(|rs| rs.iter().any(|r| r == &check.requirement_id))
                })
                && check.status != "incomplete"
            {
                return Err("此需求仍有失敗測試；修正後重測，或標示incomplete保留草稿。".into());
            }
        }
        self.reviewed_revision = Some(revision.into());
        self.checks = checks.to_vec();
        Ok(())
    }
    pub fn ready(&self, revision: &str) -> AppResult<()> {
        if self.reviewed_revision.as_deref() != Some(revision) {
            return Err("PY副本需先依需求核對目前版本；使用review_code_change檢查差異，再提交各項核對結果。".into());
        }
        if self.checks.iter().any(|c| c.status == "incomplete") {
            return Err("仍有未完成需求，已保留同一路徑草稿；請修正，不可發布為完成成果。".into());
        }
        Ok(())
    }
    pub fn summary(&self, revision: &str) -> Value {
        json!({"requirements":self.requirements.iter().map(|r|json!({"id":r.id,"description":r.description,"origin":r.origin})).collect::<Vec<_>>(),
            "review_current":self.reviewed_revision.as_deref()==Some(revision),
            "checks":self.checks,"tests_current":self.tested_revision.as_deref()==Some(revision),"tests":self.tests,
            "notice":"需求與程式核對由模型提出；tested僅代表所列測試通過，模擬依賴與未驗證項目仍有界線。"})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn evidence_must_match_revision_requirement_and_real_test() {
        let mut state = State::default();
        state
            .plan(
                &[Requirement {
                    id: "R1".into(),
                    description: "保留行為".into(),
                    origin: "user".into(),
                }],
                "pass",
            )
            .unwrap();
        state.inspected_revision = Some("v1".into());
        let mut check = Check {
            requirement_id: "R1".into(),
            status: "tested".into(),
            evidence: "結果".into(),
            first_line: 1,
            last_line: 1,
            test_ids: vec!["T1".into()],
        };
        assert!(state.review("v1", 1, &[check.clone()]).is_err());
        state.tested_revision = Some("v1".into());
        state.tests = vec![json!({"id":"T1","status":"passed","requirement_ids":["R1"]})];
        state.review("v1", 1, &[check.clone()]).unwrap();
        assert!(state.ready("v1").is_ok());
        assert!(state.ready("v2").is_err());
        let restored: State =
            serde_json::from_value(serde_json::to_value(&state).unwrap()).unwrap();
        assert!(restored.ready("v1").is_ok());
        assert!(restored.ready("v2").is_err());
        state.tests[0]["status"] = json!("failed");
        check.status = "reviewed".into();
        assert!(state.review("v1", 1, &[check]).is_err());
        state.invalidate();
        assert!(state.ready("v1").is_err());
        assert_eq!(state.requirements.len(), 1);
    }
}
