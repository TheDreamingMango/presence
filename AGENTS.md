# Presence

Presence is a small macOS terminal app that helps interrupt maladaptive daydreaming. Keep it fast, calm, local-first, and easy to understand.

## Project Shape

- `src/main.rs` contains the Ratatui UI, timer, speech queue, and Ollama integration.
- `prompt.txt` defines the grounding-prompt contract and is read at runtime.
- `alfred/` is the optional Alfred workflow that starts Presence in Kitty via the `presence` keyword.
- External commands are macOS `say`, `osascript` (pause/resume other media while speaking), `pgrep`, `caffeinate`, and local `ollama`; do not add network services without an explicit requirement.

## Working Agreements

- Prefer the smallest clear change; avoid abstractions until they remove real duplication or complexity.
- Keep blocking process work off the UI loop so input and rendering remain responsive.
- Preserve terminal restoration on every normal error and exit path.
- Serialize speech so announcements do not overlap.
- Treat unavailable commands, model failures, empty output, and channel disconnects gracefully; avoid panics in runtime paths.
- Keep user-facing language brief, compassionate, and non-judgmental.
- Preserve `OLLAMA_MODEL` support and runtime editing of `prompt.txt`.
- Add dependencies only when the standard library or existing crates are insufficient.

## Verification

Run before handing off:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

For behavior changes, also run `cargo run` on macOS and check start/stop, quit, terminal cleanup, prompt generation, and spoken output.
