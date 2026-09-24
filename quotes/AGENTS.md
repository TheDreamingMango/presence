# Quotes

Workshop for the offline grounding list Presence uses when Ollama is unavailable.

This folder exists to make `quotes.md`. After a regenerate or audit, run
`./quotes/to_csv.sh` so `quotes.csv` at the repo root stays in sync.

`prompt.txt` at the repo root is the live runtime contract for Ollama — do not
change it unless asked. This folder holds a working copy plus the quote list.

## Which job

Follow what was asked this turn.

**Regenerate** — replace `quotes.md` with a new list. Use this when asked to
start over, regenerate, write a new file, or build the list from scratch. Do
not preserve old lines. Write the full set (110 unless told otherwise). Every
line original. Every opening and action unique. Then run `./quotes/to_csv.sh`.

**Audit** — edit the existing `quotes.md`. Use this when asked to audit,
improve, fix, or keep what already lands. Keep quotes that work. Rewrite only
the weak ones. Do not pad every line to 3–4, and do not treat length as the
goal. Then run `./quotes/to_csv.sh`.

If it is unclear which job this is, ask before touching the file.

## Voice

Brief. Direct. Non-judgmental. Spoken to “you”. The person already asked to
come back.

Firm, compassionate, immediate — no guilt, shame, diagnosis, or cliché.
Generic affirmations (“I am in control of my mind”) are a poor fit. Write
original lines; do not copy blogs, Reddit mantras, or clinical pamphlets.

One or two short sentences is enough. Extra room (up to 3–4 short lines) only
if it earns its keep.

## What each quote must do

- name a real cost: time, energy, sleep, goals, or future you
- end with one specific action using only what they always have: their body,
  the clothes they are wearing, or looking at the room they are already in
- be unique in wording, opening, and action

A quote is weak if the action needs anything we cannot know is there. The
interrupt can catch them mid-pace, lying down, standing in a hallway, or away
from a workstation. **Do not assume a computer, desk, or chair.**

## Action constraints

- do not assume a computer, laptop, phone, screen, keyboard, trackpad, cursor,
  menu bar, Dock, or any device in reach
- do not assume furniture: desk, table, chair, bed, sofa, counter
- do not assume they are sitting, standing at a workstation, or close enough
  to a wall to touch it
- looking at a wall, ceiling, floor, light, or shadow is allowed; touching
  those is not
- never assume chores, objects, or waiting tasks (no mail, laundry, towels,
  dishes, unread messages, clocks, doors, windows)
- avoid Focus keybinds as actions: `q`, `Esc`, Space, Enter, `s`
- do not default to standing up or mentioning feet
- do not suggest visualization, talking to characters, 5-4-3-2-1 inventories,
  or other inward exercises

## Themes (paraphrase, do not paste)

There is MD writing online — blogs, Reddit, clinical grounding lists — but
nothing that already matches this voice. Use it for themes, not as a quote
bank:

- a life, not stories about a life
- the ghost who misses the hour
- “just finish this scene” never ends
- postpone, don’t forbid: not yet; later
- stop the maintenance (music, pacing) before the plot

## Files

- `prompt.txt` — working contract for a single intervention (copy of the live
  prompt, with extra constraints for this list)
- `quotes.md` — the current list. Regenerating replaces it. Auditing edits it.
  Drop the old 20-word / 100-character cap if a line needs more room.
- `to_csv.sh` — flattens `quotes.md` into `../quotes.csv` for the app
