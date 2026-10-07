//! 大型紀錄檔的唯讀搜尋。逐行解碼、限制每頁輸出，原文不進入文件全文快取。
//! 續頁保存查詢及 SHA256，任何來源改變都必須重新搜尋，避免混合不同版本。
use super::{files, text, Project};
use crate::AppResult;
use chrono::{NaiveDate, NaiveTime, Timelike};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    fs::File,
    io::{BufRead, BufReader, Read, Seek},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

// 逐行讀取，不一次配置整份文件；每頁與 Python 分段另有較小輸出上限。
const MAX_FILE: u64 = 1024 * 1024 * 1024;
const MAX_LINE: usize = 128 * 1024;
const PAGE_CHARS: usize = 12_000;
const SEARCH_LINES: usize = 250_000;

pub fn supported(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        matches!(
            e.to_ascii_lowercase().as_str(),
            "log" | "out" | "err" | "jsonl"
        )
    })
}
pub fn first_line() -> usize {
    1
}
pub fn default_lines() -> usize {
    100
}

/// 使用者提供的 YYYYMMDD_分類_站別.log；站別原樣保留，例如 A01-01、Z01-CY。
pub(super) fn name_parts(path: &Path) -> Option<(NaiveDate, String, String)> {
    let mut parts = path.file_stem()?.to_str()?.splitn(3, '_');
    let date = NaiveDate::parse_from_str(parts.next()?, "%Y%m%d").ok()?;
    let category = parts.next()?.to_owned();
    let station = parts.next()?.to_owned();
    if category.is_empty() || station.is_empty() {
        return None;
    }
    Some((date, category, station))
}

/// 只列指定資料夾的檔名，不讀內容；日期／分類／站別先篩選，再固定排序分頁。
pub(super) fn list(
    project: &Project,
    path: &str,
    date: Option<&str>,
    category: Option<&str>,
    station: Option<&str>,
    offset: usize,
    cancel: &AtomicBool,
) -> AppResult<Value> {
    let date = date
        .map(|d| {
            NaiveDate::parse_from_str(d, "%Y-%m-%d")
                .map_err(|_| "日期請使用 YYYY-MM-DD。".to_owned())
        })
        .transpose()?;
    let target = project.root.join(files::relative(path)?);
    let _guards = files::pin(&target)?;
    if let Some(directory) = _guards.last() {
        files::reject_internal(directory)?;
    }
    let mut entries = Vec::new();
    for (index, entry) in std::fs::read_dir(&target)
        .map_err(|e| e.to_string())?
        .enumerate()
    {
        check_cancel(cancel)?;
        if index >= 20_000 {
            return Err("此資料夾超過 20000 個項目，請改用日期子資料夾；未回傳不完整清單。".into());
        }
        let entry = entry.map_err(|e| e.to_string())?;
        if !supported(&entry.path()) || entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            continue;
        }
        use std::os::windows::fs::MetadataExt;
        if entry
            .metadata()
            .map_err(|e| e.to_string())?
            .file_attributes()
            & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
            != 0
        {
            continue;
        }
        let parts = name_parts(&entry.path());
        if date.is_some_and(|d| parts.as_ref().is_none_or(|p| p.0 != d))
            || category.is_some_and(|c| parts.as_ref().is_none_or(|p| !p.1.eq_ignore_ascii_case(c)))
            || station.is_some_and(|s| parts.as_ref().is_none_or(|p| !p.2.eq_ignore_ascii_case(s)))
        {
            continue;
        }
        entries.push(json!({"name":entry.file_name().to_string_lossy(),"path":Path::new(path).join(entry.file_name()).to_string_lossy(),"date":parts.as_ref().map(|p|p.0.to_string()),"category":parts.as_ref().map(|p|&p.1),"station":parts.as_ref().map(|p|&p.2),"bytes":entry.metadata().map_err(|e|e.to_string())?.len()}));
    }
    entries.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    let total = entries.len();
    if offset > total {
        return Err("清單位置超過目前檔案數，請重新列出。".into());
    }
    let page = entries
        .into_iter()
        .skip(offset)
        .take(100)
        .collect::<Vec<_>>();
    let next = offset + page.len();
    Ok(
        json!({"entries":page,"total":total,"has_more":next<total,"next_offset":next,"note":"依目前檔名清單分頁；檔案增減後請重新列出，搜尋仍會核對內容版本"}),
    )
}

/// 查詢條件與續頁位置分開保存；模型只能拿桌面簽發的游標接續同一查詢。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    pub paths: Vec<String>,
    #[serde(default)]
    pub terms: Vec<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub date: Option<String>,
    #[serde(default)]
    pub case_sensitive: bool,
    #[serde(default)]
    pub context_lines: usize,
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Cursor {
    query: Query,
    revisions: Vec<Option<String>>,
    file: usize,
    line: usize,
    errors: Vec<Value>,
    unclassified: usize,
    #[serde(default)]
    scanned_lines: Vec<usize>,
}

