# Chapter 8: Integrations

Asylum does not live in a vacuum — work starts as an issue and lands as a pull
request. The **Integrations** surface connects the fleet to GitHub and Linear so
you can start a worktree from an issue and open a PR from a winning run without
leaving the ADE. This chapter covers both.

## GitHub

Asylum talks to GitHub through the **`gh` CLI**. That means it uses whatever
GitHub authentication `gh` already has — install `gh`, run `gh auth login` once,
and the integration works with no token to paste into Asylum. If `gh` is not
installed or not authenticated, the GitHub features are unavailable.

From the Integrations surface you can:

- **List pull requests.** See the repository's open PRs.
- **List issues.** Browse open issues.
- **Create a pull request.** Open a PR from a run's branch — the natural
  alternative to a local merge when you want the change reviewed on GitHub. This
  is the same "open a PR from the winner" path referenced in
  [Chapter 6](06-diffs-checks-and-review.md).

### Issue → worktree

The most useful GitHub flow is turning an issue into work. Asylum can **derive a
worktree branch from an issue**: pick an issue, and it starts a task/worktree on
a branch named for that issue, so the agents you fan out are already on the right
branch for a PR that closes it. You go from "issue #123 needs doing" to "three
agents attempting #123 in isolated worktrees" in a couple of clicks.

### PR links flow back to notes

Remember from [Chapter 7](07-notes-and-knowledge.md) that a created pull request
appends its link to every note attached to the task. So opening a PR from a
winning run also leaves a durable trail in your project memory — the note that
started the task ends up carrying the link to the PR that finished it.

## Linear

Asylum also integrates with **Linear** over Linear's GraphQL API. Unlike GitHub,
Linear needs an **API token**. Create one at
`https://linear.app/settings/api` and set it in `settings.json`:

```jsonc
{
  // Linear API token. Empty disables Linear.
  "linear_token": "lin_api_..."
}
```

Live reload picks the token up immediately. With it set, the Integrations surface
browses your Linear workspace:

- **Teams** and **projects**.
- **Issues** — browse them, and open a worktree from an issue just as with
  GitHub, so a Linear ticket becomes a fanned-out task.
- **Create an issue** from within the ADE.

An empty `linear_token` leaves Linear disabled and the surface simply does not
offer it.

## Choosing local merge vs. a PR

You now have two ways to land a winning run:

- **Local merge** ([Chapter 6](06-diffs-checks-and-review.md)) — fastest, fully
  in Asylum, with the guarded preflight. Best for solo work and small changes.
- **Pull request** — opens the change on GitHub for review and CI. Best for team
  work, anything that needs a second pair of eyes, or repositories with required
  checks.

Both start from the same place: a selected run whose diff is right and whose
checks pass.

## Try it

1. Run `gh auth login` if you have not, then open Integrations and list your
   repository's open PRs and issues.
2. Pick a small issue and start a worktree from it; fan the resulting task out to
   two agents.
3. When one wins, open a PR from its branch and confirm the PR link appears in
   the task's attached note.
4. If you use Linear, set `linear_token` and confirm your teams and issues load.

## Recap

- GitHub works through the `gh` CLI (no token in Asylum): list PRs and issues,
  create a PR, and derive a worktree branch from an issue.
- Linear works over its GraphQL API and needs `linear_token`: browse teams,
  projects, and issues, create issues, and open a worktree from an issue.
- Created PR links flow back into attached notes.
- Choose a local merge for speed or a PR for review.

## Next

[Chapter 9: Terminal, Editor, Preview, Browser](09-terminal-editor-preview-browser.md)
covers the surfaces you use to work *inside* a run.
