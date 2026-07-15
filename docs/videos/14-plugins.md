# Episode 14 — Plugins

**Duration:** ~7 min · **Level:** Advanced
**You'll learn:** the `plugin.toml` manifest, the process and WASM runtimes, installing and discovering plugins, and building a simple one.
**Prerequisites:** [Episode 11](11-the-cli-tour.md) — you know `asylum plugin`.

## Shot list

| Time | On screen | Action | Narration |
|------|-----------|--------|-----------|
| 0:00 | A plugins directory with a `plugin.toml`. | Show the folder. | "Asylum is extensible. A plugin is just a directory with a plugin-dot-toml manifest — contributing palette commands, a panel, a webview, event triggers, and tools for the agents." |
| 0:24 | Path overlay: plugins dir. | Show `$XDG_DATA_HOME/asylum/plugins/<id>/`. | "Plugins live under your data directory, in asylum slash plugins slash the plugin id. On startup Asylum scans that directory, loads the good ones, and reports a diagnostic for each bad manifest — one broken plugin never blocks the others." |
| 0:52 | An annotated `plugin.toml`. | Scroll the manifest. | "Here's a manifest. Up top: identity — id, name, version, description — and capabilities, which we'll come back to." |
| 1:16 | `[[command]]` block. | Highlight command. | "The extension points. Double-bracket command adds a command-palette action. Its mode decides what running it does — invoke calls a runtime method, panel opens the plugin's panel, webview opens its web surface — and it can carry a keybind." |
| 1:42 | `[panel]` and `[webview]` blocks. | Show both. | "panel is a side-drawer rendered from the runtime's responses. webview is a native web surface placed as a panel, tab, or window — sourced from a url, a bundled entry, or a service." |
| 2:08 | `[[trigger]]` block. | Highlight trigger. | "Double-bracket trigger hooks an ADE event. Its action is notify — post a desktop notification — or invoke a runtime method, and it can be conditioned with a when clause." |
| 2:34 | Trigger events list. | Reveal the eight events. | "The events you can hook: task_created, run_started, run_finished, run_failed, worktree_created, worktree_removed, diff_ready, and task_merged." |
| 3:00 | `[[tool]]` block with typed params. | Highlight tool. | "And double-bracket tool exposes a tool to the coding agents themselves, with typed parameters — so an agent can call your plugin's functionality directly." |
| 3:24 | `capabilities = [...]` line. | Zoom capabilities. | "Now capabilities. A plugin declares what it's allowed to touch — git, agents, store, network, filesystem, clipboard, notify. Under the process runtime these are advisory. Under WASM they're the enforced gate." |
| 3:52 | Split: process vs WASM runtimes. | Show both. | "There are two runtimes. The process runtime is a normal program the app talks to over newline-delimited JSON on stdin and stdout — request in, result or error out. Non-JSON lines, like stray logging, are ignored, so your runtime can print debug freely. One-shot, or persistent to stay warm across calls." |
| 4:24 | WASM runtime callout. | Show the sandbox note. | "The WASM runtime runs a WebAssembly module in a sandbox. The crucial property: the host links only the host functions your declared capabilities allow — so a guest that never asked for notify literally cannot import the notify function. That makes WASM the right tier for untrusted or shared plugins." |
| 4:56 | Terminal. | `asylum plugin install acme/asylum-linear`. | "Installing is a shallow clone into the plugins directory. asylum plugin install, owner slash repo — optionally at a branch, tag, or commit. It refuses if the destination exists or the repo has no manifest, so you can't accidentally install a non-plugin." |
| 5:26 | Terminal. | `asylum plugin list` then `asylum plugin search --limit 10`. | "list shows what's installed and reports broken manifests. search discovers community plugins tagged with the GitHub topic asylum-plugin — via gh, so authenticate it first. Tag your own repo with that topic to make it discoverable." |
| 5:56 | A minimal `hello` plugin manifest + `runtime.ts`. | Show the two files. | "Let's build one. A minimal process plugin is a manifest and a runtime script. The manifest declares an id, the notify capability, a process runtime running the script, and one greet command." |
| 6:24 | The `runtime.ts` reading newline-JSON. | Scroll the Bun script. | "The runtime reads one JSON request per line and answers each — returning a result for greet, an error otherwise, and skipping anything that isn't JSON. Bun and TypeScript are a natural fit here." |
| 6:48 | Command palette shows "Say Hello". | Run the command. | "Drop the directory under the plugins path, and 'Say Hello' appears in the command palette. From there you grow it — a panel, a trigger on run_finished, a tool the agents can call." |

## B-roll / capture notes
- Have a real `hello` plugin directory ready (manifest + `runtime.ts`) so you can show it appearing in `asylum plugin list` and the palette.
- For `asylum plugin search`, authenticate `gh`; if no community results appear, show the "no plugins found" message honestly.
- Keep the manifest on screen long enough to read each block; consider highlighting one block at a time in post.
- The `acme/asylum-linear` install target is illustrative — use a real plugin repo if you have one, or show the refusal when a repo lacks a manifest.

## Recap card (end screen)
- A plugin is a directory with a `plugin.toml` under the plugins path; bad manifests are reported, not fatal.
- The manifest contributes commands, a panel, a webview, triggers, and tools, and declares capabilities.
- Process runtime speaks newline-JSON over stdio (one-shot or warm); WASM is sandboxed and enforces capabilities by linking only allowed host functions.
- Install with `asylum plugin install owner/repo[@ref]`; discover by the `asylum-plugin` GitHub topic.

## Next
- [Episode 15 — Expert Workflows](15-expert-workflows.md)

Go deeper: [book chapter 13](../book/13-plugins.md).
