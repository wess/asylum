# Asylum 1.0 Plan

Written 2026-07-20 from a four-angle source audit (UI/onboarding, core engine,
distribution, peripheral surfaces), plus `AUDIT.md`'s deferred items,
`docs/roadmap.md`, and a competitive pass against NousResearch's hermes-agent.

**Execution status (2026-07-20): Phases 0–5 are implemented and verified —
671 workspace tests, clippy `-D warnings` clean, fmt clean. The only open
items need the user: run `packaging/signing.md` on the release machine, cut
the release (Phase 5), and capture the README screenshots (recipe in the 5A
task — needs an unlocked session).**

**Verdict:** Asylum is engineering-complete and unusually clean — zero
TODO/stub markers across the workspace, transactional migrations, an
abort-safe merge flow, hardened loopback servers. What stands between it and
1.0 is (1) trust/distribution (unsigned builds, no release ever cut, no
logging), (2) two engine lifecycle bugs, and (3) a **presentation problem**:
12 co-equal surfaces, the core "compare agents" control hidden behind an
advanced toggle, and power features configurable only via JSON + CLI. The
"feels complex" complaint is fixable without architectural change.

---

## How to execute this plan (next session — read this first)

This plan is designed for multi-agent execution with the Agent tool.

1. **Work phase by phase, batch by batch.** Batches inside a phase run in
   order (`A`, then `B`, …). Tasks inside one batch are file-disjoint — spawn
   them **in parallel, in a single message, as multiple Agent tool calls**.
   A task marked `solo` runs alone (it touches files other tasks need).
2. **Every task carries a model tag.** Pass it as the Agent tool's `model`
   parameter. The tags mean:
   - `fable` — architecture, concurrency, cross-cutting gpui refactors.
   - `opus` — complex features and UI work needing design judgment.
   - `sonnet` — well-scoped implementation with clear acceptance criteria.
   - `haiku` — mechanical edits, copy, docs, config.
   Use `subagent_type: general-purpose` for code changes; `Explore` for any
   re-verification reads.
3. **Tasks marked `(human)` cannot be done by an agent.** Surface them to the
   user at session start — the Apple cert has calendar lead time; start it
   before any code work.
4. **Each subagent prompt must include:** the task bullet verbatim (the
   file:line refs are the context — they were verified during the audit), the
   Repo rules below, and the Verification loop below. Subagents report what
   changed plus verification output; the session lead reviews the diff before
   moving on.
5. **Never let two parallel agents edit the same file.** The batches below
   are already file-disjoint; if you re-plan, check overlap first. Related
   small items are pre-clustered into single tasks for exactly this reason.

A batch fan-out is one message containing several Agent calls — e.g. batch 1C:

```
Agent(subagent_type: "general-purpose", model: "sonnet",
      description: "Servers settings section",
      prompt: <task bullet verbatim> + <Repo rules> + <Verification loop>)
Agent(subagent_type: "general-purpose", model: "sonnet",
      description: "Onboarding actions cluster", prompt: …)
Agent(subagent_type: "general-purpose", model: "sonnet",
      description: "Editor dead click fix", prompt: …)
```

