# Plugins

Asylum plugins use a `plugin.toml` manifest model: a plugin is a directory
with a `plugin.toml` manifest that contributes extension points, plus an optional
runtime process the app talks to over JSON.

Plugins live under `$XDG_DATA_HOME/asylum/plugins/<id>/` (see
`plugin::default_dir`). Discovery loads the good ones and reports a diagnostic
per bad manifest — a broken plugin never blocks the others.

## Manifest

```toml
id = "linear"
name = "Linear"
version = "1.0.0"
description = "Browse issues and PRs without leaving the fan-out board."
capabilities = ["network", "notify"]

# A runtime the app invokes over JSON on stdin/stdout. `persistent = true` keeps
# it warm across calls instead of spawning per event.
[runtime]
type = "process"          # or "wasm" (declared; execution planned)
command = "bun run server.ts"
persistent = true

# A side-drawer panel rendered from the runtime's responses.
[panel]
id = "issues"
title = "Issues"
icon = "◪"

# A native web surface (panel | tab | window). Source is url | entry | service.
[webview]
id = "board"
title = "Board"
placement = "tab"
url = "https://linear.app"

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
description = "Create a Linear issue from the current task."
param = [{ name = "title", type = "string", required = true }]
```

## Capabilities

Declared with `capabilities = [...]`. Advisory under the process runtime (which
runs with full user privileges); the enforced gate list under the WASM runtime.

| capability   | grants |
|--------------|--------|
| `git`        | read/modify worktrees and branches |
| `agents`     | start / inspect agent runs |
| `store`      | read tasks and runs |
| `network`    | make network requests |
| `filesystem` | read or write files |
| `clipboard`  | read or write the clipboard |
| `notify`     | post desktop notifications |

## Trigger events

`task_created`, `run_started`, `run_finished`, `run_failed`,
`worktree_created`, `worktree_removed`, `diff_ready`, `task_merged`.

## Runtime protocol

The app speaks newline-delimited JSON to a `[runtime]` process. Each request is
`{ "id": N, "method": "...", "params": {...} }`; each reply is
`{ "id": N, "result": {...} }` or `{ "id": N, "error": "..." }`. Lines that are
not JSON objects (a runtime's stray logging) are ignored. `pluginrt` provides
`invoke_once` for one-shot calls and `Session` for a warm persistent runtime.

## WASM tier (planned)

A `type = "wasm"` runtime is parsed and validated today but not yet executed —
`pluginrt` returns `Unsupported`. The plan: a wasmtime
component-model host with capability-gated host imports, so a guest can only
reach the interfaces whose capability it declared and was granted.
