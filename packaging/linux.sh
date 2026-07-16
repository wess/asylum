#!/usr/bin/env bash
# Build Asylum (release) for Linux and produce a .tar.gz, a .deb, and an
# AppImage under dist/linux. The binary is the `asylumdev` bin from crates/app,
# shipped as `asylum`; the version is read from the workspace Cargo.toml. Builds
# natively for the host architecture (no cross-compiling) — pass x86_64 or
# aarch64 only to label the artifacts and pick the right helper downloads.
#
# Requirements (install beforehand): a Rust toolchain, the gpui/webkit system
# deps (clang, libssl-dev, libwayland-dev, libxkbcommon-dev, libxcb1-dev,
# libgtk-3-dev, libwebkit2gtk-4.1-dev), curl, and file. cargo-deb is installed on
# demand if missing.
#
# Usage: packaging/linux.sh [x86_64|aarch64]
set -euo pipefail

arch="${1:-$(uname -m)}"
case "$arch" in
  x86_64 | amd64) arch="x86_64"; triple="x86_64-unknown-linux-gnu"; debarch="amd64" ;;
  aarch64 | arm64) arch="aarch64"; triple="aarch64-unknown-linux-gnu"; debarch="arm64" ;;
  *) echo "error: unsupported arch '$arch' (want x86_64 or aarch64)" >&2; exit 1 ;;
esac

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

version="$(sed -n 's/^version = "\([0-9][^"]*\)".*/\1/p' Cargo.toml | head -1)"
[ -n "$version" ] || { echo "error: could not read version from Cargo.toml" >&2; exit 1; }
echo "[linux] Asylum $version for $triple"

out="$root/dist/linux"
rm -rf "$out"
mkdir -p "$out"

# --- build ----------------------------------------------------------------
rustup target add "$triple" >/dev/null 2>&1 || true
cargo build --release -p app --target "$triple"
# The cargo bin target is `asylumdev`; it's installed as `asylum` below.
bin="target/$triple/release/asylumdev"
strip "$bin" 2>/dev/null || true

# --- staging tree (shared by tar.gz and the AppImage AppDir) ---------------
appdir="$out/AppDir"
mkdir -p "$appdir/usr/bin" "$appdir/usr/share/applications" "$appdir/usr/share/pixmaps"
cp "$bin" "$appdir/usr/bin/asylum"
cp assets/asylum.desktop "$appdir/usr/share/applications/asylum.desktop"
# 512px icon: linuxdeploy accepts standard sizes (<=512), not the 1024 master.
cp assets/icon512.png "$appdir/usr/share/pixmaps/asylum.png"

# --- .tar.gz ---------------------------------------------------------------
stem="asylum-$version-linux-$arch"
stage="$out/$stem"
mkdir -p "$stage"
cp -r "$appdir/usr" "$stage/usr"
cp LICENSE README.md "$stage/" 2>/dev/null || true
tar -C "$out" -czf "$out/$stem.tar.gz" "$stem"
rm -rf "$stage"
echo "[linux] -> $stem.tar.gz"

# --- .deb (cargo-deb) ------------------------------------------------------
command -v cargo-deb >/dev/null 2>&1 || cargo install cargo-deb --locked
cargo deb -p app --no-build --target "$triple" --output "$out/asylum_${version}_${debarch}.deb"
echo "[linux] -> asylum_${version}_${debarch}.deb"

# --- AppImage (linuxdeploy + appimagetool) ---------------------------------
# Runners often lack FUSE, so extract-and-run the helper AppImages.
export APPIMAGE_EXTRACT_AND_RUN=1
tools="$out/tools"
mkdir -p "$tools"
ld="$tools/linuxdeploy-$arch.AppImage"
ait="$tools/appimagetool-$arch.AppImage"
curl -fsSL -o "$ld" "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-$arch.AppImage"
curl -fsSL -o "$ait" "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-$arch.AppImage"
chmod +x "$ld" "$ait"
"$ld" --appdir "$appdir" \
  --executable "$appdir/usr/bin/asylum" \
  --desktop-file "$appdir/usr/share/applications/asylum.desktop" \
  --icon-file "$appdir/usr/share/pixmaps/asylum.png"
ARCH="$arch" "$ait" "$appdir" "$out/Asylum-$version-$arch.AppImage"
echo "[linux] -> Asylum-$version-$arch.AppImage"

# --- cleanup intermediates, leave only shippable artifacts -----------------
rm -rf "$appdir" "$tools"
echo "[linux] artifacts in dist/linux:"
ls -1 "$out"
