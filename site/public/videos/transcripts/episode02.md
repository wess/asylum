# Install and first launch

Beginner · Episode 2

## 1. Terminal at the repo root.

Asylum is a native Rust app. To run it you need a Rust toolchain and git. That's it for the core.

## 2. Terminal.

From the repository root, one command builds and launches the app: cargo run, dash p, app.

## 3. Cargo compiling output scrolling.

The first build pulls the UI component library and the terminal engine, so it takes a while. Later launches are fast.

## 4. Callout overlay: asylumdev vs asylum.

The dev build installs itself as asylumdev — deliberately, so it never collides with a released copy installed as asylum. Same behavior either way.

## 5. The Asylum window finishes launching.

And here's the app. Let's take the tour.

## 6. Header bar with palette and quick-open affordances.

Up top is the header, with the command palette and quick-open for jumping around fast.

## 7. Left activity switcher + navbar.

On the left, the activity switcher and the project and task navbar — where your projects and their tasks live, with pins for the ones you use most.

## 8. Main area showing the Tasks surface.

The big area in the middle is the routed main view. Right now it's on Tasks — the fan-out board, the home screen.

## 9. Slow pan across surface tabs/switcher.

Asylum routes between thirteen surfaces — Tasks, Diff, Search, Notes, Integrations, Terminal, Editor, Preview, Browser, Plugins, Accounts, Inbox, and Settings. We'll visit each across the series.

## 10. Status footer at the bottom.

The footer is your status line — what's happening at a glance.

## 11. Terminal beside the app.

Two more commands worth knowing: cargo build compiles the whole workspace, and cargo test runs the suite.

## 12. Settings pane opening.

And command-comma opens Settings — an editor over your settings file. We'll live there next episode.

## 13. App idle on Tasks.

That's the build and the tour. Next we open a real project and pick which agents to race.

## 14. End card.

See you in episode two.
