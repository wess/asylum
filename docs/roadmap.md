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
- MCP gateway: one aggregating MCP server every agent connects to, fronting the
  configured upstream servers (stdio or HTTP) under per-service namespaces
  (`github__create_pr`), with per-server tool `allow`/`deny`, project scoping, a
  lazy `search` exposure mode, and per-run auditing of tool calls.
- Plugin install from GitHub (`asylum plugin install <owner/repo>`) and topic
  discovery (`asylum plugin search`).
- Plugin `[[trigger]]` dispatch on 8 ADE events, off the UI thread with a
  per-invocation timeout, for a plugin the user has explicitly enabled — a
  trust confirmation gates enabling a process runtime; WASM enables directly.
- Selected-run diff review, inline comments, same-worktree continuation, and
  automatic type-check/lint/test execution.
- Per-hunk and per-file staging on the Review surface: a successful run's
  commit is deferred to merge time (regular or squash) so the diff shown stays
  the live worktree, and staging state always reads from Git.
- Merge preflight, failed-check blocking, dirty-base protection, conflict
  detection, PR creation, and clean-worktree cleanup.
- First-run repository flow with explicit git initialization consent, agent
  executable configuration, installed-versus-verified state, and a setup
  doctor for Git, branches, worktrees, agents, Bun, and Cargo.
- Background, cancellable project setup commands (`asylum.toml` `setup`): each
  command runs separately with its own output in a cancellable "Preparing"
  banner, under a 10-minute per-command timeout, and a failure names the exact
  command and exit code.
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
- Desktop packaging across all three platforms: macOS `.dmg`, Linux `.deb` /
  `.tar.gz` / AppImage (x64 + arm64), and Windows `.zip` / `.msi` — Windows
  compiles and links in CI but is **beta**: not runtime-tested on a real machine,
  and its installers are unsigned.
- A version-driven release workflow (bump the version, push to `main`, and it
  tags, builds every platform, and uploads the artifacts) and a launch update
  check against GitHub Releases.
- Package-manager distribution: a Homebrew cask refreshed per release in
  `wess/homebrew-packages`, plus Scoop and Chocolatey manifests rewritten with
  each release's version and checksum. Chocolatey community-feed publishing needs
  an API key and moderation, so it stays manual.
- App icon (`assets/icon.svg` plus generated raster/`.icns`, regenerated with
  `packaging/icon.sh`).
- macOS code-signing and notarization wired into `packaging/macos.sh` and the
  release workflow, gated on repository secrets — a build without them still
  succeeds, producing an unsigned bundle. The certificates themselves are not yet
  provisioned, so today's builds are unsigned and Gatekeeper blocks them on
  install (see the README for the workaround).
- Provider account add + hot-swap and an install-guidance setup doctor.
- Provider sign-in probes for the account meter (Claude, GitHub, and Codex have
  a safe non-interactive status check; Gemini does not, and is reported
  unsupported rather than guessed), run on demand under a timeout so a hung CLI
  never blocks the app.

## Next

- Live usage feeds for the account meter — it tracks used/limit/reset from
  local schema today, without a per-provider network fetch.
- Host dispatch for the three still-unwired plugin contribution types:
  plugin-contributed panels and webviews, and plugin tools offered to the
  agents. All three parse and validate today; `[[command]]` and `[[trigger]]`
  are the two the app acts on.
- Provisioning real signing certificates: a Developer ID for macOS (the pipeline
  is already wired for it) and an Authenticode certificate for the Windows
  installers, which an EV certificate would clear SmartScreen for.
- Runtime-testing Windows on real hardware to take it out of beta.
- An AUR package for a one-command install on Arch.

## Later

- Remote run execution over SSH surfaced in the UI/CLI (the argv builders exist).
- Full session restore across restart (terminals cannot survive a process exit;
  runs are recovered and their output persists, but live ptys are not resumed).
- Per-worktree browser sessions and design-mode screenshots.
- Native mobile shells around the companion API.
