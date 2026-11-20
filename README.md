# dikt

**D**ecisions, **i**deas, **k**nowledge and **t**asks as validated local Markdown,
built for people and AI agents working inside a project.

`dikt` is a single Rust binary. It stores items as Markdown files with YAML
frontmatter, validates every write (schemas, tags, references, lifecycle
rules), and generates backlogs and daily/weekly activity reports. Agents get
skills and instructions that tell them to retrieve before reasoning and to
preserve only verified, reusable results.

Highlights:

- **Safe writes.** Every change is validated, and invalid writes touch nothing.
- **Team-friendly IDs.** Items get hash IDs (`T-3f9a1c2e`) and record their author, so branches that each add items [merge without conflicts](#working-in-a-team).
- **Agent-ready.** `dikt init` installs skills and instructions for Claude Code, GitHub Copilot and `AGENTS.md` readers. See the [recorded example](#example-two-agent-chats).

**→ Full guide: [docs/TUTORIAL.md](docs/TUTORIAL.md)**

## Install into a project (uv)

```sh
uv add --dev dikt-cli
uv run dikt init                          # create dikt/ + agent skills and instructions
uv run dikt add task --title "Try DIKT" --tag dikt
```

[`dikt-cli`](https://pypi.org/project/dikt-cli/) ships prebuilt binaries for
Linux, macOS and Windows, so no Rust toolchain is needed.

> Install **`dikt-cli`**; the installed command is `dikt`. The PyPI package
> named `dikt` is an unrelated project, so don't `uv add dikt`.

Other ways to install (details in the [tutorial](docs/TUTORIAL.md#install)):

- As a global tool: `uv tool install dikt-cli`
- Without installing: `uvx --from dikt-cli dikt --help`
- As a standalone binary: download `dikt-<version>-<target>.tar.gz`/`.zip` from the [releases page](https://github.com/dark-neonus/dikt/releases)
- From source (needs Rust ≥ 1.88): `uv add --dev "dikt-cli @ git+https://github.com/dark-neonus/dikt"` or `cargo install --git https://github.com/dark-neonus/dikt`

`dikt init` writes `uv run dikt` into the generated docs when the binary lives
in the project's `.venv`, and plain `dikt` otherwise. Override with `--command`.

### What `dikt init` creates

| Path | Owner | Purpose |
|---|---|---|
| `dikt/dikt.toml` | project | Collection marker used for discovery |
| `dikt/tags.yaml`, `Inbox.md`, `Scratch.md`, `procedures/*` | project | Starter data, never overwritten |
| `dikt/tasks/`, `ideas/`, `knowledge/`, `decisions/`, `reports/`, `context/`, `tools/`, `guides/`, `docs/` | project | Item storage and free-form resources |
| `dikt/README.md`, `STRUCTURE.md`, `AGENTS.md`, `CLAUDE.md`, `.gitignore` | dikt | Managed docs and full agent rules |
| `.claude/skills/dikt*/SKILL.md` | dikt | Claude Code skills (`--agents claude`) |
| `.github/skills/dikt*/SKILL.md`, `.github/prompts/dikt-*.prompt.md` | dikt | GitHub Copilot skills and prompts (`--agents copilot`) |
| Marked section in `AGENTS.md` / `CLAUDE.md` | dikt | Short pointer to the rules (`--agents agents-md`, `claude`) |

Running `init` again is safe: it creates anything that is missing and reports
what differs. `dikt init --refresh` (for example after upgrading dikt) rewrites
the managed files and the marked `<!-- dikt:start -->…<!-- dikt:end -->`
sections. It never touches items, reports, `tags.yaml`, procedures, Inbox or
Scratch. Text outside the marked sections of your `AGENTS.md`/`CLAUDE.md` is
always preserved.

## Usage

```sh
dikt add task --title "Fix export" --tag csv-export --priority 2 --due 2026-12-01
dikt list --query "export" --type task
dikt update T-3f9a1c2e --status in_progress --add-check "Reproduce"
dikt log T-3f9a1c2e "Reproduced with an empty file"
dikt close T-3f9a1c2e --outcome "Fixed; regression test added"
dikt add knowledge --title "..." --source "Benchmark run 2026-11-20" --tag ...
dikt add decision --title "..." --tag ... --based-on K-7d21b0a4 \
  --context "..." --options "..." --decision "..." --consequences "..."
dikt report week
dikt validate
```

`add` prints the new item's ID (here `T-3f9a1c2e` is only an example); use the ID it
returns. `dikt list` shows the oldest items first.

The collection is found by walking up from the current directory and looking
for `dikt.toml` or `dikt/dikt.toml`. You can also pass `--root PATH` (before
the subcommand) or set `DIKT_ROOT`. The installed `dikt/README.md` has the full
command contract, lifecycle and tagging policy. `dikt <command> --help` lists
every option.

## Example: two agent chats

This is a real, unedited recording of two separate Claude Code sessions
(Claude Sonnet 5.5, started with `claude -p`) in a fresh project where dikt 0.2.0
was installed (a local build) and `uv run dikt init` had been run. Nothing else was set up: the
agent found the `dikt` skill and the project instructions by itself. The
sessions could run `dikt`, read files and edit them, and use a few read-only
shell commands. The subject is a cat.

### Chat 1: saving things

Prompt:

```text
I am owner of cat.

I have next tasks: feed him this evening, go buy him new toy tommorow

I have an idea of finding him a friend

After a few playtimes with him I get to know that my cat like pink color, this knowledge can be used when buying him stuff

Also I decided to move his bowl with food to kitchen because in living room it made mess
```

Response:

> I searched the project records first and found nothing on your cat, so I saved everything as new records. `uv run dikt validate` passes.
> 
> | ID | Type | Content |
> |---|---|---|
> | T-431db8c8 | Task | Feed the cat this evening (due 2026-11-20) |
> | T-e7e9a721 | Task | Buy the cat a new toy, preferably pink (due 2026-11-21) |
> | I-fded8849 | Idea | Find the cat a friend |
> | K-b9329026 | Knowledge | The cat likes pink, from your playtime observations. I marked it informal, not rigorously verified. |
> | D-524ab929 | Decision | Move the food bowl to the kitchen, because the living room got messy |
> 
> I saved the bowl decision as accepted because you said you decided it. I also registered six new tags in `dikt/tags.yaml`: `cat`, `cat-feeding`, `cat-toys`, `cat-food-bowl`, `cat-companion` and `pink`.
> 
> Nothing is committed. If you want a commit, I can make one.

What ended up on disk (`uv run dikt list`; oldest first):

```text
T-431db8c8 [todo] Feed the cat this evening
T-e7e9a721 [todo] Buy the cat a new toy
I-fded8849 [open] Find the cat a friend
K-b9329026 [active] Cat likes the color pink
D-524ab929 [accepted] Move the cat's food bowl to the kitchen
```

The decision, as the agent stored it (`uv run dikt get D-524ab929 --full`):

```markdown
---
schema_version: 1
id: D-524ab929
type: decision
title: Move the cat's food bowl to the kitchen
author: Alex Morgan
tags:
- cat
- cat-food-bowl
- cat-feeding
created: "2026-11-20T13:50:03.486248+02:00"
updated: "2026-11-20T13:50:03.486248+02:00"
links:
  jira: []
  mr: []
  commits: []
  confluence: []
  items: []
status: accepted
decided: "2026-11-20"
revisit: null
revisit_when: The kitchen location causes problems for the cat or the household
based_on: []
supersedes: []
superseded_by: null
---

## Description


## Context
The cat's food bowl was in the living room, where eating made a mess.

## Options
Keep the bowl in the living room; move it to the kitchen.

## Decision
Move the cat's food bowl to the kitchen.

## Consequences
Less mess in the living room. The cat must be fed in the kitchen from now on.

## Log
- 2026-11-20T13:50:03.486295+02:00 [created] Move the cat's food bowl to the kitchen
```

### Chat 2: using what was saved

A new chat, with no memory of chat 1. Prompt:

```text
Hello, I want buy my cat a toy, what would you suggest? Why?

Also I have found my cats bowl in kitchen, butI remember that it was in my living room, do you have any idea why is it there?
```

Response:

> **The bowl:** You moved it yourself. Your project records have an accepted decision, D-524ab929, dated today (2026-11-20). It says to move the cat's food bowl from the living room to the kitchen, because eating in the living room made a mess. So the kitchen is now the bowl's permanent spot. Feed the cat there from now on. The decision says to revisit it if the kitchen causes problems for the cat or the household.
> 
> **The toy:** Your records already have a task for this, T-e7e9a721, due tomorrow (2026-11-21). It says to prefer pink, because you noticed the cat seems to like that color (K-b9329026). That's an informal observation, not a verified one.
> 
> I'd suggest a **pink feather wand or a pink crinkle ball**, for these reasons:
> - **Feather wand:** It gets you playing with the cat. That gives the cat exercise and bonding time, and you're already watching what it responds to during playtimes. It also satisfies the pink preference.
> - **Crinkle ball or small plush mouse:** It's cheap and light, and the cat can bat it around alone. That helps on days you aren't around.
> - **Pink as a tiebreaker only:** Cats see color much less vividly than people do, so pink may not be what draws the cat in. Motion, sound and texture matter more. I'd choose by how the toy moves, then pick pink within that.
> 
> You could also buy two cheap toys and see which one the cat prefers. The result would be a useful note for the records.
> 
> I haven't changed anything in the records. Once you've bought the toy, I can mark the task done and log which toy and how the cat reacted.

The agent searched DIKT before answering, found the bowl decision and the toy
task, and cited them by ID. It also treated the pink preference as an informal
observation rather than a fact, because that is how it was recorded. It wrote
nothing: every item's `created` and `updated` timestamps are still identical.

**How reliable is the search?** The agent decides whether to look. With the
first wording of the instructions, 2 of 6 repeats of chat 2 searched DIKT and
the rest answered generically. After the instructions were changed to say that
everyday questions count too ("Search before you answer"), 6 of 6 repeats
searched. The final recording above was made with that wording. Check your own
setup with a fresh chat (see the [tutorial](docs/TUTORIAL.md#12-working-with-ai-agents)).

You can repeat this yourself (needs dikt 0.2.0 or newer):

```sh
mkdir cat-owner && cd cat-owner && git init && uv init --python 3.14
uv add --dev dikt-cli && uv run dikt init
claude          # then paste the chat 1 prompt, start a new chat, paste the chat 2 prompt
```

## Working in a team

Item IDs are short hashes of the type, title, first ten tags, creation time
and author. Two people who each add an item on their own branch therefore get
different IDs and different files, and Git merges the branches cleanly. Here
both Ada and Grace added a task titled "Update the README" on separate clones,
then Grace pulled. First with dikt 0.1.0 (sequential IDs):

```console
$ git pull --no-rebase
Auto-merging dikt/tasks/T-00001-update-the-readme.md
CONFLICT (add/add): Merge conflict in dikt/tasks/T-00001-update-the-readme.md
Automatic merge failed; fix conflicts and then commit the result.
$ dikt validate
Error: Invalid YAML: could not find expected ':' at line 8 column 1, while scanning a simple key at line 7 column 1
```

And with the current version:

```console
$ git pull --no-rebase
Merge made by the 'ort' strategy.
 dikt/tasks/T-9c7055c3-update-the-readme.md | 29 +++++++++++++++++++++++++++++
 1 file changed, 29 insertions(+)
$ dikt validate
Valid: 2 items
$ dikt list
T-9c7055c3 [todo] Update the README
T-c7a4b990 [todo] Update the README
```

Every new item records its `author` (`--author`, else `$DIKT_AUTHOR`, else
`git config user.name`), and `dikt list --author "Ada"` filters by it. IDs from
0.1.0 stay valid. Two people editing the *same* item, adding tags at the same
spot in `tags.yaml`, or generating the same day's report can still conflict in
Git; see [Git and teams](docs/TUTORIAL.md#14-git-and-teams).

## Development

```sh
cargo test                 # unit and integration tests
cargo clippy --all-targets
uv build                   # binary wheel + sdist via maturin
```

Releasing: bump `version` in `Cargo.toml`, commit, then tag and push
(`git tag v0.2.0 && git push origin v0.2.0`). The `Release` workflow runs the
tests, builds wheels (Linux glibc x86_64/aarch64, Linux musl x86_64, macOS
x86_64/arm64, Windows x86_64), an sdist and standalone binaries, and publishes them with
`SHA256SUMS` as a GitHub release, then uploads the wheels and sdist to PyPI
as `dikt-cli`.

PyPI upload uses [trusted publishing](https://docs.pypi.org/trusted-publishers/)
(no API token). One-time setup on pypi.org → *Your account → Publishing →
Add a new pending publisher*: PyPI project `dikt-cli`, owner `dark-neonus`,
repository `dikt`, workflow `release.yml`, environment `pypi`. To upload an
already-built release, run *Actions → Release → Run workflow* with its tag.

Layout:

```text
src/store.rs     parsing, validation (hand-written schema), IDs, lifecycle, transactional writes
src/views.rs     backlog, decision review flags, daily/weekly reports
src/cli.rs       clap adapter, projection and filtering
src/init.rs      project installer (templates embedded at compile time)
src/yaml.rs      strict YAML loading and deterministic emitting
templates/       files installed by `dikt init` ({{DIKT}}, {{ROOT}}, {{UP}} placeholders)
tests/           store, cli, views, collaboration and init tests
```

Keep the core rules in `store`/`views` independent of the CLI adapter. Update
templates, tests and this README together.

## Limits

- Search is substring matching, listed oldest first. There is no ranking or index.
- Multi-file writes roll back on ordinary failures but are not crash-atomic.
- The write lock serializes writers on one machine only, not separate clones.
- There is no delete command and no schema migration command.
- Tags must be registered by editing `tags.yaml`; there is no tag command yet.
- Whether an agent searches DIKT or follows the saving rules is up to the agent. The instructions make it likely, and a fresh-chat check confirms it for your setup.
