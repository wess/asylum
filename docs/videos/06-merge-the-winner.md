# Episode 06 — Merge the Winner

**Duration:** ~5 min · **Level:** Intermediate
**You'll learn:** how to pick a winning run and merge it behind Asylum's guarded preflight — or open a pull request from it instead.
**Prerequisites:** [Episode 05](05-review-diffs-checks-annotations.md) — a reviewed run whose checks pass.

## Shot list

| Time | On screen | Action | Narration |
|------|-----------|--------|-----------|
| 0:00 | Two finished runs, one with green checks. | Land on the board. | "You've reviewed the diffs and run the checks. One run is clearly best. Now you make it the winner and merge." |
| 0:20 | Select the winning run; merge action. | Click merge. | "Select the run whose diff is right and whose checks pass, and merge. But Asylum's merge is deliberately careful — it won't let you land broken or conflicting work by accident. Watch the guards fire in order." |
| 0:44 | Guard 1 overlay: failed-check blocking. | Show a blocked-merge message on a failing run. | "One: failed-check blocking. If the run's checks failed, the merge is simply blocked. The arbiter is the checks, not your gut." |
| 1:08 | Guard 2 overlay: dirty-base protection. | Show the base-clean verification. | "Two: dirty-base protection. It verifies the base worktree is clean before it touches it, so it never merges onto uncommitted changes." |
| 1:34 | Guard 3 overlay: conflict preflight. | Show the non-destructive conflict check. | "Three: a conflict preflight. It runs a non-destructive check for merge conflicts first — so a conflict gets reported, not left half-applied in your tree." |
| 2:02 | Guard 4: confirmation dialog. | The confirm prompt appears. | "Four: explicit confirmation. It asks you to confirm before it merges the run's branch back to the base branch. Nothing merges behind your back." |
| 2:28 | Confirm; merge completes. | Click confirm. | "Confirm, and the winning branch merges into base." |
| 2:48 | Worktrees cleaning up; branches kept. | Show the worktree list shrinking. | "Then cleanup. Asylum removes the clean, finished worktrees but keeps their branches — so your workspace stays tidy and no work is ever lost." |
| 3:14 | Task status flips to `merged`. | Point at the task status. | "And the task's status moves to merged. That's the full loop closed — one prompt, several attempts, the best one landed safely." |
| 3:38 | Split: local merge vs. pull request. | Show the two paths. | "There's a second way to land a winner. Instead of merging locally, you can open a pull request from the winning run's branch." |
| 4:02 | Integrations surface, create-PR action. | Gesture toward Integrations. | "That opens the change on GitHub for review and CI — best for team work or anything that needs a second pair of eyes. Same starting point: a selected run whose diff is right and whose checks pass. We'll do that in the integrations episode." |
| 4:28 | Back on a tidy board. | Hold. | "Local merge for speed, a PR for review. Either way, the guards and the checks keep you honest." |
| 4:50 | End card. | Fade. | "Next: layouts — racing a whole set of agents in one gesture." |

## B-roll / capture notes
- Have one run that PASSES (to merge) and, ideally, one that FAILS (to show the merge being blocked in guard 1).
- Capture the confirmation dialog clearly — it's the human-in-the-loop beat.
- Show the worktree folder (or `asylum worktree list`) before and after cleanup so viewers see the checkouts disappear but branches remain.
- Don't fully record the PR flow here — tease it and save the real capture for episode 09.

## Recap card (end screen)
- Merge is guarded: failed checks block, the base is protected, conflicts are pre-flighted, and confirmation is required.
- Cleanup removes clean worktrees but keeps branches; the task becomes `merged`.
- A PR from the winning run is the review-oriented alternative to a local merge.
- Both start from the same place: a run whose diff is right and whose checks pass.

## Next
- [Episode 07 — Layouts and Presets](07-layouts-and-presets.md)

Go deeper: [book chapter 6](../book/06-diffs-checks-and-review.md).
