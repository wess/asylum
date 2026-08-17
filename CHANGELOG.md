# Changelog

All notable changes to Asylum are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.2.0] - 2026-08-17

### Added

- **Getting started, in the app** (Help → Getting Started Video) — an 8 minute 41 second captioned tour that goes from the mental model to a first task, a review of what came back, and a safe merge. Player, video, poster, captions, chapters and transcript are compiled into the binary and extracted beside the database on first use, so a packaged build plays it with no network and no resource-lookup rules — the tutorial cannot rot separately from the app it teaches.
- **Devpipe machines, not just the vault** — the Devpipe client could read an account's secrets and nothing else. It now lists the boxes on the account, wakes a sleeping one, resolves where its daemon is, and lists or starts the terminals running on it. Same client, same token, same transport: a box is not a different service, it is the same account seen from another angle.

### Changed

- The video curriculum leads with the finished overview rather than a shot list, and the site carries it with chapters and a transcript.

### Security

- **A Devpipe forward names a port and never a host.** The daemon's authenticated websocket is the only way into a box — Devpipe closes port 22 to everything but its own control plane — and a forward that could be pointed anywhere would turn every box into a relay for whoever holds its token, which is the abuse that gets a whole provider account locked rather than one customer's machine.
- **A box's bearer token is held in memory and never written down.** It reaches one machine and is useless anywhere else, so it is treated as the short-lived thing it is rather than persisted beside the account's own credentials.

### Fixed

- Reattaching to a terminal on a box returns to the running shell instead of opening a new one. A session asked for with an empty argv is listed back as the shell the daemon resolved (`[]` in, `["/bin/zsh"]` out); comparing those literally is what made Devpipe's own client start a fresh shell on every connect, and the persistence the product is built on was invisible.

## [1.1.0] - 2026-08-13

### Added

- **Named agents with their own memory** (`asylum agent`) — a run is a thing that happened; a named agent is somebody who keeps happening. A name, a brief, and a growing list of what it has learned about *this* repository, put in front of every prompt it gets. Project-scoped, because a role is about a codebase. Selectable as chips in the composer, carrying the count of things each one knows.
- **`asylum control remember`** — a running agent writes to its own memory the same way it already reports activity and spawns helpers. Only its own: memory is the one thing on the control surface that outlives the task, so a sibling writing to it could plant an instruction that survives long after the run that planted it. Every write shows in the delegation thread.
- **Scheduled runs** (`asylum schedule`) — work that starts with nobody watching. A scheduled run is an ordinary run: same worktrees, same board, same review. Missed periods are skipped rather than replayed, so a laptop closed over a weekend wakes to one run instead of every night at once.
- **Routines** (`asylum routine`) — show Asylum a workflow once in an instrumented shell, replay it thereafter. Recording captures commands rather than keystrokes and screenshots, because what is worth replaying in a repository is the commands, and a command does not care where a window moved to.
- **Delegation thread** — the fleet view now shows what the agents said to each other, in order, built from control-surface events that were already being recorded. Delegation reads as a conversation rather than runs appearing for no stated reason.
- **Devpipe vault, read-through** — Asylum reads secrets from a Devpipe vault without copying them locally, so revoking a grant takes effect immediately rather than after the next sync.

### Security

- **Repository trust** — a repository's own `setup` commands and `asylum.toml` `env` no longer run until the project is trusted, and a scheduled run is never an exception. `env` is process-environment injection, where `PATH`, `NODE_OPTIONS` or `GIT_SSH_COMMAND` are code execution.
- **Memory writes are self-only**, enforced in `authorize` against the token's own run id rather than in routing.
- **A named agent may not take an agent id as its name** — fan-out entries resolve against the roster first, so an agent called `codex` would shadow codex itself and carry somebody else's memory into every run.
- **Preview CDN assets are pinned** with subresource integrity hashes.
- **Remote ssh hosts are validated** before being used to build a command.

### Fixed

- A timed-out probe or plugin killed only the shell it started; a forking shell left a grandchild holding the stdout pipe and the drain-thread join blocked until it exited on its own — a deadline that waits for the process it is supposed to bound. Now the whole process group is killed, and a timeout returns without joining.

## [1.0.0] - 2026-08-12

### Added

