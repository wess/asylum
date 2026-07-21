# Contributing to Asylum

## Development Setup

Asylum is a Rust Cargo workspace using the `gpui` GPU-accelerated UI library.

### Prerequisites

- Rust stable (via [rustup](https://rustup.rs/))
- For macOS: `xcode-select --install`
- For Linux: see [gpui platform prerequisites](https://github.com/zed-industries/gpui#linux)
- Bun 1.0+ (for site tooling; install from [bun.sh](https://bun.sh))

### Quick Start

```sh
# Launch the dev ADE (dev binary stays named asylumdev, never collides with release asylum)
cargo run -p app

# Run the full test suite
cargo test

# Lint all targets
cargo clippy --all-targets -- -D warnings

# Format code
cargo fmt
```

## Crate Layout

The workspace is organized in layers, with each crate depending only on those below it. See [`docs/architecture.md`](docs/architecture.md) for full details.

**Core logic** (gpui-free, thoroughly tested):
- `git` — worktree, branch, status, and diff operations; creates/lists/removes isolated worktrees
- `store` — SQLite persistence: projects, tasks, runs, annotations, accounts, usage, notifications
- `config` — layered settings.json, project asylum.toml, keybindings, and layouts
- `agent` — 31-agent registry, custom agents, command build, fan-out planning
- `runner` — pty agent execution and lifecycle supervision
- `github`, `linear` — integration via GitHub's `gh` CLI and Linear's GraphQL API
- `checks` — type-check / lint / test runner with PASS/FAIL classification
- `search` — cross-worktree content search (ripgrep / git grep)
- `notes` — Markdown vault CRUD, properties, wiki links, backlinks, tags
- `preview` — markdown → HTML, image / PDF / text classification
- `remote` — SSH remote-worktree and port-forward builders
- `notify` — desktop notifications
- `designmode` — design-mode click capture and annotation
- `fuzzy` — subsequence match and ranking for command palette and quick-open
- `companion` — mobile companion HTTP server
- `control` — agent control surface (local JSON API)
- `proxy` — secrets proxy for masked API calls
- `keep` — encrypted, project-scoped secret store
- `plugin` — plugin.toml manifest parsing and GitHub discovery
- `pluginrt` — process and WASM plugin runtimes
- `cli` — the `asylum` CLI binary

**UI glue**:
- `app` — gpui application, 13 surfaces, window and theme management

## Code Conventions

- **File naming**: lowercase, no spaces, hyphens, or underscores. Split modules by directory instead of compound names (`src/foo/bar.rs`, not `src/foo-bar.rs`).
- **Module organization**: keep files small and focused. Tests live in a sibling `tests/` directory mirroring `src/`, pulled back as a private module:

  ```rust
  // at the bottom of src/foo.rs
  #[cfg(test)]
  #[path = "../tests/foo.rs"]
  mod tests;
  ```

- **Testing**: every crate sets `autotests = false`. Coverage lives in the pure-logic crates (`git`, `store`, `config`, `agent`, `notes`, `plugin`, `pluginrt`); prefer adding tests there. `app` is gpui glue and minimal testing.
- **Style**: functional programming over classes/OO. Avoid unnecessary state and complexity.
- **gpui boundary**: the `app` crate imports gpui; all others stay gpui-free.
- **Tooling**:
  - Site (`site/`) uses Bun instead of node/npm
  - Use `cargo fmt` to format
  - Use `cargo clippy --all-targets -- -D warnings` to lint
  - Pin Rust version in `rust-toolchain.toml` when warranted

## Pull Request Expectations

1. **Tests**: behavior changes must include tests. New pure-logic functions should be covered in the relevant `tests/` sibling.
2. **Linting**: `cargo clippy --all-targets -- -D warnings` must pass; no warnings allowed.
3. **Format**: `cargo fmt` must pass.
4. **CI**: GitHub Actions runs clippy, fmt, and tests; the CI check must pass before merge.
5. **UI changes**: include a screenshot or screencast (paste into the PR description).
6. **Commit messages**: keep them terse and honest; describe the *why*, not the *what*.
7. **No force-push to main**: use a regular merge or rebase.

## Questions?

See the crate-level documentation in [`docs/architecture.md`](docs/architecture.md), the [`CLAUDE.md`](CLAUDE.md) / [`AGENTS.md`](AGENTS.md) developer guides, and the [`docs/`](docs/) subsystem detail (plugins, secrets, roadmap, etc.).
