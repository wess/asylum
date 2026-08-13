# Chapter 10: The CLI

The `asylum` command-line tool scripts the ADE from a shell — or from inside an
agent's worktree. This chapter documents every subcommand with real examples —
`worktree`, `run`, `search`, `control`, `wait`, `plugin`, `layout`, `keep`,
`call`, and the computer-use trio. It is a reference; skim it once, then come
back when you need a specific command.

## Invoking the CLI

A released install puts `asylum` on your PATH. From a source checkout, run it
through cargo:

```sh
cargo run -p cli -- <subcommand> [args]
```

This book writes the short form. The top-level commands are:

```text
asylum worktree <create|list|remove>
asylum run <agent> <prompt...> [--cwd <dir>]
asylum search <pattern> [--dir <dir>]
asylum control <status|read <id>|spawn <agent> <prompt>|activity <state>|check|skill>
asylum call [<upstream> <METHOD> <path> [--data <body>]] [--skill]
asylum mcp <list | serve [--bind addr] | stdio | skill>
asylum keep <set <name> [--project <id>] [--value <v>] | rm <name> | list>
asylum wait run <id> [--status <s>] [--activity <a>] [--timeout <secs>]
asylum plugin <install <owner/repo> | search | list>
asylum layout <list | show <name>>
asylum snapshot [<out.png>]
asylum click <x> <y>
asylum fill <text...>
asylum completions <bash|zsh|fish>
```

Run `asylum help` for the same summary, `asylum <subcommand> --help` for that
one command's usage, and `asylum --version` for the version.

## `worktree` — manage git worktrees

Create, list, and remove the isolated checkouts runs execute in. All accept
`--repo <dir>` to target a repository other than the current directory.

```sh
# Create a worktree at <path>, optionally on a named branch, from a start ref.
asylum worktree create ../wt-feature --branch feature --start main

# List every worktree (the primary is marked with *).
asylum worktree list

# Remove a worktree; --force removes it even if dirty.
asylum worktree remove ../wt-feature --force
```

`create` prints the created path; `list` prints one worktree per line as
`path <tab> branch`.

## `run` — launch one agent

Run a single agent against a prompt and stream its output. This is the quickest
way to confirm an agent is installed and on your PATH (see
[Chapter 2](02-installation-and-setup.md)).

```sh
asylum run claude-code "add a --version flag to the CLI"
asylum run codex "write a unit test for parse_id" --cwd ./crates/companion
```

The CLI echoes the exact command it will launch (for example
`$ claude -p "add a --version flag to the CLI"`), runs the agent on a real pty,
prints the captured screen text when it finishes, and exits with the agent's
exit code. `--cwd` sets the working directory (default: the current directory).

## `search` — cross-worktree content search

Search file contents under a directory, using ripgrep where available and falling
back to `git grep`.

```sh
asylum search "TODO"
asylum search "fn fanout" --dir ./crates/agent
```

Results print as `file:line:column: text`, with a match count on stderr. `--dir`
sets the search root (default: current directory).

## `control` — orchestrate the fleet

`asylum control` is how a *running agent* drives the ADE from inside its
worktree, and how you inspect the fleet from the shell. It reads the environment
variables the app injects (`ASYLUM_CONTROL_URL`, `ASYLUM_TASK_ID`,
`ASYLUM_RUN_ID`, `ASYLUM_CONTROL_TOKEN`) and talks to the local control server.
The full model is [Chapter 11](11-agent-orchestration-and-the-control-surface.md);
the commands are:

```sh
asylum control status              # your run + its siblings, with live activity
asylum control read <run-id>       # a sibling's recent transcript tail
asylum control spawn <agent> "..." # queue another agent on this task
asylum control activity <state>    # report yourself: working|blocked|done|idle
asylum control check               # run this project's checks in your worktree
asylum control skill               # print the agent skill document
```

Most of these only work when you are *inside* an Asylum run (that is, when
`ASYLUM_RUN_ID` is set). `asylum control skill` always works — it just prints
instructions.

## `wait` — block until a run reaches a state

Poll a run until its status or activity matches, then return. Useful for an agent
that spawns a helper and needs to wait for it.

```sh
# Block until run 42 finishes successfully (default timeout 600s).
asylum wait run 42 --status succeeded

# Block until run 42 goes blocked (needs input), give up after 120s.
asylum wait run 42 --activity blocked --timeout 120
```

You must pass `--status`, `--activity`, or both (both must match). It polls
roughly every 750ms and errors on timeout. When you waited on an activity but the
run *ended* first, it reports that the run finished before reaching the state, so
you never wait forever on a dead run.

## `plugin` — install and discover plugins

Manage the plugins directory. Details are in [Chapter 13](13-plugins.md).

