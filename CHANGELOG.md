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

---

**Convention:** each release cut moves `Unreleased` under its version heading (e.g., `## [1.0.0] - 2024-12-01`), with sections for Added/Changed/Deprecated/Removed/Fixed/Security. New development targets the `Unreleased` section above.
