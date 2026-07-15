# Episode 02 — Open a Project and Pick Agents

**Duration:** ~5 min · **Level:** Beginner
**You'll learn:** how to open a repo as a project, where `settings.json` lives, how to choose default agents, and how to confirm an agent is on your PATH.
**Prerequisites:** [Episode 01](01-install-and-first-launch.md) — Asylum built and running.

## Shot list

| Time | On screen | Action | Narration |
|------|-----------|--------|-----------|
| 0:00 | Asylum on the setup flow. | Start the open-project flow. | "A project in Asylum is just a git repository. On first run it walks you through opening one." |
| 0:16 | File picker on a small repo folder. | Point Asylum at a repo. | "Point it at a repo folder. If that folder isn't a git repo yet, Asylum asks for explicit consent before it runs git init — it never touches your folders silently." |
| 0:36 | Setup doctor panel with check rows. | Show the doctor results. | "A built-in setup doctor checks that git, branches, worktrees, your agents, Bun, and Cargo are all in order, and tells you what's missing." |
| 0:58 | Project now open, base branch shown. | Highlight the base branch label. | "Once it's open, the project has a base branch — usually main — where merged winners will land. Per-task worktrees get created under the project, by default in dot-asylum slash worktrees." |
| 1:20 | Settings surface opening. | Press `cmd-,`. | "Now let's pick our agents. Command-comma opens Settings, which is a real editor over your settings file." |
| 1:36 | Callout: `$XDG_CONFIG_HOME/asylum/settings.json`. | Overlay the path. | "That file lives under your config directory, at asylum slash settings dot json. It's JSON with comments, so you can annotate it freely." |
| 1:56 | The `default_agents` key in the editor. | Scroll to `default_agents`. | "Here's the key that matters: default underscore agents. These are the agents a task fans out to by default." |
| 2:14 | Editing the array to `["claude-code", "codex"]`. | Type the two ids. | "Ids come from the built-in catalog — claude-code, codex, opencode, gemini, aider, cursor-agent, and many more. I'll set two I have installed." |
| 2:36 | Save; theme/keybindings note. | Save the file. | "Save it, and it applies live — a watcher reloads the config, no restart. Leave the list empty instead and Asylum asks you which agents to use each time." |
| 2:58 | External terminal. | Run `claude --version` then `codex --version`. | "One common first snag: an agent that isn't actually installed. Asylum launches agents by program name, so if your shell can't find it, Asylum can't either. Check with the agent's own version command." |
| 3:24 | Terminal. | Run `cargo run -p cli -- run claude-code "say hello"`. | "You can also let Asylum drive one agent from the CLI. From source that's cargo run, dash p, cli, dash dash, run, the agent id, and a prompt." |
| 3:46 | Terminal echoing `$ claude -p "say hello"`. | Point at the echoed command. | "Watch the line it echoes — dollar sign, claude, dash p, your prompt. That's exactly what Asylum runs, no shell in between. If that program is missing, install the agent's CLI and retry." |
| 4:10 | Back to the setup doctor's agent rows. | Show installed-vs-verified state. | "The doctor in the app shows installed-versus-verified state for each configured agent too, so you're not guessing." |
| 4:32 | Settings with a per-agent override stub. | Scroll to the `agents` key. | "And if you need to tweak one agent — a wrapper program, extra arguments, or disabling it — that's the agents key. We'll cover overrides in the config episode." |
| 4:50 | Project open, two agents set. | Hold. | "Project open, agents chosen and verified. Next: our first fan-out." |

## B-roll / capture notes
- Use a genuinely small throwaway repo (a few files) so worktrees stay cheap in later episodes.
- Install two real agent CLIs before recording (e.g. `claude-code` and `codex`) so the PATH checks succeed on camera.
- From a released install the CLI call is `asylum run claude-code "say hello"`; show the source form (`cargo run -p cli -- ...`) since you're recording against the dev build.
- Capture the setup doctor with at least one green row and, ideally, one "missing" row to show what a gap looks like.

## Recap card (end screen)
- A project is a git repo; the setup doctor checks your toolchain and agents.
- `settings.json` is JSON-with-comments at `$XDG_CONFIG_HOME/asylum/settings.json`, live-reloaded.
- Set `default_agents` to the ids you want to race; empty means "ask each time."
- Verify each agent on your PATH — `asylum run <agent> "..."` echoes the exact launch command.

## Next
- [Episode 03 — Your First Fan-Out](03-your-first-fanout.md)

Go deeper: [book chapter 2](../book/02-installation-and-setup.md).
