# DIKT Tutorial

This guide takes you from installation to day-to-day use of `dikt`, both by
hand and through AI agents. Every command and output below was produced by a
real `dikt` run. Only timestamps and paths differ on your machine.

**Contents**

1. [What DIKT is](#1-what-dikt-is)
2. [Install](#install)
3. [Set up a project](#3-set-up-a-project)
4. [Tags come first](#4-tags-come-first)
5. [Tasks](#5-tasks)
6. [Ideas](#6-ideas)
7. [Knowledge](#7-knowledge)
8. [Decisions](#8-decisions)
9. [Finding things](#9-finding-things)
10. [Backlog and reports](#10-backlog-and-reports)
11. [Inbox, Scratch and procedures](#11-inbox-scratch-and-procedures)
12. [Working with AI agents](#12-working-with-ai-agents)
13. [Running from anywhere](#13-running-from-anywhere)
14. [Git and teams](#14-git-and-teams)
15. [Recovery and troubleshooting](#15-recovery-and-troubleshooting)
16. [Upgrading and uninstalling](#16-upgrading-and-uninstalling)
17. [Reference](#17-reference)

---

## 1. What DIKT is

DIKT is the durable memory of a project, stored as plain Markdown files
inside the project itself:

| Type | Prefix | Use it for | Statuses |
|---|---|---|---|
| **Task** | `T-` | Something to do | todo, in_progress, blocked, done, cancelled |
| **Idea** | `I-` | A possibility worth keeping | open, promoted, dropped |
| **Knowledge** | `K-` | A verified, reusable fact with a source | active, outdated |
| **Decision** | `D-` | A choice and its rationale | proposed, accepted, rejected, superseded |

Every item has a stable ID: its type letter and 8 hex digits, such as
`T-3f9a1c2e`. The digits are a hash of the type, title, first ten tags,
creation time and author, so people working on separate branches practically
never create the same ID (see [Git and teams](#14-git-and-teams)). They carry no
order; `dikt list` shows the oldest items first. Each new item also records
its `author`: the `--author` value, else `$DIKT_AUTHOR`, else
`git config user.name`.

The `dikt` CLI is the only writer. It validates every change: required
fields, allowed statuses, tags, links between items and lifecycle rules. For
example, a done task needs an outcome, and an accepted decision needs its full
rationale. Invalid writes are rejected and nothing is changed on disk.

Why this helps with AI agents: agents forget everything between chats. With
DIKT they can **retrieve** earlier decisions and evidence before reasoning,
and **preserve** what they verified or decided. The CLI keeps the records
consistent even when an agent makes a mistake.

---

## Install

You need [uv](https://docs.astral.sh/uv/) for options A–C. Option D needs
nothing at all.

> **Note:** the package is called **`dikt-cli`**; the command it installs is
> `dikt`. The PyPI package named `dikt` is an **unrelated** project.

### A. Into a uv project (recommended)

One command, from the project directory:

```sh
uv add --dev dikt-cli
uv run dikt --version
```

This adds `dikt-cli` to the project's dev dependencies and pins it in
`uv.lock`, so everyone who runs `uv sync` gets the same version. If the
project has no `pyproject.toml` yet, run `uv init` first.

[`dikt-cli` on PyPI](https://pypi.org/project/dikt-cli/) ships prebuilt
binaries for Linux x86_64/aarch64 (glibc ≥ 2.17), Linux x86_64 musl, macOS
x86_64/arm64 and Windows x86_64, so no Rust is needed. On other platforms uv
builds it from source, which needs Rust (see B).

### B. From source (needs Rust ≥ 1.88)

```sh
uv add --dev "dikt-cli @ git+https://github.com/dark-neonus/dikt"
# or from a local checkout:
uv add --dev /path/to/dikt
# or globally with Cargo:
cargo install --git https://github.com/dark-neonus/dikt
```

### C. As a global tool

```sh
uv tool install dikt-cli
dikt --version
```

To try it without installing anything: `uvx --from dikt-cli dikt --help`.

### D. Standalone binary

Download the archive for your platform from the
[releases page](https://github.com/dark-neonus/dikt/releases), check it
against `SHA256SUMS`, and put `dikt` somewhere on your `PATH`:

```sh
# Linux x86_64 (static musl build)
curl -LO https://github.com/dark-neonus/dikt/releases/download/v0.1.0/dikt-v0.1.0-x86_64-unknown-linux-musl.tar.gz
tar xzf dikt-v0.1.0-x86_64-unknown-linux-musl.tar.gz
install dikt-v0.1.0-x86_64-unknown-linux-musl/dikt ~/.local/bin/
```

Other targets: `aarch64-unknown-linux-musl`, `x86_64-apple-darwin`,
`aarch64-apple-darwin` (`.tar.gz`) and `x86_64-pc-windows-msvc` (`.zip`).

### Which command to type

If dikt is a project dependency (A or B), run it as **`uv run dikt ...`**.
If it's on your `PATH` (C or D), just run **`dikt ...`**. The rest of this
tutorial writes `dikt`; prefix it with `uv run` when needed.

---

## 3. Set up a project

From the project root:

```console
$ dikt init
  dikt/dikt.toml: created
  dikt/tags.yaml: created
  ...
  .claude/skills/dikt/SKILL.md: created
  .github/skills/dikt/SKILL.md: created
  .github/prompts/dikt-triage-inbox.prompt.md: created
  AGENTS.md: section added
  CLAUDE.md: created

DIKT collection: /home/me/weather-app/dikt (0 items, valid)
Agent command: uv run dikt
Next: uv run dikt add task --title "..." --tag dikt
```

What you get:

```text
weather-app/
├── AGENTS.md                 # + a marked "DIKT project memory" section (rest untouched)
├── CLAUDE.md                 # + the same section, for Claude Code
├── .claude/skills/           # dikt, dikt-triage-inbox, dikt-weekly-review
├── .github/skills/           # the same skills for GitHub Copilot
├── .github/prompts/          # dikt-triage-inbox, dikt-weekly-review prompt files
└── dikt/
    ├── dikt.toml             # marker: this is how dikt finds the collection
    ├── tags.yaml             # your tag vocabulary
    ├── AGENTS.md CLAUDE.md   # full agent rules (managed)
    ├── README.md STRUCTURE.md# reference docs (managed)
    ├── Inbox.md Scratch.md   # free-form notes
    ├── tasks/ ideas/ knowledge/ decisions/
    ├── reports/daily/ reports/weekly/
    ├── procedures/           # triage-inbox.md, weekly-review.md
    └── context/ tools/ guides/ docs/
```

Useful options:

- `--agents claude,copilot,agents-md` selects integrations. All three are
  installed by default; `--agents claude` installs only the Claude Code files.
- `--command "uv run dikt"` sets the command written into the docs and
  skills. It is auto-detected: `uv run dikt` when the binary lives in the
  project's `.venv`, otherwise `dikt`.
- `dikt init path/to/project` sets up a different directory.

Running `init` again is safe: it only creates what's missing and tells you
what differs. Commit the result:

```sh
git add dikt AGENTS.md CLAUDE.md .claude .github
git commit -m "Add DIKT project memory"
```

`dikt/.gitignore` already excludes the generated `Backlog.md` and the write lock.

---

## 4. Tags come first

Every item needs **at least one specific tag**, and every tag must be
registered in `dikt/tags.yaml`:

```yaml
tags:
  - architecture
  - tooling
  - research
  - documentation
  - testing
  - dikt
  - weather-api      # ← add your own
  - caching
  - redis
  - open-meteo
  - csv-export

broad_tags:          # context only; an item tagged ONLY with these is rejected
  - architecture
  - tooling
  - research
```

Rules:

- Names use lowercase letters and digits, with hyphens between words: `csv-export`, not `CSV_Export`.
- `broad_tags` must also be listed in `tags`. They add context but never count as the "specific" tag.
- There is no limit on tag count. Tag every central technology, format and workflow; 20 relevant tags is fine.
- Don't invent synonyms. Reuse `redis`; don't add `redis-cache` next to it.

What the errors look like:

```console
$ dikt add task --title "Bad" --tag nonexistent
Error: Unknown tags: ["nonexistent"]; register canonical tags in tags.yaml first

$ dikt add task --title "Bad" --tag architecture
Error: T-fa75f7e2: at least one specific tag outside broad_tags is required
```

---

## 5. Tasks

Create a task. `add` prints the new ID; always use the ID it returns.

```console
$ dikt add task --title "Add CSV export" --tag csv-export --priority 2 --due 2026-11-27 \
    --description "Users want to download forecasts as CSV."
T-962f5dff
```

Priority runs from 1 (highest) to 4, and the default is 3. Dates use
`YYYY-MM-DD`.

Work on it:

```console
$ dikt update T-962f5dff --status in_progress
$ dikt update T-962f5dff --add-check "Write exporter"     # one checklist item per call
$ dikt update T-962f5dff --add-check "Add tests"
$ dikt update T-962f5dff --check 1                        # 1-based; --uncheck N undoes
$ dikt log T-962f5dff "Excel needs a UTF-8 BOM to show non-ASCII city names"
```

Log real findings, blockers and progress, not every keystroke. Other updates:

```sh
dikt update T-962f5dff --title "Add CSV and Excel export"   # renames the file, never the ID
dikt update T-962f5dff --priority 1 --due 2026-11-25
dikt update T-962f5dff --set due=null                       # clear a field (JSON values)
dikt update T-962f5dff --status blocked
dikt update T-962f5dff --link items=K-e817ebbe                 # link related items by ID
dikt edit T-962f5dff --section Description --text "New description"
dikt edit T-962f5dff --section Description --file notes.md  # long text from a file
```

Finish with an outcome; a done task without one is rejected:

```console
$ dikt close T-962f5dff --outcome "Exporter shipped with BOM; 4 tests added"
T-962f5dff
```

Use `dikt update T-962f5dff --status cancelled` for work you decide not to do.
Records are never deleted.

This is what is on disk:

```console
$ dikt get T-962f5dff --full
---
schema_version: 1
id: T-962f5dff
type: task
title: Add CSV export
author: Alex Morgan
tags:
- csv-export
created: "2026-11-20T13:50:50.076643+02:00"
updated: "2026-11-20T13:50:50.092210+02:00"
links:
  jira: []
  mr: []
  commits: []
  confluence: []
  items: []
status: in_progress
priority: 2
due: "2026-11-27"
checklist:
- text: Write exporter
  done: true
- text: Add tests
  done: false
---

## Description
Users want to download forecasts as CSV.

## Outcome

## Log
- 2026-11-20T13:50:50.076916+02:00 [created] Add CSV export
- 2026-11-20T13:50:50.080163+02:00 [updated] {"fields": {"status": "in_progress"}}
- 2026-11-20T13:50:50.083084+02:00 [updated] {"add_check": "Write exporter"}
- 2026-11-20T13:50:50.086254+02:00 [updated] {"add_check": "Add tests"}
- 2026-11-20T13:50:50.089293+02:00 [updated] {"check": 1}
- 2026-11-20T13:50:50.092235+02:00 [note] Excel needs a UTF-8 BOM to show non-ASCII city names
```

The file lives at `dikt/tasks/T-962f5dff-add-csv-export.md`. You can read it,
but **don't edit item files by hand**: always go through `dikt`.

---

## 6. Ideas

An idea is a possibility you don't want to lose but haven't committed to:

```console
$ dikt add idea --title "Cache API responses" --tag caching --tag weather-api \
    --description "Forecasts change hourly; most requests repeat."
I-92990322
```

When you decide to do it, **promote** it. This creates a linked task and keeps the idea:

```console
$ dikt promote I-92990322
T-459eaace
$ dikt get I-92990322 --fields status,promoted_to
{
  "status": "promoted",
  "promoted_to": "T-459eaace"
}
```

An idea can be promoted once. Drop ideas you've rejected with
`dikt update I-92990322 --status dropped`.

---

## 7. Knowledge

Knowledge is a **verified, reusable fact** with a concrete source. A source is required:

```console
$ dikt add knowledge --title "Open-Meteo free tier allows 10k calls per day" \
    --tag open-meteo --tag weather-api \
    --source "https://open-meteo.com/en/terms (read 2026-11-20)"
K-e817ebbe
```

- After re-checking a fact, record when: `dikt update K-e817ebbe --set last_verified=2026-11-20`.
- If a fact stopped being true, use `dikt update K-e817ebbe --status outdated`.
- If a newer fact replaces it, create the new item with
  `--set 'supersedes=["K-e817ebbe"]'` (shown in the next section).

Don't store opinions, guesses or generic advice as knowledge. A preference is
not a fact; if it's a choice, it belongs in a decision.

---

## 8. Decisions

A decision records **what was chosen, from which options, why, and what it
costs**. Decisions start as `proposed`:

```console
$ dikt add decision --title "Cache forecasts in Redis for 30 minutes" \
    --tag caching --tag redis --based-on K-e817ebbe \
    --context "Forecasts change hourly; API quota is 10k calls/day" \
    --options "No cache; in-process LRU; Redis" \
    --decision "Redis with a 30-minute TTL" \
    --consequences "Deployment needs Redis" \
    --revisit-when "API quota changes or traffic exceeds 5k requests/day"
D-ea78b4cb
```

`--based-on` links supporting knowledge, and the IDs must exist. You can
revise a proposed decision freely with `dikt edit D-ea78b4cb --section Options --text ...`.

### Accepting

Accept only what the human actually approved. Acceptance requires all four
rationale sections and sets the `decided` date:

```console
$ dikt update D-ea78b4cb --status accepted
D-ea78b4cb
$ dikt get D-ea78b4cb --fields status,decided,based_on
{
  "status": "accepted",
  "decided": "2026-11-20",
  "based_on": [
    "K-e817ebbe"
  ]
}
```

From then on the rationale is **immutable**:

```console
$ dikt edit D-ea78b4cb --section Decision --text "Use memcached"
Error: Accepted rationale is immutable; create and supersede with a new decision
```

Only the revisit fields and tags can still change:

```sh
dikt update D-ea78b4cb --set revisit=2027-05-01      # review on/after this date
dikt update D-ea78b4cb --set 'revisit_when="traffic exceeds 5k/day"'
dikt update D-ea78b4cb --add-tag weather-api
```

### Review flags

An accepted decision is flagged for review when its revisit date passes, or
when its evidence becomes outdated or superseded. Here the quota changes:

```console
$ dikt add knowledge --title "Open-Meteo free tier now allows 5k calls per day" \
    --tag open-meteo --source "https://open-meteo.com/en/terms (read 2027-01-10)" \
    --set 'supersedes=["K-e817ebbe"]'
K-16c8cb1d

$ dikt list --revisit-due
D-ea78b4cb [accepted] Cache forecasts in Redis for 30 minutes

$ dikt backlog && tail -3 dikt/Backlog.md
## Decisions to Revisit

- D-ea78b4cb Cache forecasts in Redis for 30 minutes: superseded evidence K-e817ebbe (by K-16c8cb1d)
```

The flag **does not change** the decision; a human or agent has to review it.
The `revisit_when` text conditions are not evaluated automatically; check them
yourself during reviews.

### Superseding

To change your mind, record a new accepted decision and supersede the old one.
Both stay on record, linked both ways:

```console
$ dikt add decision --title "Cache forecasts in Redis for 60 minutes" --tag caching --tag redis \
    --based-on K-16c8cb1d --status accepted --context "Quota halved to 5k/day" \
    --options "Keep 30 min; 60 min" --decision "60-minute TTL" --consequences "Data up to an hour stale"
D-519ccede
$ dikt supersede D-ea78b4cb --by D-519ccede
D-519ccede
$ dikt list --type decision
D-ea78b4cb [superseded] Cache forecasts in Redis for 30 minutes
D-519ccede [accepted] Cache forecasts in Redis for 60 minutes
```

Use `rejected` for proposals that were turned down. The record of why is still valuable.

---

## 9. Finding things

```console
$ dikt list                                   # everything, oldest first, 20 per page
T-962f5dff [done] Add CSV export
I-92990322 [promoted] Cache API responses
T-459eaace [todo] Cache API responses
K-e817ebbe [active] Open-Meteo free tier allows 10k calls per day
D-ea78b4cb [superseded] Cache forecasts in Redis for 30 minutes
K-16c8cb1d [active] Open-Meteo free tier now allows 5k calls per day
D-519ccede [accepted] Cache forecasts in Redis for 60 minutes
```

| Filter | Meaning |
|---|---|
| `--query "redis ttl"` | Every word appears (case-insensitive) somewhere in the item: metadata, body or log |
| `--type decision` | task, idea, knowledge, decision |
| `--status in_progress` | Exact status |
| `--tag caching` | Has this tag |
| `--since 2026-11-01` | Updated on or after |
| `--due 2026-11-30` | Has a due date on or before |
| `--based-on K-e817ebbe` | Decisions using this evidence |
| `--related D-ea78b4cb` | Items linking to it (links, evidence, supersession, promotion) |
| `--author "Alex Morgan"` | Created by this author (exact match) |
| `--revisit-due` | Accepted decisions needing review |
| `--limit 50 --offset 20` | Paging; a "More results" hint appears on stderr |

Output shapes:

```console
$ dikt list --query redis --fields id,title,status
[
  {
    "id": "D-ea78b4cb",
    "title": "Cache forecasts in Redis for 30 minutes",
    "status": "superseded"
  },
  ...
]
$ dikt list --type task --json        # id/title/status as JSON
$ dikt list --tag caching --full      # complete files
$ dikt get D-519ccede --full             # one item, everything
$ dikt get D-519ccede --json --full      # one item as JSON, body included
```

There is no relevance ranking: results are listed oldest first (IDs are
hashes and carry no order). `list` only
searches items, so use `grep` for `context/`, `guides/`, `procedures/` and so on.

---

## 10. Backlog and reports

`dikt/Backlog.md` is regenerated on every write. You can also rebuild it with
`dikt backlog`, which is useful when dates have moved on. It groups open tasks
into *Overdue / Due Soon* (within 7 days), *In Progress*, *Blocked* and *Todo*,
then lists *Decisions to Revisit*. It is gitignored and disposable.

Reports are built from the item logs and saved as files:

```console
$ dikt report                    # today
reports/daily/2026/2026-11-20.md
$ dikt report yesterday
$ dikt report 2026-11-17
$ dikt report week               # current ISO week (also refreshes its daily reports)
reports/weekly/2026/2026-W47.md
$ dikt report week --date 2026-11-03
```

A daily report looks like this:

```markdown
# Daily Report: 2026-11-20

## Summary

<!-- dikt:generated:start -->
## Tasks Completed

- T-962f5dff Add CSV export: Exporter shipped with BOM; 4 tests added

## Other Activity

- T-962f5dff [created] Add CSV export
- T-962f5dff [updated] {"fields": {"status": "in_progress"}}
- T-962f5dff [updated] {"add_check": "Write exporter"}
- T-962f5dff [updated] {"add_check": "Add tests"}
- T-962f5dff [updated] {"check": 1}
- T-962f5dff [note] Excel needs a UTF-8 BOM to show non-ASCII city names
- I-92990322 [created] Cache API responses
- T-459eaace [created] Cache API responses
- I-92990322 [promoted] Promoted to T-459eaace
- K-e817ebbe [created] Open-Meteo free tier allows 10k calls per day
- D-ea78b4cb [created] Cache forecasts in Redis for 30 minutes
- D-ea78b4cb [updated] {"fields": {"status": "accepted"}}
- D-ea78b4cb [updated] {"fields": {"revisit": "2027-05-01"}}
- K-16c8cb1d [created] Open-Meteo free tier now allows 5k calls per day
- D-519ccede [created] Cache forecasts in Redis for 60 minutes
- D-ea78b4cb [superseded] Replaced by D-519ccede
- D-519ccede [supersedes] Replaces D-ea78b4cb
<!-- dikt:generated:end -->
```

Activity is listed in the order it happened.

Write your own summary **under `## Summary`, outside the markers**.
Regenerating replaces only the marked block, and weekly reports include your
daily summaries. If a report file has lost its markers, dikt refuses to
overwrite it rather than destroy handwritten text.

---

## 11. Inbox, Scratch and procedures

- **`dikt/Inbox.md`** is for quick unstructured capture: "look into rate
  limits", a pasted link, a half-thought. Triage it later into items (by hand,
  or with the `dikt-triage-inbox` skill).
- **`dikt/Scratch.md`** holds temporary notes for the current piece of work.
- **`dikt/procedures/`** holds repeatable workflows that worked. Each starts with:

  ```yaml
  ---
  verification_status: untested   # untested | verified | stale
  last_verified: null             # date of last successful run
  evidence: []                    # e.g. [T-00012]
  ---
  ```

  Mark a procedure `verified` only after actually running it and checking the result.
- **`context/`, `tools/`, `guides/`, `docs/`** are free-form: environment
  facts, tool setup, learning notes, and copies of external material with their source.

Nothing clears Inbox or Scratch automatically. Agents are told to remove
entries only after you approve.

---

## 12. Working with AI agents

`dikt init` installs three layers of instructions:

1. A **short section in the project's `AGENTS.md` / `CLAUDE.md`**, loaded at
   the start of each chat. It says: search DIKT before reasoning, write only
   through the CLI, and read `dikt/AGENTS.md`.
2. **Full rules in `dikt/AGENTS.md`**: what's worth saving, how to tag,
   decision etiquette, procedures and commits. `dikt/CLAUDE.md` imports it
   for Claude Code.
3. **Skills**: `dikt`, which the agent loads automatically for item work, plus
   `dikt-triage-inbox` and `dikt-weekly-review`, which you invoke yourself.

| Harness | Reads | Invoke workflows |
|---|---|---|
| Claude Code | `CLAUDE.md`, `.claude/skills/` | `/dikt-triage-inbox`, `/dikt-weekly-review` |
| GitHub Copilot (VS Code) | `AGENTS.md`, `.github/skills/`, `.github/prompts/` | `/dikt-triage-inbox`, `/dikt-weekly-review` prompts |
| Codex & other AGENTS.md readers | `AGENTS.md` | Ask: "follow dikt/procedures/weekly-review.md" |

A complete, recorded example of two such chats (saving records in one, using
them in the next) is in the [README](../README.md#example-two-agent-chats).

### Check that discovery works

In a **fresh** chat, ask:

> What does this project use for durable memory, and how do you add a task to it?

A correct answer mentions `dikt/` and `uv run dikt add task ... --tag ...`.
If it doesn't, the harness didn't load the instructions. Attach
`dikt/AGENTS.md` to the chat or check the harness's instruction settings.

### Example prompts

- "Before designing the cache, check DIKT for related decisions and knowledge."
- "Start T-459eaace and log what you find."
- "We verified the API limit: save it as knowledge with the docs URL as source."
- "Propose a decision for the export format; don't accept it."
- "I approve D-519ccede, accept it."
- "Triage the inbox." (or `/dikt-triage-inbox`)
- "Do the weekly review." (or `/dikt-weekly-review`)

### What good agent behavior looks like

- It searches first (`dikt list --query ...`) and cites IDs it relied on.
- It saves only verified, reusable facts and real choices, and says nothing
  was worth saving when that's the case.
- It never marks a decision `accepted` without your explicit approval.
- It reports saved IDs briefly instead of pasting records.
- If `dikt` fails, it reports the error instead of editing files by hand.

---

## 13. Running from anywhere

`dikt` looks for the collection starting from the current directory and walking up:

- a `dikt.toml` in the directory (you're inside the collection), or
- `dikt/dikt.toml` (you're in the project or one of its subfolders).

So `cd src/api && dikt list` just works. From elsewhere:

```sh
dikt --root ~/code/weather-app/dikt list            # --root goes BEFORE the subcommand
DIKT_ROOT=~/code/weather-app/dikt dikt validate
uv run --project ~/code/weather-app dikt --root ~/code/weather-app/dikt list
```

Several projects each have their own `dikt/` collection. In multi-root editor
workspaces, run commands from the right project or pass `--root`, so records
don't land in the wrong project.

---

## 14. Git and teams

- `dikt/` is ordinary project content: commit items, reports, tags and
  procedures with the work they describe.
- **Item IDs practically never collide between branches.** Each ID is a hash
  (32 bits) that includes the creation time and author, so two people adding
  "Update the README" on separate clones get different IDs and different
  files. Here is that exact situation, merged with Git:

  ```console
  $ git pull --no-rebase              # Grace pulls after Ada pushed her task
  Merge made by the 'ort' strategy.
   dikt/tasks/T-9c7055c3-update-the-readme.md | 29 +++++++++++++++++++++++++++++
   1 file changed, 29 insertions(+)
  $ dikt validate
  Valid: 2 items
  ```

  With dikt 0.1.0's sequential IDs, both clones had created `T-00001`: Git
  reported `CONFLICT (add/add)` and `dikt validate` failed on the merged file.
  Items created by 0.1.0 keep their IDs and remain valid next to the new ones.
  If two items ever did get the same ID, `dikt validate` reports
  `Duplicate ID` and neither copy is used until you re-create one.
- `dikt list --author "Name"` shows who created what. The author comes from
  `git config user.name`, so set it in each clone. Set `DIKT_AUTHOR` to
  override it (for example in CI), or pass `--author` to `add`.
- **Still conflicts, as with any shared file:** two people editing *the same
  item*, both adding a tag at the same spot in `tags.yaml`, or both generating
  the same day's report. Resolve these in Git as usual, and regenerate reports
  after merging (`dikt report YYYY-MM-DD`).
- The write lock (`dikt/.dikt-write.lock/`) only serializes processes on one machine.
- Never delete items; use `cancelled`, `dropped`, `outdated`, `rejected` or `superseded`.

---

## 15. Recovery and troubleshooting

All normal reads and writes are **strict**, so a single broken file blocks
them. That's deliberate:

```console
$ dikt list
Error: I-92990322-x.md: expected YAML frontmatter
```

To look around anyway, use a **recovery read**. It skips invalid items (and
items that link to them), warns on stderr, and never modifies anything:

```console
$ dikt list --skip-invalid
Warning: recovery read enabled; results may be incomplete. No files are repaired. Run dikt validate for a strict check.
Warning: Skipped ideas/I-92990322-x.md: I-92990322-x.md: expected YAML frontmatter
T-962f5dff [done] Add CSV export
I-92990322 [promoted] Cache API responses
T-459eaace [todo] Cache API responses
...
```

`dikt get ID --skip-invalid` works the same way. Fix the file (usually with
`git diff` / `git checkout -- <file>`), then run `dikt validate`.

| Message | Cause and fix |
|---|---|
| `No DIKT collection found ...` | You're outside the project. `cd` into it, or pass `--root` / `DIKT_ROOT`. Run `dikt init` if it was never set up. |
| `Unknown tags: [...]` | Register the tag in `dikt/tags.yaml` first. |
| `at least one specific tag outside broad_tags is required` | Add a tag that isn't in `broad_tags`. |
| `Another dikt write is active ...` | Another dikt is writing. If one crashed, check that none is running, then delete `dikt/.dikt-write.lock/`. |
| `Done tasks need an Outcome` | Use `dikt close ID --outcome "..."`. |
| `Accepted decisions need decided date and Context, Options, Decision, Consequences` | Fill those sections (`dikt edit`) before accepting. |
| `Accepted rationale is immutable` | Create a new decision and `supersede` the old one. |
| `missing or self reference X` | An ID in links/evidence doesn't exist. Check for typos. |
| `Refusing to overwrite report without unique ordered generation markers` | Restore the `<!-- dikt:generated:start/end -->` markers in that report. |
| `the argument '--add-check' cannot be used multiple times` | One checklist item per `update` call. |
| `uv add dikt` installed something odd | That's the unrelated PyPI package. Remove it and install `dikt-cli` as in [Install](#install). |

---

## 16. Upgrading and uninstalling

Upgrade:

```sh
uv add --dev dikt-cli --upgrade # project install (A); global: uv tool upgrade dikt-cli
uv run dikt validate
uv run dikt init --refresh      # update managed docs and skills
git diff                        # review what changed
```

**From 0.1.0:** existing items keep their sequential IDs (`T-00001`) and
need no migration; they simply have no `author`. New items get hash IDs and an
author. Run `init --refresh` to update the agent instructions as well.

`init --refresh` rewrites only dikt-owned files (`dikt/README.md`,
`STRUCTURE.md`, `AGENTS.md`, `CLAUDE.md`, `.gitignore`, the skills and
prompts) and the marked sections in your root `AGENTS.md` / `CLAUDE.md`. It
**never** touches items, reports, `tags.yaml`, procedures, Inbox or Scratch.
If you customized a managed file, re-apply your changes after refreshing, or
skip the refresh.

Uninstall: `uv remove --dev dikt-cli` (or `uv tool uninstall dikt-cli`). Your
`dikt/` data stays; it's just Markdown. To remove the agent integration,
delete `.claude/skills/dikt*`, `.github/skills/dikt*`,
`.github/prompts/dikt-*` and the marked sections.

---

## 17. Reference

### Commands

| Command | Purpose |
|---|---|
| `dikt init [PATH] [--agents ...] [--refresh] [--command CMD]` | Set up or refresh a project |
| `dikt add TYPE --title T --tag TAG [...]` | Create an item; prints its ID |
| `dikt get ID [--full\|--fields a,b] [--json] [--skip-invalid]` | Read one item |
| `dikt list [filters] [--full\|--fields a,b] [--json] [--limit N --offset N]` | Search and filter |
| `dikt update ID [--title --status --priority --due --add-tag --rm-tag --link K=V --add-check --check N --uncheck N --set F=V]` | Change metadata |
| `dikt edit ID --section NAME (--text T \| --file F)` | Replace a body section |
| `dikt log ID TEXT` | Append a log note |
| `dikt close ID --outcome TEXT` | Complete a task |
| `dikt promote I-ID` | Idea → linked task |
| `dikt supersede D-OLD --by D-NEW` | Replace an accepted decision |
| `dikt report [today\|yesterday\|YYYY-MM-DD\|week] [--date D]` | Save a report |
| `dikt backlog` | Rebuild `Backlog.md` |
| `dikt validate` | Strict check of the whole collection |

`dikt <command> --help` shows every option. The global `--root PATH` (or
`DIKT_ROOT`) must come before the command. Errors exit with code 2 and an
`Error: ...` line on stderr.

### `add` options by type

| Option | Applies to |
|---|---|
| `--description TEXT` | all |
| `--author NAME` | all (default: `$DIKT_AUTHOR`, else `git config user.name`, else the OS user) |
| `--tag TAG` (repeatable) | all |
| `--status S` | all (decisions default to `proposed`) |
| `--priority 1-4`, `--due YYYY-MM-DD` | task |
| `--source TEXT` | knowledge (required) |
| `--based-on K-ID` (repeatable) | decision |
| `--context --options --decision --consequences` | decision |
| `--revisit YYYY-MM-DD`, `--revisit-when TEXT` | decision |
| `--set FIELD=VALUE` | any other metadata field (JSON values: `null`, `3`, `["K-e817ebbe"]`) |

### Item fields

| Field | Types | Notes |
|---|---|---|
| `schema_version`, `id`, `type`, `created`, `updated` | all | Managed by dikt, immutable |
| `title`, `status`, `tags` | all | Title is one line |
| `author` | all | Set on creation, immutable. Optional when reading, so items from dikt 0.1.0 (which had none) stay valid |
| `links` | all | `items` holds DIKT IDs; `jira`, `mr`, `commits`, `confluence` hold free-form references (no integrations) |
| `priority`, `due`, `checklist` | task | |
| `promoted_to` | idea | Set by `promote` |
| `source`, `last_verified`, `supersedes` | knowledge | `supersedes` lists K-IDs |
| `decided`, `revisit`, `revisit_when`, `based_on`, `supersedes`, `superseded_by` | decision | Lifecycle links set by `supersede` |

Body sections: every type has `Description` and `Log`. Tasks add `Outcome`;
decisions add `Context`, `Options`, `Decision`, `Consequences`. `Log` is
append-only, through `log` and automatic entries.

### Limits

- Search is substring matching, listed oldest first; there is no ranking or index.
- Multi-file writes roll back on ordinary errors but are not crash-atomic.
- There is no delete command and no schema migration command.
- Tag relevance is your (or your agent's) judgment; validation checks only
  that tags are registered and specific.
