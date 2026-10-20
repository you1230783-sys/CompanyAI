//! 本機 JSON 搬移。預覽持有已驗證的快照，密碼與額外欄位只留在原生層。
use super::*;
use serde::de::{MapAccess, Visitor};
use std::{
    collections::BTreeSet,
    fmt,
    io::{Seek, Write},
    os::windows::fs::OpenOptionsExt,
};

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportMode {
    Add,
    Replace,
}

pub struct Preview {
    pub id: String,
    pub revision: u64,
    name: String,
    machines: Machines,
}

/// 一般 JSON map 會默默保留最後一個同名鍵；匯入清單不可因此遺失分類。
struct UniqueGroups(Machines);
impl<'de> Deserialize<'de> for UniqueGroups {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct GroupsVisitor;
        impl<'de> Visitor<'de> for GroupsVisitor {
            type Value = UniqueGroups;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("分類名稱對應機台陣列的物件")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut groups = Machines::new();
                while let Some((name, machines)) = map.next_entry::<String, Vec<Machine>>()? {
                    if groups.insert(name, machines).is_some() {
                        return Err(serde::de::Error::custom("分類名稱重複"));
                    }
                }
                Ok(UniqueGroups(groups))
            }
        }
        deserializer.deserialize_map(GroupsVisitor)
    }
}

fn count(machines: &Machines) -> usize {
    machines.values().map(Vec::len).sum()
}

/// 同分類以不分大小寫的名稱判重；來源自身重名也使用相同規則處理。
/// 截短只用在新副本名稱，且不切斷 UTF-8 字元。
fn unique_name(
    name: &str,
    names: &mut BTreeSet<String>,
    next_numbers: &mut BTreeMap<String, usize>,
) -> AppResult<(String, bool)> {
    if names.insert(name.to_lowercase()) {
        return Ok((name.to_owned(), false));
    }
    // 同一批大量重名時從上次序號接續，避免每台都從2重新掃描。
    let next = next_numbers.entry(name.to_lowercase()).or_insert(2);
    for number in *next..=100_000 {
        *next = number + 1;
        let suffix = format!("（匯入 {number}）");
        let mut end = name.len().min(256 - suffix.len());
        while !name.is_char_boundary(end) {
            end -= 1;
        }
        let candidate = format!("{}{suffix}", &name[..end]);
        if names.insert(candidate.to_lowercase()) {
            return Ok((candidate, true));
        }
    }
    Err("同名機台過多，請先整理來源清單。".into())
}

impl Preview {
    pub fn load(path: &Path, revision: u64) -> AppResult<Self> {
        if !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        {
            return Err("請選擇 JSON 機台清單。".into());
        }
        let bytes = read_optional(path)?.ok_or("選取的 JSON 檔已不存在。")?;
        let incoming = decode::<UniqueGroups>(&bytes)?.0;
        if count(&incoming) > 20_000 {
            return Err("匯入機台上限為 20000 台，請拆分清單。".into());
        }
        let mut machines = Machines::new();
        for (group, items) in incoming {
            let group = group.trim().to_owned();
            // 正規化後同名分類也拒絕，避免使用者誤以為兩分類都已保留。
            if machines.contains_key(&group) {
                return Err("JSON 含去除頭尾空白後重複的分類名稱，請先整理。".into());
            }
            validate_machine(&group, "分類驗證", "", "")?;
            let mut validated = Vec::with_capacity(items.len());
            for mut machine in items {
                machine.name = machine.name.trim().to_owned();
                machine.ip = machine.ip.trim().to_owned();
                // 驗證錯誤只描述規則，不回顯原始資料或密碼。
                validate_machine(&group, &machine.name, &machine.ip, &machine.password)?;
                validated.push(machine);
            }
            machines.insert(group, validated);
        }
        Ok(Self {
            id: crate::jobs::new_id()?,
            revision,
            name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            machines,
        })
    }

    pub fn merged(&self, current: &Machines, mode: ImportMode) -> AppResult<(Machines, usize)> {
        let mut result = match mode {
            ImportMode::Add => current.clone(),
            ImportMode::Replace => Machines::new(),
        };
        let mut renamed = 0;
        for (group, incoming) in &self.machines {
            let items = result.entry(group.clone()).or_default();
            let mut names = items.iter().map(|m| m.name.to_lowercase()).collect();
            let mut next_numbers = BTreeMap::new();
            for machine in incoming {
                let mut machine = machine.clone();
                let (name, changed) = unique_name(&machine.name, &mut names, &mut next_numbers)?;
                machine.name = name;
                renamed += usize::from(changed);
                items.push(machine);
            }
        }
        // 合併後也受原本的儲存容量限制；預覽成功不保證磁碟一定可寫。
        encode(&result)?;
        Ok((result, renamed))
    }

