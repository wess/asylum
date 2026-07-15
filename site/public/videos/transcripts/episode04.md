# Run your first fan-out

Beginner · Episode 4

## 1. The Tasks surface (the fan-out board).

This is the Tasks surface — the fan-out board, and the home screen of Asylum. Everything starts here.

## 2. New-task composer.

Create a new task. A task is a title and a prompt against the current project. Right now it's just a draft — nothing has run.

## 3. Prompt field.

Make your first prompt small and verifiable. 'Add a version flag to the CLI that prints the version and exits zero.' A clear success criterion means you can actually score the result later.

## 4. Agent picker on the composer.

Pick the agents to race. Two is plenty for a first task — claude-code and codex.

## 5. Dispatch button.

Now dispatch. This is the fan-out — and here's exactly what Asylum does.

## 6. Diagram: branch + worktree per agent.

One: it allocates a branch and a worktree per agent. Two: it creates each worktree on its branch, an isolated checkout on disk. Three: it records one run per agent and launches that agent in its own terminal pane.

## 7. Two run cards appear on the board.

Two runs, racing the same prompt in complete isolation. Neither can see or clobber the other's files.

## 8. A run card in detail.

Each card shows the agent, a status badge, the branch it's on, elapsed time, and a live terminal you can open.

## 9. Status badge cycling queued → running.

Two different signals tell you what's happening. First, status — the lifecycle of the process. Queued means the worktree's allocated but the agent hasn't started. Running means it's live.

## 10. Status legend overlay.

When it ends: succeeded if it exited cleanly, failed if it exited non-zero, cancelled if you stopped it.

## 11. Activity chip on a running card.

The second signal is activity — what the agent is doing right now, shown as a colored chip. We'll spend the whole next episode on this, because it's how you know which agent needs you.

## 12. Open a run's live terminal pane.

Click a run to watch its live terminal. This is a real pane streaming the agent's output — the same engine as the standalone terminal.

## 13. Board with both runs progressing.

As each pty produces output, Asylum snapshots the transcript into its store, so it survives even after the process is gone.

## 14. The task's own status label.

The task itself moves through states too — draft, then running as its runs work, then review when they're ready, and merged once you pick a winner.

## 15. Both runs reach a terminal state.

Give them a moment. When a run succeeds, Asylum commits its changes and kicks off that worktree's checks automatically.

## 16. Board with two finished runs.

Two attempts at one prompt, side by side, ready to compare. That's a fan-out.

## 17. End card.

Next: reading the activity chips to spot the agent that needs you.
