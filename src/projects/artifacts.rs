//! 成果索引及可攜交付包。所有檔案仍留原位；只接受已登記的_AI_Output成果。
use super::{files, Project};
use crate::AppResult;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::Path,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub path: String,
    pub sha256: String,
    pub run: String,
    pub conversation: String,
    pub created: u64,
    pub sources: BTreeMap<String, String>,
    pub legacy: bool,
}
pub fn bytes(project: &Project, path: &str) -> AppResult<Vec<u8>> {
    let relative = files::relative(path)?;
    if !relative.starts_with("_AI_Output") || relative.components().count() < 3 {
        return Err("成果只能位於本專案_AI_Output內。".into());
    }
    let full = project.root.join(relative);
    let _pins = files::pin(full.parent().ok_or("成果缺少目錄。")?)?;
    let mut file = files::checked_file(&full)?;
    files::reject_internal(&file)?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("單份成果超過64MiB，未載入交付包。".into());
    }
    Ok(bytes)
}
pub fn entry(
    project: &Project,
    path: &str,
    run: &str,
    conversation: &str,
    sources: BTreeMap<String, String>,
    legacy: bool,
) -> AppResult<Entry> {
    let bytes = bytes(project, path)?;
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    let id = super::text::revision(&format!("{run}:{path}:{sha256}"));
    Ok(Entry {
        id,
        path: path.into(),
        sha256,
        run: run.into(),
        conversation: conversation.into(),
        created: crate::unix_now(),
        sources,
        legacy,
    })
}
pub fn status(project: &Project, entry: &Entry) -> Value {
    status_detail(project, entry, true)
}

/// 畫面只顯示短預覽；可攜交付清單保留完整來源及變更紀錄，供離線追溯。
fn status_detail(project: &Project, entry: &Entry, preview: bool) -> Value {
    let output =
        bytes(project, &entry.path).map(|b| format!("{:x}", Sha256::digest(&b)) == entry.sha256);
    let changed: Vec<_> = entry
        .sources
        .iter()
        .filter_map(|(path, revision)| match files::fingerprint(project, path) {
            Ok(current) if &current == revision => None,
            _ => Some(path),
        })
        .collect();
    let mut displayed = serde_json::to_value(entry).unwrap_or(Value::Null);
    let source_limit = if preview { 3 } else { entry.sources.len() };
    let changed_limit = if preview { 8 } else { changed.len() };
    displayed["sources"] = json!(entry
        .sources
        .iter()
        .take(source_limit)
        .collect::<BTreeMap<_, _>>());
    json!({"entry":displayed,"sources_omitted":entry.sources.len().saturating_sub(source_limit),"artifact_status":match output{Ok(true)=>"verified",Ok(false)=>"changed",Err(_)=>"unavailable"},
        "source_status":if entry.legacy || entry.sources.is_empty(){"unknown"}else if changed.is_empty(){"unchanged"}else{"needs_update"},
        "changed_sources":changed.iter().take(changed_limit).collect::<Vec<_>>(),"changed_source_count":changed.len(),"notice":"來源關係採整個任務的保守集合；版本核對不代表內容或外觀驗收。歷史成果缺少來源時不猜測。"})
}

