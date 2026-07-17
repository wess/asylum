# Episode 11 — The CLI Tour

**Duration:** ~8 min · **Level:** Advanced
**You'll learn:** every `asylum` subcommand at a glance — worktree, run, search, control, wait, MCP gateway, plugin, layout, and the computer-use trio.
**Prerequisites:** [Episode 03](03-your-first-fanout.md) — you understand runs and worktrees.

## Shot list

| Time | On screen | Action | Narration |
|------|-----------|--------|-----------|
| 0:00 | A clean terminal. | Run `asylum help`. | "The asylum command-line tool scripts the ADE from a shell — or from inside an agent's worktree. asylum help prints the whole surface. Let's tour it." |
| 0:22 | Callout: source vs. released form. | Overlay both invocations. | "One note up front. A released install puts asylum on your PATH. From a source checkout, run it through cargo: cargo run, dash p, cli, dash dash, then the subcommand. I'll say the short form." |
| 0:44 | Terminal. | `asylum worktree list`. | "First, worktree — create, list, and remove the isolated checkouts runs execute in. List marks the primary worktree with a star." |
| 1:06 | Terminal. | `asylum worktree create ../wt-tmp --branch tmp` then remove. | "Create takes a path and an optional branch and start ref; it prints the created path. Remove tears one down — add dash dash force to remove a dirty one. All of these take dash dash repo to target another repository." |
| 1:34 | Terminal. | `asylum run claude-code "print the date"`. | "run launches a single agent against a prompt and streams its output. This is the fastest way to confirm an agent is installed." |
| 1:56 | Echoed `$ claude -p ...` line. | Point at the echo. | "It echoes the exact command it will launch, runs the agent on a real pty, prints the captured screen when it finishes, and exits with the agent's exit code. dash dash cwd sets the working directory." |
| 2:22 | Terminal. | `asylum search "fanout" --dir ./crates`. | "search does cross-worktree content search — ripgrep where available, falling back to git grep. Results print as file, colon, line, colon, column, colon, text, with a match count on stderr." |
| 2:48 | Terminal. | `asylum control status` (inside a run). | "control is how a running agent drives the fleet from inside its worktree — status, read, spawn, activity, check, and skill. It reads the environment variables the app injects. This is a whole episode of its own, coming next." |
| 3:16 | Terminal. | `asylum control skill` prints the doc. | "One control subcommand always works, even outside a run: asylum control skill prints the instruction document that teaches an agent the API." |
| 3:34 | Settings, MCP gateway section. | Enable the gateway and show two configured servers. | "MCP solves the same configuration problem for tools. Instead of teaching every agent about every service, Asylum runs one loopback-only gateway and connects the fleet to it. Each upstream gets a namespace, so create pull request on GitHub becomes github, double underscore, create pull request." |
| 4:02 | `settings.json`, `mcp` and `mcp_servers`. | Show a stdio server and an HTTP server. | "Configure local stdio servers with a command and arguments, or remote Streamable HTTP servers with a URL. Keep credentials out of settings: reference a Keep secret in a stdio environment value or name the secret used for HTTP authorization. You can allow only specific tools, deny dangerous ones, and scope a server to one project." |
| 4:38 | Direct/search exposure diagram. | Switch `mcp.expose` from direct to search. | "Direct exposure advertises every namespaced tool. That's simple for a small catalog. For a wide fleet, search exposure advertises only find tool and call tool, loading definitions on demand so dozens of integrations do not consume every agent's context window." |
| 5:10 | Terminal inside a managed run. | `asylum mcp list`, then `asylum mcp skill`. | "Inside a run, asylum mcp list shows exactly which services and tools this project can reach. The app injects a run-scoped URL and signed token; every tool call is attributable to that run. MCP skill prints the agent-facing instructions, and MCP stdio bridges clients that cannot speak the gateway's HTTP transport." |
| 3:40 | Terminal. | `asylum wait run 42 --status succeeded`. | "wait blocks until a run reaches a status or activity, then returns. Block until run forty-two succeeds — or until it goes blocked, with dash dash timeout to cap the wait. It polls about every three-quarters of a second and errors on timeout." |
| 4:10 | Terminal. | `asylum plugin list` then `asylum plugin search --limit 10`. | "plugin manages the plugins directory — install from GitHub by owner-slash-repo, list what's installed, and search discovers community plugins by GitHub topic. Search uses gh, so authenticate it first." |
| 4:38 | Terminal. | `asylum layout list` and `asylum layout show duel`. | "layout reads the fan-out presets from your settings — list them all, or show one in full with its resolved concurrency." |
| 5:04 | Terminal. | `asylum snapshot shot.png`. | "The last three are low-level computer-use commands for driving the desktop itself. snapshot takes a screenshot and prints its path." |
| 5:26 | Terminal. | `asylum click 640 400` and `asylum fill "hello world"`. | "click moves and clicks at screen coordinates; fill types text. They're platform-aware and shell out to the OS's automation tooling — the building blocks of computer-use flows." |
| 5:50 | `asylum --version`. | Run version. | "And asylum dash dash version prints the version. That's the whole CLI. Next, the deep one: an agent commanding its own siblings through control." |

## B-roll / capture notes
- Record inside your throwaway repo so `worktree` commands are safe and cheap.
- For `asylum control status`, run it from inside an active Asylum run's pane so it has the injected env vars; otherwise it will report it's not inside a worktree (worth showing briefly).
- `asylum plugin search` and PR discovery need `gh` authenticated.
- Keep each subcommand beat tight — this is a reference tour, not a deep dive. The control/wait deep dive is episode 12; the MCP configuration reference is in `docs/mcp.md`.

## Recap card (end screen)
- Subcommands: `worktree`, `run`, `search`, `control`, `wait`, `mcp`, `plugin`, `layout`, plus computer-use `snapshot`/`click`/`fill`.
- The MCP gateway aggregates stdio and HTTP servers under namespaced tools, with Keep-backed secrets, project scoping, filtering, and direct/search exposure modes.
- `run` echoes the exact launch command — the fastest PATH check.
- Source form is `cargo run -p cli -- ...`; released form is `asylum ...`.
- `control` and `wait` are the agent-facing orchestration commands — next episode.

## Next
- [Episode 12 — The Agent Control Surface](12-agent-control-surface.md)

Go deeper: [book chapter 10](../book/10-the-cli.md).
