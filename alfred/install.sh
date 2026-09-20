#!/usr/bin/env bash
# Copy the Presence workflow into Alfred's preferences.
set -euo pipefail

src="$(cd "$(dirname "$0")" && pwd)"
alfred_root="${HOME}/Library/Application Support/Alfred"
dest="${alfred_root}/Alfred.alfredpreferences/workflows/user.workflow.start-presence"
old_dest="${alfred_root}/Alfred.alfredpreferences/workflows/user.workflow.start-focus"

if [[ ! -d "$alfred_root" ]]; then
  echo "install: Alfred does not look installed at $alfred_root" >&2
  exit 1
fi

mkdir -p "$dest"
cp "$src/info.plist" "$src/start-presence.sh" "$dest/"
chmod +x "$dest/start-presence.sh"
if [[ -d "$old_dest" ]]; then
  rm -rf "$old_dest"
fi
echo "Installed Presence. Type presence in Alfred."
