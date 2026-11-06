use super::*;
use crate::projects::{files::Broker, workflows::Step, Project};
use serde_json::json;

fn grant() -> Grant {
    let template = Template {
        id: "template".into(),
        name: "固定讀取".into(),
        created: 1,
        steps: vec![Step {
            id: "read".into(),
            request: json!({"tool":"read_file","path":"source.md"}),
            result_format: None,
            bindings: BTreeMap::new(),
        }],
    };
    Grant {
        target: crate::reminders::WorkflowTarget {
            conversation: "chat".into(),
            project: "project".into(),
            principal: "owner".into(),
            root_identity: [1, 2, 3],
            template: template.id.clone(),
            revision: text::revision(&serde_json::to_string(&template).unwrap()),
        },
        template,
        parameters: BTreeMap::new(),
        due_at: 1000,
        repeat_days: 1,
        continuous_retry: true,
    }
}
fn root() -> std::path::PathBuf {
    let path = std::env::current_dir()
        .unwrap()
        .join(".build")
        .join(format!("schedule-unit-{}", jobs::new_id().unwrap()));
    fs::create_dir_all(&path).unwrap();
    path
}
#[test]
fn claims_are_unique_and_recurrence_advances_only_after_confirmed_completion() {
    let mut store = Store::default();
    let id = store.add(grant(), 1).unwrap();
    assert!(store.claim(&id, 999).is_err());
    let (entry, execution) = store.claim(&id, 1000).unwrap();
    assert!(store.claim(&id, 1000).is_err());
    let mut journal = Journal::new(execution.clone(), &entry.grant).unwrap();
    journal.phase = Phase::Completed;
    store.reconcile(&execution, &journal, 1001).unwrap();
    assert_eq!(store.entries[0].next_due, Some(87400));
    assert!(store.claim(&id, 1001).is_err());
    let (_, next) = store.claim(&id, 87400).unwrap();
    assert_ne!(execution, next);
}

#[test]
fn preflight_skip_advances_recurring_schedule_but_never_replays_uncertain_work() {
    for (repeat, disabled, step) in [(1, false, 0), (0, false, 0), (1, true, 0), (1, false, 1)] {
        let mut store = Store::default();
        let mut grant = grant();
        grant.repeat_days = repeat;
        let id = store.add(grant, 1).unwrap();
        let (entry, execution) = store.claim(&id, 1000).unwrap();
        let mut journal = Journal::new(execution.clone(), &entry.grant).unwrap();
        journal.phase = Phase::Skipped;
        journal.next = step;
        store.entries[0].enabled = !disabled;
        store.reconcile(&execution, &journal, 1001).unwrap();
        assert_eq!(
            store.entries[0].enabled,
            repeat == 1 && !disabled && step == 0
        );
        assert_eq!(store.entries[0].execution.is_none(), step == 0);
        assert!(journal.can_resume(&entry.grant).is_err());
    }
}
#[test]
fn missed_runs_are_skipped_and_single_shots_do_not_reappear() {
    let mut store = Store::default();
    store.add(grant(), 1).unwrap();
    let mut single = grant();
    single.repeat_days = 0;
    store.add(single, 1).unwrap();
    assert!(!store.skip_missed(1300).unwrap());
    assert!(store.skip_missed(1301).unwrap());
    assert_eq!(store.entries[0].next_due, Some(87400));
    assert!(!store.entries[1].enabled);
    assert_eq!(store.audit.len(), 2);
    assert!(store
        .audit
        .iter()
        .all(|a| a.next_step == 0 && a.outputs.is_empty()));
    assert!(!store.skip_missed(1302).unwrap());
    assert_eq!(store.audit.len(), 2);
}
#[test]
fn unknown_and_blocked_steps_cannot_resume_or_start_next_recurrence() {
    for phase in [
        Phase::Preparing,
        Phase::InFlight,
        Phase::Blocked,
        Phase::Completed,
    ] {
        let mut journal = Journal::new("execution".into(), &grant()).unwrap();
        journal.phase = phase;
        assert!(journal.can_resume(&grant()).is_err());
    }
    let mut store = Store::default();
    let id = store.add(grant(), 1).unwrap();
    let (entry, execution) = store.claim(&id, 1000).unwrap();
    let mut journal = Journal::new(execution.clone(), &entry.grant).unwrap();
    journal.phase = Phase::InFlight;
    store.reconcile(&execution, &journal, 1001).unwrap();
    assert!(!store.entries[0].enabled);
    assert!(store.claim(&id, 87400).is_err());
}
#[test]
fn journals_cannot_be_overwritten_and_grant_changes_invalidate_resume() {
    let root = root();
    let grant = grant();
    let mut journal = Journal::new("execution".into(), &grant).unwrap();
    journal.create(&root).unwrap();
    assert!(journal.create(&root).is_err());
    let broker = Broker::new(
        Project {
            id: "project".into(),
            name: "測試".into(),
            root: root.clone(),
            imports: BTreeMap::new(),
        },
        "execution".into(),
    )
    .unwrap();
    journal.phase = Phase::Ready;
    journal.broker = Some(broker.saved().unwrap());
    journal.save(&root).unwrap();
    let restored = Journal::load(&root, "execution").unwrap();
    assert!(restored.can_resume(&grant).is_ok());
    let mut changed = grant;
    changed.target.principal = "different".into();
    assert!(restored.can_resume(&changed).is_err());
    fs::write(root.join(STORE_FILE), b"corrupt").unwrap();
    assert!(Store::load(&root).is_err());
}
#[test]
fn persisted_claim_recovers_without_new_execution_or_identity_guessing() {
    let root = root();
    let mut store = Store::default();
    let id = store.add(grant(), 1).unwrap();
    let (entry, execution) = store.claim(&id, 1000).unwrap();
    store.save(&root).unwrap();
    let mut restored = Store::load(&root).unwrap();
    assert!(restored.claim(&id, 1000).is_err());
    let mut journal = Journal::new(execution.clone(), &entry.grant).unwrap();
    journal.phase = Phase::InFlight;
    let mut tampered = entry.grant.clone();
    tampered.target.principal = "other".into();
    journal.grant_revision = grant_revision(&tampered).unwrap();
    assert!(restored.reconcile(&execution, &journal, 1001).is_err());
}