fn check_cancel(cancel: &AtomicBool) -> AppResult<()> {
    if cancel.load(Ordering::Relaxed) {
        Err("LOG 讀取已取消。".into())
    } else {
        Ok(())
    }
}

/// 每次只持有一份文件；沿用 broker 的路徑、連結及檔案鎖規則。
struct Source {
    _directories: Vec<File>,
    reader: BufReader<File>,
    revision: String,
    bytes: u64,
    utf16: Option<bool>,
    utf8_bom: bool,
    line: usize,
    encodings: std::collections::BTreeSet<String>,
    encoding_ambiguous: bool,
}
impl Source {
    fn open(
        project: &Project,
        path: &str,
        expected: Option<&str>,
        cancel: &AtomicBool,
    ) -> AppResult<Self> {
        let relative = files::relative(path)?;
        if !supported(&relative)
            && relative
                .extension()
                .and_then(|e| e.to_str())
                .is_none_or(|e| !e.eq_ignore_ascii_case("txt"))
        {
            return Err("LOG 工具支援 LOG、OUT、ERR、JSONL、TXT，不執行任何內容。".into());
        }
        let target = project.root.join(relative);
        let directories = files::pin(target.parent().ok_or("缺少 LOG 目錄。")?)?;
        let mut file = files::checked_file(&target)
            .map_err(|e| format!("無法唯讀開啟 LOG；若正被寫入，請使用已輪替或另存的紀錄。{e}"))?;
        files::reject_internal(&file)?;
        let bytes = file.metadata().map_err(|e| e.to_string())?.len();
        if bytes > MAX_FILE {
            return Err("單個 LOG 上限為 1 GiB，請依日期或機台拆分。".into());
        }
        let mut hash = Sha256::new();
        let mut buffer = [0; 65536];
        loop {
            check_cancel(cancel)?;
            let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
        let revision = format!("log:{:x}", hash.finalize());
        if expected.is_some_and(|r| r != revision) {
            return Err("LOG 來源已變更，請從第一頁重新搜尋，不可混用舊游標或行號。".into());
        }
        file.rewind().map_err(|e| e.to_string())?;
        let mut reader = BufReader::new(file);
        let head = reader.fill_buf().map_err(|e| e.to_string())?;
        let (utf16, bom) = if head.starts_with(&[0xff, 0xfe]) {
            (Some(true), 2)
        } else if head.starts_with(&[0xfe, 0xff]) {
            (Some(false), 2)
        } else if head.starts_with(&[0xef, 0xbb, 0xbf]) {
            (None, 3)
        } else {
            (None, 0)
        };
        reader.consume(bom);
        Ok(Self {
            _directories: directories,
            reader,
            revision,
            bytes,
            utf16,
            utf8_bom: bom == 3,
            line: 0,
            encodings: Default::default(),
            encoding_ambiguous: false,
        })
    }

    /// UTF-16 以 code unit 找換行，避免把中文字的低位元組誤認為 LF。
    fn next(&mut self, cancel: &AtomicBool) -> AppResult<Option<String>> {
        self.next_encoded(cancel, None)
    }

    fn next_encoded(
        &mut self,
        cancel: &AtomicBool,
        encoding: Option<&str>,
    ) -> AppResult<Option<String>> {
        check_cancel(cancel)?;
        let mut bytes = Vec::new();
        if let Some(little) = self.utf16 {
            loop {
                let mut word = [0; 2];
                if self
                    .reader
                    .read(&mut word[..1])
                    .map_err(|e| e.to_string())?
                    == 0
                {
                    break;
                }
                self.reader
                    .read_exact(&mut word[1..])
                    .map_err(|_| "UTF-16 LOG 長度不正確。")?;
                bytes.extend_from_slice(&word);
                if bytes.len() > MAX_LINE {
                    return Err("LOG 單行超過 128 KiB，未截斷或略過；請拆分此筆紀錄。".into());
                }
                if (if little {
                    u16::from_le_bytes(word)
                } else {
                    u16::from_be_bytes(word)
                }) == 10
                {
                    break;
                }
            }
            if bytes.is_empty() {
                return Ok(None);
            }
            let mut encoded = if little {
                vec![0xff, 0xfe]
            } else {
                vec![0xfe, 0xff]
            };
            encoded.extend_from_slice(&bytes);
            bytes = encoded;
        } else {
            // take 限制配置量；超長單行不允許 read_until 無界成長。
            self.reader
                .by_ref()
                .take((MAX_LINE + 1) as u64)
                .read_until(b'\n', &mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.is_empty() {
                return Ok(None);
            }
            if bytes.len() > MAX_LINE {
                return Err("LOG 單行超過 128 KiB，未截斷或略過；請拆分此筆紀錄。".into());
            }
        }
        self.line += 1;
        let decoded = if self.utf8_bom {
            self.encodings.insert("utf8-bom".into());
            // BOM 宣告 UTF-8 後不得逐行退回 ANSI，避免損壞資料看似成功。
            std::str::from_utf8(&bytes)
                .map_err(|_| "UTF-8 BOM 與內容不一致。".to_owned())
                .and_then(|s| text::validate(s).map(|()| s.to_owned()))
        } else if self.utf16.is_some() {
            self.encodings.insert("utf16-bom".into());
            text::decode(&bytes).map(|(line, _)| line)
        } else if let Some(mode) = encoding {
            super::python::encoding::decode(&bytes, true, Some(mode)).map(|decoded| {
                self.encodings.insert(decoded.name.into());
                self.encoding_ambiguous |= decoded.ambiguous;
                decoded.text
            })
        } else {
            text::decode(&bytes).map(|(line, _)| line)
        };
        let line = decoded.map_err(|e| format!("第 {} 行無法可靠解碼：{e}", self.line))?;
        Ok(Some(line.trim_end_matches(['\r', '\n']).to_owned()))
    }
}

/// 只把完整行交給 Python；下一段必須帶來源版本，保留一基原始行號。
pub(super) fn python_chunk(
    project: &Project,
    path: &str,
    revision: Option<&str>,
    start: usize,
    count: usize,
    encoding: Option<&str>,
    cancel: &AtomicBool,
) -> AppResult<Value> {
    if start == 0 || !(1..=50_000).contains(&count) || (start > 1 && revision.is_none()) {
        return Err("LOG 分段需從第 1 行開始，每段 1–50000 行；續段必須提供 revision。".into());
    }
    if encoding.is_some_and(|e| !matches!(e, "auto" | "big5" | "utf8")) {
        return Err("encoding 必須為 auto、big5 或 utf8。".into());
    }
    let mut source = Source::open(project, path, revision, cancel)?;
    if source.utf16.is_some() && encoding.is_some_and(|e| e != "auto") {
        return Err("UTF-16 LOG 分段應依 BOM 自動解碼，不可覆寫為 Big5／UTF-8。".into());
    }
    let mut text = String::new();
    let mut rows = 0usize;
    let mut eof = false;
    loop {
        if rows == count {
            // 用底層是否到尾端判斷，不為了探測下一段而解碼尚未要求的行。
            eof = source
                .reader
                .fill_buf()
                .map_err(|e| e.to_string())?
                .is_empty();
            break;
        }
        let Some(line) = source.next_encoded(cancel, Some(encoding.unwrap_or("auto")))? else {
            eof = true;
            break;
        };
        if source.line < start {
            continue;
        }
        if text.len() + line.len() + 1 > 2 * 1024 * 1024 {
            break;
        }
        text.push_str(&line);
        text.push('\n');
        rows += 1;
    }
    if rows == 0 && start > source.line + 1 {
        return Err("起始行超過 LOG 尾端。".into());
    }
    Ok(
        json!({"kind":"text","path":path,"revision":source.revision,"text":text,"start_line":start,"line_count":rows,
        "next_line":start+rows,"eof":eof,"bytes":source.bytes,"encoding":source.encodings.into_iter().collect::<Vec<_>>().join(" / "),"encoding_ambiguous":source.encoding_ambiguous,
        "scope":format!("LOG 分段：原始第 {start} 行起 {rows} 行；{}",if eof {"已到檔尾"} else {"尚有後續"})}),
    )
}

/// 以一基行號、零基字元位置續讀；長行可跨頁，不能只回傳前段並假裝讀完。
pub(super) fn read(
    project: &Project,
    path: &str,
    revision: Option<&str>,
    start: usize,
    column: usize,
    count: usize,
    cancel: &AtomicBool,
) -> AppResult<Value> {
    if start == 0
        || !(1..=200).contains(&count)
        || ((start > 1 || column > 0) && revision.is_none())
    {
        return Err("起始行需從 1 開始，每頁 1–200 行；續讀必須帶上一頁 revision。".into());
    }
    let mut source = Source::open(project, path, revision, cancel)?;
    let mut lines = Vec::new();
    let mut remaining = PAGE_CHARS;
    let mut next_line = start;
    let mut next_column = column;
    let mut eof = false;
    loop {
        let Some(line) = source.next(cancel)? else {
            eof = true;
            break;
        };
        if source.line < start {
            continue;
        }
        if lines.len() == count || remaining == 0 {
            break;
        }
        let offset = if source.line == start { column } else { 0 };
        let length = line.chars().count();
        if offset > length {
            return Err("起始字元超過該行長度。".into());
        }
        let content: String = line.chars().skip(offset).take(remaining).collect();
        let taken = content.chars().count();
        let partial = offset + taken < length;
        lines.push(json!({"line":source.line,"column":offset,"text":content,"continued":partial}));
        remaining = remaining.saturating_sub(taken.max(1));
        next_line = if partial {
            source.line
        } else {
            source.line + 1
        };
        next_column = if partial { offset + taken } else { 0 };
        if partial {
            break;
        }
    }
    if eof && source.line + 1 < start {
        return Err("起始行超過 LOG 尾端。".into());
    }
    Ok(
        json!({"path":path,"revision":source.revision,"bytes":source.bytes,"lines":lines,"next_line":next_line,"next_column":next_column,"has_more":!eof,"eof":eof,"scope":"僅本頁原文，不代表整份 LOG 已讀完"}),
    )
}

#[derive(Clone, Debug)]
struct Stamp {
    date: Option<NaiveDate>,
    millis: u32,
}

fn time(value: &str, end: bool) -> AppResult<u32> {
    let minute = value.len() == 5;
    let parsed = if minute {
        NaiveTime::parse_from_str(value, "%H:%M")
    } else {
        NaiveTime::parse_from_str(value, "%H:%M:%S%.f")
    }
    .map_err(|_| "時間請使用 HH:MM、HH:MM:SS 或 HH:MM:SS.sss。")?;
    let result = parsed.num_seconds_from_midnight() * 1000 + parsed.nanosecond() / 1_000_000;
    Ok(result + if end && minute { 59_999 } else { 0 })
}

/// 僅辨識行首／前綴中的明確時間；不從訊息正文內提及的任意時間猜事件時間。
fn stamp(line: &str) -> Option<Stamp> {
    let prefix = line.trim_start_matches(|c: char| c.is_whitespace() || matches!(c, '[' | '('));
    let bytes = prefix.as_bytes();
    let mut date = None;
    let mut start = 0;
    if bytes.len() >= 11 && bytes.get(4).is_some_and(|c| matches!(c, b'-' | b'/')) {
        let date_text = prefix.get(..10)?;
        date = NaiveDate::parse_from_str(date_text, "%Y-%m-%d")
            .or_else(|_| NaiveDate::parse_from_str(date_text, "%Y/%m/%d"))
            .ok();
        date?;
        if !matches!(bytes[10], b' ' | b'T' | b',') {
            return None;
        }
        start = 11;
    }
    let rest = prefix.get(start..)?.trim_start();
    let time_text: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || matches!(c, ':' | '.' | ','))
        .collect();
    if time_text.len() < 5 {
        return None;
    }
    // 逗號可作毫秒小數點，也可分隔時間與訊息；只移除尾端分隔符。
    let millis = time(&time_text.trim_end_matches(',').replace(',', "."), false).ok()?;
    Some(Stamp { date, millis })
}

struct Filter {
    start: Option<u32>,
    end: Option<u32>,
    date: Option<NaiveDate>,
    terms: Vec<String>,
}
impl Filter {
    fn new(query: &Query) -> AppResult<Self> {
        if query.paths.is_empty()
            || query.paths.len() > 30
            || query.terms.len() > 12
            || query.context_lines > 5
            || query
                .terms
                .iter()
                .any(|t| t.trim().is_empty() || t.chars().count() > 200)
        {
            return Err(
                "搜尋需指定 1–30 個 LOG，最多 12 個非空關鍵字、每字串 200 字，前後文各最多 5 行。"
                    .into(),
            );
        }
        let start = query
            .start_time
            .as_deref()
            .map(|s| time(s, false))
            .transpose()?;
        let end = query
            .end_time
            .as_deref()
            .map(|s| time(s, true))
            .transpose()?;
        if start.is_some() != end.is_some() {
            return Err("請同時提供起始與結束時間。".into());
        }
        let date = query
            .date
            .as_deref()
            .map(|d| {
                NaiveDate::parse_from_str(d, "%Y-%m-%d")
                    .map_err(|_| "日期請使用 YYYY-MM-DD。".to_owned())
            })
            .transpose()?;
        Ok(Self {
            start,
            end,
            date,
            terms: query
                .terms
                .iter()
                .map(|t| {
                    if query.case_sensitive {
                        t.clone()
                    } else {
                        t.to_lowercase()
                    }
                })
                .collect(),
        })
    }
    fn time_matches(&self, stamp: Option<&Stamp>) -> Option<bool> {
        if self.start.is_none() && self.date.is_none() {
            return Some(true);
        }
        let stamp = stamp?;
        let date_matches = match self.date {
            Some(date) => stamp.date? == date,
            None => true,
        };
        let time_matches = match (self.start, self.end) {
            (Some(a), Some(b)) if a > b => stamp.millis >= a || stamp.millis <= b,
            (Some(a), Some(b)) => (a..=b).contains(&stamp.millis),
            _ => true,
        };
        Some(date_matches && time_matches)
    }
}

fn excerpt(line: &str) -> Value {
    let mut chars = line.chars();
    let text: String = chars.by_ref().take(600).collect();
    json!({"text":text,"excerpt_truncated":chars.next().is_some()})
}

/// 批次擷取只在本地處理；不受搜尋預覽的 30 筆上限影響，也不回傳整批原文。
/// 任一來源讀取失敗即停止，不把缺檔或解碼錯誤當成完整 CSV。
pub(super) fn dataset(
    project: &Project,
    query: &Query,
    revisions: &[String],
    fields: &[super::datasets::Field],
    cancel: &AtomicBool,
) -> AppResult<(super::datasets::Table, Value)> {
    let filter = Filter::new(query)?;
    if query.context_lines != 0 || (!revisions.is_empty() && revisions.len() != query.paths.len()) {
        return Err("CSV 擷取不含前後文；revisions 請留空或依 paths 順序提供全部版本。".into());
    }
    for field in fields {
        field.validate()?;
    }
    let mut table = super::datasets::Table::new(fields.iter().map(|f| f.name.clone()).collect())?;
    let mut sources = Vec::new();
    let mut scanned = 0usize;
    let mut unknown_time = 0usize;
    let mut seen = std::collections::BTreeSet::new();
    for (i, path) in query.paths.iter().enumerate() {
        if !seen.insert(path.replace('\\', "/").to_lowercase()) {
            return Err("CSV 來源不可重複。".into());
        }
        let mut source = Source::open(project, path, revisions.get(i).map(String::as_str), cancel)?;
        let file_date = name_parts(Path::new(path)).map(|p| p.0);
        let mut current: Option<Stamp> = None;
        let before = table.rows.len();
        while let Some(line) = source.next(cancel)? {
            scanned += 1;
            if let Some(mut parsed) = stamp(&line) {
                parsed.date = parsed
                    .date
                    .or(current.as_ref().and_then(|s| s.date))
                    .or(file_date);
                current = Some(parsed);
            } else {
                let prefix = line.trim_start_matches(|c: char| c.is_whitespace() || c == '[');
                if prefix.as_bytes().first().is_some_and(u8::is_ascii_digit)
                    && prefix.chars().take(32).any(|c| c == ':')
                {
                    current = None;
                }
            }
            let matches = filter.time_matches(current.as_ref());
            if matches.is_none() {
                unknown_time += 1;
            }
            let haystack = if query.case_sensitive {
                line.clone()
            } else {
                line.to_lowercase()
            };
            if matches != Some(true)
                || !(filter.terms.is_empty() || filter.terms.iter().any(|t| haystack.contains(t)))
            {
                continue;
            }
            let values = fields
                .iter()
                .map(|f| f.extract(&line, current.as_ref().map(|s| s.millis)))
                .collect::<AppResult<Vec<_>>>()?;
            table.push(super::datasets::Row {
                path: path.clone(),
                revision: source.revision.clone(),
                sheet: 0,
                row: source.line,
                values,
                kinds: vec![],
                texts: vec![],
            })?;
        }
        sources.push(json!({"path":path,"revision":source.revision,"scanned_lines":source.line,"selected_rows":table.rows.len()-before}));
    }
    Ok((
        table,
        json!({"sources":sources,"scanned_lines":scanned,"unknown_time_excluded":unknown_time,
        "time_rule":"內文日期優先，純時間可沿用前文／檔名日期；續行沿用上一筆時間，不推斷跨午夜日期。timestamp_seconds 是當日秒數。"}),
    ))
}

/// 每次最多掃描 25 萬行、回傳 30 個命中；無命中也可能需要續頁。
/// 先驗證所有檔案的版本，避免先前頁讀過的文件在後續頁被悄悄替換。
pub(super) fn search(
    project: &Project,
    query: &Query,
    previous: Option<&Cursor>,
    cancel: &AtomicBool,
) -> AppResult<(Value, Option<Cursor>)> {
    let filter = Filter::new(query)?;
    let mut state = if let Some(previous) = previous {
        if &previous.query != query {
            return Err("續頁的檔案及搜尋條件必須與原查詢相同。".into());
        }
        previous.clone()
    } else {
        Cursor {
            query: query.clone(),
            revisions: vec![None; query.paths.len()],
            file: 0,
            line: 1,
            errors: vec![],
            unclassified: 0,
            scanned_lines: vec![0; query.paths.len()],
        }
    };
    state.scanned_lines.resize(query.paths.len(), 0);
    for (index, path) in query.paths.iter().enumerate() {
        check_cancel(cancel)?;
        if previous.is_some() && state.revisions[index].is_none() {
            continue;
        }
        match Source::open(project, path, state.revisions[index].as_deref(), cancel) {
            Ok(source) => state.revisions[index] = Some(source.revision),
            Err(error) => {
                check_cancel(cancel)?;
                if previous.is_some() || error.contains(super::interaction::BUSY) {
                    return Err(format!("{path}：{error}"));
                }
                state.errors.push(json!({"path":path,"error":error}));
            }
        }
    }
    let mut matches: Vec<Value> = Vec::new();
    let mut scanned = 0;
    let mut output_chars = 0;
    while state.file < query.paths.len() {
        let path = &query.paths[state.file];
        let Some(revision) = state.revisions[state.file].as_deref() else {
            state.file += 1;
            state.line = 1;
            continue;
        };
        let mut source = Source::open(project, path, Some(revision), cancel)?;
        let mut current_stamp = None;
        let file_date = name_parts(Path::new(path)).map(|p| p.0);
        let mut before: VecDeque<Value> = VecDeque::new();
        let mut trailing: Vec<(usize, usize)> = Vec::new();
        let mut resume_at = None;
        let mut eof = false;
        loop {
            let line = match source.next(cancel) {
                Ok(Some(line)) => line,
                Ok(None) => {
                    eof = true;
                    break;
                }
                Err(error) => {
                    check_cancel(cancel)?;
                    state
                        .errors
                        .push(json!({"path":path,"line":source.line+1,"error":error}));
                    eof = true;
                    break;
                }
            };
            let parsed = stamp(&line);
            let inherited = parsed.is_none();
            let prefix = line.trim_start_matches(|c: char| c.is_whitespace() || c == '[');
            if inherited
                && prefix.as_bytes().first().is_some_and(u8::is_ascii_digit)
                && prefix.chars().take(32).any(|c| c == ':')
            {
                // 看似新時間卻無法解析，不可誤當成上一筆事件的續行。
                current_stamp = None;
            }
            // 有日期的行後，純時間行保留日期；跨午夜的自動推斷不作為證據。
            if let Some(mut parsed) = parsed {
                if parsed.date.is_none() {
                    parsed.date = current_stamp
                        .as_ref()
                        .and_then(|s: &Stamp| s.date)
                        .or(file_date);
                }
                current_stamp = Some(parsed);
            }
            let mut row = excerpt(&line);
            row["line"] = json!(source.line);
            for (index, left) in &mut trailing {
                if *left > 0 {
                    matches[*index]["after"]
                        .as_array_mut()
                        .ok_or("LOG 前後文狀態不正確。")?
                        .push(row.clone());
                    *left -= 1;
                }
            }
            trailing.retain(|(_, left)| *left > 0);
            if source.line >= state.line {
                // 達頁面上限時，這一行留給下一頁，不遺漏任何候選紀錄。
                if matches.len() >= 30 || output_chars >= PAGE_CHARS || scanned >= SEARCH_LINES {
                    resume_at.get_or_insert(source.line);
                }
                if resume_at.is_some() && trailing.is_empty() {
                    break;
                }
                if resume_at.is_none() {
                    scanned += 1;
                    state.scanned_lines[state.file] += 1;
                    let in_time = filter.time_matches(current_stamp.as_ref());
                    if in_time.is_none() {
                        state.unclassified += 1;
                    }
                    let candidate = if query.case_sensitive {
                        line.clone()
                    } else {
                        line.to_lowercase()
                    };
                    if in_time == Some(true)
                        && (filter.terms.is_empty()
                            || filter.terms.iter().any(|t| candidate.contains(t)))
                    {
                        // 依最長摘錄預留整組前後文，不能用短命中行低估長前後文。
                        output_chars += 600 * (1 + 2 * query.context_lines);
                        let matched = json!({"path":path,"revision":source.revision,"line":source.line,"excerpt":row,"time_inherited":inherited,
                        "time_millis":current_stamp.as_ref().map(|s|s.millis),"date":current_stamp.as_ref().and_then(|s|s.date).map(|d|d.to_string()),
                        "before":before,"after":[]});
                        matches.push(matched);
                        trailing.push((matches.len() - 1, query.context_lines));
                    }
                }
            }
            before.push_back(row);
            while before.len() > query.context_lines {
                before.pop_front();
            }
        }
        if let Some(line) = resume_at {
            state.line = line;
            eof = false;
        }
        if eof {
            state.file += 1;
            state.line = 1;
        }
        if !eof || scanned >= SEARCH_LINES || matches.len() >= 30 || output_chars >= PAGE_CHARS {
            break;
        }
    }
    let has_more = state.file < query.paths.len();
    let result = json!({"matches":matches,"scanned_lines_this_page":scanned,"has_more":has_more,
        "coverage":query.paths.iter().enumerate().map(|(i,path)|json!({"path":path,"revision":state.revisions[i],"scanned_lines":state.scanned_lines[i],
            "status":if state.errors.iter().any(|e|e["path"]==*path) {"failed"} else if i<state.file {"complete"} else if i==state.file && state.line>1 {"partial"} else {"pending"}})).collect::<Vec<_>>(),
        "scan_complete":!has_more,"complete":!has_more && state.errors.is_empty() && state.unclassified==0,
        "errors":state.errors,"unclassified_time_lines":state.unclassified,
        "time_basis":"LOG 原文時間；HH:MM 結束包含該分鐘，日期未指定時套用各日；跨午夜使用時間 OR 條件",
        "context_note":"before/after 是定位前後文，可能超出時間範圍；無時間行沿用前一筆可辨識時間，已標示 time_inherited"});
    Ok((result, has_more.then_some(state)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dataset_extracts_all_rows_with_time_only_or_terms_missing_fields_and_version_checks() {
        let project = fixture();
        let cancel = AtomicBool::new(false);
        let path = "20260623_system_A01-01.log";
        std::fs::write(project.root.join(path),"unknown EVENT p=9;\n09:59:00.000 EVENT p=1;\n10:03:00.000 EVENT p=2;\n10:04:00.000 RETRY no value\n10:09:00.000 EVENT p=4;\n").unwrap();
        let mut query = Query {
            paths: vec![path.into()],
            terms: vec!["EVENT".into(), "RETRY".into()],
            start_time: Some("10:00".into()),
            end_time: Some("10:08".into()),
            date: Some("2026-06-23".into()),
            ..Default::default()
        };
        let fields = vec![super::super::datasets::Field {
            name: "p".into(),
            mode: "between".into(),
            start: Some("p=".into()),
            end: Some(";".into()),
            delimiter: None,
            index: None,
        }];
        let (table, meta) = dataset(&project, &query, &[], &fields, &cancel).unwrap();
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[0].row, 3);
        assert_eq!(table.rows[0].values[0], "2");
        assert_eq!(table.rows[1].values[0], "");
        assert_eq!(meta["unknown_time_excluded"], 1);
        assert!(dataset(
            &project,
            &query,
            &["wrong_revision".into()],
            &fields,
            &cancel
        )
        .is_err());
        query.paths.push("missing.log".into());
        assert!(dataset(&project, &query, &[], &fields, &cancel).is_err());
        query.paths.pop();
        cancel.store(true, Ordering::Relaxed);
        assert!(dataset(&project, &query, &[], &fields, &cancel).is_err());
        std::fs::remove_dir_all(project.root).unwrap();
    }
    #[test]
    fn utf8_bom_never_falls_back_to_big5() {
        let project = fixture();
        let mut bytes = vec![0xef, 0xbb, 0xbf];
        bytes.extend(text::encode("中文", text::Encoding::CodePage(950)).unwrap());
        std::fs::write(project.root.join("bad.log"), bytes).unwrap();
        assert!(read(&project, "bad.log", None, 1, 0, 1, &AtomicBool::new(false)).is_err());
    }
    fn fixture() -> Project {
        let root =
            std::env::temp_dir().join(format!("lmai-log-{}", crate::jobs::new_id().unwrap()));
        std::fs::create_dir_all(&root).unwrap();
        Project {
            id: "test".into(),
            name: "test".into(),
            root,
            imports: Default::default(),
        }
    }
    #[test]
    fn paged_search_keeps_all_adjacent_matches_and_continuations() {
        let project = fixture();
        let cancel = AtomicBool::new(false);
        let source = "2026/06/23, 12:25:48.084 ERROR 線路異常\n  detail timeout\n".repeat(50);
        std::fs::write(project.root.join("20260622_system_Z01-CY.log"), source).unwrap();
        let query = Query {
            paths: vec!["20260622_system_Z01-CY.log".into()],
            terms: vec!["error".into(), "timeout".into()],
            start_time: Some("12:25".into()),
            end_time: Some("12:33".into()),
            date: Some("2026-06-23".into()),
            context_lines: 5,
            ..Default::default()
        };
        let mut cursor = None;
        let mut numbers = Vec::new();
        loop {
            let (result, next) = search(&project, &query, cursor.as_ref(), &cancel).unwrap();
            numbers.extend(
                result["matches"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|r| r["line"].as_u64().unwrap()),
            );
            if next.is_none() {
                assert_eq!(result["complete"], true);
                break;
            }
            assert!(numbers.len() <= 100);
            cursor = next;
        }
        assert_eq!(numbers, (1..=100).collect::<Vec<_>>());
    }
    #[test]
    fn encodings_file_dates_and_partial_failures_are_explicit() {
        let project = fixture();
        let cancel = AtomicBool::new(false);
        let line = "12:25:48.084 ERROR 測試\r\n";
        for (name, encoding) in [
            ("20260623_system_A01-01.log", text::Encoding::Utf16(true)),
            ("20260623_system_Z01-CY.log", text::Encoding::CodePage(950)),
        ] {
            std::fs::write(
                project.root.join(name),
                text::encode(line, encoding).unwrap(),
            )
            .unwrap();
        }
        let query = Query {
            paths: vec![
                "20260623_system_A01-01.log".into(),
                "20260623_system_Z01-CY.log".into(),
                "missing.log".into(),
            ],
            start_time: Some("12:25".into()),
            end_time: Some("12:33".into()),
            date: Some("2026-06-23".into()),
            ..Default::default()
        };
        let (result, next) = search(&project, &query, None, &cancel).unwrap();
        assert!(next.is_none());
        assert_eq!(result["matches"].as_array().unwrap().len(), 2);
        assert_eq!(result["complete"], false);
        assert_eq!(result["errors"].as_array().unwrap().len(), 1);
        std::fs::write(
            project.root.join("invalid.log"),
            "header\n2026/06/23, 12:25:48.084 ok\n2026/99/23, 12:25:48.084 ERROR\n",
        )
        .unwrap();
        let query = Query {
            paths: vec!["invalid.log".into()],
            ..query
        };
        let (result, _) = search(&project, &query, None, &cancel).unwrap();
        assert_eq!(result["unclassified_time_lines"], 2);
        assert_eq!(result["complete"], false);
        cancel.store(true, Ordering::Relaxed);
        assert!(search(&project, &query, None, &cancel).is_err());
    }
    #[test]
    fn time_ranges_are_inclusive_and_do_not_guess_body_timestamps() {
        assert_eq!(
            time("12:33", true).unwrap(),
            time("12:33:59.999", false).unwrap()
        );
        assert_eq!(
            stamp("[2026-10-05 12:25:03,123] ERROR").unwrap().millis,
            44_703_123
        );
        assert!(stamp("message says error at 12:25:03").is_none());
        assert!(stamp("2026-99-05 12:25:03").is_none());
        assert_eq!(
            stamp("2026/06/23, 15:25:48.084 message").unwrap().millis,
            55_548_084
        );
        assert_eq!(
            name_parts(Path::new("20260622_connection_Z01-CY.log"))
                .unwrap()
                .2,
            "Z01-CY"
        );
        let query = Query {
            paths: vec!["x.log".into()],
            start_time: Some("23:59".into()),
            end_time: Some("00:01".into()),
            ..Default::default()
        };
        let filter = Filter::new(&query).unwrap();
        assert_eq!(
            filter.time_matches(stamp("00:00:30 hello").as_ref()),
            Some(true)
        );
        assert_eq!(
            filter.time_matches(stamp("12:00:30 hello").as_ref()),
            Some(false)
        );
        assert_eq!(filter.time_matches(None), None);
    }

    #[test]
    fn approximate_window_finds_time_only_anchor_with_comma_separator() {
        let project = fixture();
        let path = "20260622_system_A01-01.log";
        std::fs::write(project.root.join(path),
            "09:54:00.000, 待機\n10:03:21.084, 上料判定\n  批號 A123\n10:04:00,084, 作動\n10:06, 完成\n").unwrap();
        let mut query = Query {
            paths: vec![path.into()],
            start_time: Some("09:55".into()),
            end_time: Some("10:05".into()),
            date: Some("2026-06-22".into()),
            ..Default::default()
        };
        let cancel = AtomicBool::new(false);
        let (result, next) = search(&project, &query, None, &cancel).unwrap();
        assert!(next.is_none());
        assert_eq!(result["complete"], true);
        let lines: Vec<_> = result["matches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["line"].as_u64().unwrap())
            .collect();
        assert_eq!(lines, vec![2, 3, 4]);
        assert_eq!(result["matches"][0]["date"], "2026-06-22");
        assert_eq!(result["matches"][1]["time_inherited"], true);
        // 沒有可用日期時仍可搜尋時間，但嚴格日期篩選不能冒充完整。
        std::fs::copy(project.root.join(path), project.root.join("unknown.log")).unwrap();
        query.paths = vec!["unknown.log".into()];
        let (strict, _) = search(&project, &query, None, &cancel).unwrap();
        assert_eq!(strict["complete"], false);
        assert!(strict["matches"].as_array().unwrap().is_empty());
        query.date = None;
        let (undated, _) = search(&project, &query, None, &cancel).unwrap();
        assert_eq!(undated["matches"].as_array().unwrap().len(), 3);
        assert!(undated["matches"][0]["date"].is_null());
    }
}
