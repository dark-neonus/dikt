//! Command-line adapter. Core rules live in `store` and `views`; this module only
//! parses arguments, projects output and maps errors to exit code 2.

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand};
use serde_json::{Map, Value, json};

use crate::bail;
use crate::error::{Error, Result};
use crate::init::{self, Agent};
use crate::store::{Change, Item, Items, Store, discover_root, parse_date, prefix, require, today};
use crate::views::{backlog_text, report as generate_report, review_reasons};
use crate::yaml::py_json;

#[derive(Parser)]
#[command(
    name = "dikt",
    version,
    arg_required_else_help = true,
    about = "Decisions, ideas, knowledge and tasks as validated local Markdown. All item writes are validated."
)]
struct Cli {
    /// DIKT collection directory (contains dikt.toml and tags.yaml). Default: discovered from the
    /// current directory upwards (./dikt.toml or ./dikt/dikt.toml). Must precede the subcommand.
    #[arg(long, env = "DIKT_ROOT", global = false)]
    root: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Args)]
struct Output {
    /// Include the full metadata and body.
    #[arg(long)]
    full: bool,
    /// Comma-separated metadata fields to project (JSON output).
    #[arg(long)]
    fields: Option<String>,
    /// Machine-readable JSON output.
    #[arg(long = "json")]
    as_json: bool,
    /// Recovery read: warn and exclude invalid or ambiguous items. Never repairs files.
    #[arg(long)]
    skip_invalid: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Set up DIKT in a project: create the dikt/ collection plus agent skills and instructions.
    Init {
        /// Project directory to set up.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Agent integrations to install (comma-separated).
        #[arg(long, value_enum, value_delimiter = ',', default_values = ["claude", "copilot", "agents-md"])]
        agents: Vec<Agent>,
        /// Overwrite dikt-managed docs and skills with this version's templates.
        /// Never touches items, reports, tags.yaml, procedures, Inbox.md or Scratch.md.
        #[arg(long)]
        refresh: bool,
        /// Command agents should run, written into docs (default: `uv run dikt` inside a project venv, else `dikt`).
        #[arg(long)]
        command: Option<String>,
    },
    /// Create a typed item (task, idea, knowledge, decision). Decisions default to proposed, never silently accepted.
    Add {
        kind: String,
        #[arg(long)]
        title: String,
        #[arg(long, default_value = "")]
        description: String,
        #[arg(long)]
        tag: Vec<String>,
        #[arg(long)]
        source: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long, allow_negative_numbers = true)]
        priority: Option<i64>,
        #[arg(long)]
        due: Option<String>,
        #[arg(long)]
        based_on: Vec<String>,
        #[arg(long)]
        revisit: Option<String>,
        #[arg(long)]
        revisit_when: Option<String>,
        #[arg(long)]
        context: Option<String>,
        #[arg(long)]
        options: Option<String>,
        #[arg(long)]
        decision: Option<String>,
        #[arg(long)]
        consequences: Option<String>,
        /// Additional metadata FIELD=VALUE (JSON values supported).
        #[arg(long = "set")]
        set_value: Vec<String>,
    },
    /// Read an item by its stable ID.
    Get {
        identifier: String,
        #[command(flatten)]
        output: Output,
    },
    /// Search content and filter metadata. Defaults to 20 compact results; use --offset to page.
    List {
        #[arg(long = "type")]
        kind: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        tag: Option<String>,
        /// All whitespace-separated words must appear (case-insensitive) in metadata, body or log.
        #[arg(long)]
        query: Option<String>,
        /// Updated on or after YYYY-MM-DD.
        #[arg(long)]
        since: Option<String>,
        /// Due on or before YYYY-MM-DD.
        #[arg(long)]
        due: Option<String>,
        #[arg(long)]
        based_on: Option<String>,
        #[arg(long)]
        related: Option<String>,
        /// Accepted decisions whose revisit date or evidence needs review.
        #[arg(long)]
        revisit_due: bool,
        #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u64).range(1..))]
        limit: u64,
        #[arg(long, default_value_t = 0)]
        offset: u64,
        #[command(flatten)]
        output: Output,
    },
    /// Update metadata. Checklist indexes start at 1; --set due=null clears a date.
    Update {
        identifier: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long, allow_negative_numbers = true)]
        priority: Option<i64>,
        #[arg(long)]
        due: Option<String>,
        #[arg(long)]
        add_tag: Vec<String>,
        #[arg(long)]
        rm_tag: Vec<String>,
        /// KIND=VALUE where KIND is jira|mr|commits|confluence|items.
        #[arg(long)]
        link: Option<String>,
        #[arg(long, allow_negative_numbers = true)]
        check: Option<i64>,
        #[arg(long, allow_negative_numbers = true)]
        uncheck: Option<i64>,
        #[arg(long)]
        add_check: Option<String>,
        /// Additional metadata FIELD=VALUE (JSON values supported).
        #[arg(long = "set")]
        set_value: Vec<String>,
    },
    /// Replace one body section, retaining the rest. Log is append-only.
    Edit {
        identifier: String,
        #[arg(long)]
        section: String,
        #[arg(long)]
        text: Option<String>,
        /// Read UTF-8 section text from a file.
        #[arg(long)]
        file: Option<PathBuf>,
    },
    /// Append a useful progress note, finding or blocker.
    Log { identifier: String, text: String },
    /// Complete a task with its outcome.
    Close {
        identifier: String,
        #[arg(long)]
        outcome: String,
    },
    /// Create a task from an open idea, preserving the original and links.
    Promote { identifier: String },
    /// Replace an accepted decision with another accepted decision.
    Supersede {
        identifier: String,
        #[arg(long)]
        by: String,
    },
    /// Save today's, yesterday's, a dated or weekly report; preserve manual summaries.
    Report {
        /// today | yesterday | YYYY-MM-DD | week
        #[arg(default_value = "today")]
        period: String,
        /// Any date in the requested ISO week (with `week`).
        #[arg(long = "date")]
        anchor: Option<String>,
    },
    /// Rebuild the local, gitignored Backlog.md, including decision review flags.
    Backlog,
    /// Validate schemas, tags, references, filenames and lifecycle relationships.
    Validate,
}

