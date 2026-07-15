# Episode 08 — Notes and Knowledge

**Duration:** ~6 min · **Level:** Intermediate
**You'll learn:** the Markdown note vault — wiki links, backlinks, tags, templates — and how attaching a note becomes durable context every fan-out run reads.
**Prerequisites:** [Episode 03](03-your-first-fanout.md) — you can fan a task out.

## Shot list

| Time | On screen | Action | Narration |
|------|-----------|--------|-----------|
| 0:00 | The Notes surface. | Land on Notes. | "Agents do better work with better context. Notes is a project knowledge vault made of ordinary Markdown — and it feeds straight into the fleet." |
| 0:20 | A `.md` file open in the editor. | Show the raw Markdown. | "These are plain Markdown files on disk — the source of truth. Asylum's database stores only the vault choice and note attachments, never the note bodies. Your notes stay readable, diffable, and portable." |
| 0:44 | Vault toggle: private vs. repository. | Show the vault switch. | "Every project starts with a private vault only you see. From the toolbar you can switch to the repository's notes folder, so the team can version notes alongside the code." |
| 1:08 | Switching vaults; copy-not-delete callout. | Perform a switch. | "Switching copies missing Markdown into the target and never deletes the source. If a file already exists there, the conflict is kept and reported — never overwritten." |
| 1:34 | New note from a template picker. | Create from a template. | "New notes can start from a template. Asylum ships a few shapes — task, decision, investigation, retrospective — so recurring documents get a consistent skeleton." |
| 2:00 | YAML frontmatter block. | Show properties. | "A note can open with a YAML frontmatter block, shown in the UI as editable properties — a title, a type, tags." |
| 2:24 | Inline `#tags` in the body. | Type a `#tag`; click to filter. | "Tags work in the frontmatter or inline in the body with a hash. Click a tag and the vault filters to notes carrying it." |
| 2:50 | Typing `[[` for autocomplete. | Type `[[` and pick a note. | "Type double square brackets to link another note. Links resolve by title, file stem, or path, and you can add an alias after a pipe." |
| 3:16 | Links pane showing backlinks. | Open the Links pane. | "The Links pane shows a note's outgoing links and its backlinks — every note that links to it — so the vault becomes a navigable web. Rename a note and the incoming links update across the whole vault automatically. Links never rot." |
| 3:44 | Preview render: callouts, Mermaid, code. | Toggle preview. | "In preview it renders richly — callouts, Mermaid diagrams, syntax-highlighted code — and clicking a link opens the target inside Notes." |
| 4:08 | Attach-to-task action. | Attach the note to a task. | "Here's where the vault earns its place. Notes aren't just documentation — they're context you hand to agents. Attach a note to a task." |
| 4:34 | Fan out the task; note inherited. | Launch runs. | "A note attached to a task is inherited by every fan-out run it generates, and its Markdown is appended to the launch prompt. So a design note or a spec becomes part of what every racing agent reads." |
| 5:02 | Send-selection from a note to a run. | Select text; send to run. | "You can also send an exact editor selection from a note to a run. A live, stdin-capable agent gets it immediately; a finished run starts a new attempt with it." |
| 5:26 | Note showing appended check/PR links. | Reopen the note after checks. | "And it runs both ways. When checks finish or a PR is created, Asylum writes those links back into the attached notes. Over time a task note accumulates the whole trail — prompt, runs, checks, and the PR that landed." |
| 5:50 | Search surface across notes + code. | Quick search demo. | "One last thing: project Search queries notes, files, task prompts, run metadata, and terminal transcripts together. Next: connecting the fleet to GitHub and Linear." |

## B-roll / capture notes
- Prepare two or three linked notes ahead of time so backlinks and rename-relinking have something to show.
- Use the investigation template for the create-from-template beat, then add a couple of `#tags`.
- Attach a note to a task and fan out so you can show inheritance; if you can, capture the agent's prompt containing the note text.
- After a run's checks complete, reopen the attached note to show the appended check/PR link (may need episode 09's PR flow recorded first).

## Recap card (end screen)
- The vault is plain Markdown; the DB stores only the vault choice and attachments.
- Vaults are private or repository-backed; switching copies, never deletes.
- Syntax: YAML properties, `#tags`, `[[wiki links]]` with backlinks and rename relinking, templates.
- Attached notes are inherited by runs and appended to prompts; check/PR links flow back in.

## Next
- [Episode 09 — Integrations](09-integrations.md)

Go deeper: [book chapter 7](../book/07-notes-and-knowledge.md).
