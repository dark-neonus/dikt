//! Item parsing, validation, ID allocation, lifecycle rules and transactional writes.

use std::cell::Cell;
use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::io::Write as _;
use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

use chrono::{DateTime, Local, NaiveDate};
use indexmap::IndexMap;
use regex::Regex;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use crate::bail;
use crate::error::{Error, Result};
use crate::yaml;

pub const KINDS: [(&str, &str); 4] = [
    ("task", "T"),
    ("idea", "I"),
    ("knowledge", "K"),
    ("decision", "D"),
];
pub const MARKER: &str = "dikt.toml";
pub const DEFAULT_COLLECTION: &str = "dikt";
pub const LOCK: &str = ".dikt-write.lock";
const COMMON: [&str; 9] = [
    "schema_version",
    "id",
    "type",
    "title",
    "status",
    "tags",
    "created",
    "updated",
    "links",
];
/// Optional on read (items written before authorship existed have none); set on creation.
const OPTIONAL: [&str; 1] = ["author"];
const LINK_KINDS: [&str; 5] = ["jira", "mr", "commits", "confluence", "items"];
const RATIONALE: [&str; 4] = ["Context", "Options", "Decision", "Consequences"];

pub fn prefix(kind: &str) -> Option<&'static str> {
    KINDS
        .iter()
        .find(|(name, _)| *name == kind)
        .map(|(_, prefix)| *prefix)
}

pub fn directory(kind: &str) -> &'static str {
    match kind {
        "task" => "tasks",
        "idea" => "ideas",
        "knowledge" => "knowledge",
        "decision" => "decisions",
        _ => unreachable!("unknown kind {kind}"),
    }
}

pub fn section_names(kind: &str) -> &'static [&'static str] {
    match kind {
        "task" => &["Description", "Outcome", "Log"],
        "idea" | "knowledge" => &["Description", "Log"],
        "decision" => &[
            "Description",
            "Context",
            "Options",
            "Decision",
            "Consequences",
            "Log",
        ],
        _ => &[],
    }
}

pub fn defaults(kind: &str) -> Map<String, Value> {
    let value = match kind {
        "task" => json!({"status": "todo", "priority": 3, "due": null, "checklist": []}),
        "idea" => json!({"status": "open", "promoted_to": null}),
        "knowledge" => {
            json!({"status": "active", "source": "", "last_verified": null, "supersedes": []})
        }
        "decision" => {
            json!({"status": "proposed", "decided": null, "revisit": null, "revisit_when": "",
                             "based_on": [], "supersedes": [], "superseded_by": null})
        }
        _ => json!({}),
    };
    value.as_object().cloned().unwrap_or_default()
}

pub fn now() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S%.6f%:z").to_string()
}

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

pub fn parse_date(text: &str) -> Result<NaiveDate> {
    static DATE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}$").unwrap());
    if DATE.is_match(text)
        && let Ok(date) = NaiveDate::parse_from_str(text, "%Y-%m-%d")
    {
        return Ok(date);
    }
    bail!("Invalid date (expected YYYY-MM-DD): {text}")
}

pub fn slug(title: &str) -> String {
    static NON_ALNUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-z0-9]+").unwrap());
    let ascii: String = title
        .nfkd()
        .filter(char::is_ascii)
        .collect::<String>()
        .to_ascii_lowercase();
    let replaced = NON_ALNUM.replace_all(&ascii, "-");
    let trimmed: String = replaced.trim_matches('-').chars().take(70).collect();
    let result = trimmed.trim_end_matches('-');
    if result.is_empty() {
        "item".into()
    } else {
        result.into()
    }
}

