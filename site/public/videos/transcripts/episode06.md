# Diffs, checks, and annotations

Intermediate · Episode 6

## 1. Board with two finished runs.

Fanning out is half the value. The other half is choosing well — and that happens on the Diff surface.

## 2. The Diff surface opens for the run.

Select a run and open Diff. A diff is this run's changes against the project's base branch, parsed into a reviewable tree: files, then hunks, then lines — added, removed, or context.

## 3. Unified view of a changed file.

Old and new line numbers are tracked on every line, so a comment can anchor to exactly the right side.

## 4. Toggle to side-by-side view.

There are two views — unified, changes inline in one column, and side-by-side, old on the left and new on the right. Use whichever reads better.

## 5. Branch chips on the diff header.

Branch chips show which branch the run is on. Because every run has its own branch, you review and compare each agent's attempt independently.

## 6. Reviewing the actual change.

Ask the reviewer's questions: did it change the right files, is the change complete, did it touch anything it shouldn't?

## 7. Checks panel with running spinners.

But a diff that reads well can still be broken. Run the checks — Asylum's real verification, in the run's own worktree.

## 8. Ecosystem detection overlay.

You don't configure these per project. Asylum detects them from the project's shape.

## 9. Rust checks: cargo check/clippy/test.

Rust runs cargo check, clippy, and test. JavaScript and TypeScript run your package manager's type-check, lint, and test — chosen by lockfile, defaulting to Bun.

## 10. Python + Go check rows.

Python runs ruff and pytest. Go runs go build, vet, and test. Each reports PASS, FAIL, or skipped when it doesn't apply, with a short summary and a timing.

## 11. One run PASS, the other FAIL.

Now you're comparing on correctness, not just on how the diff looks. Checks run automatically after a successful run finishes, and you can re-run them here anytime.

## 12. Click a specific line in the diff.

Say one run's diff is close, but a single line is wrong. Click that line and attach a comment. That's an annotation — anchored to a specific file, line, and side.

## 13. Several annotations listed.

Leave as many as you need. They're durable — they survive an app restart — so you can review across sessions. You can resolve or delete them as the run addresses each one.

## 14. The "ship back to agent" action.

Here's the powerful part. Collected annotations get sent back to the agent as feedback — and sending them starts a fresh attempt in the same worktree.

## 15. Same run card, attempt count increments.

The agent iterates on its own changes with your line-level comments in hand. It doesn't start over and it doesn't lose context. The worktree's attempt count ticks up and the new output streams into the same card.

## 16. Loop diagram: fan out → check → annotate → ship → re-check.

That turns the board into a review-driven loop. Fan out, review, run checks, annotate the promising runs, ship the feedback, and repeat — steering several agents at once with precise, line-anchored feedback.

## 17. Re-run checks on the new attempt.

Re-check the new attempt. When the fix lands and checks go green, this run is looking like the winner.

## 18. Both runs, one clearly ahead.

Diffs read, checks run, one run corrected in place. Next: merging the winner safely.
