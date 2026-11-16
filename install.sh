#!/usr/bin/env bash
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_SRC="$DIR/steamrollbackr-lite"
[ -f "$BIN_SRC" ] || BIN_SRC="$DIR/target/release/steamrollbackr-lite"

ICON_SRC="$DIR/steamrollbackr-lite.png"
[ -f "$ICON_SRC" ] || ICON_SRC="$DIR/packaging/steamrollbackr-lite.png"

DESKTOP_SRC="$DIR/steamrollbackr-lite.desktop"
[ -f "$DESKTOP_SRC" ] || DESKTOP_SRC="$DIR/packaging/steamrollbackr-lite.desktop"

install -Dm755 "$BIN_SRC" "$HOME/.local/bin/steamrollbackr-lite"
install -Dm644 "$ICON_SRC" "$HOME/.local/share/icons/hicolor/256x256/apps/steamrollbackr-lite.png"
install -Dm644 "$DESKTOP_SRC" "$HOME/.local/share/applications/steamrollbackr-lite.desktop"

# Use full path in Exec= so GUI launchers work even if ~/.local/bin isn't in desktop PATH
sed -i "s|^Exec=.*|Exec=$HOME/.local/bin/steamrollbackr-lite|" "$HOME/.local/share/applications/steamrollbackr-lite.desktop"

update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" 2>/dev/null || true

echo "Installed SteamRollbackrLite! You can now launch it from your application menu."

