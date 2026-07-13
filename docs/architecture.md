# Architecture

Asylum is a Cargo workspace layered bottom-up. Each crate depends only on those
below it; the core crates are gpui-free and the boundary is `app`.

```
app  ─────────────────────────────  gpui + guise-ui + libsinclair
 │
 ├── agent ── config                 registry, command build, fan-out plan
 ├── plugin ── pluginrt              manifests + process runtime
 ├── store                           SQLite: projects / tasks / runs
 └── git                             worktrees, status, diff
```

## The core loop

The ADE exists to run one prompt across many agents and compare. That loop maps
onto the crates like this:

1. **Pose a task.** A *project* is a git repo (`store::Project`). A *task* is a
   prompt against it (`store::Task`).
2. **Fan out.** `agent::plan::fanout(task_id, title, agents, worktree_dir)`
   returns one `RunPlan` per agent — a unique branch and worktree path.
3. **Allocate.** For each plan, `git::worktree::create` makes the isolated
   worktree on its branch, and `store::Db::create_run` records a queued
   `store::Run`.
4. **Launch.** `agent::command::build(def, prefs, prompt, cwd)` produces a
   `SpawnSpec` (program, args, cwd, optional stdin). The app runs it on a pty
   inside a `libsinclair` terminal pane. `store::Db::start_run` marks it running.
5. **Track.** The process exit drives `store::Db::finish_run` (0 → succeeded,
   non-zero → failed). The fleet board reflects each run's status live.
6. **Review & merge.** `git::diff::since_fork(worktree, base_branch)` yields the
   reviewable diff; the winning run's branch merges back to the project's base.

Steps 1–3 and 6's diff model are built and tested; 4–5's live execution and the
review UI are the next wiring (see `docs/roadmap.md`).

## Crate detail

### `git`
Shells out to the `git` binary; no libgit2. `run.rs` is the invocation helper
and `Error`. `worktree.rs` parses `git worktree list --porcelain` and
adds/removes worktrees. `status.rs` parses porcelain v2 into `Entry` records.
`diff.rs` parses unified diffs into `DiffFile → DiffHunk → DiffLine`, tracking
old/new line numbers so annotations can anchor to a side.

### `store`
`rusqlite` with the `bundled` SQLite — synchronous, no async runtime, because
the gpui app has no tokio. `schema.rs` runs ordered migrations guarded by
`PRAGMA user_version`. `model.rs` holds the row types and their status enums
(round-tripped through lowercase tokens). `project.rs` / `task.rs` / `run.rs`
are the CRUD, implemented as inherent methods on `Db`.

### `config`
`model.rs` is the typed `Settings` schema with serde defaults so a partial file
still deserializes. `jsonc.rs` blanks `//` and `/* */` comments to spaces
(preserving offsets) before `serde_json`. `load.rs` resolves the path and
turns any parse failure into a `Diagnostic` plus defaults — the app always gets
a usable `Settings`.

### `agent`
`registry.rs` is the static catalog of CLI agents and the `Delivery` vocabulary
(prompt as arg vs. stdin). `command.rs` lowers a def + user prefs + prompt into
a `SpawnSpec`. `plan.rs` does fan-out and the `slugify` used for branch names.
The crate never spawns a process — that is the app's job.

### `plugin`
`model.rs` is the parsed manifest and the fixed vocabularies (`CAPABILITIES`,
`TRIGGER_EVENTS`). `parse.rs` deserializes `plugin.toml` into private `Raw*`
shapes then validates/lowers them, turning unknown tokens into error strings.
`load.rs` scans a plugins directory, loading the good and collecting a
`Diagnostic` per bad one.

### `pluginrt`
The runtime host. `invoke_once` spawns a runtime process, sends one JSON
`Request`, and reads one `Response`; `Session` keeps a `persistent` runtime warm
across many `call`s. Non-JSON lines from a chatty runtime are skipped. The
`wasm` tier returns `Error::Unsupported` until the execution engine lands.

### `app`
`main.rs` loads settings, installs the guise theme, wires the native menu, and
opens the window. `state.rs` is the single `Root` entity — it owns the
`store::Db` and the current project/task selection and exposes render snapshots.
`root.rs` composes guise's `AppShell` (header / navbar / main / footer).
`sidebar.rs` builds the project + task nav; `fleet.rs` builds the fan-out board
of run cards; `theme.rs` bridges settings to the guise theme.
