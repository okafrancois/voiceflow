#!/bin/zsh
# Assemble VoiceFlow.app à partir du package SwiftPM.
# Usage : ./build.sh [release]   (debug par défaut)
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

# Signer avec le Developer ID si le certificat est présent : son empreinte
# est stable, donc macOS conserve les autorisations d'un build à l'autre.
# Sinon, signature ad hoc — mais il faudra réaccorder l'accessibilité à
# chaque recompilation.
IDENTITY="${DEV_ID:-$(security find-identity -v -p codesigning 2>/dev/null \
	| grep "Developer ID Application" | head -1 | awk '{print $2}')}"

if [[ -n "$IDENTITY" ]]; then
	codesign --force --options runtime --timestamp=none \
		--entitlements "$ROOT/voiceflow.entitlements" \
		--sign "$IDENTITY" "$APP"
	echo "signé : $IDENTITY"
else
	codesign --force --sign - "$APP"
	echo "signé ad hoc (autorisations à réaccorder après chaque build)"
fi

echo "→ $APP"
echo "Lancer : open $(cd .. && pwd)/dist/VoiceFlow.app"
