# Getting Started

## Prerequisites

Install Git and at least one agent CLI. Sign in to that CLI in your shell before
you run a real task. Asylum can find executables on `PATH`, or you can set an
executable name or absolute path under Settings > Agents.

Declared `typecheck`, `lint`, and `test` package scripts use Bun. Rust project
checks use Cargo. Asylum does not install or guess missing JavaScript tooling.

## First Run

1. Run `cargo run -p app`.
2. Open an existing repository.
3. If you choose a plain folder, Asylum asks before it runs `git init` and
   creates an empty initial commit. It does not commit the folder's files.
4. Read the setup doctor. Fix each blocked item before dispatch.
5. Select one installed agent and run the setup test. A successful real run
   marks that agent verified, including its authentication and launch path.

The setup doctor checks Git, repository state, the configured base branch,
worktree support, agent executables, and the project toolchain. "Installed"
means Asylum found the executable. "Verified" means that agent completed a run.

## Run a Task

Choose a template or write one concrete outcome. Select one agent for a normal
task; select several when independent implementations will help you compare
tradeoffs. `Create and run` prepares one worktree per agent in the background,
then launches up to the parallel limit from Settings.

Each run card shows its branch, absolute worktree path, lifecycle status,
attempt number, changed files, terminal output, and check result. You can cancel
queued or running work, retry a terminal run in the same worktree, or open its
full terminal.

Asylum marks running processes failed after an app restart because the old pty
cannot survive. Retry continues from the preserved worktree. Review follow-ups
also use that worktree and persist while queued.

## Review and Finish

Select a run, open Review, and compare its diff, terminal output, and checks. A
successful run's changes stay uncommitted in its worktree until merge, so you
can stage or unstage individual hunks (or a whole file) before deciding what
lands; a counter tracks how many hunks are staged. Click a diff line to add a
comment. `Send review to agent` queues another attempt for the selected run and
resolves the sent comments after the queue operation succeeds.

Asylum runs detected checks in each successful run's worktree. Failed or active
checks block merge and PR actions. Before merge, Asylum also checks for user
changes in the base worktree and computes conflicts without changing the index.
The final confirmation performs the merge — as a regular merge or a squash
merge — which is also when the accepted changes are committed onto the run's
branch. A separate `Clean up finished worktrees` action removes clean finished
worktrees and deletes any branch that is now safely merged, leaving dirty
worktrees and any unmerged branch alone.

## Project Configuration

Commit `asylum.toml` at the repository root when the project needs shared
defaults:

```toml
base_branch = "main"
default_agents = ["opencode"]
setup = ["bun install"]

[env]
RUST_BACKTRACE = "1"
```

Asylum runs each `setup` command once in a new worktree before the agent starts,
one command at a time with its own captured output, shown in a cancellable
"Preparing" banner while it runs. Each command gets a 10-minute timeout; a
failure names the exact command and its exit code, and creates a durable failed
run with the worktree path and error so you can inspect or remove it from the
app.

## Keyboard

Use the command palette for navigation and selected-run actions. The native
menus show current shortcuts, and Settings lists the resolved keymap. You can
override keys in `settings.json`; malformed entries appear as Settings
diagnostics without disabling the valid bindings.
