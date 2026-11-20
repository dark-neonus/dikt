//! Store behaviour: validation, lifecycle, tag policy and transactional writes.

use std::fs;

use dikt::store::{Change, Store, parse_item};
use serde_json::{Value, json};
use tempfile::TempDir;

fn store() -> (TempDir, Store) {
    let directory = TempDir::new().unwrap();
    fs::write(
        directory.path().join("tags.yaml"),
        "tags: [pm, python, test-topic]\nbroad_tags: [pm, python]\n",
    )
    .unwrap();
    let store = Store::new(directory.path()).unwrap();
    (directory, store)
}

fn fields(value: Value) -> serde_json::Map<String, Value> {
    value.as_object().cloned().unwrap()
}

fn create(store: &Store, kind: &str, title: &str, extra: Value) -> dikt::Result<dikt::store::Item> {
    store.create(kind, title, "", &[], fields(extra))
}

fn md_files(store: &Store, folder: &str) -> Vec<std::path::PathBuf> {
    let mut paths: Vec<_> = fs::read_dir(store.root.join(folder))
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .filter(|p| p.extension().is_some_and(|e| e == "md"))
                .collect()
        })
        .unwrap_or_default();
    paths.sort();
    paths
}

fn assert_err<T: std::fmt::Debug>(result: dikt::Result<T>, needle: &str) {
    let error = result.expect_err("expected an error");
    assert!(
        error.message().contains(needle),
        "{:?} does not contain {needle:?}",
        error.message()
    );
}

#[test]
fn round_trip_and_stable_id_on_rename() {
    let (_dir, store) = store();
    let item = store
        .create(
            "task",
            "Fix export",
            "Unicode: caf\u{e9}\n\n```python\n## Not a section\n```",
            &[],
            fields(json!({"tags": ["pm", "test-topic"]})),
        )
        .unwrap();
    let original = md_files(&store, "tasks").remove(0);
    assert!(
        parse_item(&original)
            .unwrap()
            .section("Description")
            .ends_with("```")
    );
    store
        .change(
            item.id(),
            Change::fields(json!({"title": "Fix report export"})),
        )
        .unwrap();
    assert!(!original.exists());
    assert_eq!(
        store.load().unwrap().keys().collect::<Vec<_>>(),
        [item.id()]
    );
    assert!(
        store
            .root
            .join(format!("tasks/{}-fix-report-export.md", item.id()))
            .exists()
    );
    assert!(
        fs::read_to_string(store.root.join("Backlog.md"))
            .unwrap()
            .contains("Fix report export")
    );
}

#[test]
fn invalid_create_has_no_side_effects() {
    for extra in [
        json!({"tags": ["unknown"]}),
        json!({"priority": 8}),
        json!({"due": "2026-02-30"}),
        json!({"unexpected": true}),
    ] {
        let (_dir, store) = store();
        let mut all = fields(json!({"tags": ["test-topic"]}));
        all.extend(fields(extra));
        assert!(store.create("task", "Invalid", "", &[], all).is_err());
        assert!(md_files(&store, "tasks").is_empty());
        assert!(!store.root.join("Backlog.md").exists());
    }
}

#[test]
fn unknown_reference_rejected() {
    let (_dir, store) = store();
    assert_err(
        create(
            &store,
            "decision",
            "Pick",
            json!({"tags": ["test-topic"], "based_on": ["K-00042"]}),
        ),
        "reference",
    );
}

#[test]
fn promote_is_linked_and_cannot_repeat() {
    let (_dir, store) = store();
    let idea = create(
        &store,
        "idea",
        "Try search",
        json!({"tags": ["test-topic"]}),
    )
    .unwrap();
    let task = store.promote(idea.id()).unwrap();
    let items = store.load().unwrap();
    assert_eq!(items[idea.id()].str("promoted_to"), task.id());
    assert_eq!(task.links("items"), [idea.id()]);
    assert!(store.promote(idea.id()).is_err());
}

fn accepted(store: &Store, title: &str) -> dikt::store::Item {
    let body: Vec<(&str, String)> = ["Context", "Options", "Decision", "Consequences"]
        .iter()
        .map(|name| (*name, format!("{name} of {title}")))
        .collect();
    store
        .create(
            "decision",
            title,
            "",
            &body,
            fields(json!({"status": "accepted", "tags": ["test-topic"]})),
        )
        .unwrap()
}

