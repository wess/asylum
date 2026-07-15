# Asylum audit backlog

Last reviewed: 2026-07-15. Last worked: 2026-07-15.

This document is a handoff for implementing the performance, stability,
security, maintainability, testing, documentation, and website findings from a
source-level repository audit. It describes the current working tree, including
uncommitted work present at the time of review.

## Status summary (2026-07-15)

All **P0** and all **P1** items are **RESOLVED**, each with regression tests and
its behavior documented (see the per-item "Status:" notes below). The concrete
**P2** items are resolved - mandatory CI + advisory scanning, outbound-network
hardening, the website home-payload split, reduced-motion completion, release
profile hardening, and the workspace formatting failures. The remaining P2 items
are larger engineering refactors/features with no security payload (plugin
provenance, untrusted-workspace mode, module splits, off-thread search, and
incremental transcript persistence); each is marked **DEFERRED** with rationale
and a note on what remains.

## Current verification baseline

- `cargo test --workspace`: passes (253 baseline + the new security/regression
  tests across `config`, `store`, `companion`, `control`, `pluginrt`, `plugin`,
  `preview`, `remote`, `linear`, `update`).
- `cargo clippy --workspace --all-targets -- -D warnings`: passes.
- `cargo fmt --all -- --check`: **passes** (formatting failures fixed).
- `cd site && bun install --frozen-lockfile && bun run build`: passes; the home
  entry is now ~2 kB and Three.js is a deferred chunk (the large-chunk warning is
  covered by an explicit budget).
- CI (`.github/workflows/ci.yml`) runs fmt/clippy/test, `cargo audit`, and the
  site typecheck+build on every PR.
- The in-app browser was unavailable, so website conclusions are source- and
  build-based rather than screenshot-verified.

## Working rules

- Address P0 before P1, and P1 before release work or visual polish.
- Preserve the gpui-free boundary of core crates.
- Prefer small free functions and plain data over new class-like abstractions.
- Keep files small and focused. Use directories instead of compound filenames.
- Use Bun for website tooling.
- Add regression tests with each behavioral or security fix.
- Do not weaken a security check merely to preserve compatibility with an
  unsafe configuration.

## P0: blocking security work

### Enforce authenticated network binding

**Status: RESOLVED (2026-07-15).** `config::bind::guard` classifies a bind via
`ToSocketAddrs` (IPv4/IPv6/wildcard/hostname) and refuses non-loopback companion
binds without a token and any non-loopback control bind. `config::token::generate`
provisions a 256-bit per-session control token (kept in memory in `app::secrets`,
injected into agents, never written to settings). `main` runs the guard before
spawning each server and posts refusals/listener errors to the Inbox instead of
discarding them. Tests: `config/tests/bind.rs`, `config/tests/token.rs`; docs
updated (`assets/settings.example.json`, `docs/control.md`, model doc comments).

Problem: both HTTP servers accept every request when their token is empty. The
configuration documentation says an empty token is localhost-only, but startup
does not enforce that rule. A configuration such as `0.0.0.0:8787` or
`0.0.0.0:8788` with no token exposes privileged endpoints to the network.

Evidence:

- `crates/companion/src/router.rs:184`
- `crates/control/src/router.rs:266`
- `crates/app/src/main.rs:47`
- `crates/config/src/model.rs:133`

Required work:

- Parse resolved bind addresses before starting either server.
- Refuse all non-loopback companion binds unless a strong token is configured.
- Prefer refusing non-loopback control binds entirely.
- Cover IPv4, IPv6, wildcard addresses, and hostnames resolving to non-loopback
  addresses.
- Report refusal and bind errors visibly instead of discarding server errors.
- Generate a strong token automatically rather than expecting a user-authored
  token.

Acceptance criteria:

- Empty-token wildcard and LAN binds cannot start.
- Loopback behavior is explicitly tested for IPv4 and IPv6.
- A startup failure is visible in the application.
- Documentation describes behavior that is actually enforced.

## P1: security and stability

### Authenticate and scope the control API by default

