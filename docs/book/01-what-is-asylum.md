# Chapter 1: What Is Asylum?

In this chapter you will learn what an Agent Development Environment is, why
running several AI coding agents in parallel is a good idea, the three words that
describe everything Asylum does, and what each of its screens is for. No prior
knowledge is assumed.

## The idea: an environment for agents

A traditional IDE is built for one person at one keyboard. Simple wrappers
around an AI coding agent stop at a single terminal. Asylum is a whole
**environment for agents** — an *Agent Development Environment*, or ADE.

The core move is this: you write one prompt, hand it to *several* coding agents
at once, let each one work in complete isolation, then compare what they produced
and keep the best result. Instead of betting your afternoon on a single agent's
attempt, you run a small tournament and pick the winner.

Why does isolation matter? Because two agents editing the same files at the same
time would collide. Asylum gives each agent its own **git worktree**.

### Git, briefly

If you are new to git: a **repository** ("repo") is a project's folder tracked
by git. A **branch** is a named line of changes — you can make edits on a branch
without touching anyone else's work. A **worktree** is a separate checkout of the
same repository on its own branch, in its own folder on disk. Two worktrees share
the same history but have independent working files, so two agents can edit
freely without stepping on each other. A **diff** is the set of line-by-line
changes between two states of the code — what an agent added, removed, or
modified.

Asylum creates and tears down these worktrees for you. You never juggle branches
or stash changes by hand.

## The vocabulary: project, task, run

Everything in Asylum is built from three nouns:

- **Project** — a git repository you work in. You point Asylum at a repo folder
  and it becomes a project.
- **Task** — a prompt you pose against a project. "Add rate limiting to the
  login endpoint" is a task.
- **Run** — one agent's attempt at a task, in its own worktree, on its own
  branch. Fan a task out to three agents and you get three runs.

Hold these three words firmly and the rest of Asylum falls into place. You open a
**project**, compose a **task**, fan it out into several **runs**, and choose a
winner to merge.

## The core loop

1. **Pose a task.** Write a prompt against a project.
2. **Fan out.** Asylum allocates one branch and one worktree per selected agent.
3. **Launch.** Each agent runs live in its own terminal pane.
4. **Track.** You watch progress. A **status** (queued → running → succeeded /
   failed) says whether the process is alive; a live **activity** (working,
   blocked, done, idle) says what it is *doing* right now — most importantly,
   which agent is blocked waiting for you.
5. **Review.** Read each run's diff, run the project's checks (type-check, lint,
   test), and leave inline comments.
6. **Merge or open a PR.** Pick the winning run and merge its branch back, or
   open a pull request from it.

## A tour of the surfaces

Asylum's window routes between thirteen surfaces. You will meet each in depth
later; here is the map:

- **Tasks** — the fan-out board where you compose tasks and watch runs race.
- **Diff** — annotatable diff review with PASS/FAIL checks and inline comments.
- **Search** — cross-worktree content search.
- **Notes** — a Markdown knowledge vault with wiki links, tags, and templates.
- **Integrations** — GitHub pull requests and issues, plus Linear.
- **Terminal** — an embedded, splittable terminal.
- **Editor** — a code editor with a file tree.
- **Preview** — rich previews of Markdown, images, and PDFs.
- **Browser** — an embedded web view with *design mode* (click an element, send
  it to an agent).
- **Plugins** — manage installed extensions.
- **Accounts** — provider accounts and usage.
- **Inbox** — notifications (a run finished, a check failed).
- **Settings** — an editor over your `settings.json`.

There is also an `asylum` **command-line tool** for scripting the ADE from the
shell, and a **mobile companion** server so you can check on the fleet from your
phone.

## Try it

You do not need Asylum installed to do this one. On paper, pick a small change
you would ask an AI agent to make in a codebase you know. Write it as a one-
sentence prompt. That sentence is a *task*. Now imagine handing it to three
different agents at once — each in its own copy of the repo — and comparing their
work side by side. That mental model *is* Asylum.

## Recap

- Asylum is an ADE: run many coding agents in parallel, compare, merge the best.
- Isolation comes from git worktrees; Asylum manages them for you.
- The three nouns are **project** (a repo), **task** (a prompt), and **run** (one
  agent's attempt).
- The loop is pose → fan out → track → review → merge.

## Next

[Chapter 2: Installation and Setup](02-installation-and-setup.md) gets Asylum
building and running on your machine, and opens your first project.