    pub fn public(&self, manager: &Manager) -> Value {
        let describe = |mode| match self.merged(&manager.machines, mode) {
            Ok((result, renamed)) => {
                json!({"renamed":renamed,"groups":result.len(),"machines":count(&result)})
            }
            Err(error) => json!({"error":error}),
        };
        json!({"id":self.id,"revision":self.revision,"name":self.name,
            "groups":self.machines.len(),"machines":count(&self.machines),
            "current_machines":count(&manager.machines),
            "add":describe(ImportMode::Add),"replace":describe(ImportMode::Replace)})
    }
}

/// 匯出保留原 JSON 位元組；新檔以 create_new 保留同資料夾的舊備份。
/// user_config.json 與 Viewer 設定完全由原有設定流程管理。
pub fn export(manager: &Manager, folder: &Path) -> AppResult<PathBuf> {
    manager.ensure_unchanged()?;
    let bytes = match &manager.original_machines {
        Some(bytes) => bytes.clone(),
        None => encode(&manager.machines)?,
    };
    for number in 1..=10_000 {
        let name = if number == 1 {
            "machines.json".into()
        } else {
            format!("machines_{number}.json")
        };
        let path = folder.join(name);
        let mut file = match fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .share_mode(0)
            .open(&path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("無法建立匯出檔：{error}")),
        };
        let result = (|| {
            file.write_all(&bytes)
                .and_then(|_| file.sync_all())
                .map_err(|e| e.to_string())?;
            file.rewind().map_err(|e| e.to_string())?;
            let mut actual = Vec::new();
            file.take(MAX_FILE_BYTES + 1)
                .read_to_end(&mut actual)
                .map_err(|e| e.to_string())?;
            if actual != bytes {
                return Err("匯出讀回不一致。".to_string());
            }
            Ok(())
        })();
        result
            .map_err(|error| format!("匯出未完成，{} 可能有不完整內容：{error}", path.display()))?;
        return Ok(path);
    }
    Err("同名備份過多，請選擇另一個資料夾。".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder() -> PathBuf {
        let root = std::env::current_dir()
            .unwrap()
            .join(".build")
            .join(format!("vnc-json-{}", crate::jobs::new_id().unwrap()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn add_replace_rename_and_export_preserve_passwords_and_extra_fields() {
        let root = folder();
        let current = root.join("machines.json");
        let original = br#"{"G":[{"name":"PC","ip":"127.0.0.1","password":"synthetic"}]}"#;
        fs::write(&current, original).unwrap();
        let mut manager = Manager::load(current.clone()).unwrap();
        let incoming = root.join("incoming.json");
        fs::write(&incoming,br#"{"G":[{"name":"pc","ip":"localhost","password":"test-only","custom":17},{"name":"PC","ip":"localhost"}]}"#).unwrap();
        let preview = Preview::load(&incoming, 5).unwrap();
        let (added, renamed) = preview.merged(&manager.machines, ImportMode::Add).unwrap();
        assert_eq!(renamed, 2);
        assert_eq!(added["G"][1].name, "pc（匯入 2）");
        assert_eq!(added["G"][2].name, "PC（匯入 3）");
        assert_eq!(added["G"][1].password, "test-only");
        assert_eq!(added["G"][1].extra["custom"], 17);
        let public = preview.public(&manager).to_string();
        assert!(!public.contains("test-only") && !public.contains("synthetic"));
        assert_eq!(fs::read(&current).unwrap(), original);
        let (replaced, renamed) = preview
            .merged(&manager.machines, ImportMode::Replace)
            .unwrap();
        assert_eq!(replaced["G"].len(), 2);
        assert_eq!(renamed, 1);
        manager.save_machines(added).unwrap();
        let backup = root.join("backup");
        fs::create_dir(&backup).unwrap();
        let first = export(&manager, &backup).unwrap();
        let second = export(&manager, &backup).unwrap();
        assert_ne!(first, second);
        assert_eq!(fs::read(first).unwrap(), fs::read(&current).unwrap());
        fs::write(&current, b"{}").unwrap();
        assert!(export(&manager, &backup).is_err());
    }

    #[test]
    fn malformed_duplicate_groups_and_oversized_input_never_replace_current_data() {
        let root = folder();
        let path = root.join("import.json");
        for source in [
            r#"{"G":[],"G":[]}"#,
            r#"{"G":[]," G ":[]}"#,
            r#"{"G":[{"name":"PC"}]}"#,
            r#"{"G":{}}"#,
            r#"[]"#,
        ] {
            fs::write(&path, source).unwrap();
            assert!(Preview::load(&path, 0).is_err(), "{source}");
        }
        fs::write(&path, vec![b' '; MAX_FILE_BYTES as usize + 1]).unwrap();
        assert!(Preview::load(&path, 0).is_err());
    }
}