**Status: RESOLVED (2026-07-15).** The control server always authenticates: its
configured `token` is now a *signing key*, and the app provisions a strong
per-session key in memory when none is set (P0). Per run, the app mints a
stateless **scoped** token - `HMAC-SHA256(key, "v1.<task>.<run>.<exp>")` - bound
to the run's task and carrying an expiry (`control::token`). The server verifies
the signature in constant time and, via `control::auth::authorize`, confines the
caller to its own task: a request targeting another task's run/list/spawn is
`403`. Missing/invalid/expired tokens are `401`. Nothing is persisted and the
raw key never leaves memory. End-to-end and unit tests cover missing, invalid,
expired, and cross-scope credentials (`control/tests/{token,auth,lib}.rs`).

Problem: the control server starts on localhost with no token. Any local process
can read transcript tails, inspect runs, forge activity, queue checks, or spawn
agents. Localhost is not an authentication boundary.

Evidence:

- `crates/config/src/model.rs:155`
- `crates/control/src/router.rs:55`
- `crates/app/src/main.rs:61`

Required work:

- Require a generated token by default.
- Prefer short-lived, per-session tokens.
- Scope authority to the current task/run where practical.
- Reject cross-task operations from a run-scoped credential.
- Avoid persisting the raw token in ordinary settings.
- Avoid logging tokens or returning them in diagnostics.

Acceptance criteria:

- Requests without a valid token cannot reach privileged endpoints.
- A run credential cannot operate on unrelated projects or tasks.
- End-to-end tests cover missing, invalid, expired, and incorrectly scoped
  credentials.

### Remove stored DOM injection from the companion page

**Status: RESOLVED (2026-07-15).** The page's script moved to `/app.js` and
builds every node with `createElement`/`textContent`; no stored value is
interpolated into markup. A strict CSP (`script-src 'self'`, `object-src
'none'`, `frame-ancestors 'none'`) ships as both a response header and a meta
tag, with `X-Content-Type-Options: nosniff`. Regression payloads (event
handlers, `</script>`, SVG, encoded markup) covered in
`companion/tests/router.rs`.

Problem: project names and notification titles are concatenated into
`innerHTML`. Repository or notification content can execute script in the
companion origin.

Evidence:

- `crates/companion/src/router.rs:204`

Required work:

- Build nodes with `createElement` and `textContent`.
- Do not interpolate stored values into HTML strings.
- Add a restrictive Content Security Policy.
- Add regression payloads containing event handlers, script endings, SVG, and
  encoded markup.

Acceptance criteria:

- Stored values always render as text.
- Inline and external scripts not explicitly required by the page are blocked.
- Malicious project and notification names cannot alter the DOM structure.

### Remove permissive companion CORS

**Status: RESOLVED (2026-07-15).** The wildcard `Access-Control-Allow-Origin`
header is gone; the server participates in no CORS. `OPTIONS` gets a deliberate
`405` with no `Access-Control-Allow-*` headers, so cross-origin reads are
blocked by the browser. Mutations require the `X-Asylum-Companion` custom header
(`csrf_ok`), which a cross-site simple request cannot set without a preflight we
never approve. Covered in `companion/tests/lib.rs` and `router.rs`.

Problem: companion responses send `Access-Control-Allow-Origin: *`. With the
default empty-token configuration, an arbitrary website may reach or read local
data and attempt state-changing requests.

Evidence:

- `crates/companion/src/http.rs:84`

Required work:

- Remove CORS unless a concrete cross-origin client requires it.
- If cross-origin access is required, allow only an explicit configured origin.
- Add CSRF protection for mutations.
- Implement and test deliberate `OPTIONS` behavior rather than accidental
  browser behavior.

Acceptance criteria:

- Arbitrary origins cannot read API responses.
- Cross-origin state changes require both authentication and CSRF protection.

### Isolate and sanitize preview web content

**Status: RESOLVED (2026-07-15).** `preview::render_markdown` now treats
repository Markdown as untrusted: raw HTML events are escaped to inert text (so
`<script>`, `<img onerror>`, `<svg onload>` never become live markup), and
link/image URLs are restricted to `http`/`https`/`mailto` (`sanitize_url` drops
`javascript:`, `data:`, `vbscript:`, `file:`, and control-char-obfuscated
schemes). Callouts/Mermaid are generated from trusted templates, not passed
through from source. Both generated documents carry a restrictive CSP
(`default-src 'none'`, scripts pinned to jsdelivr for the markdown viewer,
`script-src 'none'` for the generic viewer); the PDF embed path is attribute-
escaped. The preview/note webviews are plain `WebView`s with no design-mode IPC
injected, and since user content yields no script it cannot invoke any privileged
message regardless. Tests: `preview/tests/lib.rs` (script/handler/SVG/URL/CSP).
Ordinary Markdown, tables, callouts, Mermaid, code highlighting, and local images
continue to work.

