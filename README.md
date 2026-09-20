# presence

A small macOS terminal app for interrupting maladaptive daydreaming and returning your attention to the present.

Presence tracks elapsed time, announces each minute, and periodically uses a local Ollama model to generate a short grounding prompt. The prompt is displayed and read aloud.

While Presence speaks, other media is paused and then resumed. That includes Music, Spotify, and whatever is currently in macOS Now Playing (YouTube in a browser, or another music app that shows up in Control Center). Browser tabs are not scanned. The first run may ask for Automation permission for Music and Spotify.

## Requirements

- macOS
- Rust and Cargo
- [Kitty](https://sw.kovidgoyal.net/kitty/)
- [Ollama](https://ollama.com)
- The `gemma4:12b` model
- [Alfred](https://www.alfredapp.com) with Powerpack, if you want the `presence` keyword

## Setup

```sh
ollama pull gemma4:12b
cargo install --path . --root ~/.local
presence
```

Presence starts the Ollama server when needed. The server remains running after Presence exits.

## Controls

- `Space`, `Enter`, or `s` — start or stop
- `q` or `Esc` — quit

Edit `prompt.txt` to change the style of the generated prompts. To use another model:

```sh
OLLAMA_MODEL=model-name presence
```

All prompt generation runs locally through Ollama.

## Alfred

The `presence` keyword is an Alfred workflow that lives in this repo (`alfred/`). It starts Presence in Kitty: split a pane if Kitty is already open, focus that pane if Presence is already running, or launch Kitty if it is not.

1. Install Presence as above so `presence` is on your `PATH` (`~/.local/bin/presence`).
2. Add this to `kitty.conf`, then quit and reopen Kitty:

```
allow_remote_control socket-only
listen_on unix:${HOME}/.cache/kitty/control
```

3. Install the workflow:

```sh
./alfred/install.sh
```

Then type `presence` in Alfred.
