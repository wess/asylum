# Parity with Grok Bot

Coverage of the desktop checklist in [spec.md](spec.md) §4 (items 1–306).
Asylum is local-first: one person, one Mac, their own model providers. Where
the original depends on xAI's cloud (hosted VMs), Asylum provides a local
equivalent. Asylum is a free, open-source alternative, so subscription,
billing, and account items are out of scope by design. The admin console's controls are provided by a
managed policy file (`/Library/Application Support/asylum/policy.json`,
admin-writable only, the way MDM-managed settings work).

Legend: **✓** built · **≈** built as a local equivalent (how, below) · **✗** not built (why) · **—** out of scope by the owner's decision (free and open source; macOS only for now)

## 4.1 Install, auth, onboarding

| # | Item | | Notes |
|---|---|---|---|
| 1 | macOS build, .dmg | ✓ | `scripts/bundle.sh`, `scripts/dmg.sh` |
| 2 | Windows | — | macOS only for now (owner's decision) |
| 3 | Linux packages | — | macOS only for now (owner's decision) |
| 4 | Welcome screen | ✓ | Onboarding step 1 |
| 5–7 | Browser sign-in, Cursor sign-in, SSO | ≈ | No account backend. "Signing in" is connecting a model provider (xAI key, Ollama, LiteLLM, Claude Code, Codex); plugins use real browser OAuth |
| 8–9 | Paywall, access gates | — | Free and open source: no paywall or account gates |
| 10 | Intro tour | ✓ | |
| 11 | Tools questionnaire | ✓ | Shapes suggestions only |
| 12 | Background computer provisioning | ✓ | "Starting your computer" state |
| 13 | Meet a future teammate + Create your own | ✓ | Six suggested teammates ranked by tools chosen |
| 14 | Auto-created first Bot | ≈ | The gallery flow (the spec's primary flow) is used instead |
| 15–16 | Multiple accounts, sign out | ≈ | Provider profiles are switchable; keys removable in Settings |
| 17 | Update required gate | ✓ | 14-day rule against the latest GitHub release |

## 4.2 App shell and navigation — ✓ all except

- 36 "Get Grok Bot for mobile": — (macOS only for now). "Update available/Install": ≈ checks releases; install is a download.
- 25 Palette finds messages, files, routines ✓; links are found through message text search (≈).

## 4.3 Sidebar — ✓ all except

- 46 Sections sync across devices: ≈ through a shared folder (iCloud Drive by
  default; Settings → General). Sections, Bot placement, and pins converge
  both ways; Bots are matched by name (`e2e.rs::sections_sync_between_devices`).

## 4.4 Bots and profiles — ✓ all except

- 71 Owner-only edits: ≈ single-user app; Team Bots show their owner.

## 4.5 Templates / sharing

| # | | Notes |
|---|---|---|
| 78–80, 83, 85 | ✓ | Links are `asylum://template/…`; public or team-only |
| 81 | ✓ | Policy `templates: "team"` or `"off"` limits sharing |
| 82, 84 | ≈ | In-app preview (contents, warnings) with third-party terms acceptance instead of a web page |

## 4.6 Memory — ✓ all except

- 91 Per-person notes: ≈ supported in the data model (scope "person"); one user locally.
- 93 Selective memory transfer: ✓ a per-memory picker (facts and summaries on, preferences off by default) and an option to move routines.

## 4.7 Chat and composer — ✓ all except

- 110 Link hover preview: ✓ hovering a link card fetches its title, description, and site (Open Graph), through Network Controls.

## 4.8 Voice — ✓ all (optional, off by default)

Voice is turned on in Settings → General → Voice features; until then
dictation, voice chat, voice memos, and their settings are hidden and the
`voice_memo` tool isn't offered to Bots. When on:
Dictation (xAI speech-to-text; ⌘D tap to toggle or hold to talk), live voice
chat (xAI realtime: server VAD, barge-in, transcripts, mute, one call at a
time, return-to-call, post-call card, follow-up work, feedback), voice memos
(xAI TTS, falling back to the system voice), microphone picker, and Voice /
Speed / Language changed mid-call. The realtime protocol (session setup,
audio up and down, transcripts, barge-in, tool calls) and the speech
endpoints are exercised end to end against fake xAI servers in
`voice/tests/`; they have not been run against the live xAI API (no key on
the build machine).

## 4.9 Group chats and handoffs — ✓ all except

- 151 Teammate's Team Bot in a group: ≈ a joined Team Bot is a local Bot and can be added.
- 157 Proactive work: ≈ via routines and memory; no autonomous scanning.
- 158 Cloud Agents/subagents: ≈ hand work to a Bot pinned to the Claude Code or Codex provider.
- Handoffs are rate-limited (3 per Bot pair per 10 minutes without user input) to stop thank-you loops.

## 4.10 The computer

| # | | Notes |
|---|---|---|
| 159 | ≈ | A persistent local computer: shared workspace, `sandbox-exec` confinement of Bot shell writes, shared Chromium profile — not a cloud microVM |
| 160–170 | ✓ | Per-Bot screens, preview, takeover (in panel or full-window), I'm done, Action needed card, file manager, terminal, time-of-day wallpaper |
| 171 | ≈ | Work continues with the window closed; not with the app quit |
| 172 | ✓ | "Open in a window" relaunches the browser visibly for passkeys and security keys |
| 173 | ≈ | Traffic already leaves from this Mac |
| 174–181, 184–186 | ✓ | States, recover, update (software/computer), reset from backup, guards, disk warnings, Disk Saver, local transfer |
| 176–177 | ≈ | Guard messages rather than separate Keep waiting / Retry dialogs |
| 178 | ✓ | Update now, or "Update tonight" (2:00 local, once no Bot is working) |
| 182 | ✓ | Recreate computer: Bots pause at a safe point (between jobs), the computer is rebuilt on the same disk, work resumes (`e2e.rs::recreate_pauses_at_safe_point`, live Chrome) |
| 183 | ✓ | Idle hibernation (browser stopped after 30 idle minutes; wakes on next use or Wake) |

## 4.11 Connectors — ✓ all except

- 191 "Disabled by your admin" and 200 required plugins: ✓ via policy
  (`disabled-plugins`, `required-plugins`); disabled plugins can't be added or
  offered to Bots, required ones are added for everyone and can't be removed.
- 201 Built-ins: GitHub, Linear, Notion, Jira/Confluence, Sentry, Stripe, Supabase, Vercel, Hugging Face, DeepWiki, Context7, Google Workspace, Slack, PagerDuty, Datadog, Salesforce, Brave, Postgres, Playwright, Figma. Google/Slack/Datadog/Salesforce/PagerDuty run as local MCP servers with your own credentials (no shared OAuth app).

## 4.12 Skills — ✓ all except

- 212 Team skills: ≈ skills saved by a Team Bot are marked team skills.

## 4.13 Routines — ✓ all except

- 217–219 Slack, GitHub, Linear, Sentry, PagerDuty, email triggers: ≈ each posts to the routine's webhook (`127.0.0.1`) and is matched by event/channel/text filters; exposing it to the internet (a tunnel) is up to you.
- 229 Runs while the app is running (window may be closed).
- 232 Trigger and channel are set by asking the Bot, not a dropdown.

## 4.14 Approvals and Auto-review — ✓ all except

- 240 Auto-review "Required by your admin" and 242 locked team rules: ✓ via
  policy (`auto-review`, `rules`); locked rows can't be edited or deleted.
- 245 Team Bot personal-connector card: ≈ approval kind exists; single user means it rarely triggers.
- 247 Spend requests: — (payments are out of scope for a free app).

## 4.15 Secrets — ✓ all

## 4.16 Local execution — ✓ all except

- 261 Admin ceiling: ✓ policy `local-exec` (stricter wins; "Always allow"
  is disabled on the card).
- 259 The first-use choices appear on the approval card (Always allow / Allow once / Never / Deny once).

## 4.17 Notifications — ✓ all

## 4.18 Team Bots

| # | | Notes |
|---|---|---|
| 268–272, 275, 277–278, 280 | ✓ | Create, edit (140-char description), visibility gate, setup rows, team files, Publish to Team (copy or start fresh), publish/unpublish, join by link |
| 273–274, 276, 279, 281–286, 294 | ≈ | Teammates each add the Team Bot from its link and run it on their own Mac; there is no shared server, so identity routing, usage attribution, and cross-person privacy are local |
| 288–291 | ✓ | Optional — nothing runs until a Team Bot is connected. Bring to your team's Slack: each Team Bot gets its own Slack app from a generated manifest, connected over Socket Mode (nothing exposed to the internet). It answers every DM, answers @mentions in channels and group DMs and then follows the thread, ignores other bots, and asks unlinked teammates (Settings → Team Setup → Linked Slack accounts) to link, privately. Remove from Slack disconnects it (`slack/tests/`, `e2e.rs::slack_dm_round_trip`) |
| 287 | ✓ | Team Bots Not Available (policy `team-bots-enabled: false`), Bot Not Found (bad or unpublished link), usage limit screens; privacy-mode mismatch has no local analogue |
| 289 | ✓ | Awaiting approval / Check approval / Cancel request (Slack sends the request itself on install) |
| 292 | ✓ | Remove from Slack |
| 293 | ✓ | Policy `team-bots`: added for everyone, can't be hidden or deleted, marked "Required by your admin" |

## 4.19 Settings and billing

| # | | Notes |
|---|---|---|
| 295–299, 303, 305 | ✓ | Settings sections, timezone, Usage (weekly limit, Keep going, monthly allowance — caps on your own provider spend, nothing is charged), updates check, automatic updates |
| 300–301 | — | SuperGrok linking and trials: not applicable to a free app |
| 302 | ✓ | Usage limit reached screen (and in-chat notice) |
| 304 | ≈ | Team Setup page covers Team Bots; no enterprise manifests |
| 306 | ≈ | A model picker exists on purpose: providers and models are chosen per install and per Bot |

## 4.A Admin (items 324–346)

Admin controls come from the managed policy file; the data views live in
Settings → Activity.

| # | | Notes |
|---|---|---|
| 324–325 | — | Group access and invitations need a shared account server; teammates join Team Bots by link instead |
| 326 | ✓ | `cloud-agents: false` turns off CLI coding-agent providers (Claude Code, Codex) |
| 327–329, 331–332 | ✓ | Template sharing, connector policy, local-execution ceiling, locked rules, forced Auto-review (above) |
| 330 | ✓ | `local-egress: false` runs commands on this Mac with outbound network denied (sandboxed) |
| 333 | ✓ | Network Controls, `network.mode`: `open`, `default` (blocklist), `allowlist`, `offline`. Enforced by a filtering egress proxy the browser launches through and computer commands must use (the sandbox denies any other outbound connection); web tools go through it too (`policy.rs::network_controls_gate_commands_and_web_tools`) |
| 334 | ✓ | `setup-script` runs once per script version on the computer; `check-script` on every start; failures notify and are audited |
| 335 | ✓ | Team Bot secrets: 100 per Bot, 32 KB each, 96 KB total; reserved names (PATH, HOME, proxies, `ASYLUM_*`) |
| 336 | ✓ | Recreate, Stop, and Delete computer and data (Settings → Computer) |
| 337 | ✓ | Software update restarts the computer to apply changed settings; Network Controls changes relaunch the browser automatically |
| 338 | ✓ | `terminate-inactive-days` deletes an unused computer's data |
| 339 | ✓ | Action Recording: tool, target, outcome, duration — no arguments or results — kept 90 days |
| 340 | ✓ | Conversation export (prompts, responses, tool I/O) as JSON + Markdown, per conversation (sidebar menu) or all |
| 341 | ✓ | Audit log: settings, rules, plugins, policy, computer operations, exports |
| 342 | ✓ | OpenTelemetry: turn and tool spans over OTLP/HTTP (ids and names only), endpoint in Settings or forced by policy |
| 343 | — | Per-group overrides: there is one group (this Mac) |
| 344 | ✓ | Default Team Bot assignment via `team-bots` |
| 345 | ✓ | Admin API on `127.0.0.1` with a keychain token: Bots, actions, audit, insights, export, send message (`admin.rs` tests) |
| 346 | ✓ | Conversation Insights: conversations, messages, actions, outcomes, top tools and Bots |

## Verification note

Voice and Slack are verified end to end against faithful fakes of the xAI and
Slack APIs; live runs against the real services were judged unnecessary for
now (voice is off by default, Slack is optional).

## Beyond the spec

- Multiple model providers (xAI, OpenAI, Anthropic, LiteLLM, Ollama local and
  cloud, Claude Code, Codex, custom endpoints), with each Bot able to pin its
  own provider and model — e.g. one Bot on GPT, another on Opus, in the same
  group chat (verified in `agent/tests/e2e.rs::bots_use_their_own_providers`).
- Headless CLI modes and an embedded terminal on the computer.
