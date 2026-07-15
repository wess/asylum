# Architecture

Asylum is a Cargo workspace layered bottom-up. Each crate depends only on those
below it; the core crates are gpui-free and the boundary is `app`.

```
app  ─────────────────────────────  gpui + guise-ui + libsinclair
 │
 ├── agent ── config                 registry, command build, fan-out plan, activity
 ├── plugin ── pluginrt              manifests + process runtime + GitHub install
 ├── companion / control ── store    mobile + agent JSON servers over the store
 ├── notes                           Markdown vault + knowledge index
 ├── store                           SQLite: projects / tasks / runs / events
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
5. **Track.** Pty events snapshot terminal output into SQLite. Each snapshot is
   also classified into a **semantic activity** (`agent::activity`:
   working / blocked / done / idle) so the board shows which agent is *blocked
   waiting on you*, and every transition appends to the `store` event log. Exit
   updates the durable status, commits a successful run's worktree changes, and
   starts that worktree's detected checks. The queue launches another run when
   capacity opens.
   - **Agents can steer.** The app injects control-surface env vars into each
     run (`ASYLUM_CONTROL_URL`, `ASYLUM_TASK_ID`, `ASYLUM_RUN_ID`). A running
     agent uses the `control` server to spawn a helper run, read a sibling,
     report its state, run checks, or wait on another run. Write-effects are
     queued as `store::ControlRequest`s and drained by the app on a timer,
     exactly like mobile follow-ups.
6. **Review.** `git::diff::since_fork(worktree, base_branch)` yields the selected
   run's diff. Review comments queue another attempt in the same worktree and
   survive app restarts.
7. **Merge or open a PR.** The app blocks failed checks, checks the base
   worktree, runs a non-destructive conflict preflight, then asks for explicit
   confirmation. Cleanup removes clean finished worktrees and keeps branches.

Project memory crosses this loop without replacing it. `notes` indexes plain
Markdown and wiki-style metadata; `store` remembers the selected vault and
note attachments. Task attachments are inherited by every generated run, their
Markdown is appended to the launch prompt, and run/check/PR links are written
back to the attached notes.

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
are the CRUD, implemented as inherent methods on `Db`. A run also carries a
live `activity` token (ephemeral, distinct from the lifecycle `status`).
`control.rs` is the agent control-request queue and `event.rs` the append-only
event log both servers replay from a cursor; both follow the `followup.rs`
drain contract.

### `notes`
Plain Markdown is the source of truth. `vault.rs` performs path-safe recursive
CRUD and updates incoming wiki links on rename. `parse.rs` reads YAML
frontmatter, `[[target|alias]]` links, tags, and completion fragments.
`search.rs` ranks note title/path/body hits, and `template.rs` supplies task,
decision, investigation, and retrospective structures. The crate never imports
gpui or SQLite.

### `config`
`model.rs` is the typed `Settings` schema with serde defaults so a partial file
still deserializes — including named fan-out `Layout` presets (built-in
`duel` / `triad` / `swarm`, overridable) and the `control` server prefs. `jsonc.rs` blanks `//` and `/* */` comments to spaces
(preserving offsets) before `serde_json`. `load.rs` resolves the path and
turns any parse failure into a `Diagnostic` plus defaults — the app always gets
a usable `Settings`. `edit.rs` sets or removes one top-level key in the
settings.json *text*, preserving every comment and hand-formatted value, and
persists via temp-file + rename (an unreadable or non-object file refuses the
write); the app's Settings surface writes through it, so the file stays the
single source of truth. `watch.rs` polls the file's mtime on a background
thread for live reload. `keys.rs` is the chord→action `Keymap`: compiled
defaults layered with the user's `keybindings` entries (`chord=action`;
`chord=` unbinds), consumed by the app's `menus::rebind`.

### `agent`
`registry.rs` is the static catalog of CLI agents and the `Delivery` vocabulary
(prompt as arg vs. stdin). `command.rs` lowers a def + user prefs + prompt into
a `SpawnSpec` (whose `env` carries the injected control vars). `plan.rs` does
fan-out and the `slugify` used for branch names. `activity.rs` classifies a
transcript snapshot into `working / blocked / done / idle` using generic rules
plus per-agent additions — a pure function the app calls on each pty snapshot.
The crate never spawns a process — that is the app's job.

### `companion` / `control`
Two dependency-light blocking HTTP/JSON servers over the same `store`, each with
a pure `route()` tested without sockets. `companion` is the read-mostly mobile
API (projects / tasks / runs / notifications, a follow-up endpoint, a mobile web
page) plus an `/api/events` stream. `control` is the agent-facing surface: an
in-worktree agent lists siblings, reads a run, reports its activity, queues a
helper run or a checks pass, and follows `/control/events`. A running agent
learns the API from the `SKILL` document (`asylum control skill`) and reaches it
through the injected env vars; writes are queued for the app to drain.

### `plugin`
`model.rs` is the parsed manifest and the fixed vocabularies (`CAPABILITIES`,
`TRIGGER_EVENTS`). `parse.rs` deserializes `plugin.toml` into private `Raw*`
shapes then validates/lowers them, turning unknown tokens into error strings.
`load.rs` scans a plugins directory, loading the good and collecting a
`Diagnostic` per bad one. `install.rs` parses an `owner/repo[@ref]` spec, builds
the shallow-clone command into the plugins directory, and exposes the
`asylum-plugin` GitHub topic used for `asylum plugin search`.

### `pluginrt`
The runtime host. `invoke_once` spawns a runtime process, sends one JSON
`Request`, and reads one `Response`; `Session` keeps a `persistent` runtime warm
across many `call`s. Non-JSON lines from a chatty runtime are skipped.
`invoke_wasm` runs a module under `wasmi` and links only the host functions
allowed by the manifest's declared capabilities.

### `app`
`main.rs` loads settings, installs the guise theme, wires the native menu, and
opens the window. `state.rs` owns the `Root` entity, SQLite connection,
selection, live terminals, notices, and view snapshots. `run.rs` coordinates
fan-out, queue capacity, pty lifecycle, checks, continuation, merge, and cleanup.
`root.rs` composes guise's `AppShell`, including the collapsible activity rail;
`fleet.rs`, `diff.rs`, and `setup.rs` render the main task workflow. `note/` owns the native project-memory surface
and its task/run context actions; project-wide search combines source hits with
notes and SQLite task/run/transcript records.
