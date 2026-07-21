# Chapter 2: Installation and Setup

In this chapter you will build and run Asylum from source, learn the difference
between the development and released binaries, open your first project, find and
edit your settings, choose which agents to run, and confirm those agents are
actually installed on your machine.

## Prerequisites

Asylum is a native Rust application, so you need a Rust toolchain (`cargo`). It
shells out to the `git` binary for all worktree work, so git must be installed.
Some optional surfaces lean on other tools: the GitHub integration uses the `gh`
CLI, and project checks call your ecosystem's tools (`cargo`, `bun`/`npm`,
`ruff`/`pytest`, `go`). Install those as you need them.

The coding agents themselves are separate programs. Asylum launches them by name
(for example it runs `claude` for `claude-code`, `codex` for `codex`); it does
not bundle them. You install the agent CLIs you want and Asylum drives them.

## Build and run

From the repository root:

```sh
cargo run -p app          # build and launch the ADE
```

The first build fetches the UI dependencies (the `guise-ui` component library
and the `libsinclair` terminal), so it takes a while. Subsequent launches are
fast.

Other useful commands:

```sh
cargo build               # build the whole workspace
cargo test                # run the test suite
cargo clippy --all-targets   # lint
```

### `asylumdev` vs. `asylum`

The development build you launch with `cargo run -p app` installs itself as
**`asylumdev`**. That name is deliberate: it never collides with a released copy
of the app installed as **`asylum`**. When you package a real release, the same
binary ships as `asylum`. Throughout this book, "the app" means whichever you are
running; the behavior is identical. Either one also answers `--version` directly
— printing `asylum <version>` and exiting without opening the window — handy for
confirming what you have installed.

The command-line tool is a separate binary, also named `asylum` (from the `cli`
crate). In a source checkout you can invoke it with cargo:

```sh
cargo run -p cli -- worktree list
```

A released install puts it on your PATH as `asylum`, so you would just write
`asylum worktree list`. This book uses the short `asylum ...` form; translate to
`cargo run -p cli -- ...` when working from source.

## Open a project

A project is just a git repository. On first run Asylum walks you through a setup
flow: you point it at a repo folder, and if that folder is not yet a git repo it
asks for explicit consent before running `git init`. A built-in **setup doctor**
checks that git, branches, worktrees, your agents, Bun, and Cargo are in order
and tells you what is missing.

Once a project is open, its base branch (usually `main`) is where merged winners
land, and per-task worktrees are created under a directory inside the project
(by default `.asylum/worktrees`).

## Where settings live

Asylum reads a layered configuration: compiled-in defaults, overridden by your
own `settings.json`. The file lives under your config directory —
`$XDG_CONFIG_HOME/asylum/settings.json` (on macOS this resolves under your home
config path). It is JSON *with comments*, so you can annotate it freely.

Two things make the settings pleasant to work with:

- **Live reload.** A watcher polls the file, so saving it applies changes
  immediately — the theme and keybindings update without a restart.
- **Comment-preserving writes.** The in-app **Settings** surface (open it with
  `cmd-,`) is a real editor over the same file. When a control writes a key back,
  it edits only that key and leaves your comments and hand-formatting untouched.
  The file stays the single source of truth.

Every key is optional. A missing or malformed value never aborts the load — it
falls back to the built-in default (and, for a bad value, is reported as a
diagnostic). The full annotated reference is
[Chapter 14](14-configuration-reference.md).

## Pick your agents

Asylum ships a catalog of built-in agents, each identified by a short **id**.
Realistic ids you will use throughout the book include `claude-code`, `codex`,
`opencode`, `gemini`, `aider`, `cursor-agent`, `copilot`, and `goose`, among many
others. You choose which agents a task fans out to.

To set the agents used by default, edit `default_agents` in `settings.json`:

```jsonc
{
  // Agents fanned out by default when a task is dispatched.
  "default_agents": ["claude-code", "codex"]
}
```

An empty list means Asylum asks you which agents to use each time. You can also
override individual agents (a custom launch program, extra arguments, or
disabling one entirely) under the `agents` key — see
[Chapter 14](14-configuration-reference.md).

## Verify an agent is on your PATH

Because Asylum launches agents by program name, a common first snag is an agent
that is not installed or not on your PATH. The fastest check is to run the agent
program yourself from a terminal — for example `claude --version` or
`codex --version`. If your shell cannot find it, Asylum cannot either.

You can also let Asylum drive one agent directly from the CLI, which prints the
exact command it will launch:

```sh
asylum run claude-code "say hello"
```

The line it echoes (`$ claude -p "say hello"`) is precisely what Asylum runs — no
shell in between. If that program is missing, install the agent's CLI and try
again. The setup doctor in the app reports installed-versus-verified state for
each configured agent too.

## Try it

1. Launch the app with `cargo run -p app`.
2. Open a small git repository as a project (let Asylum `git init` a throwaway
   folder if you like).
3. Open Settings with `cmd-,`, set `default_agents` to two agents you have
   installed, and save. Watch the change take effect live.
4. From a terminal, run `asylum run <agent> "print your name"` for each agent to
   confirm both are on your PATH.

## Recap

- Build and run with `cargo run -p app`; the dev binary is `asylumdev`, the
  release is `asylum`.
- A project is a git repo; the setup doctor checks your toolchain.
- `settings.json` is JSON-with-comments, live-reloaded, and edited in place by
  the Settings surface.
- Agents are external programs launched by id; verify each is on your PATH.

## Next

[Chapter 3: Your First Task](03-your-first-task.md) walks the full loop —
compose, fan out, watch, review, and merge — start to finish.