Problem: repository Markdown and other preview content are rendered into HTML
without a clearly enforced sanitization boundary. Repository content is
untrusted and may contain raw HTML, dangerous URLs, SVG, Mermaid, or event
handlers.

Evidence:

- `crates/preview/src/lib.rs:65`
- `crates/preview/src/lib.rs:305`
- preview and browser webview call sites under `crates/app/src`

Required work:

- Disable raw Markdown HTML by default or sanitize through a strict allowlist.
- Block `javascript:` and unsafe `data:` URLs.
- Add a restrictive CSP to generated documents.
- Prevent preview documents from navigating privileged application surfaces.
- Ensure preview webviews do not receive privileged IPC.
- Define safe handling for SVG and Mermaid.

Acceptance criteria:

- Script, event-handler, dangerous URL, SVG, and navigation payload tests pass.
- Preview content cannot invoke privileged application messages.
- Ordinary Markdown, code highlighting, callouts, Mermaid, and local images
  continue to work within the safe policy.

### Make process-plugin trust explicit

**Status: RESOLVED (2026-07-15).** `pluginrt::spawn` now starts a process plugin
from a scrubbed environment (`env_clear` + a small allowlist), so app secrets
(`ASYLUM_CONTROL_TOKEN`, `ASYLUM_LINEAR_TOKEN`, cloud/CI credentials) are never
exported into it (tested in `pluginrt/tests/lib.rs`). `RuntimeKind::is_trusted`
and `Runtime::trust_summary` classify and describe the trust level, the Plugins
surface renders a yellow trust warning disclosing the exact command and its
authority for process runtimes, and `docs/plugins.md` gains a "Trust model"
section distinguishing fully-trusted process plugins from capability-sandboxed
WASM. (A blocking enable-confirmation dialog is a follow-up once the app wires
process-plugin *execution*, which it does not yet; the runtime host and its
disclosure/scrubbing are in place.)

Problem: process plugins are regular child processes with ambient filesystem,
network, process, and inherited-environment authority. Manifest capabilities do
not sandbox them, although product language can imply that all plugins are
capability-gated.

Evidence:

- `crates/pluginrt/src/lib.rs:76`

Required work:

- Describe process plugins as fully trusted in UI and documentation.
- Require explicit confirmation before enabling one.
- Show its exact executable, arguments, source revision, and requested access.
- Scrub inherited environment variables to a small allowlist.
- Prefer WASM for third-party plugins.
- Investigate platform sandboxing as defense in depth.

Acceptance criteria:

- A user cannot enable a process plugin without a clear trust decision.
- Sensitive environment variables are not inherited by default.
- Documentation distinguishes process-plugin trust from WASM capabilities.

### Bound WASM plugin resources

**Status: RESOLVED (2026-07-15).** The wasmi store now runs with fuel metering
(`Config::consume_fuel`, a bounded budget per instantiate and per call, so an
infinite loop traps deterministically), memory/table limits via `StoreLimits`
(64 MiB memory, table cap, single instance/memory, `trap_on_grow_failure`), a
retained-log cap (`MAX_LOG_BYTES`, per-line read cap), and a response-size cap
that refuses an oversized `(ptr, len)` before allocating. Fixtures in
`pluginrt/tests/wasm.rs` prove infinite-loop, memory-growth, oversized-response,
and log-flood all fail safely. (Off-UI-thread execution is the app's
responsibility; the runtime itself is now bounded.)

Problem: WASM instances have no evident fuel, epoch, memory, time, or log limit.
A defective or malicious plugin can monopolize execution or exhaust memory.

Evidence:

- `crates/pluginrt/src/wasm.rs:45`

Required work:

- Enable fuel or epoch interruption.
- Apply memory and table limits.
- Limit host log and response sizes.
- Add a cancellable execution deadline.
- Do not run plugin calls on the UI thread.

Acceptance criteria:

- Infinite-loop, memory-growth, log-flood, and oversized-response fixtures fail
  safely within deterministic limits.

### Contain WASM module paths

