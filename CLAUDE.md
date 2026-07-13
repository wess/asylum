<!-- Keep in sync with AGENTS.md. Same content; this file is the Claude Code
     copy, AGENTS.md is the tool-neutral one. Edit both when either changes. -->

# CLAUDE.md

Guidance for AI agents working in this repository. Other agents read
`AGENTS.md`, which carries the same content.

## What this is

**Asylum** is an Agent Development Environment (ADE): a desktop app for running
a fleet of AI coding agents in parallel, each isolated in its own git worktree,
then comparing and merging the winner. It is written in Rust as a Cargo
workspace. The UI is built on the `gpui` library; `guise-ui` provides the
components, `libsinclair` provides the embeddable terminal, and SQLite
(`rusqlite`) is the store. Plugins use a manifest model (`plugin.toml`).

The GUI binary is `asylumdev`: a dev build (`cargo run -p app`) is named that so
it never collides with an installed `asylum`. Release packaging installs the
same binary as `asylum`.

## Commands

```sh
cargo run -p app                 # build and launch the ADE (asylumdev)
cargo build                      # build the workspace
cargo test                       # run all tests
cargo test -p git                # test one crate
cargo test -p store run          # tests matching "run" in one crate
cargo clippy --all-targets       # lint
```

Each crate keeps its tests in a sibling `tests/` directory mirroring `src/`,
pulled back in as a private module so unit tests keep access to private items:

```rust
// at the bottom of src/foo.rs
#[cfg(test)]
#[path = "../tests/foo.rs"]
mod tests;
```

Every crate sets `autotests = false`. The pure-logic crates (`git`, `store`,
`config`, `agent`, `plugin`, `pluginrt`) carry the coverage — prefer adding
tests there; `app` is gpui glue.

## gpui / guise-ui / libsinclair dependency

`gpui` comes from a pinned git rev (`96285fc1`). Because cargo
`[patch.crates-io]` entries do not propagate through git dependencies, the root
`Cargo.toml` carries the transitive patches (`async-process`, `async-task`, a
vendored `block` in `thirdparty/block`) and **redirects the crates.io `gpui` that
`guise-ui` requests onto the same rev**, so one gpui resolves across the whole
tree. `guise-ui` (`github.com/wess/guise`) and `libsinclair`
(`github.com/wess/sinclair`) are git dependencies pinned to that same gpui rev.
If you bump the gpui rev, match it across the `[patch]` and the workspace deps.
See `docs/gpui.md`.

## Architecture

The workspace is layered bottom-up; each crate depends only on those below it,
and the gpui-free core crates never import gpui.

- **`git`** — worktree, status, and diff operations. Pure, shells out to the
  `git` binary. Creates/lists/removes the isolated worktrees every task runs in,
  reports porcelain-v2 status, and parses unified diffs into a reviewable
  `DiffFile`/`DiffHunk`/`DiffLine` tree for the annotatable-diff surface.
- **`store`** — synchronous SQLite persistence (`rusqlite`, bundled). Owns the
  domain model — **projects** (repos), **tasks** (a prompt), and **runs** (one
  agent's attempt in one worktree) — plus idempotent `user_version` migrations.
  Deliberately blocking: the app owns one `Db` and calls it directly.
- **`config`** — layered settings: compiled defaults overridden by the user's
  `settings.json` (JSON with comments). A bad value never aborts the load — it
  becomes a `Diagnostic` and the default is used. `$XDG_CONFIG_HOME/asylum/settings.json`.
- **`agent`** — the agent-facing half of orchestration. A `registry` of known
  CLI agents (Claude Code, Codex, OpenCode, Gemini, Aider, Cursor) and how each
  is launched; `command` building (agent def + user prefs + prompt → a
  `SpawnSpec`); and `plan` (fan a task out to N agents → one branch + worktree
  per agent). Pure — it never spawns a process; the app runs a `SpawnSpec`
  inside a `libsinclair` terminal pane.
- **`plugin`** — manifest-based plugins (`plugin.toml`). A manifest contributes
  `[[command]]` palette actions, a `[panel]`, a
  `[webview]`, `[[trigger]]` hooks on ADE events (`run_finished`,
  `worktree_created`, …), and `[[tool]]`s exposed to the agents. Plugins declare
  `capabilities`. Pure parsing + validation.
- **`pluginrt`** — the plugin runtime host. A **process runtime** (newline JSON
  over stdio, one-shot `invoke_once` or warm `Session`) and a sandboxed **WASM
  runtime** (`wasmi`): `invoke_wasm` loads a module and calls its `invoke` export
  over a linear-memory string ABI, linking only the capability host functions the
  plugin declared (a guest that never asked for `notify` can't import
  `host_notify`). See `docs/plugins.md`.
- **`runner`** — the execution engine. Launches an `agent::SpawnSpec` on a real
  pty via the headless terminal core, snapshots its output, and tracks
  its lifecycle (running → exited/code). Used by the CLI and background runs;
  the gpui app renders interactive panes with `libsinclair::TermView`.
- **`github`** — GitHub via the `gh` CLI: list PRs/issues, create a PR, derive a
  worktree branch from an issue. Pure JSON parsers + thin `gh` wrappers.
- **`linear`** — Linear over its GraphQL API (transport via `curl`): teams,
  projects, issues, create-issue. Pure response parsers + a `Client`.
- **`checks`** — detect a project's checks (type-check / lint / test by
  ecosystem), run them, and classify each PASS / FAIL / skipped.
