# Episode 04 — Reading Semantic States

**Duration:** ~4 min · **Level:** Beginner
**You'll learn:** the four activity chips — working, blocked, done, idle — and how to spot at a glance the one agent that needs your input.
**Prerequisites:** [Episode 03](03-your-first-fanout.md) — a task fanned out to several agents.

## Shot list

| Time | On screen | Action | Narration |
|------|-----------|--------|-----------|
| 0:00 | Board with several running cards. | Land on a busy board. | "When five agents are racing, which one needs you right now? Status can't tell you that — a process can be alive and still stuck waiting. That's what activity is for." |
| 0:22 | Two-column overlay: status vs. activity. | Show the distinction. | "Status is the lifecycle of the process. Activity is the live semantic state — what the agent is doing this second, classified from its terminal output on every snapshot." |
| 0:44 | Chip legend: four chips with colors. | Reveal all four. | "There are four. Working, in blue — thinking, editing, or running a command. Blocked, in orange — stopped at a prompt, waiting on you. Done, in green — it printed a completion marker, ready for review. And idle, in gray — started but not yet showing a recognizable signal." |
| 1:16 | A card flips to the orange **blocked** chip. | Zoom the blocked card. | "This is the one that matters most. Blocked, orange, means 'this agent needs me.' On a board of five, that chip is how you find the one to attend to instead of scanning five terminals." |
| 1:42 | Open the blocked run's pane. | Click into it; answer the prompt. | "Open it and there's the prompt waiting — a yes-or-no, a menu, a password. Answer it, and watch the chip flip back to working." |
| 2:08 | Card returns to blue **working**. | Chip changes. | "Back to working. You unblocked it in seconds because the chip told you exactly where to look." |
| 2:30 | Diagram: strip ANSI → match markers. | Simple animation. | "How does it know? It's a pure classifier over recent output — it strips the color codes, lowercases the text, and matches substring markers against the last few lines." |
| 2:54 | Precedence list: blocked → done → working. | Show the order. | "The precedence is deliberate. Blocked is checked first, in a tight window of the last few lines, so an old prompt scrolled up in history doesn't read as a live one. Then done, then working over a slightly wider window." |
| 3:18 | Marker examples floating in. | Show `(y/n)`, `press enter`, `✓`, `done`. | "Markers are things like a yes-slash-no, 'press enter', or a prompt caret for blocked; a checkmark or 'done' for done; 'thinking' or 'compiling' for working. If nothing matches, the previous state just stays." |
| 3:40 | A card whose chip changes on its own. | Note a self-report. | "An agent can also self-report its own state over the control surface — and when it does, that's authoritative, because the agent knows itself better than any classifier. More on that later in the series." |
| 3:58 | Board settled. | Hold. | "That's the fleet made legible. Next: reviewing what these agents actually produced." |

## B-roll / capture notes
- The gold shot is a real **blocked** chip. Fan out a prompt to an agent that pauses for confirmation (or trigger any interactive prompt) so you can capture orange → answer → blue on camera.
- If you can't force a natural block, `asylum control activity blocked` from inside a run will flip the chip (foreshadows episode 12).
- Keep the four-chip legend on screen long enough to read the colors.
- Capture at least three cards so the "scan five terminals" point lands visually.

## Recap card (end screen)
- **Status** = process lifecycle; **activity** = what the agent is doing now.
- Four chips: working (blue), blocked (orange), done (green), idle (gray).
- **Blocked** is the "needs me" signal — blocked-first precedence keeps it honest.
- Activity is classified from output, and an agent can self-report it authoritatively.

## Next
- [Episode 05 — Review: Diffs, Checks, Annotations](05-review-diffs-checks-annotations.md)

Go deeper: [book chapter 4](../book/04-the-fleet-in-depth.md).
