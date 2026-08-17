# Asylum: Start Here

An inclusive first-launch tour from the basic mental model through safe review and merge.

## Start here: Asylum, without the jargon

Welcome to Asylum. We will start with the five-year-old version, then build the real workflow one step at a time. You do not need to memorize every screen, and you do not need to be a Git expert. By the end, you will know what to open, what to ask, what to watch, how to check the result, and when it is safe to keep it.

## Explain like I am 5: One job. Three tables.

Imagine you ask three people to build the same little bridge. If all three reach into one pile of blocks, their hands collide and nobody knows who changed what. So you give each person their own table, their own copy of the blocks, and the same instruction card. When they finish, you compare the bridges and keep the strongest one. Asylum does that with coding agents and project files.

## Explain like I am 5: Project. Task. Run.

Asylum uses three important words. A project is the box of building blocks, which is your code repository. A task is the instruction card, written in ordinary language. A run is one agent's attempt, at its own table. Give one task to three agents and you get three separate runs. If you remember project, task, and run, the rest of the app has somewhere to land.

## Explain like I am 5: Ask. Race. Compare. Keep.

The whole product is one loop. Open a project. Describe one outcome. Let one or more agents work in isolation. Compare the evidence. Then merge the winner, or send feedback and let a promising run try again. Everything else in Asylum helps you see that loop clearly, make it safer, or repeat it without busywork.

## Your pace: Start simple. Grow when ready.

If this is your first coding tool, follow the blue path and use one agent for your first task. Asylum explains branches and worktrees in tooltips, and every important change waits for your confirmation. If you already know Git, the exact branch, worktree, terminal output, checks, and diff are all visible. Beginners and experienced developers use the same workflow at different depths.

## First launch: Three things to be ready

Install the packaged app for your operating system, make sure Git is available, and install at least one supported coding-agent command-line tool that you can already sign in to. On first launch, Asylum's setup doctor checks the repository, base branch, worktree support, agent executables, authentication verification, and the project toolchain. Installed means the command was found. Verified means it completed a real run.

## First launch: Open a project safely

Click Open project and choose a code repository. Opening it does not modify the repository. If you choose a plain folder, Asylum asks before initializing Git and creates only an empty first commit. A repository can declare setup commands and environment values, so Asylum withholds those until you explicitly trust that project. Trust can be revoked, and checks are gated by the same rule.

## Your first task: The Tasks board is home

The Tasks board is the home screen. The left side keeps projects and tasks in view. The composer at the top is where you describe new work. Below it, run cards show every attempt without hiding its branch, private worktree, output, checks, or elapsed time. Review is the next major stop. Search, Notes, and the other tools stay available without interrupting the task.

## Your first task: Write a result you can verify

A strong first task is small and testable. Say what should be true, name any constraint that matters, and include an acceptance check. For example: replace generic validation failures with field-specific messages, add tests for empty names and malformed email addresses, and keep the public API stable. The built-in templates help shape a bug fix, feature, test, refactor, review, or design request.

## Your first task: One agent first. Fan out on purpose.

Start with one ready agent while learning the loop. Use two or more when independent approaches are valuable. The duel, triad, and swarm layouts are reusable agent selections; swarm also limits how many start at once. Your global parallel limit protects the machine and your quota. Extra runs wait in the queue and start as capacity becomes available. More agents are useful only when comparison is worth the cost.

## What isolation means: Every run gets private files

Under the hood, each run gets a branch and a Git worktree. A branch is a named line of changes. A worktree is another checkout of the same history in a different folder. That is the separate table from our bridge story. Agents can edit the same source file at the same time because each one touches its own copy. Asylum creates the worktrees, tracks them, and cleans them up safely later.

## Watch the fleet: Status and activity answer different questions

A run's status says what happened to the process: queued, running, succeeded, failed, or cancelled. Activity says what the agent is doing right now: working, blocked, done, or idle. The orange blocked chip is the one to notice. It means the agent is waiting for you, so open its terminal and answer. Succeeded only means the command exited cleanly. It does not prove the change is correct.

## Watch the fleet: The worktree survives the attempt

Open a run's terminal when you need the full conversation. Cancel if the direction is wrong. Retry after a temporary failure. A follow-up or review comment continues in the same worktree and increments the attempt count, so the agent keeps its files instead of starting over. If Asylum closes during a run, the old live terminal cannot survive, but the worktree and saved transcript remain available for inspection and retry.

## Review the evidence: Compare the change, not the confidence

Select a run and open Review. Switch between runs with the comparison buttons. Green lines were added, red lines were removed, and unchanged lines provide context. Unified and side-by-side views show the same evidence in different shapes. Ask whether the right files changed, whether the request is complete, and whether unrelated code moved. A confident summary is useful, but the diff is the source of truth.

## Review the evidence: Checks are the health signal

Run checks for each serious candidate. Asylum detects the project's real type check, lint, and test commands for Rust, JavaScript, Python, and Go, and runs them inside that run's worktree. A passing badge is strong evidence. A failing or active check blocks merge and pull-request actions. If you do not read code yet, use checks as a health signal, then ask for an explanation when something still feels unclear.

## Review the evidence: Feedback stays attached to a line

If a run is close, click the exact changed line and write the correction in plain language. The comment is stored against that file and line. Send the open comments back to the agent and Asylum starts another attempt in the same worktree. This is often better than discarding a good approach. Review becomes a loop: inspect, comment, revise, and check again until one candidate is clearly strongest.

## Finish safely: Stage, preflight, confirm

A successful run stays uncommitted so you can stage only the hunks or files you want. Before merge, Asylum blocks failed checks, verifies that your base worktree is clean, and performs a non-destructive conflict preflight. Then it asks for confirmation. Choose a regular merge, a squash merge, or create a pull request. Cleanup removes only clean finished worktrees and safely merged branches. Losing or dirty work is left alone.

## Add durable context: Notes make the next task better

Notes are plain Markdown project memory. Keep them private or store them in the repository, add properties and tags, link notes with double brackets, and see backlinks. Attach a note to a task or run and its Markdown becomes agent context. This is the right place for a specification, a decision, or a recurring constraint that should outlive one prompt. The code and the knowledge stay connected without hiding the source files.

## After the first win: Grow into the power tools

After the core loop feels natural, add only the tools that solve a real problem. Layouts repeat useful races. Named agents keep a role and project memory across tasks. Schedules start ordinary runs on a cadence. Routines record a shell workflow and replay it. GitHub and Linear turn issues into worktrees. The browser's design mode sends an exact element to an agent. The command line, control surface, mobile companion, secrets proxy, MCP gateway, and plugins are there when you need deeper automation.

## Good defaults: Keep scope, cost, and trust visible

Use small outcomes with visible acceptance checks. Match the number of agents to the decision's value. Never merge on a polished summary alone. Read the diff and run the checks. Review repository setup commands before granting trust. Keep the control and companion servers token-protected. Store durable context in notes, and use the secrets proxy when an agent needs an API without seeing the credential. The final merge remains your decision.

## Your next five clicks: Open. Ask. Run. Review. Keep.

Your first session is five moves. Open a repository. Write one small outcome. Choose one verified agent and create the run. Review its diff and checks. Then merge it only when the evidence is clear. On the second task, try a duel and compare two approaches. You can return to this video from Asylum's Help menu at any time, with captions and the full transcript. That is the whole mental model: separate tables, visible evidence, one deliberate winner.