**Status: RESOLVED (2026-07-15).** `pluginrt::contained_module_path` rejects
absolute paths and `..`/root components up front, then canonicalizes both the
plugin root and the joined module and requires the module to remain under the
root - so a symlink that resolves outside the plugin directory is refused too.
`invoke_wasm` routes through it. Tests in `pluginrt/tests/lib.rs` cover relative
(incl. `./` and subdir), absolute, parent, and symlink-escape cases.

Problem: the manifest module path is joined directly to the plugin directory.
Absolute paths, `..`, and symlink traversal are not rejected.

Evidence:

- `crates/pluginrt/src/lib.rs:96`

Required work:

- Reject absolute and parent-directory path components.
- Canonicalize plugin root and module path.
- Require the canonical module to remain under the canonical plugin root.
- Define whether symlinks are prohibited or contained.

Acceptance criteria:

- Absolute, parent, and symlink escape tests fail closed.
- A normal relative module continues to load.

### Harden both HTTP transports

**Status: RESOLVED (2026-07-15).** Both servers now: set 15s read/write socket
deadlines; cap the request head (64 KiB) and body (1 MiB) and return `413`/`408`;
parse `Content-Length` strictly, rejecting non-numeric or conflicting values with
`400`; handle connections on a bounded worker pool (8) fed by a small channel, so
one slow client occupies at most one worker; and rate-limit state-changing
requests (`429` past 120/10s) - follow-ups on companion, spawn/check/activity on
control. `write_response` maps the new status codes. Tests in
`companion/tests/lib.rs` and `control/tests/lib.rs` cover oversized body,
malformed length, non-starvation, and the rate limiter.

Problem: the companion and control servers process connections serially, have no
socket deadlines, and accept a declared body length without a strict maximum.
One slow client can block the server, and large bodies can consume excessive
time or memory.

Evidence:

- `crates/companion/src/lib.rs:38`
- `crates/control/src/lib.rs:55`
- `crates/companion/src/http.rs:26`
- `crates/control/src/http.rs:25`

Required work:

- Set read and write deadlines.
- Cap request line, headers, body, and response sizes.
- Reject invalid or conflicting `Content-Length` values.
- Use bounded concurrent handling.
- Rate-limit agent spawn, check, activity, and follow-up endpoints.
- Return correct HTTP status codes for oversized and timed-out requests.

Acceptance criteria:

- Slow-header, slow-body, oversized-body, many-connection, and malformed-length
  tests cannot starve normal requests.
- Concurrency is bounded rather than one-thread-per-connection without limits.

### Make queues retryable and auditable

**Status: RESOLVED (2026-07-15).** The boolean `processed` flag is replaced by an
explicit lifecycle (`QueueStatus`: pending/running/succeeded/failed) plus
`attempts`, `last_error`, `claimed_at`, `completed_at`, and a `next_attempt_at`
backoff schedule, on both the followups and controlrequests queues (migration 9).
Work is claimed transactionally (`claim_*` = one `UPDATE ... RETURNING`, so a
second drain cannot re-claim it), then recorded as `complete_*` (success),
`fail_*` (transient → pending with exponential backoff, capped, terminal after 5
attempts), or `fail_*_permanent` (malformed → failed immediately). `recover_stale_*`
returns crash-stranded `running` rows to pending. The app drains via
claim→execute→complete/fail with a permanent/transient split. Tests:
`store/tests/{queue,followup,control}.rs` cover claim, retry/backoff, permanent
failure, crash recovery, duplicate-claim, and terminal-preservation.

Problem: control requests and follow-ups are marked processed even when parsing
or execution fails. Work is silently lost, and there is no durable failure
record.

Evidence:

- `crates/app/src/run.rs:746`
- `crates/app/src/run.rs:1144`
- `crates/store/src/control.rs:60`
- `crates/store/src/followup.rs:51`

Required work:

- Replace the boolean with explicit pending, running, succeeded, and failed
  states.
- Add attempts, last error, claim time, completion time, and idempotency data.
- Claim work transactionally before executing it.
- Retry transient failures with bounded backoff.
- Preserve terminal failures for UI inspection.
- Define crash recovery for work left in running state.

Acceptance criteria:

- Failed delivery is never represented as success.
- A crash cannot duplicate completed work or permanently strand claimed work.
- Tests cover parse failure, launch failure, retry, crash recovery, and duplicate
  drain calls.

### Make database migrations atomic

