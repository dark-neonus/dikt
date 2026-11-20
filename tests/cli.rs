//! Port of the blueprint's tests/test_cli.py.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use dikt::cli::run_captured;
use dikt::store::{Store, parse_item};
use serde_json::Value;
use tempfile::TempDir;

struct Cli {
    _dir: TempDir,
    root: PathBuf,
}

impl Cli {
    fn new() -> Self {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("tags.yaml"),
            "tags: [pm, cli]\nbroad_tags: [pm]\n",
        )
        .unwrap();
        let root = dir.path().canonicalize().unwrap();
        Cli { _dir: dir, root }
    }

    fn raw(&self, args: &[&str]) -> (i32, String, String) {
        run_captured(Some(&self.root), args)
    }

    fn ok(&self, args: &[&str]) -> String {
        let (code, out, err) = self.raw(args);
        assert_eq!(code, 0, "{args:?}\nstdout: {out}\nstderr: {err}");
        out
    }

    fn id(&self, args: &[&str]) -> String {
        self.ok(args).trim().to_string()
    }

    fn store(&self) -> Store {
        Store::new(&self.root).unwrap()
    }
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(path: &Path, into: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(path).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, into);
            } else {
                into.insert(path.clone(), fs::read(&path).unwrap());
            }
        }
    }
    let mut result = BTreeMap::new();
    walk(root, &mut result);
    result
}

fn first_md(folder: PathBuf) -> PathBuf {
    let mut paths: Vec<_> = fs::read_dir(folder)
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .collect();
    paths.sort();
    paths.remove(0)
}

#[test]
fn task_workflow() {
    let cli = Cli::new();
    let identifier = cli.id(&[
        "add",
        "task",
        "--title",
        "Fix a bug",
        "--tag",
        "pm",
        "--tag",
        "cli",
    ]);
    cli.ok(&[
        "update",
        &identifier,
        "--status",
        "in_progress",
        "--add-check",
        "Test",
    ]);
    cli.ok(&["update", &identifier, "--check", "1", "--due", "2026-12-01"]);
    cli.ok(&[
        "edit",
        &identifier,
        "--section",
        "Description",
        "--text",
        "A reproducible failure",
    ]);
    cli.ok(&["log", &identifier, "Reproduced with empty input"]);
    cli.ok(&[
        "close",
        &identifier,
        "--outcome",
        "Fixed; regression tested",
    ]);
    assert_eq!(cli.store().load().unwrap()[&identifier].status(), "done");
    let row: Value = serde_json::from_str(&cli.ok(&["get", &identifier, "--json"])).unwrap();
    assert_eq!(row["id"], identifier.as_str());
    assert!(
        cli.ok(&["get", &identifier, "--full"])
            .contains("A reproducible failure")
    );
    assert!(cli.ok(&["validate"]).contains("Valid: 1"));
}

#[test]
fn decision_search_and_evidence() {
    let cli = Cli::new();
    let knowledge = cli.id(&[
        "add",
        "knowledge",
        "--title",
        "Evidence",
        "--source",
        "Measured locally",
        "--tag",
        "cli",
    ]);
    let decision = cli.id(&[
        "add",
        "decision",
        "--title",
        "CLI first",
        "--tag",
        "cli",
        "--status",
        "accepted",
        "--based-on",
        &knowledge,
        "--context",
        "One user",
        "--options",
        "CLI or MCP",
        "--decision",
        "CLI",
        "--consequences",
        "Shell calls",
    ]);
    assert!(
        cli.ok(&["list", "--query", "cli first", "--type", "decision"])
            .contains(&decision)
    );
    assert!(
        cli.ok(&["list", "--based-on", &knowledge])
            .contains(&decision)
    );
    cli.ok(&["update", &knowledge, "--status", "outdated"]);
    assert!(cli.ok(&["list", "--revisit-due"]).contains(&decision));
    assert!(
        fs::read_to_string(cli.root.join("Backlog.md"))
            .unwrap()
            .contains(&format!("outdated evidence {knowledge}"))
    );
}

#[test]
fn projection_and_pagination() {
    let cli = Cli::new();
    for index in 0..3 {
        cli.ok(&[
            "add",
            "idea",
            "--title",
            &format!("Idea {index}"),
            "--tag",
            "cli",
        ]);
    }
    let rows: Vec<Value> =
        serde_json::from_str(&cli.ok(&["list", "--limit", "2", "--json"])).unwrap();
    assert_eq!(rows.len(), 2);
    let rows: Value =
        serde_json::from_str(&cli.ok(&["list", "--limit", "2", "--offset", "2", "--fields", "id"]))
            .unwrap();
    assert_eq!(rows, serde_json::json!([{"id": "I-00003"}]));
    let (_, _, err) = cli.raw(&["list", "--limit", "2"]);
    assert!(err.contains("More results: --offset 2 (3 total)"));
}

#[test]
fn report_selector_is_positional() {
    for arguments in [
        vec!["today"],
        vec!["yesterday"],
        vec!["2026-11-17"],
        vec!["week", "--date", "2026-11-17"],
    ] {
        let cli = Cli::new();
        let mut args = vec!["report"];
        args.extend(arguments);
        let relative = cli.id(&args);
        assert!(cli.root.join(relative).is_file());
    }
}

