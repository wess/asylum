# Notes, context, and reusable workflows

Intermediate · Episode 8

## 1. The Notes surface.

Agents do better work with better context. Notes is a project knowledge vault made of ordinary Markdown — and it feeds straight into the fleet.

## 2. A .md file open in the editor.

These are plain Markdown files on disk — the source of truth. Asylum's database stores only the vault choice and note attachments, never the note bodies. Your notes stay readable, diffable, and portable.

## 3. Vault toggle: private vs. repository.

Every project starts with a private vault only you see. From the toolbar you can switch to the repository's notes folder, so the team can version notes alongside the code.

## 4. Switching vaults; copy-not-delete callout.

Switching copies missing Markdown into the target and never deletes the source. If a file already exists there, the conflict is kept and reported — never overwritten.

## 5. New note from a template picker.

New notes can start from a template. Asylum ships a few shapes — task, decision, investigation, retrospective — so recurring documents get a consistent skeleton.

## 6. YAML frontmatter block.

A note can open with a YAML frontmatter block, shown in the UI as editable properties — a title, a type, tags.

## 7. Inline #tags in the body.

Tags work in the frontmatter or inline in the body with a hash. Click a tag and the vault filters to notes carrying it.

## 8. Typing [[ for autocomplete.

Type double square brackets to link another note. Links resolve by title, file stem, or path, and you can add an alias after a pipe.

## 9. Links pane showing backlinks.

The Links pane shows a note's outgoing links and its backlinks — every note that links to it — so the vault becomes a navigable web. Rename a note and the incoming links update across the whole vault automatically. Links never rot.

## 10. Preview render: callouts, Mermaid, code.

In preview it renders richly — callouts, Mermaid diagrams, syntax-highlighted code — and clicking a link opens the target inside Notes.

## 11. Attach-to-task action.

Here's where the vault earns its place. Notes aren't just documentation — they're context you hand to agents. Attach a note to a task.

## 12. Fan out the task; note inherited.

A note attached to a task is inherited by every fan-out run it generates, and its Markdown is appended to the launch prompt. So a design note or a spec becomes part of what every racing agent reads.

## 13. Send-selection from a note to a run.

You can also send an exact editor selection from a note to a run. A live, stdin-capable agent gets it immediately; a finished run starts a new attempt with it.

## 14. Note showing appended check/PR links.

And it runs both ways. When checks finish or a PR is created, Asylum writes those links back into the attached notes. Over time a task note accumulates the whole trail — prompt, runs, checks, and the PR that landed.

## 15. Search surface across notes + code.

One last thing: project Search queries notes, files, task prompts, run metadata, and terminal transcripts together. Next: connecting the fleet to GitHub and Linear.

## 16. The task composer with an agent picker.

Ticking the same set of agents for every task gets old fast. A layout is a named fan-out preset — a fixed set of agents, and an optional concurrency cap, that you pick in one gesture.

## 17. Layout chips on the composer.

A layout is data, not a keybinding. It answers one question: which agents race this task, and how many at a time?

## 18. Table overlay of the three built-ins.

Out of the box you get three. Duel — a fast head-to-head, claude-code versus codex. Triad — three takes on one prompt, adding aider. And swarm — a wide net across five agents, but only three running at once.

## 19. Pick duel; fan out.

Pick duel and dispatch. Two runs, no ticking boxes — the same task shape re-run in a click.

## 20. Terminal beside the app.

You can inspect layouts from the shell too. asylum layout list shows each preset — name, description, and the agents it races.

## 21. Terminal output of layout list.

From a source checkout that's cargo run, dash p, cli, dash dash, layout, list. Same output.

## 22. Terminal.

asylum layout show swarm gives you one in full — including its resolved concurrency.

## 23. Output showing concurrency: 3.

There it is: five agents, concurrency three. That means three run at once; the other two queue and launch as capacity frees. Lookups are case-insensitive, so SWARM in caps works too.

## 24. Settings surface, layouts key.

To make your own, set the layouts key in settings. Each entry has a name, a description, the agent ids, and an optional concurrency.

## 25. Editing in a quick and a bake-off layout.

Tailor them to how you work. A one-agent 'quick' for routine changes. A wider 'bake-off' — five agents, concurrency two — for hard problems where you don't yet know which agent will win.

## 26. Save; overlay on concurrency rules.

Concurrency zero defers to your global max-parallel-runs. A non-zero value overrides it for that layout — so a swarm can be wider or narrower than your global default. Omit the layouts key entirely and you keep the built-in three.

## 27. asylum layout list shows the new presets.

Save, and the new presets show up immediately in the picker and in asylum layout list, because settings reload live.

## 28. Compose launched from bake-off.

Launch a hard task from bake-off and watch the extras queue up. Breadth without the cost spike. Next: notes — feeding agents better context than a prompt alone.
