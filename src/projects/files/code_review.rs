//! 需求與目前工作副本的核對入口。完整測試程式／結果沿用操作簿保存。
use super::*;
use crate::projects::code_review::{Check, Requirement, Test};

impl Broker {
    pub(super) fn plan_code_change(
        &mut self,
        id: &str,
        requirements: &[Requirement],
    ) -> AppResult<Value> {
        let copy = self.copies.get_mut(id).ok_or("工作副本不存在。")?;
        if extension(Path::new(&copy.name))? != "py" {
            return Err("程式需求核對目前只支援PY副本。".into());
        }
        copy.code_review.plan(requirements, &copy.text)?;
        copy.saved_revision = None;
        for path in &copy.paths {
            self.published.retain(|p| p != path);
        }
        copy.paths.clear();
        Ok(
            json!({"copy_id":id,"requirements":requirements,"notice":"先依需求分段修改；每段保存同一草稿。完成後檢查差異、測試並核對需求。"}),
        )
    }

    pub(super) fn review_code_change(
        &mut self,
        id: &str,
        revision: &str,
        checks: &[Check],
    ) -> AppResult<Value> {
        let copy = self.copies.get_mut(id).ok_or("工作副本不存在。")?;
        if extension(Path::new(&copy.name))? != "py" || text::revision(&copy.text) != revision {
            return Err("PY副本版本不符。".into());
        }
        if checks.is_empty() {
            if copy.code_review.requirements.is_empty() {
                return Err("請先用plan_code_change保存需求清單。".into());
            }
            let old: Vec<_> = copy
                .code_review
                .baseline
                .as_deref()
                .unwrap_or("")
                .lines()
                .collect();
            let new: Vec<_> = copy.text.lines().collect();
            let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
            let suffix = old[prefix..]
                .iter()
                .rev()
                .zip(new[prefix..].iter().rev())
                .take_while(|(a, b)| a == b)
                .count();
            let excerpt = |lines: &[&str]| {
                lines
                    .iter()
                    .take(35)
                    .copied()
                    .collect::<Vec<_>>()
                    .join("\n")
                    .chars()
                    .take(3000)
                    .collect::<String>()
            };
            copy.code_review.inspected_revision = Some(revision.into());
            return Ok(
                json!({"copy_id":id,"revision":revision,"requirements":copy.code_review.requirements,
                "changed_first_line":prefix+1,"old_last_line":old.len()-suffix,"new_last_line":new.len()-suffix,
                "before":excerpt(&old[prefix..old.len()-suffix]),"after":excerpt(&new[prefix..new.len()-suffix]),
                "excerpt_only":true,"total_lines":new.len(),"tests":copy.code_review.tests,
                "guidance":"差異摘要可能截斷；用read_code_section核對相關函式與呼叫處。執行test_python後，逐項提交checks；無法測試要明示理由，不能以語法通過代替。"}),
            );
        }
        copy.code_review
            .review(revision, copy.text.lines().count(), checks)?;
        Ok(
            json!({"copy_id":id,"revision":revision,"verification":copy.code_review.summary(revision)}),
        )
    }

