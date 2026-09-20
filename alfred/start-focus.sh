#!/usr/bin/env bash
# Start Focus in a Kitty pane. If Kitty is already running, split a pane in the
# current tab. If Focus is already open, just bring that pane forward. If Kitty
# is not running, launch it with Focus.
set -euo pipefail

FOCUS_BIN="${FOCUS_BIN:-${HOME}/.local/bin/focus}"
FOCUS_DIR="${FOCUS_DIR:-${HOME}/Code/focus}"
SOCKET_PREFIX="${HOME}/.cache/kitty/control"
export PATH="${HOME}/.local/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin"

if [[ -d "${HOME}/Applications/kitty.app" ]]; then
  KITTY_APP="${HOME}/Applications/kitty.app"
elif [[ -d /Applications/kitty.app ]]; then
  KITTY_APP="/Applications/kitty.app"
else
  KITTY_APP=""
fi

if [[ -x "${HOME}/.local/bin/kitten" ]]; then
  KITTEN="${HOME}/.local/bin/kitten"
elif [[ -n "$KITTY_APP" && -x "${KITTY_APP}/Contents/MacOS/kitten" ]]; then
  KITTEN="${KITTY_APP}/Contents/MacOS/kitten"
else
  KITTEN="$(command -v kitten || true)"
fi

if [[ -x "${HOME}/.local/bin/kitty" ]]; then
  KITTY_BIN="${HOME}/.local/bin/kitty"
elif [[ -n "$KITTY_APP" && -x "${KITTY_APP}/Contents/MacOS/kitty" ]]; then
  KITTY_BIN="${KITTY_APP}/Contents/MacOS/kitty"
else
  KITTY_BIN="$(command -v kitty || true)"
fi

if [[ ! -x "$FOCUS_BIN" ]]; then
  FOCUS_BIN="$(command -v focus || true)"
fi

if [[ ! -x "${FOCUS_BIN:-}" ]]; then
  echo "start-focus: focus is missing; install with cargo install --path . --root ~/.local" >&2
  exit 1
fi

if [[ ! -d "$FOCUS_DIR" ]]; then
  FOCUS_DIR="${HOME}"
fi

mkdir -p "${HOME}/.cache/kitty"

# listen_on unix:.../control becomes .../control-<pid>
find_socket() {
  local sock
  shopt -s nullglob
  for sock in "${SOCKET_PREFIX}"-*; do
    if [[ -S "$sock" ]] && "$KITTEN" @ --to "unix:${sock}" ls >/dev/null 2>&1; then
      printf '%s\n' "$sock"
      return 0
    fi
  done
  if [[ -S "$SOCKET_PREFIX" ]] && "$KITTEN" @ --to "unix:${SOCKET_PREFIX}" ls >/dev/null 2>&1; then
    printf '%s\n' "$SOCKET_PREFIX"
    return 0
  fi
  return 1
}

kitty_control() {
  "$KITTEN" @ --to "unix:${SOCKET}" "$@"
}

activate_kitty() {
  if [[ -n "$KITTY_APP" ]]; then
    open -a "$KITTY_APP"
  fi
}

# `open -a` without -n only activates a running Kitty and drops --args.
launch_new_os_window() {
  if [[ -n "$KITTY_APP" ]]; then
    open -n -a "$KITTY_APP" --args --directory "$FOCUS_DIR" --title focus "$FOCUS_BIN"
  elif [[ -n "$KITTY_BIN" ]]; then
    "$KITTY_BIN" --detach --directory "$FOCUS_DIR" --title focus "$FOCUS_BIN"
  else
    echo "start-focus: kitty is not installed" >&2
    exit 1
  fi
}

if [[ -z "${KITTEN:-}" ]]; then
  launch_new_os_window
  exit 0
fi

if SOCKET="$(find_socket)"; then
  if kitty_control focus-window --match 'cmdline:focus$' >/dev/null 2>&1; then
    activate_kitty
    exit 0
  fi

  if ! kitty_control launch --type=window --location=vsplit \
    --cwd="$FOCUS_DIR" --title=focus \
    --env "PATH=$PATH" \
    "$FOCUS_BIN" >/dev/null 2>&1; then
    if ! kitty_control launch --type=window \
      --cwd="$FOCUS_DIR" --title=focus \
      --env "PATH=$PATH" \
      "$FOCUS_BIN" >/dev/null 2>&1; then
      launch_new_os_window
      exit 0
    fi
  fi
  activate_kitty
  exit 0
fi

launch_new_os_window
