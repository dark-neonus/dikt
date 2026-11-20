# dikt

**D**ecisions, **i**deas, **k**nowledge and **t**asks as validated local Markdown,
built for people and AI agents working inside a project.

`dikt` is a single Rust binary. It stores items as Markdown files with YAML
frontmatter, validates every write (schemas, tags, references, lifecycle
rules), and generates backlogs and daily/weekly activity reports. Agents get
skills and instructions that tell them to retrieve before reasoning and to
preserve only verified, reusable results.

It is a Rust port of the DIKT blueprint's Python implementation. The behaviour
and the blueprint's 47 regression scenarios are preserved (see
[Differences from the blueprint](#differences-from-the-blueprint)).

**→ Full guide: [docs/TUTORIAL.md](docs/TUTORIAL.md)**

## Install into a project (uv)

Prebuilt wheels for Linux, macOS and Windows are attached to every
[GitHub release](https://github.com/dark-neonus/dikt/releases), so no Rust
toolchain is needed. Add the release as a pinned package source in your
project's `pyproject.toml`:

```toml
[[tool.uv.index]]
name = "dikt"
url = "https://github.com/dark-neonus/dikt/releases/expanded_assets/v0.1.0"
format = "flat"
explicit = true

[tool.uv.sources]
dikt-cli = { index = "dikt" }
```

```sh
uv add --dev dikt-cli
uv run dikt init                          # create dikt/ + agent skills and instructions
uv run dikt add task --title "Try DIKT" --tag dikt
```

> The Python distribution is **`dikt-cli`**. The PyPI package named `dikt` is
> an unrelated project, so don't `uv add dikt`. The installed command is `dikt`.

Other ways to install (details in the [tutorial](docs/TUTORIAL.md#install)):

- From source (needs Rust ≥ 1.88): `uv add --dev "dikt-cli @ git+https://github.com/dark-neonus/dikt"`
- As a global tool: `uv tool install dikt-cli --find-links https://github.com/dark-neonus/dikt/releases/expanded_assets/v0.1.0`
- As a standalone binary: download `dikt-<version>-<target>.tar.gz`/`.zip` from the releases page
- With Cargo: `cargo install --git https://github.com/dark-neonus/dikt`

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
dikt update T-00001 --status in_progress --add-check "Reproduce"
dikt log T-00001 "Reproduced with an empty file"
dikt close T-00001 --outcome "Fixed; regression test added"
dikt add knowledge --title "..." --source "Benchmark run 2026-11-20" --tag ...
dikt add decision --title "..." --tag ... --based-on K-00001 \
  --context "..." --options "..." --decision "..." --consequences "..."
dikt report week
dikt validate
```

The collection is found by walking up from the current directory and looking
for `dikt.toml` or `dikt/dikt.toml`. You can also pass `--root PATH` (before
the subcommand) or set `DIKT_ROOT`. The installed `dikt/README.md` has the full
command contract, lifecycle and tagging policy. `dikt <command> --help` lists
every option.

## Development

```sh
cargo test                 # unit + ported blueprint regression tests
cargo clippy --all-targets
uv build                   # binary wheel + sdist via maturin
```

Releasing: bump `version` in `Cargo.toml`, commit, then tag and push
(`git tag v0.2.0 && git push origin v0.2.0`). The `Release` workflow runs the
tests, builds wheels (Linux glibc x86_64/aarch64, Linux musl x86_64, macOS
x86_64/arm64, Windows x86_64), an sdist and standalone binaries, and publishes them with
`SHA256SUMS` as a GitHub release. Then update the `v0.1.0` URLs in this
README and the tutorial.

Layout:

```text
src/store.rs     parsing, validation (hand-written schema), IDs, lifecycle, transactional writes
src/views.rs     backlog, decision review flags, daily/weekly reports
src/cli.rs       clap adapter, projection and filtering
src/init.rs      project installer (templates embedded at compile time)
src/yaml.rs      strict YAML loading and deterministic emitting
templates/       files installed by `dikt init` ({{DIKT}}, {{ROOT}}, {{UP}} placeholders)
tests/           ported blueprint tests (store, cli, views) + init tests
```

Keep the core rules in `store`/`views` independent of the CLI adapter. Update
templates, tests and this README together.

## Differences from the blueprint

- **Implementation:** Rust instead of Python. The JSON Schemas are hand-coded in `store::schema`, and `date-time` fields are actually checked as RFC 3339 (the Python build had no date-time format checker installed).
- **Packaging:** it installs into any project as a tool (`uv add`, `uv tool install`, `cargo install`) instead of being its own repository. Discovery uses `dikt.toml` instead of `[tool.pm]` in `pyproject.toml`.
- **New command:** `dikt init` installs the collection and the agent integrations: Claude Code, GitHub Copilot and AGENTS.md.
- **Renames:** the lock file is `.dikt-write.lock`, and report markers are `<!-- dikt:generated:start/end -->`. Skills are prefixed `dikt-` so they don't collide with the host project's own skills.
- **Single-line titles:** titles containing any newline are rejected. Python's `$` accepted a single trailing newline.
- **Starter tags:** `tags.yaml` starts with a small generic vocabulary instead of DIKT-specific tags.

The known boundaries are unchanged. Search is ordered by ID with no ranking. Multi-file writes roll back on ordinary failures but are not crash-atomic. The lock serializes local writers only. There is no delete or migration command.
