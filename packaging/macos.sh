#!/usr/bin/env bash
# Package Asylum.app into a distributable .dmg, signing and notarizing when the
# signing secrets are available. Safe to run unsigned locally.
#
#   packaging/macos.sh <rust-target> <label>
#   packaging/macos.sh aarch64-apple-darwin macos-arm64
set -euo pipefail

target="${1:?usage: macos.sh <rust-target> <label>}"
label="${2:?usage: macos.sh <rust-target> <label>}"

app="target/${target}/release/bundle/osx/Asylum.app"
[ -d "$app" ] || { echo "missing bundle: $app" >&2; exit 1; }

mkdir -p dist
dmg="dist/asylum-${label}.dmg"

# Codesign only when an identity is configured. Import the cert from a base64
# .p12 into a throwaway keychain first (CI), otherwise use the login keychain.
if [ -n "${MACOS_SIGN_IDENTITY:-}" ]; then
  if [ -n "${MACOS_CERT_P12:-}" ]; then
    keychain="build.keychain"
    password="${MACOS_CERT_PASSWORD:-actionsci}"
    echo "$MACOS_CERT_P12" | base64 --decode > cert.p12
    security create-keychain -p "$password" "$keychain"
    security default-keychain -s "$keychain"
    security unlock-keychain -p "$password" "$keychain"
    security import cert.p12 -k "$keychain" -P "$password" -T /usr/bin/codesign
    security set-key-partition-list -S apple-tool:,apple: -s -k "$password" "$keychain"
    rm -f cert.p12
  fi
  echo "signing $app"
  codesign --deep --force --options runtime \
    --sign "$MACOS_SIGN_IDENTITY" "$app"
else
  echo "no signing identity set; producing an unsigned bundle"
fi

# Build a compressed dmg from a staging folder holding the app + /Applications.
staging="$(mktemp -d)"
cp -R "$app" "$staging/"
ln -s /Applications "$staging/Applications"
hdiutil create -volname "Asylum" -srcfolder "$staging" -ov -format UDZO "$dmg"
rm -rf "$staging"

# Notarize when credentials are present (Apple ID + app-specific password).
if [ -n "${MACOS_SIGN_IDENTITY:-}" ] && [ -n "${AC_USERNAME:-}" ] && [ -n "${AC_PASSWORD:-}" ]; then
  echo "notarizing $dmg"
  xcrun notarytool submit "$dmg" \
    --apple-id "$AC_USERNAME" --password "$AC_PASSWORD" \
    --team-id "${AC_TEAM_ID:?set AC_TEAM_ID to notarize}" --wait
  xcrun stapler staple "$dmg"
fi

echo "built $dmg"
