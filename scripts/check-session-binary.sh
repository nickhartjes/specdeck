#!/usr/bin/env bash
# Fails when an installer in the Tauri bundle directory lacks the session binary, which hosted
# sessions run as (session-processes design decision 1; scripts/session-binary.mjs adds it).
# Usage: check-session-binary.sh [bundle dir]
set -euo pipefail
bundle_dir=${1:-target/release/bundle}
status=0
found=0

check() {
  found=1
  local listing
  # Listed in full first: grep -q stopping early would fail the lister under pipefail.
  listing=$("${@:3}")
  if grep -q "$2" <<< "$listing"; then
    echo "ok       $1"
  else
    echo "missing  $1: no $2" >&2
    status=1
  fi
}

shopt -s nullglob
for app in "$bundle_dir"/macos/*.app; do
  check "$app" specdeck-session ls "$app/Contents/MacOS"
done
for deb in "$bundle_dir"/deb/*.deb; do
  check "$deb" './usr/bin/specdeck-session$' dpkg-deb --contents "$deb"
done
for image in "$bundle_dir"/appimage/*.AppImage; do
  # Extracting needs no FUSE; into a temporary folder, as it writes squashfs-root/.
  path="$(cd "$(dirname "$image")" && pwd)/$(basename "$image")"
  extract=$(mktemp -d)
  (cd "$extract" && "$path" --appimage-extract usr/bin/specdeck-session > /dev/null)
  check "$image" specdeck-session ls "$extract/squashfs-root/usr/bin"
  rm -rf "$extract"
done
for installer in "$bundle_dir"/nsis/*.exe; do
  check "$installer" 'specdeck-session\.exe' 7z l "$installer"
done

if [ "$found" -eq 0 ]; then
  echo "no installers found in $bundle_dir" >&2
  exit 1
fi
exit "$status"
