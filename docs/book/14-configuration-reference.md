# Chapter 14: Configuration Reference

This chapter documents every key in `settings.json`, plus the two environment
variables that can stand in for a secret key. It is a reference — read the
sections you need. The file is JSON *with comments* (JSONC), lives at
`$XDG_CONFIG_HOME/asylum/settings.json`, is live-reloaded on save, and is edited
in place (comments preserved) by the in-app Settings surface (`cmd-,`). Every key
is optional: a missing or malformed value falls back to its built-in default and,
if malformed, is reported as a diagnostic rather than aborting the load.

## A complete example

```jsonc
{
  // Chrome theme: "dark" or "light".
  "theme": "dark",

  // Reveal the full activity rail (Notes, Integrations, Terminal, Editor,
  // Preview, Browser, Plugins, Accounts, Inbox) instead of collapsing it
  // behind the rail's "MORE" toggle.
  "sidebar_more": false,

  // Where per-task worktrees are created, relative to a project root.
  "worktree_dir": ".asylum/worktrees",

  // Agents fanned out by default when a task is dispatched.
  "default_agents": ["claude-code", "codex"],

  // Named fan-out presets (see Chapter 5).
  "layouts": [
    { "name": "duel",  "description": "Two frontier agents, head to head.",
      "agents": ["claude-code", "codex"] },
    { "name": "triad", "description": "Three takes on one prompt.",
      "agents": ["claude-code", "codex", "aider"] },
    { "name": "swarm", "description": "A wide net; three at a time.",
      "agents": ["claude-code", "codex", "opencode", "gemini", "aider"],
      "concurrency": 3 }
  ],

  // Concurrency cap across all tasks (0 = unlimited), and per-run timeout in
  // minutes (0 = no timeout).
  "max_parallel_runs": 4,
  "run_timeout_minutes": 60,

  // Per-agent overrides, keyed by agent id.
  "agents": {
    "codex": { "extra_args": ["--model", "o1"] },
    "aider": { "enabled": false }
  },

  // Bring-your-own agents added on top of the built-in catalog.
  "custom_agents": [],

  // Built-in editor.
  "editor": {
    "font_family": "monospace",
    "font_size": 13,
    "tab_width": 4,
    "autosave": true
  },

  // Keybindings layered over the defaults, as "chord=action" strings.
  "keybindings": [],

  // Plugin ids enabled from the Plugins surface. A disabled plugin's triggers
  // never fire and its commands never run (see Chapter 13).
  "enabled_plugins": [],

  // Linear API token. Empty disables Linear.
  "linear_token": "",

  // Mobile companion server. Off by default; a token is required once enabled.
  "companion": {
    "enabled": false,
    "bind": "127.0.0.1:8787",
    "token": ""
  },

  // Agent control surface.
  "control": {
    "enabled": true,
    "bind": "127.0.0.1:8788",
    "token": ""
  },

  // Secrets proxy: masked outbound API access for agents.
  "proxy": {
    "enabled": false,
    "bind": "127.0.0.1:8789"
  },

  // Named upstreams the proxy forwards to. Secret VALUES live in the encrypted
  // keep, never here; `secret` names the keep entry.
  "upstreams": [],

  // MCP gateway: one aggregating MCP server every agent connects to.
  "mcp": {
    "enabled": false,
    "bind": "127.0.0.1:8790",
    "expose": "direct"
  },

  // The upstream MCP servers the gateway aggregates, each under its own
  // namespace (`<name>__<tool>`).
  "mcp_servers": []
}
```

## Top-level keys

### `theme`
String. The chrome theme, `"dark"` (default) or `"light"`. Reloads live.

### `sidebar_more`
Bool. Default `false`. The activity rail shows **Tasks**, **Review**, and
**Search** plus a collapsed **MORE** toggle for the rest of the surfaces
(Notes, Integrations, Terminal, Editor, Preview, Browser, Plugins, Accounts,
Inbox); set `true` to keep them revealed instead. Clicking **MORE** in the rail
toggles this key directly.

### `worktree_dir`
String. Where per-task worktrees are created, relative to a project's root.
Default `".asylum/worktrees"`. This is the folder that fills with isolated
checkouts during a fan-out and empties as clean worktrees are cleaned up after a
merge.

### `default_agents`
Array of agent ids. The agents fanned out by default when a task is dispatched.
Ids come from the built-in registry — for example `claude-code`, `codex`,
`opencode`, `gemini`, `aider`, `cursor-agent`, `copilot`, `goose`. Default is
empty, which means Asylum asks you each time.

