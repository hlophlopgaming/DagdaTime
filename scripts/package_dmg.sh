#!/bin/bash
# Build a macOS DMG containing "Draugr Time.app" and a link to /Applications.
# Requires the .app bundle to already exist (run scripts/package_macos.py first).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP="$ROOT/target/release/Draugr Time.app"
DMG="$ROOT/target/release/Draugr-Time-macOS.dmg"

[ -d "$APP" ] || { echo "$APP not found; run scripts/package_macos.py first" >&2; exit 1; }

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"

hdiutil create -volname "Draugr Time" -srcfolder "$STAGE" -ov -format UDZO "$DMG" >/dev/null
echo "$DMG"