#[test]
fn decision_rationale_and_supersession() {
    let (_dir, store) = store();
    let (old, new) = (accepted(&store, "Old"), accepted(&store, "New"));
    assert_err(
        store.change(
            old.id(),
            Change {
                section: Some(("Decision".into(), "Changed mind".into())),
                ..Default::default()
            },
        ),
        "immutable",
    );
    store.supersede(old.id(), new.id()).unwrap();
    let items = store.load().unwrap();
    assert_eq!(items[old.id()].status(), "superseded");
    assert_eq!(items[new.id()].list("supersedes"), [old.id()]);
    assert_eq!(items[old.id()].section("Decision"), "Decision of Old");
}

#[test]
fn failed_update_preserves_item_and_log() {
    let (_dir, store) = store();
    let task = create(&store, "task", "Original", json!({"tags": ["test-topic"]})).unwrap();
    let before = fs::read(&md_files(&store, "tasks")[0]).unwrap();
    assert!(
        store
            .change(task.id(), Change::fields(json!({"due": "not a date"})))
            .is_err()
    );
    assert_eq!(fs::read(&md_files(&store, "tasks")[0]).unwrap(), before);
    assert!(
        store
            .change(
                task.id(),
                Change {
                    section: Some(("Log".into(), "erase".into())),
                    ..Default::default()
                }
            )
            .is_err()
    );
}

#[test]
fn write_lock_rejects_concurrent_writer() {
    let (_dir, store) = store();
    let _lock = store.write_lock().unwrap();
    assert_err(
        create(
            &store,
            "task",
            "Concurrent",
            json!({"tags": ["test-topic"]}),
        ),
        "Another dikt write",
    );
}

