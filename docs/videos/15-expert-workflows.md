# Episode 15 — Expert Workflows

**Duration:** ~7 min · **Level:** Advanced
**You'll learn:** how to combine everything into real workflows — multi-agent tournaments, review-driven iteration, agent-driven sub-fleets — and the opinions worth holding.
**Prerequisites:** All prior episodes. This assumes fluency with the whole app.

## Shot list

| Time | On screen | Action | Narration |
|------|-----------|--------|-----------|
| 0:00 | Montage of the board, diff, and control panes. | Fast recap montage. | "You know every surface, the CLI, the control surface, events, and plugins. This last episode is about judgment — combining them into workflows that get real work done." |
| 0:22 | A swarm layout fanning out. | Launch a tournament. | "The default mode of Asylum is a tournament: one prompt across several agents, pick the winner. A few habits make it pay off." |
| 0:44 | Layout picker: quick / duel / swarm. | Match layout to stakes. | "Match the layout to the stakes. A one-agent quick layout for routine changes, duel for anything with a real decision in it, and a wide swarm — with a modest concurrency — for hard, ambiguous problems where you don't yet know which agent will find the good approach." |
| 1:16 | Two diffs with checks, one green. | Compare on checks. | "Judge on checks, not vibes. Two diffs can both look plausible; the one whose test suite passes is the one to trust. Run checks on every finalist before you compare style." |
| 1:44 | A crisp, verifiable prompt on screen. | Show a good prompt. | "Keep the prompt verifiable. A task with a clear success criterion produces a tournament you can actually score. Vague tasks produce diffs you can't rank." |
| 2:10 | Concurrency cap overlay. | Show swarm concurrency 3. | "And cap concurrency to your quota. Five agents at once burns CPU and API limits; a swarm with concurrency three gives you breadth without the cost spike. max-parallel-runs is your global backstop." |
| 2:38 | The review loop diagram. | Animate the four steps. | "The strongest workflow isn't 'fan out once and merge.' It's a loop. Fan out to a handful of agents. Run checks; discard the runs that fail outright." |
| 3:06 | Annotating finalists in parallel. | Ship annotations back to survivors. | "On the survivors, annotate the exact lines you want changed and ship the annotations back. Each iterates in its own worktree, in parallel, with your line-level feedback. Re-check, and repeat until one run is clearly best." |
| 3:38 | Two runs converging over attempts. | Show attempt counts climbing. | "You're steering several agents at once with precise feedback, and worktrees are durable across attempts, so nothing is lost between rounds. This beats both trusting one agent and starting over." |
| 4:06 | Lead agent spawning helpers (control pane). | Show a sub-fleet forming. | "The expert-level move is letting an agent grow its own team through the control surface. A lead agent can split work that genuinely parallelizes — spawn a test-writer while it implements, a docs sibling while it refactors — each helper isolated in its own worktree." |
| 4:38 | Reads + waits between siblings. | Show read / wait usage. | "It coordinates through reads and waits — read a sibling to align on an interface, wait on it instead of polling — and reports state honestly, blocked before it pauses, done when it finishes." |
| 5:06 | Reminder overlay: queued/drained, app running. | Show the caveat. | "To enable this well: seed each agent's rules directory with the control skill, keep control enabled and localhost-only, and remember spawns and checks are queued and drained by the app — so the desktop must be running for a sub-fleet to come alive. Treat it as a power tool: it multiplies both output and cost." |
| 5:36 | Phone watching a long run's blocked chip. | Follow remotely. | "For long tournaments, stay in the loop from anywhere. Bind the companion to the LAN with a token and watch the blocked activity from your phone; send a follow-up to nudge a live run; or tail the event stream and react to run_finished instead of polling." |
| 6:04 | Plugin trigger on task_merged. | Show a pipeline trigger. | "And plugins turn a merge into the first step of a pipeline. A trigger on run_finished or task_merged can notify a channel or kick a deploy; a tool exposes your systems to the agents themselves — and for anything shared or untrusted, prefer WASM, where capabilities are enforced." |
| 6:32 | Best-practices card. | List the condensed rules. | "The condensed opinions: small verifiable tasks over big vague ones. Checks are the arbiter — never merge on looks. Iterate in place with annotations. Right-size concurrency. Keep control and companion secured. Let agents self-report state. Reserve sub-fleets for work that genuinely parallelizes. And put durable context in notes, attached." |
| 7:00 | End card over the table of contents. | Fade out. | "That's the whole ADE — from what a worktree is to a lead agent commanding a sub-fleet you follow from your phone. Now go run a tournament." |

## B-roll / capture notes
- This is a synthesis episode: reuse strong shots from earlier episodes (blocked chip, annotation ship-back, control spawn, phone view) so it feels like a capstone.
- If you record one end-to-end flow, use the book's suggested one: a verifiable task with an attached spec note, launched from a swarm, answered via a phone follow-up, finalists annotated, winner merged behind the preflight, and the PR/check links landing in the note.
- Keep the best-practices card readable — it doubles as the recap.
- Don't introduce any feature not shown earlier; this episode only recombines.

## Recap card (end screen)
- Tournaments, judged on checks, are the core; review-driven iteration converges several attempts in parallel.
- Agent-driven sub-fleets multiply output and cost — a power tool, used sparingly.
- Events and the companion keep you in the loop remotely.
- Plugins turn merges into pipelines and expose your systems to agents under enforced capabilities.

## Next
- Back to the [series index](index.md), or revisit any [book chapter](../book/index.md).

Go deeper: [book chapter 15](../book/15-expert-workflows.md).
