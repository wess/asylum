# CLI, MCP gateway, integrations, and plugins

Advanced · Episode 9

## 1. A clean terminal.

The asylum command-line tool scripts the ADE from a shell — or from inside an agent's worktree. asylum help prints the whole surface. Let's tour it.

## 2. Callout: source vs. released form.

One note up front. A released install puts asylum on your PATH. From a source checkout, run it through cargo: cargo run, dash p, cli, dash dash, then the subcommand. I'll say the short form.

## 3. Terminal.

First, worktree — create, list, and remove the isolated checkouts runs execute in. List marks the primary worktree with a star.

## 4. Terminal.

Create takes a path and an optional branch and start ref; it prints the created path. Remove tears one down — add dash dash force to remove a dirty one. All of these take dash dash repo to target another repository.

## 5. Terminal.

run launches a single agent against a prompt and streams its output. This is the fastest way to confirm an agent is installed.

## 6. Echoed $ claude -p ... line.

It echoes the exact command it will launch, runs the agent on a real pty, prints the captured screen when it finishes, and exits with the agent's exit code. dash dash cwd sets the working directory.

## 7. Terminal.

search does cross-worktree content search — ripgrep where available, falling back to git grep. Results print as file, colon, line, colon, column, colon, text, with a match count on stderr.

## 8. Terminal.

control is how a running agent drives the fleet from inside its worktree — status, read, spawn, activity, check, and skill. It reads the environment variables the app injects. This is a whole episode of its own, coming next.

## 9. Terminal.

One control subcommand always works, even outside a run: asylum control skill prints the instruction document that teaches an agent the API.

## 10. Settings, MCP gateway section.

MCP solves the same configuration problem for tools. Instead of teaching every agent about every service, Asylum runs one loopback-only gateway and connects the fleet to it. Each upstream gets a namespace, so create pull request on GitHub becomes github, double underscore, create pull request.

## 11. settings.json, mcp and mcp_servers.

Configure local stdio servers with a command and arguments, or remote Streamable HTTP servers with a URL. Keep credentials out of settings: reference a Keep secret in a stdio environment value or name the secret used for HTTP authorization. You can allow only specific tools, deny dangerous ones, and scope a server to one project.

## 12. Direct/search exposure diagram.

Direct exposure advertises every namespaced tool. That's simple for a small catalog. For a wide fleet, search exposure advertises only find tool and call tool, loading definitions on demand so dozens of integrations do not consume every agent's context window.

## 13. Terminal inside a managed run.

Inside a run, asylum mcp list shows exactly which services and tools this project can reach. The app injects a run-scoped URL and signed token; every tool call is attributable to that run. MCP skill prints the agent-facing instructions, and MCP stdio bridges clients that cannot speak the gateway's HTTP transport.

## 14. Terminal.

wait blocks until a run reaches a status or activity, then returns. Block until run forty-two succeeds — or until it goes blocked, with dash dash timeout to cap the wait. It polls about every three-quarters of a second and errors on timeout.

## 15. Terminal.

plugin manages the plugins directory — install from GitHub by owner-slash-repo, list what's installed, and search discovers community plugins by GitHub topic. Search uses gh, so authenticate it first.

## 16. Terminal.

layout reads the fan-out presets from your settings — list them all, or show one in full with its resolved concurrency.

## 17. Terminal.

The last three are low-level computer-use commands for driving the desktop itself. snapshot takes a screenshot and prints its path.

## 18. Terminal.

click moves and clicks at screen coordinates; fill types text. They're platform-aware and shell out to the OS's automation tooling — the building blocks of computer-use flows.

## 19. asylum --version.

And asylum dash dash version prints the version. That's the whole CLI. Next, the deep one: an agent commanding its own siblings through control.

## 20. The Integrations surface.

Work usually starts as an issue and lands as a pull request. Integrations connects the fleet to GitHub and Linear so you can do both without leaving Asylum.

