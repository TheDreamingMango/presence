#!/usr/bin/env bash
# Wrap a Presence binary in Presence.app and zip it for GitHub Releases.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
binary="${1:?usage: packaging/package.sh <presence-binary> [out-dir]}"
out="${2:-$root/dist}"
version="$(awk -F '"' '/^version = / { print $2; exit }' "$root/Cargo.toml")"

if [[ ! -f "$binary" ]]; then
  echo "package: binary not found: $binary" >&2
  exit 1
fi
if [[ -z "$version" ]]; then
  echo "package: could not read version from Cargo.toml" >&2
  exit 1
fi

app="$out/Presence.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS"

# `presence` and `Presence` are the same name on a case-insensitive disk.
cp "$binary" "$app/Contents/MacOS/presence-bin"
chmod +x "$app/Contents/MacOS/presence-bin"

cat > "$app/Contents/MacOS/Presence" << 'EOF'
#!/bin/bash
# Open the Presence binary beside this launcher in Terminal.
set -euo pipefail
root="$(cd "$(dirname "$0")" && pwd)"
bin="${root}/presence-bin"
escaped=${bin//\'/\'\\\'\'}
osascript <<APPLESCRIPT
tell application "Terminal"
  activate
  do script "exec '${escaped}'"
end tell
APPLESCRIPT
EOF
chmod +x "$app/Contents/MacOS/Presence"

sed "s/__VERSION__/${version}/g" "$root/packaging/Info.plist" > "$app/Contents/Info.plist"

# An open eye: come back and look at what is in front of you.
icon_src="$root/packaging/presence-icon.png"
if [[ -f "$icon_src" ]]; then
  iconset="$(mktemp -d)/Presence.iconset"
  mkdir -p "$iconset" "$app/Contents/Resources"
  sips -z 16 16 "$icon_src" --out "$iconset/icon_16x16.png" >/dev/null
  sips -z 32 32 "$icon_src" --out "$iconset/icon_16x16@2x.png" >/dev/null
  sips -z 32 32 "$icon_src" --out "$iconset/icon_32x32.png" >/dev/null
  sips -z 64 64 "$icon_src" --out "$iconset/icon_32x32@2x.png" >/dev/null
  sips -z 128 128 "$icon_src" --out "$iconset/icon_128x128.png" >/dev/null
  sips -z 256 256 "$icon_src" --out "$iconset/icon_128x128@2x.png" >/dev/null
  sips -z 256 256 "$icon_src" --out "$iconset/icon_256x256.png" >/dev/null
  sips -z 512 512 "$icon_src" --out "$iconset/icon_256x256@2x.png" >/dev/null
  sips -z 512 512 "$icon_src" --out "$iconset/icon_512x512.png" >/dev/null
  sips -z 1024 1024 "$icon_src" --out "$iconset/icon_512x512@2x.png" >/dev/null
  iconutil -c icns "$iconset" -o "$app/Contents/Resources/Presence.icns"
fi

# Sign when a Developer ID is available. An empty identity leaves the app unsigned.
if [[ -n "${APPLE_CODESIGN_IDENTITY:-}" ]]; then
  codesign --force --deep --sign "$APPLE_CODESIGN_IDENTITY" "$app"
fi

rm -f "$out/Presence-macos.zip"
ditto -c -k --norsrc --keepParent "$app" "$out/Presence-macos.zip"
echo "Wrote $out/Presence-macos.zip"