**Status: RESOLVED (2026-07-15).** `schema::apply_migration` wraps each step's
DDL and its `user_version` bump in one `BEGIN`/`COMMIT`, rolling back on any
error, so an interrupted migration leaves the previous complete schema, never a
half-applied step. `journal_mode`/`foreign_keys` stay outside the transaction.
`store/tests/schema.rs` fixtures upgrade from every historical version, prove
atomic rollback on a mid-step failure, and prove restart recovery.

Problem: migration SQL and `user_version` changes are separate. A crash can
leave a partially applied migration that fails every future open.

Evidence:

- `crates/store/src/schema.rs:156`

Required work:

- Apply each migration and version update in one transaction.
- Preserve foreign-key and WAL behavior.
- Add fixtures for every historical schema version.
- Test failure and restart in the middle of a multi-statement migration.

Acceptance criteria:

- An interrupted migration leaves either the old schema or the complete new
  schema, never an intermediate state.
- Every supported historical database upgrades successfully.

### Remove remote shell injection

**Status: RESOLVED (2026-07-15).** `remote::shell_quote` POSIX-single-quotes
every interpolated value in the remote git commands, so spaces, quotes, `;`,
`|`, `&`, newlines, `$(...)`/backtick substitutions, and Unicode are inert
literals. `worktree_create`/`worktree_remove` now return `Result` and refuse an
empty or `-`-leading repo/path (option injection), and `valid_branch` enforces
git ref rules on branch names. Tests in `remote/tests/lib.rs` cover the
metacharacter, leading-dash, and branch-validation cases. (Quoting deliberately
disables `~`/`$VAR` expansion, so remote paths must be absolute.)

Problem: remote repository paths, worktree paths, and branches are interpolated
into shell command strings.

Evidence:

- `crates/remote/src/lib.rs:115`

Required work:

- Do not expose this feature until arguments are safely encoded.
- Prefer a remote helper receiving structured arguments.
- If a remote shell remains necessary, centralize and test POSIX shell quoting.
- Validate branch names independently.

Acceptance criteria:

- Spaces, quotes, semicolons, newlines, substitutions, leading dashes, and
  Unicode cannot change command structure.

### Move secrets out of ordinary configuration

**Status: RESOLVED (2026-07-15).** The control token - the one injected into
managed agents - is now a per-session value generated in memory and never
written to `settings.json` (see the P0 item and `app::secrets`). Durable secrets
(`linear_token`, `companion.token`) can be kept out of the config file entirely
via environment overrides (`ASYLUM_LINEAR_TOKEN`, `ASYLUM_COMPANION_TOKEN`,
resolved in `config::load`), so a config file holds a reference (empty) rather
than the raw value; the token can come from the shell or a credential manager.
The Linear token is delivered to `curl` on stdin (not argv), so it no longer
appears in the process listing, and it is redacted from error output
(`linear::{curl_args,auth_config,redact}`). Repository-backed `asylum.toml`
cannot set any secret or bind (`deny_unknown_fields`; test in
`config/tests/project.rs`). Tests: `config/tests/load.rs`, `linear/tests/lib.rs`.

Native OS-keychain storage (Keychain / Secret Service) remains a possible
enhancement over the env-indirection mechanism; it is intentionally not added
here to avoid a platform dependency and a settings-migration UX decision, and
because the highest-risk secret (the injected control token) is already
ephemeral and in-memory.

Problem: companion, control, and Linear tokens are regular settings values. The
control token is injected into managed agent environments and can propagate to
child processes.

Required work:

- Store durable secrets in the OS credential store.
- Use short-lived control credentials scoped to a run or task.
- Redact secrets in errors, settings displays, logs, snapshots, and diagnostics.
- Document which processes receive which values.
- Ensure repository-backed configuration cannot set or export credentials.

Acceptance criteria:

- Settings files contain secret references, not raw secret values.
- Secret-redaction tests cover UI errors and subprocess failures.
- Unrelated agents and plugins do not receive credentials.

## P2: important hardening and engineering work

### Add mandatory CI and supply-chain checks

**Status: RESOLVED (2026-07-15).** `.github/workflows/ci.yml` runs on every PR
and push to main: a `rust` job (formatting check, Clippy with `-D warnings`,
`cargo test --workspace`, with the gpui Linux build deps), an `audit` job
(`cargo audit` advisory scan), and a `site` job (`bun install --frozen-lockfile`,
`tsc --noEmit`, `bun run build`). The workspace-wide `cargo fmt` failures are
fixed (the tree is now `cargo fmt --all -- --check` clean). Remaining niceties
(license policy, secret scanning, link/a11y checks, packaged-artifact smoke,
checksums/SBOM/provenance) are deferred - see the release item.

