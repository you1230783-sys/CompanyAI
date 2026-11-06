//! 在可寫副本操作開始前取得來源／Office 擁有權；不在使用者確認卡片等待期間持鎖。
use super::*;
use crate::resource_locks::{self, Guard, Resource};

fn office_resource(name: &str) -> Option<Resource> {
    match Path::new(name)
        .extension()?
        .to_str()?
        .to_ascii_lowercase()
        .as_str()
    {
        "doc" | "docx" | "docm" => Some(Resource::Word),
        "xls" | "xlsx" | "xlsm" | "xlsb" => Some(Resource::Excel),
        "ppt" | "pptx" | "pptm" => Some(Resource::PowerPoint),
        _ => None,
    }
}

impl Broker {
    /// 先驗證來源路徑，再一次取得全部鎖；因此等待失敗時可以確定尚未執行本次操作。
    pub(super) fn operation_ownership(&self, tool: &Tool) -> AppResult<Guard> {
        let mut resources = Vec::new();
        let mut sources = Vec::new();
        match tool {
            Tool::InspectExcel { .. }
            | Tool::ReadExcelRange { .. }
            | Tool::PlanExcelAnalysis { .. }
            | Tool::ExportPlannedExcel { .. }
            | Tool::ExportExcelDataset { .. } => {
                resources.push(Resource::Excel);
            }
            Tool::ReadFile { path, .. } => resources.extend(office_resource(path)),
            Tool::EditText { copy_id, .. }
            | Tool::EditCodeSection { copy_id, .. }
            | Tool::EditOffice { copy_id, .. }
            | Tool::OfficeAction { copy_id, .. }
            | Tool::OfficeBatch { copy_id, .. }
            | Tool::WriteExcelFormulas { copy_id, .. }
            | Tool::SaveCopy { copy_id, .. } => {
                let copy = self.copies.get(copy_id).ok_or("找不到工作副本。")?;
                if let Some(super::super::review::Origin::Source { path, .. }) = &copy.origin {
                    sources.push(path.as_str());
                } else if let Some(path) = copy
                    .office
                    .as_ref()
                    .and_then(|office| office.source.as_deref())
                {
                    // 相容尚未保存 Origin 的舊版 Office 副本。
                    sources.push(path);
                }
                resources.extend(office_resource(&copy.name));
            }
            Tool::CreateWorkingCopy { source, name } => {
                sources.extend(source.as_deref());
                resources.extend(office_resource(name));
            }
            Tool::DocumentTask {
                command: super::super::documents::Command::FinishTranslation { id, .. },
            } => {
                let translation = self.translations.get(id).ok_or("找不到翻譯任務。")?;
                sources.push(&translation.source);
                resources.push(Resource::Word);
            }
            // 讀值與改圖是同一次原生操作，避免等 CAD 時先消耗座標計畫。
            Tool::CadExcelApply { path, .. } => {
                sources.extend(path.as_deref());
                resources.extend([Resource::Excel, Resource::Cad]);
            }
            Tool::CadEdit { path, .. } => {
                sources.push(path);
                resources.push(Resource::Cad);
            }
            _ => (),
        }
        for path in sources {
            let target = self.project.root.join(relative(path)?);
            let _pins = pin(target.parent().ok_or("來源缺少父目錄。")?)?;
            let file = checked_file(&target)?;
            reject_internal(&file)?;
            resources.push(Resource::file(&target, false)?);
        }
        resource_locks::acquire(resources)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{atomic::AtomicBool, mpsc, Arc};
    use std::time::Duration;

    #[test]
    fn different_copy_names_of_same_source_share_ownership() {
        let root = std::env::current_dir()
            .unwrap()
            .join(".build")
            .join(format!("source-lock-{}", crate::jobs::new_id().unwrap()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("source.txt"), "source").unwrap();
        std::fs::write(root.join("other.txt"), "other").unwrap();
        let project = Project {
            id: "p".into(),
            name: "p".into(),
            root,
            imports: Default::default(),
        };
        let a = Broker::new(project.clone(), "task_a".into()).unwrap();
        let request = Tool::CreateWorkingCopy {
            source: Some("source.txt".into()),
            name: "a.txt".into(),
        };
        let guard = a.operation_ownership(&request).unwrap();
        let (wait_tx, wait_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let other = project.clone();
        let b = std::thread::spawn(move || {
            let _context = resource_locks::Context::enter(Some(resource_locks::Context {
                label: "B".into(),
                cancel: Arc::new(AtomicBool::new(false)),
                notify: Arc::new(move |message| {
                    if message.is_some() {
                        wait_tx.send(()).unwrap();
                    }
                }),
            }));
            let broker = Broker::new(other, "task_b".into()).unwrap();
            let _guard = broker
                .operation_ownership(&Tool::CreateWorkingCopy {
                    source: Some("source.txt".into()),
                    name: "b.txt".into(),
                })
                .unwrap();
            done_tx.send(()).unwrap();
        });
        wait_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(done_rx.try_recv().is_err());
        std::thread::spawn(move || {
            let broker = Broker::new(project, "task_c".into()).unwrap();
            let _guard = broker
                .operation_ownership(&Tool::CreateWorkingCopy {
                    source: Some("other.txt".into()),
                    name: "c.txt".into(),
                })
                .unwrap();
        })
        .join()
        .unwrap();
        drop(guard);
        done_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        b.join().unwrap();
    }
}
