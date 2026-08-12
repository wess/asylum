# Performance and resource baseline

Measured numbers, so a regression is something you can *see* rather than
suspect. Re-measure with the commands below and update the table when a change
moves one materially.

Last measured **2026-08-12**, release profile, macOS arm64 (Darwin 25.5).

## Artifacts

| | Size |
|---|---|
| `asylum` (CLI) | **1.6 MB** |
| `asylumdev` / `asylum` (GUI) | **13.3 MB** |

The GUI binary carries the whole environment — GPU terminal emulation, editor,
embedded browser host, agent orchestration, SQLite, and a WASM plugin runtime.
For comparison, the Electron-based tools in this category ship 100–200 MB before
their own code. Stripping the release binary changes nothing because
`[profile.release]` already sets `strip = "symbols"` alongside `lto = "fat"` and
`codegen-units = 1`.

## Startup and memory

| | Measured |
|---|---|
| `asylum --version` | **~39 ms** per invocation (10 runs, includes shell spawn) |
| `asylum` peak RSS | **5.8 MB** |
| Linked dylibs (GUI) | 18 |
| Workspace crates / total dependencies | 25 / 898 |

**Not measured here:** GUI window startup, idle RSS with panes open, and memory
with several terminals plus a browser pane. Those need a real windowed session
and a sampling profiler; do not guess them from the numbers above. The embedded
browser is a native `wry` web view, so its memory belongs to the platform web
process rather than to `asylumdev`'s RSS — measure both, or the total will look
better than it is.

## Reproducing

```sh
cargo build --release --workspace
ls -l target/release/asylum target/release/asylumdev

# startup
time (for i in $(seq 10); do ./target/release/asylum --version >/dev/null; done)

# peak resident set
/usr/bin/time -l ./target/release/asylum --version 2>&1 | grep "maximum resident"
```

## Why `panic = "unwind"` stays

Deliberate, and load-bearing rather than an oversight: the companion, control,
proxy and MCP servers plus the update check all run as detached background
threads, and `abort` would take the whole app down with any one of them instead
of containing the panic to its thread. The crash hook and the non-blocking log
writer also need the process alive past a panic to flush to disk. See
`[profile.release]` in the root `Cargo.toml`.
