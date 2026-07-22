# Asylum

**An Agent Development Environment.** Run a fleet of AI coding agents in
parallel — each isolated in its own git worktree — then compare their work and
merge the winner.

Asylum is a native Rust application. The GPU-accelerated UI is built on the
`gpui` library, with the [guise-ui](https://github.com/wess/guise) component
library, terminals powered by
[libsinclair](https://github.com/wess/sinclair), and SQLite for persistence.
Plugins use a `plugin.toml` manifest model.

## Why

Traditional IDEs are built for one human at a keyboard. Parallel-agent wrappers
stop at a terminal. Asylum is the whole environment for *agents*: fan one prompt
across several agents (Claude Code, Codex, OpenCode, Gemini, Aider, Cursor),
each working in an isolated worktree, watch them run, review annotatable diffs,
and merge the best result — without branch juggling or stashing.

## Status

Each feature is backed by tested logic and a working UI or CLI surface. See
[`docs/parity.md`](docs/parity.md) for the feature-by-feature matrix.

- **Fan-out orchestration** — one prompt across N agents, each in an isolated
  worktree; run execution on a real pty; compare, then merge the winner or open a
  PR (`agent`, `runner`, `git`, `github`). Reusable **fan-out layouts** race a
  named set of agents in one pick (`duel`, `triad`, `swarm`, or your own).
- **Semantic agent states** — every run shows a live activity —
  **working / blocked / done / idle** — classified from its output, so the board
  tells you at a glance which of your parallel agents is *blocked waiting on you*
  (`agent::activity`).
- **Agent control surface** — a running agent can orchestrate the fleet from
  inside its worktree: spawn a helper run, read a sibling, run checks, report its
  own state, and wait on another run, over a local JSON API it learns from a
  skill (`control`, `asylum control`).
- **Annotatable diff review** — inline comments shipped back to agents, plus
  PASS/FAIL checks and branch chips (`git`, `store`, `checks`).
- **Embedded terminal** (splittable, powered by
  [libsinclair](https://github.com/wess/sinclair)), **code editor** + file tree,
  **markdown/image/PDF preview**, and an **embedded browser with design mode**
  (click an element → its HTML/CSS to an agent).
- **GitHub / Linear** integration, **cross-worktree search**, **command palette**
  + **quick-open** (fuzzy), **desktop notifications** + unread inbox, **accounts**
  + usage, **pinned/recent** projects, and a collapsible icon-only activity rail.
- **Project memory** — private or repository-backed Markdown vaults with YAML
  properties, `[[wiki links]]`, backlinks, tags, templates, live preview, and
  durable task/run/check/PR links. Attached notes become agent context.
- **Plugins** — manifest system with a process runtime *and* a sandboxed WASM
  runtime (`wasmi`, capability-gated). Install from GitHub with
  `asylum plugin install <owner/repo>` and discover community plugins by topic.
- **CLI** (`asylum`) with computer-use automation, fleet control
  (`control`, `wait`, `plugin`, `layout`), and masked secrets (`keep` stores a
  credential encrypted; `call` spends it through the proxy so an agent uses a key
  it never sees), an opt-in **mobile companion** server (`:8787`, token
  required once enabled), and an **event stream** both expose so a phone or an
  agent can follow the fleet without polling.

## Install

Asylum is a packaged desktop app — running it needs no Rust toolchain. Every
release builds and publishes these artifacts to the
[releases page](https://github.com/wess/asylum/releases):

| Platform | Artifacts |
|---|---|
| macOS (Apple Silicon) | `Asylum.dmg` |
| Linux (x64 + arm64) | `.deb`, `.tar.gz`, `.AppImage` |
| Windows (x64) | `.msi`, `.zip` |

Package managers, once a release is published:

```sh
brew install --cask wess/packages/asylum        # macOS
scoop install https://raw.githubusercontent.com/wess/asylum/main/packaging/scoop/asylum.json   # Windows
```

A Chocolatey `.nupkg` is built and attached to each release; it is not pushed to
the community feed (that needs moderation), so install it from the downloaded
package. Until the first release is published, build from source as below.

The installed binary is `asylum`; a local `cargo run -p app` stays `asylumdev`,
so a dev build never collides with an installed release.

### macOS: unsigned, so Gatekeeper blocks it

The `.dmg` is **not signed or notarized**. The pipeline signs and notarizes only
when Developer ID secrets are present, and they are not provisioned yet, so
published builds carry no signature at all. macOS 15+ therefore refuses to open
the app, sometimes claiming it is damaged — it isn't, macOS just can't identify
the publisher. Control-click → Open **no longer** bypasses this. Either:

- Open it once, let it be blocked, then go to **System Settings → Privacy &
  Security → Open Anyway**, or
- clear the quarantine attribute:

  ```sh
  xattr -dr com.apple.quarantine /Applications/Asylum.app
  ```

Once real signing certificates are wired in, this step goes away.

### Windows: beta

The Windows binaries compile and link in CI but have **not been runtime-tested on
a real machine** — treat them as beta. 1.0 targets macOS and Linux; Windows
leaves beta once it has been runtime-tested on real hardware. The installers are also unsigned, so
SmartScreen shows an "unknown publisher" prompt until an Authenticode
certificate is added. The `.zip` is the guaranteed deliverable; the `.msi` is
best-effort.

See [`packaging/readme.md`](packaging/readme.md) for the full pipeline, local
builds, and the signing setup.

## Build & run

```sh
cargo run -p app          # launch the ADE (dev binary: asylumdev)
cargo test                # run the suite
cargo clippy --all-targets
```

[guise-ui](https://github.com/wess/guise) and
[libsinclair](https://github.com/wess/sinclair) are git dependencies; the first
build fetches them.
See [`docs/gpui.md`](docs/gpui.md) for the dependency recipe.

## Website

The GitHub Pages site lives in `site/` and uses Bun with Vite:

```sh
cd site
bun install
bun run dev
bun run build
```

Pushes to `main` that touch `site/` deploy through
`.github/workflows/pages.yml`. The production output is `site/dist/`.

## Layout

```
crates/
  git       worktree + branch + status + diff (pure)
  store     SQLite persistence: projects/tasks/runs, annotations, accounts,
            usage, notifications, pins/recents (rusqlite)
  config    layered settings.json + project asylum.toml + keybindings
  agent     31-agent registry, custom agents, command build, fan-out planning
  runner    pty agent execution + lifecycle supervisor (libsinclair headless)
  github    GitHub via gh: PRs, issues, create PR
  linear    Linear GraphQL: teams, projects, issues
  checks    type-check / lint / test runner with PASS/FAIL
  search    cross-worktree content search (ripgrep / git grep)
  notes     Markdown vault CRUD, properties, links/backlinks, templates, search
  preview   markdown → HTML, image / PDF / text classification
  remote    SSH remote-worktree + port-forward command builders
  update    launch update check against GitHub Releases (via curl)
  notify    desktop notifications
  designmode click an element → capture HTML/CSS/selector for an agent
  fuzzy     subsequence match + ranking (command palette, quick-open)
  companion mobile companion HTTP server + mobile web page + event stream
  control   agent control surface: spawn/read/report/wait over a local JSON API
  proxy     secrets proxy: masked outbound API calls for agents (keys never seen)
  keep      encrypted, project-scoped secret store (AES-256-GCM)
  plugin    plugin.toml manifest parsing + GitHub install/discovery
  pluginrt  process runtime (JSON over stdio) + WASM runtime (wasmi)
  cli       the `asylum` binary (worktree/run/search/control/wait/plugin/
            layout/keep/call)
  app       the gpui application (asylumdev) - 13 surfaces
```

## Docs

- **[The Asylum book](docs/book/index.md)** — fifteen chapters, start to finish:
  first task, fan-out, diffs and checks, notes, the CLI, agent orchestration,
  plugins, and a full [configuration reference](docs/book/14-configuration-reference.md).
- [`docs/gettingstarted.md`](docs/gettingstarted.md) — the first-run workflow.
- [`docs/beginners.md`](docs/beginners.md) — the plain-English version, if you are
  not a developer.
- [`docs/`](docs/) — subsystem detail ([architecture](docs/architecture.md),
  [plugins](docs/plugins.md), [secrets](docs/secrets.md),
  [roadmap](docs/roadmap.md), [parity](docs/parity.md)).
- [`CLAUDE.md`](CLAUDE.md) / [`AGENTS.md`](AGENTS.md) — architecture and
  conventions for agents working in this repo.
- [`packaging/readme.md`](packaging/readme.md) — how releases are built and signed.

The marketing/docs site is built from `site/` (see [Website](#website) above).

## License

Apache-2.0. See [`LICENSE`](LICENSE).

♥ [Sponsor this project](https://github.com/sponsors/wess)
