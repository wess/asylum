# Asylum

**An Agent Development Environment.** Run a fleet of AI coding agents in
parallel — each isolated in its own git worktree — then compare their work and
merge the winner.

Asylum is a native Rust application. The GPU-accelerated UI is built on the
`gpui` library, with the [guise-ui](https://github.com/wess/guise) component
library, `libsinclair` for embedded terminals, and SQLite for persistence.
Plugins use a `plugin.toml` manifest model.

## Why

Traditional IDEs are built for one human at a keyboard. Parallel-agent wrappers
stop at a terminal. Asylum is the whole environment for *agents*: fan one prompt
across several agents (Claude Code, Codex, OpenCode, Gemini, Aider, Cursor),
each working in an isolated worktree, watch them run, review annotatable diffs,
and merge the best result — without branch juggling or stashing.

## Status

Each feature is backed by tested logic and a working UI or CLI surface. See
[`docs/parity.md`](docs/parity.md) for the feature-by-feature matrix.

- **Fan-out orchestration** — one prompt across N agents, each in an isolated
  worktree; run execution on a real pty; compare, then merge the winner or open a
  PR (`agent`, `runner`, `git`, `github`).
- **Annotatable diff review** — inline comments shipped back to agents, plus
  PASS/FAIL checks and branch chips (`git`, `store`, `checks`).
- **Embedded terminal** (splittable, `libsinclair`), **code editor** + file tree,
  **markdown/image/PDF preview**, and an **embedded browser with design mode**
  (click an element → its HTML/CSS to an agent).
- **GitHub / Linear** integration, **cross-worktree search**, **command palette**
  + **quick-open** (fuzzy), **desktop notifications** + unread inbox, **accounts**
  + usage, **pinned/recent** projects, and a collapsible icon-only activity rail.
- **Project memory** — private or repository-backed Markdown vaults with YAML
  properties, `[[wiki links]]`, backlinks, tags, templates, live preview, and
  durable task/run/check/PR links. Attached notes become agent context.
- **Plugins** — manifest system with a process runtime *and* a sandboxed WASM
  runtime (`wasmi`, capability-gated).
- **CLI** (`asylum`) with computer-use automation, and a **mobile companion**
  server (live on `:8787`).

## Build & run

```sh
cargo run -p app          # launch the ADE (dev binary: asylumdev)
cargo test                # run the suite
cargo clippy --all-targets
```

`guise-ui` and `libsinclair` are git dependencies; the first build fetches them.
See [`docs/gpui.md`](docs/gpui.md) for the dependency recipe.

## Website

The GitHub Pages site lives in `site/` and uses Bun with Vite:

```sh
cd site
bun install
bun run dev
bun run build
```

Pushes to `main` that touch `site/` deploy through
`.github/workflows/pages.yml`. The production output is `site/dist/`.

## Layout

```
crates/
  git       worktree + branch + status + diff (pure)
  store     SQLite persistence: projects/tasks/runs, annotations, accounts,
            usage, notifications, pins/recents (rusqlite)
  config    layered settings.json + project asylum.toml + keybindings
  agent     31-agent registry, custom agents, command build, fan-out planning
  runner    pty agent execution + lifecycle supervisor (libsinclair headless)
  github    GitHub via gh: PRs, issues, create PR
  linear    Linear GraphQL: teams, projects, issues
  checks    type-check / lint / test runner with PASS/FAIL
  search    cross-worktree content search (ripgrep / git grep)
  notes     Markdown vault CRUD, properties, links/backlinks, templates, search
  preview   markdown → HTML, image / PDF / text classification
  remote    SSH remote-worktree + port-forward command builders
  notify    desktop notifications
  designmode click an element → capture HTML/CSS/selector for an agent
  fuzzy     subsequence match + ranking (command palette, quick-open)
  companion mobile companion HTTP server + mobile web page
  plugin    plugin.toml manifest parsing
  pluginrt  process runtime (JSON over stdio) + WASM runtime (wasmi)
  cli       the `asylum` binary (worktree/run/search/computer-use)
  app       the gpui application (asylumdev) - 13 surfaces
```

See [`CLAUDE.md`](CLAUDE.md) / [`AGENTS.md`](AGENTS.md) for the architecture and
conventions. Start with [`docs/gettingstarted.md`](docs/gettingstarted.md) for
the first-run workflow, then use [`docs/`](docs/) for subsystem detail.
