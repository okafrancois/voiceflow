#!/bin/zsh
# Assembles VoiceFlow.app from the SwiftPM package.
# Usage: ./build.sh [release]   (debug by default)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT/VoiceFlow"

CONFIG="${1:-debug}"
swift build -c "$CONFIG"

BIN=".build/$CONFIG/VoiceFlow"
APP="../dist/VoiceFlow.app"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"
cp "$BIN" "$APP/Contents/MacOS/VoiceFlow"
cp Info.plist "$APP/Contents/Info.plist"
mkdir -p "$APP/Contents/Resources"
cp AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"
cp Resources/*.wav "$APP/Contents/Resources/"
cp -R Resources/fr.lproj Resources/en.lproj "$APP/Contents/Resources/"

# Sign with the Developer ID if the certificate is present: its fingerprint
# is stable, so macOS keeps the permissions across builds.
# Otherwise, ad hoc signature — but accessibility will need to be
# re-granted after every rebuild.
IDENTITY="${DEV_ID:-$(security find-identity -v -p codesigning 2>/dev/null \
	| grep "Developer ID Application" | head -1 | awk '{print $2}')}"

if [[ -n "$IDENTITY" ]]; then
	codesign --force --options runtime --timestamp \
		--entitlements "$ROOT/voiceflow.entitlements" \
		--sign "$IDENTITY" "$APP"
	echo "signed: $IDENTITY"
else
	codesign --force --sign - "$APP"
	echo "signed ad hoc (permissions must be re-granted after each build)"
fi

echo "→ $APP"
echo "Run: open $(cd .. && pwd)/dist/VoiceFlow.app"