The model tags in this file (`fable`, `opus`, `sonnet`, `haiku`) are the Agent
tool's literal `model` enum values — pass them unchanged. Do not use
`subagent_type: "fork"` for these tasks (forks ignore `model` and inherit the
parent's). If the session has workflow orchestration enabled (ultracode), the
same tags map to `agent(prompt, {model})` in a Workflow script instead.

**Repo rules (binding on every subagent):**
- Never run `git commit` / `push` / `tag` — the user handles all git.
- Filenames lowercase, no spaces/`-`/`_`; split by directory
  (`src/run/merge.rs`, not `src/run-merge.rs`). Small, focused files.
- Tests live in a sibling `tests/` dir pulled in via
  `#[cfg(test)] #[path = "../tests/foo.rs"] mod tests;`. Add a regression
  test with every behavioral fix.
- Functional style — free functions + plain data over class-like shapes.
- gpui imports only inside `crates/app`; core crates stay gpui-free.
- Bun (never node/npm) for `site/` tooling.
- No AI/agent mentions in any commit text, code comment, or doc.

**Verification loop (every code task, before reporting done):**
```sh
cargo build -p <crate> && cargo test -p <crate>
cargo clippy --all-targets -- -D warnings
cargo fmt --all
```
UI tasks additionally: `cargo run -p app` and exercise the changed surface.

**Context from the audit (so you don't re-derive it):**
- Onboarding screen, empty states, agent auto-detect, and setup doctor
  already exist and are good (`root.rs:57-203`, `control.rs:10-27`,
  `agent/src/doctor.rs`). The problem starts *after* the first project.
- The app crate has essentially no stubs or dead buttons; polish gaps are
  localized and listed per task below.
- `AUDIT.md` = security audit, all P0/P1 resolved with tests. Do not
  re-litigate it; its still-deferred items appear as tasks here.
- 369 workspace tests pass; clippy `-D warnings` and fmt are clean. Keep it
  that way.

---

## Phase 0 — Ship blockers (trust + the two engine bugs)

**Status 2026-07-20: every agent-executable Phase 0 task is done and verified —
465 workspace tests pass, clippy `-D warnings` clean, fmt clean, plus a live
smoke test (app boots, only control starts by default, a second launch is
refused by the instance lock, the log file is written). The two (human) items
are resolved: platform scope is decided (1.0 = macOS + Linux; Windows stays
beta), and signing is being provisioned by the user from their release machine
— runbook at `packaging/signing.md`; the 1.0 cut (Phase 5) stays gated on the
six secrets existing.** Implementation notes: pty kill = agent pgid captured at spawn
via a sh wrapper (`crates/app/src/reap.rs`), all seven terminal-discard sites
plus an on_app_quit hook escalate HUP→KILL; companion = default off + token
required even on loopback (`config::bind::Policy::TokenRequired`), router fails
closed on an empty token; single-instance = std `File::try_lock` guard in
`store::lock` acquired before recovery/servers; logging = tracing daily-rolling
file in `<data dir>/logs/` (7-file retention, `ASYLUM_LOG` level), chained
panic hook writing crash reports, Help ▸ Open Log Folder; release copy is now
version-agnostic (version story: next published release is 1.0.0, bumped by
the user at cut time).

- [x] (human) Provision an Apple Developer ID, wire the signing/notarization
  secrets into the repo. The pipeline is already built and gated on them
  (`packaging/macos.sh`, `.github/workflows/release.yml:157-181`) — today
  every `.dmg` is unsigned and Gatekeeper blocks it. This is the #1 launch
  blocker. — **model: n/a (human)** — *2026-07-20: user is provisioning from
  their release machine (their deploy/release agent); exact steps written to
  `packaging/signing.md`. The Phase 5 release cut stays gated on the secrets
  existing.*
- [x] (human) Decide 1.0 platform scope: runtime-test Windows on real
  hardware to take it out of beta, or scope 1.0 to macOS + Linux and label
  Windows beta on README/site. — **model: n/a (human)** — *Decided
  2026-07-20: 1.0 targets macOS + Linux. Windows keeps building, stays
  labeled beta (README updated), and exits beta post-1.0 after real-hardware
  testing.*

Batch 0A (parallel):
- [x] **Kill the agent pty explicitly on cancel/close.** `cancel_run` only
  does `run_terms.remove(&run_id)` + a DB update (`crates/app/src/run.rs:1184`),
  relying on `gpui::Entity<TermView>` drop (`crates/app/src/state.rs:195`) to
  end the child — unverified; if Drop doesn't kill it, cancelled agents keep
  editing the worktree. Mirror `runner`'s explicit `session.shutdown()`
  (`crates/runner/src/lib.rs:179`) before removal, and prove it with a test
  or a documented manual repro. — **model: fable**
- [x] **Companion server: default OFF + require a token when enabled.**
  Today it auto-starts on `127.0.0.1:8787` with auth disabled on an empty
  token (`crates/config/src/model.rs:167`,
  `crates/companion/src/router.rs:231-237`), so any local process can read
  runs and post follow-ups into a live agent. Flip the default, require a
  non-empty token to start, update `docs/` + `assets/settings.example.json`.
  — **model: sonnet**
- [x] **Reconcile version drift + stale install copy.** Workspace is 0.1.3
  (`Cargo.toml:46`) while README says "0.1.0 is the first release"
  (`README.md:81`); no git tag exists. Align README/site wording with the
  real release plan and pick the 1.0 version story. — **model: haiku**

Batch 0B:
- [x] **Single-instance guard.** No lockfile/flock exists; a second launch
  runs `recover_interrupted_runs`, marking the first instance's live runs
  failed (`crates/store/src/run.rs:228-235`, called at
  `crates/app/src/state.rs:342`), and both drain the queues. Take an
  exclusive flock on the data dir at startup; on contention, refuse with a
  clear message (focus-existing if cheap). — **model: opus**

Batch 0C:
- [x] **Logging + panic hook + log file.** Zero logging exists anywhere (no
  `tracing`/`log`, 3 stray `println!`), no panic hook, and `Cargo.toml:76-78`
  leaves panic strategy undecided pending exactly this. Add a small `tracing`
  setup writing to `~/.local/share/asylum/logs/`, a panic hook that records a
  crash report, and a "reveal log file" affordance (Help menu + Settings).
  This unblocks the release-profile panic decision in Phase 5.
  — **model: sonnet**

## Phase 1 — The first five minutes (the "feels complex" fix)

**Status 2026-07-20: Phase 1 complete — 470 workspace tests, clippy
`-D warnings`, fmt clean, and visually verified in the live app (rail shows
Tasks/Review/Search + a collapsed MORE; composer leads with ready-agent chips
and duel/triad/swarm presets; run cards lead with plain status).**
Implementation notes: rail split = `View::PRIMARY`/`View::MORE` with a
`sidebar_more` settings key — hidden active surfaces peek in under the toggle
and the inbox badge moves onto it while Inbox is hidden; the composer's
advanced disclosure now holds only the full catalog + start-from-ref, the
disabled Create-and-run explains itself (`run_disabled_reason`), and the
doctor/stepper render only when relevant; Settings gained a Servers section
(companion/control — env-sourced tokens are never written back to
settings.json); cmd-N focuses the compose input; doctor install hints have
one-click copy; onboarding "Configure agents" deep-links the Agents section;
an empty-project Editor click shows a notice instead of dying silently; merge
preflight copy is plain-language; the surface is named "Review" everywhere
(rail, menu, tab, page title, docs).

Batch 1A (solo — reshapes `state.rs`/`sidebar.rs`/`root.rs`):
- [x] **Progressive disclosure for the activity rail.** All 12 surfaces
  render unconditionally with equal weight (`View::BAR`,
  `crates/app/src/state.rs:45-58`; `crates/app/src/sidebar.rs:110-173`) —
  the single biggest complexity driver. Split into a primary set (Tasks,
  Diff, Search, + Settings) and a "More" reveal for
  Notes/Integrations/Terminal/Editor/Preview/Browser/Plugins/Accounts/Inbox;
  persist the user's reveal state in settings; keep every surface reachable
  via palette/quick-open regardless. Replace the odd "TOOLS" divider
  (`sidebar.rs:111`). — **model: fable**

Batch 1B (solo — one agent owns `fleet.rs`):
- [x] **Composer simplification cluster (all in `crates/app/src/fleet.rs`).**
  (1) Surface ready-agent chips + one-click duel/triad layout presets
  directly in the composer instead of behind the subtle "Choose agents"
  toggle (`fleet.rs:330-390`) — comparing agents is the product's thesis and
  is currently hidden; keep start-ref/full-catalog under advanced. (2) Give
  the disabled "Create and run" button a tooltip stating the computed reason
  — no ready agent / setup blocked / preparing (`fleet.rs:32-37,425`).
  (3) Show the setup doctor + workflow stepper only when relevant instead of
  always above the composer (`fleet.rs:55-60`). (4) Lead run cards with
  plain status language, demoting `.asylum/worktrees/...` paths to secondary
  detail (`fleet.rs:655-677`). — **model: opus**

Batch 1C (parallel):
- [x] **Servers section in Settings.** Companion and control run with no UI
  presence — Settings sections are only
  `general/agents/editor/proxy/mcp/keys` (`crates/app/src/settings.rs:857`).
  Add a section with enable toggles, bind, and token status for both, writing
  through `config::edit`. — **model: sonnet**
- [x] **Onboarding actions cluster.** (1) Make `cmd-N` / File ▸ New Task
  focus the compose input, not just open the Tasks tab
  (`crates/app/src/menus.rs:278`, handler `crates/app/src/root.rs:432-437`).
  (2) Give the setup doctor's install hints one-click copy buttons
  (`crates/app/src/setup.rs:117-138`) and make the onboarding "Configure
  agents" button deep-link straight into the Agents settings section
  (`root.rs:184-193`). — **model: sonnet**
- [x] **Fix the Editor dead click.** `open_view(View::Editor)` silently does
  nothing when `project_files()` is empty
  (`crates/app/src/state.rs:1005-1009`); open an empty editor state or show a
  notice instead. — **model: sonnet**
- [x] **Plain-language merge preflight copy.** "Base worktree is dirty"
  (`crates/app/src/run.rs:1421`) and friends assume git fluency; reword with
  what-it-means + what-to-do while keeping the technical detail secondary.
  — **model: sonnet**

Batch 1D (solo — touches files from 1B/1C):
- [x] **One name for the review surface.** Rail says "Diff"
  (`state.rs:83`), menu says "Diff Review" (`menus.rs:131`), page title says
  "Review changes" (`diff.rs:52`), run card says "Review" (`fleet.rs:799`).
  Pick one term and apply it everywhere (menus, palette, docs).
  — **model: haiku**

## Phase 2 — UI polish

**Status 2026-07-20: complete.** Zero hex color literals remain in the app
crate (all theme tokens, verified against the pinned guise rev); overlay
chrome derives from shared constants and anchors to the real sidebar extent;
Editor got a distinct `file-code` icon and diff comments a real icon; rail
tooltips show bound shortcuts; disabled agent switches explain themselves.

Batch 2A (solo — the sweep touches many files):
- [x] **Theme-token sweep.** Replace hardcoded colors with guise theme
  tokens (`theme.primary()/border()/surface()` — used correctly in
  `sidebar.rs:22-31`): onboarding brand `0x3b82f6` (`root.rs:96,159`),
  run-card border/select (`fleet.rs:893-895`), usage bar
  (`accounts.rs:164,169`), diff washes/focus rings
  (`diff.rs:449-459,505-518,539,570,584,605`), design-pin ring
  (`browser.rs:184`). Light theme must look intentional afterward.
  — **model: sonnet**

Batch 2B (parallel):
- [x] **Anchor and theme the overlay chrome (`root.rs`).** The confirm bar
  is absolutely placed at hardcoded `left: 300px` with a hardcoded dark
  background (`root.rs:342-345`) while the sidebar resizes 180–560px and
  collapses to 52px — anchor it to the real sidebar width and use theme
  tokens. Also stop the notice stack (`root.rs:302-308`) from overlapping
  the header action cluster. — **model: sonnet**
- [x] **Icon/emoji cleanup cluster.** Remove the never-rendered `glyph`
  column of `View::BAR` (`state.rs:45-58`; ignored at `sidebar.rs:110`,
  `root.rs:972`); de-duplicate the Notes/Editor `file-pen` icon
  (`state.rs:66,69`); replace the `💬` in diff comments (`diff.rs:554,558`)
  and emoji agent icons (`agent/src/registry.rs:164,167`) with Lucide icons.
  — **model: haiku**

Batch 2C:
- [x] **Tooltip coverage pass.** Rail items, agent enable switches, and
  other disabled controls lack why-tooltips while run cards/workflow steps
  have good ones (`fleet.rs:146,248,657,669`) — bring the rest up to that
  bar. — **model: sonnet**

## Phase 3 — Engine hardening

**Status 2026-07-20: complete.** Worktrees prune on project open and merged
branches delete safely on cleanup; checks kill hung suites (process-group,
600s default) and respect `.venv`; the activity classifier is line-anchored
with a spinner-quiescence veto and per-agent transcript fixtures; config gets
11 semantic validation rules; git output is locale-pinned and spaced paths
parse; squash merge exists end-to-end (with the correct `reset --merge`
recovery — squash never writes MERGE_HEAD); search runs debounced on a
background read-only connection with FTS5 (~30x on transcripts, migration
10) and a stale-result generation guard; the four oversized app modules split
into 26 focused files (byte-verified moves, nothing over ~500 lines);
transcript persistence is change-gated on a 5s cadence with a 256 KB cap.

Batch 3A (parallel — different crates/files):
- [x] **Worktree + branch hygiene.** `git::worktree::prune`
  (`crates/git/src/worktree.rs:85`) and `git::branch::delete`
  (`crates/git/src/branch.rs:58`) have zero call sites: stale
  `.git/worktrees/*` records accumulate after crashes, and every run branch
  leaks forever ("branch was kept", `crates/app/src/run.rs:1596-1640`). Call
  prune on project open + after cleanup; offer safe (`-d`) branch deletion
  for terminal non-winning runs in `cleanup_task_now`. — **model: sonnet**
- [x] **Checks hardening: timeout + venv.** `checks::run` blocks forever on
  a hanging suite (`crates/checks/src/lib.rs:149-152`) and that gates merge —
  add a deadline (kill on expiry, report "timed out"). Python checks invoke
  bare `ruff`/`pytest` (`lib.rs:115-116`), ignoring virtualenvs — prefer
  `python -m` / detect `.venv`. — **model: sonnet**
- [x] **Fix activity classification false positives.** Generic rules treat
  bare substrings `done`/`success`/`finished` in the last 3 lines as Done
  and prompt glyphs `❯`/`› ` as Blocked
  (`crates/agent/src/activity.rs:165-182`) — so "not done yet" reads as
  review-ready and idle TUIs ping "needs input", corrupting the board's core
  signal. Anchor markers, drop bare glyphs from generic rules (keep them
  per-agent), corroborate Done with idle time. Test against real transcript
  fixtures per agent. — **model: opus**
- [x] **Config semantic validation.** `salvage` catches type errors only
  (`crates/config/src/load.rs:107-152`); add a post-load `validate()`
  emitting `Diagnostic`s for nonsensical-but-typed values (ports, paths,
  `base_branch`), surfaced through the existing diagnostics UI.
  — **model: sonnet**
- [x] **Small-fix cluster (git/search/agent).** (1) Set `LC_ALL=C` in
  `git::run` so output classification survives localized git
  (`crates/git/src/branch.rs:112-114`). (2) Porcelain-v2 paths with spaces
  truncate via `rsplit(' ')` (`crates/git/src/status.rs:91,122`) — parse the
  fixed-field remainder. (3) A malformed search regex misreports as "no
  search backend" (`crates/search/src/lib.rs:53-85`) — distinguish invalid
  pattern from missing backend. (4) Extend agent install hints from 13 to
  all 31 built-ins or add a generic fallback
  (`crates/agent/src/doctor.rs:109-131`). — **model: sonnet**

Batch 3B (parallel):
- [x] **Squash-merge option.** `git::branch::merge` is always
  `git merge --no-edit` (`crates/git/src/branch.rs:108-109`); add a squash
  path (`--squash` + commit) and expose the choice at merge time.
  — **model: sonnet**
- [x] **Move search off the UI thread + evaluate FTS5.** Search scans
  prompts/transcripts with `lower`/`instr` and indexes the note vault
  synchronously on the UI thread (`crates/store/src/search.rs:8`,
  `crates/app/src/state.rs:1391`). Debounce, run on a background executor
  with cancellation (stale results must not clobber newer queries), and
  evaluate SQLite FTS5 for task/transcript data. (AUDIT.md deferral.)
  — **model: opus**

Batch 3C (solo, run LAST in this phase — restructures files others touch):
- [x] **Split the oversized app modules.** `state.rs` ~1.6k lines, `run.rs`
  ~1.5k, `root.rs` ~1.0k, `fleet.rs` ~1.0k. Split along AUDIT.md's suggested
  boundaries (`run/launch.rs`, `run/lifecycle.rs`, `run/control.rs`,
  `run/followup.rs`, `run/check.rs`, `run/merge.rs`) using directory-based
  names per repo rules. Pure reorganization — no behavior change; keep the
  full suite green. — **model: fable**
- [x] **Incremental transcript persistence.** Active terminal output is
  rebuilt + stored as a complete string ~1/s per run
  (`crates/app/src/run.rs:675` pre-split). Append chunks / checkpoint less
  often, write off the render path, bound retained size. Do this after the
  module split lands. (AUDIT.md deferral.) — **model: opus**

## Phase 4 — Power features usable in-app (not JSON + CLI only)

**Status 2026-07-20: complete.** Settings gained full editors — MCP servers
(stdio/http, allow/deny, scope), keep unlock + secret add/remove + proxy
upstreams (values never rendered, passphrase never persisted, env-sourced
tokens never written back), custom agents, and layouts. Plugin `[[trigger]]`s
fire for all 8 ADE events but only for explicitly enabled plugins (process
runtimes require a trust confirmation; hung plugins killed at 30s;
panels/webviews/tools honestly labeled "not yet active"). Per-hunk staging
works end-to-end: hunks are sliced byte-exact from git's own diff, successful
runs defer their commit to merge time, and merge/PR finalize with exactly the
staged subset. Setup commands run per-command with separate output, a cancel
button, and timeout kills. Provider sign-in probes (claude/gh/codex; gemini
honestly Unsupported) show per-account status with a re-check. The unwired
`remote` dep is dropped.

Batch 4A (parallel):
- [x] **MCP servers editor in Settings.** `mcp_servers` is
  settings.json-only today; the Settings surface shows toggles but says "add
  in settings.json" (`crates/app/src/settings.rs:703+`). Build add/edit/
  remove for stdio + HTTP upstreams with allow/deny lists, writing through
  `config::edit`. — **model: opus**
- [x] **Keep + proxy GUI.** Secrets are CLI-only (`asylum keep set`) with an
  env-var passphrase; upstreams are JSON-only
  (`settings.rs:606-701`, `crates/keep/src/lib.rs`). Add an in-app unlock
  flow, secret add/remove (scoped global/project), and an upstream editor.
  The crypto layer is 1.0-quality — this is UI over existing functions.
  — **model: opus**
- [x] **Custom agents + layouts editors.** Both are read-only JSON
  instructions today (`settings.rs:659-666,767-775`). In-app create/edit for
  custom agents and fan-out layouts; keybindings can stay JSON with a "open
  keybindings.json" affordance. — **model: opus**
- [x] **Drop the unwired `remote` dependency from app.** `crates/remote` has
  zero call sites outside its own tests but ships in the binary
  (`crates/app/Cargo.toml:46`); remove the dep until SSH execution is
  actually surfaced (backlog). — **model: haiku**

Batch 4B (parallel):
- [x] **Wire `[[trigger]]` dispatch + honest plugin UI.** 4 of 5 plugin
  contribution types parse but do nothing
  (`crates/plugin/src/lib.rs:13-18`), while the Plugins surface renders the
  declared capabilities as if live (`crates/app/src/plugins.rs:100-136`).
  Wire trigger auto-dispatch on ADE events (`run_finished`,
  `worktree_created`, …) through `pluginrt`, and label panels/webviews/tools
  clearly as not-yet-active. — **model: opus**
- [x] **Per-hunk staging in the side-by-side diff** (roadmap "Next"). Select
  hunks in review → stage/apply only those on merge. Touches `git` (apply
  plumbing) + `diff.rs` UI. — **model: opus**
- [x] **Background/cancellable project setup commands** with per-command
  output (roadmap "Next"). — **model: opus**
- [x] (stretch) **Provider sign-in probes + live usage feeds** for the
  account meter (roadmap "Next") — nice for 1.0, not gating.
  — **model: opus**

## Phase 5 — Docs, packaging, launch

**Status 2026-07-20: complete except two user-gated items.** CHANGELOG,
CONTRIBUTING, SECURITY, issue forms, and the PR template exist; the CLI has
38 per-subcommand help topics + `completions bash|zsh|fish` (with anti-drift
tests, and three real argv panics fixed along the way); update notifications
carry release notes + a View-release action; the release workflow gains
checksums.txt, per-platform smoke tests that execute `asylum --version`
(both binaries answer it), and a decided panic=unwind rationale; docs/book/
site were reconciled in 18 files (including five that still claimed
immediate-commit semantics) and the roadmap's shipped items moved to Built.
Remaining: screenshots (below) and the release cut (user).

Batch 5A (parallel):
- [ ] **README + site screenshots.** The repo front door has zero images
  (`product.png` exists only under `site/public/`). Capture current-UI
  screenshots (after Phase 1/2 land) for README hero + key flows; the user
  should bless the picks. — **model: sonnet** — *2026-07-20: blocked on user
  presence — the machine was locked during the autonomous run, so captures
  show the lock screen. Validated recipe for next session (user unlocked):
  `XDG_DATA_HOME=<tmpdir> cargo run -p app` stages a clean workspace with no
  personal data (first-run onboarding renders; open the asylum repo itself
  via File ▸ Open… for a board shot); capture the window region with
  `screencapture -R<x,y,w,h>` using bounds from System Events; save as
  `assets/` images and wire into README + site. Everything else in 5A is
  done.*
- [x] **CHANGELOG.md** seeded from git history, plus a keep-it-current note
  in the release checklist. — **model: haiku**
- [x] **Community/trust files.** CONTRIBUTING.md, SECURITY.md (report
  channel + supported versions), `.github/ISSUE_TEMPLATE/` +
  `PULL_REQUEST_TEMPLATE.md`. Match the repo's honest, terse voice.
  — **model: haiku**
- [x] **CLI help + completions.** One static `print_help()` today, no
  per-subcommand help (`crates/cli/src/main.rs`). Add `--help` per
  subcommand and shell completions (keep the hand-rolled dispatcher —
  no clap dependency needed unless it stays small). — **model: sonnet**

Batch 5B (parallel):
- [x] **Update-check UX.** Launch check exists and posts an Inbox note with
  a link (`crates/update/src/lib.rs`); add release notes display + a direct
  download action. In-app self-update stays out of 1.0 (package managers
  cover it) — document that stance. — **model: sonnet**
- [x] **Release pipeline completion** (AUDIT.md partial): gate tagged
  packaging on the full suite from within `release.yml`, add per-package
  smoke tests, publish checksums (+SBOM if cheap), and settle the explicit
  panic strategy now that Phase 0C gave it logging/crash visibility.
  — **model: sonnet**
- [x] **Reconcile docs/book/site with the 1.0 UI** — nav changes, renamed
  review surface, servers section, new settings editors; the book (15
  chapters) and video-course pages reference the old shapes.
  — **model: sonnet**

- [ ] (human) **Cut the 1.0 release**: bump the version, push, verify the
  tag workflow, publish, confirm Homebrew/Scoop refresh, spot-check installs
  on clean machines. Gated on the signing secrets being set first (see
  `packaging/signing.md`).

## Backlog (post-1.0)

Competitive gaps vs hermes-agent worth owning in Asylum's shape, plus
remaining deferrals — none gate 1.0:

- Messaging-channel delivery (Slack/Telegram/Discord bridges over the
  companion event stream) so the fleet is reachable where you already are.
- Scheduled/unattended runs (cron-style task launches with delivery).
- Headless mode: run fleet + queues without the GUI (CLI/companion-first).
- Cross-run memory: FTS5 recall over transcripts + notes surfaced as agent
  context automatically (today's notes attach is manual).
- Session restore of live ptys across restart (roadmap "Later").
- Remote SSH execution surfaced in UI/CLI (re-add `remote` when real).
- Plugin panels/webviews/tools dispatch (the remaining contribution types);
  plugin provenance/integrity + never-auto-enable (AUDIT.md deferral).
- Untrusted-workspace mode (AUDIT.md deferral).
- OS-keychain keep backend; Argon2id; per-agent proxy rate limits; AEAD AAD
  binding (AUDIT.md "remaining" notes).
- Kill orphaned agent processes on crash recovery via persisted PIDs.
- AUR package; Chocolatey community-feed automation.
- Native mobile shells around the companion API (roadmap "Later").
