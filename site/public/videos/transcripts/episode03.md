# Projects, tasks, agents, and worktrees

Beginner · Episode 3

## 1. Asylum on the setup flow.

A project in Asylum is just a git repository. On first run it walks you through opening one.

## 2. File picker on a small repo folder.

Point it at a repo folder. If that folder isn't a git repo yet, Asylum asks for explicit consent before it runs git init — it never touches your folders silently.

## 3. Setup doctor panel with check rows.

A built-in setup doctor checks that git, branches, worktrees, your agents, Bun, and Cargo are all in order, and tells you what's missing.

## 4. Project now open, base branch shown.

Once it's open, the project has a base branch — usually main — where merged winners will land. Per-task worktrees get created under the project, by default in dot-asylum slash worktrees.

## 5. Settings surface opening.

Now let's pick our agents. Command-comma opens Settings, which is a real editor over your settings file.

## 6. Callout: $XDG_CONFIG_HOME/asylum/settings.json.

That file lives under your config directory, at asylum slash settings dot json. It's JSON with comments, so you can annotate it freely.

## 7. The default_agents key in the editor.

Here's the key that matters: default underscore agents. These are the agents a task fans out to by default.

## 8. Editing the array to ["claude-code", "codex"].

Ids come from the built-in catalog — claude-code, codex, opencode, gemini, aider, cursor-agent, and many more. I'll set two I have installed.

## 9. Save; theme/keybindings note.

Save it, and it applies live — a watcher reloads the config, no restart. Leave the list empty instead and Asylum asks you which agents to use each time.

## 10. External terminal.

One common first snag: an agent that isn't actually installed. Asylum launches agents by program name, so if your shell can't find it, Asylum can't either. Check with the agent's own version command.

## 11. Terminal.

You can also let Asylum drive one agent from the CLI. From source that's cargo run, dash p, cli, dash dash, run, the agent id, and a prompt.

## 12. Terminal echoing $ claude -p "say hello".

Watch the line it echoes — dollar sign, claude, dash p, your prompt. That's exactly what Asylum runs, no shell in between. If that program is missing, install the agent's CLI and retry.

## 13. Back to the setup doctor's agent rows.

The doctor in the app shows installed-versus-verified state for each configured agent too, so you're not guessing.

## 14. Settings with a per-agent override stub.

And if you need to tweak one agent — a wrapper program, extra arguments, or disabling it — that's the agents key. We'll cover overrides in the config episode.

## 15. Project open, two agents set.

Project open, agents chosen and verified. Next: our first fan-out.