/// Parse FIELD=VALUE pairs; values are JSON when parseable, otherwise strings.
fn assignments(values: &[String]) -> Result<Map<String, Value>> {
    let mut result = Map::new();
    for value in values {
        let Some((name, content)) = value.split_once('=').filter(|(name, _)| !name.is_empty())
        else {
            bail!("Expected FIELD=VALUE; use JSON for lists, booleans, numbers or null");
        };
        let parsed =
            serde_json::from_str(content).unwrap_or_else(|_| Value::String(content.to_string()));
        result.insert(name.to_string(), parsed);
    }
    Ok(result)
}

struct Io<'a> {
    out: &'a mut dyn Write,
    err: &'a mut dyn Write,
}

impl Io<'_> {
    fn line(&mut self, text: impl AsRef<str>) -> Result<()> {
        writeln!(self.out, "{}", text.as_ref())?;
        Ok(())
    }

    fn warn(&mut self, text: impl AsRef<str>) -> Result<()> {
        writeln!(self.err, "{}", text.as_ref())?;
        Ok(())
    }
}

fn emit(io: &mut Io, items: &[&Item], output: &Output, single: bool) -> Result<()> {
    if output.fields.is_some() && output.full {
        bail!("Choose --fields or --full, not both");
    }
    let mut rows = Vec::new();
    for item in items {
        let row: Map<String, Value> = if let Some(fields) = &output.fields {
            let mut row = Map::new();
            for name in fields.split(',') {
                let Some(value) = item.metadata.get(name) else {
                    bail!("Unknown requested field");
                };
                row.insert(name.to_string(), value.clone());
            }
            row
        } else if output.full {
            let mut row = item.metadata.clone();
            row.insert("body".into(), json!(item.body));
            row
        } else {
            ["id", "title", "status"]
                .iter()
                .map(|name| (name.to_string(), item.metadata[*name].clone()))
                .collect()
        };
        rows.push(Value::Object(row));
    }
    if output.as_json || output.fields.is_some() {
        let value = if single && !rows.is_empty() {
            rows.remove(0)
        } else {
            Value::Array(rows)
        };
        io.line(serde_json::to_string_pretty(&value).expect("JSON serialization"))
    } else if output.full {
        io.line(
            items
                .iter()
                .map(|item| item.render())
                .collect::<Vec<_>>()
                .join("\n"),
        )
    } else {
        for item in items {
            io.line(format!(
                "{} [{}] {}",
                item.id(),
                item.status(),
                item.str("title")
            ))?;
        }
        Ok(())
    }
}