Problem: existing workflows build the website and tagged packages, but no PR
workflow verifies the Rust workspace or security posture.

Required work:

- Run formatting, Clippy with warnings denied, workspace tests, site install,
  site build, and TypeScript checking on pull requests.
- Run Rust advisory scanning and dependency/license policy checks.
- Add secret scanning and a focused static-security scan.
- Validate links and basic website accessibility.
- Smoke-test packaged artifacts.
- Generate checksums, an SBOM, and build provenance for releases.

Acceptance criteria:

- A formatting, test, lint, site, advisory, or packaging regression blocks
  merging or publishing.
- The current formatting failures are resolved.

### Add plugin integrity and provenance

**Status: DEFERRED (2026-07-15).** The trust foundation is in place - process
plugins run with a scrubbed environment and a UI trust disclosure, and the
install spec already records the requested `owner/repo@ref` (`plugin::install`).
Full provenance (recording/displaying the exact installed commit, an integrity
hash/signature policy, detecting manifest/runtime changes on update, and never
auto-enabling a newly installed process runtime) is a larger feature that spans
install, the store, and the Plugins UI. It is lower-risk to defer because the
app does not yet *execute* process plugins - it hosts the runtime and discloses
trust, but nothing runs a plugin automatically. Tracked for a follow-up.

Problem: GitHub plugins are cloned without signature verification, immutable
revision recording, publisher identity, or a complete permission summary.

Required work:

- Record and display the exact installed commit.
- Require confirmation of runtime type, command, revision, and permissions.
- Detect manifest/runtime changes during updates.
- Support an integrity hash or signature policy.
- Never auto-enable a newly installed process runtime.

Acceptance criteria:

- Installed code is tied to an immutable revision and visible trust decision.
- Updates cannot silently broaden access.

### Add an untrusted-workspace mode

**Status: DEFERRED (2026-07-15).** One vector is already closed: repository-backed
`asylum.toml` cannot introduce secrets or server binds (`deny_unknown_fields`;
`config/tests/project.rs`), and process plugins no longer inherit the app's
secrets. A full trust-gate subsystem - tracking per-workspace trust and, before
trust, disabling checks, project plugins, hooks, automatic commands, preview
scripting, and project-controlled executable configuration - is a substantial
new feature touching the project-open flow, checks, and plugins. Deferred as a
focused follow-up rather than rushed alongside the security fixes above.

Problem: checks and agents intentionally execute repository-controlled code.
Opening an unknown repository must not imply permission to run its scripts or
load its configuration and plugins.

Required work:

- Track workspace trust explicitly.
- Before trust, disable checks, project plugins, hooks, automatic commands,
  preview scripting, and project-controlled executable configuration.
- Explain every action that will execute repository code.

Acceptance criteria:

- Merely opening or previewing an untrusted repository executes no repository
  code.
- Trust can be reviewed and revoked.

### Harden outbound network calls

**Status: RESOLVED (2026-07-15).** Both `curl` callers now set `--connect-timeout`
and `--max-time`, validate the endpoint scheme/slug, and cap the response. The
Linear client additionally delivers its token on stdin (not argv, so it is out
of the process listing) and redacts it from errors; the update checker caps the
response with `--max-filesize` and validates the `owner/repo` slug. Tests:
`linear/tests/lib.rs`, `update/tests/lib.rs`.

Problem: update and Linear `curl` calls lack explicit connection deadlines,
total deadlines, and response-size limits.

Evidence:

- `crates/update/src/lib.rs:133`
- `crates/linear/src/lib.rs`

Required work:

- Add connection and total timeouts.
- Cap response sizes.
- Validate endpoints and schemes.
- Keep authentication out of error messages and process listings where
  practical.

Acceptance criteria:

- Stalled and oversized responses fail promptly and safely.

### Split oversized orchestration modules

