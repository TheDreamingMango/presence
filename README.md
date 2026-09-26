# presence

A small macOS terminal app for interrupting maladaptive daydreaming and returning your attention to the present.

Presence tracks elapsed time, announces each minute, and periodically shows a short grounding prompt from `quotes.csv`. The prompt is displayed and read aloud.

While Presence speaks, other media is paused and then resumed. That includes Music, Spotify, and whatever is currently in macOS Now Playing (YouTube in a browser, or another music app that shows up in Control Center). Browser tabs are not scanned. The first run may ask for Automation permission for Music and Spotify.

You can run Presence from any terminal. [Ollama](https://ollama.com) and [Alfred](https://www.alfredapp.com) are optional.

**[Download for Mac](https://github.com/TheDreamingMango/presence/releases/latest/download/Presence-macos.zip)**

Unzip and double-click Presence. The first time, macOS asks you to confirm the app: right-click Presence, choose Open, then Open again.

## Requirements

- macOS

Rust and Cargo are only needed to build from source.

## Setup

Download the zip above, unzip it, and double-click Presence.

To build from a checkout instead:

```sh
cargo install --path . --root ~/.local
presence
```

To pull later changes from this repo and reinstall:

```sh
./update.sh
```

## Controls

- `Space`, `Enter`, or `s` — start or stop
- `q` or `Esc` — quit

## Optional: Ollama

If Ollama and the `gemma4:12b` model are installed, Presence generates a fresh grounding prompt instead of using `quotes.csv`. Presence starts the Ollama server when needed. The server remains running after Presence exits.

```sh
ollama pull gemma4:12b
```

The first launch writes editable copies to `~/Library/Application Support/Presence/`. Edit `prompt.txt` there to change the style of the generated prompts. If those files are missing, Presence uses the copies in this checkout, then the copies built into the app. To use another model:

```sh
OLLAMA_MODEL=model-name presence
```

The offline list is written in `quotes/quotes.md` and flattened into `quotes.csv`. To regenerate or audit the list, see `quotes/AGENTS.md`, then run `./quotes/to_csv.sh`.

## Optional: Alfred

The `presence` keyword is an Alfred workflow in this repo (`alfred/`). It needs Alfred with Powerpack and [Kitty](https://sw.kovidgoyal.net/kitty/). It starts Presence in Kitty: split a pane if Kitty is already open, focus that pane if Presence is already running, or launch Kitty if it is not.

1. Build from a checkout so `presence` is on your `PATH` (`~/.local/bin/presence`). The downloaded app does not install that command.
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
