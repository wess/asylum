# Chapter 13: Plugins

Asylum is extensible. A **plugin** is a directory with a `plugin.toml` manifest
that contributes palette commands, a panel, a webview, event triggers, and tools
for the agents — optionally backed by a runtime process (or a sandboxed WASM
module) the app talks to over JSON. This chapter covers the manifest, the two
runtimes, installing and discovering plugins, and writing a simple one.

## What a plugin is

A plugin is just a **directory containing a `plugin.toml`**. Plugins live under
your data directory, at `$XDG_DATA_HOME/asylum/plugins/<id>/`. On startup Asylum
scans that directory, loads the good ones, and reports a diagnostic for each bad
manifest — a single broken plugin never blocks the others.

## The manifest

`plugin.toml` declares identity, capabilities, and extension points. Here is a
fully annotated example:

```toml
id = "linear"
name = "Linear"
version = "1.0.0"
description = "Browse issues and PRs without leaving the fan-out board."
capabilities = ["network", "notify"]

# A runtime the app invokes over JSON on stdin/stdout. `persistent = true` keeps
# it warm across calls instead of spawning per event.
[runtime]
type = "process"          # or "wasm"
command = "bun run server.ts"
persistent = true

# A side-drawer panel rendered from the runtime's responses.
[panel]
id = "issues"
title = "Issues"
icon = "◪"

# A native web surface (placement: panel | tab | window; source: url | entry | service).
[webview]
id = "board"
title = "Board"
placement = "tab"
url = "https://example.com"

# A palette command. `mode` = invoke (default) | panel | webview.
[[command]]
id = "sync"
title = "Sync Issues"
run = "sync"
keybind = "cmd-shift-l"

# A hook on an ADE event. Action is `notify` or `invoke` (a runtime method).
[[trigger]]
on = "run_finished"
when = "nonzero"
invoke = "on_run_failed"

# A tool exposed to the coding agents themselves.
[[tool]]
id = "create_issue"
description = "Create an issue from the current task."
param = [{ name = "title", type = "string", required = true }]
```

### Extension points

A manifest describes five contribution types. Two run today; be clear on which
as you read:

- **`[[command]]`** — a command-palette action, wired end to end. Its `mode`
  decides what running it does: `invoke` calls a runtime method, `panel` opens
  the plugin's panel, `webview` opens its web surface. It can carry a
  `keybind`. Only `invoke` has a host behind it today.
- **`[panel]`** — a side-drawer panel rendered from the runtime's responses.
  Declared and validated; the app does not render it yet.
- **`[webview]`** — a native web surface placed as a panel, tab, or window,
  sourced from a `url`, a bundled `entry`, or a `service`. Declared and
  validated; the app does not render it yet.
- **`[[trigger]]`** — a hook that fires on an ADE event, for a plugin you have
  **enabled** (see "Enabling and trust" below). Its action is `notify` (post a
  desktop notification) or `invoke` (call a runtime method), and it can be
  conditioned with `when`. Dispatch runs off the UI thread with a
  per-invocation timeout, so a slow or hung plugin never wedges the fleet, and
  a failure surfaces as an Inbox notification naming the plugin.
- **`[[tool]]`** — a tool meant to be exposed to the coding agents, with typed
  parameters. Declared and validated; the app does not offer plugin tools to
  agents yet.

Treat the three still-unwired points — `[panel]`, `[webview]`, `[[tool]]` — as
available at the manifest level rather than as working features: the
vocabulary is stable and your manifest will validate, but host dispatch for
them is still on the roadmap (`docs/roadmap.md`). The Plugins surface labels
each "not yet active" rather than implying it runs.

### Enabling and trust

A plugin is **disabled by default**: install it and it sits inert until you
turn it on in the Plugins surface, at which point its `[[trigger]]`s start
firing and its `[[command]]`s become runnable. The enabled set is persisted as
`enabled_plugins` in `settings.json`. Enabling a **process** runtime — which
runs with your full user privileges and whose `capabilities` are only
advisory — asks you to confirm a trust disclosure naming its exact launch
command first. A **WASM** (or runtime-less) plugin is capability-sandboxed and
enables directly, no confirmation needed. Disabling takes effect on the next
event.

### Trigger events

A `[[trigger]]` may name any of these ADE events, and the parser checks the name:

`task_created`, `run_started`, `run_finished`, `run_failed`, `worktree_created`,
`worktree_removed`, `diff_ready`, `task_merged`.

The app emits these live to every **enabled** plugin. A terminal run fires
`run_finished` (carrying a `success`/`failure` status) on every completion — a
failure additionally fires `run_failed`, and a success additionally fires
`diff_ready`. A trigger's optional `when` filter matches against that status,
with `zero`/`nonzero` accepted as aliases for `success`/`failure`.

## Capabilities

A plugin declares what it is allowed to touch with `capabilities = [...]`:

