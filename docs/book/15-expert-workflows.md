# Chapter 15: Expert Workflows

You now know every surface, the CLI, the control surface, events, plugins, and
the settings. This final chapter is about *judgment* — how to combine them into
workflows that get real work done, and the opinions worth holding while you do.
It assumes fluency with everything before it.

## Multi-agent tournaments

The default mode of Asylum is a tournament: fan one prompt across several agents
and pick the winner. A few habits make tournaments pay off:

- **Match the layout to the stakes.** Use a one-agent `quick` layout for routine
  changes, `duel` for anything with a real decision in it, and a wide `swarm`
  (with a modest concurrency) for hard or ambiguous problems where you genuinely
  do not know which agent will find the good approach.
  ([Chapter 5](05-layouts-and-presets.md).)
- **Judge on checks, not vibes.** Two diffs can both look plausible; the one whose
  `cargo test` / `pytest` / `go test` passes is the one to trust. Run checks on
  every finalist before you compare style
  ([Chapter 6](06-diffs-checks-and-review.md)).
- **Keep the prompt verifiable.** A task with a clear success criterion ("adds a
  `--version` flag that exits 0") produces a tournament you can actually score.
  Vague tasks produce diffs you cannot rank.
- **Cap concurrency to your quota.** Five agents at once burns CPU and API limits.
  A `swarm` with `concurrency: 3` gives you breadth without the cost spike;
  `max_parallel_runs` is your global backstop
  ([Chapter 4](04-the-fleet-in-depth.md)).

## Review-driven iteration

The strongest workflow is not "fan out once and merge." It is a loop:

1. Fan out to a handful of agents.
2. Run checks; discard the runs that fail outright.
3. On the survivors, **annotate the exact lines** you want changed and ship the
   annotations back — each survivor iterates in its own worktree, in parallel,
   with your line-level feedback ([Chapter 6](06-diffs-checks-and-review.md)).
4. Re-check, and repeat until one run is clearly best.

You are steering several agents at once with precise feedback, and the worktrees
are durable across attempts, so nothing is lost between rounds. This tends to
beat both "trust one agent" and "start over" — you converge on a good answer by
correcting several attempts in parallel.

## Agent-driven sub-fleets

The expert-level move is to let an agent grow its own team via the control
surface ([Chapter 11](11-agent-orchestration-and-the-control-surface.md)). A lead
agent, from inside its worktree, can:

- **Split work that genuinely parallelizes.** Spawn a test-writer sibling while it
  implements; spawn a docs sibling while it refactors. Each helper is isolated in
  its own worktree, so the lead's work is never contended.
- **Coordinate through reads and waits.** Read a sibling's transcript to align on
  an interface (cite what you learned, do not copy), and `asylum wait` on a
  sibling instead of polling.
- **Report state honestly.** `asylum control activity blocked` before pausing for
  input, `done` when finished — so you and the other agents see the real picture
  on the board and on your phone.

To enable this well: seed each agent's rules directory with `asylum control
skill`, keep `control.enabled` on and localhost-only, and remember that spawns and
checks are *queued* and drained by the app — so the desktop app must be running
for a sub-fleet to come alive. Treat a sub-fleet as a power tool: it multiplies
both output and cost, so reserve it for tasks where the parallel split is real.

## Following long runs remotely

A big tournament can outlast your attention span at the desk. Use the event
stream to stay in the loop from anywhere
([Chapter 12](12-the-mobile-companion-and-events.md)):

- Bind the companion to `0.0.0.0:8787` **with a token** and watch runs — including
  their live `blocked` activity — from your phone.
- Send a **follow-up** from your phone to nudge a live run without returning to
  the machine.
- For scripting, tail `GET /api/events?since=<cursor>` (or `/control/events` from
  an agent) and react to `run_finished` / `run_failed` instead of polling.

The activity signal is the payoff here: away from your desk, you still know the
moment an agent is stuck waiting on you.

## Remote worktrees

For heavier isolation you can push run execution onto another machine over SSH.
Asylum includes the building blocks for this — command builders for **remote
worktrees** and **port-forwarding** (with ControlMaster passphrase caching and
autossh-style reconnect). This is the foundation for running the fleet on a beefy
remote host while you review locally; surfacing it fully in the UI and CLI is on
the roadmap, so treat it today as an available capability at the argv level rather
than a one-click button. When you reach for it, keep the same discipline: one
worktree per run, isolation preserved across the network.

## Plugin-extended pipelines

Plugins ([Chapter 13](13-plugins.md)) let you wire the ADE into the rest of your
world:

- A **`[[trigger]]` on `run_finished` or `task_merged`** can notify a channel,
  kick a deploy, or file a follow-up — turning a merge into the first step of a
  pipeline rather than the last.
- A **`[[tool]]`** exposes an internal capability (ticketing, a knowledge base, a
  deploy hook) to the agents themselves, so a run can act on your systems within
  the capabilities you grant it.
- Prefer the **WASM runtime** for anything shared or untrusted — capabilities are
  enforced there, not merely advisory.

## Best practices, condensed

- **Small, verifiable tasks** beat big vague ones — they make the tournament
  scorable and the diffs reviewable.
- **Checks are the arbiter.** Never merge on a diff's looks alone; the guarded
  merge will block failed checks, and so should you.
- **Iterate in place** with annotations rather than re-prompting from scratch —
  worktrees are durable, use them.
- **Right-size concurrency** to your machine and quota; breadth with a concurrency
  cap beats a stampede.
- **Keep control and companion secured** — localhost by default, a token before
  the LAN.
- **Let agents self-report state**; the classifier is a fallback, not the truth.
- **Reserve sub-fleets** for work that genuinely parallelizes; each helper costs a
  worktree and quota.
- **Put durable context in notes** and attach them — inherited by every run, and
  the trail of runs/checks/PRs flows back in ([Chapter 7](07-notes-and-knowledge.md)).

## Try it

Design and run one end-to-end expert workflow:

1. Write a verifiable task and attach a spec note to it.
2. Launch it from a `swarm`-style layout with a sane concurrency.
3. From your phone, watch for the first `blocked` and answer it via a follow-up.
4. Discard failing runs by their checks, annotate the finalists, and ship the
   feedback back for one iteration.
5. Merge the winner behind the guarded preflight, and confirm the PR/check links
   landed in the note.

## Recap

- Tournaments, judged on checks, are the core; review-driven iteration converges
  several attempts in parallel.
- Agent-driven sub-fleets multiply output and cost — power tool, used sparingly.
- Events and the companion keep you in the loop remotely; remote worktrees push
  execution over SSH.
- Plugins turn merges into pipelines and expose your systems to agents under
  enforced capabilities.

## The end, and the beginning

That is the whole ADE — from "what is a worktree" to a lead agent commanding a
sub-fleet you follow from your phone. Return to the [table of
contents](index.md) whenever you need a specific chapter. Now go run a
tournament.