### `layouts`
Array of fan-out presets ([Chapter 5](05-layouts-and-presets.md)). Each has
`name`, `description`, `agents`, and optional `concurrency` (0 defers to
`max_parallel_runs`). Omit the key to keep the built-in `duel` / `triad` /
`swarm`. Also editable as a list in **Settings → Layouts** (add/edit/remove),
without hand-editing JSON.

### `max_parallel_runs`
Integer. Maximum concurrent agent runs across all tasks. Default 4; `0` means
unlimited. Runs beyond the cap queue and launch as capacity frees.

### `run_timeout_minutes`
Integer. Stop any run exceeding this many minutes. Default 60; `0` disables the
timeout.

### `enabled_plugins`
Array of plugin ids. Default empty. A plugin is disabled — inert, its
`[[trigger]]`s never fire, its `[[command]]`s never run — until its id is in
this list. Toggled from the Plugins surface, which asks you to confirm a trust
disclosure before adding a `process`-runtime plugin (WASM enables directly).
See [Chapter 13](13-plugins.md).

### `linear_token`
String. Your Linear API token (create one at `https://linear.app/settings/api`).
When set, the Integrations surface browses Linear; empty disables it.

## `agents` — per-agent overrides

A map keyed by agent id. Each entry may set:

- **`program`** (string) — override the launch program, e.g. a wrapper script,
  instead of the agent's default.
- **`extra_args`** (array of strings) — arguments appended to the agent's command
  line. Handy for pinning a model or passing a flag:
  ```jsonc
  "agents": { "codex": { "extra_args": ["--model", "o1"] } }
  ```
- **`enabled`** (bool) — force-enable or force-disable this agent regardless of
  `default_agents`:
  ```jsonc
  "agents": { "aider": { "enabled": false } }
  ```

## `custom_agents` — bring your own agent

