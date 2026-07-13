# Roadmap

What's built vs. planned.

## Built

- **Cargo workspace** — gpui (pinned rev), guise-ui, libsinclair, SQLite — with
  the crates.io→git `gpui` patch so one gpui resolves
  across the tree.
- **`git`** — create / list / remove worktrees; porcelain-v2 status; unified
  diff parsing into a reviewable `DiffFile`/`DiffHunk`/`DiffLine` model.
- **`store`** — SQLite (rusqlite) projects / tasks / runs with migrations.
- **`config`** — layered `settings.json` (JSONC) with non-fatal diagnostics.
- **`agent`** — registry of 6 CLI agents, command building with prompt delivery
  modes, and fan-out planning (one prompt → N branches + worktrees).
- **`plugin` / `pluginrt`** — manifest parsing (commands, panels, webviews,
  triggers, tools, capabilities) and a process runtime speaking JSON over stdio.
- **`app`** — the gpui ADE shell: project/task navbar, per-agent run-card board,
  status footer, theme toggle, native menu.

## In progress / next

- **Run execution** — take a `SpawnSpec` and launch the agent on a pty inside a
  `libsinclair` terminal pane; stream output into the run card; update run
  status from the process exit.
- **Fan-out action** — wire the "Run fan-out" button: `agent::plan::fanout` →
  `git::worktree::create` per plan → `store::create_run` → launch.
- **Diff review surface** — render `git::diff` output with inline annotations
  that feed back into an agent (the signature review flow).
- **Trigger dispatch** — fire `run_finished` / `worktree_created` / … plugin
  triggers from the app; wire `pluginrt` panels and webviews into the UI.
- **Persistent store on disk** — open `Db` at a real path; project onboarding
  (add an existing repo) instead of the seeded demo.

## Later

- **WASM plugin runtime** — execute the `wasm` runtime tier (wasmtime component
  model + capability-gated host imports).
- **Embedded browser / Design Mode** — a Chromium surface per worktree; click a
  UI element to send HTML/CSS/screenshot to an agent.
- **GitHub / Linear integration** — browse PRs, issues, boards.
- **SSH worktrees** — remote execution.
- **Mobile companion** — monitor runs, send follow-ups.
- **Release packaging** — bundle `Asylum.app` / Linux packages, self-update.
