#!/usr/bin/env bash
# Pull this checkout and reinstall Presence.
set -euo pipefail

root="$(cd "$(dirname "$0")" && pwd)"
cd "$root"

git pull
cargo install --path . --root "${HOME}/.local" --force

alfred_root="${HOME}/Library/Application Support/Alfred"
if [[ -d "$alfred_root" ]]; then
  ./alfred/install.sh
fi

echo "Updated Presence. Restart it if it is already running."
