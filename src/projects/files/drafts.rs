//! 任務擁有的持續草稿：同一路徑更新，原件與其他工具成果仍不允許覆寫。
//! 更新前以獨占 handle 核對上一版雜湊，寫入／flush／讀回成功才提交新狀態。
use super::*;
use sha2::{Digest, Sha256};

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct Draft {
    pub path: String,
    pub sha256: String,
}

impl Broker {
    /// 續接前核對專屬目錄及磁碟雜湊，不以舊筆記覆蓋使用者修改。
    pub(super) fn verify_draft(&self, draft: &Draft, folder: Option<&str>) -> AppResult<()> {
        let path = relative(&draft.path)?;
        let folder = folder.ok_or("草稿缺少專屬輸出目錄。")?;
        if path.parent() != Some(Path::new("_AI_Output").join(folder).as_path()) {
            return Err("草稿路徑不屬於本次輸出目錄。".into());
        }
        let full = self.project.root.join(path);
        let _guards = pin(full.parent().ok_or("草稿缺少資料夾。")?)?;
        let file = checked_file(&full)?;
        reject_internal(&file)?;
        let mut bytes = Vec::new();
        file.take(1_000_001)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if format!("{:x}", Sha256::digest(&bytes)) != draft.sha256 {
            return Err(format!("草稿 {} 已被外部修改；未恢復覆寫。", draft.path));
        }
        Ok(())
    }

