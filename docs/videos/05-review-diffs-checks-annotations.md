# Episode 05 — Review: Diffs, Checks, Annotations

**Duration:** ~7 min · **Level:** Intermediate
**You'll learn:** how to read a run's diff, run the project's PASS/FAIL checks, and click a line to leave comments that ship back to the agent for another attempt.
**Prerequisites:** [Episode 03](03-your-first-fanout.md) — at least one finished run to review.

## Shot list

| Time | On screen | Action | Narration |
|------|-----------|--------|-----------|
| 0:00 | Board with two finished runs. | Select a run. | "Fanning out is half the value. The other half is choosing well — and that happens on the Review surface." |
| 0:16 | The Review surface opens for the run. | Open Review. | "Select a run and open Review. A diff is this run's changes against the project's base branch, parsed into a reviewable tree: files, then hunks, then lines — added, removed, or context." |
| 0:40 | Unified view of a changed file. | Scroll the diff. | "Old and new line numbers are tracked on every line, so a comment can anchor to exactly the right side." |
| 1:00 | Toggle to side-by-side view. | Switch views. | "There are two views — unified, changes inline in one column, and side-by-side, old on the left and new on the right. Use whichever reads better." |
| 1:20 | Branch chips on the diff header. | Point at the branch chip. | "Branch chips show which branch the run is on. Because every run has its own branch, you review and compare each agent's attempt independently." |
| 1:42 | Reviewing the actual change. | Read the diff critically. | "Ask the reviewer's questions: did it change the right files, is the change complete, did it touch anything it shouldn't?" |
| 2:04 | Checks panel with running spinners. | Trigger the checks. | "But a diff that reads well can still be broken. Run the checks — Asylum's real verification, in the run's own worktree." |
| 2:26 | Ecosystem detection overlay. | Show detected commands. | "You don't configure these per project. Asylum detects them from the project's shape." |
| 2:44 | Rust checks: cargo check/clippy/test. | Show Rust results. | "Rust runs cargo check, clippy, and test. JavaScript and TypeScript run your package manager's type-check, lint, and test — chosen by lockfile, defaulting to Bun." |
| 3:06 | Python + Go check rows. | Show the other ecosystems. | "Python runs ruff and pytest. Go runs go build, vet, and test. Each reports PASS, FAIL, or skipped when it doesn't apply, with a short summary and a timing." |
| 3:30 | One run PASS, the other FAIL. | Compare the two runs' checks. | "Now you're comparing on correctness, not just on how the diff looks. Checks run automatically after a successful run finishes, and you can re-run them here anytime." |
| 3:56 | Click a specific line in the diff. | Click a line; a comment box opens. | "Say one run's diff is close, but a single line is wrong. Click that line and attach a comment. That's an annotation — anchored to a specific file, line, and side." |
| 4:24 | Several annotations listed. | Add two or three. | "Leave as many as you need. They're durable — they survive an app restart — so you can review across sessions. You can resolve or delete them as the run addresses each one." |
| 4:52 | The "ship back to agent" action. | Send the annotations. | "Here's the powerful part. Collected annotations get sent back to the agent as feedback — and sending them starts a fresh attempt in the same worktree." |
| 5:18 | Same run card, attempt count increments. | Watch the new attempt stream. | "The agent iterates on its own changes with your line-level comments in hand. It doesn't start over and it doesn't lose context. The worktree's attempt count ticks up and the new output streams into the same card." |
| 5:44 | Loop diagram: fan out → check → annotate → ship → re-check. | Show the review loop. | "That turns the board into a review-driven loop. Fan out, review, run checks, annotate the promising runs, ship the feedback, and repeat — steering several agents at once with precise, line-anchored feedback." |
| 6:14 | Re-run checks on the new attempt. | Re-check; it passes. | "Re-check the new attempt. When the fix lands and checks go green, this run is looking like the winner." |
| 6:40 | Both runs, one clearly ahead. | Hold. | "Diffs read, checks run, one run corrected in place. Next: merging the winner safely." |

## B-roll / capture notes
- Set up two finished runs where one has a clean PASS and the other has a fixable FAIL — that contrast is the whole episode.
- Pick a repo whose ecosystem you can show checks for (a Rust or JS repo is easiest to make PASS/FAIL on demand).
- Capture the annotation flow slowly: click line → type comment → show it anchored → ship back → attempt count increments.
- If ship-back re-launches quickly, keep rolling to catch the new attempt streaming into the same card.

## Recap card (end screen)
- Diff parses changes into files → hunks → lines; unified or side-by-side, per branch.
- Checks run the project's real type-check/lint/test by ecosystem: PASS / FAIL / skipped.
- Annotations are durable, line-anchored comments.
- Shipping annotations back starts a new attempt in the same worktree — review is a conversation.

## Next
- [Episode 06 — Merge the Winner](06-merge-the-winner.md)

Go deeper: [book chapter 6](../book/06-diffs-checks-and-review.md).
