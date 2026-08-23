#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
ASSET_DIR="$ROOT_DIR/apps/desktop/assets"
ICON_DIR="$ASSET_DIR/icons"
INHOUSE_ICON_DIR="$ICON_DIR/inhouse"
TAURI_ASSET_DIR="$ROOT_DIR/apps/desktop/src-tauri/assets"
TAURI_CLI="$ROOT_DIR/apps/desktop/node_modules/.bin/tauri"
MAIN_SOURCE="$ASSET_DIR/voice-flow-logo.svg"
DEV_SOURCE="$ASSET_DIR/voice-flow-logo-dev.svg"
TRAY_SOURCE="$ASSET_DIR/voice-flow-tray.svg"
TRAY_DEV_SOURCE="$ASSET_DIR/voice-flow-tray-dev.svg"
TASK_TMP_DIR="$(mktemp -d /tmp/voice-flow-assets.XXXXXX)"
DEV_ICON_DIR="$TASK_TMP_DIR/dev-icons"

cleanup() {
  rm -rf "$TASK_TMP_DIR"
}
trap cleanup EXIT

if ! command -v magick >/dev/null 2>&1; then
  echo "Missing required command: magick" >&2
  exit 1
fi

if [[ ! -x "$TAURI_CLI" ]]; then
  echo "Missing Tauri CLI. Run pnpm install first." >&2
  exit 1
fi

mkdir -p "$ICON_DIR" "$INHOUSE_ICON_DIR" "$TAURI_ASSET_DIR" "$DEV_ICON_DIR"

"$TAURI_CLI" icon "$MAIN_SOURCE" --output "$ICON_DIR"
"$TAURI_CLI" icon "$DEV_SOURCE" --output "$DEV_ICON_DIR"

magick -background none "$MAIN_SOURCE" -resize 1024x1024 "PNG32:$ASSET_DIR/logo.png"
magick -background none "$DEV_SOURCE" -resize 1024x1024 "PNG32:$INHOUSE_ICON_DIR/icon-1024.png"

for filename in 32x32.png 64x64.png 128x128.png 128x128@2x.png icon.png icon.icns icon.ico; do
  cp "$DEV_ICON_DIR/$filename" "$INHOUSE_ICON_DIR/$filename"
done

magick -background none "$TRAY_SOURCE" -resize 48x48 "PNG32:$TAURI_ASSET_DIR/tray-icon.png"
magick -background none "$TRAY_DEV_SOURCE" -resize 48x48 "PNG32:$TAURI_ASSET_DIR/tray-icon-inhouse.png"

echo "Voice Flow brand assets generated."
