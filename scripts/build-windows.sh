#!/bin/bash

# Voice Flow Windows build script
# Checks and configures the signing key before building.

set -e

echo "Voice Flow Windows build"
echo "============================"
echo ""

# Check the private key.
PRIVATE_KEY_PATH=~/.tauri/voiceflow.key
if [ ! -f "$PRIVATE_KEY_PATH" ]; then
    echo "Error: private key not found"
    echo "Path: $PRIVATE_KEY_PATH"
    echo ""
    echo "Generate a key pair first:"
    echo "  pnpm tauri signer generate -w ~/.tauri/voiceflow.key"
    exit 1
fi

echo "Private key found: $PRIVATE_KEY_PATH"

# Export the private-key path.
export TAURI_SIGNING_PRIVATE_KEY="$PRIVATE_KEY_PATH"

# Request a password only when the environment does not provide one.
if [ -z "$TAURI_SIGNING_PRIVATE_KEY_PASSWORD" ]; then
    echo ""
    echo "TAURI_SIGNING_PRIVATE_KEY_PASSWORD is not set"
    echo ""
    read -s -p "Private-key password (press Enter if none): " PASSWORD
    echo ""
    
    if [ -n "$PASSWORD" ]; then
        export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$PASSWORD"
        echo "Password configured"
    else
        echo "Continuing without a private-key password"
    fi
fi

echo ""
echo "Starting build..."
echo ""

# Run the Windows build.
pnpm run tauri:build:win

echo ""
echo "Build complete"
echo ""
echo "Build artifacts:"
echo "   - NSIS: src-tauri/target/release/bundle/nsis/Voice Flow-setup.exe"
echo "   - MSI: src-tauri/target/release/bundle/msi/Voice Flow.msi"
echo "   - Signature: src-tauri/target/release/bundle/nsis/Voice Flow-setup.exe.sig"
echo "   - Signature: src-tauri/target/release/bundle/msi/Voice Flow.msi.sig"
