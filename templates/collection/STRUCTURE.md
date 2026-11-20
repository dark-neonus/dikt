# DIKT Structure

Markdown item files are authoritative data. AGENTS.md owns agent rules and
README.md owns operations. Files marked *managed* are rewritten by
`dikt init --refresh`; everything else belongs to this project.

```text
{{ROOT}}/
  dikt.toml                       # collection marker (do not delete)
  tags.yaml                       # canonical tag names and broad_tags
  README.md  STRUCTURE.md         # managed docs
  AGENTS.md  CLAUDE.md            # managed agent rules (CLAUDE.md imports AGENTS.md)
  .gitignore                      # managed: Backlog.md, write lock
  Inbox.md                        # drafts awaiting triage
  Scratch.md                      # temporary current-task notes
  tasks/                          # T-... Markdown items
  ideas/                          # I-... Markdown items
  knowledge/                      # K-... Markdown items
  decisions/                      # D-... Markdown items
  reports/daily/                  # YYYY/YYYY-MM-DD.md
  reports/weekly/                 # ISO-year/YYYY-Www.md
  procedures/                     # verified repeatable workflows
  context/                        # environment facts
  tools/                          # shared tool setup
  guides/                         # learning and recall resources
  docs/                           # preserved external references with provenance
```

Agent integrations live outside this folder: `.claude/skills/dikt*`,
`.github/skills/dikt*`, `.github/prompts/dikt-*` and a marked DIKT section in
the project's `AGENTS.md` / `CLAUDE.md`. Generated Backlog.md is ignored; do
not treat generated views as independent facts.

## Placement

1. Action to complete: task.
2. Worthwhile possibility: idea.
3. Choice/rationale to recall: decision; never confuse choice with evidence.
4. Reusable learned fact with source/applicability: knowledge.
5. Environment facts: context. Tool setup: tools. Repeated steps: procedures.
6. Learning and recall: guides. Preserved external material: docs.

All four item types have schema_version, id, type, title, author (optional,
set on creation), status, tags, created, updated, links and Description/Log
sections. Tasks add priority,
due, checklist and an Outcome section. Ideas add promoted_to. Knowledge adds
source, last_verified and supersedes. Decisions add decided, revisit,
revisit_when, based_on, supersedes, superseded_by and Context, Options,
Decision, Consequences sections.
Links use immutable IDs, not filenames. Retain historical records in place.
Item metadata is edited only via dikt; free-form documents use normal edits.
Every item needs a specific tag; there is no tag-count ceiling.

Procedures start with YAML verification_status (untested/verified/stale),
last_verified (date/null) and evidence (references). New steps are untested
until executed and output checked. A stale procedure retains its last evidence.
Skills/prompts point to procedures rather than duplicating their steps.
Validation checks item contracts, not every free-form resource or source's freshness.
