# Grok Bot Desktop Clone: Functional Specification

Status: research-derived spec, as of 2026-09-30.
Target: a feature-complete desktop clone of xAI/SpaceXAI "Grok Bot" (beta launched 2026-08-11; Team Bots 2026-09-28).

## Provenance legend

Every behavior is tagged by source:

- **[D]** Documented in official docs (docs.x.ai/grok-bot/*, cursor.com/docs/grok-bot/*, cursor.com/help/grok-bot/*) or xAI news posts. UI labels in **bold** are exact strings from those docs.
- **[T]** Reported by third-party reviews (flaviocopes.com, venturebeat, eesel.ai, note.com hands-on, digitalapplied, layer3labs), not confirmed by current official docs. Some may reflect earlier builds.
- **[I]** Inferred: needed to make a documented feature work, or a reasonable reading of screenshots/captions, but not stated anywhere.

Sources read in full: all 21 pages under docs.x.ai/grok-bot (overview, get-started, use-cases, mobile, bots, team-bots, chat-and-collaboration, files-and-results, computer-and-apps, skills-routines-and-automations, settings-and-notifications, approvals-security-and-privacy, teams-and-enterprises, identity-and-access, private-networks, proxies, computers, security, security-faq, troubleshooting, faq); cursor.com/docs/grok-bot variants (overview, get-started, work, settings, deployment); 17 cursor.com/help/grok-bot articles (onboarding, how-to, faqs, sign-in, mobile, plans, supergrok, mobile-purchase, delete-account, connect-plugins, routines, edit-bot, group-chats, team-bots, voice-chat, computer-recovery, secrets, get-help); x.ai/news: introducing-grok-bot, grok-bot-more-plans, team-bots, designing-grok-bot; grok-bots.com (unofficial fan site); reviews: venturebeat, flaviocopes, eesel.ai, note.com (masa_wunder), digitalapplied, layer3labs (x2), explainx.

Note on brand: the real product authenticates through Cursor accounts, and billing runs through Cursor plans or SuperGrok links. A clone should abstract this as "Account provider" and "Plan/usage provider". Wherever this spec says "Cursor account", read it as "the identity backend".

---

# 1. Product summary

Grok Bot is a desktop app (macOS, Windows, Linux) with mobile companions (iOS/iPadOS, Android) for creating and messaging **Bots**: persistent AI teammates, each with a name, a job (label), a description (standing instructions), an avatar, its own conversation, and memory that builds up over time. [D]

Core model: [D]
- **One cloud computer per user.** All of a user's Bots share one persistent cloud Linux VM (a Firecracker microVM) with a browser, filesystem (`/workspace`), terminal, and desktop. Files, browser cookies/sessions, and CLI credentials are shared across every Bot. Each Bot gets **its own screen** on that computer, and a Bot can run only one computer-use task on its screen at a time. Work keeps going when the laptop or app is closed.
- **Thin clients.** Desktop and mobile apps handle chat, review, and approvals. The work itself runs in the cloud.
- **Messaging UX.** It feels like iMessage/Slack with teammates. You can type, dictate, voice-chat live, attach files, `@`-mention Bots/groups/routines/connectors, and `/`-reference skills. Threads, reactions, and drafts to approve are all supported.
- **Bots coordinate.** Bots can message each other asynchronously (handoffs), share context in group chats of 2 to 6 Bots, create helper Bots, and follow a coordinator ("Chief of Staff") pattern.
- **Skills and routines.** A skill is reusable how-to instructions, either saved from a finished task or recorded through **Teach a task** (a demo of up to 10 minutes). A routine assigns a workflow to one Bot on a schedule or event trigger (Slack, GitHub, Linear, Sentry, PagerDuty, email, webhook).
- **Connectors (plugins)** are installed from a **Marketplace** (MCP-based, with OAuth tokens held server-side). For anything without a connector, the Bot uses the browser (computer use).
- **Human-in-the-loop safety.** Approval cards (**Allow once** / **Always allow** / **Deny**), a model-based **Auto-review** with user and team rules, secure secret cards, takeover of the computer for passwords/2FA/CAPTCHA, and a separate permission for running commands on the local machine.
- **Sharing.** Bot templates (public or team-only links), plus **Team Bots**: one owner-maintained Bot that the whole team chats with privately. It has team memory, shared plugins/secrets/skills/files, and an optional Slack app.
- **Plans.** Included with paid tiers. Usage is metered weekly, with optional on-demand overage and a monthly cap. There is no model picker.

---

# 2. UI layout

## 2.1 Window shell

- Single main window: **left sidebar**, **main conversation pane**, an optional **right pane** (conversation details/info pane, or the pinned computer preview), and a **title bar**. [D/I]
- In the title bar, a computer status icon **turns purple while the computer is active**. [T: design article via search summary; D: designing-grok-bot names the "Status" level]
- Zoom in/out/reset and fullscreen (see shortcuts). [D]
- The window can be closed while cloud work continues. On Mac, the app keeps running until **Quit** from the menu bar. [D]
- A dock/taskbar badge shows unread activity. [D]
- Desktop OS notifications for Bot finished/needs input, suppressed while the app is focused. [D]

## 2.2 Left sidebar

Top to bottom: [D unless marked]

1. **New** button (also labeled **New chat** in places). Shortcut `Cmd/Ctrl+N`. Opens the **New chat** picker (2.4).
2. Search / jump entry point (command palette, `Cmd/Ctrl+K`). [D]
3. **Marketplace** (also referred to as **Plugins** in help articles). Shortcut `Cmd/Ctrl+Shift+M` or `Cmd/Ctrl+Shift+W`. [D]
4. A special card after Team Bot setup: **You built a Bot. Now your whole team gets a teammate.** with **Publish to team**. [D]
5. **Pinned** Bots/groups at the top of the list. [D]
6. **Sections**: user-created groups (by project, client, business). Unsectioned items fall under **Unassigned** when a section is deleted. Sections sync across desktop and iOS. [D]
7. Bot and group-chat rows. Each row shows: [D/I]
   - Avatar (animated by state: idle, working, waiting/blocked, done) [D design article]
   - Name, plus label/title [T: eesel "names with titles"]
   - Team Bots show the owner's name next to the Bot name [D]
   - Attention state: **Needs attention** (question, approval, handoff), **Unread activity** (new result), working/typing status [D]
   - **Voice chat in progress** indicator on the row or sidebar during a live call [D]
   - Primary Bot star badge [T: flaviocopes, "Primary Bot" with small star; not in current docs]
8. **Hidden Bots** at the bottom. When every Bot is hidden, it reads **Show Hidden Bots** instead. [D]
9. Account menu (bottom or top corner) [I placement]: see 2.9.

Sidebar behaviors: [D]
- **Compact sidebar** (collapse to icons): `Cmd/Ctrl+B`. Rename by double-click is unavailable while collapsed.
- `Cmd/Ctrl+1..9` jumps to sidebar Bot 1 to 9. `Alt+↑/↓` goes to previous/next Bot. `Control+Tab` / `Control+Shift+Tab` cycles Bots.
- Double-click a Bot name to rename inline, then Enter to commit.
- Right-click context menu on a **Bot** row (exact order from screenshot caption): **Pin**, **Move to new section** (or **Move to** → section list / **Create section** once sections exist), **Mark as Unread** (toggles with Mark as Read), **Rename Bot** (owner only), **Copy conversation ID**, **Hide from sidebar** (absent for admin-required Team Bots), **Delete**. [D]
  - Published Team Bot rows add **Copy link**. [D]
- Right-click on a **group chat** row: **Rename chat**, plus pin/hide/section/mark-read [D for Rename chat; I for the rest].
- Right-click on a **section header**: **Rename** [D], Delete section [I; deletion is documented].
- **Hidden Bots** view: right-click → **Show in sidebar**. [D]
- Helper Bots (created by other Bots) and **Disk Saver** (a system Bot) show up as normal sidebar rows. [D]

## 2.3 Main conversation pane

**Header** (Bot name at the top of the chat): [D]
- Clicking the Bot/group name opens the **details** view (the "info pane"). It holds **Bot settings**, **Edit details** (Team Bot), **Tasks** (routines), **Setup** (Team Bot), **Share** menu, **Unpublish**, Slack controls, **Secrets**.
- **Agent Computer** / **Open computer** control opens the computer panel. [D]
- **Teach a task** button (top-right in 1:1 chats with the computer view open). While recording, a **Stop** button appears. [D + T note.com]
- **Voice chat** control stays under the header while a call is live. **Return to the voice chat** appears when you navigate away. [D]
- Bot menu (overflow). Items: Edit Profile → now **Bot settings**, mark read/unread, **Share menu** (**Create template**, **Publish to Team**), Duplicate [D iPhone; I desktop], Delete. [D mostly]

**Transcript**: a mixed timeline of: [D]
- User and Bot messages (markdown text, links rendered as cards with hover preview; links open in the system browser)
- Tool activity and computer-use events ("system events")
- Created-file cards, image cards, link cards, tool-result cards (click to preview; save; open source)
- Questions from the Bot
- **Approval cards** (4.12)
- **Secret request cards** (4.13)
- **Connect** cards for plugin auth, with **Reopen** / **Retry** states (4.10)
- **Computer** card marked **Action needed** with **Take over** / **Skip** (4.9)
- In-chat **forms**: one per step, used to fill page fields such as a login, checkout address, or phone number [D]
- **Draft cards**: **New Email** / **New Slack Message**, editable recipients and body, **Send email** / **Send message** / **Discard** [D]
- **Voice memo** cards: **Play voice memo** / **Pause voice memo**, expandable transcript [D]
- **Voice chat** card after a call ends (duration, full transcript) [D]
- Handoff messages between Bots [D]
- Routine run results and test-run results [D]
- Team Bot readiness card with **Publish to team** and copy-link [D]
- Structured UI replies (cards, forms, boards, visualizations) [D design article]
- Spend request cards: merchant, total, description, expandable breakdown (Stripe Link) [T: flaviocopes]
- Threads: reply to a specific message, with a thread view [D]
- Reactions on messages [D]
- Per-message hover actions: **More message actions** menu (**Copy request ID**, reply in thread, react) [D]; right-click on a message → **Copy request ID** [D]
- **Find in this chat**: `Cmd/Ctrl+F` [D]

**Error notices area** ("**Notifications**" above the composer): a list of in-app errors. Each can be dismissed, or **clear** the whole list. Some carry **Copy request ID**. [D]

**Composer**: [D]
- Multiline text input. `Enter` sends, `Shift+Enter` adds a newline, `Cmd/Ctrl+Enter` sends from anywhere. Focus with `Cmd/Ctrl+I` or `Cmd/Ctrl+L`.
- Attachment control, drag-and-drop, paste images/links. Up to **6 attachments** at once; docs/images/audio up to **25 MB**, video up to **200 MB**.
- **Start voice input** (dictation): `Cmd/Ctrl+D` while focused, or hold the keys to talk. Speech is transcribed into the composer for editing.
- **Start voice chat** (waveform button), shown **only when the composer is empty**. No keybinding.
- `/` opens the skill picker. `@` opens the mention picker (Bots, groups, routines, connectors/plugins, `@everyone` in groups).
- Per-conversation draft persistence when you navigate away. [D mobile; I desktop]
- Sending while the Bot is working is allowed; a user message takes priority and can redirect the current turn.

## 2.4 New chat picker (`Cmd/Ctrl+N`)

[D]
- Search/type field. Typing a name offers **Create "name" Bot**.
- **Create new Bot** creates a Bot named **New Bot** (or the typed name) and opens it.
- **Create new Team Bot** (only when Team Bots are enabled) creates **New team bot** and opens its setup chat.
- Multi-select 2 to 6 existing Bots to start a group chat (a name is generated).
- **Team Bots** section at the bottom: browse and search published Team Bots (each shows its creator). **Search Team Bots** requires at least 3 characters for Bots you haven't added. **Add** → **Start a chat**.
- Typing at least 3 characters in the regular New search also finds Team Bots.

## 2.5 Details / info pane (per Bot)

Opened by clicking the Bot name, **View conversation details**, or the shortcut `Cmd+Shift+I` / `Cmd+Alt+B` (mac) or `Ctrl+Alt+B` (Win/Linux). [D]

Sections: [D unless marked]
- Avatar (click to open the avatar picker, 4.1)
- **Bot settings** (`Cmd/Ctrl+Shift+,` toggles): **Name**, **Label (optional)**, **Description**, avatar, **Notifications** switch ("**Get notified when this Bot finishes or needs input**")
- **Tasks** tab → **Routines** list (4.8). Empty state: **Ask in chat to set a routine**
- **Secrets** section: list of name + description, **Add secret**, **Replace**, **Remove** (4.13)
- **Share** menu: **Create template** / **Copy link** / **View template details** / **Update template** / **Public link** vs **Team-only**; **Publish to Team**
- Team Bot only: **Setup** section with rows **Plugins**, **Secrets**, **Skills**, **Files**, each with **Add**; **Edit details**; **Publish to team** / **Unpublish**; **Bring to your team's Slack** (**Connect** / **Connect after publishing** / **Awaiting admin approval** / **Connected**); **Remove from Slack**
- Group chat details: **Group name**, **Description** (read by all member Bots), member management [D for name/desc; D mobile "manage group members"]

## 2.6 Computer panel

Three levels: [D design article + docs]
1. **Status**: title-bar icon shows the computer is active (purple). [T for color]
2. **Preview**: a pinned side panel in the conversation. It shows live clicks, typing, navigation, and current status. You can leave it while work continues.
3. **Takeover**: full screen. The user takes control of mouse and keyboard, then hands control back.

Content: the Bot's screen on the shared computer, a Linux desktop with a **Finder-like file manager, terminal, and browser** laid out together [T: note.com]. The wallpaper shifts over the day, lighter in the morning and darker at night [D design article].

Controls: **Take over** / takeover control, **I'm done** (return control), **Skip**, **Teach a task** / **Stop** (recording). [D]

Error and lifecycle states shown in or around the panel: **Starting your computer**, **Updating your computer**, **Updating Grok Bot's Computer**, **Reconnecting**, **Couldn't Reach Grok Bot's Computer**, **Retry**, **Recover computer** → confirm dialog **Recover Grok Bot's Computer**, **Continue in Background**, **Keep waiting**, **Update still running**, **Retry Recovery**, **Retry Reset**, **Backup not ready**, **Agent busy**, **Bot failed to respond**, **Computer is low on disk space** / **Computer is critically low on disk space** → **Go to Disk Saver**. [D]

## 2.7 Marketplace

[D]
- Browse and search plugins (connectors) and packaged skills. Each has **Add**.
- Plugin detail page: status (**Added**, **Needs auth**, **Disconnected**, **Connected**, **Disabled by team admin**, **Waiting for authorization**), **Authorize** / **Authenticate**, **Reopen**, **Retry**, **Remove**, **Accounts** list with **Add Another Account** (label such as work/personal), and per-tool enable/disable toggles.
- **Your plugins** (also shown as **Yours**) → **Manage plugins and skills** → tabs/lists **Installed** and **Private skills**. Private skills can be enabled per Bot. [D cdocs: "enable it for the current Bot under Settings > Plugins > Yours"]
- Team marketplace entries: team-provided plugins may be required or restricted. [D]

## 2.8 Settings dialog ("Grok Bot settings", `Cmd/Ctrl+,`)

Sections are rollout- and plan-dependent. [D]

**General**
- **Account**: signed-in account, **Sign In with Cursor**, sign out, saved accounts list (inactive rows have **Remove**), **Add account**.
- **Appearance**: Theme (**Follow System** / **Light** / **Dark**); **Language** (**Follow System** + 31 languages: English, Afrikaans, Arabic, Bengali, Chinese (Simplified), Chinese (Traditional), Czech, Danish, Dutch, Finnish, French, German, Greek, Hebrew, Hindi, Hungarian, Indonesian, Italian, Japanese, Korean, Norwegian Bokmål, Polish, Portuguese, Russian, Spanish, Swedish, Thai, Turkish, Ukrainian, Urdu, Vietnamese).
- **Bot**: **Timezone** (**Auto-detect** or a picked zone; used by routine schedules); **Execution on Local Computer** (**Ask every time** default / **Always allow** / **Never allow**); **Auto-review** switch ("**Grok Bot checks each action before it runs and asks you first when needed.**"; can show **Required by your admin**); **Auto-review Rules** table.
- **Auto-review** (rules table): personal **Ask first** and **Allow automatically** rules, plus locked team rows ("`Required by your admin. You can't edit or delete this rule.`").
- **Security Key**: **Use hardware security keys** (default on for macOS/Windows; not supported on Linux; each use asks for approval).
- **Team Bots**: **Link** Slack account; **Clear** next to **Clear connector preferences**.
- **Microphone**: input device picker. [D voice-chat help]

**Computer**
- **Computers**: registered local computers, each with **Execution on this computer**.
- Network: **Route egress through this desktop** / "**Route traffic through this computer**" toggle (per desktop). Locked state: "`Your team's admin has turned off local egress.`" / "`Disabled by your admin. You can't turn this on.`"

**Usage & Billing**
- **Weekly usage** meter (the title can name a linked SuperGrok tier), **On-demand usage** ("**Billed through Cursor**"), **On-demand monthly limit** with **Enable**, **Link SuperGrok Heavy** (tier-named), **Cancel Trial** (free plan).

**Team Setup** (Enterprise): review or reinstall admin-managed setup manifests.

**Updates**
- **Grok Bot Updates**: installed version, **Check for Updates**, **Restart to Update**, **Automatic Updates**.
- **Grok Bot's Computer**: **Update** (**Update Grok Bot's Computer**; the text says whether it's a software update or a full computer update; can be scheduled later), **Reset** (**Reset Grok Bot's Computer**).

There is **no model picker**. [D]

## 2.9 Account menu

[D]
- **Settings**
- **About** → dialog with **Version**, **Copy version info** (version, release track, OS)
- **Get Grok Bot for mobile** (opens the App Store listing)
- **Weekly usage** at-a-glance meter
- **New update available** → **Install** → **Restart to update**
- **Switch account**, **Add account**
- Sign out

## 2.10 Command / search palette

`Cmd/Ctrl+K` opens it. `Cmd/Ctrl+Shift+F` opens it scoped to Bots. [D]
- Switch between Bots and groups
- Find prior messages (cross-conversation), files, links, routines
- Open settings and common actions
- Jump back to the matching place in a conversation
- Result scopes (mobile documents these): **All**, **Messages**, **Bots**, **Group Chats**, **Files**, **Routines** [D]
- Visit history: back/forward with `Cmd/Ctrl+[` / `Cmd/Ctrl+]` [D]

## 2.11 Onboarding / first run

[D]
1. Welcome screen with **Sign in** (also **Get started**). On the **Get Started** paywall (before sign-in): **Link Grok Account**, **Link X Account**; mobile adds **Finished Linking? Refresh My Status**.
2. The browser opens for auth. The app regains focus (or the user returns manually).
3. Blocking states: Legacy Privacy Mode error, no access (request access from the app), phone-verification block, **Update required**.
4. Intro tour: Bots, the shared computer, routines.
5. Question: "which tools do you use?" (multi-select). This shapes teammate suggestions only and doesn't connect anything.
6. Computer setup runs in the background (**Starting your computer**).
7. Final step: **Meet a future teammate**, a gallery of suggested teammates, plus **Create your own** (name, one primary job, description).
   - Alternate documented flow: the first Bot is auto-created, named **Grok Bot**, and its chat opens. [D help/onboarding]
8. Mobile asks for notification permission during first run.

## 2.12 Global dialogs and screens

- **Update required** (version more than 14 days old while a newer one exists): **Update**, **Restart to update**, **Try again**, **Download the latest version**; on Linux, **Copy command** with an apt/dnf command. [D]
- **End the current voice chat?** → **End and Chat**. [D]
- **How was the call?** → **Good** / **Bad**. [D]
- **Voice chat failed** with "**The call ended unexpectedly.**" / "**The realtime connection failed.**" → **Dismiss**; mic-denied and no-mic errors. [D]
- Local-exec first-use modal (4.14). [D]
- Team Bot copy wizard: **How should your Team Bot start?** (4.17). [D]
- Delete confirmations (Bot, routine, Team Bot). [D]
- **Team Bots Not Available**, **Bot Not Found** screens. [D]

---

# 3. Feature areas

## 3.1 Bots and profiles

Data model (Bot): [D unless marked]
- `id`, `name`, `label` (optional; mobile calls it **Title (optional)**), `description` (mobile: **Instructions**; the Bot's standing job and rules), `avatar`, `notifications: bool`, `pinned`, `hidden`, `sectionId`, `ownerId`, `kind: personal | team | helper | system`, memory store, enabled skills, routines, secrets, conversation.
- Team Bot description: max **140 characters**, written in the Bot's voice.

Create: [D]
- **New** → **Create new Bot** → named **New Bot** and opened. Or type a name → **Create "name" Bot**.
- Bots can create focused helper Bots on their own (ask first if you want a small roster). Helper Bots show in the sidebar and send notifications.
- Onboarding teammate suggestions (**Meet a future teammate**).

Edit: [D]
- Rename via the context menu **Rename Bot**, double-click, or Bot settings. Owner only; the option is hidden otherwise.
- **Bot settings**: Name, Label, Description, avatar, Notifications.
- Description holds durable rules ("Never send external messages without approval"). Messages hold task-specific instructions.

Avatar picker (click the avatar in details): [D]
- Tab **Bot**: character construction (shapes, colors; "simple shapes and expressive eyes", accessories).
- Tab **Generate**: text prompt → **Generate** → **Set avatar**.
- Tab **Upload**: drag or **Browse files**, adjust/crop, **Set avatar**. Must be under 25 MB.
- Mobile: **Select from Gallery**, **Generate**, **Remove Photo**.
- Avatar animation states: idle, working, waiting/blocked, done. [D design article]

Primary Bot: one Bot marked with a star as the everyday default; **Replace with different Bot**. [T: flaviocopes; absent from current docs]

Limits: up to 50 Bots and group chats combined per account. [T: flaviocopes]

Delete: [D]
- Removes the profile, conversation, and routines. Shared-computer files and sign-ins stay. Hiding is the non-destructive alternative.

## 3.2 Pin / hide / sections / duplicate / share / templates

- **Pin**: keeps the row at the top of the sidebar. [D]
- **Hide from sidebar**: the Bot keeps working and its routines keep running, but it **sends no notifications**. Restore with **Show in sidebar** (desktop) or **Unhide** (mobile). [D]
- **Sections**: **Move to new section**, **Move to** → section / **Create section**, **Rename** section, delete section (members go to **Unassigned**). Synced across devices. [D]
- **Mark as Unread / Read**. [D]
- **Copy conversation ID**. [D]
- **Duplicate**: the copy is named "`<name> copy`". It carries profile, settings, enabled skills, routines, and avatar. It does **not** carry conversation history, learned memory, or chat attachments. [D; documented as an iPhone action, desktop location I]
- **Share template**: [D]
  1. **Share menu** → **Create template**. The Bot builds the template; when it's ready you get **Copy link**, **View template details**, **Update template**.
  2. Visibility: **Public link** or **Team-only**. Enterprise defaults to Team-only; everyone else defaults to public. An admin can disable public sharing, enforced server-side, including for already-public templates.
  3. The recipient opens a web preview and chooses **Add to Grok Bot**, which requires the app. That creates a copy on their account (identity, description, skills, routines). No computer, logins, or history transfer.
  4. Adding accepts third-party bot terms.
  5. A warning to strip API keys, internal URLs, and customer data.

## 3.3 Memory

[D]
- Per-Bot memory: stable preferences, role context, important facts, summaries of prior work. Conversations and learned context are separate per Bot.
- Cross-Bot context moves only through shared files (`/workspace`), browser sessions, group messages, and direct handoffs.
- Users correct memory by telling the Bot. Memory writes are **not** reviewed by Auto-review.
- Duplicates don't copy memory. The Team Bot copy wizard lets the owner pick which memories to share.
- Team Bot memory: **team memory**, read by every teammate's chat and saved only when someone says the whole team should know, with the Bot announcing it. Also **notes with each person**, private per user, following them across app chat and Slack DMs. The Bot never saves personal info to team memory. You view memory by asking in chat.
- Bots learn when to interrupt for approval vs. proceed, and learn the user's writing voice. [D news]
- A Bot in a new language writes in the app language unless the user writes in another. [D]

## 3.4 Chat, group chats, mentions, attachments

1:1 chat: see 2.3. Plus: [D]
- **Redirect**: a new message preempts background work. A "**Stop now**" message ends work immediately but doesn't undo completed actions.
- Threads and reactions. Reactions are acknowledgement only.
- Drafts (email/Slack) with an editable preview before sending.
- Voice memos from the Bot.

Group chats: [D]
- Create: **New** → select **2 to 6** Bots → a name is generated and editable. Mobile: **+ → New Group Chat**.
- Membership is editable later.
- Can include a teammate's published Team Bot, but **not** alongside Bots that run on your own computer.
- Routing: a plain message lets the Bots decide who responds. `@Bot` targets one, several `@`s target several, `@everyone` goes to all.
- Bots reply in the group. Bot-to-Bot handoffs and DMs to the user show in the Bots' own chats, not the group.
- User messages in groups can have attachments. **Bot-to-group handoff messages are text-only.**
- Group **Description** (via **Bot settings** on the group) applies to every member Bot.
- No per-group notification switch.
- No voice chat in groups.
- Rename: context menu **Rename chat**, or **Group name** in settings.
- Every Bot reply counts toward usage.

Mentions (`@`): Bots, groups, routines, connectors/plugins, `@everyone`. Skills (`/`): saved private skills and packaged skills. [D]

Attachments: [D]
- Up to 6 per message (desktop). 25 MB for docs/images/audio; 200 MB for video.
- Types: images, audio, video, PDF, plain text, Word, Excel, PowerPoint, CSV, JSON, YAML, source code, HTML, email files (.eml [I]), Jupyter notebooks.
- Rejected or unreadable: too large, encrypted/password-protected, damaged, unsupported, upload not finished.
- Paste images and links. Drag and drop.
- Link cards with hover preview, opening in the system browser.
- Result cards preview supported formats, with save and open-source actions.
- Mobile: camera capture, photo picker, file picker, iOS share-sheet ingestion (photo/file/link/text → pick chat → **Attach**); the Android share sheet takes text only.

## 3.5 Handoffs between Bots

[D]
- A Bot can send an **asynchronous message** to another Bot. The receiver wakes, handles it, and replies later. The handoff is visible in the conversation.
- Bots can pass **ownership** of a task.
- Bots can share images directly Bot-to-Bot (but not in group handoffs).
- There's no switch to stop Bot-to-Bot messaging. It's controlled through instructions in descriptions.
- Approvals raised by work that started without the user (routine, trigger, another Bot's message) **expire after about 10 minutes** and show **Expired**, possibly with **Always allow this in the future**.
- Bots can delegate coding tasks to Cloud Agents and launch subagents (admin can disable). [D]

## 3.6 Chief-of-staff coordination

- Pattern, not a special object: a coordinator Bot (for example "Chief of Staff") assigns work to specialist Bots, routes routine items, and pulls the user in only for judgment calls. [D news + design article; VentureBeat tester: researcher + writer coordinated by Chief of Staff "worked out of the box"]
- Documented use-case template "Chief of Staff": source-linked digest across Slack, email, calendar, and meeting notes; flags decisions owed; tuned by marking useful vs. noise; scheduled as a digest. [D]
- Implementation needs [I]: Bots have tools to list the roster, message a Bot, create a Bot, create a group, and post to a group; the coordinator's description carries its routing rules.
- Proactive work: Bots can "identify work before the user explicitly asks." [D news]

## 3.7 Cloud computer: browser, filesystem, terminal, desktop view/takeover

[D unless marked]
- One persistent Linux VM per user account (Firecracker microVM), in the US. Bots run non-root [T: eesel].
- Shared across Bots: browser cookies/sessions, `/workspace` files, CLI credentials, installed apps.
- A separate screen per Bot (parallel computer use), with one computer-use task per Bot screen at a time.
- Tools available to Bots: shell/terminal, browser (computer use), file I/O, connectors (MCP), computer use on the desktop.
- `/workspace` is durable and synced. Temp dirs, manual installs, `node_modules`, and venvs are replaceable.
- Idle computers hibernate. Durable disk. Daily control-plane backups.
- Takeover flow: **Agent Computer** / **Open computer** → takeover → enter password/passkey/2FA/CAPTCHA/payment/identity → **I'm done** → Bot continues. The Bot never sees the typed values.
- **Computer** card marked **Action needed** with **Take over** / **Skip**.
- Page-fill forms in chat (one form per step).
- Hardware security key passthrough from the desktop (macOS/Windows), with approval on every use.
- Laptop-local passkeys and biometrics don't work on the VM.
- Egress: shared static IPs by default. Optional **route egress through this desktop** (the destination sees the desktop's IP and the Bot can reach the desktop's networks); a route stops within 5 minutes if the admin disables it.
- Lifecycle actions:
  - **Update**: software update (keeps everything) or computer update (keeps Bots/files/logins, removes installed apps/packages; can be scheduled). Refuses if the backup isn't ready (**Backup not ready**) or a Bot can't pause (**Agent busy**).
  - **Recover** (only from the unreachable error state): save what's possible, rebuild on the latest image from saved data.
  - **Reset** (Settings → Updates): rebuild from the last snapshot. Last resort.
  - Guard: don't start a second Update/Reset while one runs.
  - Recommended order: Retry → quit/reopen → Update → Recover → Reset.
- Recreate (admin): the Bot pauses at a safe point and resumes on the new computer.
- **Disk Saver**: a system Bot that audits disk use on low-space warnings and proposes cleanup, deleting nothing without confirmation.
- Local machine (separate capability): run commands, read files, and move files between the cloud computer and the local machine, gated by **Execution on Local Computer**.

## 3.8 Connected apps / integrations and auth

[D]
- Connectors are MCP servers installed as plugins from Marketplace. Account-wide: every Bot can use every installed plugin and every connected account.
- Auth: OAuth in the system browser. Tokens are held on the backend and never reach the VM or the model. States: **Waiting for authorization** (**Reopen**), failed (**Retry**); complete sign-in within about 10 minutes.
- Multi-account per plugin where supported (Notion yes, Gmail no): **Add Another Account** + label, and the user refers to it by label ("Use my work Notion").
- Per-tool enable/disable within a plugin.
- Documented capabilities: Google Calendar (read, create/update events, free/busy, RSVP; no Meet settings; no Zoom add-on), Google Sheets (read/edit cells; no sharing changes), Google Drive (search/read), Gmail (search/read/draft/send/labels; one mailbox), Slack (posts as the connected user; channel limits), Notion, Linear, Jira, Datadog, GitHub, Salesforce; plus Google Docs/Slides via Google sign-in "as Grok". Zoom OAuth is currently broken (error 4700).
- Custom MCP servers: **Remote HTTPS** (with an optional OAuth per user) or **Command** (stdio on the VM).
- Admin connector policy: blocked plugins show **Disabled by team admin**. Admins can't push connectors to members.
- **Secure secret request** for connections that need keys.
- Cloud Agents delegation for coding (admin toggle).
- Event integrations for routines (Slack, GitHub, Linear, Sentry, PagerDuty, email, webhook) are separate from plugins.

## 3.9 Skills (save, teach-a-task, recording)

[D unless marked]
- Skill = reusable instructions: when to use, required inputs/access, sequence, validation, output, approval requirements.
- One private skill library per account, shared by all Bots, with per-Bot enablement [D cdocs]. Packaged skills come from Marketplace.
- Create by asking: "Save the process we just used as a skill called ...".
- Reference with `/` in the composer. If a skill is missing, check **Marketplace → Your plugins → Manage plugins and skills → Private skills**.
- **Teach a task** (gradual rollout; desktop only):
  1. Open a 1:1 Bot chat with the computer view open.
  2. Choose **Teach a task** (top right).
  3. Enter a short name and describe the result [T: note.com "short name input field"].
  4. Perform the workflow in the cloud browser. It records visible computer interaction for up to **10 minutes**, with **no microphone audio**.
  5. **Stop** recording.
  6. The Bot drafts a skill. The user reviews and edits it (adds decision rules, failure handling, approval boundaries).
  7. Test on a safe example.
- Recording indicators and status. [T]
- The Team Bot's owner saves team skills. A teammate teaching a process only creates a personal note with that teammate.

## 3.10 Routines, automations, schedules, triggers

[D unless marked]
- Routine fields: name, owning Bot, **Instruction**, **When to run** (a natural-language rendering of schedule plus triggers), active/paused, next run, **Webhook** (URL + key), run history.
- Created by asking the Bot in natural language ("Every weekday at 8:00 AM ...", "every 2 hours"). The Bot confirms and shows the next run.
- Schedules use Settings → Bot → **Timezone** (Auto-detect or explicit).
- A new routine waits for its next scheduled time and doesn't run at creation.
- Triggers:
  - Slack: Bot mentioned, word/phrase mentioned, reaction by the user, any message. Scope: one channel or all of Slack. Only new messages count.
  - GitHub, Linear, Sentry, PagerDuty events, email.
  - Webhook: **POST to** URL, **key** (sent as `Authorization: Bearer <key>`), **header** (full header, copyable). Click a field to copy. The JSON body is passed with the instruction. **200** means a run started.
  - A Slack channel listener routine lets a Team Bot respond to every message.
- Routine detail (desktop, via details → **Tasks** → **Routines**): row shows name + schedule or **Paused**, with an inline switch. Detail view shows **Instruction**, **When to run**, **Webhook**. Buttons: **Pause**/**Resume**, **Test** (**Running…**; error **Couldn't start a test run**), **Edit** (pre-fills the composer with "Edit your routine: <name>"), **Delete routine** (trash icon, confirm, no undo).
- Mobile: **Active** toggle, **Schedule**, **Next run**, **Instruction**, **Run history** (**Running** / **Succeeded** / **Failed** + reason; **No runs yet**); **Add routine** pre-fills "Set up a routine to "; swipe to delete. Editing and testing are desktop only.
- Limits: 50 routines per Bot. 20 most recent run records per routine.
- Test runs do real work. Some routines don't offer **Test**.
- Inactivity guard: after a long absence the app may ask whether to keep routines running, and pauses them if there's no response.
- Deleting a Bot deletes its routines.
- Team Bot routines are personal: they run as the creator, in the creator's chat, on the creator's usage.
- Routine run results post to the owning Bot's chat.
- Routine config UI with a trigger dropdown (Slack new messages, reactions, mentions, scheduled times) and a channel picker after Slack connects. [T: note.com]

## 3.11 Approvals / permissions

[D]
- **Approval card**: proposed operation, target, and inputs/arguments. Actions: **Allow once**, **Always allow** (saves a matching Auto-review rule under **Auto-review Rules**), **Deny**. The card is titled **Review an action** when raised by Auto-review. States: pending, approved, denied, **Expired** (non-interactive origins after about 10 minutes), not actionable (reject/cancel).
- Approvals from an interactive chat wait indefinitely.
- Pending approvals surface in chat, in the sidebar (**Needs attention**), and as OS/push notifications if enabled.
- **Auto-review**: an independent review model that evaluates shell commands, plugin calls, computer use, automation writes (routine/trigger changes), and delegation (Cloud Agent/subagent). Outcomes: proceed, require approval, deny. It does not cover memory writes or most settings changes.
  - Toggle in Settings → Bot (locked when the admin enforces it).
  - Rules: natural-language **Ask first** and **Allow automatically** rules; **Ask first** wins on conflict. Allow rules apply only if the reviewer finds no other reason to stop.
  - Team rules show as locked rows. Personal rules can only tighten.
  - Personal rules are stored per desktop and synced to that desktop's computer, not across desktops.
- Local execution approval (see 3.14).
- Team Bot personal-connector card: **Allow** <plugin> (states which account) → **Allow once**, **Always allow** (menu: **Always allow for this Bot**, **Always allow for all Team Bots**), **Skip**. Reset in Settings → Team Bots → **Clear**.
- Spend requests (merchant, total, description, expandable breakdown; Stripe Link). [T]
- Mobile uses **Approve once** / **Deny** labels. [T: digitalapplied]

## 3.12 Secrets and sensitive input

[D]
- **Secure secret card** (in chat): masked field → **Save securely** → **Saved** / "**Saved securely and kept private**", footer "**Stored securely, never shown to your Bot**". Page-fill variant: "**Filled into the page. Secret values were never shown to your Bot.**" / "**Could not fill into the page**". Error: "**The secret was not saved. Try again.**"
- Per-Bot **Secrets** section: **Add secret** (env var name, description, value) → **Save secret**. The list shows name + description only. **Replace** → **Replace value**. **Remove**. Empty: "**No secrets yet.**" Error: "**Something went wrong. Try again.**" → **Try Again**.
- Values are write-only. Bots use them by name, and values are redacted to `[REDACTED]` in command output, file reads, and plugin calls.
- Team Bot secrets: name regex `^[A-Z][A-Z0-9_]*$`, max 25 per Bot, value 8 to 4,096 bytes.
- Never paste secrets in chat or into Teach-a-task demos.

## 3.13 Notifications and attention

[D]
- Per-Bot **Notifications** switch (default [I: on]). OS notification or mobile push when the Bot finishes or needs input.
- Suppressed while the app window is focused. The sidebar and dock badge still update.
- Hidden Bots don't notify. Groups have no switch.
- Mobile global notifications toggle; "**Notifications are off**" → **Open Settings**.
- Sidebar attention states: **Needs attention**, **Unread activity**, working/typing.
- Opening a conversation marks it read. Manual mark read/unread.
- In-app error notices above the composer (dismiss, clear all, **Copy request ID**).

## 3.14 Local computer execution

[D]
- Setting: **Ask every time** (default) / **Always allow** / **Never allow**. It lives in Settings → General → Bot, and moves to Settings → Computer → Computers per registered computer once any are registered.
- First-use prompt in chat: "**Allow Grok Bot and all Bots to run commands on your local computer?**" → **Always allow**, **Allow once**, **Never**, **Deny once** (Esc). Always/Never set the global setting.
- Each approval card shows the exact command.
- The admin ceiling can cap it, and the stricter setting wins (**Always allow** is unavailable under a stricter ceiling).
- Capabilities: run commands, read files, and transfer files between local and cloud.

## 3.15 Voice

[D]
- Dictation: **Start voice input** / `Cmd/Ctrl+D` (toggle or hold-to-talk). Transcribes into the composer.
- Live voice chat: **Start voice chat** (waveform; empty composer only; 1:1 only; not with teammates' Team Bots). States: **Connecting…** (mobile **Calling…**). Controls: **Mute**/**Unmute**, **Hang up**, **Show transcript**, **Show settings** (**Voice**, **Speed**, **Language** with **Auto-detect**). Only one call at a time (**End the current voice chat?** → **End and Chat**). You can navigate other chats during a call (**Return to the voice chat**; sidebar **Voice chat in progress**). After the call: **Voice chat** card (duration, transcript), optional follow-up work, and feedback **How was the call?** **Good**/**Bad**. Errors: **Voice chat failed**, mic denied, no mic.
- Bot voice memos: play/pause, transcript.

## 3.16 Team Bots, sharing, workspaces, roles

[D]
Roles: Team Bot **owner** (sets up, publishes, edits, deletes, connects Slack), **teammate** (chats privately, pins/hides, owns personal routines; no setup changes), **team admin** (dashboard controls, default-add Team Bots), **org admin** (computer management).

Comparison: personal Bot (only you) vs. public template (a copy each) vs. Team Bot (one Bot, private chat per person, team memory).

Create: **New** → **Create new Team Bot** → **New team bot**. A guided setup chat asks what the team needs, then walks through plugins → secrets → files (and skills). Rename via **Edit details**. It's invisible to teammates until published **and** given a non-default name plus a description.

Convert: **Share** → **Publish to Team** → **How should your Team Bot start?**: **Copy <Bot name>** (chat + memories) or **Start fresh**. Setup cards for plugins, secrets, memories, skills, files, routines (move vs. keep), with **Keep private**.

Setup pane: **Setup** rows **Plugins**, **Secrets**, **Skills**, **Files** with **Add**. Files: **Upload file** or drag; .txt, .md, .markdown, .csv, .json, .yaml, .yml; at most 256,000 characters each.

Plugin identity table:
| Plugin type | Access used | Teammate setup |
|---|---|---|
| OAuth | person talking | each connects once |
| Key/token | Bot's own credential | none |
| Custom MCP Remote HTTPS | Bot credential or per-person OAuth | none or one sign-in |
| Custom MCP Command | runs on the conversation's computer; secret-needing commands run only in the owner's chat | none |

Teammates can add personal connectors, used without asking in 1:1 chats and with an **Allow** card in shared contexts.

Where work runs: the owner's chat runs on the owner's computer; a teammate's app chat or 1:1 Slack DM runs on the teammate's computer (or a separate one if the teammate's privacy mode is stricter than the owner's); Slack channels/group DMs/threads run on one shared Bot computer.

Publish: the readiness card **Publish to team** (or ask the Bot). Afterward: listed in **New chat → Team Bots**, **Copy link**, Slack. **Unpublish** removes access temporarily (chats and routines return on republish). Delete is owner-only, deletes it for everyone, no undo.

Approvals in Team Bots: they ask only in the owner's own chat. Elsewhere they run within the configured permissions without prompting (unless Auto-review is enforced), but still ask before using personal connectors.

Slack app per Team Bot: **Bring to your team's Slack** → **Connect Slack workspace** → approve → pick workspace → **Connect** → app created with the Bot's name/avatar → welcome DM → `/invite @BotName`. Admin-approval flow: **Awaiting admin approval**, **Send request in Slack**, **Check approval**, **Cancel setup**, **Cancel request**. Behavior: DMs are answered every time (one DM = one conversation); channels/group DMs answer on @mention, then follow the thread (each thread = a conversation); non-mentions are ignored. Users must link Slack in Settings → General → Team Bots (**Link**); unlinked users get a private **Link Account** button plus a thread reminder; off-team users are refused privately. **Remove from Slack**.

Usage: charged to the person chatting; routines to the creator; unlinked Slack posts to the owner. Out-of-usage message: "**This Bot's owner has reached their usage limit**".

Admin: **Manage Team Bots** → per Bot: **All team** / **None** / groups; members then see "**Required by your admin. You can't turn this off.**" and can't hide it.

Error screens: **Team Bots Not Available**, **Bot Not Found**, and the privacy-mode mismatch message.

Voice chat is unavailable with teammates' Team Bots. Mobile can chat and answer Allow cards; create/publish/edit/Slack are desktop only.

Launch news: 4 pre-built Team Bot templates; plugin examples Salesforce, Notion, GitHub. [D news]

Teams / enterprise admin (dashboard, out of scope for the desktop client but surfaced in it): enable switch + **Manage Group Access**, **Invite Team**, SCIM, Cloud Agents toggle, Public template sharing, connector policy, Execution on Local Computer ceiling, Allow Local Egress, Team Rules, Enforce Auto-review, team Auto-review rules, Network Controls (4 modes), Team Setup manifests, Team Secrets (100 max, 32 KB each, 96 KB total, reserved names), Grok Bot Computers (Recreate/Terminate/Delete VMs and Data; Terminate Inactive after 30 days), Action Recording, Conversation content export, Audit logs, OpenTelemetry, Group settings (widen-only), Admin API, Conversation Insights. [D]

## 3.17 Security

[D]
- Per-user Firecracker microVM isolation. Bots within one user are not a security boundary.
- No access by default. Bots act as the signed-in member, with no separate machine identity.
- Connector OAuth tokens stay server-side.
- Takeover for credentials. Masked secret cards.
- Outside content is marked untrusted (prompt-injection defense). Auto-review checks actions against the request.
- Network policy (admin), static shared egress, local egress option.
- Action Recording (metadata only, 90-day retention), audit logs, OTel export with redaction.
- Data: US-hosted; deletion within 30 days after account deletion; hibernation isn't deletion.
- Legacy Privacy Mode blocks the product.
- Cleanup checklist for revoking access: pause/delete routines, sign out of sites, uninstall connectors and revoke, remove `/workspace` files, hide/delete Bots, delete the account.

## 3.18 Settings

See 2.8 for the full list. Per-Bot settings in 2.5. Mobile settings: account, plugins, Bot settings, Auto Review, appearance, **Language** (**System** + English, Chinese Simplified/Traditional, French, German, Hindi, Japanese, Korean, Polish, Portuguese, Spanish), usage / store subscription management, global notifications, **Settings → Bot → Bot Computer** (**Update Computer**, **Reset Computer**), sign out, **Delete Account** (confirm, then type **Delete** in the browser after re-auth). [D]

## 3.19 Keyboard shortcuts (desktop, complete documented list)

[D] Use `Cmd` on macOS and `Ctrl` on Windows/Linux unless noted.

| Action | Shortcut |
|---|---|
| Jump to / search palette | `Cmd/Ctrl+K` |
| Search Bots | `Cmd/Ctrl+Shift+F` |
| New Bot / new chat | `Cmd/Ctrl+N` |
| Find in this chat | `Cmd/Ctrl+F` |
| Compact sidebar | `Cmd/Ctrl+B` |
| Focus prompt | `Cmd/Ctrl+I` or `Cmd/Ctrl+L` |
| Sidebar Bot 1 to 9 | `Cmd/Ctrl+1`..`9` |
| Previous / next Bot | `Alt+↑` / `Alt+↓` |
| Back / forward in visit history | `Cmd/Ctrl+[` / `Cmd/Ctrl+]` |
| Cycle Bots | `Control+Tab` / `Control+Shift+Tab` (always Control) |
| Marketplace | `Cmd/Ctrl+Shift+M` or `Cmd/Ctrl+Shift+W` |
| Toggle Bot settings | `Cmd/Ctrl+Shift+,` |
| Conversation details | mac `Cmd+Shift+I` or `Cmd+Alt+B`; Win/Linux `Ctrl+Alt+B` |
| Send | `Enter` (composer), `Cmd/Ctrl+Enter` (anywhere) |
| New line | `Shift+Enter` |
| Dictate | `Cmd/Ctrl+D` (prompt focused; tap or hold) |
| Start voice chat | button only, no binding |
| Open Settings | `Cmd/Ctrl+,` |
| Zoom in / out / reset | `Cmd/Ctrl+=` / `Cmd/Ctrl+-` / `Cmd/Ctrl+0` |
| Fullscreen | `F11`; mac also `Control+Cmd+F` |
| Deny once (local-exec prompt) | `Esc` |
| Commit inline rename | `Enter` |

## 3.20 Plans and usage limits

[D]
- Access tiers: Cursor Pro < Pro+ < Ultra (highest); self-serve Teams (seat allowance; on-demand on by default); Enterprise (admin); linked SuperGrok < SuperGrok Plus < SuperGrok Heavy, and X Premium+ (below Plus). Lite/Team/Enterprise SuperGrok can't link. Grants do **not** stack. A link is permanent, can't be unlinked, and is re-checked at renewal; tier changes take up to 24 hours to apply.
- Pricing reported: Ultra $200/mo, Teams Premium $120/seat/mo, SuperGrok Heavy $300/mo. [T]
- **Weekly usage** (included, weekly reset) → **On-demand usage** (billed at model/token cost) → **On-demand monthly limit** (soft; a running Bot can finish past it). With on-demand off, the product stops at the limit with a "reached your Grok Bot usage limit" screen.
- Free trial: a usage credit plus a 7-day window. **Cancel Trial** on free plans. Used credit isn't restored.
- Usage is metered by agent steps and tokens, not messages. Routines, tests, group replies, and Bot-to-Bot messages all consume it.
- One usage bucket across devices.
- Mobile in-app purchase: monthly individual plans only.
- There's no Grok Bot-specific spend cap for teams.
- Errors: **Model provider is overloaded**, **Bot failed to respond**.

## 3.21 Updates, versioning, support

[D]
- Auto-update check; **Automatic Updates** toggle; forced **Update required** after 14 days outdated.
- **About** → **Copy version info**.
- **Copy request ID** on messages and notices; **Copy conversation ID** on sidebar rows.
- Service status lives externally (no in-app status).
- Linux: deb/rpm/AppImage; apt/dnf repos.

## 3.22 Mobile parity (reference, not in desktop scope)

[D] Mobile covers: login + link, first-run, synced Bot list, messaging (text, dictation, voice chat, voice memos, camera/photo/file, mentions, threads, reactions, drafts), share-sheet ingestion, **+** → **New Bot** / **New Group Chat**, edit profile (**Edit name**, **Instructions**, **Title (optional)**, picture), manage group members, pin/hide/unhide/delete/duplicate, swipe → **Move to** → **New Section**, computer view/takeover, routine view/pause/delete/add, search with scopes, settings, **Authorize new account** for plugins. Missing on mobile: teach-a-task, routine edit/test, computer update/recover, Team Bot creation/publishing/Slack, advanced controls.

---

# 4. Feature checklist (definition of "100% feature complete")

Tags: D = documented, T = third-party, I = inferred. Mobile-only and admin-dashboard-only items are listed in 4.M and 4.A. They count toward full parity, but a desktop-only build can scope them out.

## 4.1 Install, auth, onboarding
1. [D] macOS build (Apple silicon + Intel), .dmg, drag to Applications
2. [D] Windows build (x64 + Arm64) with installer (silent `/S` supported)
3. [D] Linux builds (x64 + Arm64): .deb, .rpm, AppImage; apt/dnf repos
4. [D] Welcome screen with **Sign in** / **Get started**
5. [D] Browser-based OAuth sign-in with app refocus on completion
6. [D] **Sign In with Cursor** from Settings when signed out
7. [D] SSO-compatible sign-in (org flow)
8. [D] Paywall screen with **Link Grok Account** and **Link X Account** (pre-sign-in only)
9. [D] Access-gate states: Legacy Privacy Mode error, no-access / request-access, phone-verification block
10. [D] Intro tour (Bots, shared computer, routines)
11. [D] "Which tools do you use?" questionnaire feeding suggestions
12. [D] Background computer provisioning with **Starting your computer** progress
13. [D] **Meet a future teammate** suggestion gallery + **Create your own** (name, job, description)
14. [D] Auto-created default first Bot named **Grok Bot** (alternate flow)
15. [D] Multiple saved accounts: **Switch account**, **Add account**, **Remove** inactive
16. [D] Sign out
17. [D] **Update required** gate (over 14 days old) with Update / Restart / Try again / Download latest / Linux **Copy command**

## 4.2 App shell and navigation
18. [D] Left sidebar + main chat + right details/computer pane layout
19. [D] **Compact sidebar** toggle (`Cmd/Ctrl+B`)
20. [T] Title-bar computer status icon (purple when active)
21. [D] Command/search palette (`Cmd/Ctrl+K`)
22. [D] Search Bots (`Cmd/Ctrl+Shift+F`)
23. [D] Palette: switch Bots/groups
24. [D] Palette: cross-conversation message search with jump-to-message
25. [D] Palette: find files, links, routines
26. [D] Palette: open settings and common actions
27. [D] Search result scopes: All, Messages, Bots, Group Chats, Files, Routines
28. [D] Visit history back/forward (`Cmd/Ctrl+[`, `]`)
29. [D] Sidebar Bot 1 to 9 shortcuts
30. [D] Previous/next Bot (`Alt+↑/↓`)
31. [D] Cycle Bots (`Control+Tab` / `Control+Shift+Tab`)
32. [D] Zoom in/out/reset
33. [D] Fullscreen (`F11`, mac `Control+Cmd+F`)
34. [D] Dock/taskbar unread badge
35. [D] Background running after window close; full Quit from menu bar
36. [D] Account menu: Settings, About, Get Grok Bot for mobile, Weekly usage glance, update available/Install
37. [D] About dialog: Version + **Copy version info** (version, track, OS)
38. [D] Localized UI: Follow System + 31 languages
39. [D] Theme: Follow System / Light / Dark

## 4.3 Sidebar management
40. [D] **New** button → New chat picker (`Cmd/Ctrl+N`)
41. [D] Pin / unpin Bots and groups
42. [D] Sections: **Move to new section**
43. [D] Sections: **Move to** existing section / **Create section**
44. [D] Sections: **Rename** section
45. [D] Sections: delete section → members to **Unassigned**
46. [D] Sections sync across devices
47. [D] **Hide from sidebar** (keeps running, suppresses notifications)
48. [D] **Hidden Bots** list / **Show Hidden Bots** when all hidden
49. [D] **Show in sidebar** (unhide)
50. [D] **Mark as Unread** / Mark as Read
51. [D] **Rename Bot** via context menu (owner only)
52. [D] Inline rename by double-click + Enter
53. [D] **Copy conversation ID**
54. [D] **Delete** from context menu (with confirmation)
55. [D] **Rename chat** for group rows
56. [D] **Copy link** for published Team Bot rows
57. [D] Attention states: Needs attention / Unread activity / working-typing
58. [D] **Voice chat in progress** sidebar indicator
59. [D] Animated avatar states (idle, working, waiting/blocked, done)
60. [D] Team Bot row shows owner name
61. [D] Admin-required Team Bots cannot be hidden
62. [T] Primary Bot star badge + **Replace with different Bot**
63. [T] Roster cap of 50 Bots + groups combined

## 4.4 Bots and profiles
64. [D] **Create new Bot** (default name **New Bot**)
65. [D] **Create "name" Bot** from typed name
66. [D] Bot fields: Name, Label (optional), Description, avatar, Notifications
67. [D] **Bot settings** panel (toggle `Cmd/Ctrl+Shift+,`)
68. [D] Avatar picker tab **Bot** (character shapes, colors, accessories)
69. [D] Avatar picker tab **Generate** (text prompt → Generate → Set avatar)
70. [D] Avatar picker tab **Upload** (drag / Browse files, adjust, Set avatar, <25 MB)
71. [D] Owner-only edit enforcement
72. [D] Bots can create helper Bots
73. [D] Helper Bots appear in sidebar with own notification switch
74. [D] **Duplicate** Bot ("<name> copy"; profile, settings, skills, routines, avatar; no history/memory/attachments)
75. [D] Delete Bot (removes profile, conversation, routines; leaves computer files/logins)
76. [D] New Bots write in app language unless user writes otherwise
77. [D] **Disk Saver** system Bot

## 4.5 Templates / sharing
78. [D] Share menu → **Create template** (Bot builds it)
79. [D] **Copy link**, **View template details**, **Update template**
80. [D] Link visibility **Public link** / **Team-only** (plan-based default)
81. [D] Admin-disabled public sharing enforcement
82. [D] Web preview page with **Add to Grok Bot**
83. [D] Import template → copy on recipient account (identity, description, skills, routines)
84. [D] Third-party bot terms acceptance on add
85. [D] Warning to strip secrets/internal data before sharing

## 4.6 Memory
86. [D] Per-Bot persistent memory (preferences, facts, work summaries)
87. [D] Memory isolation per Bot
88. [D] User correction of memory via chat
89. [D] Memory not copied on duplicate
90. [D] Team memory (shared, announced when saved)
91. [D] Per-person private notes on Team Bots (follow across app and Slack DM)
92. [D] Memory inspection by asking the Bot
93. [D] Selective memory transfer in Publish-to-Team wizard

## 4.7 Chat and composer
94. [D] Text messaging with markdown rendering
95. [D] `Enter` send, `Shift+Enter` newline, `Cmd/Ctrl+Enter` send anywhere
96. [D] Focus prompt (`Cmd/Ctrl+I`/`L`)
97. [D] Per-conversation draft persistence
98. [D] Send while Bot is working; user message preempts/redirects
99. [D] "Stop now" handling (immediate stop; no undo)
100. [D] Paste text, links, images
101. [D] Attach files via control and drag-drop
102. [D] Attachment limits: 6 per message, 25 MB, 200 MB video
103. [D] Supported types: images, audio, video, PDF, text, Word, Excel, PowerPoint, CSV, JSON, YAML, code, HTML, email, notebooks
104. [D] Attachment error handling (size, encrypted, damaged, unsupported, incomplete upload)
105. [D] `/` skill picker
106. [D] `@` mention picker: Bots, groups, routines, connectors
107. [D] Reply to specific message / threads
108. [D] Reactions
109. [D] **Find in this chat** (`Cmd/Ctrl+F`)
110. [D] Link cards with hover preview; open in system browser
111. [D] File/image/tool-result cards with preview, save, open source
112. [D] Tool activity and computer-use events in transcript
113. [D] Bot questions rendered in transcript
114. [D] Structured UI replies (cards, forms, boards, visualizations)
115. [D] In-chat page-fill forms (one per step)
116. [D] Email draft card (**New Email**; edit recipients/body; **Send email** / **Discard**)
117. [D] Slack draft card (**New Slack Message**; **Send message** / **Discard**)
118. [D] Message hover **More message actions**
119. [D] **Copy request ID** (right-click and hover menu)
120. [D] In-app error notices above composer (dismiss one, clear all, Copy request ID)
121. [D] Errors **Bot failed to respond**, **Model provider is overloaded**
122. [D] Conversation header with Bot name → details
123. [D] Conversation details shortcut (`Cmd+Shift+I` / `Cmd+Alt+B` / `Ctrl+Alt+B`)
124. [D] Auto mark-read on open

## 4.8 Voice
125. [D] Dictation **Start voice input** (`Cmd/Ctrl+D`, tap or hold-to-talk) into composer
126. [D] **Start voice chat** waveform button (empty composer only)
127. [D] Mic permission request
128. [D] **Connecting…** state
129. [D] **Mute**/**Unmute**, **Hang up**
130. [D] **Show transcript** (live)
131. [D] **Show settings**: Voice, Speed, Language (Auto-detect)
132. [D] Persistent **Voice chat** control under header; **Return to the voice chat**
133. [D] Single active call; **End the current voice chat?** → **End and Chat**
134. [D] Post-call **Voice chat** card (duration, transcript)
135. [D] Post-call follow-up work by Bot
136. [D] **How was the call?** Good/Bad feedback
137. [D] **Voice chat failed** errors (ended unexpectedly, realtime failed, mic denied, no mic) + Dismiss
138. [D] Settings **Microphone** device picker
139. [D] Voice chat disabled in groups and with teammates' Team Bots
140. [D] Bot voice memos: play/pause, expandable transcript

## 4.9 Group chats and handoffs
141. [D] Create group from New chat (select 2 to 6 Bots)
142. [D] Generated, editable group name
143. [D] Edit group membership
144. [D] Group **Description** read by all member Bots
145. [D] Default routing (Bots decide who responds)
146. [D] `@Bot` targeting, multi-mention
147. [D] `@everyone`
148. [D] Bots post and pass work within the group
149. [D] Bot-to-group handoffs text-only
150. [D] User attachments allowed in groups
151. [D] Add teammate Team Bot to group (not mixed with local-computer Bots)
152. [D] Async Bot-to-Bot messaging with wake and later reply
153. [D] Handoff visibility in transcripts (in Bots' own chats)
154. [D] Ownership transfer of tasks between Bots
155. [D] Direct Bot-to-Bot image sharing
156. [D] Coordinator/Chief-of-Staff pattern (roster awareness, assign, route, escalate)
157. [D] Proactive work detection
158. [D] Delegation to coding Cloud Agents / subagents (admin-toggleable)

## 4.10 Cloud computer
159. [D] Per-user persistent Linux VM (microVM) shared by all Bots
160. [D] Per-Bot screen; parallel computer use; one task per screen
161. [D] Browser with persistent shared cookies/sessions
162. [D] Terminal/shell tool
163. [D] Durable `/workspace` filesystem shared across Bots
164. [T] Desktop with Finder-like file manager, terminal, browser
165. [D] Time-of-day wallpaper
166. [D] **Agent Computer** / **Open computer** control
167. [D] Preview side panel (live clicks, typing, nav, status)
168. [D] Full-screen takeover with keyboard/mouse control
169. [D] **Computer** card **Action needed** → **Take over** / **Skip**
170. [D] **I'm done** return control
171. [D] Work continues with app/laptop closed
172. [D] Hardware security key passthrough (mac/Win), per-use approval, setting toggle
173. [D] **Route egress through this desktop** toggle with admin lock states
174. [D] States: Starting / Updating / Reconnecting / Couldn't Reach Grok Bot's Computer / Retry
175. [D] **Recover computer** from error state with confirm **Recover Grok Bot's Computer**
176. [D] **Continue in Background**, **Keep waiting**, **Update still running**
177. [D] **Retry Recovery**, **Retry Reset**
178. [D] Settings → Updates → Grok Bot's Computer **Update** (software vs. computer update text; schedulable)
179. [D] **Reset** (last snapshot)
180. [D] **Backup not ready** and **Agent busy** guards
181. [D] Concurrent-operation guard (no second Update/Reset)
182. [D] Bot pause at safe point and resume across recreate
183. [D] Idle hibernation / wake
184. [D] Low/critical disk warnings → **Go to Disk Saver**
185. [D] Disk Saver proposes cleanup, confirm-only deletion
186. [D] Local file transfer between cloud and local machine (gated)

## 4.11 Connectors / Marketplace
187. [D] Marketplace browse + search (plugins and packaged skills) (`Cmd/Ctrl+Shift+M`/`W`)
188. [D] **Add** plugin
189. [D] Browser OAuth **Authorize**/**Authenticate**
190. [D] **Waiting for authorization** + **Reopen**; failure + **Retry**; ~10 min window
191. [D] Status: Added / Needs auth / Disconnected / Connected / Disabled by team admin
192. [D] **Remove** plugin
193. [D] **Your plugins** / **Manage plugins and skills** with **Installed** and **Private skills**
194. [D] Multi-account per plugin (**Add Another Account** + label)
195. [D] Per-tool enable/disable
196. [D] In-chat **Connect** card
197. [D] Account-wide plugin availability
198. [D] Server-side OAuth token custody (tools invoked without token exposure)
199. [D] Custom MCP servers: Remote HTTPS and Command (stdio)
200. [D] Team-required/restricted plugins
201. [D] Built-in connectors: Gmail, Google Calendar, Drive, Sheets, Docs, Slides, Slack, Notion, Linear, Jira, GitHub, Datadog, Salesforce (at minimum)

## 4.12 Skills
202. [D] Save skill from conversation by request
203. [D] Private skill library shared across Bots
204. [D] Per-Bot skill enablement
205. [D] Packaged skills from Marketplace
206. [D] **Teach a task** button (1:1 chat, computer view open)
207. [T] Teach-a-task short name + result description input
208. [D] Recording of visible computer interaction, max 10 min, no audio
209. [D] **Stop** recording; recording status indicator
210. [D] Bot drafts skill from demo; user review/edit
211. [D] Skill test on safe example
212. [D] Team skills (owner-only save; teammate teachings become personal notes)

## 4.13 Routines
213. [D] Create routine via natural language in chat
214. [D] Schedule parsing (weekday times, intervals) with Timezone setting (Auto-detect/explicit)
215. [D] **When to run** natural-language display
216. [D] Next-run computation; no run at creation
217. [D] Slack triggers: mention of Bot, phrase, user reaction, any message; one channel or all
218. [D] GitHub, Linear, Sentry, PagerDuty event triggers
219. [D] Email trigger
220. [D] Webhook trigger: **POST to**, **key**, **header** (click-to-copy); Bearer auth; JSON body passthrough; 200 = run started
221. [D] Details → **Tasks** → **Routines** list with row switch, **Paused** label, empty state
222. [D] Routine detail: **Instruction**, **When to run**, **Webhook**
223. [D] **Pause**/**Resume**
224. [D] **Test** (**Running…**, **Couldn't start a test run**); result posted to chat
225. [D] **Edit** → composer prefill "Edit your routine: <name>"
226. [D] **Delete routine** (trash, confirm, no undo)
227. [D] Run history (Running/Succeeded/Failed + reason; No runs yet), 20 records kept
228. [D] 50 routines per Bot cap
229. [D] Background execution in cloud while offline
230. [D] Inactivity check prompt and auto-pause
231. [D] Routine results post to owning Bot's chat
232. [T] Trigger dropdown + Slack channel picker UI
233. [D] Team Bot routines are personal (run as creator)

## 4.14 Approvals and Auto-review
234. [D] Approval card: operation, target, inputs
235. [D] **Allow once** / **Always allow** / **Deny**
236. [D] **Always allow** creates Auto-review rule
237. [D] **Review an action** card variant from Auto-review
238. [D] Expiry after ~10 min for non-interactive origins (**Expired**, **Always allow this in the future**)
239. [D] Reject/cancel non-actionable cards
240. [D] Auto-review switch (lockable **Required by your admin**)
241. [D] Auto-review rules table: Ask first / Allow automatically; Ask-first-wins
242. [D] Locked team rule rows
243. [D] Per-desktop rule storage synced to computer
244. [D] Auto-review scope: shell, plugin calls, computer use, automation writes, delegation
245. [D] Team Bot personal-connector **Allow** card (Allow once / Always allow for this Bot / all Team Bots / Skip)
246. [D] **Clear connector preferences**
247. [T] Spend-request card (merchant, total, description, breakdown)
248. [D] Pending approvals surface in sidebar and notifications

## 4.15 Secrets
249. [D] In-chat secure secret card (masked, **Save securely**, Saved states, footer)
250. [D] Page-fill secret variant (Filled into the page / Could not fill)
251. [D] Per-Bot **Secrets** list (name + description), **Add secret**, **Save secret**
252. [D] **Replace** / **Replace value**, **Remove**
253. [D] Empty and error states (No secrets yet, Something went wrong, Try Again, not saved)
254. [D] Write-only values; env-var injection by name
255. [D] Output redaction to `[REDACTED]`
256. [D] Team Bot secret constraints (name regex, 25 max, 8 to 4,096 bytes)

## 4.16 Local execution
257. [D] Execution on Local Computer setting (Ask every time / Always allow / Never allow)
258. [D] Per-registered-computer **Execution on this computer**
259. [D] First-use modal (Always allow / Allow once / Never / Deny once Esc)
260. [D] Per-command approval card showing exact command
261. [D] Admin ceiling enforcement (stricter wins; disables Always allow)
262. [D] Local command execution, file read, file transfer

## 4.17 Notifications
263. [D] Per-Bot Notifications switch with description text
264. [D] OS notifications on finish / needs input
265. [D] Suppress while app focused
266. [D] Hidden Bots silent; groups no switch
267. [D] Unread/attention badges

## 4.18 Team Bots
268. [D] **Create new Team Bot** (**New team bot**) with guided setup chat
269. [D] **Edit details** (name, 140-char description)
270. [D] Visibility gate (default name / no description hides it)
271. [D] Setup pane rows Plugins/Secrets/Skills/Files with **Add**
272. [D] Team files upload (.txt/.md/.markdown/.csv/.json/.yaml/.yml, ≤256,000 chars)
273. [D] Plugin identity rules (OAuth per person, key per Bot, MCP HTTPS/Command)
274. [D] Teammates' personal connectors usable
275. [D] **Publish to Team** from personal Bot → **How should your Team Bot start?** (Copy / Start fresh)
276. [D] Setup cards for plugins, secrets, memories, skills, files, routines (move/keep), **Keep private**
277. [D] Ready card **Publish to team** with copy-link
278. [D] **Unpublish** / republish (restores teammates' chats/routines)
279. [D] New chat → **Team Bots** directory with creator names, **Search Team Bots** (≥3 chars), **Add** → **Start a chat**
280. [D] Open Team Bot via link
281. [D] Private per-teammate chats
282. [D] Teammate permissions: pin, hide, own routines; no setup changes/delete
283. [D] Computer routing (owner/teammate/shared Slack computer; privacy-mode fallback)
284. [D] Approval behavior (owner chat only; else within permissions unless enforced)
285. [D] Owner-only delete for all (no undo)
286. [D] Usage attribution (chatter, routine creator, owner for unlinked Slack)
287. [D] Error screens: Team Bots Not Available, Bot Not Found, owner usage limit, privacy-mode mismatch
288. [D] **Bring to your team's Slack** flow (Connect workspace, pick, Connect, welcome DM)
289. [D] Slack admin approval states (Awaiting, Send request, Check approval, Cancel setup/request)
290. [D] Slack behavior (DMs, @mention + thread follow, ignore non-mentions)
291. [D] Settings → Team Bots **Link** Slack account; **Link Account** prompt in Slack
292. [D] **Remove from Slack**
293. [D] Admin default-add Team Bots (non-hideable, "Required by your admin" text)
294. [D] Pre-built Team Bot templates (4 at launch)

## 4.19 Settings and billing
295. [D] Settings dialog (`Cmd/Ctrl+,`) with General / Computer / Usage & Billing / Team Setup / Updates
296. [D] Timezone setting
297. [D] **Weekly usage** meter (tier-named)
298. [D] **On-demand usage** row ("Billed through Cursor")
299. [D] **On-demand monthly limit** + **Enable**
300. [D] **Link SuperGrok Heavy** (tier-named) link action
301. [D] **Cancel Trial**
302. [D] Usage-limit reached screen
303. [D] Soft monthly cap (running Bot finishes)
304. [D] Team Setup view (review/reinstall manifests)
305. [D] **Check for Updates**, **Restart to Update**, **Automatic Updates**
306. [D] No model picker (server-side routing)

## 4.M Mobile parity (optional for desktop clone)
307. [D] iOS 18+/iPadOS 18+ and Android 9+ apps
308. [D] **Log In or Sign Up**, **Link Grok Account**, **Finished Linking? Refresh My Status**
309. [D] Synced Bot list, conversations, routines, computer
310. [D] **Start dictation**, voice chat (**Calling…**), voice memos
311. [D] Camera capture, photo/file picker
312. [D] iOS share-sheet ingestion (→ chat → **Attach**); Android text share
313. [D] **+** → **New Bot** / **New Group Chat**
314. [D] Profile edit (**Edit name**, **Instructions**, **Title (optional)**, **Select from Gallery**/**Generate**/**Remove Photo**)
315. [D] Swipe row → **Move to** → **New Section**; pin/hide/**Unhide**/delete/duplicate
316. [D] Routine view: Active, Schedule, Next run, Instruction, Run history; **Add routine**; swipe delete
317. [D] Search with scope tabs
318. [D] **Authorize new account** for plugins; Plugins via avatar menu
319. [D] Approval cards and Team Bot Allow cards
320. [D] Mobile settings incl. **Bot Computer** (**Update Computer**, **Reset Computer**), notifications toggle, language
321. [D] **Delete Account** flow (confirm + type Delete in browser)
322. [D] In-app store subscriptions (monthly individual)
323. [D] Push notifications (rolling out)

## 4.A Admin dashboard (optional, web)
324. [D] Enable switch + **Manage Group Access**
325. [D] **Invite Team**
326. [D] Cloud Agents toggle
327. [D] Public template sharing toggle
328. [D] Connector policy / MCP allowlist
329. [D] Execution on Local Computer ceiling
330. [D] Allow Local Egress
331. [D] Team Rules (scoped to product(s))
332. [D] Enforce Auto-review + team Auto-review rules (**Configure Rules**)
333. [D] Network Controls (4 modes, group policies, **Lock for all groups**)
334. [D] Team Setup manifests (Setup Script, Check Script)
335. [D] Team Secrets (100 / 32 KB / 96 KB, reserved names, redacted logs)
336. [D] Grok Bot Computers bulk ops (Recreate VMs, Terminate VMs, Delete VMs and Data, progress card, Done, Retry Start)
337. [D] **Restart VMs to apply changed settings** / **Restart all VMs**
338. [D] Terminate Inactive Computers (30 days)
339. [D] Action Recording (metadata, 90-day retention)
340. [D] Conversation content export (Prompts, Responses, Tool I/O)
341. [D] Audit logs (Grok Bot control-plane events)
342. [D] OpenTelemetry export (`cursor.surface=grok_bot`)
343. [D] Group settings (widen-only: capabilities, network, Group Rules, Setup Scripts, **Don't enforce for this group**)
344. [D] **Manage Team Bots** default assignment
345. [D] Admin API
346. [D] Conversation Insights analytics

**Checklist total: 346 items** (306 core desktop items 1 to 306, 17 mobile, 23 admin).

---

# 5. Open questions / gaps

- Exact pixel layout, colors, and typography are not documented. Screenshots referenced in help (onboarding-landing, sidebar-bot-menu, settings-appearance, settings-bot-section, avatar-picker, bot-settings, settings-network, settings-computer) live at `cursor.com/docs-static/images/grok-bot/*-light.png` and are worth pulling for visual reference.
- "Primary Bot", spend-request cards, and the 50-Bot roster cap come only from third parties.
- The desktop location of **Duplicate** isn't stated (documented on iPhone).
- The exact desktop "Tasks" tab contents beyond Routines aren't documented.
- The complete built-in Marketplace catalog isn't published. The connectors listed are the ones named anywhere in docs or news.
