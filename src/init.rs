//! `dikt init`: install a DIKT collection plus agent skills/instructions into a project.
//!
//! Project data (items, tags, procedures, inbox notes) is never overwritten.
//! Managed files (docs, skills, prompts, marked instruction blocks) are created
//! when missing and only replaced with `--refresh`.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use clap::ValueEnum;

use crate::bail;
use crate::error::{Error, Result};
use crate::store::{DEFAULT_COLLECTION, Store, atomic_write, join_relative};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Agent {
    /// .claude/skills + a DIKT section in CLAUDE.md
    Claude,
    /// .github/skills + .github/prompts
    Copilot,
    /// A DIKT section in AGENTS.md (Codex, Copilot and other AGENTS.md readers)
    #[value(name = "agents-md")]
    AgentsMd,
}

pub const BLOCK_START: &str = "<!-- dikt:start -->";
pub const BLOCK_END: &str = "<!-- dikt:end -->";

const DIRECTORIES: [&str; 12] = [
    "tasks",
    "ideas",
    "knowledge",
    "decisions",
    "reports/daily",
    "reports/weekly",
    "procedures",
    "context",
    "tools",
    "guides",
    "docs",
    "reports",
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Owner {
    /// Belongs to the project once created; never overwritten.
    Data,
    /// Belongs to dikt; replaced by `--refresh`.
    Managed,
}

macro_rules! template {
    ($path:literal) => {
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/", $path))
    };
}

const COLLECTION: [(&str, &str, Owner); 11] = [
    ("dikt.toml", template!("collection/dikt.toml"), Owner::Data),
    ("tags.yaml", template!("collection/tags.yaml"), Owner::Data),
    ("Inbox.md", template!("collection/Inbox.md"), Owner::Data),
    (
        "Scratch.md",
        template!("collection/Scratch.md"),
        Owner::Data,
    ),
    (
        "procedures/triage-inbox.md",
        template!("collection/procedures/triage-inbox.md"),
        Owner::Data,
    ),
    (
        "procedures/weekly-review.md",
        template!("collection/procedures/weekly-review.md"),
        Owner::Data,
    ),
    (
        ".gitignore",
        template!("collection/gitignore"),
        Owner::Managed,
    ),
    (
        "README.md",
        template!("collection/README.md"),
        Owner::Managed,
    ),
    (
        "STRUCTURE.md",
        template!("collection/STRUCTURE.md"),
        Owner::Managed,
    ),
    (
        "AGENTS.md",
        template!("collection/AGENTS.md"),
        Owner::Managed,
    ),
    (
        "CLAUDE.md",
        template!("collection/CLAUDE.md"),
        Owner::Managed,
    ),
];

const SKILLS: [(&str, &str); 3] = [
    ("dikt", template!("skills/dikt.md")),
    ("dikt-triage-inbox", template!("skills/triage-inbox.md")),
    ("dikt-weekly-review", template!("skills/weekly-review.md")),
];

const PROMPTS: [(&str, &str); 2] = [
    (
        "dikt-triage-inbox.prompt.md",
        template!("prompts/triage-inbox.prompt.md"),
    ),
    (
        "dikt-weekly-review.prompt.md",
        template!("prompts/weekly-review.prompt.md"),
    ),
];

const ROOT_BLOCK: &str = template!("root-block.md");
const GITKEEP: &str = "Retain this initially empty DIKT directory.\n";

fn render(template: &str, command: &str, up: &str) -> String {
    template
        .replace("{{DIKT}}", command)
        .replace("{{ROOT}}", DEFAULT_COLLECTION)
        .replace("{{UP}}", up)
}

/// `uv run dikt` when this binary lives in the project's virtual environment, else `dikt`.
fn detect_command(project: &Path) -> String {
    let in_project_venv = std::env::current_exe()
        .ok()
        .and_then(|exe| dunce::canonicalize(exe).ok())
        .zip(dunce::canonicalize(project.join(".venv")).ok())
        .is_some_and(|(exe, venv)| exe.starts_with(venv));
    if in_project_venv {
        "uv run dikt".into()
    } else {
        "dikt".into()
    }
}

struct Installer {
    project: PathBuf,
    refresh: bool,
    log: Vec<(String, &'static str)>,
}

impl Installer {
    fn record(&mut self, path: &Path, status: &'static str) {
        let relative = path
            .strip_prefix(&self.project)
            .unwrap_or(path)
            .components()
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        self.log.push((relative, status));
    }

    fn file(&mut self, path: PathBuf, content: &str, owner: Owner) -> Result<()> {
        let status = match fs::read(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                atomic_write(&path, content)?;
                "created"
            }
            Err(error) => return Err(Error(format!("{}: {error}", path.display()))),
            Ok(existing) if existing == content.as_bytes() => "unchanged",
            Ok(_) if owner == Owner::Data => "kept (project data)",
            Ok(_) if self.refresh => {
                atomic_write(&path, content)?;
                "updated"
            }
            Ok(_) => "kept (differs; --refresh to update)",
        };
        self.record(&path, status);
        Ok(())
    }

    /// Insert or refresh the marked DIKT section in an instruction file, preserving all other text.
    fn block(&mut self, path: PathBuf, content: &str) -> Result<()> {
        let block = format!("{BLOCK_START}\n{}\n{BLOCK_END}", content.trim_end());
        let status = match fs::read_to_string(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                atomic_write(&path, &format!("{block}\n"))?;
                "created"
            }
            Err(error) => return Err(Error(format!("{}: {error}", path.display()))),
            Ok(text) => {
                let (starts, ends) = (
                    text.matches(BLOCK_START).count(),
                    text.matches(BLOCK_END).count(),
                );
                if starts == 0 && ends == 0 {
                    let separator = if text.is_empty() || text.ends_with("\n\n") {
                        ""
                    } else if text.ends_with('\n') {
                        "\n"
                    } else {
                        "\n\n"
                    };
                    atomic_write(&path, &format!("{text}{separator}{block}\n"))?;
                    "section added"
                } else if starts != 1 || ends != 1 || text.find(BLOCK_START) > text.find(BLOCK_END)
                {
                    bail!(
                        "{}: malformed DIKT section markers; fix them manually",
                        path.display()
                    );
                } else {
                    let (start, end) = (
                        text.find(BLOCK_START).unwrap(),
                        text.find(BLOCK_END).unwrap() + BLOCK_END.len(),
                    );
                    if text[start..end] == block {
                        "section unchanged"
                    } else if self.refresh {
                        atomic_write(&path, &format!("{}{block}{}", &text[..start], &text[end..]))?;
                        "section updated"
                    } else {
                        "section kept (differs; --refresh to update)"
                    }
                }
            }
        };
        self.record(&path, status);
        Ok(())
    }
}

