# Episode 03 — Your First Fan-Out

**Duration:** ~6 min · **Level:** Beginner
**You'll learn:** how to compose a task prompt, fan it out to a few agents, and watch the runs spin up in isolated worktrees.
**Prerequisites:** [Episode 02](02-open-a-project-and-pick-agents.md) — a project open with agents chosen.

## Shot list

| Time | On screen | Action | Narration |
|------|-----------|--------|-----------|
| 0:00 | The Tasks surface (the fan-out board). | Land on Tasks. | "This is the Tasks surface — the fan-out board, and the home screen of Asylum. Everything starts here." |
| 0:16 | New-task composer. | Create a new task. | "Create a new task. A task is a title and a prompt against the current project. Right now it's just a draft — nothing has run." |
| 0:34 | Prompt field. | Type: "Add a `--version` flag to the CLI that prints the package version and exits 0." | "Make your first prompt small and verifiable. 'Add a version flag to the CLI that prints the version and exits zero.' A clear success criterion means you can actually score the result later." |
| 0:58 | Agent picker on the composer. | Select `claude-code` and `codex`. | "Pick the agents to race. Two is plenty for a first task — claude-code and codex." |
| 1:18 | Dispatch button. | Click to fan out. | "Now dispatch. This is the fan-out — and here's exactly what Asylum does." |
| 1:34 | Diagram: branch + worktree per agent. | Overlay the three-step allocation. | "One: it allocates a branch and a worktree per agent. Two: it creates each worktree on its branch, an isolated checkout on disk. Three: it records one run per agent and launches that agent in its own terminal pane." |
| 2:00 | Two run cards appear on the board. | Cards render with agent name + branch chip. | "Two runs, racing the same prompt in complete isolation. Neither can see or clobber the other's files." |
| 2:24 | A run card in detail. | Zoom one card. | "Each card shows the agent, a status badge, the branch it's on, elapsed time, and a live terminal you can open." |
| 2:46 | Status badge cycling queued → running. | Watch the badge change. | "Two different signals tell you what's happening. First, status — the lifecycle of the process. Queued means the worktree's allocated but the agent hasn't started. Running means it's live." |
| 3:10 | Status legend overlay. | Show succeeded / failed / cancelled. | "When it ends: succeeded if it exited cleanly, failed if it exited non-zero, cancelled if you stopped it." |
| 3:32 | Activity chip on a running card. | Highlight the colored chip. | "The second signal is activity — what the agent is doing right now, shown as a colored chip. We'll spend the whole next episode on this, because it's how you know which agent needs you." |
| 3:56 | Open a run's live terminal pane. | Click into a running pane. | "Click a run to watch its live terminal. This is a real pane streaming the agent's output — the same engine as the standalone terminal." |
| 4:22 | Board with both runs progressing. | Return to the board. | "As each pty produces output, Asylum snapshots the transcript into its store, so it survives even after the process is gone." |
| 4:46 | The task's own status label. | Point at the task status. | "The task itself moves through states too — draft, then running as its runs work, then review when they're ready, and merged once you pick a winner." |
| 5:10 | Both runs reach a terminal state. | Let them finish. | "Give them a moment. When a run succeeds, Asylum commits its changes and kicks off that worktree's checks automatically." |
| 5:36 | Board with two finished runs. | Hold. | "Two attempts at one prompt, side by side, ready to compare. That's a fan-out." |
| 5:52 | End card. | Fade. | "Next: reading the activity chips to spot the agent that needs you." |

## B-roll / capture notes
- Use the `--version` prompt from the script against your throwaway repo so it's quick and genuinely verifiable.
- Fan out to exactly two agents you have installed so both runs actually launch on camera.
- Capture the moment both cards appear and the status badges tick queued → running.
- If a run finishes very fast, that's fine — you'll re-use these finished runs in episodes 04, 05, and 06.

## Recap card (end screen)
- A task is a title + prompt; fanning out creates one run per agent, each in its own worktree.
- Fan-out = allocate branch/worktree → create worktree → record run → launch agent.
- **Status** (queued → running → succeeded/failed/cancelled) tracks the process.
- **Activity** chips tell you what an agent is doing right now — next episode.

## Next
- [Episode 04 — Reading Semantic States](04-reading-semantic-states.md)

Go deeper: [book chapter 3](../book/03-your-first-task.md) and [chapter 4](../book/04-the-fleet-in-depth.md).