```sh
# Install from GitHub (shallow clone into the plugins dir); @ref is optional.
asylum plugin install acme/asylum-linear
asylum plugin install acme/asylum-linear@v1.2.0

# List installed plugins (and report any broken manifests).
asylum plugin list

# Discover community plugins tagged with the `asylum-plugin` GitHub topic.
asylum plugin search --limit 20
```

`search` uses the `gh` CLI, so install and authenticate `gh` for discovery.

## `agent` — persistent agents with their own memory

A run is a thing that happened. A named agent is somebody who keeps happening:
a name, a brief, and a growing list of what it has learned about *this*
repository, put in front of every prompt it gets.

The memory is the point. Without it every fresh run rediscovers that the
integration tests need a database up, that the generated files are not the ones
to edit, and that one test is flaky — and then the run ends and it is
rediscovered again tomorrow.

```sh
# Hire one. --agent picks which CLI drives it (default: claude-code).
asylum agent add Reviewer --role 'You review diffs for correctness first.'
asylum agent add Archivist --role 'You keep the docs honest.' --agent codex

# The roster, most recently used first, with how much each one knows.
asylum agent list

# Its brief and everything it knows - exactly what gets prepended to prompts.
asylum agent show Reviewer

# Teach it something yourself.
asylum agent remember Reviewer 'integration tests need postgres running'

# Wipe the memory but keep the agent; or remove the agent entirely.
asylum agent forget Reviewer
asylum agent rm Reviewer
```

Named agents are scoped to a project, because a role is about a codebase:
"Reviewer" in a Rust workspace and "Reviewer" on a marketing site have nothing
useful to say to each other.

Re-running `add` on an existing name updates the brief and **keeps** the memory —
correcting what somebody is for should not erase what they have learned. `forget`
is the command for that, and `rm` removes the agent while leaving its past runs
on the board.

A name that is already an agent id (`codex`, `claude-code`) is refused: fan-out
entries resolve against the roster first, so such a name would shadow the tool
itself.

Inside the ADE, named agents appear as chips in the composer beside the raw
agents, each carrying the number of things it knows.

A *running* agent adds to its own memory with
[`asylum control remember`](11-agent-orchestration-and-the-control-surface.md).

## `schedule` — run a task on a cadence

Work that starts with nobody watching. A schedule creates an ordinary task and
fans it out through the same path a person uses, so what you find in the morning
is reviewed exactly like anything else.

```sh
# Every night, in the most recently opened project.
asylum schedule add --every 1d --title Flakes 'Find and fix flaky tests'

# Pick who runs it. --agents takes agent ids or names from your roster.
asylum schedule add --every 12h --agents Archivist \
  'Find docs that no longer match the code.'

asylum schedule list
asylum schedule disable 3     # keep it, stop it firing
asylum schedule enable 3
asylum schedule rm 3
```

Cadences are `30m`, `1h`, `1d`, `1w`. The first run is one whole cadence away,
not immediate — adding a nightly job at 2pm means nightly from tomorrow, rather
than a fan-out while you are still typing.

A schedule that misses periods (the app was closed over a long weekend) catches
up rather than replaying: one run now, and the cadence resumes. Three runs at
once, each fanning out to several agents, for work since done twice over, is not
what anybody meant.

## `routine` — show Asylum a workflow once, then replay it

Recording instruments a shell rather than watching the screen. What is worth
replaying in a repository is the commands, and a command does not care where a
window moved to.

```sh
# Work through it in an instrumented shell; exit the shell to save.
asylum routine record release --about 'cut and publish'

asylum routine list
asylum routine show release   # the steps, in order
asylum routine run release    # replay, stopping at the first failure
asylum routine rm release
```

Leaving the shell is how you stop recording, so `exit` is never a step. A
command re-run because it failed the first time is one step, not two. Re-recording
a name replaces it — that is how you correct one.

## `layout` — inspect fan-out presets

Read the layouts defined in your settings ([Chapter 5](05-layouts-and-presets.md)):

```sh
asylum layout list
asylum layout show swarm
```

## `keep` — manage the encrypted secret store

The keep holds the credential values the secrets proxy injects on an agent's
behalf. It lives beside `settings.json` (`~/.config/asylum/keep.enc`), is
encrypted at rest, and is unlocked with a passphrase read from
`ASYLUM_KEEP_PASSPHRASE` — every `keep` command needs that variable set.

```sh
# Store a secret. Omit --value and it is read from stdin, so it stays out of
# your shell history and off the process list.
export ASYLUM_KEEP_PASSPHRASE='…'
asylum keep set openai --value sk-…
printf 'sk-…' | asylum keep set openai

# Scope a secret to one project instead of globally.
asylum keep set openai --project 3

# List secret names in a scope (names only - values never print).
asylum keep list
asylum keep list --project 3

# Remove one.
asylum keep rm openai
```

