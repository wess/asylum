#!/usr/bin/env bash
# Regenerate the app icons from assets/icon.svg: the 1024 and 512 master PNGs and
# the macOS .icns. Run after editing the SVG. Needs rsvg-convert (SVG raster) and,
# for the .icns, iconutil (macOS). Outputs land in assets/.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
assets="$root/assets"
svg="$assets/icon.svg"
[ -f "$svg" ] || { echo "error: $svg not found" >&2; exit 1; }

command -v rsvg-convert >/dev/null 2>&1 || {
  echo "error: rsvg-convert not found (brew install librsvg / apt install librsvg2-bin)" >&2
  exit 1
}

echo "[icon] rendering master PNGs"
rsvg-convert -w 1024 -h 1024 "$svg" -o "$assets/icon.png"
rsvg-convert -w 512 -h 512 "$svg" -o "$assets/icon512.png"

if command -v iconutil >/dev/null 2>&1; then
  echo "[icon] compiling icns"
  set="$(mktemp -d)/icon.iconset"
  mkdir -p "$set"
  for spec in 16:16x16 32:16x16@2x 32:32x32 64:32x32@2x \
              128:128x128 256:128x128@2x 256:256x256 512:256x256@2x \
              512:512x512 1024:512x512@2x; do
    px="${spec%%:*}"
    name="${spec#*:}"
    rsvg-convert -w "$px" -h "$px" "$svg" -o "$set/icon_${name}.png"
  done
  iconutil -c icns "$set" -o "$assets/icon.icns"
  rm -rf "$(dirname "$set")"
  echo "[icon] wrote icon.png, icon512.png, icon.icns"
else
  echo "[icon] iconutil unavailable (non-macOS) — wrote icon.png and icon512.png only"
fi
