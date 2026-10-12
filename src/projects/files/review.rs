//! 比較資料沿用任務加密紀錄；每份已發布成果保留自己的版本，不能套用後來的草稿。
use super::*;
use crate::projects::review::Snapshot;

impl Copy {
    pub(super) fn comparison(&self, path: &str, draft: bool) -> Snapshot {
        let before = self
            .baseline
            .clone()
            .or_else(|| self.code_review.baseline.clone())
            .or_else(|| {
                self.office
                    .as_ref()
                    .and_then(|o| o.original.as_ref())
                    .and_then(|s| s.serialize().ok())
            });
        Snapshot {
            name: self.name.clone(),
            path: path.into(),
            before_revision: before.as_ref().map(|s| text::revision(s)),
            before,
            after_revision: text::revision(&self.text),
            after: self.text.clone(),
            office: self.office.is_some(),
            draft,
        }
    }
}
impl Broker {
    pub fn review_snapshots(&self) -> Vec<Snapshot> {
        let mut result = Vec::new();
        for copy in self.copies.values() {
            result.extend(copy.reviews.values().cloned());
            if let Some(draft) = &copy.draft {
                result.push(copy.comparison(&draft.path, true));
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_review_survives_checkpoint_and_legacy_baseline_is_not_invented() {
        let project = Project {
            id: "review".into(),
            name: "review".into(),
            root: std::env::current_dir().unwrap(),
            imports: BTreeMap::new(),
        };
        let mut broker = Broker::new(project.clone(), "review".into()).unwrap();
        let mut copy = Copy {
            baseline: Some("原文".into()),
            reviews: BTreeMap::new(),
            office: None,
            name: "a.txt".into(),
            text: "第一次修改".into(),
            encoding: Encoding::Utf8(false),
            saved_revision: None,
            python_checked_revision: None,
            code_review: Default::default(),
            draft: None,
            code_sections: vec![],
            paths: vec![],
        };
        copy.reviews
            .insert("first.txt".into(), copy.comparison("first.txt", false));
        copy.text = "第二次修改".into();
        broker.copies.insert("copy".into(), copy);
        broker
            .analysis
            .timeline
            .push(json!({"path":"a.log","revision":"r1","line":1}));
        let mut restored = Broker::new(project, "review".into()).unwrap();
        restored
            .restore(broker.saved().unwrap(), &AtomicBool::new(false))
            .unwrap();
        let snapshot = restored.review_snapshots().pop().unwrap();
        assert_eq!(snapshot.before.as_deref(), Some("原文"));
        assert_eq!(snapshot.after, "第一次修改");
        assert_eq!(restored.analysis.timeline[0]["revision"], "r1");
        let copy = restored.copies.get_mut("copy").unwrap();
        copy.baseline = None;
        assert!(copy.comparison("unknown.txt", false).before.is_none());
    }
}
