# Chapter 10: The CLI

The `asylum` command-line tool scripts the ADE from a shell — or from inside an
agent's worktree. This chapter documents every subcommand with real examples. It
is a reference; skim it once, then come back when you need a specific command.

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
asylum wait run <id> [--status <s>] [--activity <a>] [--timeout <secs>]
asylum plugin <install <owner/repo> | search | list>
asylum layout <list | show <name>>
asylum snapshot [<out.png>]
asylum click <x> <y>
asylum fill <text...>
```

Run `asylum help` for the same summary, and `asylum --version` for the version.

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

## `layout` — inspect fan-out presets

Read the layouts defined in your settings ([Chapter 5](05-layouts-and-presets.md)):

```sh
asylum layout list
asylum layout show swarm
```

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

## Recap

- The CLI mirrors the ADE: `worktree`, `run`, `search`, `control`, `wait`,
  `plugin`, `layout`, plus computer-use `snapshot`/`click`/`fill`.
- `run` echoes the exact launch command — the fastest PATH check.
- `control` and `wait` are the agent-facing orchestration commands, detailed
  next.

## Next

[Chapter 11: Agent Orchestration and the Control Surface](11-agent-orchestration-and-the-control-surface.md)
is the deep dive into how a running agent commands the fleet.
