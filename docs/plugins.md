# Plugins

Asylum plugins use a `plugin.toml` manifest model: a plugin is a directory
with a `plugin.toml` manifest that contributes extension points, plus an optional
runtime process the app talks to over JSON.

Plugins live under `$XDG_DATA_HOME/asylum/plugins/<id>/` (see
`plugin::default_dir`). Discovery loads the good ones and reports a diagnostic
per bad manifest — a broken plugin never blocks the others.

## Installing and discovering

Because a plugin is just a directory with a `plugin.toml`, installing one is a
shallow clone into the plugins directory:

```sh
asylum plugin install <owner>/<repo>        # e.g. asylum plugin install acme/asylum-reviewr
asylum plugin install <owner>/<repo>@<ref>  # pin a branch, tag, or commit
asylum plugin list                          # what's installed (and any diagnostics)
asylum plugin search                        # community plugins, by topic
```

Community plugins tag their repository with the GitHub topic `asylum-plugin`;
`asylum plugin search` lists them via the `gh` CLI. Install refuses a
destination that already exists and rejects a repo with no `plugin.toml`, so a
non-plugin repo can't masquerade as one (`plugin::install`).

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
# Parsed and validated; the app does not render it yet.
[panel]
id = "issues"
title = "Issues"
icon = "◪"

# A native web surface (panel | tab | window). Source is url | entry | service.
# Parsed and validated; the app does not render it yet.
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
# Dispatched by the app once the plugin is enabled — the optional `when` filter
# matches the event's status (`zero`/`nonzero` alias `success`/`failure`).
[[trigger]]
on = "run_finished"
when = "nonzero"
invoke = "on_run_failed"

# A tool exposed to the coding agents themselves.
# Parsed and validated; the app does not expose tools to agents yet.
[[tool]]
id = "create_issue"
description = "Create a Linear issue from the current task."
param = [{ name = "title", type = "string", required = true }]
```

## What reaches the user today

Two of the five contribution types are wired end to end. **`[[command]]`**: the
Plugins surface lists an installed plugin and the app invokes its commands
through the runtime. **`[[trigger]]`**: the app dispatches triggers on ADE
events — but only for a plugin the user has **enabled** (see "Enabling and
trust" below), and off the UI thread with a per-invocation timeout, so a slow or
hung plugin never wedges the fleet and a failure surfaces as an Inbox
notification naming the plugin.

`[panel]`, `[webview]`, and `[[tool]]` still parse and validate — the manifest
vocabulary is stable and a plugin can declare them today — but nothing in the
app renders a panel or webview or offers a tool to an agent yet. The Plugins
surface labels them "not yet active" rather than implying they run. Their host
dispatch is on the roadmap (`docs/roadmap.md`).

## Enabling and trust

Every plugin is **disabled by default** and inert until the user enables it in
the Plugins surface: a disabled plugin's triggers never fire and its commands
never run. The enabled set is persisted as `enabled_plugins` in `settings.json`.

Enabling a **process** runtime — which runs with the user's full privileges and
whose capabilities are only advisory — requires confirming a trust disclosure
that restates the exact command and its authority. A **WASM** (or runtime-less)
plugin is capability-sandboxed and enables directly. Disabling takes effect on
the next event.

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

A `[[trigger]]` may name any of these events, and the parser validates the name:

`task_created`, `run_started`, `run_finished`, `run_failed`,
`worktree_created`, `worktree_removed`, `diff_ready`, `task_merged`.

The app emits these to enabled plugins. A terminal run fires `run_finished`
(with `status` `success`/`failure`) for every completion — a failure additionally
fires `run_failed`, and a success additionally fires `diff_ready`. Each event
carries a JSON payload with the fields it knows: `event`, and where applicable
`task`, `run`, `project` (repo path), `worktree`, `status`, and `code`. A
trigger's optional `when` filter is matched against `status`, with `zero` and
`nonzero` accepted as aliases for `success` and `failure`.

## Runtime protocol

The app speaks newline-delimited JSON to a `[runtime]` process. Each request is
`{ "id": N, "method": "...", "params": {...} }`; each reply is
`{ "id": N, "result": {...} }` or `{ "id": N, "error": "..." }`. Lines that are
not JSON objects (a runtime's stray logging) are ignored. `pluginrt` provides
`invoke_once` for one-shot calls and `Session` for a warm persistent runtime.

## WASM tier

`pluginrt::invoke_wasm` loads `type = "wasm"` runtimes under `wasmi`. Guests use
a linear-memory string ABI and export `alloc` and `invoke`. The host links only
the functions allowed by the manifest's capabilities, so an undeclared host
function cannot be imported. Each call runs fuel-metered with memory, table,
log, and response-size limits, and the module path is contained to the plugin
directory (no absolute/`..`/symlink escape).

## Trust model

The two runtimes have very different trust:

- **`process` runtimes are fully trusted.** A process plugin is an ordinary
  child process with your full user privileges — filesystem, network,
  subprocesses. Its declared `capabilities` are *advisory only*; nothing
  enforces them. Enable a process plugin only if you trust its source. The
  Plugins surface shows the exact command and a trust warning
  (`Runtime::trust_summary` / `RuntimeKind::is_trusted`), and the app scrubs its
  environment to a small allowlist before launch, so app secrets
  (`ASYLUM_CONTROL_TOKEN`, `ASYLUM_LINEAR_TOKEN`, cloud/CI credentials) are not
  exported into it.
- **`wasm` runtimes are capability-sandboxed.** A WASM plugin has no ambient
  authority: it can import only the host functions its capabilities grant, and
  runs under fuel/memory/time/log/response bounds. Prefer WASM for third-party
  plugins.
