# MCP gateway

Asylum runs **one** Model Context Protocol server that every agent connects to,
fronting all your configured MCP servers behind a single connection. Instead of
teaching Claude Code, Codex, Gemini, and every other agent about GitHub, Linear,
your database server, and so on — one config per agent, times every agent in the
fleet — you configure the servers once in Asylum and each agent connects to the
gateway.

## How it works

Each upstream server is exposed under its own **namespace**. A `create_pr` tool
on the `github` server is presented to the agent as `github__create_pr`; a call
to that name is routed back to the `github` server as `create_pr`. Resource URIs
and prompt names are namespaced the same way.

```
agent → http://127.0.0.1:8790/mcp        (one connection, per-run token)
gateway
  ├─ github  (stdio: gh-mcp)      → github__create_pull_request, github__list_issues, …
  ├─ linear  (stdio: linear-mcp)  → linear__create_issue, …
  └─ docs    (http: mcp.acme.com) → docs__search, …
```

The gateway is **loopback-only** and **token-authenticated**, like the control
surface and the secrets proxy. Each run gets a signed token naming:

- its **project** — which servers it may see (a project-scoped server shadows a
  global one of the same name), and
- its **run** — so every tool call is attributable to one run (see *Auditing*).

Asylum injects `ASYLUM_MCP_URL` and `ASYLUM_MCP_TOKEN` into each run's
environment. An agent reaches the endpoint at `$ASYLUM_MCP_URL/mcp` with
`Authorization: Bearer $ASYLUM_MCP_TOKEN`.

## Configuring servers

The gateway toggles — enable, bind address, and exposure mode — live in
**Settings → MCP gateway**, which also lists the configured servers and their
status. The server list itself is edited in `settings.json`.

Enable the gateway and list your servers in `settings.json`:

```jsonc
{
  "mcp": {
    "enabled": true,
    "bind": "127.0.0.1:8790",
    // "direct" lists every tool; "search" advertises just a find/call pair so
    // tool definitions load on demand (keeps a wide fleet's context small).
    "expose": "direct"
  },
  "mcp_servers": [
    // A local (stdio) server. `{secret:NAME}` env values are resolved from the
    // keep at spawn, scoped to this server's project.
    {
      "name": "github",
      "command": "gh-mcp",
      "args": ["--stdio"],
      "env": { "GITHUB_TOKEN": "{secret:github_pat}" }
    },
    // A remote (HTTP) server. `secret` is resolved from the keep and injected as
    // the auth header; the agent never sees it.
    {
      "name": "docs",
      "transport": "http",
      "url": "https://mcp.acme.com/mcp",
      "secret": "docs_token"
    },
    // Only expose some tools, hide others, and scope a server to one project.
    {
      "name": "db",
      "command": "pg-mcp",
      "allow": ["query"],
      "deny": ["drop_table"],
      "project": 3
    }
  ]
}
```

Server names must be a lowercase slug (`[a-z0-9-]`, no `__`) so the namespace
splits unambiguously. Secret values never live in `settings.json` — store them in
the keep (`asylum keep set github_pat`) and reference them by name.

## Exposure modes and context

Every tool from every server lands in the agent's context on connect. Aggregate
enough servers and that is a lot of tool definitions — multiplied across a fan-out
fleet. Two levers keep it in check:

- **`allow` / `deny`** per server — expose only the tools you actually want.
- **`"expose": "search"`** — the gateway advertises only `asylum_find_tool` and
  `asylum_call_tool`. The agent searches for a tool by keyword, then invokes it by
  its namespaced name, so definitions load on demand. This works even for agent
  CLIs that have no native concept of lazy tools.

## Auditing

Because the gateway knows *which run* is calling (from the token), every
`tools/call` is recorded as an `mcp_call` event against that run. The Diff surface
and sibling agents can see what a run reached for — something no standalone MCP
aggregator can know.

## From the shell

```sh
asylum mcp list           # the services + tools currently exposed to this run
asylum mcp serve          # run a standalone gateway from settings.json
asylum mcp stdio          # bridge a stdio-only MCP client to the gateway
asylum mcp skill          # print the agent-facing skill doc
```

`asylum mcp stdio` is the shim for an agent CLI that speaks MCP only over stdio:
it forwards stdin↔stdout JSON-RPC to the gateway over HTTP.

## Boundaries (this cut)

- **Server→client requests** (sampling, elicitation) are declined rather than
  routed back to the agent — the POST/JSON subset the gateway speaks carries no
  back-channel. Tool calls that don't need them work normally.
- **HTTP upstreams** are reached over Streamable HTTP via `curl` (for TLS), with
  the auth secret kept off the process argv. OAuth upstreams (refresh flows) are
  not yet handled; use a static token in the keep.
- Both are clean extension points, not dead ends. See `crates/mcp/src/lib.rs`.