    pub(super) fn test_python(
        &mut self,
        id: &str,
        revision: &str,
        tests: &[Test],
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        let copy = self.copies.get_mut(id).ok_or("工作副本不存在。")?;
        if extension(Path::new(&copy.name))? != "py"
            || text::revision(&copy.text) != revision
            || copy.python_checked_revision.as_deref() != Some(revision)
        {
            return Err("先用check_python確認目前PY副本語法，再執行功能測試。".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        if tests.is_empty()
            || tests.len() > 4
            || tests.iter().any(|test| {
                !crate::projects::code_review::valid_id(&test.id)
                    || !ids.insert(&test.id)
                    || test.code.trim().is_empty()
                    || test.code.len() > 12000
                    || test.requirement_ids.is_empty()
                    || test.requirement_ids.len() > 16
                    || test.mocked_dependencies.len() > 12
                    || test.mocked_dependencies.iter().any(|d| d.len() > 200)
                    || test
                        .requirement_ids
                        .iter()
                        .any(|id| !copy.code_review.requirements.iter().any(|r| &r.id == id))
            })
        {
            return Err("測試需1–4組、唯一id、每組12000 bytes內，並對應已保存需求；以unittest.TestCase及load_target()測試真正副本。".into());
        }
        let mut retained_ids: std::collections::BTreeSet<String> = copy
            .code_review
            .tests
            .iter()
            .filter_map(|t| t["id"].as_str().map(str::to_owned))
            .collect();
        retained_ids.extend(tests.iter().map(|t| t.id.clone()));
        if retained_ids.len() > 24 {
            return Err("目前副本最多24組測試；請沿用原測試id修正重跑，不以新id堆疊。".into());
        }
        copy.saved_revision = None;
        for path in &copy.paths {
            self.published.retain(|p| p != path);
        }
        copy.paths.clear();
        copy.code_review.reviewed_revision = None;
        copy.code_review.checks.clear();
        let input = json!([{"name":"source","kind":"text","text":copy.text,"tests":tests}]);
        let response = super::super::python::execute_tests(
            include_str!("../python/test_source.py"),
            input,
            cancel,
        );
        let summary = match response {
            Ok(value) => value["summary"].clone(),
            Err(error) => {
                json!({"functional_tests_run":false,"tests":tests.iter().map(|t|json!({"id":t.id,"requirement_ids":t.requirement_ids,"mocked_dependencies":t.mocked_dependencies,"status":"unavailable","failures":[error]})).collect::<Vec<_>>(),"notice":"測試程序未完成；保留草稿，可按原因修正或明示未驗證。"})
            }
        };
        let rows = summary["tests"].as_array().ok_or("測試結果缺少案例。")?;
        if rows.len() != tests.len()
            || rows.iter().zip(tests).any(|(r, t)| {
                r["id"] != t.id
                    || r["requirement_ids"] != json!(t.requirement_ids)
                    || r["mocked_dependencies"] != json!(t.mocked_dependencies)
                    || (r["status"] == "passed"
                        && (summary["functional_tests_run"] != true
                            || r["tests_run"].as_u64().unwrap_or(0) == 0
                            || r["target_loads"].as_u64().unwrap_or(0) == 0))
                    || !matches!(
                        r["status"].as_str(),
                        Some("passed" | "failed" | "unavailable")
                    )
            })
        {
            return Err("測試結果與已提交案例不符。".into());
        }
        if summary["functional_tests_run"] == true && summary["source_sha256"] != revision {
            return Err("測試的來源版本不符。".into());
        }
        if copy.code_review.tested_revision.as_deref() != Some(revision) {
            copy.code_review.tests.clear();
        }
        for row in rows {
            copy.code_review
                .tests
                .retain(|previous| previous["id"] != row["id"]);
            copy.code_review.tests.push(row.clone());
        }
        copy.code_review.tested_revision = Some(revision.into());
        Ok(json!({"copy_id":id,"revision":revision,"test_report":summary}))
    }

    /// 原生附加驗證界線；即使模型最終文字漏寫，也不隱藏未驗證項目。
    pub(crate) fn code_delivery_report(&self, paths: &[String]) -> String {
        let mut lines = Vec::new();
        for copy in self.copies.values().filter(|c| {
            c.paths.iter().any(|p| paths.contains(p))
                && c.name.to_ascii_lowercase().ends_with(".py")
        }) {
            lines.push(format!(
                "{}：語法已檢查；以下為需求核對，功能範圍以實際案例為準。",
                copy.name
            ));
            for check in &copy.code_review.checks {
                let status = match check.status.as_str() {
                    "tested" => "所列測試通過",
                    "reviewed" => "僅程式核對",
                    "unverified" => "未驗證",
                    _ => "未完成",
                };
                lines.push(format!(
                    "- {}：{}。{}",
                    check.requirement_id, status, check.evidence
                ));
            }
            for test in copy
                .code_review
                .tests
                .iter()
                .filter(|t| t["status"] != "passed")
            {
                lines.push(format!(
                    "- 測試 {}：{}，不可視為功能通過。",
                    test["id"].as_str().unwrap_or("?"),
                    test["status"].as_str().unwrap_or("未確認")
                ));
            }
            let mocks: std::collections::BTreeSet<_> = copy
                .code_review
                .tests
                .iter()
                .flat_map(|t| t["mocked_dependencies"].as_array().into_iter().flatten())
                .filter_map(Value::as_str)
                .collect();
            if !mocks.is_empty() {
                lines.push(format!(
                    "模擬依賴：{}；不代表真實環境已驗證。",
                    mocks.into_iter().collect::<Vec<_>>().join("、")
                ));
            }
        }
        if lines.is_empty() {
            String::new()
        } else {
            format!("\n\n程式驗證紀錄：\n{}", lines.join("\n"))
        }
    }
}