/// ZIP採標準store格式（不壓縮），不呼叫Shell／PowerShell或增加離線依賴。
/// CRC32及中央目錄均對本次已核對位元組計算；包內名稱不含使用者磁碟路徑。
fn zip(files: &[(String, Vec<u8>)]) -> AppResult<Vec<u8>> {
    let mut out = Vec::new();
    let mut directory = Vec::new();
    let u16le = |out: &mut Vec<u8>, n: u16| out.extend(n.to_le_bytes());
    let u32le = |out: &mut Vec<u8>, n: u32| out.extend(n.to_le_bytes());
    for (name, bytes) in files {
        if name.len() > 65535
            || bytes.len() > 64 * 1024 * 1024
            || out.len() + bytes.len() > 128 * 1024 * 1024
        {
            return Err("交付包超過128MiB或單檔64MiB上限。".into());
        }
        let mut crc = 0xffff_ffffu32;
        for byte in bytes {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xedb8_8320u32 & (0u32.wrapping_sub(crc & 1)));
            }
        }
        crc = !crc;
        let offset = out.len() as u32;
        u32le(&mut out, 0x04034b50);
        u16le(&mut out, 20);
        u16le(&mut out, 0x0800);
        u16le(&mut out, 0);
        u16le(&mut out, 0);
        u16le(&mut out, 33);
        u32le(&mut out, crc);
        u32le(&mut out, bytes.len() as u32);
        u32le(&mut out, bytes.len() as u32);
        u16le(&mut out, name.len() as u16);
        u16le(&mut out, 0);
        out.extend(name.as_bytes());
        out.extend(bytes);
        u32le(&mut directory, 0x02014b50);
        u16le(&mut directory, 20);
        u16le(&mut directory, 20);
        u16le(&mut directory, 0x0800);
        u16le(&mut directory, 0);
        u16le(&mut directory, 0);
        u16le(&mut directory, 33);
        u32le(&mut directory, crc);
        u32le(&mut directory, bytes.len() as u32);
        u32le(&mut directory, bytes.len() as u32);
        u16le(&mut directory, name.len() as u16);
        u16le(&mut directory, 0);
        u16le(&mut directory, 0);
        u16le(&mut directory, 0);
        u16le(&mut directory, 0);
        u32le(&mut directory, 0);
        u32le(&mut directory, offset);
        directory.extend(name.as_bytes());
    }
    let start = out.len();
    out.extend(&directory);
    u32le(&mut out, 0x06054b50);
    u16le(&mut out, 0);
    u16le(&mut out, 0);
    u16le(&mut out, files.len() as u16);
    u16le(&mut out, files.len() as u16);
    u32le(&mut out, directory.len() as u32);
    u32le(&mut out, start as u32);
    u16le(&mut out, 0);
    Ok(out)
}
pub fn export(project: &Project, entries: &[Entry]) -> AppResult<String> {
    if entries.is_empty() || entries.len() > 100 {
        return Err("每次選擇1–100份成果。".into());
    }
    let mut files = Vec::new();
    let mut manifest = Vec::new();
    for entry in entries {
        let data = bytes(project, &entry.path)?;
        if format!("{:x}", Sha256::digest(&data)) != entry.sha256 {
            return Err("成果已變更，未打包；請重新確認版本。".into());
        }
        let name = Path::new(&entry.path)
            .file_name()
            .ok_or("成果檔名無效。")?
            .to_string_lossy();
        files.push((format!("{}_{}", &entry.id[..12], name), data));
        manifest.push(status_detail(project, entry, false));
    }
    files.push((
        "manifest.json".into(),
        serde_json::to_vec_pretty(
            &json!({"format":"lmai-artifacts-v1","created":crate::unix_now(),"artifacts":manifest}),
        )
        .map_err(|e| e.to_string())?,
    ));
    let data = zip(&files)?;
    let base = project.root.join("_AI_Output");
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    let _base = files::pin(&base)?;
    let folder = format!("交付_{}", crate::jobs::new_id()?);
    std::fs::create_dir(base.join(&folder)).map_err(|e| e.to_string())?;
    let _folder = files::pin(&base.join(&folder))?;
    let relative = format!("_AI_Output/{folder}/成果交付.zip");
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(project.root.join(&relative))
        .map_err(|e| e.to_string())?;
    file.write_all(&data)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    use std::io::Seek;
    file.rewind().map_err(|e| e.to_string())?;
    let mut check = Vec::new();
    file.read_to_end(&mut check).map_err(|e| e.to_string())?;
    if check != data {
        return Err("交付包讀回不符。".into());
    }
    Ok(relative)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portable_manifest_keeps_sources_beyond_the_ui_preview() {
        let project = Project {
            id: "manifest".into(),
            name: "manifest".into(),
            root: std::env::temp_dir().join(format!("manifest-{}", crate::jobs::new_id().unwrap())),
            imports: BTreeMap::new(),
        };
        // 不建立檔案：全部來源故意不存在，確認完整清單也保留超過八筆的變更。
        let entry = Entry {
            id: "a".repeat(64),
            path: "_AI_Output/test/report.txt".into(),
            sha256: "b".repeat(64),
            run: "run".into(),
            conversation: "chat".into(),
            created: 0,
            legacy: false,
            sources: (0..12)
                .map(|n| (format!("source{n}.txt"), "c".repeat(64)))
                .collect(),
        };
        let preview = status(&project, &entry);
        let portable = status_detail(&project, &entry, false);
        assert_eq!(preview["entry"]["sources"].as_object().unwrap().len(), 3);
        assert_eq!(preview["changed_sources"].as_array().unwrap().len(), 8);
        assert_eq!(portable["entry"]["sources"].as_object().unwrap().len(), 12);
        assert_eq!(portable["changed_sources"].as_array().unwrap().len(), 12);
        assert_eq!(portable["sources_omitted"], 0);
    }
    #[test]
    fn zip_store_has_standard_crc_and_directory_offsets() {
        let data = zip(&[("data.txt".into(), b"123456789".to_vec())]).unwrap();
        assert_eq!(&data[..4], &0x04034b50u32.to_le_bytes());
        assert_eq!(
            u32::from_le_bytes(data[14..18].try_into().unwrap()),
            0xcbf43926
        );
        let end = data.len() - 22;
        assert_eq!(&data[end..end + 4], &0x06054b50u32.to_le_bytes());
        let directory = u32::from_le_bytes(data[end + 16..end + 20].try_into().unwrap()) as usize;
        assert_eq!(
            &data[directory..directory + 4],
            &0x02014b50u32.to_le_bytes()
        );
        assert_eq!(
            u16::from_le_bytes(data[end + 10..end + 12].try_into().unwrap()),
            1
        );
    }
}
