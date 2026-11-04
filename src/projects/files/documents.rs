//! 文件任務與既有副本共用儲存／讀回／防重播邊界，不另開一條任意寫檔入口。
use super::*;
use crate::projects::documents::{self, Command, Translation, TranslationMode};

impl Broker {
    /// 精簡／重啟後仍提供原生進度，不讓模型憑摘要重新翻譯或再次交付。
    pub(in crate::projects) fn translation_index(&self) -> AppResult<Value> {
        self.translations.iter().map(|(id,t)|Ok(json!({"id":id,"source":t.source,"language":t.language,"mode":t.mode,"revision":t.revision()?,"total":t.segments.len(),"completed":t.segments.iter().filter(|s|s.translation.is_some()).count(),"pending_copy":t.pending_copy,"output":t.output}))).collect::<AppResult<Vec<_>>>().map(Value::Array)
    }
    fn document_snapshot(
        &self,
        path: &str,
        styled: bool,
        cancel: &AtomicBool,
    ) -> AppResult<office::Snapshot> {
        let target = self.project.root.join(relative(path)?);
        let _pins = pin(target.parent().ok_or("缺少來源資料夾。")?)?;
        let file = checked_file(&target)?;
        reject_internal(&file)?;
        if file.metadata().map_err(|e| e.to_string())?.len() > 50_000_000 {
            return Err("文件超過50MB，請縮小範圍。".into());
        }
        if styled {
            office::translation::inspect(&target, cancel)
        } else {
            let (content, _) = read_cancel(&self.project, path, cancel, None, None)?;
            serde_json::from_str(&content).map_err(|_| "請選擇支援的Word或Excel文件。".into())
        }
    }

    pub(super) fn document_task(
        &mut self,
        command: &Command,
        worker: &mut Worker,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        match command {
            Command::PrepareTranslation {
                path,
                language,
                mode,
                terms,
            } => {
                if self.translations.len() >= 10 {
                    return Err("同一任務最多10份翻譯。".into());
                }
                let before = fingerprint(&self.project, path)?;
                let snapshot =
                    self.document_snapshot(path, *mode == TranslationMode::PreserveStyles, cancel)?;
                if fingerprint(&self.project, path)? != before {
                    return Err("讀取期間來源已改變，請重新準備翻譯。".into());
                }
                let translation = Translation::create(
                    path.clone(),
                    before.clone(),
                    language.clone(),
                    *mode,
                    terms.clone(),
                    snapshot,
                )?;
                let id = crate::jobs::new_id()?;
                let result = translation.page(&id, 0)?;
                self.translations.insert(id, translation);
                self.source_versions.insert(path.clone(), before);
                self.txt_context = true;
                Ok(result)
            }
            Command::TranslationPage { id, offset } => self
                .translations
                .get(id)
                .ok_or("找不到翻譯任務。")?
                .page(id, *offset),
            Command::ApplyTranslation {
                id,
                revision,
                items,
            } => {
                let translation = self.translations.get_mut(id).ok_or("找不到翻譯任務。")?;
                if fingerprint(&self.project, &translation.source)? != translation.fingerprint {
                    return Err("來源已修改，請建立新的翻譯任務。".into());
                }
                translation.apply(revision, items)?;
                translation.page(id, 0)
            }
            Command::FinishTranslation { id, name } => {
                self.finish_translation(id, name, worker, cancel)
            }
            Command::Compare {
                left,
                right,
                key_column,
            } => {
                let before = [
                    fingerprint(&self.project, left)?,
                    fingerprint(&self.project, right)?,
                ];
                let a = self.document_snapshot(left, false, cancel)?;
                let b = self.document_snapshot(right, false, cancel)?;
                if before
                    != [
                        fingerprint(&self.project, left)?,
                        fingerprint(&self.project, right)?,
                    ]
                {
                    return Err("比較期間來源已改變，結果作廢，請重試。".into());
                }
                let mut result = documents::compare(&a, &b, key_column.as_deref())?;
                result["sources"] =
                    json!([{"path":left,"sha256":before[0]}, {"path":right,"sha256":before[1]}]);
                self.source_versions.insert(left.clone(), before[0].clone());
                self.source_versions
                    .insert(right.clone(), before[1].clone());
                self.txt_context = true;
                Ok(result)
            }
        }
    }

