//! The agent-facing skill document for the MCP gateway.

/// Markdown describing how an in-worktree agent uses the aggregated MCP gateway.
/// Dropped into the agent's rules/skills directory, like the control and proxy
/// skills.
pub const SKILL: &str = r#"# Asylum MCP gateway

Asylum runs **one** MCP server that fronts every configured service, so you have
a single MCP connection instead of one per service. Tools from each service are
**namespaced**: a `create_pull_request` tool on the `github` service is exposed
to you as `github__create_pull_request`. Call a tool by its namespaced name and
Asylum routes it to the right service.

## Connecting

Asylum injects the endpoint and a per-run token into your environment:

- `$ASYLUM_MCP_URL` — the gateway base URL. The MCP endpoint is `$ASYLUM_MCP_URL/mcp`.
- `$ASYLUM_MCP_TOKEN` — a bearer token scoped to this run. Send it as
  `Authorization: Bearer $ASYLUM_MCP_TOKEN`.

If your CLI is configured with the gateway already, you don't need to do
anything — the namespaced tools are simply available. If your CLI speaks MCP only
over stdio, bridge to the gateway with:

```sh
asylum mcp stdio
```

which proxies stdio MCP traffic to the gateway over HTTP.

## Finding and calling tools

In the default mode every namespaced tool is listed directly. If the gateway is
in **search** mode (to keep context small across a wide fleet), it advertises two
tools instead:

- `asylum_find_tool` — search for a tool by keywords; returns matching
  `service__tool` names and descriptions.
- `asylum_call_tool` — invoke a tool you found, by its namespaced name, passing
  its `arguments`.

## Rules

- Address services only by the names Asylum exposes. A call to an unknown or
  filtered tool is rejected — you cannot reach a service that was not configured
  or was hidden by policy.
- The token scopes you to this run's project; you see only the services that
  project is allowed. Don't try to reach another project's services.
- `asylum mcp list` prints the services and tools currently available to you.
"#;