fn load_for_read(io: &mut Io, store: &Store, skip_invalid: bool) -> Result<Items> {
    if !skip_invalid {
        return store.load();
    }
    let mut diagnostics = Vec::new();
    let items = store.load_recovering(&mut diagnostics)?;
    io.warn("Warning: recovery read enabled; results may be incomplete. No files are repaired. Run dikt validate for a strict check.")?;
    for diagnostic in diagnostics {
        io.warn(format!("Warning: {diagnostic}"))?;
    }
    Ok(items)
}

fn open_store(root: Option<PathBuf>) -> Result<Store> {
    let resolved = match root {
        Some(root) => root,
        None => discover_root(&std::env::current_dir()?)?,
    };
    if !resolved.join("tags.yaml").is_file() {
        bail!("DIKT root must contain tags.yaml: {}", resolved.display());
    }
    Store::new(&resolved)
}

fn execute(cli: Cli, io: &mut Io) -> Result<()> {
    if let Command::Init {
        path,
        agents,
        refresh,
        command,
    } = cli.command
    {
        let report = init::init(&path, &agents, refresh, command.as_deref())?;
        return io.line(report);
    }
    let store = open_store(cli.root)?;
    match cli.command {
        Command::Init { .. } => unreachable!(),
        Command::Add {
            kind,
            title,
            description,
            tag,
            source,
            status,
            priority,
            due,
            based_on,
            revisit,
            revisit_when,
            context,
            options,
            decision,
            consequences,
            set_value,
        } => {
            let mut fields = assignments(&set_value)?;
            let mut protected = vec![
                "id",
                "type",
                "schema_version",
                "created",
                "updated",
                "superseded_by",
                "promoted_to",
            ];
            if kind != "knowledge" {
                protected.push("supersedes");
            }
            if fields.keys().any(|key| protected.contains(&key.as_str())) {
                bail!("Cannot supply identity/history fields");
            }
            let options_list: [(&str, Option<Value>); 8] = [
                ("tags", (!tag.is_empty()).then(|| json!(tag))),
                ("source", source.map(Value::from)),
                ("status", status.map(Value::from)),
                ("priority", priority.map(Value::from)),
                ("due", due.map(Value::from)),
                ("based_on", (!based_on.is_empty()).then(|| json!(based_on))),
                ("revisit", revisit.map(Value::from)),
                ("revisit_when", revisit_when.map(Value::from)),
            ];
            for (name, value) in options_list {
                if let Some(value) = value {
                    fields.insert(name.into(), value);
                }
            }
            let body: Vec<(&str, String)> = [
                ("Context", context),
                ("Options", options),
                ("Decision", decision),
                ("Consequences", consequences),
            ]
            .into_iter()
            .filter_map(|(name, text)| text.map(|text| (name, text)))
            .collect();
            let item = store.create(&kind, &title, &description, &body, fields)?;
            io.line(item.id())
        }
        Command::Get { identifier, output } => {
            let items = load_for_read(io, &store, output.skip_invalid)?;
            emit(io, &[require(&items, &identifier)?], &output, true)
        }
        Command::List {
            kind,
            status,
            tag,
            query,
            since,
            due,
            based_on,
            related,
            revisit_due,
            limit,
            offset,
            output,
        } => {
            if let Some(kind) = &kind
                && prefix(kind).is_none()
            {
                bail!("Unknown type: {kind}");
            }
            for value in [&since, &due].into_iter().flatten() {
                parse_date(value)?;
            }
            let items = load_for_read(io, &store, output.skip_invalid)?;
            let words: Vec<String> = query
                .as_deref()
                .unwrap_or("")
                .to_lowercase()
                .split_whitespace()
                .map(String::from)
                .collect();
            let today = today();
            let mut matches: Vec<&Item> = items
                .values()
                .filter(|item| {
                    kind.as_ref().is_none_or(|kind| item.kind() == kind)
                        && status.as_ref().is_none_or(|status| item.status() == status)
                        && tag
                            .as_ref()
                            .is_none_or(|tag| item.list("tags").contains(tag))
                        && since.as_ref().is_none_or(|since| {
                            item.str("updated").get(..10).unwrap_or("") >= since.as_str()
                        })
                        && due.as_ref().is_none_or(|due| {
                            !item.str("due").is_empty() && item.str("due") <= due.as_str()
                        })
                        && based_on
                            .as_ref()
                            .is_none_or(|evidence| item.list("based_on").contains(evidence))
                        && related
                            .as_ref()
                            .is_none_or(|related| crate::store::references(item).contains(related))
                        && (!revisit_due || !review_reasons(item, &items, today).is_empty())
                })
                .collect();
            if !words.is_empty() {
                matches.retain(|item| {
                    let text = item.render().to_lowercase();
                    words.iter().all(|word| text.contains(word.as_str()))
                });
            }
            matches.sort_by(|a, b| a.id().cmp(b.id()));
            let (offset, limit) = (offset as usize, limit as usize);
            let page: Vec<&Item> = matches.iter().copied().skip(offset).take(limit).collect();
            emit(io, &page, &output, false)?;
            if matches.len() > offset + limit {
                io.warn(format!(
                    "More results: --offset {} ({} total)",
                    offset + limit,
                    matches.len()
                ))?;
            }
            Ok(())
        }
        Command::Update {
            identifier,
            title,
            status,
            priority,
            due,
            add_tag,
            rm_tag,
            link,
            check,
            uncheck,
            add_check,
            set_value,
        } => {
            let mut fields = assignments(&set_value)?;
            for (name, value) in [
                ("title", title.map(Value::from)),
                ("status", status.map(Value::from)),
                ("priority", priority.map(Value::from)),
                ("due", due.map(Value::from)),
            ] {
                if let Some(value) = value {
                    fields.insert(name.into(), value);
                }
            }
            let parsed_link = match &link {
                Some(link) => match link.split_once('=') {
                    Some((name, value)) => Some((name.to_string(), value.to_string())),
                    None => bail!("Expected --link KIND=VALUE"),
                },
                None => None,
            };
            if fields.is_empty()
                && add_tag.is_empty()
                && rm_tag.is_empty()
                && parsed_link.is_none()
                && check.is_none()
                && uncheck.is_none()
                && add_check.is_none()
            {
                bail!("No changes requested");
            }
            let mut detail = Map::new();
            if !fields.is_empty() {
                detail.insert("fields".into(), Value::Object(fields.clone()));
            }
            if !add_tag.is_empty() {
                detail.insert("add_tags".into(), json!(add_tag));
            }
            if !rm_tag.is_empty() {
                detail.insert("remove_tags".into(), json!(rm_tag));
            }
            for (name, value) in [
                ("link", link.map(Value::from)),
                ("check", check.map(Value::from)),
                ("uncheck", uncheck.map(Value::from)),
                ("add_check", add_check.clone().map(Value::from)),
            ] {
                if let Some(value) = value {
                    detail.insert(name.into(), value);
                }
            }
            let request = Change {
                fields,
                message: py_json(&Value::Object(detail)),
                add_tags: add_tag,
                remove_tags: rm_tag,
                link: parsed_link,
                check,
                uncheck,
                checklist: add_check,
                ..Default::default()
            };
            io.line(store.change(&identifier, request)?.id())
        }
        Command::Edit {
            identifier,
            section,
            text,
            file,
        } => {
            let content = match (text, file) {
                (Some(text), None) => text,
                (None, Some(file)) => std::fs::read_to_string(&file)
                    .map_err(|error| Error(format!("{}: {error}", file.display())))?,
                _ => bail!("Supply exactly one of --text or --file"),
            };
            let message = format!("Replaced {section}");
            let request = Change {
                section: Some((section, content)),
                event: "edited".into(),
                message,
                ..Default::default()
            };
            io.line(store.change(&identifier, request)?.id())
        }
        Command::Log { identifier, text } => {
            if text.trim().is_empty() {
                bail!("Log message cannot be empty");
            }
            io.line(
                store
                    .change(
                        &identifier,
                        Change {
                            event: "note".into(),
                            message: text,
                            ..Default::default()
                        },
                    )?
                    .id(),
            )
        }
        Command::Close {
            identifier,
            outcome,
        } => {
            if outcome.trim().is_empty() {
                bail!("Outcome cannot be empty");
            }
            let items = store.load()?;
            let item = require(&items, &identifier)?;
            if item.kind() != "task" || matches!(item.status(), "done" | "cancelled") {
                bail!("Only unfinished tasks can be closed");
            }
            let request = Change {
                section: Some(("Outcome".into(), outcome.clone())),
                event: "closed".into(),
                message: outcome,
                ..Change::fields(json!({"status": "done"}))
            };
            io.line(store.change(&identifier, request)?.id())
        }
        Command::Promote { identifier } => io.line(store.promote(&identifier)?.id()),
        Command::Supersede { identifier, by } => io.line(store.supersede(&identifier, &by)?.id()),
        Command::Report { period, anchor } => {
            let anchor = anchor.as_deref().map(parse_date).transpose()?;
            io.line(generate_report(&store, &period, anchor)?)
        }
        Command::Backlog => {
            let _lock = store.write_lock()?;
            crate::store::atomic_write(
                &store.safe_path("Backlog.md")?,
                &backlog_text(&store.load()?, None),
            )?;
            io.line("Backlog.md")
        }
        Command::Validate => io.line(format!("Valid: {} items", store.load()?.len())),
    }
}

/// Run the CLI with explicit arguments and streams; returns the process exit code.
pub fn run<I, T>(args: I, out: &mut dyn Write, err: &mut dyn Write) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) => {
            let code = error.exit_code();
            let rendered = error.render().to_string();
            let _ = if code == 0 {
                write!(out, "{rendered}")
            } else {
                write!(err, "{rendered}")
            };
            return code;
        }
    };
    let mut io = Io { out, err };
    match execute(cli, &mut io) {
        Ok(()) => 0,
        Err(error) => {
            let _ = writeln!(io.err, "Error: {error}");
            2
        }
    }
}

/// Convenience for tests: run against a collection and capture (exit code, stdout, stderr).
pub fn run_captured(root: Option<&Path>, args: &[&str]) -> (i32, String, String) {
    let mut full: Vec<String> = vec!["dikt".into()];
    if let Some(root) = root {
        full.push("--root".into());
        full.push(root.display().to_string());
    }
    full.extend(args.iter().map(|arg| arg.to_string()));
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run(full, &mut out, &mut err);
    (
        code,
        String::from_utf8_lossy(&out).into_owned(),
        String::from_utf8_lossy(&err).into_owned(),
    )
}
