---
name: dikt
description: "Use when searching, creating or updating the project's tasks, ideas, knowledge or decisions; capturing useful findings; analyzing prior decisions; triaging the DIKT inbox; or generating daily and weekly reports. Operates through the validated dikt CLI."
---

# DIKT Management

Read [{{ROOT}}/AGENTS.md]({{UP}}{{ROOT}}/AGENTS.md) for retrieval and preservation rules,
[{{ROOT}}/README.md]({{UP}}{{ROOT}}/README.md) for the command contract and tagging policy, and
[{{ROOT}}/STRUCTURE.md]({{UP}}{{ROOT}}/STRUCTURE.md) for placement and fields.

## Operate

1. Run `{{DIKT}}` from the project root or a subdirectory (the collection is found via
   `{{ROOT}}/dikt.toml`). Elsewhere use `{{DIKT}} --root <abs path to {{ROOT}}> ...`.
2. Search `{{DIKT}} list --query "topic"` with relevant `--type`, `--tag`, `--based-on`
   or `--related` filters. Page with `--offset` if more results exist. Read only
   relevant IDs with `{{DIKT}} get <ID> --full`. `--json` is available for retrieval.
   If invalid records block retrieval, use `get` or `list --skip-invalid` only
   as an explicit recovery read. Inspect stderr warnings; results are incomplete
   and excluded records are not repaired. Never use recovery output as proof
   that a record does not exist. Writes and validation remain strict.
3. Decide whether the result belongs in an existing record, a new task, idea,
   knowledge item or decision. A source supports knowledge; rationale and
   alternatives explain a decision. A preference is not an empirically verified fact.
4. Run `add`, `update`, `edit`, `log`, `close`, `promote` or `supersede`.
   Use `--help` on the specific command instead of guessing flags. `add`
   prints the new ID. Use `edit --section Name --file PATH` for long text.
5. Link real evidence with `--based-on` on decisions or `--link items=ID` on
   other records. Do not invent IDs. Follow the tagging policy: register
   clear new canonical tags in `{{ROOT}}/tags.yaml` when needed and include every
   central technology or workflow. Broad-only tagging is rejected; there is no
   tag-count ceiling.
6. Read back the changed item and check it matches the intended result. Run
   `{{DIKT}} validate` after batches. Report saved IDs, not copied full records.

## Lifecycle

- Tasks: todo, in_progress, blocked, done, cancelled. `close --outcome` records
  completion. `update --add-check` adds a checklist entry; `--check N` and
  `--uncheck N` use 1-based positions.
- Ideas: open, promoted, dropped. `promote` creates a linked task without
  deleting the idea; run it once per idea.
- Knowledge: active, outdated. Supply a concrete `--source`; set verification
  date with `update --set last_verified=YYYY-MM-DD` after verification.
- Decisions: proposed, accepted, rejected, superseded. Fill `--context`, `--options`,
  `--decision` and `--consequences` before acceptance. Supply `--revisit` or
  `--revisit-when` where useful. Accepted rationale is immutable; `supersede
  D-old --by D-new` requires two accepted decisions.
- `list --revisit-due` finds accepted decisions with due dates or outdated/superseded
  evidence. Evaluate free-text revisit conditions yourself against actual facts.
- `report [today|yesterday|YYYY-MM-DD|week]` always saves. Weekly selection
  supports `--date YYYY-MM-DD`. Edit only handwritten sections outside generation
  markers; preserve existing summaries and do not inflate activity into outcomes.

If the CLI fails, report the error. Do not hand-edit item files as a fallback.
Only preserve verified reusable findings and meaningful choices; no record is
required when the session produced neither.
