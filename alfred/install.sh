#!/usr/bin/env bash
# Copy the Start Focus workflow into Alfred's preferences.
set -euo pipefail

src="$(cd "$(dirname "$0")" && pwd)"
alfred_root="${HOME}/Library/Application Support/Alfred"
dest="${alfred_root}/Alfred.alfredpreferences/workflows/user.workflow.start-focus"

if [[ ! -d "$alfred_root" ]]; then
  echo "install: Alfred does not look installed at $alfred_root" >&2
  exit 1
fi

mkdir -p "$dest"
cp "$src/info.plist" "$src/start-focus.sh" "$dest/"
chmod +x "$dest/start-focus.sh"
echo "Installed Start Focus. Type presence in Alfred."
