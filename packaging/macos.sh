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

# cargo-bundle names the executable after the cargo bin target, which is
# `asylumdev` so a dev build never collides with an installed `asylum`. Ship it
# under the real name, the way linux.sh and windows.ps1 already do - otherwise a
# release install reports itself as `asylumdev` in Activity Monitor and in crash
# reports. The rename has to happen here, before signing: the signature covers
# both the executable and the Info.plist that names it.
if [ -x "$app/Contents/MacOS/asylumdev" ]; then
  mv "$app/Contents/MacOS/asylumdev" "$app/Contents/MacOS/asylum"
  /usr/libexec/PlistBuddy -c "Set :CFBundleExecutable asylum" "$app/Contents/Info.plist"
  echo "renamed bundle executable to asylum"
fi
[ -x "$app/Contents/MacOS/asylum" ] || { echo "no asylum executable in $app" >&2; exit 1; }

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
    # codesign searches the keychain *search list*, not the default keychain -
    # setting only the default leaves the identity unfindable.
    security list-keychains -d user -s "$keychain" login.keychain
    security unlock-keychain -p "$password" "$keychain"
    # A new keychain relocks after 300s of idle by default, which lands in the
    # middle of the `notarytool --wait` below.
    security set-keychain-settings -lut 7200 "$keychain"
    security import cert.p12 -k "$keychain" -P "$password" -T /usr/bin/codesign
    security set-key-partition-list -S apple-tool:,apple: -s -k "$password" "$keychain"
    rm -f cert.p12
  fi
  echo "signing $app"
  # `--timestamp` is required: notarization rejects any signature without a
  # secure timestamp. `--deep` is not used - Apple discourages it for signing,
  # and it skips the nested code that needs signing first anyway.
  find "$app/Contents" -type f \( -perm -u+x -o -name '*.dylib' -o -name '*.so' \) \
    -exec codesign --force --timestamp --options runtime \
      --sign "$MACOS_SIGN_IDENTITY" {} + 2>/dev/null || true
  codesign --force --timestamp --options runtime \
    --sign "$MACOS_SIGN_IDENTITY" "$app"
  codesign --verify --strict --verbose=2 "$app"
else
  echo "no signing identity set; producing an unsigned bundle"
  echo "note: Gatekeeper will refuse this build; users must clear the quarantine attribute" >&2
fi

# Build a compressed dmg from a staging folder holding the app + /Applications.
staging="$(mktemp -d)"
cp -R "$app" "$staging/"
ln -s /Applications "$staging/Applications"
hdiutil create -volname "Asylum" -srcfolder "$staging" -ov -format UDZO "$dmg"
rm -rf "$staging"

# Sign the container too, so the thing users actually download carries a
# signature and can be stapled after notarization.
if [ -n "${MACOS_SIGN_IDENTITY:-}" ]; then
  codesign --force --timestamp --sign "$MACOS_SIGN_IDENTITY" "$dmg"
fi

# Notarize when credentials are present (Apple ID + app-specific password).
if [ -n "${MACOS_SIGN_IDENTITY:-}" ] && [ -n "${AC_USERNAME:-}" ] && [ -n "${AC_PASSWORD:-}" ]; then
  echo "notarizing $dmg"
  xcrun notarytool submit "$dmg" \
    --apple-id "$AC_USERNAME" --password "$AC_PASSWORD" \
    --team-id "${AC_TEAM_ID:?set AC_TEAM_ID to notarize}" --wait
  xcrun stapler staple "$dmg"
fi

echo "built $dmg"
