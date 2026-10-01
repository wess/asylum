# Features

What Asylum does, how each part is built, and what's deliberately left out.
Asylum is local-first: one person, one Mac, their own model providers.
Everything an organization would manage centrally comes from a managed policy
file (`/Library/Application Support/asylum/policy.json`).

Legend: **✓** built · **≈** built as a local design (how, below) · **—** not included (why)

## Install and onboarding

| Feature | | Notes |
|---|---|---|
| macOS app and `.dmg` | ✓ | `scripts/bundle.sh`, `scripts/dmg.sh`; signed and notarized with a Developer ID |
| Windows, Linux, mobile | — | macOS only for now |
| Welcome, intro tour, tools questionnaire | ✓ | The questionnaire only shapes suggestions |
| Connect a model | ✓ | Finds Ollama, a LiteLLM proxy, and the Claude Code and Codex CLIs, or takes a key |
| Meet a teammate gallery | ✓ | Six starter Agents ranked by the tools you chose, or create your own |
| Background computer setup | ✓ | "Starting your computer" state |
| Accounts, sign-in, paywall | — | Free and open source: no account, subscription, or paywall |
| Update check | ✓ | Against GitHub releases; a build more than 14 days behind a newer release asks to update |

## Shell, sidebar, and navigation

| Feature | | Notes |
|---|---|---|
| Sidebar with pins, sections, hidden Agents, unread and attention states | ✓ | |
| Sections across Macs | ≈ | Sync through an iCloud Drive folder; Agents matched by name |
| Command palette | ✓ | Agents, messages, files, routines, commands |
| Full keyboard shortcuts, zoom, full screen | ✓ | |
| Built-in browser | ✓ | Native web view in the right pane for links and workspace previews (⌘⇧B) |
| Built-in terminal | ✓ | libsinclair terminal in the computer panel (⌘⇧T) |
| 31 interface languages, light and dark | ✓ | |

## Agents

| Feature | | Notes |
|---|---|---|
| Name, label, description, avatar | ✓ | A pixel sprite (eight classes, your colors), a generated image, or an upload |
| Per-Agent provider and model | ✓ | One Agent on GPT, another on Opus, in the same group chat |
| Pin, hide, sections, duplicate, rename, delete | ✓ | |
| Templates by link, public or team-only | ✓ | `asylum://template/…`, previewed with warnings and terms before adding |
| Memory: facts, preferences, summaries | ✓ | Per Agent; team memory for Team Agents |
| Choose memories when publishing to the team | ✓ | Facts and summaries on, preferences off by default |
| Owner-only edits | ≈ | One user per Mac; Team Agents show their owner |

## Chat

| Feature | | Notes |
|---|---|---|
| Streaming replies with inline tool calls | ✓ | |
| Attachments, threads, reactions, find, drafts | ✓ | Up to 6 files, 25 MB each (video 200 MB) |
| `@` mentions and `/` skills | ✓ | |
| Cards: approvals, takeover, secrets, forms, drafts, tables, boards, charts, files, images, links | ✓ | |
| Link previews | ✓ | Title, description, and site on hover, through Network Controls |
| Group chats of 2–6 Agents with mention routing | ✓ | |
| Handoffs between Agents | ✓ | Limited to 3 per pair per 10 minutes without you, to stop loops |
| Coding sub-agents | ≈ | Hand work to an Agent pinned to the Claude Code or Codex provider |
| Voice: dictation, voice chat, voice memos | ✓ | Optional, off by default; uses xAI speech APIs; tested against a fake server |

## The computer

| Feature | | Notes |
|---|---|---|
| Shared workspace, sandboxed shell, terminal, file manager | ✓ | `sandbox-exec` confines writes to the workspace |
| Browser with a screen per Agent, shared sign-ins | ✓ | Chromium over CDP |
| Live preview and takeover | ✓ | In the panel or full window |
| Passkeys and security keys | ✓ | "Open in a window" relaunches the browser visibly |
| Backups, update (now or tonight), recover, recreate, reset, stop, delete | ✓ | Recreate pauses Agents between jobs and resumes them |
| Idle hibernation | ✓ | After 30 idle minutes; wakes on next use |
| Disk warnings and Disk Saver | ✓ | |
| Runs in the cloud | ≈ | The computer is on your Mac; work continues with the window closed, not with the app quit |

## Connectors, skills, routines

| Feature | | Notes |
|---|---|---|
| Marketplace of MCP connectors | ✓ | 21 built in, OAuth or token or local command, multiple accounts, per-tool switches, custom servers |
| Jira and Confluence | ✓ | Atlassian's hosted server (sign in) or mcp-atlassian with an API token; seven Jira skills |
| Skills: written, taught by demonstration, learned, packaged | ✓ | |
| Routines: cron schedules in your timezone, intervals, webhooks | ✓ | Up to 50 per Agent, last 20 runs kept |
| App-event triggers (Slack, GitHub, Linear, Sentry, PagerDuty, email) | ≈ | Posted to the routine's local webhook and filtered; exposing it is up to you |

## Safety

| Feature | | Notes |
|---|---|---|
| Approvals: Allow once, Always allow, Deny | ✓ | Background approvals expire after 10 minutes |
| Auto-review on a fast model with plain-language rules | ✓ | |
| Local execution: Ask, Always, Never | ✓ | |
| Secrets in the Keychain, injected and redacted | ✓ | |
| Spend requests | — | No payments |

## Team Agents

| Feature | | Notes |
|---|---|---|
| Create, set up, publish, unpublish, join by link | ✓ | |
| Shared server, identity routing, central usage | ≈ | Each teammate runs the Team Agent on their own Mac |
| Slack app per Team Agent | ✓ | Optional; Socket Mode; DMs and mentions; linked accounts; admin-approval states |
| Required Team Agents | ✓ | Policy `team-agents`; can't be hidden or deleted |

## Usage

| Feature | | Notes |
|---|---|---|
| Weekly limit, Keep going, monthly allowance | ✓ | Caps on your own provider spend; nothing is charged |

## Admin

| Feature | | Notes |
|---|---|---|
| Managed policy file | ✓ | Connectors, rules, Auto-review, local execution, templates, Team Agents, Cloud Agents |
| Network Controls: open, blocklist, allowlist, offline | ✓ | Enforced by a filtering egress proxy; the sandbox blocks any other route |
| Local egress off | ✓ | Commands on the Mac run with outbound network denied |
| Setup and check scripts | ✓ | |
| Terminate inactive computers | ✓ | |
| Action Recording, audit log, export, Insights | ✓ | Settings → Activity |
| OpenTelemetry | ✓ | Ids and names only |
| Admin API | ✓ | `127.0.0.1`, keychain token |
| SCIM, invitations, group policies | — | Need a shared server; there isn't one |

## Providers

| Feature | | Notes |
|---|---|---|
| xAI, OpenAI, Anthropic, LiteLLM, Ollama (local and cloud), Claude Code, Codex, custom | ✓ | Keys in the environment, Synapse, or the Keychain (an exported variable wins) |
| LiteLLM key check, teams, team-scoped models, key-reach warning | ✓ | |
| Fallback chain | ✓ | Moves on only before the first word of a reply |
| Reasoning effort | ✓ | Per provider |
| Responses API | — | Chat Completions only for now |
| Provider check | ✓ | A small real request, from Settings or `asylumdev check` |