- **Fan-out orchestration** — run one prompt across N agents (Claude Code, Codex, OpenCode, Gemini, Aider, Cursor), each in an isolated git worktree; reusable layouts (`duel`, `triad`, `swarm`) race named agent sets in one pick. Merge the winner or open a PR without branch juggling.
- **Agent control surface** — a running agent spawns helper runs, reads siblings, runs checks, reports its semantic state (`working`, `blocked`, `done`, `idle`), and waits on other runs via a local JSON API it learns from `asylum control`.
- **Annotatable diff review** — inline comments shipped back to agents; PASS/FAIL checks and branch chips; all stored and durable across the fleet.
- **MCP gateway** — aggregates configured upstream MCP servers under per-service namespaces; agents connect to one loopback server instead of configuring N; tool calls are routed and audited per run and project.
- **Secrets system** — encrypted project-scoped credential store (`keep` via AES-256-GCM); masked API proxy (`call`) so agents use keys they never see; all loopback-only with token auth.
- **Mobile companion** — optional HTTP server (`:8787`, token-required) exposing projects/tasks/runs/notifications; `/api/events` stream for real-time mobile follow-up.
- **Plugin system** — manifest-based (`plugin.toml`) with process runtime (JSON over stdio) and sandboxed WASM runtime (`wasmi`, capability-gated); install from GitHub with `asylum plugin install <owner/repo>`.
- **Notes vault** — private or repository-backed Markdown with YAML properties, `[[wiki links]]`, backlinks, tags, templates, live preview, and durable task/run/check/PR links; attached notes become agent context.
- **Embedded terminal** (splittable, `libsinclair`), **code editor** with file tree, **markdown/image/PDF preview**, and **embedded browser with design mode** (click an element → its HTML/CSS to an agent).
- **GitHub & Linear integration** — list/create PRs and issues; derive a worktree branch from an issue; usage tracking.
- **Cross-worktree search** — ripgrep (or git grep fallback) across all active worktrees; vimgrep format output.
- **Command palette & quick-open** — fuzzy finder with subsequence ranking (fzf-style scoring); layouts, agents, projects, runs, notes, commands.
- **Desktop notifications & inbox** — unread tracking; notification history; click-through to context.
- **Fan-out layouts** — reusable, configurable agent race presets (`duel`, `triad`, `swarm`, or custom).
- **Cross-platform packaging** — `asylumdev` (dev binary, never collides with release); release builds target macOS (Apple Silicon), Linux (x64 + arm64), and Windows (x64) with DMG, `.deb`, `.tar.gz`, `.AppImage`, `.msi`, and `.zip` artifacts.
- **Full documentation** — fifteen-chapter book covering first task, fan-out, diffs/checks, notes, CLI, orchestration, plugins, and full configuration reference; plain-English beginners guide; CLI-tour video.

### Changed

- **Settings UI** — collapsible accordion layout; live reload on settings.json change; per-key edit that preserves comments; MCP server configuration section.
- **Onboarding** — first-run wizard explaining core concepts and letting agents be tested from Settings before the first real task.

### Fixed

- Startup race condition reading `Root` inside its own render.
- Quick-open & command-palette indexing reliability.
- Cross-platform terminal & URL handling edge cases.
- Version string formatting in `--version` output.
- Flaky `probe` deadline test that had left CI red on `main` since 2026-07-22. It bounded a 5-second child at 2 seconds — 1.85s of headroom over a 150ms deadline — which passed on developer machines and failed on loaded runners. The child now sleeps far longer than the bound, so the assertion proves the same property with room to spare.

### Security

- **Repository trust.** A project's `asylum.toml` is written by whoever authored the repository, and two of its fields execute: `setup` runs through a login shell with your privileges, and `env` is injected into every agent process (where `PATH`, `NODE_OPTIONS` or `GIT_SSH_COMMAND` are code execution). Opening a repository no longer implies permission to run it — both are withheld until you explicitly trust the project, and the confirm bar restates the exact commands and environment entries before you grant it. Trust is revocable, and withheld commands are recorded in the setup transcript rather than skipped silently.
- **The plugin trust prompt names the commit it is authorising.** Enabling a process plugin previously asked you to trust a name, which can be any commit. The disclosure now carries the installed revision, read from the tree rather than a record so it cannot drift — and says so plainly when a directory is not a clone and has no revision to name.
- **Markdown preview CDN assets are pinned with Subresource Integrity.** highlight.js was loaded from a jsdelivr `/gh/` path carrying no version — it resolved to whatever sat on that repository's default branch — and Mermaid floated across `@11`, neither with an integrity hash, in the webview that renders repository content. Both are now pinned to exact versions with SRI where the loader supports it; a mismatch degrades the preview to unhighlighted source rather than executing.
- **Checks require trust too.** Running checks executes commands the repository declares for itself (its own `package.json` scripts, Cargo/Go entry points), so an untrusted project reports that trust is needed rather than running them.
- **Existing projects default to untrusted** on upgrade. Having previously opened a repository is not evidence that its commands were reviewed, so each project asks once, from the readiness panel.

---

**Convention:** each release cut moves `Unreleased` under its version heading (e.g., `## [1.0.0] - 2024-12-01`), with sections for Added/Changed/Deprecated/Removed/Fixed/Security. New development targets the `Unreleased` section above.
