# Expert orchestration and security

Expert · Episode 10

## 1. Diagram: you orchestrating the fleet vs. an agent doing it.

So far, you have orchestrated the fleet — composing tasks, fanning out, reviewing. This is the flagship feature: letting a running agent do the same from inside its own worktree. A lead agent that grows a small team around a task.

## 2. Architecture sketch: local control server + store.

It's a small local HTTP server the app runs. A running agent calls it to coordinate the fleet — without breaking isolation. Reads answer straight from the shared store; writes with real side effects get queued and performed by the app.

## 3. Settings control block.

It's configured under the control key — enabled, a bind address, and a token. Localhost-only by default, because it can spawn runs, so you keep it off the network.

## 4. Four env vars listed.

When the app launches an agent it injects four environment variables. ASYLUM_CONTROL_URL — the server's base URL. ASYLUM_TASK_ID — the task every sibling shares. ASYLUM_RUN_ID — this agent's own run. And ASYLUM_CONTROL_TOKEN — sent as a bearer token when one's set.

## 5. Highlight ASYLUM_RUN_ID.

One of these is the test. The presence of ASYLUM_RUN_ID is how an agent knows it's inside an Asylum-managed worktree. If it's not set, the agent isn't in a managed pane and must not attempt any orchestration. That single check keeps it well-behaved when run standalone.

## 6. Terminal.

How does an agent learn the API? Not a service call — a skill document. asylum control skill prints a Markdown instruction file you drop into the agent's rules or skills directory.

## 7. Scroll the printed skill.

It explains the 'am I inside Asylum' check, lists the env vars, shows the commands, and lays out etiquette — report blocked before you stop to ask, done when you finish, and only spawn a helper when parallel work genuinely helps. Because it's just instructions, it works with any agent that reads a rules directory. No runtime to install.

## 8. Terminal inside a run.

Now the commands, run from inside a run's pane. status prints the task, marks your own run with a star, and lists every sibling with its agent, status, and live activity — the same 'who needs me' picture the board shows, in text an agent can read.

## 9. Terminal.

read pulls a sibling's recent transcript tail — for coordination, not copying.

## 10. Terminal.

activity self-reports your own semantic state — working, blocked, done, idle. The classifier guesses from output, but the agent knows itself best, so a self-report is authoritative and updates the board, your phone, and every sibling.

## 11. Terminal.

spawn queues another agent on this same task — a helper run. And check queues a checks pass in your own worktree.

## 12. Queue/drain diagram.

Now the crucial model. Reads are immediate. But spawn and check need real git and pty work — a branch, a worktree, a launched process — so they're not done by the server. They're recorded as a control request row and returned immediately as 'queued.' The desktop app polls, does the work, and marks each processed.

## 13. Two consequences overlay.

Two consequences. One: spawn and check are asynchronous — the command returns when the request is queued, not when the new run is live, so you follow up with status or wait. Two: the app must be running to drain the queue. activity, by contrast, is a direct store write, so it applies immediately.

## 14. Terminal split: lead pane + status.

Let's watch it end to end. claude-code is the lead on a task — implement a JSON config parser with tests. Splitting genuinely helps: one agent writes the parser, another writes tests against the intended interface.

## 15. Lead pane.

Step one, confirm I'm inside Asylum — is ASYLUM_RUN_ID set. Step two, tell the board I'm working.

## 16. Lead pane.

Step three, spawn a sibling to write tests against the interface I'm about to build — explicitly telling it not to implement the function itself.

## 17. Lead pane.

Step four, check what runs exist now, once the app has drained the spawn. There's the sibling — run fifteen, codex, running.

## 18. Board shows both runs; lead works.

Step five, the lead implements the parser in its own worktree while the sibling writes tests in its own. Two isolated worktrees, one task.

## 19. Lead pane.

Step six, wait for the test-writer to finish before reconciling — wait, not poll. It returns the moment run fifteen succeeds.

## 20. Lead pane.

Step seven, read what it produced to align interfaces — cite what you learned, don't copy. Step eight, run my own checks and report done. The lead grew a team, and never once violated isolation.

## 21. Montage of the board, diff, and control panes.

You know every surface, the CLI, the control surface, events, and plugins. This last episode is about judgment — combining them into workflows that get real work done.

## 22. A swarm layout fanning out.

The default mode of Asylum is a tournament: one prompt across several agents, pick the winner. A few habits make it pay off.

## 23. Layout picker: quick / duel / swarm.

Match the layout to the stakes. A one-agent quick layout for routine changes, duel for anything with a real decision in it, and a wide swarm — with a modest concurrency — for hard, ambiguous problems where you don't yet know which agent will find the good approach.

## 24. Two diffs with checks, one green.

Judge on checks, not vibes. Two diffs can both look plausible; the one whose test suite passes is the one to trust. Run checks on every finalist before you compare style.

## 25. A crisp, verifiable prompt on screen.

Keep the prompt verifiable. A task with a clear success criterion produces a tournament you can actually score. Vague tasks produce diffs you can't rank.

## 26. Concurrency cap overlay.

And cap concurrency to your quota. Five agents at once burns CPU and API limits; a swarm with concurrency three gives you breadth without the cost spike. max-parallel-runs is your global backstop.

## 27. The review loop diagram.

The strongest workflow isn't 'fan out once and merge.' It's a loop. Fan out to a handful of agents. Run checks; discard the runs that fail outright.

## 28. Annotating finalists in parallel.

On the survivors, annotate the exact lines you want changed and ship the annotations back. Each iterates in its own worktree, in parallel, with your line-level feedback. Re-check, and repeat until one run is clearly best.

## 29. Two runs converging over attempts.

You're steering several agents at once with precise feedback, and worktrees are durable across attempts, so nothing is lost between rounds. This beats both trusting one agent and starting over.

## 30. Lead agent spawning helpers (control pane).

The expert-level move is letting an agent grow its own team through the control surface. A lead agent can split work that genuinely parallelizes — spawn a test-writer while it implements, a docs sibling while it refactors — each helper isolated in its own worktree.

## 31. Reads + waits between siblings.

It coordinates through reads and waits — read a sibling to align on an interface, wait on it instead of polling — and reports state honestly, blocked before it pauses, done when it finishes.

## 32. Reminder overlay: queued/drained, app running.

To enable this well: seed each agent's rules directory with the control skill, keep control enabled and localhost-only, and remember spawns and checks are queued and drained by the app — so the desktop must be running for a sub-fleet to come alive. Treat it as a power tool: it multiplies both output and cost.

## 33. Phone watching a long run's blocked chip.

For long tournaments, stay in the loop from anywhere. Bind the companion to the LAN with a token and watch the blocked activity from your phone; send a follow-up to nudge a live run; or tail the event stream and react to run_finished instead of polling.

## 34. Plugin trigger on task_merged.

And plugins turn a merge into the first step of a pipeline. A trigger on run_finished or task_merged can notify a channel or kick a deploy; a tool exposes your systems to the agents themselves — and for anything shared or untrusted, prefer WASM, where capabilities are enforced.

## 35. Best-practices card.

The condensed opinions: small verifiable tasks over big vague ones. Checks are the arbiter — never merge on looks. Iterate in place with annotations. Right-size concurrency. Keep control and companion secured. Let agents self-report state. Reserve sub-fleets for work that genuinely parallelizes. And put durable context in notes, attached.

## 36. End card over the table of contents.

That's the whole ADE — from what a worktree is to a lead agent commanding a sub-fleet you follow from your phone. Now go run a tournament.
