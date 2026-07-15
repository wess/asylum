# Chapter 9: Terminal, Editor, Preview, Browser

Beyond the fan-out board, Asylum bundles the workaday surfaces you expect from an
IDE — an embedded terminal, a code editor, rich previews, and a browser — plus
one that is unique to an ADE: **design mode**, where you click an element on a
page and send it straight to an agent. This chapter tours all four.

## Terminal

Asylum embeds a real terminal, powered by the `libsinclair` terminal core, and
it is the same engine that renders each running agent's live pane. On the
**Terminal** surface you get an interactive shell that you can **split** — divide
a pane to the right and work in two shells at once — so you can watch a build in
one split while poking around in another.

Every run's agent runs in one of these terminals. While a run is live its pane
streams the agent's output; after the process exits, Asylum keeps the persisted
transcript so you can still read what happened (live ptys do not survive an app
restart, but their captured output does — see
[Chapter 4](04-the-fleet-in-depth.md)).

## Editor

The **Editor** surface is a code editor with a **file tree** for navigating a
worktree. Editor behavior is configurable in `settings.json` under the `editor`
key:

```jsonc
{
  "editor": {
    "font_family": "monospace",
    "font_size": 13,
    "tab_width": 4,
    "autosave": true
  }
}
```

`autosave` writes changes as you go; `tab_width`, `font_size`, and `font_family`
tune the look. Because these live in settings, they reload as soon as you save
the file.

## Preview

The **Preview** surface renders files richly instead of as raw text. It classifies
the file and shows it appropriately:

- **Markdown** is rendered to formatted HTML.
- **Images** are displayed.
- **PDFs** are shown as documents.
- **Text** is shown as text, and unknown/binary files are classified as such so
  you are not staring at mojibake.

Preview is handy for reading a run's generated docs, checking an asset an agent
produced, or eyeballing a README without opening an external app.

## Browser and design mode

The **Browser** surface is an embedded web view — useful for viewing a local dev
server, documentation, or a deployed preview without leaving the ADE. Its
standout feature is **design mode**, and it exists specifically to feed visual
context to a coding agent.

Here is the flow:

1. **Toggle design mode** on in the Browser.
2. **Click an element** on the page. Asylum captures that element's **HTML**, its
   **CSS**, and a **selector** identifying it.
3. **Attach a note** to the capture — "make this button match the primary style,"
   "this heading is misaligned on mobile."
4. Each annotated element gets a **numbered pin badge** so you can see and manage
   what you have marked.
5. **Send the batch to an agent.** All your captures and their notes are gathered
   into a single agent prompt and handed to a run.

This closes a gap that text prompts cannot: instead of describing a UI problem in
words, you point at the exact element and let the agent see its real markup and
styles. The **Preview** surface shares the same design surface, so you can
annotate rendered content the same way.

## How these fit the loop

None of these surfaces replace the fan-out board — they support it. You fan a
task out, then use the terminal to poke at a worktree, the editor to make a quick
manual tweak, preview to read what an agent generated, and the browser's design
mode to hand an agent a precise visual bug. Then you go back to
[Diff review](06-diffs-checks-and-review.md) and choose a winner.

## Try it

1. On the Terminal surface, split a pane and run a long command in one side while
   you `ls` around in the other.
2. Open a Markdown file in Preview and confirm it renders formatted, not raw.
3. Point the Browser at a local dev server, toggle design mode, click two
   elements, attach a note to each, and send the batch to a run. Watch the agent
   receive both captures in one prompt.

## Recap

- The terminal is splittable and is the same engine that runs each agent's pane;
  transcripts persist after exit.
- The editor has a file tree and is tuned by the `editor` settings key.
- Preview renders Markdown/images/PDFs and classifies text vs. binary.
- The browser's design mode captures an element's HTML/CSS/selector, pins it, and
  batches your annotations into one agent prompt; Preview shares that surface.

## Next

[Chapter 10: The CLI](10-the-cli.md) takes everything to the command line.
