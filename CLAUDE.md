# Asylum

Repository guidance for agent sessions.

## What this is

Asylum is a free, open-source desktop app for always-on AI teammates
("Agents"), written in Rust as a Cargo workspace. The GUI is built on
[gpui](https://github.com/zed-industries/zed) (pinned zed git rev) with the
[guise](https://github.com/wess/guise) component library, and embeds terminals
through `libsinclair` from [Sinclair](https://github.com/wess/sinclair). The
Agent engine runs on tokio; persistence is SQLite through sqlx. Models come from
the xAI API.

The GUI is the `app` crate, whose bin target is `asylumdev` so a dev build
never collides with an installed `asylum`; `scripts/bundle.sh` ships the same
binary as `asylum`.

`site/` is the static website (GitHub Pages via `.github/workflows/pages.yml`):
home, `tutorial/`, `docs/` (the user manual), and `admin/` (policy and
activity). Keep it in step with behavior: settings keys, shortcuts, limits,
and policy keys there are checked against the code by hand.

`docs/features.md` lists what the app does, how each part is built, and
what's deliberately left out. Keep it current when features change.

## Commands

```sh
cargo run -p app                  # build and launch
cargo build --release             # build the workspace
cargo test                        # all tests
cargo test -p agent               # one crate
cargo test -p agent --test e2e    # the engine end to end against a mock xAI API
cargo test -p computer -- --ignored   # live browser test (needs Chrome/Edge/Chromium)
cargo clippy --all-targets        # lint

scripts/bundle.sh                 # release build + dist/Asylum.app
scripts/dmg.sh                    # dist/Asylum.dmg (needs bundle first); with
                                  # CODESIGN_IDENTITY + NOTARY_PROFILE it signs,
                                  # notarizes, and staples
scripts/icon.sh                   # regenerate assets/icon.{png,icns}
```

Each crate keeps tests in a sibling `tests/` directory mirroring `src/`. Every
crate sets `autotests = false`, and each source file pulls its test file in as
a private module:

```rust
#[cfg(test)]
#[path = "../tests/foo.rs"]
mod tests;
```

Integration tests that use only the public API are declared as `[[test]]`
targets (see `crates/agent/tests/e2e.rs`).

## gpui and guise

gpui and `gpui_platform` come from zed rev `96285fc1`, the same rev Sinclair
pins. Cargo `[patch.crates-io]` entries do not propagate through git
dependencies, so the root `Cargo.toml` mirrors zed's (`async-process`,
`async-task`) and redirects the crates.io `gpui` that guise requests onto the
zed rev so one gpui resolves (`cargo tree -d` shows a single `gpui`). guise is
a git dependency pinned to `c472878` — the `sinclair-v1.9.1` compatibility
branch Sinclair vendors. `libsinclair` is a git dependency pinned to a Sinclair
commit. Requires Rust stable >= 1.96.

Release builds set `debug = "limited"` for `build-override`: ld-27037 links
proc-macro dylibs without debug info into files dyld rejects ("mis-aligned
LINKEDIT string pool").

`gpui_platform` is built with `runtime_shaders` because this machine's Xcode
lacks the Metal Toolchain component (`xcodebuild -downloadComponent
MetalToolchain` installs it; then the feature can go).

## Architecture

Layered bottom-up; each crate depends only on those below it.

- **`config`** — `agents.json` (JSONC) under `~/.config/asylum/` (not
  `settings.json`: another tool owns that file there and must not be touched): typed
  `Settings` with per-key defaults, diagnostics that never abort the load, a
  minimal-diff writer, and live file watching. `policy` reads the admin
  policy (`/Library/Application Support/asylum/policy.json`, or
  `ASYLUM_POLICY`): disabled/required plugins, locked rules, local-exec
  ceiling, forced Auto-review, template limits, required Team Agents, Network
  Controls, local egress, Cloud Agents, setup/check scripts, idle
  termination, and an OTel endpoint. `secret` keeps secrets (the xAI
  key, plugin tokens, Agent secrets) in the OS keychain — never in the file.
- **`store`** — SQLite via sqlx (WAL, foreign keys), hand-written migrations in
  `migrations/` applied with `sqlx::migrate!`. One module of free functions
  per entity: agents, sections, chats (direct and group, unread/attention,
  drafts), messages (FTS5 search, threads, reactions), memories (agent / team /
  person scope), skills (one library, per-Agent enablement), routines, runs
  (history capped per routine), approvals, Auto-review rules, secrets
  (metadata only), plugins + accounts, templates, notifications, usage, state.
- **`chat`** — the OpenAI-compatible chat-completions wire client (it serves
  every HTTP provider; app code reaches it as `::chat` because the app has its
  own `chat` module): wire types, SSE
  decoding, streamed tool-call accumulation, and retries with jittered
  backoff before any text arrives. Pure decoding is unit-tested.
- **`provider`** — model providers, mirroring ainz: `config::Profile`
  (HTTP endpoint or CLI process, credential pointer, known models), presets
  (xai, openai, anthropic, lite-llm, ollama, ollama-cloud, claude-code,
  codex, custom), credential resolution (env / Synapse / Keychain), the
  process runner (transcript on stdin; text, JSON result, or Claude
  stream-json out), `/models` discovery, `litellm` (key check, teams,
  team-scoped models, key reach), and `Provider::Fallback` (the next
  provider answers only when one fails before its first event). `Runtime::provider(agent)` picks
  an Agent's pinned profile/model or the default; CLI providers get no Asylum
  tools (they bring their own).
- **`mcp`** — Model Context Protocol client: stdio (Command plugins) and
  Streamable HTTP (remote plugins) transports, tools/list + tools/call, and
  the MCP OAuth flow (discovery, dynamic registration, PKCE, loopback
  redirect, refresh).
- **`slack`** — Team Agents in Slack: app manifest, Socket Mode client,
  Web API calls, and the routing rules (DMs, mentions, followed threads).
- **`computer`** — the one computer every Agent shares: `workspace/` (the
  durable shared filesystem, confined path resolution), a shell that runs
  under a macOS `sandbox-exec` profile allowing writes only to the workspace
  and caches (the local stand-in for the cloud VM), secret injection with
  output redaction, web fetch/search, and a real Chromium over CDP
  (`chromiumoxide`) with a shared profile (shared sign-ins) and one page per
  Agent (its "screen"): numbered-element snapshots, click/type/keys, screenshots,
  takeover input, secret fill, and Teach-a-task recording. `proxy` is the
  egress gate for Network Controls: the browser launches through it and
  sandboxed commands can reach the network only through it.
- **`schedule`** — five-field cron evaluation in the user's timezone (DST
  safe), intervals, and the "When to run" English rendering.
- **`voice`** — cpal microphone/speaker threads, PCM math, xAI speech-to-text
  (dictation), text-to-speech (voice memos), and realtime voice chat.
- **`agent`** — the Agent runtime. `Runtime` owns the pool, computer, browser,
  plugin hub, and per-Agent job queues (a user message preempts work in that
  chat; routine runs and handoffs queue). A `turn` streams the model into a
  message (`tools::Sheet`), runs tool calls through `approve` (Free / Review /
  Consequential / Local classes; Auto-review on the fast model with Ask-first
  and Allow rules; "Always allow" saves a rule), waits on cards (approvals,
  takeover, secrets) and loops. `part::Part` is the card protocol stored in
  `messages.parts`. `audit` records actions and control-plane changes (and `otel` exports
  spans); `admin` serves the local Admin API. `ticker` fires routines, expires background approvals,
  runs the inactivity guard, watches disk, and backs up daily; `webhook`
  serves routine triggers on `127.0.0.1`. `api/` is everything the UI calls.
- **`app`** — the gpui application: windows, sidebar, conversation, cards,
  details pane, computer panel (live screen, takeover, files, terminal via
  `libsinclair::termview::TermView`), the built-in browser (`web`, a guise
  `WebView` in the right pane for links and workspace previews), palette,
  Marketplace, settings (`settings/gateway.rs`: LiteLLM keys, teams,
  reasoning, fallbacks, provider check),
  onboarding, voice chat. `tk` owns the tokio runtime; engine calls run there
  and are awaited from gpui tasks; engine `Event`s are forwarded into gpui.

### Process modes (`app/src/cli.rs`)

`asylumdev ask <agent> <message>`, `asylumdev providers`, `asylumdev plugin
add <id>`, and `asylumdev pin <agent> <provider> [model]` run the engine
without a window. `ASYLUM_SHOW=<screen>` (with optional `ASYLUM_URL`) opens a
given screen at launch for screenshots; `ASYLUM_TRACE=<file>` logs each model
exchange.

### Translations

UI strings go through `i18n::t`/`tf` with English as the key. Sources are
`crates/app/i18n/<code>.json`; `scripts/i18n.py` generates the sorted tables
in `src/i18n/lang/` (`--extract` lists new source strings, `--missing` counts
gaps). Regenerate after adding strings.

## Conventions

- File names are lowercase with no `-`/`_`; prefer `thing/{mod.rs,part.rs}`
  over `thing_part.rs`. Small, focused files. Functional style: free
  functions over data; structs for state and data, not behavior bags.
- rustfmt: 2-space indent, width 100 (`rustfmt.toml`).
- Keep gpui out of every crate but `app`; keep I/O out of the pure modules
  (`chat::sse`, `schedule`, `agent::{prompt,history,route,review}`) so they
  stay unit-testable.
- The user handles git. Commit messages and PRs carry no assistant
  attribution.