## 21. Terminal: gh auth login.

GitHub works through the gh CLI. That means Asylum uses whatever authentication gh already has — install gh, run gh auth login once, and there's no token to paste. If gh isn't installed or authenticated, the GitHub features are simply unavailable.

## 22. PR list in Integrations.

With gh authenticated, you can list the repository's open pull requests right here.

## 23. Issue list in Integrations.

And browse open issues.

## 24. Issue → worktree action.

The most useful flow is turning an issue into work. Pick an issue and Asylum derives a worktree branch named for it — so the agents you fan out are already on the right branch for a PR that closes it.

## 25. Task created on the issue branch.

You go from 'issue one-two-three needs doing' to 'three agents attempting it in isolated worktrees' in a couple of clicks.

## 26. Winning run selected; create-PR.

When one run wins, open a pull request from its branch — the natural alternative to a local merge when you want the change reviewed on GitHub with CI.

## 27. Note showing the appended PR link.

And remember from the notes episode: a created PR appends its link to every note attached to the task. So opening the PR also leaves a durable trail — the note that started the task ends up carrying the link to the PR that finished it.

## 28. Settings surface, linear_token.

Linear is the other integration. Unlike GitHub, it needs an API token. Create one at linear dot app settings API and set linear underscore token in settings.

## 29. Paste token; live reload.

Live reload picks it up immediately. An empty token leaves Linear disabled and the surface just doesn't offer it.

## 30. Linear teams and projects load.

With it set, Integrations browses your Linear workspace — teams, projects, and issues.

## 31. Open a worktree from a Linear issue.

You can open a worktree from a Linear issue just like GitHub, so a Linear ticket becomes a fanned-out task — and you can create an issue from inside the ADE too.

## 32. Split: local merge vs. PR recap.

Two ways to land a winner: a local merge for speed, or a PR for review. Next: the workaday surfaces — terminal, editor, preview, and browser.

## 33. A long-running fan-out on the board.

A big fan-out can run longer than your attention span at the desk. The mobile companion lets you keep an eye on the fleet from your phone, and the event stream lets your phone — and your agents — follow along without hammering the database.

## 34. Settings companion block.

The companion is a small HTTP server the app runs on a background thread, over the same store the desktop uses. Configure it under the companion key — enabled, a bind address, and a token.

## 35. Security overlay: localhost vs 0.0.0.0.

Here's the rule of thumb. Localhost with no token for solo desktop use. To reach it from your phone on the LAN, bind zero-dot-zero-dot-zero-dot-zero — but only with a token set. Never expose it to the network without a token. The token is the gate.

## 36. Edit bind to 0.0.0.0:8787 + a token.

Set a token, bind to all interfaces on port eight-seven-eight-seven, and save. Live reload applies it.

## 37. Phone opening the bound address.

Open that address in your phone's browser and you get a self-contained mobile status page — it polls the API and shows your projects, tasks, runs, and inbox at a glance.

## 38. Phone showing runs with activity chips.

And this is the payoff: the runs list includes each run's live activity. So the phone shows which agent is blocked and needs you — exactly like the board. You can be away from your machine and still know a run is waiting on your answer.

## 39. Phone: type and send a follow-up.

You can nudge a run from your phone too. Send a follow-up — a message the app queues and delivers into a live run, and also posts to your inbox. A live, stdin-capable agent gets it immediately; a finished run starts a fresh attempt with it.

## 40. Diagram: append-only event log.

Under all of this is the event stream. Polling every table for 'what changed' is wasteful, so every meaningful transition appends one row to an append-only log. It's never mutated after insert, so a cursor is a stable position.

## 41. Event kinds list.

The kinds you'll see: run_started, run_activity when a run's semantic state changes, run_finished, run_failed, and run_spawned. Each carries an id — the cursor — a kind, the task and run it concerns, an optional payload, and a timestamp.

## 42. Terminal.