#[test]
fn recovery_reads_skip_invalid_without_modifying_files() {
    for corruption in ["yaml", "schema", "type", "tag", "reference", "duplicate"] {
        let cli = Cli::new();
        let good = cli.id(&["add", "task", "--title", "Readable", "--tag", "cli"]);
        let bad = cli.id(&["add", "idea", "--title", "Broken", "--tag", "cli"]);
        let path = first_md(cli.root.join("ideas"));
        let text = fs::read_to_string(&path).unwrap();
        let replaced = |from: &str, to: &str| {
            assert!(
                text.contains(from),
                "{corruption}: {from:?} missing from\n{text}"
            );
            text.replace(from, to)
        };
        match corruption {
            "yaml" => fs::write(&path, "---\ntitle: [\n---\n").unwrap(),
            "schema" => {
                fs::write(&path, replaced("schema_version: 1", "schema_version: 999")).unwrap()
            }
            "type" => fs::write(&path, replaced("type: idea", "type: []")).unwrap(),
            "tag" => {
                let mut item = parse_item(&path).unwrap();
                item.metadata
                    .insert("tags".into(), serde_json::json!(["removed"]));
                fs::write(&path, item.render()).unwrap();
            }
            "reference" => fs::write(&path, replaced("items: []", "items: [T-99999]")).unwrap(),
            _ => fs::write(path.with_file_name(format!("{bad}-duplicate.md")), &text).unwrap(),
        }
        let before = snapshot(&cli.root);
        assert_ne!(cli.raw(&["list"]).0, 0, "{corruption}");
        let (code, out, err) = cli.raw(&["list", "--skip-invalid", "--json"]);
        assert_eq!(code, 0, "{corruption}: {err}");
        let ids: Vec<String> = serde_json::from_str::<Vec<Value>>(&out)
            .unwrap()
            .iter()
            .map(|row| row["id"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(ids, std::slice::from_ref(&good), "{corruption}");
        assert!(err.contains("Warning:"));
        assert!(
            err.contains(&bad)
                || err.contains(&path.file_name().unwrap().to_string_lossy().into_owned()),
            "{corruption}: {err}"
        );
        let (code, out, _) = cli.raw(&["get", &good, "--skip-invalid", "--json"]);
        assert_eq!(code, 0);
        assert_eq!(
            serde_json::from_str::<Value>(&out).unwrap()["id"],
            good.as_str()
        );
        assert_ne!(cli.raw(&["update", &good, "--title", "Unsafe write"]).0, 0);
        assert_ne!(cli.raw(&["validate"]).0, 0);
        assert_eq!(before, snapshot(&cli.root), "{corruption}: files changed");
    }
}

#[test]
fn recovery_excludes_transitive_dependents() {
    let cli = Cli::new();
    let good = cli.id(&["add", "task", "--title", "Independent", "--tag", "cli"]);
    let broken = cli.id(&["add", "idea", "--title", "Broken", "--tag", "cli"]);
    let dependent = cli.id(&["add", "task", "--title", "Dependent", "--tag", "cli"]);
    cli.ok(&["update", &dependent, "--link", &format!("items={broken}")]);
    let indirect = cli.id(&["add", "task", "--title", "Indirect", "--tag", "cli"]);
    cli.ok(&["update", &indirect, "--link", &format!("items={dependent}")]);
    fs::write(first_md(cli.root.join("ideas")), "Not an item").unwrap();
    let (code, out, err) = cli.raw(&["list", "--skip-invalid", "--json"]);
    assert_eq!(code, 0, "{err}");
    let ids: Vec<Value> = serde_json::from_str(&out).unwrap();
    assert_eq!(
        ids,
        [serde_json::json!({"id": good, "title": "Independent", "status": "todo"})]
    );
    assert!(err.contains(&dependent) && err.contains(&indirect));
}

#[test]
fn errors_exit_two_with_message() {
    let cli = Cli::new();
    let (code, _, err) = cli.raw(&["add", "task", "--title", "No specific tag", "--tag", "pm"]);
    assert_eq!(code, 2);
    assert!(
        err.starts_with("Error: ") && err.contains("specific tag"),
        "{err}"
    );
    let (code, _, err) = cli.raw(&[
        "add", "task", "--title", "x", "--tag", "cli", "--set", "id=T-1",
    ]);
    assert_eq!(code, 2);
    assert!(err.contains("identity/history"));
    let (code, _, _) = cli.raw(&["update", "T-00009", "--title", "x"]);
    assert_eq!(code, 2);
}

#[test]
fn set_values_are_json_when_parseable() {
    let cli = Cli::new();
    let identifier = cli.id(&[
        "add",
        "task",
        "--title",
        "Dated",
        "--tag",
        "cli",
        "--due",
        "2026-12-01",
    ]);
    cli.ok(&[
        "update",
        &identifier,
        "--set",
        "due=null",
        "--set",
        "priority=1",
    ]);
    let row: Value =
        serde_json::from_str(&cli.ok(&["get", &identifier, "--fields", "due,priority"])).unwrap();
    assert_eq!(row, serde_json::json!({"due": null, "priority": 1}));
}
