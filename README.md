# Asylum

**[Website](https://wess.io/asylum/) · [Tutorial](https://wess.io/asylum/tutorial/) · [Manual](https://wess.io/asylum/docs/) · [Admin guide](https://wess.io/asylum/admin/)**

Always-on AI teammates with their own computer. A native macOS app (Rust,
[gpui](https://github.com/zed-industries/zed) + [guise](https://github.com/wess/guise)).
Agents have names, jobs, memory, and a shared
computer — a workspace, a browser that stays signed in, and a terminal — that
work in your tools, run routines, hand work to each other, and ask before
anything consequential.

## Features

- **Agents** — name, label, standing instructions, pixel-sprite avatars (eight
  classes in your colors) or generated or uploaded images, per-Agent model,
  notifications, pin/hide/sections/duplicate.
- **Chat** — streaming replies with tool activity; threads, reactions, find,
  attachments (drag, paste, pick), `@` mentions and `/` skills, and optional
  dictation and live voice chat; group chats of 2–6 Agents with mention routing.
- **Cards** — approvals (Allow once / Always allow / Deny), Auto-review,
  secure secrets, takeover, forms, email/Slack drafts, tables, boards, charts,
  files, images, links, voice memos.
- **The computer** — one shared workspace (sandboxed shell on macOS), a
  Chromium the Agents drive with a screen each, live preview and takeover (or a
  real window for passkeys), a file manager, and a terminal (libsinclair, ⌘⇧T).
  Backups, update, recover, recreate, reset, idle hibernation, and Disk Saver.
- **Built-in browser** — a native web view beside the chat (⌘⇧B) for links and
  previews of workspace pages, images, and PDFs.
- **Sync** — sections, placement, and pins across Macs through iCloud Drive.
- **Skills and routines** — save skills from chat or teach one by
  demonstration; schedule routines (cron in your timezone), intervals,
  webhooks, and app events, with run history.
- **Connectors** — a Marketplace of MCP servers (remote with OAuth or token,
  or local commands), multiple labeled accounts, per-tool switches, custom
  servers. Jira and Confluence through Atlassian's hosted server or an API
  token, with seven Jira skills (triage, bug reports, sprint reports,
  standups, release notes, backlog grooming, action items).
- **Sharing** — templates by link, Team Agents with team memory, and Team Agents
  in Slack (each with its own Slack app over Socket Mode).
- **Admin policy** — an organization can disable or require connectors, lock
  approval rules, cap local execution, force Auto-review, limit template
  sharing, add Team Agents for everyone, set Network Controls (open,
  blocklist, allowlist, offline), cut local egress, run setup and check
  scripts, and retire idle computers, through a managed
  `/Library/Application Support/asylum/policy.json`.
- **Activity** — Action Recording (90 days), an audit log, Conversation
  Insights, conversation export (JSON + Markdown), OpenTelemetry spans, and
  a local Admin API with a keychain token.
- **Providers** — xAI, OpenAI/ChatGPT, Anthropic, LiteLLM, Ollama (local and cloud),
  Claude Code, Codex, or any OpenAI-compatible endpoint. Paste a key and it's
  checked, then kept in the Keychain (environment variables and Synapse work
  too). LiteLLM teams with team-scoped models, reasoning effort, a fallback
  chain, and a one-click provider check.
- 31 UI languages, light/dark, full keyboard shortcuts.

## Build and run

Requires macOS, Rust stable ≥ 1.96, and Chrome, Edge, Chromium, or Brave for
the computer's browser.

```sh
cargo run -p app                  # launch (dev build: asylumdev)
scripts/bundle.sh && scripts/dmg.sh   # dist/Asylum.app and dist/Asylum.dmg
```

For a release others can open, sign with a Developer ID and notarize:

```sh
xcrun notarytool store-credentials asylum \
  --apple-id <apple-id> --team-id <TEAMID> --password <app-specific-password>   # once
export CODESIGN_IDENTITY="Developer ID Application: <Name> (<TEAMID>)"
export NOTARY_PROFILE=asylum
scripts/bundle.sh && scripts/dmg.sh   # signed, notarized, stapled
```

On first launch, onboarding finds local models (Ollama, a LiteLLM proxy) and
installed CLIs (Claude Code, Codex), or connects LiteLLM, OpenAI, Anthropic,
xAI, or any OpenAI-compatible service with a pasted key. Settings live in
`~/.config/asylum/agents.json`; data (database, workspace, browser profile) in
`~/Library/Application Support/asylum`.

Headless modes:

```sh
asylumdev providers                        # list provider profiles
asylumdev ask <agent> "<message>"            # send a message and print the reply
asylumdev plugin add <catalog-id>          # add a Marketplace plugin
asylumdev pin <agent> <provider> [model]     # put one Agent on its own model
asylumdev check <provider> [model]         # a small real request to the provider
asylumdev teams <provider>                 # a LiteLLM key's teams and their models
```

## Providers

Profiles work like [ainz](https://github.com/wess/ainz)'s: an HTTP profile is
any chat-completions endpoint, a process profile wraps a coding-agent CLI fed
the transcript on stdin. For a LiteLLM gateway, add the **LiteLLM** preset
with your gateway's address, paste the key, load your teams, and pick from
the models your team can use (an exported `LITELLM_API_KEY` overrides the
pasted key). Any Agent can pin its own provider and model (Agent settings →
Model), so one Agent can run GPT through LiteLLM or OpenAI while another runs
Opus through Anthropic or Claude Code, side by side in the same group chat.

Voice (dictation, voice chat, voice memos) is off by default; turn it on in
Settings → General → Voice features. It and image generation use xAI's
speech and image APIs, so they need an xAI key regardless of the chat
provider. Slack is optional too: it only runs for Team Agents you connect.

Asylum is free and open source: no accounts, subscriptions, or paywalls.

## Development

The website — tutorial, manual, and admin guide — is in `site/` and
published to [wess.io/asylum](https://wess.io/asylum/) by
`.github/workflows/pages.yml`.

See [CLAUDE.md](CLAUDE.md) for the architecture and conventions and
[docs/features.md](docs/features.md) for the feature list and what's
deliberately left out.

```sh
cargo test --workspace
cargo test -p agent --test live -- --ignored --test-threads 1   # needs local Ollama
cargo clippy --workspace --all-targets
scripts/i18n.py                   # regenerate translation tables
```