Follow it from a cursor. Call with since equals zero to read from the start; the response gives you a batch and a cursor — the id of the last event. Pass that cursor back as since on the next call to get only what's new.

## 43. Second curl with the returned cursor.

That's the whole protocol. A page is capped at two hundred events; lower it with a limit parameter.

## 44. Overlay: /api/events and /control/events.

And the same stream is served on both servers — the companion for your phone, and control for your agents. So a lead agent can tail the fleet's events the same way your phone does. Next: extending the ADE itself with plugins.

## 45. A plugins directory with a plugin.toml.

Asylum is extensible. A plugin is just a directory with a plugin-dot-toml manifest — contributing palette commands, a panel, a webview, event triggers, and tools for the agents.

## 46. Path overlay: plugins dir.

Plugins live under your data directory, in asylum slash plugins slash the plugin id. On startup Asylum scans that directory, loads the good ones, and reports a diagnostic for each bad manifest — one broken plugin never blocks the others.

## 47. An annotated plugin.toml.

Here's a manifest. Up top: identity — id, name, version, description — and capabilities, which we'll come back to.

## 48. [[command]] block.

The extension points. Double-bracket command adds a command-palette action. Its mode decides what running it does — invoke calls a runtime method, panel opens the plugin's panel, webview opens its web surface — and it can carry a keybind.

## 49. [panel] and [webview] blocks.

panel is a side-drawer rendered from the runtime's responses. webview is a native web surface placed as a panel, tab, or window — sourced from a url, a bundled entry, or a service.

## 50. [[trigger]] block.

Double-bracket trigger hooks an ADE event. Its action is notify — post a desktop notification — or invoke a runtime method, and it can be conditioned with a when clause.

## 51. Trigger events list.

The events you can hook: task_created, run_started, run_finished, run_failed, worktree_created, worktree_removed, diff_ready, and task_merged.

## 52. [[tool]] block with typed params.

And double-bracket tool exposes a tool to the coding agents themselves, with typed parameters — so an agent can call your plugin's functionality directly.

## 53. capabilities = [...] line.

Now capabilities. A plugin declares what it's allowed to touch — git, agents, store, network, filesystem, clipboard, notify. Under the process runtime these are advisory. Under WASM they're the enforced gate.

## 54. Split: process vs WASM runtimes.

There are two runtimes. The process runtime is a normal program the app talks to over newline-delimited JSON on stdin and stdout — request in, result or error out. Non-JSON lines, like stray logging, are ignored, so your runtime can print debug freely. One-shot, or persistent to stay warm across calls.

## 55. WASM runtime callout.

The WASM runtime runs a WebAssembly module in a sandbox. The crucial property: the host links only the host functions your declared capabilities allow — so a guest that never asked for notify literally cannot import the notify function. That makes WASM the right tier for untrusted or shared plugins.

## 56. Terminal.

Installing is a shallow clone into the plugins directory. asylum plugin install, owner slash repo — optionally at a branch, tag, or commit. It refuses if the destination exists or the repo has no manifest, so you can't accidentally install a non-plugin.

## 57. Terminal.

list shows what's installed and reports broken manifests. search discovers community plugins tagged with the GitHub topic asylum-plugin — via gh, so authenticate it first. Tag your own repo with that topic to make it discoverable.

## 58. A minimal hello plugin manifest + runtime.ts.

Let's build one. A minimal process plugin is a manifest and a runtime script. The manifest declares an id, the notify capability, a process runtime running the script, and one greet command.

## 59. The runtime.ts reading newline-JSON.

The runtime reads one JSON request per line and answers each — returning a result for greet, an error otherwise, and skipping anything that isn't JSON. Bun and TypeScript are a natural fit here.

## 60. Command palette shows "Say Hello".

Drop the directory under the plugins path, and 'Say Hello' appears in the command palette. From there you grow it — a panel, a trigger on run_finished, a tool the agents can call.
