# Episode 09 — Integrations

**Duration:** ~5 min · **Level:** Intermediate
**You'll learn:** how the Integrations surface connects the fleet to GitHub (PRs, issues, issue→worktree) and Linear.
**Prerequisites:** [Episode 06](06-merge-the-winner.md) — you have a winning run to open a PR from.

## Shot list

| Time | On screen | Action | Narration |
|------|-----------|--------|-----------|
| 0:00 | The Integrations surface. | Land on Integrations. | "Work usually starts as an issue and lands as a pull request. Integrations connects the fleet to GitHub and Linear so you can do both without leaving Asylum." |
| 0:20 | Terminal: `gh auth login`. | Show gh being authenticated. | "GitHub works through the gh CLI. That means Asylum uses whatever authentication gh already has — install gh, run gh auth login once, and there's no token to paste. If gh isn't installed or authenticated, the GitHub features are simply unavailable." |
| 0:50 | PR list in Integrations. | List open PRs. | "With gh authenticated, you can list the repository's open pull requests right here." |
| 1:10 | Issue list in Integrations. | List open issues. | "And browse open issues." |
| 1:30 | Issue → worktree action. | Pick an issue; derive a worktree. | "The most useful flow is turning an issue into work. Pick an issue and Asylum derives a worktree branch named for it — so the agents you fan out are already on the right branch for a PR that closes it." |
| 2:02 | Task created on the issue branch. | Fan the derived task out. | "You go from 'issue one-two-three needs doing' to 'three agents attempting it in isolated worktrees' in a couple of clicks." |
| 2:30 | Winning run selected; create-PR. | Open a PR from the run's branch. | "When one run wins, open a pull request from its branch — the natural alternative to a local merge when you want the change reviewed on GitHub with CI." |
| 2:56 | Note showing the appended PR link. | Reopen the task's attached note. | "And remember from the notes episode: a created PR appends its link to every note attached to the task. So opening the PR also leaves a durable trail — the note that started the task ends up carrying the link to the PR that finished it." |
| 3:24 | Settings surface, `linear_token`. | Open settings to `linear_token`. | "Linear is the other integration. Unlike GitHub, it needs an API token. Create one at linear dot app settings API and set linear underscore token in settings." |
| 3:52 | Paste token; live reload. | Save the file. | "Live reload picks it up immediately. An empty token leaves Linear disabled and the surface just doesn't offer it." |
| 4:14 | Linear teams and projects load. | Browse teams/projects/issues. | "With it set, Integrations browses your Linear workspace — teams, projects, and issues." |
| 4:36 | Open a worktree from a Linear issue. | Derive a task from a Linear issue. | "You can open a worktree from a Linear issue just like GitHub, so a Linear ticket becomes a fanned-out task — and you can create an issue from inside the ADE too." |
| 4:56 | Split: local merge vs. PR recap. | Hold on both paths. | "Two ways to land a winner: a local merge for speed, or a PR for review. Next: the workaday surfaces — terminal, editor, preview, and browser." |

## B-roll / capture notes
- Authenticate `gh` before recording; use a repo you own so listing PRs/issues and opening a PR all succeed.
- Have at least one open issue in the repo to demo issue→worktree.
- For Linear, use a scratch workspace and a throwaway API token; blur the token when pasting.
- Tie back to episode 08: reopen a task's attached note after opening the PR to show the appended link.

## Recap card (end screen)
- GitHub works through the `gh` CLI (no token in Asylum): list PRs and issues, create a PR, derive a worktree from an issue.
- Linear works over its API and needs `linear_token`: browse teams/projects/issues, create issues, open a worktree from an issue.
- Created PR links flow back into attached notes.
- Local merge for speed; a PR for review.

## Next
- [Episode 10 — Terminal, Editor, Preview, Browser](10-terminal-editor-preview-browser.md)

Go deeper: [book chapter 8](../book/08-integrations.md).