/// Install DIKT into `project`. Returns a human-readable report.
pub fn init(
    project: &Path,
    agents: &[Agent],
    refresh: bool,
    command: Option<&str>,
) -> Result<String> {
    let project = dunce::canonicalize(project)
        .map_err(|error| Error(format!("{}: {error}", project.display())))?;
    if !project.is_dir() {
        bail!("Not a directory: {}", project.display());
    }
    let command = command
        .map(String::from)
        .unwrap_or_else(|| detect_command(&project));
    let collection = project.join(DEFAULT_COLLECTION);
    if collection.exists() && !collection.is_dir() {
        bail!("{} exists and is not a directory", collection.display());
    }
    let mut installer = Installer {
        project: project.clone(),
        refresh,
        log: Vec::new(),
    };

    for (relative, content, owner) in COLLECTION {
        installer.file(
            join_relative(&collection, relative),
            &render(content, &command, ""),
            owner,
        )?;
    }
    for relative in DIRECTORIES {
        let directory = join_relative(&collection, relative);
        fs::create_dir_all(&directory)?;
        let empty = fs::read_dir(&directory)?.next().is_none();
        if empty && relative != "reports" {
            atomic_write(&directory.join(".gitkeep"), GITKEEP)?;
            installer.record(&directory.join(".gitkeep"), "created");
        }
    }

    let skill_roots: Vec<&str> = agents
        .iter()
        .filter_map(|agent| match agent {
            Agent::Claude => Some(".claude/skills"),
            Agent::Copilot => Some(".github/skills"),
            Agent::AgentsMd => None,
        })
        .collect();
    for root in &skill_roots {
        for (name, content) in SKILLS {
            installer.file(
                join_relative(&project, root).join(name).join("SKILL.md"),
                &render(content, &command, "../../../"),
                Owner::Managed,
            )?;
        }
    }
    if agents.contains(&Agent::Copilot) {
        for (name, content) in PROMPTS {
            installer.file(
                join_relative(&project, ".github/prompts").join(name),
                &render(content, &command, ""),
                Owner::Managed,
            )?;
        }
    }
    let block = render(ROOT_BLOCK, &command, "");
    if agents.contains(&Agent::AgentsMd) {
        installer.block(project.join("AGENTS.md"), &block)?;
    }
    if agents.contains(&Agent::Claude) {
        installer.block(project.join("CLAUDE.md"), &block)?;
    }

    let count = Store::new(&collection)?.load()?.len();
    let mut report = String::new();
    for (path, status) in &installer.log {
        let _ = writeln!(report, "  {path}: {status}");
    }
    let _ = write!(
        report,
        "\nDIKT collection: {} ({count} items, valid)\nAgent command: {command}\nNext: {command} add task --title \"...\" --tag dikt",
        collection.display()
    );
    Ok(report)
}
