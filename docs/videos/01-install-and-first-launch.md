# Episode 01 — Install and First Launch

**Duration:** ~4 min · **Level:** Beginner
**You'll learn:** how to build and run Asylum from source, the difference between the dev and released binaries, and a tour of the window.
**Prerequisites:** A Rust toolchain (`cargo`) and `git` installed.

## Shot list

| Time | On screen | Action | Narration |
|------|-----------|--------|-----------|
| 0:00 | Terminal at the repo root. | Show `ls` of the workspace crates. | "Asylum is a native Rust app. To run it you need a Rust toolchain and git. That's it for the core." |
| 0:16 | Terminal. | Type `cargo run -p app` and press enter. | "From the repository root, one command builds and launches the app: cargo run, dash p, app." |
| 0:30 | Cargo compiling output scrolling. | Let it build; speed up in post. | "The first build pulls the UI component library and the terminal engine, so it takes a while. Later launches are fast." |
| 0:48 | Callout overlay: `asylumdev` vs `asylum`. | Freeze on the built binary name. | "The dev build installs itself as asylumdev — deliberately, so it never collides with a released copy installed as asylum. Same behavior either way." |
| 1:06 | The Asylum window finishes launching. | App appears. | "And here's the app. Let's take the tour." |
| 1:18 | Header bar with palette and quick-open affordances. | Point at the header. | "Up top is the header, with the command palette and quick-open for jumping around fast." |
| 1:32 | Left activity switcher + navbar. | Hover the switcher and project/task list. | "On the left, the activity switcher and the project and task navbar — where your projects and their tasks live, with pins for the ones you use most." |
| 1:50 | Main area showing the Tasks surface. | Gesture at the central pane. | "The big area in the middle is the routed main view. Right now it's on Tasks — the fan-out board, the home screen." |
| 2:08 | Slow pan across surface tabs/switcher. | Cycle a couple of surfaces. | "Asylum routes between thirteen surfaces — Tasks, Diff, Search, Notes, Integrations, Terminal, Editor, Preview, Browser, Plugins, Accounts, Inbox, and Settings. We'll visit each across the series." |
| 2:34 | Status footer at the bottom. | Point at the footer. | "The footer is your status line — what's happening at a glance." |
| 2:50 | Terminal beside the app. | Run `cargo build`, then `cargo test`. | "Two more commands worth knowing: cargo build compiles the whole workspace, and cargo test runs the suite." |
| 3:12 | Settings pane opening. | Press `cmd-,`. | "And command-comma opens Settings — an editor over your settings file. We'll live there next episode." |
| 3:30 | App idle on Tasks. | Hold. | "That's the build and the tour. Next we open a real project and pick which agents to race." |
| 3:48 | End card. | Fade. | "See you in episode two." |

## B-roll / capture notes
- Pre-warm the build once so the on-camera `cargo run -p app` compiles quickly; keep a sped-up version of the first cold build for the "takes a while" beat.
- Have a terminal side-by-side with the app window for the cargo commands.
- Don't open a project yet — that's episode 02. Just land on the empty/default Tasks board.
- Keep the surface names legible when panning the switcher.

## Recap card (end screen)
- Build and run with `cargo run -p app`; the dev binary is `asylumdev`, the release is `asylum`.
- The window: header (palette + quick-open), left activity switcher + project/task navbar, routed main area, status footer.
- Thirteen surfaces; Tasks is home.
- `cmd-,` opens Settings.

## Next
- [Episode 02 — Open a Project and Pick Agents](02-open-a-project-and-pick-agents.md)

Go deeper: [book chapter 2](../book/02-installation-and-setup.md).
