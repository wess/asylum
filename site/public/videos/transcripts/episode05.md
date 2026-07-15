# Agent activity and terminals

Intermediate · Episode 5

## 1. Board with several running cards.

When five agents are racing, which one needs you right now? Status can't tell you that — a process can be alive and still stuck waiting. That's what activity is for.

## 2. Two-column overlay: status vs. activity.

Status is the lifecycle of the process. Activity is the live semantic state — what the agent is doing this second, classified from its terminal output on every snapshot.

## 3. Chip legend: four chips with colors.

There are four. Working, in blue — thinking, editing, or running a command. Blocked, in orange — stopped at a prompt, waiting on you. Done, in green — it printed a completion marker, ready for review. And idle, in gray — started but not yet showing a recognizable signal.

## 4. A card flips to the orange blocked chip.

This is the one that matters most. Blocked, orange, means 'this agent needs me.' On a board of five, that chip is how you find the one to attend to instead of scanning five terminals.

## 5. Open the blocked run's pane.

Open it and there's the prompt waiting — a yes-or-no, a menu, a password. Answer it, and watch the chip flip back to working.

## 6. Card returns to blue working.

Back to working. You unblocked it in seconds because the chip told you exactly where to look.

## 7. Diagram: strip ANSI → match markers.

How does it know? It's a pure classifier over recent output — it strips the color codes, lowercases the text, and matches substring markers against the last few lines.

## 8. Precedence list: blocked → done → working.

The precedence is deliberate. Blocked is checked first, in a tight window of the last few lines, so an old prompt scrolled up in history doesn't read as a live one. Then done, then working over a slightly wider window.

## 9. Marker examples floating in.

Markers are things like a yes-slash-no, 'press enter', or a prompt caret for blocked; a checkmark or 'done' for done; 'thinking' or 'compiling' for working. If nothing matches, the previous state just stays.

## 10. A card whose chip changes on its own.

An agent can also self-report its own state over the control surface — and when it does, that's authoritative, because the agent knows itself better than any classifier. More on that later in the series.

## 11. Board settled.

That's the fleet made legible. Next: reviewing what these agents actually produced.

## 12. The Terminal surface.

Beyond the fan-out board, Asylum bundles the workaday surfaces you expect from an IDE — plus one that's unique to an ADE. Start with the terminal.

## 13. An interactive shell in a pane.

This is a real terminal, powered by the same engine that renders each running agent's live pane. Every run's agent runs in one of these.

## 14. Split the pane to the right.

And you can split it — divide a pane and work in two shells at once. Watch a build in one split while you poke around in the other.

## 15. Persisted transcript after exit.

While a run is live its pane streams output. After the process exits, Asylum keeps the persisted transcript, so you can still read what happened — even though live terminals don't survive an app restart, their captured output does.

## 16. The Editor surface with a file tree.

The Editor is a code editor with a file tree for navigating a worktree — quick manual tweaks without leaving the ADE.

## 17. Settings editor block.

Its behavior lives in settings under the editor key — font family, font size, tab width, and autosave. Because they're in settings, they reload the moment you save.

## 18. Toggle autosave / change font size live.

Flip autosave on and it writes changes as you go. Change the font size and it updates live.

## 19. The Preview surface on a Markdown file.

Preview renders files richly instead of as raw text. Markdown becomes formatted HTML.

## 20. Preview classifying image / PDF / binary.

It classifies the file and shows it appropriately — images displayed, PDFs shown as documents, text as text, and unknown or binary files labeled as such, so you're not staring at garbage characters.

## 21. The Browser surface on a local dev server.

The Browser is an embedded web view — handy for a local dev server, docs, or a deployed preview. But its standout feature exists specifically to feed visual context to an agent: design mode.

## 22. Toggle design mode on.

Toggle design mode on. Now the page is annotatable.

## 23. Click an element; capture overlay.

Click an element. Asylum captures that element's HTML, its CSS, and a selector that identifies it — the real markup and styles, not your description of them.

## 24. Attach a note to the capture.

Attach a note to the capture — 'make this button match the primary style,' 'this heading is misaligned on mobile.'

## 25. Numbered pin badges on the page.

Each annotated element gets a numbered pin badge, so you can see and manage what you've marked. Do a few.

## 26. Send the batch to an agent.

Then send the batch to an agent. All your captures and their notes are gathered into one prompt and handed to a run. Preview shares the same design surface, so you can annotate rendered content the same way.

## 27. Back to a run receiving the batch.

Instead of describing a UI bug in words, you point at the exact element. Next up: taking everything to the command line.