/// Map each level-2 heading outside fenced code to its (content start, end) byte span.
pub fn sections(body: &str) -> Result<IndexMap<String, (usize, usize)>> {
    static FENCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^ {0,3}(`{3,}|~{3,})").unwrap());
    static HEADING: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^## ([^\r\n]+)\r?\n?$").unwrap());
    let mut headings: Vec<(String, usize, usize)> = Vec::new();
    let mut offset = 0;
    let mut fence: Option<String> = None;
    for line in body.split_inclusive('\n') {
        if let Some(marker) = FENCE.captures(line) {
            let run = marker.get(1).unwrap().as_str();
            let rest = &line[marker.get(0).unwrap().end()..];
            match &fence {
                None => fence = Some(run.to_string()),
                Some(open)
                    if run.as_bytes()[0] == open.as_bytes()[0]
                        && run.len() >= open.len()
                        && rest.trim().is_empty() =>
                {
                    fence = None
                }
                Some(_) => {}
            }
        } else if fence.is_none()
            && let Some(heading) = HEADING.captures(line)
        {
            let name = heading.get(1).unwrap().as_str().trim().to_string();
            if headings.iter().any(|(existing, _, _)| *existing == name) {
                bail!("Duplicate section: {name}");
            }
            headings.push((name, offset, offset + line.len()));
        }
        offset += line.len();
    }
    let mut result = IndexMap::new();
    for (index, (name, _, content)) in headings.iter().enumerate() {
        let end = headings.get(index + 1).map_or(body.len(), |next| next.1);
        result.insert(name.clone(), (*content, end));
    }
    Ok(result)
}

#[derive(Debug, Clone)]
pub struct Item {
    pub metadata: Map<String, Value>,
    pub body: String,
    pub path: Option<PathBuf>,
}

impl Item {
    pub fn id(&self) -> &str {
        self.str("id")
    }

    pub fn kind(&self) -> &str {
        self.str("type")
    }

    pub fn status(&self) -> &str {
        self.str("status")
    }

    /// String field, or "" when absent or not a string.
    pub fn str(&self, name: &str) -> &str {
        self.metadata
            .get(name)
            .and_then(Value::as_str)
            .unwrap_or("")
    }

    /// String-list field, or empty when absent.
    pub fn list(&self, name: &str) -> Vec<String> {
        strings(self.metadata.get(name))
    }

    pub fn links(&self, name: &str) -> Vec<String> {
        strings(self.metadata.get("links").and_then(|links| links.get(name)))
    }

    pub fn section(&self, name: &str) -> String {
        match sections(&self.body)
            .ok()
            .and_then(|spans| spans.get(name).copied())
        {
            Some((start, end)) => self.body[start..end].trim().to_string(),
            None => String::new(),
        }
    }

    pub fn replace_section(&mut self, name: &str, text: &str) -> Result<()> {
        if !section_names(self.kind()).contains(&name) || name == "Log" {
            bail!("Section not editable: {name}");
        }
        if !sections(text)?.is_empty() {
            bail!("Section text cannot introduce another level-2 heading");
        }
        let Some(&(start, end)) = sections(&self.body)?.get(name) else {
            bail!("Missing section: {name}");
        };
        self.body = format!(
            "{}{}\n\n{}",
            &self.body[..start],
            text.trim(),
            &self.body[end..]
        );
        Ok(())
    }

    pub fn log(&mut self, event: &str, text: &str) -> Result<()> {
        let Some(&(_, end)) = sections(&self.body)?.get("Log") else {
            bail!("Missing section: Log");
        };
        let message = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let entry = format!("- {} [{event}] {message}\n", now());
        self.body = format!(
            "{}\n{entry}{}",
            self.body[..end].trim_end(),
            &self.body[end..]
        );
        Ok(())
    }

    pub fn render(&self) -> String {
        format!("---\n{}---\n\n{}", yaml::dump(&self.metadata), self.body)
    }
}

fn strings(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

pub fn parse_item(path: &Path) -> Result<Item> {
    static FRONTMATTER: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?s)\A---\r?\n(.*?)\r?\n---\r?\n(.*)\z").unwrap());
    let name = file_name(path);
    let bytes = fs::read(path).map_err(|error| Error(format!("{name}: {error}")))?;
    let text = String::from_utf8(bytes).map_err(|_| Error(format!("{name}: not valid UTF-8")))?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let Some(captures) = FRONTMATTER.captures(text) else {
        bail!("{name}: expected YAML frontmatter");
    };
    let Value::Object(metadata) = yaml::load(&captures[1])? else {
        bail!("{name}: metadata must be a mapping");
    };
    Ok(Item {
        metadata,
        body: captures[2].trim_start_matches('\n').to_string(),
        path: Some(path.to_path_buf()),
    })
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub fn atomic_write(path: &Path, text: &str) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("Path has no parent"))?;
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".dikt-")
        .tempfile_in(parent)?;
    temporary.write_all(text.as_bytes())?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(path)
        .map_err(|error| Error(format!("{}: {}", path.display(), error.error)))?;
    Ok(())
}

/// Join a `/`-separated relative path one component at a time (portable on Windows).
pub fn join_relative(base: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .filter(|part| !part.is_empty())
        .fold(base.to_path_buf(), |path, part| path.join(part))
}

/// Resolve symlinks for the existing part of a path, keeping the nonexistent remainder.
fn resolve(path: &Path) -> PathBuf {
    if let Ok(resolved) = dunce::canonicalize(path) {
        return resolved;
    }
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) => resolve(parent).join(name),
        _ => path.to_path_buf(),
    }
}

/// Find the collection: a directory with `dikt.toml`, or `<ancestor>/dikt/dikt.toml`.
pub fn discover_root(start: &Path) -> Result<PathBuf> {
    let origin = resolve(start);
    for candidate in origin.ancestors() {
        if candidate.join(MARKER).is_file() {
            return Ok(candidate.to_path_buf());
        }
        let nested = candidate.join(DEFAULT_COLLECTION);
        if nested.join(MARKER).is_file() {
            return Ok(nested);
        }
    }
    bail!(
        "No DIKT collection found (looked for {MARKER} or {DEFAULT_COLLECTION}/{MARKER} in this directory and its parents); run `dikt init` or pass --root PATH"
    )
}

/// Registered tag vocabulary.
#[derive(Debug, Clone, Default)]
pub struct Tags {
    pub known: HashSet<String>,
    pub broad: HashSet<String>,
}

/// Holds the collection's write lock until dropped.
pub struct WriteLock(PathBuf);

impl Drop for WriteLock {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.0);
    }
}

pub type Items = IndexMap<String, Item>;

pub struct Store {
    pub root: PathBuf,
    /// Recorded as `author` on items created through this store (see [`default_author`]).
    pub author: Option<String>,
    fail_write: Cell<Option<usize>>,
    write_count: Cell<usize>,
}

impl Store {
    pub fn new(root: &Path) -> Result<Self> {
        let root = dunce::canonicalize(root)
            .map_err(|error| Error(format!("{}: {error}", root.display())))?;
        Ok(Store {
            root,
            author: None,
            fail_write: Cell::new(None),
            write_count: Cell::new(0),
        })
    }

    pub fn with_author(mut self, author: Option<String>) -> Self {
        self.author = author;
        self
    }

    /// Test hook: make the n-th subsequent file write of a commit fail.
    #[doc(hidden)]
    pub fn fail_nth_write(&self, n: usize) {
        self.write_count.set(0);
        self.fail_write.set(Some(n));
    }

    fn write(&self, path: &Path, text: &str) -> Result<()> {
        let count = self.write_count.get() + 1;
        self.write_count.set(count);
        if self.fail_write.get() == Some(count) {
            bail!("Simulated disk failure");
        }
        atomic_write(path, text)
    }

    pub fn safe_path(&self, relative: &str) -> Result<PathBuf> {
        let path = join_relative(&self.root, relative);
        if Path::new(relative)
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
            || !resolve(&path).starts_with(&self.root)
        {
            bail!("Path escapes repository: {relative}");
        }
        Ok(path)
    }

    pub fn write_lock(&self) -> Result<WriteLock> {
        let path = self.safe_path(LOCK)?;
        match fs::create_dir(&path) {
            Ok(()) => Ok(WriteLock(path)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                bail!(
                    "Another dikt write is active. If it crashed, remove {LOCK} only after checking no dikt process is running."
                )
            }
            Err(error) => Err(error.into()),
        }
    }

    pub fn tags(&self) -> Result<Tags> {
        static CANONICAL: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"^[a-z0-9]+(?:-[a-z0-9]+)*$").unwrap());
        let text = fs::read_to_string(self.safe_path("tags.yaml")?)
            .map_err(|error| Error(format!("tags.yaml: {error}")))?;
        let document = yaml::load(&text)?;
        let as_strings = |value: Option<&Value>| -> Option<Vec<String>> {
            value?
                .as_array()?
                .iter()
                .map(|entry| entry.as_str().map(String::from))
                .collect()
        };
        let Some(values) = as_strings(document.get("tags"))
            .filter(|values| values.iter().all(|value| !value.trim().is_empty()))
        else {
            bail!("tags.yaml must contain a tags list of nonempty strings");
        };
        let known: HashSet<String> = values.iter().cloned().collect();
        if known.len() != values.len() {
            bail!("Duplicate tags in tags.yaml");
        }
        if values.iter().any(|value| !CANONICAL.is_match(value)) {
            bail!(
                "Tag names must use canonical lowercase letters, digits and hyphen-separated words"
            );
        }
        let Some(broad_values) = as_strings(document.get("broad_tags")) else {
            bail!("tags.yaml must contain an explicit broad_tags list");
        };
        let broad: HashSet<String> = broad_values.iter().cloned().collect();
        if broad.len() != broad_values.len() {
            bail!("Duplicate broad_tags in tags.yaml");
        }
        if !broad.is_subset(&known) {
            bail!("Every broad tag must also be registered in tags");
        }
        Ok(Tags { known, broad })
    }

    /// Strict load. Any invalid record fails the whole read.
    pub fn load(&self) -> Result<Items> {
        self.load_with(None)
    }

    /// Recovery load: invalid records and their dependents are skipped and described.
    pub fn load_recovering(&self, diagnostics: &mut Vec<String>) -> Result<Items> {
        self.load_with(Some(diagnostics))
    }

    fn load_with(&self, mut diagnostics: Option<&mut Vec<String>>) -> Result<Items> {
        let tags = self.tags()?;
        let mut result = Items::new();
        let mut duplicate_ids = HashSet::new();
        for (kind, _) in KINDS {
            let folder = directory(kind);
            let mut paths: Vec<PathBuf> = match fs::read_dir(self.safe_path(folder)?) {
                Ok(entries) => entries
                    .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                    .filter(|path| path.extension().is_some_and(|extension| extension == "md"))
                    .collect(),
                Err(_) => Vec::new(),
            };
            paths.sort();
            for path in paths {
                let attempt = (|| -> Result<Item> {
                    if !resolve(&path).starts_with(&self.root) {
                        bail!("Item path escapes repository: {}", file_name(&path));
                    }
                    let item = parse_item(&path)?;
                    self.validate_item(&item, &tags)?;
                    if item.kind() != kind
                        || !file_name(&path).starts_with(&format!("{}-", item.id()))
                    {
                        bail!("Wrong folder or filename for {}", item.id());
                    }
                    if result.contains_key(item.id()) || duplicate_ids.contains(item.id()) {
                        result.shift_remove(item.id());
                        duplicate_ids.insert(item.id().to_string());
                        bail!("Duplicate ID: {}; no copy selected", item.id());
                    }
                    Ok(item)
                })();
                match attempt {
                    Ok(item) => {
                        result.insert(item.id().to_string(), item);
                    }
                    Err(error) => match diagnostics.as_deref_mut() {
                        Some(found) => {
                            found.push(format!("Skipped {folder}/{}: {error}", file_name(&path)))
                        }
                        None => return Err(error),
                    },
                }
            }
        }
        match diagnostics {
            None => validate_links(&result, None)?,
            Some(found) => loop {
                let invalid: Vec<String> = result
                    .keys()
                    .filter_map(
                        |identifier| match validate_links(&result, Some(identifier)) {
                            Ok(()) => None,
                            Err(error) => {
                                found.push(format!("Skipped {identifier}: {error}"));
                                Some(identifier.clone())
                            }
                        },
                    )
                    .collect();
                if invalid.is_empty() {
                    break;
                }
                for identifier in invalid {
                    result.shift_remove(&identifier);
                }
            },
        }
        Ok(result)
    }

    pub fn validate_item(&self, item: &Item, tags: &Tags) -> Result<()> {
        let kind = match item.metadata.get("type") {
            Some(Value::String(kind)) if prefix(kind).is_some() => kind.clone(),
            other => bail!(
                "Unknown item type: {}",
                other.map_or("None".into(), |value| value.to_string())
            ),
        };
        let defaults = defaults(&kind);
        let unknown: BTreeSet<&String> = item
            .metadata
            .keys()
            .filter(|key| {
                !defaults.contains_key(*key)
                    && !COMMON.contains(&key.as_str())
                    && !OPTIONAL.contains(&key.as_str())
            })
            .collect();
        if !unknown.is_empty() {
            bail!("Unknown fields: {:?}", unknown.iter().collect::<Vec<_>>());
        }
        let identifier = item
            .metadata
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("?");
        schema::check(&item.metadata, &kind)
            .map_err(|message| Error(format!("{identifier}: {message}")))?;
        let unknown_tags: BTreeSet<String> = item
            .list("tags")
            .into_iter()
            .filter(|tag| !tags.known.contains(tag))
            .collect();
        if !unknown_tags.is_empty() {
            bail!(
                "Unknown tags: {:?}; register canonical tags in tags.yaml first",
                unknown_tags.iter().collect::<Vec<_>>()
            );
        }
        if item.list("tags").iter().all(|tag| tags.broad.contains(tag)) {
            bail!(
                "{}: at least one specific tag outside broad_tags is required",
                item.id()
            );
        }
        let present = sections(&item.body)?;
        let missing: BTreeSet<&str> = section_names(&kind)
            .iter()
            .copied()
            .filter(|name| !present.contains_key(*name))
            .collect();
        if !missing.is_empty() {
            bail!(
                "Missing body sections: {:?}",
                missing.iter().collect::<Vec<_>>()
            );
        }
        let status = item.status();
        if kind == "decision" {
            if matches!(status, "accepted" | "superseded")
                && (item.metadata["decided"].is_null()
                    || RATIONALE.iter().any(|name| item.section(name).is_empty()))
            {
                bail!(
                    "Accepted decisions need decided date and Context, Options, Decision, Consequences"
                );
            }
            if (status == "superseded") != !item.metadata["superseded_by"].is_null() {
                bail!("Superseded status and superseded_by must agree");
            }
        }
        if kind == "idea" && (status == "promoted") != !item.metadata["promoted_to"].is_null() {
            bail!("Promoted status and promoted_to must agree");
        }
        if kind == "task" && status == "done" && item.section("Outcome").is_empty() {
            bail!("Done tasks need an Outcome; use dikt close --outcome");
        }
        Ok(())
    }

    pub fn new_item(
        &self,
        items: &mut Items,
        kind: &str,
        title: &str,
        description: &str,
        fields: Map<String, Value>,
    ) -> Result<String> {
        let Some(letter) = prefix(kind) else {
            bail!("Unknown type: {kind}");
        };
        let tags: Vec<String> = fields
            .get("tags")
            .and_then(Value::as_array)
            .map(|tags| {
                tags.iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();
        let author = fields
            .get("author")
            .and_then(Value::as_str)
            .map(String::from)
            .or_else(|| self.author.clone());
        let timestamp = now();
        let identifier = new_id(
            letter,
            items,
            kind,
            title,
            &tags,
            &timestamp,
            author.as_deref().unwrap_or(""),
        );
        let mut metadata = Map::new();
        metadata.insert("schema_version".into(), json!(1));
        metadata.insert("id".into(), json!(identifier));
        metadata.insert("type".into(), json!(kind));
        metadata.insert("title".into(), json!(title));
        if let Some(author) = &author {
            metadata.insert("author".into(), json!(author));
        }
        metadata.insert("tags".into(), json!([]));
        metadata.insert("created".into(), json!(timestamp));
        metadata.insert("updated".into(), json!(timestamp));
        metadata.insert(
            "links".into(),
            Value::Object(
                LINK_KINDS
                    .iter()
                    .map(|name| (name.to_string(), json!([])))
                    .collect(),
            ),
        );
        metadata.extend(defaults(kind));
        metadata.extend(fields);
        let body: String = section_names(kind)
            .iter()
            .map(|name| format!("## {name}\n\n"))
            .collect();
        let mut item = Item {
            metadata,
            body,
            path: None,
        };
        item.replace_section("Description", description)?;
        item.log("created", title)?;
        items.insert(identifier.clone(), item);
        Ok(identifier)
    }

    /// Validate the whole collection, then write changed items and Backlog.md, rolling back on failure.
    pub fn commit(&self, items: &Items, changed: &[&str]) -> Result<()> {
        let tags = self.tags()?;
        for item in items.values() {
            self.validate_item(item, &tags)?;
        }
        validate_links(items, None)?;
        let mut writes: Vec<(PathBuf, String)> = Vec::new();
        let mut removals = Vec::new();
        for identifier in changed {
            let item = &items[*identifier];
            let target = self.safe_path(&format!(
                "{}/{}-{}.md",
                directory(item.kind()),
                item.id(),
                slug(item.str("title"))
            ))?;
            if target.exists() && Some(&target) != item.path.as_ref() {
                bail!("Target already exists: {}", file_name(&target));
            }
            writes.push((target.clone(), item.render()));
            if let Some(previous) = &item.path
                && *previous != target
            {
                removals.push(previous.clone());
            }
        }
        writes.push((
            self.safe_path("Backlog.md")?,
            crate::views::backlog_text(items, None),
        ));
        let mut originals: Vec<(PathBuf, Option<String>)> = Vec::new();
        for path in writes.iter().map(|(path, _)| path).chain(removals.iter()) {
            if !originals.iter().any(|(seen, _)| seen == path) {
                originals.push((
                    path.clone(),
                    if path.exists() {
                        Some(fs::read_to_string(path)?)
                    } else {
                        None
                    },
                ));
            }
        }
        let attempt = (|| -> Result<()> {
            for (path, text) in &writes {
                self.write(path, text)?;
            }
            for path in &removals {
                fs::remove_file(path)?;
            }
            Ok(())
        })();
        if let Err(error) = attempt {
            for (path, text) in &originals {
                match text {
                    None => {
                        let _ = fs::remove_file(path);
                    }
                    Some(text) => {
                        let _ = self.write(path, text);
                    }
                }
            }
            return Err(error);
        }
        Ok(())
    }

    pub fn create(
        &self,
        kind: &str,
        title: &str,
        description: &str,
        body_sections: &[(&str, String)],
        fields: Map<String, Value>,
    ) -> Result<Item> {
        let _lock = self.write_lock()?;
        let mut items = self.load()?;
        let identifier = self.new_item(&mut items, kind, title, description, fields)?;
        let item = items.get_mut(&identifier).unwrap();
        for (name, text) in body_sections {
            item.replace_section(name, text)?;
        }
        if kind == "decision" && item.status() == "accepted" && item.metadata["decided"].is_null() {
            item.metadata
                .insert("decided".into(), json!(today().to_string()));
        }
        self.commit(&items, &[&identifier])?;
        Ok(items[&identifier].clone())
    }

    pub fn change(&self, identifier: &str, request: Change) -> Result<Item> {
        let _lock = self.write_lock()?;
        let mut items = self.load()?;
        let item = items
            .get_mut(identifier)
            .ok_or_else(|| Error(format!("Item not found: {identifier}")))?;
        let Change {
            fields,
            section,
            mut event,
            mut message,
            add_tags,
            remove_tags,
            link,
            check,
            uncheck,
            checklist,
        } = request;
        let mut protected = vec![
            "id",
            "type",
            "schema_version",
            "created",
            "updated",
            "author",
            "superseded_by",
            "promoted_to",
        ];
        if item.kind() != "knowledge" {
            protected.push("supersedes");
        }
        if fields.keys().any(|key| protected.contains(&key.as_str())) {
            bail!(
                "Identity/history fields are immutable; use promote or supersede for lifecycle links"
            );
        }
        if item.kind() == "decision"
            && matches!(item.status(), "accepted" | "superseded")
            && (section.is_some()
                || fields
                    .keys()
                    .any(|key| key != "revisit" && key != "revisit_when"))
        {
            bail!("Accepted rationale is immutable; create and supersede with a new decision");
        }
        if event == "closed"
            && (item.kind() != "task" || matches!(item.status(), "done" | "cancelled"))
        {
            bail!("Only unfinished tasks can be closed");
        }
        let previous_status = item.status().to_string();
        item.metadata.extend(fields.clone());
        let mut tags: IndexMap<String, ()> = IndexMap::new();
        for tag in item
            .list("tags")
            .into_iter()
            .filter(|tag| !remove_tags.contains(tag))
            .chain(add_tags)
        {
            tags.insert(tag, ());
        }
        item.metadata
            .insert("tags".into(), json!(tags.keys().collect::<Vec<_>>()));
        if let Some((name, text)) = section {
            item.replace_section(&name, &text)?;
        }
        if let Some((name, value)) = link {
            let Some(Value::Array(entries)) = item
                .metadata
                .get_mut("links")
                .and_then(|links| links.get_mut(&name))
            else {
                bail!("Link must be jira|mr|commits|confluence|items=VALUE");
            };
            if value.trim().is_empty() {
                bail!("Link must be jira|mr|commits|confluence|items=VALUE");
            }
            if !entries.contains(&json!(value)) {
                entries.push(json!(value));
            }
        }
        if check.is_some() || uncheck.is_some() || checklist.is_some() {
            if item.kind() != "task" {
                bail!("Checklists apply only to tasks");
            }
            let Some(Value::Array(entries)) = item.metadata.get_mut("checklist") else {
                bail!("Task checklist must be a list");
            };
            if let Some(text) = checklist {
                entries.push(json!({"text": text, "done": false}));
            }
            for (index, done) in [(check, true), (uncheck, false)] {
                if let Some(index) = index {
                    if index < 1 || index as usize > entries.len() {
                        bail!("Checklist index is 1-based and must exist");
                    }
                    match entries[index as usize - 1].as_object_mut() {
                        Some(entry) => {
                            entry.insert("done".into(), json!(done));
                        }
                        None => bail!("Checklist entries must be mappings"),
                    }
                }
            }
        }
        if item.kind() == "decision"
            && item.status() == "accepted"
            && item.metadata.get("decided").is_none_or(Value::is_null)
        {
            item.metadata
                .insert("decided".into(), json!(today().to_string()));
        }
        item.metadata.insert("updated".into(), json!(now()));
        if item.kind() == "task" && previous_status != "done" && item.status() == "done" {
            event = "closed".into();
            message = item.section("Outcome");
        }
        let text = if message.is_empty() {
            yaml::py_json(&Value::Object(fields))
        } else {
            message
        };
        item.log(&event, &text)?;
        self.commit(&items, &[identifier])?;
        Ok(items[identifier].clone())
    }

    pub fn promote(&self, identifier: &str) -> Result<Item> {
        let _lock = self.write_lock()?;
        let mut items = self.load()?;
        let idea = require(&items, identifier)?.clone();
        if idea.kind() != "idea" || idea.status() != "open" {
            bail!("Only open ideas can be promoted");
        }
        let mut fields = Map::new();
        fields.insert("tags".into(), json!(idea.list("tags")));
        let task = self.new_item(
            &mut items,
            "task",
            idea.str("title"),
            &idea.section("Description"),
            fields,
        )?;
        push_link(items.get_mut(&task).unwrap(), &idea.metadata["id"]);
        let idea = items.get_mut(identifier).unwrap();
        idea.metadata.insert("status".into(), json!("promoted"));
        idea.metadata.insert("promoted_to".into(), json!(task));
        idea.metadata.insert("updated".into(), json!(now()));
        push_link(idea, &json!(task));
        idea.log("promoted", &format!("Promoted to {task}"))?;
        self.commit(&items, &[identifier, &task])?;
        Ok(items[&task].clone())
    }

    pub fn supersede(&self, identifier: &str, replacement: &str) -> Result<Item> {
        let _lock = self.write_lock()?;
        let mut items = self.load()?;
        let (old, new) = (require(&items, identifier)?, require(&items, replacement)?);
        if old.id() == new.id()
            || [old, new]
                .iter()
                .any(|item| item.kind() != "decision" || item.status() != "accepted")
        {
            bail!("Supersession needs two distinct accepted decisions");
        }
        let old = items.get_mut(identifier).unwrap();
        old.metadata.insert("status".into(), json!("superseded"));
        old.metadata
            .insert("superseded_by".into(), json!(replacement));
        old.metadata.insert("updated".into(), json!(now()));
        old.log("superseded", &format!("Replaced by {replacement}"))?;
        let new = items.get_mut(replacement).unwrap();
        if let Some(Value::Array(entries)) = new.metadata.get_mut("supersedes") {
            entries.push(json!(identifier));
        }
        new.metadata.insert("updated".into(), json!(now()));
        new.log("supersedes", &format!("Replaces {identifier}"))?;
        self.commit(&items, &[identifier, replacement])?;
        Ok(items[replacement].clone())
    }
}

fn push_link(item: &mut Item, value: &Value) {
    if let Some(Value::Array(entries)) = item
        .metadata
        .get_mut("links")
        .and_then(|links| links.get_mut("items"))
    {
        entries.push(value.clone());
    }
}

/// A requested modification for [`Store::change`].
#[derive(Debug, Clone)]
pub struct Change {
    pub fields: Map<String, Value>,
    pub section: Option<(String, String)>,
    pub event: String,
    pub message: String,
    pub add_tags: Vec<String>,
    pub remove_tags: Vec<String>,
    pub link: Option<(String, String)>,
    pub check: Option<i64>,
    pub uncheck: Option<i64>,
    pub checklist: Option<String>,
}

impl Default for Change {
    fn default() -> Self {
        Change {
            fields: Map::new(),
            section: None,
            event: "updated".into(),
            message: String::new(),
            add_tags: Vec::new(),
            remove_tags: Vec::new(),
            link: None,
            check: None,
            uncheck: None,
            checklist: None,
        }
    }
}

impl Change {
    pub fn fields(fields: Value) -> Self {
        Change {
            fields: fields.as_object().cloned().unwrap_or_default(),
            ..Default::default()
        }
    }
}

/// Number of leading tags that feed the ID hash.
const ID_TAGS: usize = 10;

/// A collision-resistant ID: the type letter plus 8 hex digits of a SHA-256 over the
/// type, title, first ten tags, creation time and author. Two people creating items
/// on separate branches get different IDs (and therefore different file names), so
/// their branches merge without conflicts. `nonce` only matters on a local collision.
pub fn new_id(
    letter: &str,
    items: &Items,
    kind: &str,
    title: &str,
    tags: &[String],
    created: &str,
    author: &str,
) -> String {
    let tags = tags
        .iter()
        .take(ID_TAGS)
        .cloned()
        .collect::<Vec<_>>()
        .join(",");
    for nonce in 0u32.. {
        let mut hasher = Sha256::new();
        for part in [kind, title, &tags, created, author, &nonce.to_string()] {
            hasher.update(part.as_bytes());
            hasher.update([0]);
        }
        let digest = hasher.finalize();
        let hex: String = digest[..4]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let identifier = format!("{letter}-{hex}");
        if !items.contains_key(&identifier) {
            return identifier;
        }
    }
    unreachable!("a free ID always exists")
}

/// Who is writing: `DIKT_AUTHOR`, else `git config user.name` of the collection, else the OS user.
pub fn default_author(root: &Path) -> Option<String> {
    let clean = |text: &str| {
        let text = text.trim();
        (!text.is_empty() && !text.contains(['\r', '\n'])).then(|| text.to_string())
    };
    if let Some(name) = std::env::var("DIKT_AUTHOR")
        .ok()
        .and_then(|name| clean(&name))
    {
        return Some(name);
    }
    let git = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["config", "user.name"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| clean(&String::from_utf8_lossy(&output.stdout)));
    git.or_else(|| {
        ["USER", "USERNAME"]
            .iter()
            .find_map(|name| std::env::var(name).ok().and_then(|value| clean(&value)))
    })
}

/// Creation time as a sortable instant (microseconds since the epoch; unparsable sorts first).
pub fn created_micros(item: &Item) -> i64 {
    DateTime::parse_from_rfc3339(item.str("created"))
        .map(|time| time.timestamp_micros())
        .unwrap_or(0)
}

pub fn references(item: &Item) -> Vec<String> {
    let mut result = item.links("items");
    result.extend(item.list("based_on"));
    result.extend(item.list("supersedes"));
    for field in ["promoted_to", "superseded_by"] {
        if !item.str(field).is_empty() {
            result.push(item.str(field).to_string());
        }
    }
    result
}

pub fn validate_links(items: &Items, only: Option<&str>) -> Result<()> {
    let selected: Vec<&Item> = match only {
        Some(identifier) => vec![&items[identifier]],
        None => items.values().collect(),
    };
    for item in selected {
        for reference in references(item) {
            if !items.contains_key(&reference) || reference == item.id() {
                bail!("{}: missing or self reference {reference}", item.id());
            }
        }
        let superseded_by = item.str("superseded_by");
        if !superseded_by.is_empty()
            && !items[superseded_by]
                .list("supersedes")
                .iter()
                .any(|entry| entry == item.id())
        {
            bail!("{}: missing reciprocal supersedes link", item.id());
        }
        if item.kind() == "decision" {
            for reference in item.list("supersedes") {
                if items[&reference].str("superseded_by") != item.id() {
                    bail!("{}: missing reciprocal superseded_by link", item.id());
                }
            }
        }
        let mut seen = HashSet::from([item.id().to_string()]);
        let mut next = item.str("superseded_by").to_string();
        while !next.is_empty() {
            let Some(target) = items.get(&next) else {
                bail!("{}: missing reference {next}", item.id());
            };
            if !seen.insert(next.clone()) {
                bail!("Decision supersession cycle");
            }
            next = target.str("superseded_by").to_string();
        }
    }
    Ok(())
}

pub fn require<'a>(items: &'a Items, identifier: &str) -> Result<&'a Item> {
    items
        .get(identifier)
        .ok_or_else(|| Error(format!("Item not found: {identifier}")))
}

/// Hand-written equivalent of the JSON schemas (common + one per type).
mod schema {
    use super::*;

    type Check = std::result::Result<(), String>;

    fn pattern(text: &str, regex: &str) -> bool {
        Regex::new(regex).unwrap().is_match(text)
    }

    fn date(value: &Value, name: &str, nullable: bool) -> Check {
        match value {
            Value::Null if nullable => Ok(()),
            Value::String(text) if parse_date(text).is_ok() => Ok(()),
            other => Err(format!(
                "{name}: {other} is not a valid date (YYYY-MM-DD{})",
                if nullable { " or null" } else { "" }
            )),
        }
    }

    fn id_list(value: &Value, name: &str, regex: &str) -> Check {
        let Some(entries) = value.as_array() else {
            return Err(format!("{name}: {value} is not of type 'array'"));
        };
        let mut seen = HashSet::new();
        for entry in entries {
            match entry.as_str() {
                Some(text) if pattern(text, regex) => {
                    if !seen.insert(text) {
                        return Err(format!("{name}: {value} has non-unique elements"));
                    }
                }
                _ => return Err(format!("{name}: {entry} does not match '{regex}'")),
            }
        }
        Ok(())
    }

    fn string_list(value: &Value, name: &str, nonempty: bool) -> Check {
        id_list(
            value,
            name,
            if nonempty { r"^[\s\S]+$" } else { r"^[\s\S]*$" },
        )
    }

    fn one_of(value: &Value, name: &str, allowed: &[&str]) -> Check {
        match value.as_str() {
            Some(text) if allowed.contains(&text) => Ok(()),
            _ => Err(format!("{name}: {value} is not one of {allowed:?}")),
        }
    }

    fn optional_id(value: &Value, name: &str, regex: &str) -> Check {
        match value {
            Value::Null => Ok(()),
            Value::String(text) if pattern(text, regex) => Ok(()),
            other => Err(format!(
                "{name}: {other} is not null and does not match '{regex}'"
            )),
        }
    }

    pub fn check(metadata: &Map<String, Value>, kind: &str) -> Check {
        let defaults = defaults(kind);
        for name in COMMON
            .iter()
            .copied()
            .chain(defaults.keys().map(String::as_str))
        {
            if !metadata.contains_key(name) {
                return Err(format!("'{name}' is a required property"));
            }
        }
        let field = |name: &str| &metadata[name];
        if field("schema_version") != &json!(1) {
            return Err("schema_version: 1 was expected".into());
        }
        let letter = prefix(kind).unwrap();
        match field("id").as_str() {
            Some(id) if pattern(id, &format!("^{letter}-(?:[0-9a-f]{{8}}|[0-9]{{5,}})$")) => {}
            _ => {
                return Err(format!(
                    "id: {} does not match '^{letter}-(?:[0-9a-f]{{8}}|[0-9]{{5,}})$'",
                    field("id")
                ));
            }
        }
        match field("title").as_str() {
            Some(title) if !title.trim().is_empty() && !title.contains(['\r', '\n']) => {}
            _ => return Err("title: must be a nonempty single-line string".into()),
        }
        if let Some(author) = metadata.get("author")
            && !author
                .as_str()
                .is_some_and(|name| !name.trim().is_empty() && !name.contains(['\r', '\n']))
        {
            return Err("author: must be a nonempty single-line string".into());
        }
        string_list(field("tags"), "tags", false)?;
        for name in ["created", "updated"] {
            if !field(name)
                .as_str()
                .is_some_and(|text| DateTime::parse_from_rfc3339(text).is_ok())
            {
                return Err(format!("{name}: {} is not a 'date-time'", field(name)));
            }
        }
        let Some(links) = field("links").as_object() else {
            return Err("links: must be a mapping".into());
        };
        for name in LINK_KINDS {
            string_list(
                links
                    .get(name)
                    .ok_or(format!("links: '{name}' is a required property"))?,
                &format!("links.{name}"),
                true,
            )?;
        }
        if let Some(extra) = links.keys().find(|key| !LINK_KINDS.contains(&key.as_str())) {
            return Err(format!(
                "links: additional property '{extra}' is not allowed"
            ));
        }
        match kind {
            "task" => {
                one_of(
                    field("status"),
                    "status",
                    &["todo", "in_progress", "blocked", "done", "cancelled"],
                )?;
                match field("priority").as_i64() {
                    Some(1..=4) if !field("priority").is_f64() => {}
                    _ => {
                        return Err(format!(
                            "priority: {} is not an integer from 1 to 4",
                            field("priority")
                        ));
                    }
                }
                date(field("due"), "due", true)?;
                let Some(entries) = field("checklist").as_array() else {
                    return Err("checklist: must be a list".into());
                };
                for entry in entries {
                    let valid = entry.as_object().is_some_and(|entry| {
                        entry.len() == 2
                            && entry
                                .get("text")
                                .and_then(Value::as_str)
                                .is_some_and(|text| !text.is_empty())
                            && entry.get("done").is_some_and(Value::is_boolean)
                    });
                    if !valid {
                        return Err(format!(
                            "checklist: {entry} must be {{text: nonempty string, done: boolean}}"
                        ));
                    }
                }
            }
            "idea" => {
                one_of(field("status"), "status", &["open", "promoted", "dropped"])?;
                optional_id(
                    field("promoted_to"),
                    "promoted_to",
                    r"^T-(?:[0-9a-f]{8}|[0-9]{5,})$",
                )?;
            }
            "knowledge" => {
                one_of(field("status"), "status", &["active", "outdated"])?;
                if !field("source")
                    .as_str()
                    .is_some_and(|source| !source.trim().is_empty())
                {
                    return Err("source: a nonempty source is required".into());
                }
                date(field("last_verified"), "last_verified", true)?;
                id_list(
                    field("supersedes"),
                    "supersedes",
                    r"^K-(?:[0-9a-f]{8}|[0-9]{5,})$",
                )?;
            }
            "decision" => {
                one_of(
                    field("status"),
                    "status",
                    &["proposed", "accepted", "rejected", "superseded"],
                )?;
                date(field("decided"), "decided", true)?;
                date(field("revisit"), "revisit", true)?;
                if !field("revisit_when").is_string() {
                    return Err("revisit_when: must be a string".into());
                }
                id_list(
                    field("based_on"),
                    "based_on",
                    r"^K-(?:[0-9a-f]{8}|[0-9]{5,})$",
                )?;
                id_list(
                    field("supersedes"),
                    "supersedes",
                    r"^D-(?:[0-9a-f]{8}|[0-9]{5,})$",
                )?;
                optional_id(
                    field("superseded_by"),
                    "superseded_by",
                    r"^D-(?:[0-9a-f]{8}|[0-9]{5,})$",
                )?;
            }
            _ => unreachable!(),
        }
        Ok(())
    }
}
