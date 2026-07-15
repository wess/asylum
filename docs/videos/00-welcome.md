# Episode 00 — Welcome to Asylum

**Duration:** ~2 min · **Level:** Beginner
**You'll learn:** what an Agent Development Environment is — a fleet of coding agents run in parallel, each isolated in its own git worktree, then compared and merged.
**Prerequisites:** None. This is the hook for the whole series.

## Shot list

| Time | On screen | Action | Narration |
|------|-----------|--------|-----------|
| 0:00 | Title card: "Asylum — the Agent Development Environment" | Fade in over the Tasks board with several run cards racing. | "This is Asylum. Instead of betting your afternoon on one AI coding agent, you run several at once — and keep the best." |
| 0:14 | Split view: one lonely terminal vs. a board of run cards. | Wipe from single terminal to the fan-out board. | "A normal IDE is built for one person. A simple agent wrapper stops at one terminal. Asylum is a whole environment for agents — an ADE." |
| 0:30 | Three cards labeled with agent names, each showing a branch chip. | Highlight each card in turn. | "You write one prompt and hand it to several agents. Each works in complete isolation, in its own copy of the repo." |
| 0:48 | Animated diagram: one repo → three worktrees on three branches. | Draw the branches splitting from the base. | "That isolation comes from git worktrees. Two agents never step on each other's files. Asylum creates and tears them down for you." |
| 1:04 | Three words on screen: Project · Task · Run. | Reveal one at a time. | "Three words run the whole app. A project is a repo. A task is a prompt. A run is one agent's attempt at that task." |
| 1:22 | The Diff surface with a green PASS badge and a merge button. | Pan across a diff, then a checks panel. | "When the runs finish, you compare their diffs, run the project's real checks, and merge the winner back — behind a safe preflight." |
| 1:40 | Loop graphic: pose → fan out → track → review → merge. | Rotate through the five steps. | "Pose, fan out, track, review, merge. That's the loop. Run a small tournament, pick the winner." |
| 1:52 | End card: "Next: Install and First Launch". | Hold. | "Let's get it running. See you in episode one." |

## B-roll / capture notes
- Have the Tasks board open with a finished fan-out (2–3 run cards, mixed statuses) for the opening shot.
- Capture a clean Diff surface with at least one PASS check for the review beat.
- Keep the diagram animations simple: one repo folder branching into worktrees.
- No CLI in this episode — it's a pure concept hook.

## Recap card (end screen)
- Asylum is an ADE: run many agents in parallel, compare, merge the best.
- Isolation comes from git worktrees, managed for you.
- The vocabulary is **project**, **task**, **run**.
- The loop: pose → fan out → track → review → merge.

## Next
- [Episode 01 — Install and First Launch](01-install-and-first-launch.md)

Go deeper: [book chapter 1](../book/01-what-is-asylum.md).
