# Asylum Video Tutorials — Series Curriculum

Ready-to-record shot lists for a video series that teaches Asylum from first
launch to expert orchestration. Each file is a **script / storyboard**: narration
paired with on-screen actions and timecodes. They are not prose articles and not
finished videos — hand one to whoever records and they have a shot-by-shot plan.

These scripts track the [Asylum Book](../book/index.md) chapter for chapter, so
the videos and the written docs stay consistent. Every episode links its matching
book chapter under "Go deeper."

## How to use these scripts

- Each episode has a **shot list** table: `Time | On screen | Action | Narration`.
  Timecodes are cue points, not hard cuts — treat them as a pace guide toward the
  stated duration.
- **Narration** is spoken-word copy. Read it aloud; keep it in second person.
- **On screen** is what the viewer sees (the surface, a chip, a terminal). **Action**
  is what the presenter does (click, type, run a command).
- The **B-roll / capture notes** tell the recorder what to have open beforehand —
  a sample repo, a prompt, which agents to install.
- Record against the dev build (`cargo run -p app`, the binary is `asylumdev`).
  The CLI from source is `cargo run -p cli -- ...`; a released install is `asylum ...`.
  Scripts use the short `asylum ...` form and note the source form where it helps.
- Use a small throwaway git repo for demos so worktrees and merges stay cheap.

## Episodes

| # | Title | Objective | Duration | Prerequisites |
|---|-------|-----------|----------|---------------|
| 00 | [Welcome to Asylum](00-welcome.md) | What an ADE is: a fleet of agents in parallel, worktree isolation, compare-and-merge. | ~2 min | None |
| 01 | [Install and First Launch](01-install-and-first-launch.md) | Build and run Asylum, tour the window. | ~4 min | Rust + git installed |
| 02 | [Open a Project and Pick Agents](02-open-a-project-and-pick-agents.md) | Open a repo, edit settings.json, verify agents are on PATH. | ~5 min | Ep 01 |
| 03 | [Your First Fan-Out](03-your-first-fanout.md) | Compose a prompt, fan out to a few agents, watch runs spin up. | ~6 min | Ep 02 |
| 04 | [Reading Semantic States](04-reading-semantic-states.md) | The working/blocked/done/idle chips; spotting the blocked agent. | ~4 min | Ep 03 |
| 05 | [Review: Diffs, Checks, Annotations](05-review-diffs-checks-annotations.md) | The annotatable diff, PASS/FAIL checks, comments shipped back to an agent. | ~7 min | Ep 03 |
| 06 | [Merge the Winner](06-merge-the-winner.md) | Choose a winner, the guarded merge, or open a PR. | ~5 min | Ep 05 |
| 07 | [Layouts and Presets](07-layouts-and-presets.md) | duel/triad/swarm, layout chips, `asylum layout`, defining your own. | ~5 min | Ep 03 |
| 08 | [Notes and Knowledge](08-notes-and-knowledge.md) | The Markdown vault: wiki links, backlinks, tags, templates, attaching notes to tasks/runs. | ~6 min | Ep 03 |
| 09 | [Integrations](09-integrations.md) | GitHub PRs/issues + issue→worktree, Linear. | ~5 min | Ep 06 |
| 10 | [Terminal, Editor, Preview, Browser](10-terminal-editor-preview-browser.md) | Splittable terminal, editor + file tree, previews, browser design mode. | ~6 min | Ep 02 |
| 11 | [The CLI Tour](11-the-cli-tour.md) | Every `asylum` subcommand, including the aggregated MCP gateway. | ~8 min | Ep 03 |
| 12 | [The Agent Control Surface](12-agent-control-surface.md) | An agent orchestrating siblings: env vars, `asylum control`, `asylum wait`, a worked demo. | ~9 min | Ep 11 |
| 13 | [Mobile Companion and Events](13-mobile-companion-and-events.md) | The companion server, the event stream, following the fleet from a phone. | ~5 min | Ep 03 |
| 14 | [Plugins](14-plugins.md) | plugin.toml, process vs WASM, `asylum plugin`, building a simple one. | ~7 min | Ep 11 |
| 15 | [Expert Workflows](15-expert-workflows.md) | Tournaments, agent-driven sub-fleets, review-driven iteration, best practices. | ~7 min | All prior |

**Total runtime:** ~91 minutes across 16 episodes.

## Suggested learning paths

- **Just getting started:** 00 → 01 → 02 → 03. Enough to be productive.
- **Daily driver:** add 04 → 05 → 06 → 07 → 08.
- **Power user:** add 09 → 10 → 11 → 13.
- **Orchestration / extending:** 12 → 14 → 15.

## A note on vocabulary (used throughout)

- **Project** — a git repository you work in.
- **Task** — a prompt you pose against a project.
- **Run** — one agent's attempt at a task, in its own git worktree.

Fan a task out and you get one run per agent. Everything else is watching,
comparing, and choosing between those runs.
