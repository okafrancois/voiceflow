#!/bin/zsh
# Build signé Developer ID + notarisation + DMG.
#
# Prérequis (une fois) :
#   1. Compte Apple Developer, certificat « Developer ID Application »
#      installé dans le trousseau.
#   2. Mot de passe d'app créé sur appleid.apple.com, puis :
#        xcrun notarytool store-credentials voiceflow-notary \
#          --apple-id "<votre identifiant Apple>" \
#          --team-id "<votre Team ID>" \
#          --password "<mot de passe d'app>"
#
# Usage : DEV_ID="Developer ID Application: Nom (TEAMID)" ./release.sh
set -euo pipefail
cd "$(dirname "$0")"

# Identité de signature : celle du trousseau, sauf si DEV_ID la précise.
DEV_ID="${DEV_ID:-$(security find-identity -v -p codesigning 2>/dev/null \
	| grep "Developer ID Application" | head -1 | awk '{print $2}')}"
if [[ -z "$DEV_ID" ]]; then
	echo "Aucun certificat « Developer ID Application » dans le trousseau." >&2
	exit 1
fi
echo "Signature : $DEV_ID"

NOTARY_PROFILE="${NOTARY_PROFILE:-voiceflow-notary}"

./build.sh release

APP="dist/VoiceFlow.app"
DMG="dist/VoiceFlow.dmg"

# Le durcissement d'exécution est exigé pour la notarisation ; seule
# l'entrée audio est déclarée (l'app n'envoie aucun Apple Event).
codesign --force --deep --options runtime --timestamp \
	--entitlements voiceflow.entitlements \
	--sign "$DEV_ID" "$APP"
codesign --verify --strict --verbose=2 "$APP"

rm -f "$DMG"
hdiutil create -volname "VoiceFlow" -srcfolder "$APP" -ov -format UDZO "$DMG"

# La notarisation n'est nécessaire que pour distribuer l'app à d'autres.
# Pour un usage local, la signature suffit.
if xcrun notarytool history --keychain-profile "$NOTARY_PROFILE" >/dev/null 2>&1; then
	xcrun notarytool submit "$DMG" --keychain-profile "$NOTARY_PROFILE" --wait
	xcrun stapler staple "$DMG"
	xcrun stapler validate "$DMG"
	echo "→ $DMG signé, notarisé et agrafé."
else
	cat <<'MSG'
→ DMG signé, mais non notarisé : aucun profil notarytool enregistré.

Pour un usage sur cette machine, c'est suffisant.
Pour distribuer l'app, enregistrez une fois vos identifiants App Store Connect
(les mêmes que ceux du workflow GitHub) :

  xcrun notarytool store-credentials voiceflow-notary \
    --key /chemin/vers/AuthKey_<KEYID>.p8 \
    --key-id <APPLE_API_KEY> \
    --issuer <APPLE_API_ISSUER>

puis relancez ./release.sh
MSG
fi
