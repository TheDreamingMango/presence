#!/bin/sh
# Flatten quotes.md numbered items into ../quotes.csv
set -eu
cd "$(dirname "$0")"

python3 - <<'PY'
from pathlib import Path
import re

here = Path(".")
src = (here / "quotes.md").read_text()
out = here.parent / "quotes.csv"

start = re.compile(r"^(\d+)\.\s+(.*)$")
items = []
current = None
for line in src.splitlines():
    match = start.match(line)
    if match:
        if current is not None:
            items.append(current)
        current = match.group(2).strip()
        continue
    if current is not None and line.startswith(" ") and line.strip():
        current = f"{current} {line.strip()}"
if current is not None:
    items.append(current)


def csv_field(text: str) -> str:
    return '"' + text.replace('"', '""') + '"'


out.write_text("text\n" + "\n".join(csv_field(q) for q in items) + "\n")
print(f"wrote {len(items)} quotes to {out}")
PY
