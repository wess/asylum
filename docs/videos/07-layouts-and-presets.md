# Episode 07 — Layouts and Presets

**Duration:** ~5 min · **Level:** Intermediate
**You'll learn:** the built-in duel/triad/swarm layouts, picking one from the composer, inspecting them with `asylum layout`, and defining your own.
**Prerequisites:** [Episode 03](03-your-first-fanout.md) — you know how to fan out.

## Shot list

| Time | On screen | Action | Narration |
|------|-----------|--------|-----------|
| 0:00 | The task composer with an agent picker. | Land on compose. | "Ticking the same set of agents for every task gets old fast. A layout is a named fan-out preset — a fixed set of agents, and an optional concurrency cap, that you pick in one gesture." |
| 0:22 | Layout chips on the composer. | Show the preset chips. | "A layout is data, not a keybinding. It answers one question: which agents race this task, and how many at a time?" |
| 0:44 | Table overlay of the three built-ins. | Reveal duel/triad/swarm. | "Out of the box you get three. Duel — a fast head-to-head, claude-code versus codex. Triad — three takes on one prompt, adding aider. And swarm — a wide net across five agents, but only three running at once." |
| 1:14 | Pick `duel`; fan out. | Launch from the duel chip. | "Pick duel and dispatch. Two runs, no ticking boxes — the same task shape re-run in a click." |
| 1:38 | Terminal beside the app. | Run `asylum layout list`. | "You can inspect layouts from the shell too. asylum layout list shows each preset — name, description, and the agents it races." |
| 2:02 | Terminal output of `layout list`. | Point at the rows. | "From a source checkout that's cargo run, dash p, cli, dash dash, layout, list. Same output." |
| 2:24 | Terminal. | Run `asylum layout show swarm`. | "asylum layout show swarm gives you one in full — including its resolved concurrency." |
| 2:44 | Output showing concurrency: 3. | Highlight the concurrency line. | "There it is: five agents, concurrency three. That means three run at once; the other two queue and launch as capacity frees. Lookups are case-insensitive, so SWARM in caps works too." |
| 3:08 | Settings surface, `layouts` key. | Open settings to `layouts`. | "To make your own, set the layouts key in settings. Each entry has a name, a description, the agent ids, and an optional concurrency." |
| 3:32 | Editing in a `quick` and a `bake-off` layout. | Type two custom layouts. | "Tailor them to how you work. A one-agent 'quick' for routine changes. A wider 'bake-off' — five agents, concurrency two — for hard problems where you don't yet know which agent will win." |
| 4:04 | Save; overlay on concurrency rules. | Save the file. | "Concurrency zero defers to your global max-parallel-runs. A non-zero value overrides it for that layout — so a swarm can be wider or narrower than your global default. Omit the layouts key entirely and you keep the built-in three." |
| 4:30 | `asylum layout list` shows the new presets. | Re-run list. | "Save, and the new presets show up immediately in the picker and in asylum layout list, because settings reload live." |
| 4:52 | Compose launched from `bake-off`. | Launch and watch runs queue. | "Launch a hard task from bake-off and watch the extras queue up. Breadth without the cost spike. Next: notes — feeding agents better context than a prompt alone." |

## B-roll / capture notes
- Show both the in-app layout chips and the `asylum layout list` / `show` CLI output so viewers see they're the same data.
- Pre-write the `quick` and `bake-off` layouts so the on-camera edit is quick; then save and show them appearing in `list`.
- For the queueing beat, launch a `bake-off` with concurrency lower than the agent count so a couple of runs visibly sit `queued`.
- Reminder for the recorder: `asylum layout` reads `settings.json`, so save before you run it.

## Recap card (end screen)
- A layout is a named preset: a set of agents + an optional concurrency cap.
- Built-ins: `duel`, `triad`, `swarm`. Override them with the `layouts` key.
- `concurrency: 0` defers to `max_parallel_runs`; non-zero overrides it for that layout.
- Inspect with `asylum layout list` and `asylum layout show <name>` (case-insensitive).

## Next
- [Episode 08 — Notes and Knowledge](08-notes-and-knowledge.md)

Go deeper: [book chapter 5](../book/05-layouts-and-presets.md).
