#!/usr/bin/env bash
set -euo pipefail

REPO="foodiemonster/SteamRollbackrLite"
TARBALL_URL="https://github.com/$REPO/releases/latest/download/steamrollbackr-lite-linux-x86_64.tar.gz"

if [ "${1:-}" = "--uninstall" ]; then
    rm -f "$HOME/.local/bin/steamrollbackr-lite" \
          "$HOME/.local/share/applications/steamrollbackr-lite.desktop" \
          "$HOME/.local/share/icons/hicolor/256x256/apps/steamrollbackr-lite.png"
    update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
    echo "Uninstalled SteamRollbackrLite."
    exit 0
fi

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

echo "Downloading latest SteamRollbackrLite release..."
curl -fsSL "$TARBALL_URL" -o "$TMP_DIR/steamrollbackr-lite.tar.gz"

echo "Extracting and installing..."
tar -xzf "$TMP_DIR/steamrollbackr-lite.tar.gz" -C "$TMP_DIR"
bash "$TMP_DIR/install.sh"

