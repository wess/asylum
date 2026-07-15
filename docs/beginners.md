# Asylum for non-engineers

You do not need to be a programmer to use Asylum. If you can describe what you
want in plain English, you can put a fleet of AI coding agents to work and review
what they produce. This page is the no-jargon path.

## The one-minute mental model

- A **project** is a folder of code (a "repository") you point Asylum at.
- A **task** is something you want done, written in plain English —
  "add a dark-mode toggle", "fix the login bug".
- When you run a task, Asylum gives each AI agent its **own private copy** of the
  project (a "worktree") so they never trip over each other. Running the same
  task on several agents at once is called a **fan-out**.
- You then **compare** what each agent changed, pick the best, and **merge** it
  back — or send comments and let an agent try again.

Everywhere those words appear in the app, hover the label for a plain-language
explanation.

## Getting set up (once)

1. **Install Asylum.** Download the `.dmg` (macOS) or `.deb` (Linux) from the
   releases page and open it — no developer tools required.
2. **Install at least one agent.** Asylum runs coding agents you already have
   access to (Claude Code, Codex, and others). The **setup doctor** on the Tasks
   screen tells you which are installed and shows a copy-paste install command
   for the ones that aren't. Paste it into a terminal once.
3. **Open a project.** Click *Open a folder…* and choose a code folder. If it
   isn't yet tracked by git, Asylum asks first before setting it up — nothing
   happens without your say-so.

## Your first task

1. On the **Tasks** screen, type what you want in plain English.
2. Click *Choose agents* if you want more than one to try it; otherwise Asylum
   picks a ready one for you.
3. Click **Create and run**. Each agent starts working in its own copy.
4. Watch progress on the run cards. When they finish, click **Review** on one.
5. In review you see exactly what changed. Click a line to leave a comment, then
   **Send review to agent** to ask for changes — or **Merge** to keep it.

## Reviewing without reading code

- Green lines were **added**, red lines were **removed**.
- The **checks** badge (PASS/FAIL) tells you whether the project's own tests
  still pass — a quick health signal even if you don't read the code.
- Switch between **Unified** and **Side-by-side** views, whichever is clearer.
- If you're unsure, leave a comment in plain English and send it back; the agent
  will explain or revise.

## From your phone

Turn on the mobile companion in Settings (set a token to reach it from your
phone). You can watch runs, read what agents are doing, and send a follow-up
message that is delivered straight to the running agent.

## When something looks wrong

Asylum won't do anything destructive without asking. Merges, deletes, and
worktree removal all pop a confirmation first, and error messages tell you what
to do next. If an agent isn't found, the setup doctor shows how to install it.
