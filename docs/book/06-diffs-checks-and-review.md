# Chapter 6: Diffs, Checks, and Review

Fanning out is only half the value; the other half is *choosing well*. This
chapter covers the annotatable diff surface, the PASS/FAIL checks Asylum detects
by ecosystem, how inline annotations ship back to an agent to drive iteration,
and the safe path to merging a winner.

## Reading a diff

Select a run and open the **Review** surface. A diff is the run's changes against
the project's base branch, parsed into a reviewable tree: each changed **file**
contains **hunks** (contiguous regions of change), and each hunk contains
**lines** — added, removed, or unchanged context. Old and new line numbers are
tracked on each line so a comment can anchor to the correct side.

Asylum offers both a **unified** view (changes inline, one column) and a
**side-by-side** view (old on the left, new on the right). Branch chips show
which branch the run is on. Because every run is on its own branch, you review
and compare each agent's attempt independently — open one run's diff, form an
opinion, switch to the next.

## Staging what you want

A successful run's changes are deliberately left **uncommitted** in its
worktree — nothing lands on the run's branch until you merge. That is what
lets you stage exactly what you want: click a hunk to stage or unstage it, or
stage a whole file at once, and a counter ("N of M hunks staged") tracks how
much of the run you have accepted so far. Staging state is read live from Git,
not from a shadow copy, so it always matches what is actually on disk. Whatever
is staged when you merge is what gets committed to the branch.

## Checks: PASS or FAIL

A diff that reads well can still be broken. **Checks** run the project's real
verification in the run's worktree and classify each as PASS, FAIL, or skipped.
Asylum detects the right checks from the project's shape — you do not configure
them per project:

- **Rust** (`Cargo.toml`): `cargo check`, `cargo clippy`, `cargo test`.
- **JavaScript/TypeScript**: the package manager's type-check, lint, and test.
  The manager is chosen by lockfile — `pnpm-lock.yaml` → pnpm, `yarn.lock` →
  yarn, `package-lock.json` → npm, otherwise **bun** by default.
- **Python**: `ruff check .` (lint) and `pytest -q` (test).
- **Go**: `go build ./...`, `go vet ./...`, `go test ./...`.

Each check reports a status and a short summary, and how long it took. Checks run
automatically after a successful run finishes, and you can re-run them from the
Review surface. A run's health is shown with PASS/FAIL indicators on its card and
in review, so you can compare two agents' work on *correctness*, not just on how
the diff looks.

An agent working inside the fleet can also request a checks pass on its own
worktree over the [control surface](11-agent-orchestration-and-the-control-surface.md)
with `asylum control check`.

## Annotating a line

Review in Asylum is a conversation with the agent, not a verdict. When you spot
something — a wrong branch of logic, a missing guard, a naming nit — click that
line in the diff and attach a comment. That is an **annotation**. It is anchored
to a specific file, line, and side (old or new), and it is durable: annotations
survive an app restart, so you can review across sessions.

Leave as many as you need. You can resolve or delete them as the run addresses
each. The set of open annotations is your review of that run.

## Shipping annotations back to an agent

Here is the part that makes review powerful. Collected annotations are **sent
back to the agent as feedback**, and sending them starts a fresh attempt in the
*same worktree*. The agent iterates on its own changes with your line-level
comments in hand — it does not start from scratch, and it does not lose its
context. The worktree's attempt count increments, and the new attempt's output
streams into the same run card.

This turns the fan-out board into a review-driven loop:

1. Fan out to several agents.
2. Review each diff; run checks.
3. On the promising runs, annotate the specific lines you want changed.
4. Ship the annotations back; the agents revise in place.
5. Re-check, and repeat until one run is clearly the winner.

You are steering several agents with precise, line-anchored feedback at once.

## Choosing and merging a winner

When one run's diff is right and its checks pass, make it the winner and merge.
Asylum's merge is deliberately careful — it will not let you merge broken or
conflicting work by accident:

1. **Failed-check blocking.** If the run's checks failed, the merge is blocked.
2. **Dirty-base protection.** It verifies the base worktree is clean before
   touching it.
3. **Conflict preflight.** It runs a *non-destructive* check for merge conflicts
   first, so a conflict is reported rather than left half-applied.
4. **Explicit confirmation.** It asks you to confirm before it merges the run's
   branch back to the base branch — as a regular merge, or a **squash merge**
   that collapses the run's commits into one. This is also the moment your
   staged changes are actually committed onto the branch.
5. **Cleanup.** When you are ready, **Clean up finished worktrees** removes
   clean, finished worktrees and deletes any branch that is now safely merged —
   Git refuses to delete anything that is not, so a losing run's branch is
   always left alone.

The task's status moves to `merged`. If you would rather land the change through
code review, open a **pull request** from the winning run instead — see
[Chapter 8](08-integrations.md).

## Try it

1. Fan a task out to two agents and let both finish.
2. Run checks on each; note which pass.
3. On the run with the nicer diff but a failing check, annotate the offending
   line with a specific instruction and ship it back.
4. Re-run checks on the new attempt; when it passes, unstage any hunk you do not
   want, merge it, and clean up the finished worktrees.

## Recap

- The Review surface parses changes into files → hunks → lines, in unified or
  side-by-side view, per branch.
- A successful run stays uncommitted in its worktree so you can stage or
  unstage individual hunks or files before merge.
- Checks run the project's real type-check/lint/test by ecosystem and report
  PASS/FAIL.
- Annotations are durable, line-anchored comments that ship back to the agent and
  start a new attempt in the same worktree.
- Merge is guarded: failed checks block, the base is protected, conflicts are
  pre-flighted, and confirmation is required — as a regular or squash merge.
  Cleanup afterward deletes only branches that are now safely merged.

## Next

[Chapter 7: Notes and Knowledge](07-notes-and-knowledge.md) shows how project
memory feeds agents better context than a prompt alone.
