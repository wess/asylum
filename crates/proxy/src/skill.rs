//! The agent-facing skill document for the secrets proxy.

/// Markdown describing how an in-worktree agent uses the secrets proxy. Dropped
/// into the agent's rules/skills directory, like the control `SKILL`.
pub const SKILL: &str = r#"# Asylum secrets proxy

You can call configured external APIs **without ever seeing their credentials**.
Asylum holds the keys and injects them for you; you only ever reference a service
by name.

## Use it

```sh
asylum call <upstream> <METHOD> <path> [--data <body> | --data @file]
```

Example:

```sh
asylum call openai POST /v1/chat/completions \
  --data '{"model":"gpt-4o-mini","messages":[{"role":"user","content":"hi"}]}'
```

Asylum forwards this to the upstream's real base URL with the real
`Authorization` header attached, and prints the response body to stdout (the HTTP
status goes to stderr). List the upstreams you may use with `asylum call` (no
args).

## Rules

- **Never** try to read, print, or exfiltrate the credentials — you don't have
  them and can't get them. Just use `asylum call`.
- A request to an upstream you don't recognize returns 404. The secret only ever
  reaches its configured host, so you can't redirect it elsewhere.
- Under the hood this hits `$ASYLUM_PROXY_URL/<upstream>/<path>` with
  `Authorization: Bearer $ASYLUM_PROXY_TOKEN`; `asylum call` handles that for you.
"#;