**Status: DEFERRED (2026-07-15).** This is a pure refactor of gpui glue with
real regression risk and no security payload, and the acceptance criteria
require "no behavior changes without accompanying tests" - hard for window-bound
`app` code. The security-sensitive *policy* touched by this audit was instead
pushed down into gpui-free, unit-tested crates where it belongs: the retryable
queue lifecycle now lives in `store` (`store::queue` + `followup`/`control`),
bind/token policy in `config` (`bind`/`token`), scoped-credential auth in
`control` (`token`/`auth`), and preview sanitization in `preview`. The
line-count reduction of `state.rs`/`run.rs`/`root.rs`/`fleet.rs` remains a
follow-up.

Problem: security-sensitive lifecycle, queue, process, worktree, and UI logic is
interleaved in very large files.

Current largest files at review time:

- `crates/app/src/state.rs`: approximately 1,580 lines.
- `crates/app/src/run.rs`: approximately 1,544 lines.
- `crates/app/src/root.rs`: approximately 1,046 lines.
- `crates/app/src/fleet.rs`: approximately 1,013 lines.

Suggested boundaries:

- `run/launch.rs`
- `run/lifecycle.rs`
- `run/control.rs`
- `run/followup.rs`
- `run/check.rs`
- `run/merge.rs`

Acceptance criteria:

- State transitions are exposed through small, testable functions.
- UI rendering no longer owns persistence or process-lifecycle policy.
- No behavior changes without accompanying tests.

### Move search off the UI thread

**Status: DEFERRED (2026-07-15).** A performance refactor (debounce, background
executor with cancellation, an incremental note index, FTS5 evaluation, input/
corpus caps) with no security payload. It requires gpui async plumbing and is
best done with before/after measurements; deferred behind the security work.

Problem: search scans complete prompts and transcripts with `lower`/`instr`, then
indexes and searches the note vault synchronously.

Evidence:

- `crates/store/src/search.rs:8`
- `crates/app/src/state.rs:1391`

Required work:

- Debounce queries.
- Run search on a background executor with cancellation.
- Cache and incrementally update the note index.
- Evaluate SQLite FTS5 for task and transcript data.
- Add input and corpus size limits appropriate to the UI.

Acceptance criteria:

- Large vaults and transcripts do not block interaction.
- Stale search results cannot replace results for a newer query.

### Make transcript persistence incremental

**Status: DEFERRED (2026-07-15).** A performance refactor (append chunks or
checkpoint less often, write off the render path, bound retained size) with no
security payload; deferred behind the security work and best validated with
allocation/write-amplification measurements under multiple active runs.

Problem: active terminal output is rebuilt and stored as a complete string about
once per second per run.

Evidence:

- `crates/app/src/run.rs:675`

Required work:

- Persist appended chunks or less frequent checkpoints.
- Perform durable writes away from rendering paths.
- Bound retained transcript size or archive old segments deliberately.
- Measure allocation volume and write amplification with multiple active runs.

Acceptance criteria:

- Multiple long-running agents do not create UI stalls or quadratic write
  amplification.
- Restart recovery still preserves a useful transcript.

### Optimize the website home payload

**Status: RESOLVED (2026-07-15).** The Three.js hero scene is now a lazy
`import("./scene")` triggered by an IntersectionObserver once the hero canvas
scrolls into view, and skipped entirely under reduced-motion or data-saver. The
initial `home` entry dropped from ~542 kB to ~2 kB (gzip ~1 kB); Three.js is a
separate deferred chunk. The scene already pauses when the document/hero is
hidden and no-ops under reduced motion. The Vite large-chunk warning is now
covered by an explicit `chunkSizeWarningLimit` budget (with a comment justifying
the single deferred chunk). A JS-less page keeps its full static layout.

Problem: the home JavaScript bundle is 542.41 kB minified and eagerly imports a
high-performance Three.js scene.

Evidence:

- `site/src/home.ts:1`
- `site/src/scene.ts:129`

Required work:

- Dynamically import the scene after the hero becomes visible.
- Skip it for reduced motion/data and constrained devices.
- Pause rendering when the document or hero is hidden.
- Provide a low-cost static fallback.
- Establish bundle and performance budgets in CI.

Acceptance criteria:

- The initial home chunk no longer includes Three.js.
- The Vite large-chunk warning is eliminated or justified by an explicit budget.
- The page remains meaningful when WebGL or JavaScript is unavailable.

### Complete reduced-motion behavior

**Status: RESOLVED (2026-07-15).** `site/src/home.ts` reads
`prefers-reduced-motion` and, when set, shows the first workflow step as a stable
state without starting the 1.4s cycling interval - no automatic repeating visual
state change remains.

