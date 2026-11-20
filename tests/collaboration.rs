//! Collaboration: collision-free IDs, authorship and stable, chronological ordering.

use std::fs;
use std::path::Path;

use dikt::cli::run_captured;
use dikt::store::{Item, Items, Store, default_author, new_id};
use serde_json::{Map, Value, json};
use tempfile::TempDir;

fn collection() -> TempDir {
    let directory = TempDir::new().unwrap();
    fs::write(
        directory.path().join("tags.yaml"),
        "tags: [pm, docs, testing]\nbroad_tags: [pm]\n",
    )
    .unwrap();
    directory
}

fn fields(value: Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap()
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap().flatten() {
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn is_hash_id(id: &str, letter: char) -> bool {
    id.len() == 10
        && id.starts_with(&format!("{letter}-"))
        && id[2..]
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

#[test]
fn ids_are_short_hashes_and_name_the_file() {
    let directory = collection();
    let store = Store::new(directory.path()).unwrap();
    for (kind, letter, extra) in [
        ("task", 'T', json!({"tags": ["docs"]})),
        ("idea", 'I', json!({"tags": ["docs"]})),
        (
            "knowledge",
            'K',
            json!({"tags": ["docs"], "source": "Measured"}),
        ),
        ("decision", 'D', json!({"tags": ["docs"]})),
    ] {
        let item = store
            .create(kind, "Same title", "", &[], fields(extra))
            .unwrap();
        assert!(is_hash_id(item.id(), letter), "{}", item.id());
        let name = item.path.as_ref().map_or_else(
            || format!("{}-same-title.md", item.id()),
            |p| p.file_name().unwrap().to_string_lossy().into_owned(),
        );
        assert!(name.starts_with(item.id()));
    }
    assert_eq!(store.load().unwrap().len(), 4);
}

#[test]
fn separate_clones_creating_the_same_item_merge_without_conflicts() {
    let base = collection();
    let (a_dir, b_dir) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    copy_tree(base.path(), a_dir.path());
    copy_tree(base.path(), b_dir.path());
    let (a, b) = (
        Store::new(a_dir.path())
            .unwrap()
            .with_author(Some("Ada".into())),
        Store::new(b_dir.path())
            .unwrap()
            .with_author(Some("Grace".into())),
    );
    // Both people add "the first item" to their own branch, identical title and tags.
    for store in [&a, &b] {
        store
            .create(
                "knowledge",
                "Write the docs",
                "",
                &[],
                fields(json!({"tags": ["docs"], "source": "Review"})),
            )
            .unwrap();
    }
    // "Merge": bring B's files into A. A path that already exists would be a git conflict.
    for entry in fs::read_dir(b_dir.path().join("knowledge"))
        .unwrap()
        .flatten()
    {
        let target = a_dir.path().join("knowledge").join(entry.file_name());
        assert!(!target.exists(), "merge conflict on {}", target.display());
        fs::copy(entry.path(), target).unwrap();
    }
    let merged = a.load().unwrap();
    assert_eq!(merged.len(), 2);
    let authors: Vec<&str> = merged.values().map(|item| item.str("author")).collect();
    assert!(authors.contains(&"Ada") && authors.contains(&"Grace"));
}

#[test]
fn a_local_hash_collision_picks_another_id() {
    let tags = vec!["docs".to_string()];
    let none = Items::new();
    let first = new_id(
        "T",
        &none,
        "task",
        "Same",
        &tags,
        "2026-11-20T10:00:00+00:00",
        "Ada",
    );
    assert_eq!(
        first,
        new_id(
            "T",
            &none,
            "task",
            "Same",
            &tags,
            "2026-11-20T10:00:00+00:00",
            "Ada"
        ),
        "deterministic for equal inputs"
    );
    let mut taken = Items::new();
    taken.insert(
        first.clone(),
        Item {
            metadata: fields(json!({"id": first})),
            body: String::new(),
            path: None,
        },
    );
    let second = new_id(
        "T",
        &taken,
        "task",
        "Same",
        &tags,
        "2026-11-20T10:00:00+00:00",
        "Ada",
    );
    assert_ne!(first, second);
    assert!(is_hash_id(&second, 'T'));
}

#[test]
fn only_the_first_ten_tags_feed_the_hash() {
    let none = Items::new();
    let ten: Vec<String> = (0..10).map(|i| format!("t{i}")).collect();
    let mut eleven = ten.clone();
    eleven.push("extra".into());
    let at = "2026-11-20T10:00:00+00:00";
    assert_eq!(
        new_id("T", &none, "task", "Same", &ten, at, ""),
        new_id("T", &none, "task", "Same", &eleven, at, "")
    );
    assert_ne!(
        new_id("T", &none, "task", "Same", &ten, at, ""),
        new_id("T", &none, "task", "Other", &ten, at, "")
    );
}

#[test]
fn sequential_ids_from_earlier_versions_stay_valid() {
    let directory = collection();
    let store = Store::new(directory.path()).unwrap();
    let item = store
        .create("task", "Legacy", "", &[], fields(json!({"tags": ["docs"]})))
        .unwrap();
    let old_path = fs::read_dir(directory.path().join("tasks"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let text = fs::read_to_string(&old_path)
        .unwrap()
        .replace(item.id(), "T-00001");
    fs::remove_file(&old_path).unwrap();
    fs::write(directory.path().join("tasks/T-00001-legacy.md"), text).unwrap();
    assert!(store.load().unwrap().contains_key("T-00001"));
    let newer = store
        .create("task", "Newer", "", &[], fields(json!({"tags": ["docs"]})))
        .unwrap();
    assert!(is_hash_id(newer.id(), 'T'));
    store
        .change(
            newer.id(),
            dikt::store::Change {
                link: Some(("items".into(), "T-00001".into())),
                ..Default::default()
            },
        )
        .unwrap();
}

#[test]
fn author_is_recorded_filterable_and_immutable() {
    let directory = collection();
    let root = directory.path();
    let add = |author: &str, title: &str| {
        let (code, out, err) = run_captured(
            Some(root),
            &[
                "add", "task", "--title", title, "--tag", "docs", "--author", author,
            ],
        );
        assert_eq!(code, 0, "{err}");
        out.trim().to_string()
    };
    let ada = add("Ada Lovelace", "First");
    add("Grace Hopper", "Second");
    let (code, out, _) = run_captured(Some(root), &["list", "--author", "Ada Lovelace", "--json"]);
    assert_eq!(code, 0);
    let rows: Vec<Value> = serde_json::from_str(&out).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["id"], ada.as_str());
    let (_, out, _) = run_captured(Some(root), &["get", &ada, "--fields", "author"]);
    assert_eq!(
        serde_json::from_str::<Value>(&out).unwrap(),
        json!({"author": "Ada Lovelace"})
    );
    let (code, _, err) = run_captured(Some(root), &["update", &ada, "--set", "author=Mallory"]);
    assert_eq!(code, 2);
    assert!(err.contains("immutable"), "{err}");
    let (code, _, err) = run_captured(
        Some(root),
        &[
            "add", "task", "--title", "x", "--tag", "docs", "--author", "  ",
        ],
    );
    assert_eq!(code, 2, "{err}");
}

#[test]
fn promote_records_the_default_author() {
    let directory = collection();
    let store = Store::new(directory.path())
        .unwrap()
        .with_author(Some("Ada".into()));
    let idea = store
        .create("idea", "Try it", "", &[], fields(json!({"tags": ["docs"]})))
        .unwrap();
    let task = store.promote(idea.id()).unwrap();
    assert_eq!(task.str("author"), "Ada");
}

#[test]
fn author_falls_back_to_git_user_name() {
    if std::env::var_os("DIKT_AUTHOR").is_some() {
        return; // the environment override wins; nothing to check
    }
    let directory = TempDir::new().unwrap();
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(directory.path())
            .output()
    };
    if !git(&["init", "-q"]).is_ok_and(|output| output.status.success()) {
        return; // git unavailable
    }
    git(&["config", "user.name", "Grace Hopper"]).unwrap();
    assert_eq!(
        default_author(directory.path()).as_deref(),
        Some("Grace Hopper")
    );
}

#[test]
fn list_is_oldest_first_not_alphabetical_or_by_id() {
    let directory = collection();
    let root = directory.path();
    let titles = ["Zeta", "Alpha", "Mid", "Beta"];
    let ids: Vec<String> = titles
        .iter()
        .map(|title| {
            let (_, out, _) = run_captured(
                Some(root),
                &["add", "idea", "--title", title, "--tag", "docs"],
            );
            out.trim().to_string()
        })
        .collect();
    let (_, out, _) = run_captured(Some(root), &["list", "--fields", "id"]);
    let listed: Vec<String> = serde_json::from_str::<Vec<Value>>(&out)
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(listed, ids);
}

#[test]
fn report_activity_is_chronological() {
    let directory = collection();
    let store = Store::new(directory.path()).unwrap();
    let first = store
        .create(
            "task",
            "Second alphabetically",
            "",
            &[],
            fields(json!({"tags": ["docs"]})),
        )
        .unwrap();
    let second = store
        .create(
            "idea",
            "Another",
            "",
            &[],
            fields(json!({"tags": ["docs"]})),
        )
        .unwrap();
    let third = store
        .create(
            "task",
            "Aardvark",
            "",
            &[],
            fields(json!({"tags": ["docs"]})),
        )
        .unwrap();
    let path = directory
        .path()
        .join(dikt::views::report(&store, "today", None).unwrap());
    let text = fs::read_to_string(path).unwrap();
    let position = |id: &str| text.find(&format!("- {id} [created]")).unwrap();
    assert!(position(first.id()) < position(second.id()));
    assert!(position(second.id()) < position(third.id()));
}
