//! Port of the blueprint's tests/test_views.py.

use std::fs;

use chrono::{Duration, NaiveDate};
use dikt::store::{Change, Store, today};
use dikt::views::{report, review_reasons};
use serde_json::json;
use tempfile::TempDir;

fn store() -> (TempDir, Store) {
    let directory = TempDir::new().unwrap();
    fs::write(
        directory.path().join("tags.yaml"),
        "tags: [test-topic]\nbroad_tags: []\n",
    )
    .unwrap();
    let store = Store::new(directory.path()).unwrap();
    (directory, store)
}

fn tags() -> serde_json::Map<String, serde_json::Value> {
    json!({"tags": ["test-topic"]})
        .as_object()
        .cloned()
        .unwrap()
}

#[test]
fn daily_reports_are_saved_and_preserve_summary() {
    let (_dir, store) = store();
    let task = store.create("task", "Fix", "", &[], tags()).unwrap();
    store
        .change(
            task.id(),
            Change {
                section: Some(("Outcome".into(), "Verified".into())),
                event: "closed".into(),
                message: "Verified".into(),
                ..Change::fields(json!({"status": "done"}))
            },
        )
        .unwrap();
    let path = store.root.join(report(&store, "today", None).unwrap());
    let text = fs::read_to_string(&path).unwrap();
    assert!(
        text.contains("Tasks Completed\n\n- T-00001 Fix: Verified"),
        "{text}"
    );
    fs::write(
        &path,
        text.replace("## Summary\n\n", "## Summary\n\nUseful summary.\n\n"),
    )
    .unwrap();
    report(&store, "today", None).unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("Useful summary."));
    assert_eq!(text.matches("Tasks Completed").count(), 1);
}

#[test]
fn yesterday_and_iso_week() {
    let (_dir, store) = store();
    let yesterday = today() - Duration::days(1);
    assert!(
        report(&store, "yesterday", None)
            .unwrap()
            .contains(&yesterday.to_string())
    );
    let path = report(&store, "week", NaiveDate::from_ymd_opt(2021, 1, 1)).unwrap();
    assert_eq!(path, "reports/weekly/2020/2020-W53.md");
    assert!(store.root.join(path).exists());
}

#[test]
fn refuse_to_overwrite_unmarked_report() {
    let (_dir, store) = store();
    let path = store.root.join(report(&store, "today", None).unwrap());
    fs::write(&path, "Important handwritten notes").unwrap();
    let error = report(&store, "today", None).unwrap_err();
    assert!(error.message().contains("Refusing"));
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "Important handwritten notes"
    );
}

#[test]
fn superseded_active_evidence_flags_decision_without_changing_it() {
    let (_dir, store) = store();
    let mut knowledge = tags();
    knowledge.insert("source".into(), json!("Experiment A"));
    let original = store
        .create("knowledge", "Original evidence", "", &[], knowledge.clone())
        .unwrap();
    let mut decision_fields = tags();
    decision_fields.insert("status".into(), json!("accepted"));
    decision_fields.insert("based_on".into(), json!([original.id()]));
    let body: Vec<(&str, String)> = ["Context", "Options", "Decision", "Consequences"]
        .iter()
        .map(|name| (*name, name.to_string()))
        .collect();
    let decision = store
        .create("decision", "Use evidence", "", &body, decision_fields)
        .unwrap();
    let decision_path = fs::read_dir(store.root.join("decisions"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let before = fs::read(&decision_path).unwrap();
    knowledge.insert("source".into(), json!("Experiment B"));
    knowledge.insert("supersedes".into(), json!([original.id()]));
    let replacement = store
        .create("knowledge", "New evidence", "", &[], knowledge)
        .unwrap();
    let items = store.load().unwrap();
    assert_eq!(items[original.id()].status(), "active");
    let reasons = review_reasons(&items[decision.id()], &items, today());
    assert!(reasons.iter().any(|reason| {
        reason.contains(&format!("superseded evidence {}", original.id()))
            && reason.contains(replacement.id())
    }));
    assert!(
        fs::read_to_string(store.root.join("Backlog.md"))
            .unwrap()
            .contains(&format!("superseded evidence {}", original.id()))
    );
    assert_eq!(items[decision.id()].status(), "accepted");
    assert_eq!(fs::read(&decision_path).unwrap(), before);
}
