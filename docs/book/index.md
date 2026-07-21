# The Asylum Book

*From beginner to expert with the Agent Development Environment.*

Asylum is a desktop application for running a **fleet of AI coding agents in
parallel** — each isolated in its own git worktree — then comparing their diffs,
running checks, and merging the winner. This book teaches it end to end: the
core idea, the day-to-day workflow, and the deep machinery that lets an agent
orchestrate its own siblings.

## How to read this

The book is ordered beginner → expert. If you are brand new, read the first
three chapters in order and stop; that is enough to be productive. Come back for
the intermediate and advanced chapters as questions arise. Experts can jump
straight to the chapter they need — each stands on its own and cross-links the
rest.

## Table of contents

### Beginner

1. [What Is Asylum?](01-what-is-asylum.md) — the ADE idea, why parallel agents +
   worktrees + compare-and-merge, the vocabulary (project / task / run), and a
   tour of the surfaces.
2. [Installation and Setup](02-installation-and-setup.md) — build and run,
   open a project, where `settings.json` lives, picking agents, and verifying an
   agent is on your PATH.
3. [Your First Task](03-your-first-task.md) — compose a prompt, fan it out,
   watch the runs, read the activity chips, review a diff, run checks, annotate a
   line, and merge the winner.

### Intermediate

4. [The Fleet in Depth](04-the-fleet-in-depth.md) — fan-out mechanics,
   worktrees and branches, parallel limits, retries and follow-ups, timeouts, and
   the semantic states in detail.
5. [Layouts and Presets](05-layouts-and-presets.md) — declarative fan-out
   presets, defining your own, concurrency, and `asylum layout`.
6. [Diffs, Checks, and Review](06-diffs-checks-and-review.md) — the annotatable
   diff, per-hunk staging, PASS/FAIL checks by ecosystem, shipping annotations
   back to an agent, and merging (or squash-merging) a winner.
7. [Notes and Knowledge](07-notes-and-knowledge.md) — the Markdown vault, wiki
   links, backlinks, tags, templates, and durable task/run/check/PR references.
8. [Integrations](08-integrations.md) — GitHub PRs and issues, issue →
   worktree, Linear, and opening a PR.
9. [Terminal, Editor, Preview, Browser](09-terminal-editor-preview-browser.md) —
   the splittable terminal, the editor and file tree, previews, and the browser's
   design mode.

### Advanced / Expert

10. [The CLI](10-the-cli.md) — every `asylum` subcommand with examples.
11. [Agent Orchestration and the Control Surface](11-agent-orchestration-and-the-control-surface.md)
    — the deep chapter: how a running agent orchestrates the fleet, the skill,
    env vars, spawning helpers, reading siblings, reporting state, waiting, and
    the queue/drain model.
12. [The Mobile Companion and Events](12-the-mobile-companion-and-events.md) —
    the companion server, the append-only event stream, and following the fleet
    from a phone.
13. [Plugins](13-plugins.md) — the `plugin.toml` manifest, process vs. WASM
    runtimes, enabling and trust, event triggers, installing and discovering
    plugins, and writing a simple one.
14. [Configuration Reference](14-configuration-reference.md) — a complete,
    annotated `settings.json` reference: every key, keybindings, per-agent
    overrides, custom agents, and control/companion prefs.
15. [Expert Workflows](15-expert-workflows.md) — multi-agent tournaments,
    agent-driven sub-fleets, review-driven iteration, and opinionated best
    practices.

## A note on vocabulary

Three words recur throughout and are worth fixing in your mind now:

- **Project** — a git repository you work in.
- **Task** — a prompt you pose against a project.
- **Run** — one agent's attempt at a task, executing in its own worktree.

Fan a task out and you get one run per agent. Everything else in Asylum is built
around watching, comparing, and choosing between those runs.

Start with [Chapter 1](01-what-is-asylum.md).
