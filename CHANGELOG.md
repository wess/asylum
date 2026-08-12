# Changelog

All notable changes to Asylum are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