Secrets are scoped `Global` or `Project(<id>)`; a project-scoped secret overrides
a global one of the same name for that project. `list` prints names only — the
keep never prints a value back, and neither does the proxy.

## `call` — masked outbound API calls

Make an HTTP request to a configured upstream *through* the secrets proxy. You
name the upstream; Asylum resolves the credential from the keep and injects it
server-side, then forwards only to that upstream's fixed host. The command never
handles the secret itself, so an agent can use a key it never sees.

This only works from inside a run: it reads `ASYLUM_PROXY_URL` and
`ASYLUM_PROXY_TOKEN` from the environment the app injects, and errors out
elsewhere. It also needs `proxy.enabled` and at least one upstream configured
([Chapter 14](14-configuration-reference.md)).

```sh
# List the upstreams this run may address.
asylum call

# GET / POST an upstream by name; the path is everything after the name.
asylum call openai GET /v1/models
asylum call openai POST /v1/chat/completions --data '{"model":"gpt-4o"}'

# --data also reads a file, curl-style.
asylum call openai POST /v1/chat/completions --data @body.json

# Print the skill doc that teaches an agent this API.
asylum call --skill
```

The method defaults to `GET` and the path to `/`. A `--data` body is sent as
`application/json`. Requests go out over `curl`.

## `mcp` — the aggregated MCP gateway

Work with the MCP gateway: one MCP server that fronts every configured upstream
server under per-service namespaces (`github__create_pr`). See
[Chapter 14](14-configuration-reference.md) for `mcp` / `mcp_servers`, and
`docs/mcp.md`.

```sh
# Inside a run: list the services + tools currently exposed to you.
asylum mcp list

# Stand up a gateway from settings.json (for an agent launched outside the app).
asylum mcp serve
asylum mcp serve --bind 127.0.0.1:8790

# Bridge a stdio-only MCP client to the gateway over HTTP.
asylum mcp stdio

# Print the skill doc that teaches an agent this gateway.
asylum mcp skill
```

`list` and `stdio` read `ASYLUM_MCP_URL` / `ASYLUM_MCP_TOKEN` from the run's
environment; `serve` reads `mcp` / `mcp_servers` from `settings.json` (resolving
any `{secret:NAME}` from the keep when `ASYLUM_KEEP_PASSPHRASE` is set). Use
`stdio` when an agent CLI speaks MCP only over stdio — it proxies stdin/stdout
JSON-RPC to the gateway's HTTP endpoint.

## `completions` — shell completions

Print a completion script for your shell to stdout, then source or install it
the way that shell expects:

```sh
asylum completions bash > /etc/bash_completion.d/asylum
asylum completions zsh > "${fpath[1]}/_asylum"
asylum completions fish > ~/.config/fish/completions/asylum.fish
```

Exactly three shells are supported — `bash`, `zsh`, `fish` — any other name is
an error.

## Computer use: `snapshot`, `click`, `fill`

Three low-level OS automation commands for driving the desktop itself — the
building blocks of computer-use flows.

```sh
asylum snapshot                    # screenshot to asylum-snapshot.png
asylum snapshot shot.png           # ...or to a named file
asylum click 640 400               # mouse click at screen (x, y)
asylum fill "hello world"          # type text
```

`snapshot` writes a PNG (default `asylum-snapshot.png`) and prints its path;
`click` moves and clicks at the given screen coordinates; `fill` types the given
text. These are platform-aware and shell out to the OS's automation tooling.

## Try it

1. `asylum worktree list` in a repo, then `asylum worktree create ../wt-tmp` and
   list again.
2. `asylum run <agent> "print the current date"` for an agent you have installed.
3. `asylum search "fanout" --dir ./crates` and read the results.
4. `asylum layout show duel` to see a preset resolved.
5. `asylum agent add Reviewer --role 'You review diffs for correctness first.'`,
   teach it one thing with `asylum agent remember`, then `asylum agent show
   Reviewer` — that output is what every future run of Reviewer starts with.

## Recap

- The CLI mirrors the ADE: `worktree`, `run`, `search`, `control`, `wait`,
  `plugin`, `agent`, `schedule`, `routine`, `layout`, `keep`, `call`, plus
  computer-use `snapshot`/`click`/`fill` and shell `completions`.
- `agent` gives an agent a name and a memory that survives the task; `schedule`
  runs work on a cadence with nobody watching; `routine` replays a workflow you
  demonstrated once.
- Every subcommand answers its own `--help`; `run` echoes the exact launch
  command — the fastest PATH check.
- `control` and `wait` are the agent-facing orchestration commands, detailed
  next.
- `keep` stores credentials encrypted; `call` spends them through the proxy
  without ever revealing them to the agent.

## Next

[Chapter 11: Agent Orchestration and the Control Surface](11-agent-orchestration-and-the-control-surface.md)
is the deep dive into how a running agent commands the fleet.
