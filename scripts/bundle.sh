#!/usr/bin/env bash
# Build Asylum (release) and assemble dist/Asylum.app. The binary is the
# `asylumdev` bin from crates/app, shipped as `asylum`; the icon comes from
# assets/icon.icns; the version is read from the workspace Cargo.toml.
# Codesigns with CODESIGN_IDENTITY if set, otherwise ad-hoc ("-") so it still
# runs locally. Usage: scripts/bundle.sh
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

app_name="Asylum"
src_bin="asylumdev"
bin_name="asylum"
bundle_id="io.wess.asylum"
identity="${CODESIGN_IDENTITY:--}"

version="$(sed -n 's/^version = "\([0-9][^"]*\)".*/\1/p' Cargo.toml | head -1)"
[ -n "$version" ] || { echo "error: could not read version from Cargo.toml" >&2; exit 1; }
echo "[bundle] $app_name $version"

if [ ! -f assets/icon.icns ]; then
  echo "[bundle] assets/icon.icns missing — generating"
  scripts/icon.sh
fi

echo "[bundle] cargo build --release -p app"
cargo build --release -p app

app="dist/$app_name.app"
contents="$app/Contents"
rm -rf "$app"
mkdir -p "$contents/MacOS" "$contents/Resources"
cp "target/release/$src_bin" "$contents/MacOS/$bin_name"
cp assets/icon.icns "$contents/Resources/icon.icns"

cat > "$contents/Info.plist" << PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleName</key>
	<string>$app_name</string>
	<key>CFBundleDisplayName</key>
	<string>$app_name</string>
	<key>CFBundleIdentifier</key>
	<string>$bundle_id</string>
	<key>CFBundleExecutable</key>
	<string>$bin_name</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleVersion</key>
	<string>$version</string>
	<key>CFBundleShortVersionString</key>
	<string>$version</string>
	<key>CFBundleIconFile</key>
	<string>icon</string>
	<key>CFBundleURLTypes</key>
	<array>
		<dict>
			<key>CFBundleURLName</key>
			<string>$bundle_id</string>
			<key>CFBundleURLSchemes</key>
			<array><string>asylum</string></array>
		</dict>
	</array>
	<key>LSApplicationCategoryType</key>
	<string>public.app-category.productivity</string>
	<key>LSMinimumSystemVersion</key>
	<string>11.0</string>
	<key>NSHighResolutionCapable</key>
	<true/>
	<key>NSMicrophoneUsageDescription</key>
	<string>Asylum uses the microphone for dictation and voice chats with your Bots.</string>
</dict>
</plist>
PLIST

echo "[bundle] codesign ($identity)"
runtime_opts=()
[ "$identity" != "-" ] && runtime_opts=(--options runtime --timestamp)
codesign --force ${runtime_opts[@]+"${runtime_opts[@]}"} \
  --entitlements assets/asylum.entitlements \
  -s "$identity" "$contents/MacOS/$bin_name"
codesign --force ${runtime_opts[@]+"${runtime_opts[@]}"} \
  --entitlements assets/asylum.entitlements \
  -s "$identity" "$app"
codesign --verify --strict --verbose=2 "$app" || true
echo "[bundle] -> $app"
