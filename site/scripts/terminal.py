#!/usr/bin/env python3
"""Capture real Feanor terminal sessions for the "In your terminal" section.

Runs each command in a pseudo-terminal (so Feanor prints in colour) on copies
of the page's own examples, and writes src/terminal.json: the command line as
shown, its output with ANSI colours, and its exit status.

Usage (from site/): python3 scripts/terminal.py [path/to/feanor]
"""

import json
import pathlib
import shutil
import subprocess
import sys
import tempfile

SITE = pathlib.Path(__file__).resolve().parent.parent
EXAMPLES = SITE / "examples"
FEANOR = sys.argv[1] if len(sys.argv) > 1 else str(SITE.parent / "target/release/feanor")

# (id, tab label, command as shown, files to put in place: name -> example)
SESSIONS = [
    ("lint", "Lint", "feanor lint --glyphs glyphs.txt features.fea",
     {"features.fea": "lint.fea", "glyphs.txt": "playground-glyphs.txt"}),
    ("format", "Format", "feanor fmt --diff features.fea", {"features.fea": "playground.fea"}),
    ("ci", "CI", "feanor check --format github features.fea", {"features.fea": "lint.fea"}),
    ("explain", "Explain", "feanor rule R003", {}),
]


def main():
    out = []
    for id_, label, command, files in SESSIONS:
        with tempfile.TemporaryDirectory() as tmp:
            for name, example in files.items():
                shutil.copyfile(EXAMPLES / example, pathlib.Path(tmp) / name)
            argv = [FEANOR] + command.split()[1:]
            # `script` gives the command a terminal; it reports the exit status in its own
            run = subprocess.run(["script", "-qec", " ".join(argv), "/dev/null"], cwd=tmp,
                                 capture_output=True, text=True)
            text = run.stdout.replace("\r\n", "\n").rstrip("\n")
        out.append({"id": id_, "label": label, "command": command, "output": text, "status": run.returncode})
        print(f"{id_}: {len(text.splitlines())} lines, exit {run.returncode}")
    (SITE / "src" / "terminal.json").write_text(json.dumps(out, indent=1, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    main()
