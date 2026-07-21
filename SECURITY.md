# Security Policy

## Supported Versions

Asylum follows semantic versioning. **Only the latest release is supported** with security updates. If you find a security issue, upgrade to the latest version first to verify it is not already fixed.

## Reporting a Vulnerability

**Do not open a public GitHub issue for security vulnerabilities.** Instead:

1. Go to the [Asylum repository](https://github.com/wess/asylum)
2. Click **Security** → **Report a vulnerability**
3. Follow GitHub's private disclosure process

Alternatively, email **me@wess.io** with:
- A description of the vulnerability
- Steps to reproduce (if applicable)
- The affected Asylum version
- Any proposed fix (optional)

## Response Expectation

You will receive acknowledgment of your report within **one week**. Security fixes are prioritized and released as soon as feasible.

## Scope

### In Scope

- Code execution vulnerabilities in Asylum itself
- Privilege escalation or access control bypasses
- Secrets leaks or crypto weaknesses in the encrypted `keep` store or `proxy`

### Out of Scope

- Upstream library vulnerabilities (report to the library maintainers first)
- Agent-specific security issues (report directly to the agent provider, e.g., Anthropic for Claude)
- Social engineering or phishing
- Denial of service via resource exhaustion (Asylum runs on your machine; local DoS is not a security issue)

### Architecture & Threat Model

Asylum is a **local desktop application** with optional networked services:

- **Local servers**: the MCP gateway, agent control surface, secrets proxy, and mobile companion server all bind to loopback (`127.0.0.1` or `::1`) only, and require token authentication. Network access is not possible without explicitly configuring a reverse proxy.
- **Secrets**: credentials are stored encrypted (AES-256-GCM) in `~/.config/asylum/keep.enc` and decrypted only into memory during runs. The secrets `proxy` intercepts outbound API calls from agents and injects credentials server-side, so agents never see the raw key.
- **Git worktrees**: each agent runs in an isolated git worktree with its own `HEAD`, branch, and working directory. Agents cannot directly access the main repository or each other's worktrees.
- **Plugin sandbox**: WASM plugins run in a `wasmi` sandbox with no default capabilities; host functions are capability-gated and only exposed if the plugin declares them in `plugin.toml`.

For the complete threat model and architectural details, see [`AUDIT.md`](AUDIT.md).
