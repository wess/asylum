# gpui / guise-ui / libsinclair dependency recipe

The three UI dependencies — `gpui`, `guise-ui`, and `libsinclair` — must all
resolve to **one** `gpui`, or the build fails with duplicate-type errors deep
inside gpui.

## The single-gpui rule

- `gpui` comes from a **pinned git rev** (`96285fc1`), used directly by `app`
  (and transitively by `libsinclair`).
- `guise-ui` depends on `gpui` from **crates.io** (`^0.2.x`). A cargo
  `[patch.crates-io]` entry in the root `Cargo.toml` redirects that onto the same
  git rev, so `guise-ui` and the app compile against the identical gpui.
- Patches do **not** propagate through git dependencies, so the root workspace
  also carries the transitive patches: `async-process`, `async-task`, and a
  vendored `block` (in `thirdparty/block`).

```toml
[patch.crates-io]
gpui = { git = "https://github.com/zed-industries/zed", rev = "96285fc1" }
async-process = { git = "...", rev = "..." }
async-task = { git = "...", rev = "..." }
block = { path = "thirdparty/block" }
```

## Where guise-ui and libsinclair come from

Both are git dependencies:

```toml
[workspace.dependencies]
guise-ui    = { git = "https://github.com/wess/guise" }
libsinclair = { git = "https://github.com/wess/sinclair", default-features = false }
```

They pin the **same** gpui rev as the `[patch]` above (verify after any bump).
Pin a `rev`/`tag` on each to lock a version for reproducible builds.

## Bumping the gpui rev

1. Update the `rev` in both `[patch.crates-io]` and `[workspace.dependencies]`.
2. Re-check the source repo's `[patch.crates-io]` at that rev and match any
   changed `async-process` / `async-task` (or new) pins here.
3. Make sure `guise-ui`/`libsinclair` point at revs that use the same gpui rev.
4. Rebuild `app`; duplicate-gpui type errors mean a patch fell out of sync.

## The window backend

The app is constructed with `gpui_platform::application()` (not
`Application::new()`), and `gpui_platform` is a direct dependency with the
`font-kit`, `wayland`, and `x11` features — the platform backend gpui needs to
open a real window.
