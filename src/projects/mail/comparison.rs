//! 本機前文涵蓋比對。內文只在這次函式的記憶體中使用，不寫入工具結果／模型歷史。
//! 只有完整正規化文字包含才可略過舊信；字數、相似度、同主旨都不能證明完整。
use super::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

const MAX_LOCAL: usize = 1000;
const MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Row {
    mail_id: String,
    total_chars: Option<usize>,
    normalized_chars: Option<usize>,
    recommended: bool,
    covered_by: Option<String>,
    previous_mail_id: Option<String>,
    previous_text_present: Option<bool>,
    status: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Report {
    rows: Vec<Row>,
    complete: bool,
}

/// 僅去掉行首引用層級、统一空白；不刪簽名、數字、否定詞或任何實質文字。
fn normalize(body: &str) -> String {
    body.lines()
        .map(|line| {
            let mut line = line.trim();
            while let Some(rest) = line.strip_prefix('>') {
                line = rest.trim_start();
            }
            line.split_whitespace().collect::<Vec<_>>().join(" ")
        })
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// 必須在完整文字邊界匹配，避免把「同意 10」誤當成「不同意 100」的完整前文。
fn contains_complete(newer: &str, older: &str) -> bool {
    !older.is_empty()
        && newer.match_indices(older).any(|(start, _)| {
            let end = start + older.len();
            (start == 0 || newer[..start].ends_with(char::is_whitespace))
                && (end == newer.len() || newer[end..].starts_with(char::is_whitespace))
        })
}

/// 主旨僅作無 ConversationID 時的保守候選分組，仍須全文包含才判定涵蓋。
fn group(header: &Header) -> String {
    if !header.thread_id.is_empty() {
        return format!("thread:{}", header.thread_id);
    }
    let mut subject = header.subject.trim();
    while let Some((prefix, rest)) = subject.split_once(':').or_else(|| subject.split_once('：')) {
        if !matches!(
            prefix.trim().to_ascii_lowercase().as_str(),
            "re" | "fw" | "fwd" | "回覆" | "回复" | "轉寄" | "转发"
        ) {
            break;
        }
        subject = rest.trim();
    }
    let mut people = header.recipients.clone();
    people.push(header.sender.clone());
    people.sort();
    format!("fallback:{}:{people:?}", subject.to_lowercase())
}

impl Session {
    /// 同組 ID 的續頁只回已保存的比較摘要；不重讀 Outlook，不消耗 AI 內文額度。
    pub fn compare(
        &mut self,
        source: &mut dyn Source,
        ids: &[String],
        offset: usize,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        self.require_consent()?;
        if ids.is_empty() || ids.len() > MAX_LOCAL {
            return Err("本機比對每次需指定 1 至 1000 封已列出的郵件。".into());
        }
        let unique: BTreeSet<_> = ids.iter().collect();
        if unique.len() != ids.len() {
            return Err("比對郵件不可重複。".into());
        }
        let mut headers = Vec::new();
        for id in unique {
            let header = self
                .saved
                .mails
                .get(id)
                .ok_or("只能比對本次標題清單中的郵件。")?;
            let folder = self
                .saved
                .folders
                .get(&header.folder_id)
                .ok_or("郵件資料夾已失效。")?;
            if !self.policy.permits(&folder.store, &folder.entry) {
                return Err("郵件不在勾選範圍。".into());
            }
            headers.push(header.clone());
        }
        let cache_key = super::super::text::revision(
            &serde_json::to_string(&headers).map_err(|e| e.to_string())?,
        );
        if !self.saved.comparisons.contains_key(&cache_key) {
            if offset != 0 {
                return Err("比對快照不存在，請从 offset=0 開始。".into());
            }
            if self.saved.comparisons.len() >= 20 {
                return Err("本次已保存 20 組比對；請使用既有建議整理交付。".into());
            }
            let identities: Vec<_> = headers
                .iter()
                .map(|h| {
                    let f = &self.saved.folders[&h.folder_id];
                    super::super::text::revision(&format!(
                        "{}\n{}\n{}",
                        f.store, h.entry, h.modified
                    ))
                })
                .collect();
            let additional = identities
                .iter()
                .filter(|id| !self.saved.compared.contains(*id))
                .collect::<BTreeSet<_>>()
                .len();
            if self.saved.compared.len() + additional > MAX_LOCAL {
                return Err(
                    "本次本機比對最多 1000 封不同郵件版本，請依既有結果整理或另開任務。".into(),
                );
            }
            let started = Instant::now();
            let mut bytes = 0usize;
            let mut bodies = BTreeMap::new();
            let mut rows = BTreeMap::new();
            for (header, identity) in headers.iter().zip(identities) {
                if cancel.load(Ordering::Relaxed) {
                    return Err("本機郵件比對已取消。".into());
                }
                let mut row = Row {
                    mail_id: header.id.clone(),
                    total_chars: None,
                    normalized_chars: None,
                    recommended: true,
                    covered_by: None,
                    previous_mail_id: None,
                    previous_text_present: None,
                    status: "not_scanned_limit".into(),
                };
                if started.elapsed() < Duration::from_secs(120) && bytes < MAX_BYTES {
                    self.saved.compared.insert(identity);
                    let folder = &self.saved.folders[&header.folder_id];
                    // 不回傳 COM 原始錯誤，以免錯誤描述夾帶未選讀的信件資料。
                    match source.body(folder, header, cancel) {
                        Ok(body) if body.len() <= 256_000 && bytes + body.len() <= MAX_BYTES => {
                            bytes += body.len();
                            row.total_chars = Some(body.chars().count());
                            let normalized = normalize(&body);
                            row.normalized_chars = Some(normalized.chars().count());
                            row.status = "needs_read".into();
                            bodies.insert(header.id.clone(), normalized);
                        }
                        _ => row.status = "unavailable_or_too_large".into(),
                    }
                }
                rows.insert(header.id.clone(), row);
            }
            let mut groups: BTreeMap<String, Vec<&Header>> = BTreeMap::new();
            for header in &headers {
                groups.entry(group(header)).or_default().push(header);
            }
            let mut complete = rows.values().all(|r| r.total_chars.is_some());
            for group in groups.values_mut() {
                group.sort_by(|a, b| b.sent_at.cmp(&a.sent_at).then_with(|| b.id.cmp(&a.id)));
                let mut selected: Vec<&Header> = Vec::new();
                for (index, header) in group.iter().enumerate() {
                    if cancel.load(Ordering::Relaxed) {
                        return Err("本機郵件比對已取消。".into());
                    }
                    if started.elapsed() >= Duration::from_secs(120) {
                        complete = false;
                        break;
                    }
                    let Some(body) = bodies.get(&header.id) else {
                        continue;
                    };
                    let covered = selected.iter().find(|newer| {
                        !body.is_empty()
                            && bodies
                                .get(&newer.id)
                                .is_some_and(|text| contains_complete(text, body))
                    });
                    let row = rows.get_mut(&header.id).ok_or("比對紀錄不存在。")?;
                    if let Some(newer) = covered {
                        row.recommended = false;
                        row.covered_by = Some(newer.id.clone());
                        row.status = "covered_exact_normalized_text".into();
                    } else {
                        selected.push(header);
                    }
                    if let Some(older) = group.get(index + 1) {
                        row.previous_mail_id = Some(older.id.clone());
                        row.previous_text_present = bodies
                            .get(&older.id)
                            .map(|previous| contains_complete(body, previous));
                    }
                }
            }
            // 原文到此即釋放，Saved 只保存不含正文的摘要。未掃描／比對不明者永不標成已涵蓋。
            let mut result: Vec<_> = rows.into_values().collect();
            result.sort_by(|a, b| a.mail_id.cmp(&b.mail_id));
            self.saved.comparisons.insert(
                cache_key.clone(),
                Report {
                    rows: result,
                    complete,
                },
            );
        }
        let report = &self.saved.comparisons[&cache_key];
        if offset > report.rows.len() {
            return Err("比對續頁超出範圍。".into());
        }
        let page: Vec<_> = report.rows.iter().skip(offset).take(30).collect();
        for row in &page {
            let header = &self.saved.mails[&row.mail_id];
            source.verify(&self.saved.folders[&header.folder_id], header, cancel)?;
        }
        let next = offset + page.len();
        Ok(
            json!({"comparison_id":cache_key,"rows":page,"total":report.rows.len(),"next_offset":next,"has_more":next<report.rows.len(),
            "complete":report.complete,"recommended_count":report.rows.iter().filter(|r|r.recommended).count(),
            "local_compared_count":self.saved.compared.len(),"local_remaining":MAX_LOCAL-self.saved.compared.len(),
            "ai_read_count":self.saved.bodies.len(),"ai_read_remaining":50-self.saved.bodies.len(),
            "note":"只比較本次選取郵件的純文字。previous_text_present=false 僅表示前文未完整匹配，可能省略、改寫、格式差異或不同分支，不證明刪改。字數增加不證明完整；covered_by 僅證明正規化文字涵蓋，不取代原作者／日期的核對。未掃描或失敗項目不得宣稱已涵蓋。優先讀最新信與 recommended=true 的相關補充信，資料足夠即交付，不必讀滿50封。附件與圖片未比較。"}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalization_preserves_meaning_and_numbers() {
        assert_eq!(normalize("> 原文\r\n>> 第二行"), "原文 第二行");
        assert_ne!(normalize("不同意 10"), normalize("同意 100"));
        assert!(!normalize("新增很長的段落 取代舊內容").contains("關鍵舊內容"));
        assert!(!contains_complete("不同意 100", "同意 10"));
        assert!(contains_complete("新增\n同意 10", "同意 10"));
    }

    struct Bodies {
        text: BTreeMap<String, String>,
        reads: usize,
        fail: bool,
    }
    impl Source for Bodies {
        fn folders(
            &mut self,
            _: &str,
            _: Option<&Folder>,
            _: &AtomicBool,
        ) -> AppResult<(Vec<Folder>, Vec<String>)> {
            unreachable!()
        }
        fn headers(
            &mut self,
            _: &Folder,
            _: NaiveDate,
            _: NaiveDate,
            _: &AtomicBool,
        ) -> AppResult<Scan> {
            unreachable!()
        }
        fn body(&mut self, _: &Folder, h: &Header, _: &AtomicBool) -> AppResult<String> {
            self.reads += 1;
            if self.fail {
                return Err("PRIVATE_ERROR_BODY".into());
            }
            Ok(self.text[&h.id].clone())
        }
    }
    fn fixture(values: Vec<String>) -> (Session, Bodies, Vec<String>) {
        let mut session = Session {
            allowed: Some(true),
            ..Default::default()
        };
        session.saved.folders.insert(
            "folder".into(),
            Folder {
                id: "folder".into(),
                name: "工作".into(),
                path: "工作".into(),
                scope: "online_sent".into(),
                store: "s".into(),
                entry: "f".into(),
                children: 0,
                readable: true,
                excluded: vec![],
            },
        );
        let mut bodies = Bodies {
            text: BTreeMap::new(),
            reads: 0,
            fail: false,
        };
        let mut ids = vec![];
        for (i, body) in values.into_iter().enumerate() {
            let id = format!("m{i:04}");
            session.saved.mails.insert(
                id.clone(),
                Header {
                    id: id.clone(),
                    folder_id: "folder".into(),
                    subject: "主題".into(),
                    sender: "a@test".into(),
                    recipients: vec!["b@test".into()],
                    recipients_are_groups: false,
                    sent_at: format!("{i:05}"),
                    received_at: format!("{i:05}"),
                    entry: id.clone(),
                    modified: "version1".into(),
                    duplicate_key: None,
                    thread_id: "thread1".into(),
                },
            );
            bodies.text.insert(id.clone(), body);
            ids.push(id);
        }
        (session, bodies, ids)
    }
    #[test]
    fn ten_messages_with_gap_recommend_four_and_ten_without_sending_bodies() {
        let mut values = vec![];
        let mut text = String::new();
        for i in 1..=10 {
            if i == 5 {
                text.clear();
            }
            text = format!("PRIVATE_CONTENT_{i} {text}");
            values.push(text.clone());
        }
        let (mut session, mut source, ids) = fixture(values);
        let result = session
            .compare(&mut source, &ids, 0, &AtomicBool::new(false))
            .unwrap();
        let recommended: Vec<_> = result["rows"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["recommended"] == true)
            .map(|r| r["mail_id"].as_str().unwrap())
            .collect();
        assert_eq!(recommended, vec!["m0003", "m0009"]);
        assert_eq!(result["rows"][4]["previous_text_present"], false);
        assert_eq!(result["ai_read_count"], 0);
        assert!(!result.to_string().contains("PRIVATE_CONTENT"));
        assert!(!serde_json::to_string(&session.saved)
            .unwrap()
            .contains("PRIVATE_CONTENT"));
        session
            .compare(&mut source, &ids, 0, &AtomicBool::new(false))
            .unwrap();
        assert_eq!(source.reads, 10);
        session
            .body(&mut source, "m0009", 0, &AtomicBool::new(false))
            .unwrap();
        assert_eq!(session.saved.bodies.len(), 1);
    }
    #[test]
    fn branches_and_rewritten_short_text_are_not_silently_dropped() {
        let (mut session, mut source, ids) = fixture(vec![
            "同意 10".into(),
            "分支A 同意 10".into(),
            "分支B 不同意 100".into(),
        ]);
        let result = session
            .compare(&mut source, &ids, 0, &AtomicBool::new(false))
            .unwrap();
        assert_eq!(result["recommended_count"], 2);
        assert_eq!(result["rows"][2]["previous_text_present"], false);
        assert_eq!(result["rows"][0]["covered_by"], "m0001");
    }
    #[test]
    fn local_thousand_and_ai_fifty_have_independent_enforced_limits() {
        let (mut session, mut source, ids) =
            fixture((0..1001).map(|i| format!("PRIVATE_CONTENT_{i}")).collect());
        let cancel = AtomicBool::new(false);
        assert!(session.compare(&mut source, &ids, 0, &cancel).is_err());
        assert_eq!(source.reads, 0);
        let first = session
            .compare(&mut source, &ids[..1000], 0, &cancel)
            .unwrap();
        assert_eq!(first["local_compared_count"], 1000);
        assert_eq!(first["ai_read_remaining"], 50);
        assert!(session
            .compare(&mut source, &ids[1000..], 0, &cancel)
            .is_err());
        let mut offset = 30;
        while offset < 1000 {
            let page = session
                .compare(&mut source, &ids[..1000], offset, &cancel)
                .unwrap();
            offset = page["next_offset"].as_u64().unwrap() as usize;
        }
        assert_eq!(source.reads, 1000);
        for id in ids.iter().take(50) {
            session.body(&mut source, id, 0, &cancel).unwrap();
        }
        assert!(session.body(&mut source, &ids[50], 0, &cancel).is_err());
        session.body(&mut source, &ids[0], 0, &cancel).unwrap();
        assert_eq!(source.reads, 1050);
    }
    #[test]
    fn failures_and_cancellation_never_claim_complete_coverage() {
        let (mut session, mut source, ids) = fixture(vec!["PRIVATE_CONTENT".into()]);
        source.fail = true;
        let result = session
            .compare(&mut source, &ids, 0, &AtomicBool::new(false))
            .unwrap();
        assert_eq!(result["complete"], false);
        assert_eq!(result["rows"][0]["recommended"], true);
        assert!(!result.to_string().contains("PRIVATE_ERROR"));
        let (mut session, mut source, ids) = fixture(vec!["PRIVATE_CONTENT".into()]);
        assert!(session
            .compare(&mut source, &ids, 0, &AtomicBool::new(true))
            .is_err());
        assert_eq!(source.reads, 0);
    }
}
