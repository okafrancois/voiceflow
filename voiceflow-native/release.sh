#!/bin/zsh
# Developer ID signed build + notarization + DMG.
#
# Prerequisites (one time):
#   1. Apple Developer account, "Developer ID Application" certificate
#      installed in the keychain.
#   2. App password created at appleid.apple.com, then:
#        xcrun notarytool store-credentials voiceflow-notary \
#          --apple-id "<your Apple ID>" \
#          --team-id "<your Team ID>" \
#          --password "<app password>"
#
# Usage: DEV_ID="Developer ID Application: Name (TEAMID)" ./release.sh
set -euo pipefail
cd "$(dirname "$0")"

# Signing identity: the keychain's, unless DEV_ID specifies one.
DEV_ID="${DEV_ID:-$(security find-identity -v -p codesigning 2>/dev/null \
	| grep "Developer ID Application" | head -1 | awk '{print $2}')}"
if [[ -z "$DEV_ID" ]]; then
	echo "No \"Developer ID Application\" certificate in the keychain." >&2
	exit 1
fi
echo "Signature: $DEV_ID"

NOTARY_PROFILE="${NOTARY_PROFILE:-voiceflow-notary}"

./build.sh release

APP="dist/VoiceFlow.app"
DMG="dist/VoiceFlow.dmg"

# Hardened runtime is required for notarization; only audio input
# is declared (the app doesn't send any Apple Events).
codesign --force --deep --options runtime --timestamp \
	--entitlements voiceflow.entitlements \
	--sign "$DEV_ID" "$APP"
codesign --verify --strict --verbose=2 "$APP"

rm -f "$DMG"
hdiutil create -volname "VoiceFlow" -srcfolder "$APP" -ov -format UDZO "$DMG"

# Notarization is only needed to distribute the app to others.
# For local use, signing is enough.
if xcrun notarytool history --keychain-profile "$NOTARY_PROFILE" >/dev/null 2>&1; then
	xcrun notarytool submit "$DMG" --keychain-profile "$NOTARY_PROFILE" --wait
	xcrun stapler staple "$DMG"
	xcrun stapler validate "$DMG"
	echo "→ $DMG signed, notarized and stapled."
else
	cat <<'MSG'
→ DMG signed, but not notarized: no notarytool profile registered.

For use on this machine, that's enough.
To distribute the app, register your App Store Connect credentials once
(the same ones as the GitHub workflow):

  xcrun notarytool store-credentials voiceflow-notary \
    --key /path/to/AuthKey_<KEYID>.p8 \
    --key-id <APPLE_API_KEY> \
    --issuer <APPLE_API_ISSUER>

then run ./release.sh again
MSG
fi
