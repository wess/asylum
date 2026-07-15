# Episode 10 — Terminal, Editor, Preview, Browser

**Duration:** ~6 min · **Level:** Intermediate
**You'll learn:** the splittable terminal, the editor and its file tree, rich previews, and the browser's design mode — clicking an element and sending it to an agent.
**Prerequisites:** [Episode 02](02-open-a-project-and-pick-agents.md) — a project open.

## Shot list

| Time | On screen | Action | Narration |
|------|-----------|--------|-----------|
| 0:00 | The Terminal surface. | Land on Terminal. | "Beyond the fan-out board, Asylum bundles the workaday surfaces you expect from an IDE — plus one that's unique to an ADE. Start with the terminal." |
| 0:20 | An interactive shell in a pane. | Type a command. | "This is a real terminal, powered by the same engine that renders each running agent's live pane. Every run's agent runs in one of these." |
| 0:42 | Split the pane to the right. | Split; run two shells. | "And you can split it — divide a pane and work in two shells at once. Watch a build in one split while you poke around in the other." |
| 1:06 | Persisted transcript after exit. | Let a process exit; scroll back. | "While a run is live its pane streams output. After the process exits, Asylum keeps the persisted transcript, so you can still read what happened — even though live terminals don't survive an app restart, their captured output does." |
| 1:34 | The Editor surface with a file tree. | Open Editor; navigate the tree. | "The Editor is a code editor with a file tree for navigating a worktree — quick manual tweaks without leaving the ADE." |
| 2:00 | Settings `editor` block. | Show the editor settings. | "Its behavior lives in settings under the editor key — font family, font size, tab width, and autosave. Because they're in settings, they reload the moment you save." |
| 2:26 | Toggle autosave / change font size live. | Edit a setting; watch it apply. | "Flip autosave on and it writes changes as you go. Change the font size and it updates live." |
| 2:52 | The Preview surface on a Markdown file. | Open a `.md` in Preview. | "Preview renders files richly instead of as raw text. Markdown becomes formatted HTML." |
| 3:16 | Preview classifying image / PDF / binary. | Cycle a few file types. | "It classifies the file and shows it appropriately — images displayed, PDFs shown as documents, text as text, and unknown or binary files labeled as such, so you're not staring at garbage characters." |
| 3:44 | The Browser surface on a local dev server. | Point the browser at localhost. | "The Browser is an embedded web view — handy for a local dev server, docs, or a deployed preview. But its standout feature exists specifically to feed visual context to an agent: design mode." |
| 4:12 | Toggle design mode on. | Flip the design-mode switch. | "Toggle design mode on. Now the page is annotatable." |
| 4:32 | Click an element; capture overlay. | Click a button on the page. | "Click an element. Asylum captures that element's HTML, its CSS, and a selector that identifies it — the real markup and styles, not your description of them." |
| 5:00 | Attach a note to the capture. | Type a note on the element. | "Attach a note to the capture — 'make this button match the primary style,' 'this heading is misaligned on mobile.'" |
| 5:24 | Numbered pin badges on the page. | Show pins on two elements. | "Each annotated element gets a numbered pin badge, so you can see and manage what you've marked. Do a few." |
| 5:46 | Send the batch to an agent. | Send captures to a run. | "Then send the batch to an agent. All your captures and their notes are gathered into one prompt and handed to a run. Preview shares the same design surface, so you can annotate rendered content the same way." |
| 6:06 | Back to a run receiving the batch. | Hold. | "Instead of describing a UI bug in words, you point at the exact element. Next up: taking everything to the command line." |

## B-roll / capture notes
- Have a local dev server running (any simple page) so the Browser and design-mode captures are real.
- For the terminal split, run something long-lived (a `tail -f` or a build) in one pane so the split is visually meaningful.
- Prepare a Markdown file, an image, and a PDF to cycle through Preview's classification.
- Capture the numbered pin badges clearly and the moment the batch is sent to a run.

## Recap card (end screen)
- The terminal is splittable and shares the agent-pane engine; transcripts persist after exit.
- The editor has a file tree and is tuned by the `editor` settings key (live reload).
- Preview renders Markdown/images/PDFs and classifies text vs. binary.
- Browser design mode captures an element's HTML/CSS/selector, pins it, and batches annotations into one agent prompt; Preview shares that surface.

## Next
- [Episode 11 — The CLI Tour](11-the-cli-tour.md)

Go deeper: [book chapter 9](../book/09-terminal-editor-preview-browser.md).
