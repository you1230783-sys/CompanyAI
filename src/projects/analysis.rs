//! 分析狀態：程式觀察到的處理範圍與模型的語意判斷分開保存。
//! 證據只能引用成功工具的既存結果，核對數字從該結果取回，不接受模型手填「通過」。
use crate::AppResult;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRef {
    pub operation_id: String,
    /// RFC 6901 JSON Pointer；空字串代表整份工具結果。
    pub pointer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    pub claim: String,
    /// confirmed 仍是模型判斷；程式只確認證據存在，不驗證因果關係。
    pub status: String,
    pub evidence: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Check {
    pub label: String,
    pub operation_id: String,
    /// balance 核對 total = sum(parts)；zero 核對 total 是否為零。
    pub kind: String,
    pub total_pointer: String,
    pub part_pointers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Method {
    pub title: String,
    pub applicability: String,
    pub steps: String,
    pub validation: String,
    pub limitations: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub goal: String,
    pub current_step: String,
    pub open_questions: Vec<String>,
    pub superseded: Vec<String>,
    pub findings: Vec<Finding>,
    pub checks: Vec<Check>,
    pub method: Option<Method>,
}

fn bounded(text: &str, max: usize) -> AppResult<()> {
    if text.trim().is_empty() || text.chars().count() > max {
        return Err(format!("分析文字需為 1–{max} 字。"));
    }
    Ok(())
}

impl Report {
    pub fn validate(&self) -> AppResult<()> {
        bounded(&self.goal, 500)?;
        bounded(&self.current_step, 500)?;
        if self.open_questions.len() > 12
            || self.superseded.len() > 12
            || self.findings.len() > 12
            || self.checks.len() > 12
        {
            return Err("每種分析項目最多 12 項。".into());
        }
        for text in self.open_questions.iter().chain(&self.superseded) {
            bounded(text, 500)?;
        }
        for finding in &self.findings {
            bounded(&finding.claim, 500)?;
            if !matches!(
                finding.status.as_str(),
                "confirmed" | "hypothesis" | "rejected"
            ) || finding.evidence.len() > 4
            {
                return Err(
                    "結論狀態只能是 confirmed／hypothesis／rejected，每項最多 4 份證據。".into(),
                );
            }
            if finding.status == "confirmed" && finding.evidence.is_empty() {
                return Err("已確認的結論至少需引用一份實際工具證據。".into());
            }
        }
        if let Some(method) = &self.method {
            bounded(&method.title, 80)?;
            bounded(&method.applicability, 300)?;
            bounded(&method.steps, 700)?;
            bounded(&method.validation, 300)?;
            bounded(&method.limitations, 300)?;
        }
        Ok(())
    }
}

/// 只讀成功工具的 result；權限及操作身分仍由 broker 的紀錄索引核對。
pub fn select(result: &Value, pointer: &str) -> AppResult<Value> {
    if result["ok"] != true || pointer.len() > 400 {
        return Err("只能引用成功工具的結果，JSON Pointer 最多 400 bytes。".into());
    }
    let value = result["result"]
        .pointer(pointer)
        .ok_or("找不到引用的 JSON Pointer；以工具 result 為起點。")?;
    if value.is_null() || value.to_string().len() > 6000 {
        return Err("引用資料為空或超過 6000 bytes；請選擇較小的原文範圍。".into());
    }
    Ok(value.clone())
}

pub fn check(check: &Check, result: &Value) -> AppResult<Value> {
    bounded(&check.label, 100)?;
    let total = select(result, &check.total_pointer)?
        .as_u64()
        .ok_or("核對值必須是非負整數筆數。")?;
    let passed = match check.kind.as_str() {
        "zero" if check.part_pointers.is_empty() => total == 0,
        "balance" if (2..=12).contains(&check.part_pointers.len()) => {
            let mut sum = 0u64;
            let mut unique = std::collections::BTreeSet::new();
            for pointer in &check.part_pointers {
                if pointer == &check.total_pointer || !unique.insert(pointer) {
                    return Err("核對分項不可重複或引用總數本身。".into());
                }
                let value = select(result, pointer)?
                    .as_u64()
                    .ok_or("核對分項必須是非負整數。")?;
                sum = sum.checked_add(value).ok_or("核對筆數溢位。")?;
            }
            sum == total
        }
        _ => return Err("核對方式為 balance（2–12 分項）或 zero（無分項）。".into()),
    };
    Ok(
        json!({"label":check.label,"kind":check.kind,"operation_id":check.operation_id,"total":total,"passed":passed,
        "total_pointer":check.total_pointer,"parts":check.part_pointers.iter().map(|p|json!({"pointer":p,"value":result["result"].pointer(p)})).collect::<Vec<_>>(),
        "scope":"僅核對工具輸出的筆數關係；解析規則、配對方式及因果推論仍需原文與反例驗證。"}),
    )
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Coverage {
    pub path: String,
    pub scope: String,
    pub status: String,
    pub revision: Option<String>,
    pub detail: String,
    #[serde(default)]
    pub ranges: Vec<(usize, usize)>,
    #[serde(default)]
    pub end: Option<usize>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct State {
    /// 每個檔案／查詢分開，不因另一次查詢成功就抹除尚未完成的搜尋。
    pub coverage: BTreeMap<String, Coverage>,
    pub coverage_truncated: bool,
    pub report: Option<Value>,
    pub review_required: bool,
}

impl State {
    /// 合併同版本實際取得的區間；只讀檔尾或重疊分段不能冒充全文完成。
    fn range(
        &mut self,
        path: &str,
        scope: &str,
        revision: &Value,
        start: usize,
        end: usize,
        eof: bool,
    ) {
        let key = format!("{}|{scope}", path.replace('\\', "/").to_lowercase());
        let mut item = self
            .coverage
            .get(&key)
            .filter(|v| v.revision.as_deref() == revision.as_str())
            .cloned()
            .unwrap_or_default();
        item.path = path.into();
        item.scope = scope.into();
        item.revision = revision.as_str().map(str::to_owned);
        if end > start {
            item.ranges.push((start, end));
        }
        item.ranges.sort_unstable();
        let mut merged: Vec<(usize, usize)> = Vec::new();
        for (a, b) in &item.ranges {
            if let Some(last) = merged.last_mut().filter(|last| last.1 >= *a) {
                last.1 = last.1.max(*b);
            } else {
                merged.push((*a, *b));
            }
        }
        if merged.len() > 500 {
            self.coverage_truncated = true;
            merged.truncate(500);
        }
        item.ranges = merged;
        if eof {
            item.end = Some(end);
        }
        item.status = if item.end.is_some_and(|last| {
            last == 0 || item.ranges.first().is_some_and(|r| r.0 == 0 && r.1 == last)
        }) {
            "complete"
        } else {
            "partial"
        }
        .into();
        item.detail = format!(
            "已取得 {} 個不重複單位，{} 個區間；{}。",
            item.ranges.iter().map(|(a, b)| b - a).sum::<usize>(),
            item.ranges.len(),
            if item.end.is_some() {
                "已知尾端，仍須核對前段是否有缺口"
            } else {
                "尚未確認尾端"
            }
        );
        self.put(item);
    }
    fn put(&mut self, item: Coverage) {
        let key = format!(
            "{}|{}",
            item.path.replace('\\', "/").to_lowercase(),
            item.scope
        );
        // 不同工具的版本可能分別表示原始 bytes 與轉換後文字；只比較同一種來源範圍。
        if item.revision.is_some()
            && self.report.is_some()
            && self.coverage.get(&key).is_some_and(|previous| {
                previous.revision.is_some() && previous.revision != item.revision
            })
        {
            self.review_required = true;
        }
        if self.coverage.len() >= 500 && !self.coverage.contains_key(&key) {
            self.coverage_truncated = true;
            return;
        }
        self.coverage.insert(key, item);
    }

    /// 從原生工具結果建立紀錄；「交給 Python」不等於模型已閱讀或完整解析。
    pub fn observe(&mut self, tool: &super::Tool, result: &Value) {
        use super::Tool;
        let value = &result["result"];
        if result["ok"] != true {
            let request = serde_json::to_value(tool).unwrap_or_default();
            let mut paths = Vec::new();
            if let Some(path) = request["path"].as_str() {
                paths.push(path.to_owned());
            }
            if let Tool::RunPython { inputs, .. } = tool {
                paths.extend(inputs.iter().map(|i| i.path.clone()));
            }
            if let Tool::SearchLogs { query, .. } = tool {
                paths.extend(query.paths.clone());
            }
            for path in paths {
                self.put(Coverage {
                    path,
                    scope: tool.label().into(),
                    status: "failed".into(),
                    detail: result["error"]
                        .as_str()
                        .unwrap_or("工具失敗")
                        .chars()
                        .take(500)
                        .collect(),
                    ..Default::default()
                });
            }
            return;
        }
        let put = |state: &mut Self,
                   path: &str,
                   scope: String,
                   status: &str,
                   revision: &Value,
                   detail: String| {
            state.put(Coverage {
                path: path.into(),
                scope,
                status: status.into(),
                revision: revision.as_str().map(str::to_owned),
                detail,
                ..Default::default()
            });
        };
        match tool {
            Tool::ListFiles { path: directory }
            | Tool::ListLogs {
                path: directory, ..
            } => {
                if let Some(entries) = value["entries"].as_array() {
                    for entry in entries {
                        if entry["kind"] == "directory"
                            || entry["is_dir"] == true
                            || entry["directory"] == true
                        {
                            continue;
                        }
                        let derived = entry["name"].as_str().map(|n| {
                            std::path::Path::new(directory)
                                .join(n)
                                .to_string_lossy()
                                .into_owned()
                        });
                        if let Some(path) = entry["path"].as_str().or(derived.as_deref()) {
                            put(
                                self,
                                path,
                                "檔案清單".into(),
                                "listed",
                                &Value::Null,
                                "僅列出；不代表已分析".into(),
                            );
                        }
                    }
                }
            }
            Tool::SearchLogs { query, .. } => {
                let scope = format!(
                    "LOG 查詢 {}",
                    super::text::revision(&serde_json::to_string(query).unwrap_or_default())
                );
                if let Some(entries) = value["coverage"].as_array() {
                    for entry in entries {
                        if let Some(path) = entry["path"].as_str() {
                            put(
                                self,
                                path,
                                scope.clone(),
                                entry["status"].as_str().unwrap_or("partial"),
                                &entry["revision"],
                                format!(
                                    "關鍵字：{}；日期 {:?}／時間 {:?}–{:?}；已掃描 {} 行；此查詢時間未分類 {} 行",
                                    serde_json::to_string(&query.terms).unwrap_or_default().chars().take(300).collect::<String>(),
                                    query.date, query.start_time, query.end_time,
                                    entry["scanned_lines"],
                                    value["unclassified_time_lines"]
                                ),
                            );
                        }
                    }
                }
            }
            Tool::ReadLog {
                path,
                start_line,
                start_column,
                ..
            } => {
                if let Some(end) = value["next_line"].as_u64() {
                    // 未完整取得的首行不計入；下一個未讀位置之前都是完整行。
                    let start = start_line - 1 + usize::from(*start_column > 0);
                    self.range(
                        path,
                        "LOG 原文（行）",
                        &value["revision"],
                        start,
                        end as usize - 1,
                        value["eof"] == true,
                    );
                }
            }
            Tool::ReadFile { path, offset } => {
                if let (Some(end), Some(total)) =
                    (value["next_offset"].as_u64(), value["total"].as_u64())
                {
                    self.range(
                        path,
                        "文件閱讀（字元）",
                        &value["revision"],
                        *offset,
                        end as usize,
                        end == total,
                    );
                }
            }
            Tool::RunPython { .. } => {
                if let Some(sources) = value["sources"].as_array() {
                    for source in sources {
                        if let Some(path) = source["path"].as_str() {
                            if let (Some(start), Some(end)) =
                                (source["start_line"].as_u64(), source["next_line"].as_u64())
                            {
                                self.range(
                                    path,
                                    "Python 分段輸入（行）",
                                    &source["revision"],
                                    start as usize - 1,
                                    end as usize - 1,
                                    source["eof"] == true,
                                );
                                continue;
                            }
                            put(
                                self,
                                path,
                                "Python 輸入".into(),
                                "provided",
                                &source["revision"],
                                format!(
                                    "{}；處理筆數需另核對",
                                    source["scope"].as_str().unwrap_or("完整快照已交給本機計算")
                                ),
                            );
                        }
                    }
                }
            }
            Tool::ExportLogDataset { .. } => {
                if let Some(sources) = value["extraction"]["sources"]
                    .as_array()
                    .or_else(|| value["sources"].as_array())
                {
                    for source in sources {
                        if let Some(path) = source["path"].as_str() {
                            put(
                                self,
                                path,
                                "LOG 條件匯出".into(),
                                "complete",
                                &source["revision"],
                                format!(
                                    "掃描 {} 行，選取 {} 列",
                                    source["scanned_lines"], source["selected_rows"]
                                ),
                            );
                        }
                    }
                }
            }
            Tool::ReadExcelRange { path, .. }
            | Tool::InspectExcel { path, .. }
            | Tool::ExportExcelDataset { path, .. } => put(
                self,
                path,
                "Excel 選定範圍".into(),
                "partial",
                &value["revision"],
                "僅工具指定的工作表、列與欄；不代表整本已讀。".into(),
            ),
            _ => (),
        }
    }

    /// 每輪只傳簡短索引；完整檔案清單及證據快照留在本機畫面與工具紀錄。
    pub fn summary(&self) -> Value {
        let pending: Vec<_> = self
            .coverage
            .values()
            .filter(|v| matches!(v.status.as_str(), "failed" | "partial" | "pending"))
            .take(15)
            .collect();
        let mut summary = json!({"tracked_scopes":self.coverage.len(),"coverage_truncated":self.coverage_truncated,"attention":pending,
            "report":self.report.as_ref().map(|r| json!({"goal":r["goal"],"current_step":r["current_step"],"open_questions":r["open_questions"],"superseded":r["superseded"],
                "findings":r["findings"].as_array().map(|items|items.iter().map(|f|json!({"claim":f["claim"],"status":f["status"],"evidence_operations":f["evidence"].as_array().map(|e|e.iter().map(|e|e["operation_id"].clone()).collect::<Vec<_>>())})).collect::<Vec<_>>()),
                "checks":r["checks"].as_array().map(|items|items.iter().map(|c|json!({"label":c["label"],"operation_id":c["operation_id"],"passed":c["passed"],"total":c["total"]})).collect::<Vec<_>>()),"method_note":r["method_note"]})),"review_required":self.review_required,
            "overview_truncated":false,"rule":"部分閱讀／尚未搜尋不是沒有命中。來源與條件不同不可互相替代。使用者更正後重新核對結論；概況節錄時可用 read_work_log 查回 record_analysis 與原始操作。"});
        // 概況是索引，不讓數百個區間或長路徑擠掉使用者要求與近期工具結果。
        while summary.to_string().len() > 24_000 {
            let mut removed = false;
            for pointer in [
                "/attention",
                "/report/superseded",
                "/report/open_questions",
                "/report/checks",
                "/report/findings",
            ] {
                if summary
                    .pointer_mut(pointer)
                    .and_then(Value::as_array_mut)
                    .is_some_and(|a| a.pop().is_some())
                {
                    removed = true;
                    break;
                }
            }
            if !removed {
                break;
            }
            summary["overview_truncated"] = json!(true);
        }
        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coverage_requires_all_ranges_and_keeps_query_scopes_separate() {
        let mut state = State::default();
        state.range("a.log", "Python 分段輸入（行）", &json!("r1"), 5, 10, true);
        assert_eq!(state.coverage.values().next().unwrap().status, "partial");
        state.range("a.log", "Python 分段輸入（行）", &json!("r1"), 0, 5, false);
        assert_eq!(state.coverage.values().next().unwrap().status, "complete");
        state.range("a.log", "Python 分段輸入（行）", &json!("r2"), 0, 5, false);
        assert_eq!(state.coverage.values().next().unwrap().status, "partial");
        state.range("a.log", "其他條件", &json!("r2"), 5, 10, true);
        assert_eq!(state.coverage.len(), 2);
        let restored: State =
            serde_json::from_value(serde_json::to_value(&state).unwrap()).unwrap();
        assert_eq!(restored.coverage.len(), 2);
        state.report = Some(
            json!({"goal":"驗證","current_step":"整理","findings":[{"claim":"保留重要發現","status":"hypothesis","evidence":[]}]}),
        );
        assert_eq!(
            state.summary()["report"]["findings"][0]["claim"],
            "保留重要發現"
        );
    }
    #[test]
    fn checks_use_recorded_counts_and_reject_duplicate_parts() {
        let result = json!({"ok":true,"result":{"summary":{"total":10,"parsed":8,"bad":2,"counterexamples":1}}});
        let mut spec = Check {
            label: "解析數量".into(),
            operation_id: "op".into(),
            kind: "balance".into(),
            total_pointer: "/summary/total".into(),
            part_pointers: vec!["/summary/parsed".into(), "/summary/bad".into()],
        };
        assert_eq!(check(&spec, &result).unwrap()["passed"], true);
        spec.part_pointers[1] = "/summary/parsed".into();
        assert!(check(&spec, &result).is_err());
        spec.kind = "zero".into();
        spec.total_pointer = "/summary/counterexamples".into();
        spec.part_pointers.clear();
        assert_eq!(check(&spec, &result).unwrap()["passed"], false);
        assert!(select(&json!({"ok":false}), "").is_err());
        assert!(select(&result, "/missing").is_err());
    }
}
