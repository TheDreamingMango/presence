# focus

A small macOS terminal app for interrupting maladaptive daydreaming and returning your attention to the present.

Focus tracks elapsed time, announces each minute, and periodically uses a local Ollama model to generate a short grounding prompt. The prompt is displayed and read aloud.

## Requirements

- macOS
- Rust and Cargo
- [Ollama](https://ollama.com)
- The `gemma4:12b` model

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
