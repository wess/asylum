# Chapter 5: Layouts and Presets

Ticking the same set of agents for every task gets old fast. A **layout** is a
named fan-out preset — a fixed set of agents (and an optional concurrency cap)
you can pick in one gesture. This chapter covers the built-in layouts, defining
your own, how concurrency interacts with the global limit, and the
`asylum layout` command.

## What a layout is

A layout is *data, not a keybinding*. It answers one question: **which agents
race this task, and how many at a time?** Picking a layout when composing a task
fans it out across every agent it lists, so the same task shape can be re-run
without re-ticking boxes each time.

Each layout has four fields:

- **name** — a stable, human-facing label (used in the picker and by the CLI).
- **description** — a one-line summary of what the preset is for.
- **agents** — the agent ids that each get a run.
- **concurrency** — the maximum simultaneous runs for a task launched from this
  layout. `0` defers to the global `max_parallel_runs`.

## The built-in layouts

Out of the box, before you define any of your own, Asylum ships three:

| Name    | Agents                                             | Concurrency |
|---------|----------------------------------------------------|-------------|
| `duel`  | `claude-code`, `codex`                             | all at once |
| `triad` | `claude-code`, `codex`, `aider`                    | all at once |
| `swarm` | `claude-code`, `codex`, `opencode`, `gemini`, `aider` | 3 at a time |

`duel` is a fast head-to-head between two frontier agents. `triad` gets three
takes on one prompt. `swarm` casts a wide net across five agents but keeps only
three running at once, so you get breadth without overwhelming your machine or
your quota.

## Defining your own

Set the `layouts` key in `settings.json` to replace the built-ins with your own
list. This mirrors the shipped shape:

```jsonc
{
  "layouts": [
    { "name": "duel",  "description": "Two frontier agents, head to head.",
      "agents": ["claude-code", "codex"] },
    { "name": "triad", "description": "Three takes on one prompt.",
      "agents": ["claude-code", "codex", "aider"] },
    { "name": "swarm", "description": "A wide net; three at a time.",
      "agents": ["claude-code", "codex", "opencode", "gemini", "aider"],
      "concurrency": 3 }
  ]
}
```

Because layouts are data, tailor them to how you work. A few ideas:

```jsonc
{
  "layouts": [
    // A cheap, fast first pass on routine changes.
    { "name": "quick", "description": "One reliable agent.",
      "agents": ["claude-code"] },

    // A broad tournament for hard problems, two at a time to save quota.
    { "name": "bake-off", "description": "Five agents, two at a time.",
      "agents": ["claude-code", "codex", "opencode", "gemini", "cursor-agent"],
      "concurrency": 2 },

    // Local/offline-leaning agents only.
    { "name": "local", "description": "Local-first agents.",
      "agents": ["aider", "goose"] }
  ]
}
```

Omit `layouts` entirely and you keep the built-in `duel` / `triad` / `swarm`.

## Concurrency and the global limit

Two limits can apply to a task:

- **Global**: `max_parallel_runs` caps concurrent runs across *all* tasks.
- **Per-layout**: a layout's `concurrency` caps concurrent runs for a task
  launched from *that* layout.

A layout's `concurrency` of `0` means "use the global cap." A non-zero value
lets a specific preset be wider or narrower than your global default — a `swarm`
of five with `concurrency: 3` runs three at a time even if your global
`max_parallel_runs` is higher. Runs beyond the effective cap simply queue and
launch as capacity frees, exactly as in
[Chapter 4](04-the-fleet-in-depth.md).

## Inspecting layouts from the CLI

The `asylum layout` command reads your settings so you can check presets from the
shell or a script:

```sh
asylum layout list
```

```
duel       Two frontier agents, head to head.     [claude-code, codex]
triad      Three takes on one prompt.              [claude-code, codex, aider]
swarm      A wide net; three at a time.            [claude-code, codex, opencode, gemini, aider]
```

And to see one in full, including its resolved concurrency:

```sh
asylum layout show swarm
```

```
name:        swarm
description: A wide net; three at a time.
agents:      claude-code, codex, opencode, gemini, aider
concurrency: 3
```

Layout lookups are case-insensitive, so `asylum layout show SWARM` works too.

## Try it

1. Add a `quick` layout with a single agent and a `bake-off` layout with four.
2. Save `settings.json` and confirm both appear in `asylum layout list`.
3. Compose a small task and launch it from `quick`; compose a harder one and
   launch it from `bake-off` with a low concurrency, watching runs queue.

## Recap

- A layout is a named preset: a set of agents plus an optional concurrency cap.
- Built-ins are `duel`, `triad`, and `swarm`; override them with the `layouts`
  key.
- `concurrency: 0` defers to `max_parallel_runs`; a non-zero value overrides it
  for that layout.
- `asylum layout list` and `asylum layout show <name>` inspect them.

## Next

[Chapter 6: Diffs, Checks, and Review](06-diffs-checks-and-review.md) is where
you decide which of a layout's runs actually wins.