#[test]
fn checklist_and_outcome() {
    let (_dir, store) = store();
    let task = create(&store, "task", "Work", json!({"tags": ["test-topic"]})).unwrap();
    store
        .change(
            task.id(),
            Change {
                checklist: Some("Reproduce".into()),
                ..Default::default()
            },
        )
        .unwrap();
    store
        .change(
            task.id(),
            Change {
                check: Some(1),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(
        store
            .change(task.id(), Change::fields(json!({"status": "done"})))
            .is_err()
    );
    store
        .change(
            task.id(),
            Change {
                section: Some(("Outcome".into(), "Fixed and tested".into())),
                event: "closed".into(),
                ..Change::fields(json!({"status": "done"}))
            },
        )
        .unwrap();
    assert_eq!(
        store.load().unwrap()[task.id()].metadata["checklist"][0]["done"],
        json!(true)
    );
}

#[test]
fn knowledge_can_supersede_knowledge() {
    let (_dir, store) = store();
    let original = create(
        &store,
        "knowledge",
        "Original",
        json!({"source": "Experiment A", "tags": ["test-topic"]}),
    )
    .unwrap();
    let replacement = create(
        &store,
        "knowledge",
        "Replacement",
        json!({"source": "Experiment B", "tags": ["test-topic"], "supersedes": [original.id()]}),
    )
    .unwrap();
    store
        .change(original.id(), Change::fields(json!({"status": "outdated"})))
        .unwrap();
    assert_eq!(
        store.load().unwrap()[replacement.id()].list("supersedes"),
        [original.id()]
    );
}

#[test]
fn reject_duplicate_yaml_keys() {
    let (_dir, store) = store();
    create(&store, "task", "Task", json!({"tags": ["test-topic"]})).unwrap();
    let path = md_files(&store, "tasks").remove(0);
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("priority: 3"));
    fs::write(
        &path,
        text.replace("priority: 3", "priority: 3\npriority: 1"),
    )
    .unwrap();
    assert_err(store.load(), "Duplicate YAML");
}

#[test]
fn failed_multi_file_write_rolls_back() {
    let (_dir, store) = store();
    let idea = create(&store, "idea", "Try", json!({"tags": ["test-topic"]})).unwrap();
    let original = fs::read(&md_files(&store, "ideas")[0]).unwrap();
    store.fail_nth_write(2);
    assert_err(store.promote(idea.id()), "Simulated");
    assert_eq!(fs::read(&md_files(&store, "ideas")[0]).unwrap(), original);
    assert!(md_files(&store, "tasks").is_empty());
    assert!(!store.root.join(".dikt-write.lock").exists());
}

#[test]
fn tag_policy_requires_a_specific_tag() {
    for tags in [json!([]), json!(["pm"]), json!(["pm", "python"])] {
        let (_dir, store) = store();
        fs::write(
            store.root.join("tags.yaml"),
            "tags: [pm, python, typer]\nbroad_tags: [pm, python]\n",
        )
        .unwrap();
        assert_err(
            create(&store, "task", "Choose a CLI tool", json!({"tags": tags})),
            "specific tag",
        );
        assert!(md_files(&store, "tasks").is_empty());
    }
}

#[test]
fn tag_policy_allows_twenty_specific_tags() {
    let (_dir, store) = store();
    let topics: Vec<String> = (0..20).map(|index| format!("topic-{index}")).collect();
    fs::write(
        store.root.join("tags.yaml"),
        format!("tags: [pm, {}]\nbroad_tags: [pm]\n", topics.join(", ")),
    )
    .unwrap();
    let mut tags = vec!["pm".to_string()];
    tags.extend(topics);
    let item = create(
        &store,
        "task",
        "Large cross-topic effort",
        json!({"tags": tags}),
    )
    .unwrap();
    assert_eq!(store.load().unwrap()[item.id()].list("tags"), tags);
}

#[test]
fn tag_policy_rejects_removing_last_specific_tag() {
    let (_dir, store) = store();
    fs::write(
        store.root.join("tags.yaml"),
        "tags: [pm, typer]\nbroad_tags: [pm]\n",
    )
    .unwrap();
    let item = create(
        &store,
        "task",
        "Choose a CLI tool",
        json!({"tags": ["pm", "typer"]}),
    )
    .unwrap();
    assert_err(
        store.change(
            item.id(),
            Change {
                remove_tags: vec!["typer".into()],
                ..Default::default()
            },
        ),
        "specific tag",
    );
    assert_eq!(
        store.load().unwrap()[item.id()].list("tags"),
        ["pm", "typer"]
    );
}

#[test]
fn tag_policy_rejects_invalid_registry() {
    for registry in [
        "tags: [pm, typer]\n",
        "tags: [pm, typer]\nbroad_tags: [missing]\n",
        "tags: [pm, typer]\nbroad_tags: [pm, pm]\n",
        "tags: [pm, PyYAML]\nbroad_tags: [pm]\n",
        "tags: [pm, tool_choice]\nbroad_tags: [pm]\n",
    ] {
        let (_dir, store) = store();
        fs::write(store.root.join("tags.yaml"), registry).unwrap();
        assert!(store.tags().is_err(), "{registry}");
    }
}

#[test]
fn tag_policy_applies_to_every_item_type() {
    for kind in ["task", "idea", "knowledge", "decision"] {
        let (_dir, store) = store();
        let extra = if kind == "knowledge" {
            json!({"source": "Measured locally", "tags": ["pm", "python"]})
        } else {
            json!({"tags": ["pm", "python"]})
        };
        assert_err(
            create(&store, kind, "Specific subject needed", extra),
            "specific tag",
        );
    }
}

#[test]
fn accepted_decision_can_gain_specific_tags_without_rationale_changes() {
    let (_dir, store) = store();
    fs::write(
        store.root.join("tags.yaml"),
        "tags: [pm, python, test-topic, cli]\nbroad_tags: [pm, python]\n",
    )
    .unwrap();
    let decision = accepted(&store, "Choose CLI tooling");
    let before = store.load().unwrap()[decision.id()].clone();
    store
        .change(
            decision.id(),
            Change {
                add_tags: vec!["cli".into()],
                ..Default::default()
            },
        )
        .unwrap();
    let after = store.load().unwrap()[decision.id()].clone();
    assert_eq!(after.list("tags"), ["test-topic", "cli"]);
    assert_eq!(after.status(), "accepted");
    for name in ["Context", "Options", "Decision", "Consequences"] {
        assert_eq!(before.section(name), after.section(name));
    }
    assert!(after.section("Log").starts_with(&before.section("Log")));
}