| capability   | grants                                |
|--------------|---------------------------------------|
| `git`        | read/modify worktrees and branches    |
| `agents`     | start / inspect agent runs            |
| `store`      | read tasks and runs                   |
| `network`    | make network requests                 |
| `filesystem` | read or write files                   |
| `clipboard`  | read or write the clipboard           |
| `notify`     | post desktop notifications            |

Capabilities are **advisory** under the process runtime (which runs with your
full user privileges) but are the **enforced** gate under the WASM runtime, as
described below.

## The two runtimes

### Process runtime

The app speaks **newline-delimited JSON** to a `[runtime]` process over
stdin/stdout. Each request is `{ "id": N, "method": "...", "params": {...} }`;
each reply is `{ "id": N, "result": {...} }` or `{ "id": N, "error": "..." }`.
Lines that are not JSON objects (a runtime's stray logging) are ignored, so your
runtime can print debug output freely. A one-shot call spawns the runtime, sends
one request, and reads one reply; a `persistent = true` runtime is kept warm
across many calls instead of respawning per event.

Because it is a normal process, you can write a process runtime in anything —
Bun/TypeScript is a natural fit here.

### WASM runtime

A `type = "wasm"` runtime runs a WebAssembly module under a sandbox (`wasmi`).
Guests use a linear-memory string ABI and export two functions, `alloc` and
`invoke`. The crucial property is capability enforcement: the host **links only
the host functions allowed by the manifest's declared capabilities**, so a guest
that never asked for `notify` cannot import the notify host function at all. The
sandbox makes WASM the right tier for untrusted or shared plugins.

## Installing and discovering plugins

Because a plugin is just a directory, "installing" one is a shallow clone into the
plugins directory. Use the CLI ([Chapter 10](10-the-cli.md)):

```sh
# Install owner/repo (optionally at a branch/tag/commit) into the plugins dir.
asylum plugin install acme/asylum-linear
asylum plugin install acme/asylum-linear@v1.2.0

# List what's installed (and report any broken manifests).
asylum plugin list

# Discover community plugins by GitHub topic.
asylum plugin search --limit 20
```

`install` shallow-clones the repo and refuses if the destination already exists
or the cloned repo has no `plugin.toml` — so you cannot accidentally install a
non-plugin. `search` finds repositories tagged with the GitHub **topic
`asylum-plugin`** (via the `gh` CLI, so authenticate `gh` first). Tag your own
plugin repo with that topic to make it discoverable.

## Writing a simple plugin

A minimal process plugin is a directory with a manifest and a runtime script.
`plugin.toml`:

```toml
id = "hello"
name = "Hello"
version = "0.1.0"
description = "A one-command example plugin."
capabilities = ["notify"]

[runtime]
type = "process"
command = "bun run runtime.ts"

[[command]]
id = "greet"
title = "Say Hello"
run = "greet"
```

`runtime.ts` reads newline-JSON requests on stdin and writes replies on stdout:

```ts
// Read one JSON request per line; answer each.
for await (const line of console) {
  let req;
  try { req = JSON.parse(line); } catch { continue; } // ignore non-JSON logging
  if (req.method === "greet") {
    process.stdout.write(JSON.stringify({ id: req.id, result: { text: "Hello!" } }) + "\n");
  } else {
    process.stdout.write(JSON.stringify({ id: req.id, error: "unknown method" }) + "\n");
  }
}
```

Drop the directory under the plugins path (or `asylum plugin install` it), and
"Say Hello" appears in the command palette. From there you can grow it: add a
`[panel]` to render results, a `[[trigger]]` to react to `run_finished`, or a
`[[tool]]` so agents can call it.

## Try it

1. `asylum plugin search --limit 10` to see community plugins tagged
   `asylum-plugin`.
2. Create the `hello` plugin above under your plugins directory, confirm it
   shows in `asylum plugin list`, then enable it from the Plugins surface (a
   process runtime like this one asks you to confirm a trust disclosure first).
3. Run its command from the palette; then add a `[[trigger]]` on `run_finished`
   and watch it fire when a run completes.

## Recap

- A plugin is a directory with a `plugin.toml` under the plugins path; bad
  manifests are reported, not fatal.
- The manifest contributes commands, a panel, a webview, triggers, and tools, and
  declares capabilities.
- A plugin is disabled until you enable it in the Plugins surface — a process
  runtime asks you to confirm a trust disclosure first — after which its
  commands run and its triggers fire live on ADE events.
- The process runtime speaks newline-JSON over stdio (one-shot or warm); the WASM
  runtime is sandboxed and enforces capabilities by linking only allowed host
  functions.
- Install with `asylum plugin install owner/repo[@ref]`; discover by the
  `asylum-plugin` GitHub topic.

## Next

[Chapter 14: Configuration Reference](14-configuration-reference.md) documents
every settings key in one place.
