//! `dikt init`: installation into a host project.

use std::fs;

use dikt::cli::run_captured;
use dikt::init::{Agent, init};
use dikt::store::discover_root;
use tempfile::TempDir;

const ALL: [Agent; 3] = [Agent::Claude, Agent::Copilot, Agent::AgentsMd];

#[test]
fn creates_collection_skills_and_instruction_blocks() {
    let project = TempDir::new().unwrap();
    let root = project.path();
    let report = init(root, &ALL, false, Some("uv run dikt")).unwrap();
    assert!(report.contains("0 items, valid"), "{report}");
    for path in [
        "dikt/dikt.toml",
        "dikt/tags.yaml",
        "dikt/AGENTS.md",
        "dikt/CLAUDE.md",
        "dikt/README.md",
        "dikt/STRUCTURE.md",
        "dikt/.gitignore",
        "dikt/Inbox.md",
        "dikt/Scratch.md",
        "dikt/procedures/triage-inbox.md",
        "dikt/tasks/.gitkeep",
        "dikt/reports/daily/.gitkeep",
        ".claude/skills/dikt/SKILL.md",
        ".claude/skills/dikt-weekly-review/SKILL.md",
        ".github/skills/dikt-triage-inbox/SKILL.md",
        ".github/prompts/dikt-weekly-review.prompt.md",
        "AGENTS.md",
        "CLAUDE.md",
    ] {
        assert!(root.join(path).is_file(), "missing {path}");
    }
    let skill = fs::read_to_string(root.join(".claude/skills/dikt/SKILL.md")).unwrap();
    assert!(
        skill.contains("uv run dikt list")
            && skill.contains("](../../../dikt/AGENTS.md)")
            && !skill.contains("{{")
    );
    let skill_dir = root.join(".claude/skills/dikt");
    assert!(
        skill_dir.join("../../../dikt/AGENTS.md").is_file(),
        "relative skill link must resolve"
    );
    assert!(
        root.join(".github/prompts")
            .join("../../dikt/procedures/weekly-review.md")
            .is_file()
    );
    assert_eq!(
        discover_root(&root.join(".claude/skills")).unwrap(),
        dunce::canonicalize(root).unwrap().join("dikt")
    );
}

#[test]
fn is_idempotent_and_preserves_project_content() {
    let project = TempDir::new().unwrap();
    let root = project.path();
    fs::write(root.join("AGENTS.md"), "# Project rules\n\nKeep me.\n").unwrap();
    init(root, &ALL, false, Some("dikt")).unwrap();
    let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
    assert!(
        agents.starts_with("# Project rules\n\nKeep me.\n\n<!-- dikt:start -->"),
        "{agents}"
    );

    fs::write(
        root.join("dikt/tags.yaml"),
        "tags: [mine]\nbroad_tags: []\n",
    )
    .unwrap();
    fs::write(root.join("dikt/README.md"), "local edit").unwrap();
    let report = init(root, &ALL, false, Some("dikt")).unwrap();
    assert!(report.contains("dikt/README.md: kept (differs"), "{report}");
    assert_eq!(
        fs::read_to_string(root.join("dikt/README.md")).unwrap(),
        "local edit"
    );
    assert_eq!(fs::read_to_string(root.join("AGENTS.md")).unwrap(), agents);

    let report = init(root, &ALL, true, Some("uv run dikt")).unwrap();
    assert!(report.contains("dikt/README.md: updated"), "{report}");
    assert!(report.contains("AGENTS.md: section updated"), "{report}");
    assert_eq!(
        fs::read_to_string(root.join("dikt/tags.yaml")).unwrap(),
        "tags: [mine]\nbroad_tags: []\n"
    );
    let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
    assert!(
        agents.starts_with("# Project rules\n\nKeep me.\n\n")
            && agents.contains("uv run dikt list")
    );
    assert_eq!(agents.matches("<!-- dikt:start -->").count(), 1);
}

#[test]
fn agent_selection_limits_integrations() {
    let project = TempDir::new().unwrap();
    let root = project.path();
    init(root, &[Agent::Claude], false, None).unwrap();
    assert!(root.join(".claude/skills/dikt/SKILL.md").is_file());
    assert!(root.join("CLAUDE.md").is_file());
    assert!(!root.join(".github").exists());
    assert!(!root.join("AGENTS.md").exists());
}

#[test]
fn malformed_markers_are_refused() {
    let project = TempDir::new().unwrap();
    fs::write(
        project.path().join("CLAUDE.md"),
        "<!-- dikt:end -->\n<!-- dikt:start -->\n",
    )
    .unwrap();
    let error = init(project.path(), &[Agent::Claude], true, None).unwrap_err();
    assert!(error.message().contains("malformed"));
}

#[test]
fn cli_init_then_use_from_subdirectory() {
    let project = TempDir::new().unwrap();
    let path = project.path().display().to_string();
    let (code, out, err) = run_captured(None, &["init", &path, "--agents", "agents-md"]);
    assert_eq!(code, 0, "{out}{err}");
    let collection = project.path().join("dikt");
    let (code, out, err) = run_captured(
        Some(&collection),
        &["add", "task", "--title", "First", "--tag", "dikt"],
    );
    assert_eq!(code, 0, "{err}");
    let id = out.trim();
    assert!(
        id.len() == 10 && id.starts_with("T-") && id[2..].chars().all(|c| c.is_ascii_hexdigit()),
        "{id}"
    );
}