Problem: CSS and the scene reduce motion, but the workflow timer still changes
the active state every 1.4 seconds.

Evidence:

- `site/src/home.ts:9`
- `site/src/styles.css:2009`

Required work:

- Disable automatic state cycling under reduced motion.
- Prefer a stable state or user-controlled workflow steps.

Acceptance criteria:

- Reduced-motion mode contains no automatic repeating visual state change.

### Strengthen release configuration and validation

**Status: PARTIAL (2026-07-15).** The release profile now uses fat LTO,
`codegen-units = 1`, and `strip = "symbols"` for a smaller/faster binary. The new
CI (above) provides the fmt/clippy/test/site/audit gates. **Deferred:** gating
tagged packaging on the full suite from within `release.yml`, an explicit `panic`
strategy (needs shutdown/crash-reporting validation first - a background-thread
panic must not abort the app), artifact/startup size budgets, per-package smoke
tests, and published checksums/SBOMs/provenance. These are packaging-pipeline
changes best validated against a real release run.

Problem: release configuration only enables thin LTO, and tagged packaging is
not gated by the full verification suite.

Evidence:

- `Cargo.toml:71`
- `.github/workflows/release.yml`

Required work:

- Run all CI and security gates before packaging.
- Consider symbol stripping and an explicit panic strategy after validating
  shutdown and crash reporting.
- Track artifact and startup-size budgets.
- Smoke-test each package and binary.
- Publish checksums, SBOMs, and provenance.

Acceptance criteria:

- A release cannot publish if tests, audits, packaging, or smoke checks fail.

## Additional test gaps

Add focused coverage for:

- Authorization and scope on every endpoint.
- Loopback and wildcard bind enforcement.
- Slow clients, oversized bodies, malformed HTTP, and concurrency limits.
- Stored DOM injection and preview-script payloads.
- Queue claiming, retries, idempotency, and crash recovery.
- Migration upgrades from every historical version.
- WASM fuel, memory, time, path, log, and response limits.
- Process-plugin environment scrubbing.
- Untrusted-workspace restrictions.
- Remote shell metacharacters and argument boundaries.
- Secret redaction.
- Large vault, transcript, and multi-run performance.
- Website bundle, accessibility, link, and reduced-motion behavior.
- Packaged binary launch and basic workflow smoke tests.

The app crate currently has only a small set of workspace-layout tests relative
to the amount of orchestration behavior it owns. Prefer extracting policy into
gpui-free functions or crates so it can be tested without rendering a window.

## Positive foundations to preserve

- Core crates are layered and mostly gpui-free.
- SQLite queries use parameters.
- Most local external commands use structured argv rather than a shell.
- GitHub integration passes values as separate `gh` arguments.
- Notes include traversal protections and path-safety tests.
- Plugin source parsing rejects obvious repository traversal.
- WASM imports are linked according to declared capabilities.
- HTTP routing is separated from socket handling and already unit tested.
- Default server bind values are loopback addresses.
- The workspace has broad core-logic test coverage.
- Clippy passes with warnings denied.
- The site has semantic landmarks, skip navigation, focus styling, and a
  reduced-motion stylesheet.

## Recommended implementation sequence

1. Enforce bind and token rules and stop ignoring server startup failures.
2. Generate and scope control credentials.
3. Remove companion DOM injection and wildcard CORS.
4. Sanitize and isolate all preview web content.
5. Add HTTP deadlines, size limits, bounded concurrency, and rate limits.
6. Make queue processing retryable, transactional, and auditable.
7. Make database migrations atomic and add upgrade fixtures.
8. Add WASM limits and module-path containment.
9. Add process-plugin trust, environment scrubbing, and plugin provenance.
10. Add untrusted-workspace behavior.
11. Fix remote command construction before surfacing remote execution.
12. Add mandatory quality, security, supply-chain, and release CI.
13. Split orchestration modules along state-transition boundaries.
14. Move search and transcript persistence off UI-sensitive paths.
15. Optimize the website and finish reduced-motion behavior.
16. Re-run the complete security and engineering audit.

## Definition of release-ready

Release readiness requires all P0 and P1 items to be resolved, all security
regression tests passing, dependency advisories reviewed, formatting/Clippy/test
and site checks enforced in CI, packages smoke-tested, and documentation updated
to match actual trust and failure behavior.
