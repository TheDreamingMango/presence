# focus

A small macOS terminal app for interrupting maladaptive daydreaming and returning your attention to the present.

Focus tracks elapsed time, announces each minute, and periodically uses a local Ollama model to generate a short grounding prompt. The prompt is displayed and read aloud.

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
focus
```

Focus starts the Ollama server when needed. The server remains running after Focus exits.

## Controls

- `Space`, `Enter`, or `s` — start or stop
- `q` or `Esc` — quit

Edit `prompt.txt` to change the style of the generated prompts. To use another model:

```sh
OLLAMA_MODEL=model-name focus
```

All prompt generation runs locally through Ollama.

## Alfred

The `presence` keyword is an Alfred workflow that lives in this repo (`alfred/`). It starts Focus in Kitty: split a pane if Kitty is already open, focus that pane if Focus is already running, or launch Kitty if it is not.

1. Install Focus as above so `focus` is on your `PATH` (`~/.local/bin/focus`).
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