    fn finish_translation(
        &mut self,
        id: &str,
        name: &str,
        worker: &mut Worker,
        cancel: &AtomicBool,
    ) -> AppResult<Value> {
        let translation = self.translations.get(id).ok_or("找不到翻譯任務。")?.clone();
        if let Some(output) = translation.output {
            return Ok(json!({"path":output,"already_saved":true}));
        }
        let target = self.project.root.join(relative(&translation.source)?);
        let _pins = pin(target.parent().ok_or("來源資料夾不存在。")?)?;
        let _source = checked_file(&target)?;
        if fingerprint(&self.project, &translation.source)? != translation.fingerprint {
            return Err("來源已改變，不能發布舊譯文。".into());
        }
        if relative(name)?.components().count() != 1
            || !name.to_ascii_lowercase().ends_with(".docx")
        {
            return Err("請指定單一DOCX副本檔名。".into());
        }
        let items = translation.completed()?;
        let copy_id = if let Some(copy) = translation.pending_copy {
            if self.copies.get(&copy).is_none_or(|c| c.name != name) {
                return Err("待交付副本名稱已固定，請沿用原名稱。".into());
            }
            copy
        } else {
            let source = (translation.mode == TranslationMode::PreserveStyles)
                .then_some(translation.source.clone());
            let result = self.perform(
                &Tool::CreateWorkingCopy {
                    source,
                    name: name.into(),
                },
                worker,
                cancel,
            )?;
            let copy_id = result["copy_id"].as_str().ok_or("未建立副本。")?.to_owned();
            // 全部翻譯先在候選文件重建；失敗只丟棄尚未發布的記憶體副本。
            let prepared = (|| -> AppResult<()> {
                let copy = self.copies.get(&copy_id).ok_or("副本遺失。")?;
                let mut candidate = copy.office.clone().ok_or("不是Word副本。")?;
                candidate.actions = if translation.mode == TranslationMode::PreserveStyles {
                    vec![office::Action::TranslateRuns {
                        snapshot: Box::new(translation.original.clone()),
                        items,
                    }]
                } else {
                    items
                        .chunks(100)
                        .map(|part| office::Action::WordParagraphs {
                            before: None,
                            paragraphs: part.iter().map(|s| s.translation.clone()).collect(),
                            format: Default::default(),
                        })
                        .collect()
                };
                let snapshot = render_office(&self.project, name, &candidate, None, cancel)?;
                let content = snapshot.serialize()?;
                candidate.desired = snapshot;
                let copy = self.copies.get_mut(&copy_id).ok_or("副本遺失。")?;
                copy.office = Some(candidate);
                copy.text = content;
                Ok(())
            })();
            if let Err(error) = prepared {
                self.copies.remove(&copy_id);
                return Err(error);
            }
            self.translations
                .get_mut(id)
                .ok_or("翻譯任務遺失。")?
                .pending_copy = Some(copy_id.clone());
            copy_id
        };
        let revision = text::revision(&self.copies.get(&copy_id).ok_or("副本遺失。")?.text);
        let mut result = self.perform(&Tool::SaveCopy { copy_id, revision }, worker, cancel)?;
        self.translations
            .get_mut(id)
            .ok_or("翻譯任務遺失。")?
            .output = Some(
            result["path"]
                .as_str()
                .ok_or("未回報成果路徑。")?
                .to_owned(),
        );
        result["omitted"] = json!(translation.omitted);
        result["notice"] = json!("翻譯只涵蓋已列出的正文／表格文字；新文件模式不複製圖片及版面。原樣式模式保留原稿結構，但文字長度可能造成重排。譯文尚需人工校對。");
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn translation_checkpoint_keeps_segments_and_native_resume_index() {
        let root = std::env::temp_dir().join(format!(
            "translation-checkpoint-{}",
            crate::jobs::new_id().unwrap()
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("source.docx"), b"fixture source").unwrap();
        let project = Project {
            id: "project".into(),
            name: "test".into(),
            root,
            imports: Default::default(),
        };
        let snapshot = office::Snapshot {
            scope: "Word正文".into(),
            structure: json!({}),
            formats: vec![],
            calculation: vec![],
            blocks: vec![office::Block {
                id: "p1".into(),
                label: "paragraph".into(),
                text: "original".into(),
                kind: "text".into(),
                format: Value::Null,
            }],
        };
        let mut translation = Translation::create(
            "source.docx".into(),
            fingerprint(&project, "source.docx").unwrap(),
            "English".into(),
            TranslationMode::NewDocument,
            vec![],
            snapshot,
        )
        .unwrap();
        let revision = translation.revision().unwrap();
        translation
            .apply(
                &revision,
                &[documents::Translated {
                    id: "p1".into(),
                    source: "original".into(),
                    translation: "translated".into(),
                }],
            )
            .unwrap();
        let mut broker = Broker::new(project.clone(), "task".into()).unwrap();
        broker
            .translations
            .insert("translation".into(), translation);
        let checkpoint = serde_json::to_vec(&broker.saved().unwrap()).unwrap();
        let mut resumed = Broker::new(project, "task".into()).unwrap();
        resumed
            .restore(
                serde_json::from_slice(&checkpoint).unwrap(),
                &AtomicBool::new(false),
            )
            .unwrap();
        assert_eq!(
            resumed.translation_index().unwrap(),
            broker.translation_index().unwrap()
        );
        assert_eq!(
            resumed.translations["translation"].completed().unwrap()[0].translation,
            "translated"
        );
    }
}
