//! `.lmai` 是專案記憶，不是文件來源。模型僅透過專用工具操作語意筆記；
//! 檔案版本、實際結果及快取完整性由程式管理，全部以目前 Windows 帳號 DPAPI 保存。
mod context;
mod vault;
use super::{files, text, Project};
use crate::{jobs, protocol::Message, unix_now, AppResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use vault::Vault;

#[derive(Clone, Serialize, Deserialize)]
struct NoteVersion {
    title: String,
    body: String,
    deleted: bool,
}
#[derive(Clone, Serialize, Deserialize)]
struct Note {
    id: String,
    scope: String,
    conversation: String,
    title: String,
    body: String,
    revision: u64,
    deleted: bool,
    updated: u64,
    previous: Vec<NoteVersion>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Section {
    id: String,
    title: String,
    start: usize,
    end: usize,
    summary: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Document {
    note_revision: u64,
    path: String,
    stamp: String,
    revision: String,
    content: String,
    sections: Vec<Section>,
    summary: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
struct DocumentEntry {
    path: String,
    revision: String,
    summary: Option<String>,
    updated: u64,
}
#[derive(Clone, Serialize, Deserialize)]
struct RunEntry {
    id: String,
    conversation: String,
    request: String,
    summary: String,
    state: String,
    outputs: Vec<String>,
    updated: u64,
}
#[derive(Clone, Serialize, Deserialize)]
struct RunResult {
    entry: RunEntry,
    request: String,
    result: String,
}
#[derive(Default, Serialize, Deserialize)]
struct Index {
    notes: Vec<Note>,
    documents: Vec<DocumentEntry>,
    runs: Vec<RunEntry>,
}
#[derive(Serialize, Deserialize)]
struct PdfCache {
    source_hash: String,
    markdown_hash: String,
    profile: String,
    markdown: String,
}

pub struct Memory {
    vault: Vault,
    project: Project,
    conversation: String,
    /// 只有本次實際送給模型的範圍可以支援新摘要；快取全文存在不等於模型已讀取。
    read_ranges: BTreeMap<String, Vec<(usize, usize)>>,
}
fn key(path: &str) -> String {
    text::revision(&path.replace('\\', "/").to_lowercase())
}
fn short(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}
fn check_text(value: &str, max: usize) -> AppResult<()> {
    if value.trim().is_empty() || value.chars().count() > max {
        return Err(format!("筆記需為 1–{max} 字。"));
    }
    Ok(())
}
fn accessible(note: &Note, conversation: &str) -> bool {
    note.scope == "project" || note.conversation == conversation
}

impl Memory {
    pub fn open(project: Project, conversation: &str) -> AppResult<Self> {
        jobs::validate_id(conversation)?;
        let vault = Vault::new(&project.root)?;
        // 有既存但無法解密的索引時不覆寫，也不假裝取得空專案。
        let _: Option<Index> = vault.transaction()?.read("project", "index")?;
        Ok(Self {
            vault,
            project,
            conversation: conversation.into(),
            read_ranges: BTreeMap::new(),
        })
    }
    /// 跨任務摘要也可能含受保護文件內容，沿用原有的 TXT 成果限制。
    pub fn has_protected_documents(&self) -> AppResult<bool> {
        let index: Index = self
            .vault
            .transaction()?
            .read("project", "index")?
            .unwrap_or_default();
        Ok(index
            .documents
            .iter()
            .any(|d| !d.path.to_ascii_lowercase().ends_with(".md")))
    }
    pub fn list_notes(&self, query: &str) -> AppResult<Value> {
        let tx = self.vault.transaction()?;
        let index: Index = tx.read("project", "index")?.unwrap_or_default();
        let mut notes: Vec<_> = index
            .notes
            .iter()
            .filter(|n| accessible(n, &self.conversation))
            .collect();
        notes.sort_by_key(|n| {
            std::cmp::Reverse((
                context::score(query, &format!("{} {}", n.title, n.body)),
                n.updated,
            ))
        });
        let count = notes.len();
        Ok(
            json!({"notes":notes.into_iter().take(30).map(|n|json!({"id":n.id,"scope":n.scope,"title":n.title,"revision":n.revision.to_string(),"deleted":n.deleted})).collect::<Vec<_>>(),"truncated":count>30}),
        )
    }
    pub fn read_note(&self, id: &str) -> AppResult<Value> {
        let index: Index = self
            .vault
            .transaction()?
            .read("project", "index")?
            .unwrap_or_default();
        let note = index
            .notes
            .iter()
            .find(|n| n.id == id && accessible(n, &self.conversation))
            .ok_or("找不到本對話可讀取的筆記。")?;
        Ok(
            json!({"id":note.id,"scope":note.scope,"title":note.title,"body":note.body,"revision":note.revision.to_string(),"deleted":note.deleted,"restorable":!note.previous.is_empty()}),
        )
    }
    pub fn create_note(&self, scope: &str, title: &str, body: &str) -> AppResult<Value> {
        if !matches!(scope, "project" | "conversation") {
            return Err(
                "筆記範圍只接受 project 或 conversation；文件摘要請用 update_document_note。"
                    .into(),
            );
        }
        check_text(title, 100)?;
        check_text(body, 2000)?;
        let tx = self.vault.transaction()?;
        let mut index: Index = tx.read("project", "index")?.unwrap_or_default();
        if index.notes.len() >= 200 {
            return Err("筆記已達 200 份上限。".into());
        }
        let id = jobs::new_id()?;
        index.notes.push(Note {
            id: id.clone(),
            scope: scope.into(),
            conversation: self.conversation.clone(),
            title: title.into(),
            body: body.into(),
            revision: 1,
            deleted: false,
            updated: unix_now(),
            previous: vec![],
        });
        tx.write("project", "index", &index)?;
        Ok(json!({"id":id,"revision":"1"}))
    }
    /// 編輯／軟刪除／復原都要核對版本；保留最近五份內容供復原，不授予其他文件寫入。
    pub fn change_note(
        &self,
        id: &str,
        revision: &str,
        edit: Option<(&str, &str)>,
        restore: bool,
    ) -> AppResult<Value> {
        if let Some((title, body)) = edit {
            check_text(title, 100)?;
            check_text(body, 2000)?;
        }
        let tx = self.vault.transaction()?;
        let mut index: Index = tx.read("project", "index")?.unwrap_or_default();
        let note = index
            .notes
            .iter_mut()
            .find(|n| n.id == id && accessible(n, &self.conversation))
            .ok_or("找不到筆記。")?;
        if note.revision.to_string() != revision {
            return Err("筆記版本已改變，請重新讀取。".into());
        }
        let previous = NoteVersion {
            title: note.title.clone(),
            body: note.body.clone(),
            deleted: note.deleted,
        };
        if restore {
            let saved = note.previous.pop().ok_or("沒有可復原的筆記版本。")?;
            note.title = saved.title;
            note.body = saved.body;
            note.deleted = saved.deleted;
        } else {
            if let Some((title, body)) = edit {
                note.title = title.into();
                note.body = body.into();
                note.deleted = false;
            } else {
                note.deleted = true;
            }
            note.previous.push(previous);
            if note.previous.len() > 5 {
                note.previous.remove(0);
            }
        }
        note.revision += 1;
        note.updated = unix_now();
        let value = json!({"id":id,"revision":note.revision.to_string(),"deleted":note.deleted});
        tx.write("project", "index", &index)?;
        Ok(value)
    }
    /// 來源指紋包含磁碟版本及匯入快照；舊快照仍由使用者決定何時重新匯入。
    fn stamp(&self, path: &str) -> AppResult<String> {
        let source = files::fingerprint(&self.project, path)?;
        Ok(text::revision(&format!(
            "{source}:{}",
            self.project
                .imports
                .get(&path.replace('\\', "/"))
                .map(|s| text::revision(s))
                .unwrap_or_default()
        )))
    }
    pub fn register_document(
        &mut self,
        path: &str,
        content: &str,
        stamp_before: &str,
    ) -> AppResult<()> {
        files::relative(path)?;
        if self.stamp(path)? != stamp_before {
            return Err("讀取期間來源已變更，未保存文件筆記。".into());
        }
        let tx = self.vault.transaction()?;
        let id = key(path);
        let mut index: Index = tx.read("project", "index")?.unwrap_or_default();
        let revision = text::revision(content);
        let old: Option<Document> = tx.read("documents", &id)?;
        if old
            .as_ref()
            .is_some_and(|d| d.revision == revision && d.stamp == stamp_before)
        {
            return Ok(());
        }
        if old.is_none() && index.documents.len() >= 200 {
            return Err("文件記憶已達 200 份上限。".into());
        }
        let doc = Document {
            note_revision: 1,
            path: path.replace('\\', "/"),
            stamp: stamp_before.into(),
            revision: revision.clone(),
            content: content.into(),
            sections: sections(content),
            summary: None,
        };
        tx.write("documents", &id, &doc)?;
        index.documents.retain(|d| key(&d.path) != id);
        index.documents.push(DocumentEntry {
            path: doc.path,
            revision,
            summary: None,
            updated: unix_now(),
        });
        tx.write("project", "index", &index)?;
        self.read_ranges.remove(&id);
        Ok(())
    }
    pub fn source_stamp(&self, path: &str) -> AppResult<String> {
        self.stamp(path)
    }
    fn document(&self, path: &str) -> AppResult<Document> {
        files::relative(path)?;
        let doc: Document = self
            .vault
            .transaction()?
            .read("documents", &key(path))?
            .ok_or("尚未建立文件索引，請先 read_file。")?;
        if self.stamp(path)? != doc.stamp || text::revision(&doc.content) != doc.revision {
            return Err("文件或匯入內容已變更，請 read_file 重新建立摘要。".into());
        }
        Ok(doc)
    }
    pub fn record_read(&mut self, path: &str, start: usize, end: usize) {
        let ranges = self.read_ranges.entry(key(path)).or_default();
        ranges.push((start, end));
        ranges.sort_unstable();
        let mut merged: Vec<(usize, usize)> = vec![];
        for &(a, b) in ranges.iter() {
            if let Some(last) = merged.last_mut().filter(|last| a <= last.1) {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        *ranges = merged;
    }
    fn covered(&self, path: &str, start: usize, end: usize) -> bool {
        self.read_ranges
            .get(&key(path))
            .is_some_and(|ranges| ranges.iter().any(|&(a, b)| a <= start && b >= end))
    }
    pub fn document_info(&self, path: &str, offset: usize) -> AppResult<Value> {
        let doc = self.document(path)?;
        if offset > doc.sections.len() {
            return Err("分段索引位置超過文件範圍。".into());
        }
        let full = self.covered(path, 0, doc.content.chars().count());
        Ok(
            json!({"path":doc.path,"revision":doc.revision,"note_revision":doc.note_revision.to_string(),"summary":doc.summary,"fully_read_this_run":full,
            "summary_needed":full&&doc.summary.is_none(),"section_offset":offset,"next_section_offset":(offset+10).min(doc.sections.len()),"total_sections":doc.sections.len(),"sections":doc.sections.iter().skip(offset).take(10).map(|s|json!({"section_id":s.id,"title":s.title,"start":s.start,"end":s.end,"summary":s.summary,"read_this_run":self.covered(path,s.start,s.end)})).collect::<Vec<_>>() }),
        )
    }
    /// 一般分頁閱讀只附相交區段的索引，避免每次又塞入整份文件的章節目錄。
    pub fn read_info(&self, path: &str, start: usize, end: usize) -> AppResult<Value> {
        let doc = self.document(path)?;
        let full = self.covered(path, 0, doc.content.chars().count());
        Ok(
            json!({"path":doc.path,"revision":doc.revision,"note_revision":doc.note_revision.to_string(),"summary":doc.summary,
            "fully_read_this_run":full,"summary_needed":full&&doc.summary.is_none(),"total_sections":doc.sections.len(),
            "sections":doc.sections.iter().filter(|s|s.start<end&&s.end>start).map(|s|json!({"section_id":s.id,"title":s.title,"start":s.start,"end":s.end,"summary":s.summary,"read_this_run":self.covered(path,s.start,s.end)})).collect::<Vec<_>>() }),
        )
    }
    pub fn read_section(
        &mut self,
        path: &str,
        revision: &str,
        section_id: &str,
    ) -> AppResult<Value> {
        let doc = self.document(path)?;
        if doc.revision != revision {
            return Err("文件版本已變更，請重新取得分段索引。".into());
        }
        let s = doc
            .sections
            .iter()
            .find(|s| s.id == section_id)
            .ok_or("找不到文件區段。")?;
        self.record_read(path, s.start, s.end);
        Ok(
            json!({"path":doc.path,"revision":doc.revision,"note_revision":doc.note_revision.to_string(),"section_id":s.id,"offset":s.start,"next_offset":s.end,"total":doc.content.chars().count(),"text":doc.content.chars().skip(s.start).take(s.end-s.start).collect::<String>()}),
        )
    }
    pub fn update_document_note(
        &self,
        path: &str,
        revision: &str,
        note_revision: &str,
        section_id: Option<&str>,
        summary: &str,
    ) -> AppResult<Value> {
        check_text(summary, 1000)?;
        let mut doc = self.document(path)?;
        if doc.revision != revision {
            return Err("文件版本已改變，不能更新舊摘要。".into());
        }
        if let Some(id) = section_id {
            let section = doc
                .sections
                .iter_mut()
                .find(|s| s.id == id)
                .ok_or("找不到文件區段。")?;
            if section.summary.is_none() && !self.covered(path, section.start, section.end) {
                return Err("此區段尚未完整提供給模型，請先讀取該區段。".into());
            }
            section.summary = Some(summary.into());
        } else {
            if doc.summary.is_none()
                && !self.covered(path, 0, doc.content.chars().count())
                && !doc.sections.iter().all(|s| s.summary.is_some())
            {
                return Err("尚未讀完全文或完成所有分段摘要，不可標示全文件摘要。".into());
            }
            doc.summary = Some(summary.into());
        }
        let tx = self.vault.transaction()?;
        let mut index: Index = tx.read("project", "index")?.unwrap_or_default();
        // 跨程序有人換了文件版本時，拒絕以先前讀到的副本覆蓋。
        let current: Document = tx
            .read("documents", &key(path))?
            .ok_or("文件索引已移除。")?;
        if current.revision != revision
            || current.stamp != doc.stamp
            || current.note_revision.to_string() != note_revision
        {
            return Err("文件索引已改變，請重新讀取。".into());
        }
        doc.note_revision = current.note_revision + 1;
        tx.write("documents", &key(path), &doc)?;
        if let Some(entry) = index
            .documents
            .iter_mut()
            .find(|d| key(&d.path) == key(path))
        {
            entry.summary = doc.summary;
            entry.updated = unix_now();
        }
        tx.write("project", "index", &index)?;
        Ok(
            json!({"path":path,"revision":revision,"note_revision":doc.note_revision.to_string(),"section_id":section_id,"saved":true}),
        )
    }
    pub fn read_task_result(&self, id: &str, field: &str, offset: usize) -> AppResult<Value> {
        jobs::validate_id(id)?;
        let task: RunResult = self
            .vault
            .transaction()?
            .read("runs", id)?
            .ok_or("找不到任務紀錄。")?;
        if task.entry.conversation != self.conversation {
            return Err("只能讀取本對話的任務結果。".into());
        }
        let content = match field {
            "request" => &task.request,
            "result" => &task.result,
            _ => return Err("field 只接受 request 或 result。".into()),
        };
        let total = content.chars().count();
        if offset > total {
            return Err("讀取位置超過結果。".into());
        }
        let part: String = content.chars().skip(offset).take(6000).collect();
        Ok(
            json!({"task_id":id,"state":task.entry.state,"outputs":task.entry.outputs,"field":field,"offset":offset,"next_offset":offset+part.chars().count(),"total":total,"text":part}),
        )
    }
    pub fn save_run(
        &self,
        id: &str,
        request: &str,
        result: &str,
        state: &str,
        summary: Option<&str>,
        outputs: &[String],
    ) -> AppResult<()> {
        jobs::validate_id(id)?;
        let tx = self.vault.transaction()?;
        let mut index: Index = tx.read("project", "index")?.unwrap_or_default();
        if !index.runs.iter().any(|r| r.id == id) && index.runs.len() >= 1000 {
            return Err("專案任務記憶已達 1000 筆上限。".into());
        }
        let summary = summary
            .filter(|s| !s.trim().is_empty())
            .map(|s| short(s, 1000))
            .unwrap_or_else(|| format!("結果節錄（非完整摘要）：{}", short(result, 220)));
        let entry = RunEntry {
            id: id.into(),
            conversation: self.conversation.clone(),
            request: short(request, 300),
            summary,
            state: state.into(),
            outputs: outputs.to_vec(),
            updated: unix_now(),
        };
        tx.write(
            "runs",
            id,
            &RunResult {
                entry: entry.clone(),
                request: request.into(),
                result: result.into(),
            },
        )?;
        index.runs.retain(|r| r.id != id);
        index.runs.push(entry);
        tx.write("project", "index", &index)
    }
    pub fn context(&self, messages: &[Message]) -> AppResult<Vec<Message>> {
        context::build(self, messages)
    }
}

/// 優先在章節或空白段落邊界分段；超長段落以 4000 Unicode 字元拆分。
fn sections(content: &str) -> Vec<Section> {
    let chars: Vec<_> = content.chars().collect();
    let mut result = vec![];
    let mut start = 0;
    while start < chars.len() {
        let max = (start + 4000).min(chars.len());
        let mut end = max;
        if max < chars.len() {
            if let Some(boundary) = ((start + 1000).min(max)..max).rev().find(|&i| {
                chars[i] == '\n'
                    && (chars.get(i + 1) == Some(&'\n') || chars.get(i + 1) == Some(&'#'))
            }) {
                end = boundary + 1;
            }
        }
        let title: String = chars[start..end].iter().take(70).collect();
        result.push(Section {
            id: format!("section_{:03}", result.len() + 1),
            title: title.lines().next().unwrap_or("").into(),
            start,
            end,
            summary: None,
        });
        start = end;
    }
    result
}

/// PDF 持久快取依來源雜湊、服務及轉換規則定位；損毀只當快取失效，不覆蓋筆記。
pub(super) fn pdf_read(
    root: &std::path::Path,
    hash: &str,
    profile: &str,
) -> AppResult<Option<String>> {
    let vault = Vault::new(root)?;
    let tx = vault.transaction()?;
    let cache: Option<PdfCache> =
        tx.read("cache", &text::revision(&format!("{hash}:{profile}")))?;
    Ok(cache
        .filter(|c| {
            c.source_hash == hash
                && c.profile == profile
                && text::revision(&c.markdown) == c.markdown_hash
        })
        .map(|c| c.markdown))
}
pub(super) fn pdf_write(
    root: &std::path::Path,
    hash: &str,
    profile: &str,
    markdown: &str,
) -> AppResult<()> {
    let vault = Vault::new(root)?;
    let tx = vault.transaction()?;
    let cache_key = text::revision(&format!("{hash}:{profile}"));
    let mut entries: Vec<String> = tx.read("cache", "index")?.unwrap_or_default();
    entries.retain(|id| id != &cache_key);
    while entries.len() >= 20 {
        let oldest = entries.remove(0);
        tx.remove_cache(&oldest)?;
    }
    entries.push(cache_key.clone());
    tx.write("cache", "index", &entries)?;
    tx.write(
        "cache",
        &text::revision(&format!("{hash}:{profile}")),
        &PdfCache {
            source_hash: hash.into(),
            profile: profile.into(),
            markdown_hash: text::revision(markdown),
            markdown: markdown.into(),
        },
    )
}

#[cfg(test)]
#[path = "memory/tests.rs"]
mod tests;