An array of agent definitions added on top of the built-in catalog. Also
editable as a list in **Settings → Custom agents** (add/edit/remove), not just
by hand here. A custom agent whose `id` matches a built-in overrides it (the
custom entry wins, in the built-in's position). Each entry:

- **`id`** — stable id, used in fan-out, branch names, and the store.
- **`name`** — display name (defaults to the id if empty).
- **`icon`** — a single-glyph icon (defaults to `•`).
- **`program`** — the program to launch (looked up on PATH).
- **`args`** — the argument template; `{prompt}` is substituted under `arg`
  delivery.
- **`delivery`** — how the prompt reaches the agent: `"arg"` (default, substituted
  into `args` where `{prompt}` appears, or appended if there is no token) or
  `"stdin"` (piped to the process's stdin; `args` used as-is).

```jsonc
"custom_agents": [
  {
    "id": "myagent",
    "name": "My Agent",
    "icon": "★",
    "program": "myagent",
    "args": ["--prompt", "{prompt}"],
    "delivery": "arg"
  }
]
```

## `editor`

Built-in code-editor preferences:

- **`font_family`** (string, default `"monospace"`)
- **`font_size`** (number, default `13`)
- **`tab_width`** (integer, default `4`)
- **`autosave`** (bool, default `true`)

## `keybindings`

An array of `"chord=action"` strings, in gpui keystroke syntax, layered over the
defaults. An empty action (`"chord="`) *unbinds* a default. For example:

```jsonc
"keybindings": [
  "cmd-shift-p=command_palette",
  "cmd-enter="
]
```

The bindable **actions** are:

`command_palette`, `quick_open`, `find_in_project`, `new_task`, `open_project`,
`run_fanout`, `review_diff`, `new_terminal`, `split_right`, `close_tab`,
`settings`, `open_settings_file`, `toggle_theme`, `switch_account`,
`notifications`, `quit`, `tasks`, `search`, `integrations`, `terminal`, `editor`,
`browser`, `preview`, `plugins`.

The last several (`tasks` through `plugins`) switch directly to a surface. Save
the file and bindings reload live.

## `companion` — mobile companion server

See [Chapter 12](12-the-mobile-companion-and-events.md). Also toggled from
**Settings → Servers**, alongside `control` below.

- **`enabled`** (bool, default `false`) — whether the server runs. Off by
  default: it exposes projects, tasks, runs, and notifications, and accepts
  follow-ups into a live agent, so it only starts once you opt in.
- **`bind`** (string, default `"127.0.0.1:8787"`) — the bind address. Use
  `"0.0.0.0:8787"` to also reach it from a phone on the LAN.
- **`token`** (string, default empty) — bearer token, required whenever the
  server is enabled, **loopback or not**: an empty token is refused at startup
  and the server does not run (it would let any local process read your store
  and inject follow-ups into a live agent). The refusal appears in the Inbox
  rather than failing silently. Sent as `Authorization: Bearer <token>` on
  every `/api/*` request.

## `control` — agent control surface

See [Chapter 11](11-agent-orchestration-and-the-control-surface.md). Lets a
running agent orchestrate the fleet from inside its worktree. Also toggled from
**Settings → Servers**, alongside `companion` above.

- **`enabled`** (bool, default `true`) — whether the control server runs. When
  off, `asylum control` commands report that they are not inside a worktree.
- **`bind`** (string, default `"127.0.0.1:8788"`) — the bind address.
  **Loopback-only, enforced**: a non-loopback bind is refused at startup and the
  server does not run — the refusal appears in the Inbox. The surface can spawn
  runs, so it is never exposed to the network. Agents reach it at
  `127.0.0.1:<port>`.
- **`token`** (string, default empty) — bearer token. **The control surface is
  always authenticated.** Empty does *not* mean "no auth": when you leave it
  empty the app provisions a strong per-session token, kept in memory and never
  written to disk. Either way, the token is injected into each agent as
  `ASYLUM_CONTROL_TOKEN`, and requests must present it. Localhost is not treated
  as an authentication boundary here, because anything running on your machine
  could otherwise spawn runs. Set the key explicitly only if you need a stable
  token across sessions (for a script outside the fleet); a per-session token is
  the better default.

## `proxy` — secrets proxy

Masked outbound API access for agents ([Chapter 11](11-agent-orchestration-and-the-control-surface.md);
`docs/secrets.md`). An agent calls a named upstream (`asylum call openai POST
/v1/chat/...`), Asylum resolves the credential from the encrypted keep and
injects it server-side, and forwards only to that upstream's host — so the agent
uses a key it never sees and cannot redirect.

- **`enabled`** (bool, default `false`) — whether the proxy runs. Off by default;
  it only does something once you define `upstreams`.
- **`bind`** (string, default `"127.0.0.1:8789"`) — the bind address.
  **Loopback-only, enforced**: a non-loopback bind is refused at startup. Like
  the control surface, the proxy is always authenticated — each run gets a signed
  token naming its project, injected as `ASYLUM_PROXY_TOKEN` alongside
  `ASYLUM_PROXY_URL`.

## `upstreams` — what the proxy may forward to

An array of named upstreams. Also editable as a list in **Settings → Secrets
proxy** (add/edit/remove), not just by hand here. Each binds a stored secret to
a fixed destination.
Secret *values* never appear in `settings.json` — they live in the encrypted keep
(`~/.config/asylum/keep.enc`, managed with `asylum keep set <name>` and unlocked
with `ASYLUM_KEEP_PASSPHRASE`); `secret` only names the keep entry.

- **`name`** (string) — the name the agent addresses (`/<name>/...`). Lowercase
  slug.
- **`base_url`** (string) — the upstream base URL, e.g.
  `"https://api.openai.com"`. Requests forward to `base_url` + the path after
  `/<name>`, and only this host ever receives the secret.
- **`secret`** (string) — which keep entry to inject, resolved against the calling
  agent's project.
- **`header`** (string, default `"Authorization"`) — the header the secret goes
  into.
- **`format`** (string, default `"Bearer {secret}"`) — how the header value is
  formatted; `{secret}` is replaced with the resolved value.
- **`project`** (integer, default `0`) — the project this upstream belongs to, or
  `0` for a global upstream available to every project. A project-scoped upstream
  overrides a global one of the same name for that project.

```jsonc
"upstreams": [
  {
    "name": "openai",
    "base_url": "https://api.openai.com",
    "secret": "openai",
    "header": "Authorization",
    "format": "Bearer {secret}",
    "project": 0
  }
]
```

## `mcp` — MCP gateway

One aggregating [Model Context Protocol](https://modelcontextprotocol.io) server
that every agent connects to, fronting the servers in `mcp_servers` under
per-service namespaces (a `create_pr` tool on the `github` server is exposed as
`github__create_pr`, and a call to it is routed back). See `docs/mcp.md`. The
gateway toggles and the server list are both editable in **Settings → MCP
gateway** — add, edit, or remove a server without leaving Settings — or by hand
in this file.

- **`enabled`** (bool, default `false`) — whether the gateway runs. Off by
  default; it only does something once you define `mcp_servers`.
- **`bind`** (string, default `"127.0.0.1:8790"`) — the bind address.
  **Loopback-only, enforced**. Like the proxy, the gateway is always
  authenticated — each run gets a signed token naming its project and run,
  injected as `ASYLUM_MCP_TOKEN` alongside `ASYLUM_MCP_URL` (the MCP endpoint is
  that URL plus `/mcp`).
- **`expose`** (string, default `"direct"`) — how tools are surfaced.
  `"direct"` lists every upstream tool, namespaced. `"search"` advertises only
  `asylum_find_tool` / `asylum_call_tool`, so tool definitions load on demand —
  which keeps a wide fleet's context small.

## `mcp_servers` — the servers the gateway aggregates

An array of upstream MCP servers. Each is either a local process
(`transport: "stdio"`) or a remote endpoint (`transport: "http"`), exposed under
its `name` as a namespace.

- **`name`** (string) — the namespace agents see (`<name>__<tool>`). Lowercase
  slug (`[a-z0-9-]`, no `__`).
- **`transport`** (string, default `"stdio"`) — `"stdio"` or `"http"`.
- **`command`** / **`args`** (stdio) — the program to launch and its arguments.
- **`url`** (http) — the MCP endpoint URL.
- **`env`** (stdio, object) — extra environment for the child. A value of the
  form `"{secret:NAME}"` is resolved from the keep at spawn (scoped to the
  server's project); any other value is literal.
- **`secret`** (http) — a keep entry injected as the auth header (never stored
  here). Empty means no auth.
- **`header`** (http, default `"Authorization"`) / **`format`** (http, default
  `"Bearer {secret}"`) — how the secret is injected.
- **`allow`** / **`deny`** (arrays) — expose only these tool names, or hide
  these (by their *upstream* name, before namespacing).
- **`project`** (integer, default `0`) — the project this server belongs to, or
  `0` for a global server. A project-scoped server shadows a global one of the
  same name.
- **`enabled`** (bool, default `true`) — set `false` to keep a server's config
  without aggregating it.

```jsonc
"mcp_servers": [
  {
    "name": "github",
    "command": "gh-mcp",
    "args": ["--stdio"],
    "env": { "GITHUB_TOKEN": "{secret:github_pat}" }
  },
  {
    "name": "docs",
    "transport": "http",
    "url": "https://mcp.example.com/mcp",
    "secret": "docs_token"
  }
]
```

## Environment overrides

Two secret keys can be filled from the environment instead of the file, so the
value never has to be committed or synced. A configured (non-empty) value in
`settings.json` always wins; a blank override is ignored.

| Key | Environment variable |
|---|---|
| `linear_token` | `ASYLUM_LINEAR_TOKEN` |
| `companion.token` | `ASYLUM_COMPANION_TOKEN` |

Leave the key empty in the file and export the variable to use it. Related but
separate: `ASYLUM_KEEP_PASSPHRASE` unlocks the encrypted keep, and is not a
`settings.json` key at all.

## A minimal, opinionated starting config

You do not need most of these keys. A practical starter that overrides only what
matters:

```jsonc
{
  "theme": "dark",
  "default_agents": ["claude-code", "codex"],
  "max_parallel_runs": 3,
  "run_timeout_minutes": 45
}
```

Everything else — layouts, companion, control, proxy, editor — takes sensible
defaults. Add keys as you hit a reason to.

## Try it

1. Add `"agents": { "codex": { "extra_args": ["--model", "o1"] } }` and run a
   task; confirm the extra args appear in the launched command (the run card /
   `asylum run` preview echoes the command).
2. Unbind a default and rebind it: add `"cmd-shift-p=command_palette"` and watch
   it take effect on save.
3. Define a `custom_agents` entry wrapping a script and fan a task out to it.

## Recap

- `settings.json` is JSONC, live-reloaded, comment-preserving; every key is
  optional.
- Top-level: `theme`, `sidebar_more`, `worktree_dir`, `default_agents`,
  `layouts`, `max_parallel_runs`, `run_timeout_minutes`, `enabled_plugins`,
  `linear_token`.
- `agents` overrides per agent (`program`, `extra_args`, `enabled`);
  `custom_agents` adds your own (`id`/`program`/`args`/`delivery`).
- `editor`, `keybindings` (chord=action, empty unbinds), `companion`, `control`,
  `proxy`, `upstreams`, `mcp`, and `mcp_servers` round out the file.
- The control surface, the proxy, and the MCP gateway are loopback-only
  (enforced) and *always* authenticated — an empty `control.token` provisions a
  per-session token, it does not disable auth. The companion is off by default
  and, once enabled, is also always authenticated — an empty `companion.token`
  refuses to start, loopback or not.
- `linear_token` and `companion.token` fall back to `ASYLUM_LINEAR_TOKEN` and
  `ASYLUM_COMPANION_TOKEN` when left empty.

## Next

[Chapter 15: Expert Workflows](15-expert-workflows.md) puts the whole book to
work.
