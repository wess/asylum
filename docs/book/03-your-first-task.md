# Chapter 3: Your First Task

This is the chapter where Asylum clicks. You will compose a real prompt, fan it
out to two agents, watch both runs, read the activity chips that tell you which
agent needs you, review a diff, run the project's checks, leave an inline
comment, and merge the winner. By the end you will have done the whole core loop
once.

## Step 1: Compose a task

Open the **Tasks** surface. This is the fan-out board — the home screen of
Asylum. Create a new task and give it a prompt. A good first prompt is small and
verifiable, for example:

> Add a `--version` flag to the CLI that prints the package version and exits 0.

A **task** is exactly this: a title and a prompt against the current project. It
has not run anything yet; it is a *draft*.

## Step 2: Choose agents and fan out

Pick the agents you want to race. For your first task, two is plenty — say
`claude-code` and `codex`. When you dispatch the task, Asylum **fans it out**:

- It allocates one **branch** and one **worktree** per agent.
- It creates each worktree on its branch (an isolated checkout on disk).
- It records one **run** per agent and launches that agent inside its own
  terminal pane.

You now have two runs racing the same prompt in complete isolation. Neither can
see or clobber the other's files.

## Step 3: Watch the runs

Each run shows as a card on the board with the agent's name, a status badge, the
branch it is working on, elapsed time, and a live terminal you can open. Two
different signals tell you what is happening:

**Status** is the lifecycle of the *process*:

- `queued` — worktree allocated, agent not started yet.
- `running` — the agent process is live.
- `succeeded` — the agent exited cleanly (code 0).
- `failed` — the agent exited non-zero.
- `cancelled` — you stopped it.

**Activity** is the live semantic state — what the agent is *doing right now* —
shown as a colored chip while the run is running:

- **working** (blue) — thinking, editing, or running a command.
- **blocked** (orange) — stopped at a prompt, waiting for your input. This is the
  one that matters most: it is the "this agent needs me" signal.
- **done** (green) — printed a completion marker; ready for review.
- **idle** (gray) — started but not yet showing a recognizable signal.

Asylum classifies activity by reading each agent's terminal output. When five
agents are racing and one goes **blocked**, you can see at a glance which one to
attend to instead of scanning five terminals. (Activity is covered in depth in
[Chapter 4](04-the-fleet-in-depth.md).)

## Step 4: Review a diff

When a run finishes (or even while it works), select it and open the **Diff**
surface. A **diff** shows the run's changes line by line against the project's
base branch: added lines, removed lines, and context. Each run's work is on its
own branch, so you compare the two agents' diffs independently.

Read both. Ask: did the agent change the right files? Is the change complete? Did
it touch anything it should not have?

## Step 5: Run the checks

A correct-looking diff is not the same as a working one. On the Diff surface you
can run the project's **checks** — Asylum detects them from your project's shape:

- **Rust** (`Cargo.toml`): `cargo check`, `cargo clippy`, `cargo test`.
- **JavaScript/TypeScript**: your package manager's type-check, lint, and test
  (bun, npm, pnpm, or yarn, chosen by lockfile).
- **Python**: `ruff` and `pytest`.
- **Go**: `go build`, `go vet`, `go test`.

Each check runs in that run's worktree and reports **PASS** or **FAIL** (or is
skipped when not applicable). A run whose checks fail is a run you should not
merge — and Asylum will block the merge for you.

## Step 6: Annotate a line

Suppose one agent's diff is close but a single line is wrong. On the Diff
surface, click that line and attach a comment — this is an **annotation**. You
can leave several. Annotations are anchored to a specific line and side of the
diff, and they survive an app restart.

Collected annotations are **shipped back to the agent** as feedback: sending them
starts a fresh attempt *in the same worktree*, so the agent iterates on its own
work with your review in hand rather than starting over. Review becomes a
conversation. (Full detail in [Chapter 6](06-diffs-checks-and-review.md).)

## Step 7: Merge the winner

Once a run's diff looks right and its checks pass, make it the winner and merge.
Asylum runs a safe merge: it blocks if checks failed, verifies the base worktree
is clean, runs a non-destructive conflict preflight, and asks for explicit
confirmation before it merges the run's branch back to the base branch. Afterward
it cleans up the finished worktrees (keeping the branches) so your workspace does
not fill with stale checkouts.

If you would rather review the change on GitHub, you can open a **pull request**
from the winning run instead of merging locally
([Chapter 8](08-integrations.md)).

## Try it

Run the whole loop once on a throwaway repo:

1. Compose the `--version` task above.
2. Fan it out to `claude-code` and `codex`.
3. Watch the activity chips; open a terminal on whichever agent goes **blocked**.
4. Review both diffs, run checks on each.
5. Annotate one line on the weaker run and ship it back.
6. Merge the run whose checks pass.

## Recap

- A task is a prompt; fanning it out creates one run per agent, each in its own
  worktree.
- **Status** tracks the process; **activity** (working / blocked / done / idle)
  tells you which agent needs you now.
- Review diffs, run PASS/FAIL checks, annotate lines to iterate, and merge the
  winner behind a safe preflight.

## Next

[Chapter 4: The Fleet in Depth](04-the-fleet-in-depth.md) opens up fan-out,
worktrees, parallel limits, retries, timeouts, and exactly how the semantic
states are detected.
