# Project memory

Asylum stores project knowledge as ordinary Markdown. It does not hide note
content in SQLite or a proprietary format.

## Vaults

Every project starts with a private vault under Asylum's data directory. The
Notes toolbar can switch it to the repository's `notes/` directory so the team
can review and version notes with the code. Switching copies missing Markdown
files into the target and never deletes the source; target conflicts are kept
and reported.

The Notes file list recursively indexes `.md` files and ignores hidden
directories such as `.obsidian` and `.git`.

## Obsidian syntax

YAML frontmatter is exposed as properties. `tags` accepts a YAML list or a
space/comma-separated value, and inline `#tags` are indexed outside fenced code.

```markdown
---
title: Cache investigation
type: investigation
tags: [backend, reliability]
---

# Cache investigation

The decision is recorded in [[Cache policy|the policy]]. #active
```

Type `[[` in the editor to complete another note. Links resolve by title, file
stem, or relative path. Backlinks and outgoing links appear in the Links pane.
Renaming a note updates incoming wiki links across the vault. Preview links open
the target inside Notes.

## Workflow

- **Create task** creates a task whose attached note is its source of truth.
- **Attach to run** adds the note to an existing run's durable context.
- **Send selection** sends the exact editor selection to the selected run. A
  live stdin-capable agent receives it immediately; a finished run starts a new
  attempt in the same worktree.
- Task notes are inherited by every fan-out run and appended to the launch
  prompt. Completed checks and created pull requests append links to every
  attached Markdown note.

Project Search queries notes, repository files, task prompts, run metadata, and
persisted terminal transcripts from one input.
