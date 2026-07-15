# Select and merge the winner

Intermediate · Episode 7

## 1. Two finished runs, one with green checks.

You've reviewed the diffs and run the checks. One run is clearly best. Now you make it the winner and merge.

## 2. Select the winning run; merge action.

Select the run whose diff is right and whose checks pass, and merge. But Asylum's merge is deliberately careful — it won't let you land broken or conflicting work by accident. Watch the guards fire in order.

## 3. Guard 1 overlay: failed-check blocking.

One: failed-check blocking. If the run's checks failed, the merge is simply blocked. The arbiter is the checks, not your gut.

## 4. Guard 2 overlay: dirty-base protection.

Two: dirty-base protection. It verifies the base worktree is clean before it touches it, so it never merges onto uncommitted changes.

## 5. Guard 3 overlay: conflict preflight.

Three: a conflict preflight. It runs a non-destructive check for merge conflicts first — so a conflict gets reported, not left half-applied in your tree.

## 6. Guard 4: confirmation dialog.

Four: explicit confirmation. It asks you to confirm before it merges the run's branch back to the base branch. Nothing merges behind your back.

## 7. Confirm; merge completes.

Confirm, and the winning branch merges into base.

## 8. Worktrees cleaning up; branches kept.

Then cleanup. Asylum removes the clean, finished worktrees but keeps their branches — so your workspace stays tidy and no work is ever lost.

## 9. Task status flips to merged.

And the task's status moves to merged. That's the full loop closed — one prompt, several attempts, the best one landed safely.

## 10. Split: local merge vs. pull request.

There's a second way to land a winner. Instead of merging locally, you can open a pull request from the winning run's branch.

## 11. Integrations surface, create-PR action.

That opens the change on GitHub for review and CI — best for team work or anything that needs a second pair of eyes. Same starting point: a selected run whose diff is right and whose checks pass. We'll do that in the integrations episode.

## 12. Back on a tidy board.

Local merge for speed, a PR for review. Either way, the guards and the checks keep you honest.

## 13. End card.

Next: layouts — racing a whole set of agents in one gesture.
