# Asylum for non-engineers

You do not need to be a programmer to use Asylum. If you can describe what you
want in plain English, you can put a fleet of AI coding agents to work and review
what they produce. This page is the no-jargon path.

## The one-minute mental model

- A **project** is a folder of code (a "repository") you point Asylum at.
- A **task** is something you want done, written in plain English —
  "add a dark-mode toggle", "fix the login bug".
- When you run a task, Asylum gives each AI agent its **own private copy** of the
  project (a "worktree") so they never trip over each other. Running the same
  task on several agents at once is called a **fan-out**.
- You then **compare** what each agent changed, pick the best, and **merge** it
  back — or send comments and let an agent try again.

Everywhere those words appear in the app, hover the label for a plain-language
explanation.

## Getting set up (once)

1. **Install Asylum.** Go to the
   [releases page](https://github.com/wess/asylum/releases) and download the file
   for your computer — no developer tools required:
   - **macOS**: `Asylum.dmg`. Open it and drag Asylum to Applications.
   - **Linux**: the `.deb` (Debian/Ubuntu), the `.AppImage` (any distribution —
     make it executable and run it), or the `.tar.gz`.
   - **Windows**: the `.msi` installer or the `.zip`. Windows support is still
     **beta** — it builds, but it has not been tested on a real Windows machine
     yet, so expect rough edges. Windows will also warn about an "unknown
     publisher" because the installer is not signed yet; that warning is expected.

2. **Get past the macOS security warning.** *(macOS only — do this once.)* Asylum
   is not yet signed with an Apple certificate, so macOS blocks it the first time
   and may say the app is damaged or cannot be opened. It isn't damaged; macOS
   just doesn't recognize the publisher. Control-clicking the app no longer gets
   around this on recent macOS versions. Do this instead:
   - Try to open Asylum once, and let it be blocked.
   - Open **System Settings → Privacy & Security**, scroll to the **Security**
     section, find the message about Asylum, and click **Open Anyway**.
   - Confirm when it asks again.

   If that doesn't work, open the **Terminal** app and paste this line, then press
   Return:

   ```sh
   xattr -dr com.apple.quarantine /Applications/Asylum.app
   ```

   Then open Asylum normally. You only have to do this once.

3. **Install at least one agent.** Asylum runs coding agents you already have
   access to (Claude Code, Codex, and others). The **setup doctor** on the Tasks
   screen tells you which are installed and shows a copy-paste install command
   for the ones that aren't. Paste it into a terminal once.

4. **Open a project.** Click *Open a folder…* and choose a code folder. If it
   isn't yet tracked by git, Asylum asks first before setting it up — nothing
   happens without your say-so.

## Your first task

1. On the **Tasks** screen, type what you want in plain English.
2. Click *Choose agents* if you want more than one to try it; otherwise Asylum
   picks a ready one for you.
3. Click **Create and run**. Each agent starts working in its own copy.
4. Watch progress on the run cards. When they finish, click **Review** on one.
5. In review you see exactly what changed. Click a line to leave a comment, then
   **Send review to agent** to ask for changes — or **Merge** to keep it.

## Reviewing without reading code

- Green lines were **added**, red lines were **removed**.
- The **checks** badge (PASS/FAIL) tells you whether the project's own tests
  still pass — a quick health signal even if you don't read the code.
- Switch between **Unified** and **Side-by-side** views, whichever is clearer.
- If you're unsure, leave a comment in plain English and send it back; the agent
  will explain or revise.

## From your phone

Turn on the mobile companion in Settings and set a token — it's required
either way, whether you're reaching it from this machine or from your phone.
You can watch runs, read what agents are doing, and send a follow-up message
that is delivered straight to the running agent.

## When something looks wrong

Asylum won't do anything destructive without asking. Merges, deletes, and
worktree removal all pop a confirmation first, and error messages tell you what
to do next. If an agent isn't found, the setup doctor shows how to install it.
