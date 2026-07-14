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
- Project Markdown vaults with private/repository storage, YAML properties,
  wiki links, backlinks, tags, templates, autocomplete, note/task/run context,
  unified search, and automatic task/run/check/PR links.
- Process and capability-gated WASM plugin runtimes.

## Next

- Side-by-side file diffs with syntax highlighting and per-hunk staging.
- Background/cancellable project setup commands with per-command output.
- Provider-specific sign-in probes where a CLI exposes a stable status command.
- Plugin trigger dispatch and plugin-contributed app panels.
- Release packaging, signing, update delivery, and fresh-install testing.

## Later

- Remote run execution over SSH.
- Per-worktree browser sessions and design-mode screenshots.
- Native mobile shells around the companion API.
