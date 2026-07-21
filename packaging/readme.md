# Packaging & distribution

Asylum ships as a native desktop app. The release pipeline turns a version bump
into installable artifacts so people who are not Rust developers can run it
without a toolchain.

## What ships

| Platform | Artifact(s) | How it's built |
|---|---|---|
| macOS (Apple Silicon) | `Asylum.dmg` | `cargo bundle` → `.app` → `packaging/macos.sh` → signed/notarized `.dmg` |
| Linux (x64 + arm64) | `asylum_<v>_<arch>.deb`, `asylum-<v>-linux-<arch>.tar.gz`, `Asylum-<v>-<arch>.AppImage` | `packaging/linux.sh <arch>` (cargo-deb + linuxdeploy) |
| Windows (x64) | `asylum-<v>-windows-x86_64.zip`, `.msi`, Chocolatey `.nupkg` | `packaging/windows.ps1` (WiX v4) |

The bundled binary installs as `asylum`; local `cargo run -p app` stays
`asylumdev`, so a dev build never collides with an installed release.

**Windows is beta**: the binaries compile and link on Windows via CI but have
not been runtime-tested on a real machine, and the installers are **unsigned** —
expect a SmartScreen "unknown publisher" prompt until an Authenticode
certificate is wired in. The `.zip` is the guaranteed deliverable; a WiX failure
is non-fatal.

## Cutting a release

The release is **version-driven** — no manual tagging:

1. Bump `version` in the root `Cargo.toml` (`[workspace.package]`).
2. Push to `main`.
3. `.github/workflows/release.yml` notices the version has no matching `vX.Y.Z`
   tag, creates the tag + a GitHub Release (auto-generated notes), builds every
   platform, uploads the artifacts, and refreshes the Homebrew cask and Scoop
   manifest.

Idempotent: pushing again without a version bump is a no-op (the tag already
exists). `workflow_dispatch` re-runs the same logic on demand.

## Package managers

- **Homebrew** (macOS, Apple Silicon): the workflow rewrites the cask in
  `wess/homebrew-packages` with the released `Asylum.dmg` URL + SHA-256. Needs a
  `HOMEBREW_TAP_TOKEN` repo secret with push access to the tap.
  `brew install --cask wess/packages/asylum`
- **Scoop** (Windows): `scoop/asylum.json`'s version, URL, `extract_dir`, and
  hash are placeholders (`0.0.0` / zeroed) in git and rewritten + committed per
  release.
  `scoop install https://raw.githubusercontent.com/wess/asylum/main/packaging/scoop/asylum.json`
- **Chocolatey** (Windows): `chocolatey/asylum.nuspec` +
  `tools/chocolateyinstall.ps1` are rewritten with the version + checksum and
  `choco pack`ed; the `.nupkg` is uploaded to the release. Pushing to the
  community feed (`choco push`) needs an API key and passes moderation — it is
  **not** automated; publish manually when ready.

## Local builds

```sh
# macOS: .app + signed/notarized .dmg
cargo install cargo-bundle --locked
cd crates/app && cargo bundle --release --format osx --target aarch64-apple-darwin && cd ../..
packaging/macos.sh aarch64-apple-darwin macos-arm64      # -> dist/asylum-macos-arm64.dmg

# Linux: .deb + .tar.gz + AppImage
packaging/linux.sh x86_64                                 # -> dist/linux/*

# Windows (PowerShell 7+)
pwsh packaging/windows.ps1 -Arch x86_64                   # -> dist/windows/*
```

The app icon lives in `assets/icon.svg`; regenerate the raster/`.icns` with
`packaging/icon.sh` (needs `rsvg-convert`, and `iconutil` on macOS for the
`.icns`).

## Signing & notarization (macOS)

`packaging/macos.sh` signs and notarizes only when the relevant secrets are set,
so unsigned local and fork builds still succeed. Configure these repository
secrets to produce distributable macOS builds (provisioning steps:
[`packaging/signing.md`](signing.md)):

- `MACOS_CERT_P12` — base64 of the Developer ID Application `.p12`
- `MACOS_CERT_PASSWORD` — its password
- `MACOS_SIGN_IDENTITY` — e.g. `Developer ID Application: Name (TEAMID)`
- `AC_USERNAME`, `AC_PASSWORD`, `AC_TEAM_ID` — notarization credentials

## Windows signing (deferred)

To sign the MSI, add an Authenticode certificate as a CI secret and a
`signtool sign` step after `wix build`. An EV certificate is what clears
SmartScreen reputation prompts.

## Auto-update

The app checks GitHub Releases on launch (`update` crate) and, when a newer
version is published, drops an Inbox notification linking to the download. In-app
binary replacement is intentionally left to the platform package (Homebrew, the
`.deb`, or a re-download) rather than a bespoke updater.
