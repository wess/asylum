#!/usr/bin/env bash
# Package dist/Asylum.app into dist/Asylum.dmg — a compressed disk image with the
# app and a /Applications symlink for drag-to-install. Run scripts/bundle.sh
# first. Usage: scripts/dmg.sh
#
# Signing and notarization (both optional; without them the image is only
# usable on this Mac):
#   CODESIGN_IDENTITY  "Developer ID Application: Name (TEAMID)" — also used
#                      by scripts/bundle.sh for the app itself
#   NOTARY_PROFILE     a notarytool keychain profile, created once with
#                      xcrun notarytool store-credentials <profile> \
#                        --apple-id <id> --team-id <TEAMID> --password <app-password>
#   or APPLE_ID + APPLE_TEAM_ID + APPLE_APP_PASSWORD (app-specific password)
# With an identity and credentials, the image is submitted to Apple, waited
# on, and the ticket is stapled so Gatekeeper accepts it offline.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

app_name="Asylum"
app="dist/$app_name.app"
dmg="dist/$app_name.dmg"
[ -d "$app" ] || { echo "error: $app not found — run scripts/bundle.sh first" >&2; exit 1; }

stage="$(mktemp -d)"
cp -R "$app" "$stage/"
ln -s /Applications "$stage/Applications"

rm -f "$dmg"
echo "[dmg] building $dmg"
hdiutil create \
  -volname "$app_name" \
  -srcfolder "$stage" \
  -fs HFS+ \
  -format UDZO \
  -ov \
  "$dmg" >/dev/null
rm -rf "$stage"

identity="${CODESIGN_IDENTITY:--}"
if [ "$identity" = "-" ]; then
  echo "[dmg] -> $dmg (unsigned: set CODESIGN_IDENTITY to sign and notarize)"
  exit 0
fi

echo "[dmg] codesign"
codesign --force --timestamp -s "$identity" "$dmg"

# Credentials for notarytool, never echoed.
notary=()
if [ -n "${NOTARY_PROFILE:-}" ]; then
  notary=(--keychain-profile "$NOTARY_PROFILE")
elif [ -n "${APPLE_ID:-}" ] && [ -n "${APPLE_TEAM_ID:-}" ] && [ -n "${APPLE_APP_PASSWORD:-}" ]; then
  notary=(--apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_PASSWORD")
else
  echo "[dmg] -> $dmg (signed, not notarized: set NOTARY_PROFILE or APPLE_ID/APPLE_TEAM_ID/APPLE_APP_PASSWORD)"
  exit 0
fi

# The app inside must carry a Developer ID signature with the hardened
# runtime, or Apple rejects the submission.
if ! codesign -dv --verbose=2 "$app" 2>&1 | grep -q "flags=.*runtime"; then
  echo "error: $app isn't signed with the hardened runtime — run scripts/bundle.sh with CODESIGN_IDENTITY set" >&2
  exit 1
fi

echo "[dmg] notarizing (this usually takes a few minutes)"
out="$(xcrun notarytool submit "$dmg" "${notary[@]}" --wait --output-format json)"
status="$(printf '%s' "$out" | sed -n 's/.*"status" *: *"\([^"]*\)".*/\1/p')"
id="$(printf '%s' "$out" | sed -n 's/.*"id" *: *"\([^"]*\)".*/\1/p')"
if [ "$status" != "Accepted" ]; then
  echo "error: notarization ${status:-failed}${id:+ (submission $id)}" >&2
  [ -n "$id" ] && xcrun notarytool log "$id" "${notary[@]}" >&2 || true
  exit 1
fi

echo "[dmg] stapling"
xcrun stapler staple "$dmg"
xcrun stapler validate "$dmg"
spctl -a -t open --context context:primary-signature -v "$dmg"
echo "[dmg] -> $dmg (signed, notarized, stapled)"
