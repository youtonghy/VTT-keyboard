#!/usr/bin/env bash
# Render src-tauri/dmg/background.svg into a Retina-aware TIFF (1x + 2x) for the DMG window.
# Requires rsvg-convert (brew install librsvg) and macOS tiffutil.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
dir="$root/src-tauri/dmg"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

rsvg-convert -w 660 -h 400 "$dir/background.svg" -o "$tmp/background.png"
rsvg-convert -w 1320 -h 800 "$dir/background.svg" -o "$tmp/background@2x.png"
tiffutil -cathidpicheck "$tmp/background.png" "$tmp/background@2x.png" -out "$dir/background.tiff"
echo "Wrote $dir/background.tiff"
