# Chapter 4: The Fleet in Depth

You have run one task. Now you will learn what actually happens when you fan out:
how worktrees and branches are allocated, how Asylum caps parallelism, how
retries and follow-ups feed an agent more input, how timeouts stop a stuck run,
and — in detail — how the four semantic activity states are detected.

## Fan-out mechanics

Fanning a task out is a precise sequence:

1. **Plan.** Asylum asks the planner for one *run plan* per selected agent. Each
   plan is a unique **branch name** and a unique **worktree path**, derived from
   the task and the agent id (the agent id is slugified so it makes a safe branch
   name).
2. **Allocate.** For each plan it creates the git worktree on its branch — an
   isolated checkout on disk — and records a **run** in the store with status
   `queued`.
3. **Launch.** For each run it builds a *spawn spec* (the program, the fully
   substituted arguments, the working directory, and — for stdin-delivery agents
   — the prompt piped to stdin) and starts that agent on a real pty inside a
   terminal pane. The run flips to `running`.
4. **Track.** As the pty produces output, Asylum snapshots the transcript into
   the store so it survives the pty being gone. On exit it records the exit code,
   updates status, leaves a successful run's changes uncommitted in its worktree
   so you can stage exactly what you want on the Review surface, and kicks off
   that worktree's detected checks.

Because every run has its own branch and worktree, the agents never contend. The
task's own status moves `draft` → `running` → `review` as its runs progress, and
`merged` once you merge a winner.

## Worktrees and branches

A **worktree** is a second working copy of your repository, checked out on its
own branch, living in its own folder (by default under
`.asylum/worktrees` inside the project). Worktrees share the repository's history
but have independent files, which is what makes parallel agents safe.

You do not manage these by hand. Asylum creates them on fan-out; a **Clean up
finished worktrees** action removes the clean, finished ones and deletes any
branch that is now safely merged — Git's safe delete refuses anything that is
not, so a losing run's branch and work are never lost. You can also inspect and
manipulate worktrees directly with the CLI
([Chapter 10](10-the-cli.md)):

```sh
asylum worktree list
asylum worktree create ../wt-experiment --branch experiment
asylum worktree remove ../wt-experiment
```

## Parallel limits and the queue

Racing agents cost CPU, memory, and API quota, so Asylum bounds how many run at
once. The global cap is `max_parallel_runs` in `settings.json` (default 4; 0
means unlimited):

```jsonc
{ "max_parallel_runs": 4 }
```

When you fan out more runs than the cap allows, the extra runs sit `queued`. As
each running agent finishes and frees capacity, the queue launches the next one.
A [layout](05-layouts-and-presets.md) can set its own per-task concurrency that
overrides this global cap for tasks launched from it.

## Retries, follow-ups, and continuation

A run is not a dead end when it finishes. Several mechanisms feed an agent more:

- **Retry** relaunches a run in its existing worktree — useful after a transient
  failure.
- **Review continuation.** Shipping [diff annotations](06-diffs-checks-and-review.md)
  back to a finished run starts a fresh attempt in the *same* worktree, so the
  agent iterates on its own changes. The store tracks an **attempt** count per
  worktree.
- **Follow-ups.** A message queued against a task from outside the app — today,
  the [mobile companion](12-the-mobile-companion-and-events.md) — is drained by
  the app and delivered to an active run. A live, stdin-capable agent receives it
  immediately; a finished run starts a new attempt with it.
- **Send selection from Notes.** You can send an exact editor selection from a
  note to a run as context ([Chapter 7](07-notes-and-knowledge.md)).

The through-line: an agent's worktree is durable, so you can keep handing it
input across attempts without losing state.

## Timeouts and recovery

A stuck agent should not run forever. `run_timeout_minutes` stops any run that
exceeds it (default 60; 0 disables the timeout):

```jsonc
{ "run_timeout_minutes": 60 }
```

If the app itself is interrupted, runs are recovered on restart and their
persisted output is intact. Live ptys, however, cannot survive a process exit —
so a run that was mid-flight is recovered as a record with its transcript, not as
a resumed live terminal.

## Semantic states in depth

A run's **status** tells you the process is alive. It does not tell you that
agent #3 of five is *blocked waiting for your answer* while the others churn. On
a fan-out board that "which one needs me" signal is the missing piece, so Asylum
classifies each run's live transcript into one of four **activities**:

- **blocked** — stopped at an input prompt (a `(y/n)`, a selection menu, a
  password). The most actionable state.
- **working** — actively thinking, editing, or running a command.
- **done** — printed a completion marker; awaiting review.
- **idle** — initialized but showing no recognizable signal.

### How activity is detected

Detection is a pure function over a snapshot of recent output. Asylum strips ANSI
escape codes from the transcript, lowercases it, and matches **substring
markers** case-insensitively against the last several non-empty lines. It uses a
generic marker set that covers most CLI agents, plus per-agent additions for
agents like `claude-code`, `codex`, `aider`, and `gemini`.

The precedence is deliberate, because a live input prompt is the most useful
thing to surface:

1. **blocked** is checked first, in a *tight* window of the last few lines — so
   an old `(y/n)` scrolled up in history does not read as a live prompt.
2. **done** is checked next, also near the tail of the turn.
3. **working** is checked last, over a slightly wider recent window.

Markers include things like `(y/n)`, `press enter`, `do you want`, `password:`,
and an `❯` prompt caret for **blocked**; `done`, `completed`, `✓`, `committed`,
and `no changes` for **done**; and `thinking`, `editing`, `running`, `compiling`,
and spinner glyphs for **working**. If nothing matches, the prior state stays.

Because it is just substrings on stripped text, detection is cheap and ages well
as agents change their output. An agent can also **self-report** its activity
over the [control surface](11-agent-orchestration-and-the-control-surface.md) —
`asylum control activity blocked` — which is authoritative when the agent knows
its own state better than the classifier.

Every activity change is recorded as a `run_activity`
[event](12-the-mobile-companion-and-events.md), so the board, your phone, and
other agents all see the same live picture.

## Try it

1. Fan a task out to more agents than your `max_parallel_runs` and watch the
   extras sit `queued`, then launch as capacity frees up.
2. Lower `run_timeout_minutes` to `1` on a deliberately long task and watch the
   timeout stop it.
3. Ship a diff annotation back to a finished run and watch the attempt count
   increment in the same worktree.

## Recap

- Fan-out is plan → allocate worktree/branch → launch on a pty → track.
- `max_parallel_runs` bounds concurrency; extras queue and launch as capacity
  frees.
- Retries, review continuation, and follow-ups feed a durable worktree more
  input across attempts; `run_timeout_minutes` stops stuck runs.
- Activity (working / blocked / done / idle) is classified from output by
  substring markers with blocked-first precedence, and can be self-reported.

## Next

[Chapter 5: Layouts and Presets](05-layouts-and-presets.md) turns "which agents,
how many at once" into named, reusable presets.