    pub(super) fn write_draft(
        &mut self,
        name: &str,
        previous: Option<&Draft>,
        bytes: &[u8],
    ) -> AppResult<Draft> {
        if relative(name)?.components().count() != 1 || bytes.len() > 1_000_000 {
            return Err("草稿名稱或內容超過限制。".into());
        }
        let _root = pin(&self.project.root)?;
        let base = self.project.root.join("_AI_Output");
        match fs::create_dir(&base) {
            Ok(()) => (),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(error) => return Err(error.to_string()),
        }
        let _base = pin(&base)?;
        if self.output_folder.is_none() {
            self.output_folder = Some(create_output_folder(&base)?);
        }
        let folder_name = self.output_folder.as_deref().ok_or("缺少草稿輸出目錄。")?;
        let folder = base.join(folder_name);
        let _folder = pin(&folder)?;
        let (path, mut file, old) = if let Some(previous) = previous {
            let rel = relative(&previous.path)?;
            if rel.parent() != Some(Path::new("_AI_Output").join(folder_name).as_path()) {
                return Err("草稿不屬於本次輸出目錄。".into());
            }
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .share_mode(0)
                .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
                .open(self.project.root.join(rel))
                .map_err(|e| format!("無法更新草稿，請關閉占用草稿的程式：{e}"))?;
            let mut info = BY_HANDLE_FILE_INFORMATION::default();
            if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0
                || info.dwFileAttributes & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY)
                    != 0
                || info.nNumberOfLinks != 1
            {
                return Err("草稿已變成連結或無法驗證，未覆寫。".into());
            }
            reject_internal(&file)?;
            let mut old = Vec::new();
            (&file)
                .take(1_000_001)
                .read_to_end(&mut old)
                .map_err(|e| e.to_string())?;
            if format!("{:x}", Sha256::digest(&old)) != previous.sha256 {
                return Err("草稿已由其他程式修改，未覆寫；請保留外部修改後重新建立任務。".into());
            }
            (previous.path.clone(), file, old)
        } else {
            let (actual, file) = reserve_output(&folder, name)?;
            (
                format!("_AI_Output/{folder_name}/{actual}"),
                file,
                Vec::new(),
            )
        };
        // 固定 handle 寫入，不能先刪原檔再重建；已知 I/O 失敗盡量回復上一版。
        let write = |file: &mut File, value: &[u8]| -> AppResult<()> {
            file.rewind()
                .and_then(|()| file.write_all(value))
                .and_then(|()| file.set_len(value.len() as u64))
                .and_then(|()| file.sync_all())
                .map_err(|e| e.to_string())?;
            file.rewind().map_err(|e| e.to_string())?;
            let mut actual = Vec::new();
            Read::by_ref(file)
                .take(1_000_001)
                .read_to_end(&mut actual)
                .map_err(|e| e.to_string())?;
            if actual != value {
                return Err("草稿讀回內容不一致。".into());
            }
            Ok(())
        };
        if let Err(error) = write(&mut file, bytes) {
            let restored = write(&mut file, &old).is_ok();
            return Err(format!(
                "草稿 {path} 未完成更新：{error}；上一版回復{}。",
                if restored {
                    "成功"
                } else {
                    "未確認，請保留檔案"
                }
            ));
        }
        Ok(Draft {
            path,
            sha256: format!("{:x}", Sha256::digest(bytes)),
        })
    }

    pub(super) fn save_python_draft(&mut self, id: &str, next: &str) -> AppResult<Value> {
        let copy = self.copies.get(id).ok_or("工作副本不存在。")?;
        if copy.text == next && copy.draft.is_some() {
            return Ok(
                json!({"copy_id":id,"revision":text::revision(next),"draft_path":copy.draft.as_ref().map(|d|&d.path),"changed":false,"draft_saved":true,"notice":"內容未變更；請修改下一個尚未完成的區段。"}),
            );
        }
        let (name, previous, encoding) = (copy.name.clone(), copy.draft.clone(), copy.encoding);
        let bytes = text::encode(next, encoding)?;
        let draft = self.write_draft(&name, previous.as_ref(), &bytes)?;
        let copy = self.copies.get_mut(id).ok_or("工作副本不存在。")?;
        copy.draft = Some(draft.clone());
        copy.text = next.into();
        copy.python_checked_revision = None;
        copy.saved_revision = None;
        // 已完成的同一路徑再次修改時，先退回草稿；不能在停止或交付時
        // 將尚未重新檢查的內容沿用上一版「完成成果」標記。
        copy.paths.retain(|path| path != &draft.path);
        self.published.retain(|path| path != &draft.path);
        Ok(
            json!({"copy_id":id,"revision":text::revision(next),"draft_path":draft.path,
            "changed":true,"draft_saved":true,"syntax_checked":false,"notice":"這次修改已更新同一份草稿；尚未完成語法檢查。下一步修改相關小段，勿重新建立副本或重讀全文。"}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incremental_draft_survives_resume_and_rejects_external_changes() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".build")
            .join(format!("draft-{}", crate::jobs::new_id().unwrap()));
        fs::create_dir_all(&root).unwrap();
        let project = Project {
            id: "draft".into(),
            name: "draft".into(),
            root: root.clone(),
            imports: Default::default(),
        };
        let mut broker = Broker::new(project.clone(), "run".into()).unwrap();
        broker.copies.insert(
            "copy".into(),
            Copy {
                name: "draft.py".into(),
                text: "pass\r\n".into(),
                encoding: Encoding::Utf8(false),
                office: None,
                saved_revision: None,
                python_checked_revision: None,
                draft: None,
                paths: vec![],
            },
        );
        let result = broker
            .edit_code_section(
                "copy",
                &text::revision("pass\r\n"),
                1,
                1,
                &text::revision("pass\r\n"),
                "def unfinished(:\n",
            )
            .unwrap();
        let path = result["draft_path"].as_str().unwrap().to_owned();
        assert_eq!(
            fs::read_to_string(root.join(&path)).unwrap(),
            "def unfinished(:\r\n"
        );
        assert_eq!(result["syntax_checked"], false);
        let mut resumed = Broker::new(project, "run".into()).unwrap();
        resumed
            .restore(broker.saved().unwrap(), &AtomicBool::new(false))
            .unwrap();
        // 模擬上一版已發布，接續修改必須撤回完成標記，但保留同一草稿檔。
        resumed.published.push(path.clone());
        resumed
            .copies
            .get_mut("copy")
            .unwrap()
            .paths
            .push(path.clone());
        let result = resumed
            .edit_code_section(
                "copy",
                result["revision"].as_str().unwrap(),
                1,
                1,
                &text::revision("def unfinished(:\r\n"),
                "def finished():\n    return 42\n",
            )
            .unwrap();
        assert_eq!(result["draft_path"], path);
        assert!(resumed.published.is_empty());
        assert!(resumed.copies["copy"].paths.is_empty());
        assert_eq!(
            fs::read_dir(root.join(&path).parent().unwrap())
                .unwrap()
                .count(),
            1
        );
        assert_eq!(
            fs::read_to_string(root.join(&path)).unwrap(),
            "def finished():\r\n    return 42\r\n"
        );
        assert!(resumed
            .edit_code_section("copy", "stale", 1, 1, "hash", "pass")
            .is_err());
        fs::write(root.join(&path), "user change").unwrap();
        assert!(resumed.save_python_draft("copy", "pass\r\n").is_err());
        assert_eq!(fs::read_to_string(root.join(&path)).unwrap(), "user change");
        fs::remove_dir_all(root).unwrap();
    }
}
