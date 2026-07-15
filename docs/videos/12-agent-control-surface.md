# Episode 12 — The Agent Control Surface

**Duration:** ~9 min · **Level:** Advanced
**You'll learn:** how a running agent orchestrates its siblings — the env vars, the skill, `asylum control`, `asylum wait`, the queue/drain model, and a worked lead-spawns-test-writer demo.
**Prerequisites:** [Episode 11](11-the-cli-tour.md) — you know the `asylum control` commands.

## Shot list

| Time | On screen | Action | Narration |
|------|-----------|--------|-----------|
| 0:00 | Diagram: you orchestrating the fleet vs. an agent doing it. | Show the shift. | "So far, you have orchestrated the fleet — composing tasks, fanning out, reviewing. This is the flagship feature: letting a running agent do the same from inside its own worktree. A lead agent that grows a small team around a task." |
| 0:26 | Architecture sketch: local control server + store. | Draw the server. | "It's a small local HTTP server the app runs. A running agent calls it to coordinate the fleet — without breaking isolation. Reads answer straight from the shared store; writes with real side effects get queued and performed by the app." |
| 0:56 | Settings `control` block. | Show the control config. | "It's configured under the control key — enabled, a bind address, and a token. Localhost-only by default, because it can spawn runs, so you keep it off the network." |
| 1:24 | Four env vars listed. | Reveal the injected vars. | "When the app launches an agent it injects four environment variables. ASYLUM_CONTROL_URL — the server's base URL. ASYLUM_TASK_ID — the task every sibling shares. ASYLUM_RUN_ID — this agent's own run. And ASYLUM_CONTROL_TOKEN — sent as a bearer token when one's set." |
| 1:58 | Highlight `ASYLUM_RUN_ID`. | Emphasize it. | "One of these is the test. The presence of ASYLUM_RUN_ID is how an agent knows it's inside an Asylum-managed worktree. If it's not set, the agent isn't in a managed pane and must not attempt any orchestration. That single check keeps it well-behaved when run standalone." |
| 2:32 | Terminal. | `asylum control skill`. | "How does an agent learn the API? Not a service call — a skill document. asylum control skill prints a Markdown instruction file you drop into the agent's rules or skills directory." |
| 3:00 | Scroll the printed skill. | Show the skill content. | "It explains the 'am I inside Asylum' check, lists the env vars, shows the commands, and lays out etiquette — report blocked before you stop to ask, done when you finish, and only spawn a helper when parallel work genuinely helps. Because it's just instructions, it works with any agent that reads a rules directory. No runtime to install." |
| 3:34 | Terminal inside a run. | `asylum control status`. | "Now the commands, run from inside a run's pane. status prints the task, marks your own run with a star, and lists every sibling with its agent, status, and live activity — the same 'who needs me' picture the board shows, in text an agent can read." |
| 4:04 | Terminal. | `asylum control read 15`. | "read pulls a sibling's recent transcript tail — for coordination, not copying." |
| 4:26 | Terminal. | `asylum control activity working`. | "activity self-reports your own semantic state — working, blocked, done, idle. The classifier guesses from output, but the agent knows itself best, so a self-report is authoritative and updates the board, your phone, and every sibling." |
| 4:54 | Terminal. | `asylum control spawn codex "write tests"`. | "spawn queues another agent on this same task — a helper run. And check queues a checks pass in your own worktree." |
| 5:20 | Queue/drain diagram. | Animate request row → app drains. | "Now the crucial model. Reads are immediate. But spawn and check need real git and pty work — a branch, a worktree, a launched process — so they're not done by the server. They're recorded as a control request row and returned immediately as 'queued.' The desktop app polls, does the work, and marks each processed." |
| 5:54 | Two consequences overlay. | List them. | "Two consequences. One: spawn and check are asynchronous — the command returns when the request is queued, not when the new run is live, so you follow up with status or wait. Two: the app must be running to drain the queue. activity, by contrast, is a direct store write, so it applies immediately." |
| 6:24 | Terminal split: lead pane + status. | Begin the worked demo. | "Let's watch it end to end. claude-code is the lead on a task — implement a JSON config parser with tests. Splitting genuinely helps: one agent writes the parser, another writes tests against the intended interface." |
| 6:50 | Lead pane. | `test -n "$ASYLUM_RUN_ID"` then `asylum control activity working`. | "Step one, confirm I'm inside Asylum — is ASYLUM_RUN_ID set. Step two, tell the board I'm working." |
| 7:14 | Lead pane. | `asylum control spawn codex "Write unit tests for parse_config... do not implement it"`. | "Step three, spawn a sibling to write tests against the interface I'm about to build — explicitly telling it not to implement the function itself." |
| 7:40 | Lead pane. | `asylum control status` shows run 15 appear. | "Step four, check what runs exist now, once the app has drained the spawn. There's the sibling — run fifteen, codex, running." |
| 8:04 | Board shows both runs; lead works. | Cut to the board. | "Step five, the lead implements the parser in its own worktree while the sibling writes tests in its own. Two isolated worktrees, one task." |
| 8:28 | Lead pane. | `asylum wait run 15 --status succeeded --timeout 300`. | "Step six, wait for the test-writer to finish before reconciling — wait, not poll. It returns the moment run fifteen succeeds." |
| 8:48 | Lead pane. | `asylum control read 15`, then `asylum control check` + `activity done`. | "Step seven, read what it produced to align interfaces — cite what you learned, don't copy. Step eight, run my own checks and report done. The lead grew a team, and never once violated isolation." |

## B-roll / capture notes
- You need the app running with `control.enabled` true, and an agent whose rules/skills directory you can edit (drop `asylum control skill` output into it beforehand).
- Run all `asylum control` commands from inside an actual run's pane so the injected env vars are present.
- The worked demo is the centerpiece: pre-script the eight steps so they flow, and capture the moment the spawned sibling (run 15 here) appears in `status` after the app drains the queue — call out that it's asynchronous.
- Use the exact run ids you get on camera; the "15" in the script is illustrative.

## Recap card (end screen)
- The control surface lets a running agent orchestrate the fleet from inside its worktree via a local HTTP server.
- `ASYLUM_RUN_ID` present = inside Asylum; also injected: `ASYLUM_CONTROL_URL`, `ASYLUM_TASK_ID`, `ASYLUM_CONTROL_TOKEN`.
- Agents learn the API from `asylum control skill` — a Markdown file, not a service.
- Reads are immediate; `spawn`/`check` are queued and drained by the app (asynchronous); `activity` applies immediately.

## Next
- [Episode 13 — Mobile Companion and Events](13-mobile-companion-and-events.md)

Go deeper: [book chapter 11](../book/11-agent-orchestration-and-the-control-surface.md).
