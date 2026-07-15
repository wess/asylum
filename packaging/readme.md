# Packaging & distribution

Asylum ships as a native desktop app. The release pipeline turns a tagged commit
into installable artifacts so people who are not Rust developers can run it
without a toolchain.

## What ships

| Platform | Artifact | How it's built |
|---|---|---|
| macOS (Apple Silicon) | `asylum-macos-arm64.dmg` | `cargo bundle` → `.app` → `packaging/macos.sh` → signed/notarized `.dmg` |
| macOS (Intel) | `asylum-macos-x64.dmg` | same, `x86_64-apple-darwin` |
| Linux (x64) | `asylum-linux-x64.deb` | `cargo bundle --format deb` |

Homebrew and the Arch AUR wrap the same `.dmg`/binary once a release exists.

## Cutting a release

1. Bump `version` in the root `Cargo.toml` (`[workspace.package]`).
2. Tag it: `git tag v0.2.0 && git push --tags`.
3. The `release` workflow (`.github/workflows/release.yml`) builds every target,
   packages, and publishes a GitHub Release with the artifacts attached.

`workflow_dispatch` can also build an arbitrary tag on demand.

## Local build

```sh
cargo install cargo-bundle --locked
cd crates/app
cargo bundle --release --format osx     # macOS → target/<t>/release/bundle/osx/Asylum.app
cargo bundle --release --format deb      # Linux → …/bundle/deb/*.deb
```

The bundled binary installs as `asylum`; local `cargo run -p app` stays
`asylumdev` so a dev build never collides with an installed release.

## Signing & notarization (macOS)

`packaging/macos.sh` signs and notarizes only when the relevant secrets are set,
so unsigned local and fork builds still succeed. Configure these repository
secrets to produce distributable macOS builds:

- `MACOS_CERT_P12` — base64 of the Developer ID Application `.p12`
- `MACOS_CERT_PASSWORD` — its password
- `MACOS_SIGN_IDENTITY` — e.g. `Developer ID Application: Name (TEAMID)`
- `AC_USERNAME`, `AC_PASSWORD`, `AC_TEAM_ID` — notarization credentials

## Auto-update

The app checks GitHub Releases on launch (`update` crate) and, when a newer
version is published, drops an Inbox notification linking to the download. In-app
binary replacement is intentionally left to the platform package (Homebrew, the
`.deb`, or a re-download) rather than a bespoke updater.

## Still to add

- An app icon (`.icns` / PNG set) referenced from `[package.metadata.bundle]`.
- A Homebrew cask formula and AUR `PKGBUILD` pointing at published `.dmg`s.
