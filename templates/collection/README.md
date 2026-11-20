# DIKT

DIKT manages decisions, ideas, knowledge and tasks as local Markdown files.
The `dikt` CLI provides deterministic validation, mutation, retrieval and
reporting. An AI agent provides interpretation, research, triage and decision
analysis. There are no remote-service integrations or automatic synchronization.

Agent rules: [AGENTS.md](AGENTS.md). Placement and ownership: [STRUCTURE.md](STRUCTURE.md).

## Running

`{{DIKT}}` works from the project root or any subdirectory: it walks up the
directory tree looking for `dikt.toml` or `{{ROOT}}/dikt.toml`. Elsewhere,
pass the collection explicitly before the subcommand:

```sh
{{DIKT}} --root /absolute/path/to/{{ROOT}} validate
DIKT_ROOT=/absolute/path/to/{{ROOT}} {{DIKT}} list --query "topic"
```

## CLI Contract

All examples are illustrative. Obtain real IDs from `add`; do not assume
an example ID points at the intended record.

| Command | Purpose |
|---|---|
| `dikt add TYPE --title TEXT --tag TAG` | Create task, idea, knowledge or decision |
| `dikt get ID --full` | Read metadata and body |
| `dikt list --type TYPE --query WORDS` | Filter and search records |
| `dikt update ID --status VALUE` | Update validated metadata |
| `dikt update ID --set FIELD=VALUE` | Other metadata; JSON values supported |
| `dikt edit ID --section NAME --text TEXT` | Replace one permitted body section |
| `dikt edit ID --section NAME --file FILE` | Read UTF-8 section text from a file |
| `dikt log ID TEXT` | Append a timestamped note |
| `dikt close ID --outcome TEXT` | Complete a task with a real outcome |
| `dikt promote I-ID` | Create a task and retain linked original idea |
| `dikt supersede D-ID --by D-ID` | Link two accepted decisions |
| `dikt report [today/yesterday/YYYY-MM-DD/week]` | Save a report; selectors are alternatives |
| `dikt report week --date YYYY-MM-DD` | Select an ISO week |
| `dikt backlog` | Refresh the local, generated backlog |
| `dikt validate` | Strict schema, tag, reference and lifecycle checks |
| `dikt init --refresh` | Update dikt-managed docs and skills |

Run command-specific `--help` for actual options. `list` defaults to 20 results;
use `--limit` and `--offset` to page. Filters include `--status`, `--tag`,
`--since`, `--due`, `--based-on`, `--related` and `--revisit-due`. Search
matches all whitespace-separated words case-insensitively across rendered
metadata and body, including logs. Ordering is by stable ID, not relevance.
Use `--fields id,title,tags` for projection and `--json` for machine output.
Diagnostics are on stderr. Search free-form resources separately.

`update` supports `--add-tag`, `--rm-tag`, `--add-check`, `--check N` and
`--uncheck N` (1-based), plus `--link KIND=VALUE`. `--set due=null` clears
a due date. `--set` uses JSON when parseable, otherwise a string. Identity,
version and managed lifecycle-link fields cannot be casually overwritten.
The generic external-reference link slots (jira, mr, commits, confluence)
are optional metadata; they execute no remote calls and imply no integration.

## Item Lifecycle

Tasks: todo, in_progress, blocked, done, cancelled; priority 1 (high) to 4;
optional due date, checklist and required Outcome for done records.
Ideas: open, promoted, dropped; promote once and retain the original.
Knowledge: active or outdated; nonempty source, optional last_verified date,
and optional supersedes knowledge IDs. Do not claim verification without evidence.
Decisions: proposed, accepted, rejected, superseded. Default is proposed.
Acceptance requires decided date and Context, Options, Decision, Consequences.
Only accept choices explicitly approved by the user. Accepted rationale is
immutable; create and supersede with a new accepted decision to change it.
Evidence IDs in based_on must identify existing knowledge. Due revisit dates
and outdated/superseded evidence flag review without reversing the choice.
Evaluate free-text revisit conditions against observed facts.

Filenames contain stable prefixed IDs: T-, I-, K-, D-, with at least five
digits. Title changes rename the file, never the ID. Closed records stay put.
Creation/update timestamps carry a timezone; calendar fields use YYYY-MM-DD.

## Tagging Policy

Every record needs at least one specific registered tag outside broad_tags.
Tags are lowercase letters/digits with hyphen-separated words. The registry
explicitly lists all tags and a duplicate-free broad_tags subset. Broad
context such as architecture or tooling is supplementary.
Tag every central technology, format and genuinely relevant workflow.
Prefer including a useful specific retrieval key over omitting it. No count
limit or quota exists: 20 or more tags are valid if all are relevant.
Avoid incidental mentions, synonyms, filler and status/type duplicates.
Agents may register clearly applicable canonical tags before use without
asking individually. Classify new broad categories too; ask about ambiguity.
Validation cannot determine semantic relevance or completeness.

## Reports and Procedures

Inbox.md holds drafts; Scratch.md holds temporary current-task notes.
Preserve useful content and obtain authorization before removing entries.
Reports use dated event logs, not inferred work or time tracking. Generated
sections are bounded by markers; preserve handwritten Summary text outside
them. Weekly generation refreshes daily activity and incorporates summaries.
Backlog.md is disposable and gitignored; regenerate as dates change.
Use the dikt-triage-inbox and dikt-weekly-review skills for the two included
procedures. Both procedures start untested.

## Recovery and Limits

Default reads, writes and validate are strict. One bad record can block them.
`dikt list --skip-invalid --json` and `dikt get ID --skip-invalid` are explicit
recovery reads: warn, exclude invalid/ambiguous records and invalid dependents,
and never repair files. A valid tag registry is still required. Absence from
partial recovery results is not proof that a record does not exist.
File writes use atomic replacement and serialize writers with `.dikt-write.lock`.
Ordinary multi-file write failures attempt rollback, but process/OS crashes
can interrupt a multi-file change. Inspect differences, back up and validate
before approved repair. Remove a stale lock only if no writer is running.
IDs allocate from retained files: never delete records or reuse IDs. Separate
clones can allocate conflicting IDs; resolve Git conflicts explicitly.
No database, full-text index, embeddings, MCP adapter or migration command
is implemented.

## Routine Maintenance

1. Validate; review tasks, sources, procedures and decisions narrowly. Keep
   unsupported findings out of durable memory.
2. Register replacement tags, retag via dikt, validate, then retire unused
   names. Before marking a category broad, first fix affected broad-only items.
3. Back up authoritative records, reports, notes, procedures and configuration.
   Git alone does not protect uncommitted notes.
4. After upgrading the dikt package, run `{{DIKT}} validate` and
   `{{DIKT}} init --refresh` to pick up updated docs and skills.
