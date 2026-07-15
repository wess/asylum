# Chapter 7: Notes and Knowledge

Agents do better work with better context. Asylum's **Notes** surface is a
project knowledge vault made of ordinary Markdown — wiki links, backlinks, tags,
templates, and durable references to your tasks and runs — that you can attach to
a task so its content becomes agent context. This chapter covers the vault, the
note syntax, and the workflow that connects notes to the fleet.

## Plain Markdown, not a silo

Asylum stores project knowledge as **ordinary Markdown files**. It does not hide
note content in a database or a proprietary format — the `.md` files on disk are
the source of truth. SQLite stores only the vault *choice* and note
*attachments*, never the note bodies. That means your notes are yours: readable,
diffable, and portable with any editor.

## Vaults: private or repository

Every project starts with a **private** vault under Asylum's data directory —
notes only you see. From the Notes toolbar you can switch the vault to the
repository's `notes/` directory, so the team can review and version notes
alongside the code. Switching **copies** missing Markdown into the target and
never deletes the source; if a file already exists in the target, the conflict is
kept and reported rather than overwritten.

The file list recursively indexes every `.md` file and ignores hidden
directories such as `.git`.

## Note syntax

### Properties (YAML frontmatter)

A note can open with a YAML frontmatter block, exposed in the UI as editable
**properties**:

```markdown
---
title: Cache investigation
type: investigation
tags: [backend, reliability]
---

# Cache investigation

The decision is recorded in [[Cache policy|the policy]]. #active
```

`tags` accepts a YAML list or a space/comma-separated string, and inline
`#tags` in the body are indexed too (outside fenced code blocks). Click a tag to
filter the vault to notes carrying it.

### Wiki links and backlinks

Type `[[` in the editor to autocomplete another note. A link like
`[[Cache policy|the policy]]` resolves by title, file stem, or relative path, and
renders the alias after the `|`. The Links pane shows a note's outgoing links and
its **backlinks** — every other note that links *to* it — so the vault becomes a
navigable web. Renaming a note updates the incoming wiki links across the vault
automatically, so links never rot. In preview, clicking a link opens the target
inside Notes.

### Templates

New notes can start from a template. Asylum ships structures for common shapes —
a **task**, a **decision**, an **investigation**, and a **retrospective** — and
you can add your own. Templates give recurring documents a consistent skeleton so
you fill in content instead of reinventing headings.

The Notes surface also renders callouts, Mermaid diagrams, and syntax-highlighted
code blocks, so a note reads well in preview.

## Connecting notes to the fleet

This is where the vault earns its place in an ADE. Notes are not just
documentation — they are **context you hand to agents**.

- **Create task from a note.** Creating a task from a note makes that note the
  task's source of truth.
- **Attach to run.** Attaching a note adds it to an existing run's durable
  context.
- **Inheritance.** A note attached to a task is **inherited by every fan-out
  run** it generates, and its Markdown is appended to the launch prompt. So a
  design note or a spec becomes part of what every racing agent reads.
- **Send selection.** Send the exact editor selection from a note to the selected
  run. A live, stdin-capable agent receives it immediately; a finished run starts
  a new attempt in the same worktree with it.

Because attachments are recorded in the store, this context survives restarts —
an attached note is durable, not a one-time paste.

## Durable task/run/check/PR references

The connection runs both ways. When work happens, Asylum **writes links back into
the attached notes**: completed checks and created pull requests append their
links to every note attached to the task. Over time a task note accumulates a
durable trail — the prompt, the runs it produced, their check results, and the PR
that landed — as plain Markdown you can read months later.

## Unified search

The project **Search** surface queries more than source files. From one input it
searches your **notes**, the repository's **files**, task **prompts**, run
**metadata**, and persisted terminal **transcripts** together. So "where did that
agent mention the cache key?" finds the answer whether it lives in a note, the
code, or a run's output. (Cross-worktree source search is also available from the
CLI — see [Chapter 10](10-the-cli.md).)

## Try it

1. In Notes, create an investigation note from the template and give it a couple
   of `#tags`.
2. Link it to another note with `[[`, then open the Links pane and see the
   backlink appear on the target.
3. Attach the note to a task, fan the task out, and confirm the note's content
   reached the agents (it is appended to the prompt).
4. After a run's checks finish, reopen the note and find the check/PR links
   appended.

## Recap

- The vault is plain Markdown; SQLite stores only the vault choice and
  attachments.
- Vaults are private or repository-backed; switching copies, never deletes.
- Syntax: YAML properties, `#tags`, `[[wiki links]]` with backlinks and rename
  relinking, and templates.
- Attached notes become durable agent context (inherited by runs, appended to
  prompts), and check/PR links are written back into them.

## Next

[Chapter 8: Integrations](08-integrations.md) connects the fleet to GitHub and
Linear.
