# Roadmap

## Built

- Durable SQLite projects, tasks, runs, terminal output, errors, attempts,
  review comments, and per-run check results.
- One prompt fanned across selected agents, with one branch and absolute
  worktree path per run.
- Background worktree setup, bounded parallel launches, queued/running status,
  cancellation, retry, timeout, and interrupted-run recovery.
- Live run terminals plus persisted output after exit or restart.
- Side-by-side run cards with status, elapsed time, changed-file counts,
  terminal output, and verification state.
- Semantic agent states (working/blocked/done/idle) classified from each run's
  output, shown as a board chip and exposed over the mobile and control APIs.
- Named fan-out layouts (built-in duel/triad/swarm, plus user presets) that race
  a set of agents in one pick, with an optional per-layout concurrency cap.
- Agent control surface: a running agent orchestrates the fleet from inside its
  worktree — spawn a helper run, read a sibling, report state, run checks, and
  wait on another run — with write-effects queued and drained by the app.
- Append-only event log streamed over both the companion (`/api/events`) and
  control (`/control/events`) servers so a phone or an agent follows the fleet
  without polling.
- Plugin install from GitHub (`asylum plugin install <owner/repo>`) and topic
  discovery (`asylum plugin search`).
- Selected-run diff review, inline comments, same-worktree continuation, and
  automatic type-check/lint/test execution.
- Merge preflight, failed-check blocking, dirty-base protection, conflict
  detection, PR creation, and clean-worktree cleanup.
- First-run repository flow with explicit git initialization consent, agent
  executable configuration, installed-versus-verified state, and a setup
  doctor for Git, branches, worktrees, agents, Bun, and Cargo.
- Task templates, per-task agent selection, workflow stages, actionable empty
  states, command palette actions, and keyboard shortcuts.
- Editor, terminal, preview, browser/design mode, search, integrations,
  plugins, accounts, inbox, settings, CLI, and companion server surfaces.
- Project Markdown vaults with private/repository storage, YAML properties
  (view + inline edit), wiki links, backlinks, click-to-filter tags, built-in
  and user templates, note embeds/transclusion, autocomplete, note/task/run
  context, unified search, and automatic task/run/check/PR links.
- Rendered-note callouts, Mermaid diagrams, and code syntax highlighting.
- Unified and side-by-side diff views.
- Worktrees started from a chosen branch or commit.
- Checks across JavaScript (bun/npm/pnpm/yarn), Cargo, Python, and Go.
- Process and capability-gated WASM plugin runtimes, with in-app command
  invocation and a runnable example plugin.
- Linear issue browsing and issue → worktree (with an API token).
- Mobile companion with bearer-token auth and follow-up delivery into a live run.
- Desktop packaging (`.dmg` / `.deb`), a tagged release workflow, and a launch
  update check against GitHub Releases.
- Provider account add + hot-swap and an install-guidance setup doctor.

## Next

- Per-hunk staging in the side-by-side diff.
- Background/cancellable project setup commands with per-command output.
- Provider-specific sign-in probes and live usage feeds for the account meter.
- Plugin trigger auto-dispatch on ADE events and plugin-contributed app panels.
- App icon, code signing/notarization credentials, Homebrew cask, and AUR
  package for a one-command install.

## Later

- Remote run execution over SSH surfaced in the UI/CLI (the argv builders exist).
- Full session restore across restart (terminals cannot survive a process exit;
  runs are recovered and their output persists, but live ptys are not resumed).
- Per-worktree browser sessions and design-mode screenshots.
- Native mobile shells around the companion API.