- **`search`** — cross-worktree content search via ripgrep, falling back to
  `git grep`; parses the shared vimgrep format.
- **`preview`** — rich file previews: markdown → HTML (`pulldown-cmark`) plus
  image / PDF / text / binary classification.
- **`remote`** — SSH remote-worktree and port-forward command builders
  (ControlMaster passphrase caching, ServerAlive/autossh reconnect). Pure argv.
- **`notify`** — desktop notifications (`osascript` / `notify-send`).
- **`designmode`** — the design-mode injectable JS (click an element → capture
  its HTML/CSS/selector via `window.ipc.postMessage`) plus the capture parser and
  agent-prompt builder.
- **`fuzzy`** — subsequence match + ranking (fzf-style scoring) behind the
  command palette and quick-open.
- **`companion`** — the mobile companion HTTP server: a dependency-light blocking
  server over the store (projects / tasks / runs / notifications, a follow-up
  endpoint, and a mobile web page). Routing is pure over a `Db`, so it is tested
  without sockets.
- **`cli`** — the `asylum` binary: `worktree` ops, `run <agent> <prompt>`,
  `search`, and computer-use `snapshot` / `click` / `fill`.
- **`app`** — the gpui application. Owns the window, the guise theme bridge, and
  the ADE shell composed with guise's `AppShell`: a header (with the command
  palette + quick-open overlays), an activity switcher + project/task navbar
  (with pins), a status footer, and a routed main area with eleven surfaces —
  Tasks (fan-out board with drag-drop, merge, PR-create), Diff review (+ PASS/FAIL
  checks, branch chips, inline annotations), Search, Integrations (GitHub PRs/
  issues + issue→worktree + Linear), Terminal (`libsinclair::TermView`, splittable),
  Editor (+ file tree), Preview (markdown/image/PDF), Browser (embedded web view +
  design mode), Plugins, Accounts (+ usage), and the notification Inbox. `main`
  also launches the `companion` server on a background thread. State lives in the
  single `Root` entity, which owns the on-disk `store::Db` and the selection/view.

### Data model

A **project** is a git repo. A **task** is a prompt you pose against a project.
Fanning a task out creates one **run** per selected agent: `agent::plan::fanout`
allocates a branch and worktree path per agent, `git::worktree::create` makes
the worktree, and `store::Db::create_run` records it. Each run's agent is
launched from a `SpawnSpec` inside a terminal pane; its status
(`queued`→`running`→`succeeded`/`failed`/`cancelled`) is tracked in the store.
When a run wins, its branch merges back to the project's base branch.

## Working in this repo

- File naming: lowercase, no spaces/`-`/`_`. Split by directory instead of
  compound names (`src/foo/bar.rs`, not `src/foo-bar.rs`). Small, focused files.
- Functional style; avoid classes/OO where a free function + plain data will do.
- Bun over node for any tooling/plugins.
- Commit messages / PRs never mention AI authorship and carry no co-author
  trailer.
- Keep the gpui-free core crates free of gpui — the boundary is `app`.

## Docs

- `docs/roadmap.md` — built vs. planned.
- `docs/architecture.md` — crate-by-crate detail and data flow.
- `docs/plugins.md` — the plugin manifest, runtime, and capability model.
- `docs/gpui.md` — the gpui/guise/libsinclair dependency recipe.
