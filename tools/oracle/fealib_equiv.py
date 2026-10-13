"""Formatter equivalence oracle, independent of fea-rs: compile each file before
and after `fea fmt` with fontTools feaLib and compare the compiled layout tables.

Usage: fealib_equiv.py FEA_BINARY LIST_FILE OUT_JSON
LIST_FILE has one entry per line: `path` or `path<TAB>glyph-list.txt`.

The glyph order is every name-like word in the file (plus the glyph list when
given), so stale names don't stop the compile. Files that only parse inside a
feature block (feature bodies) are wrapped in one, the same way on both sides.
Files that feaLib can't compile on their own (classes defined elsewhere,
includes) are reported as such, not as passes.
"""
import json
import re
import subprocess
import sys

from fontTools.feaLib.builder import addOpenTypeFeaturesFromString
from fontTools.fontBuilder import FontBuilder

TABLES = ("GSUB", "GPOS", "GDEF")
WORD = re.compile(r"[A-Za-z0-9._\-+*:^|~]+")


def glyph_order(text, extra):
    names = set(WORD.findall(text)) | set(extra)
    names = {n for n in names if not n[0].isdigit() and not n.startswith("-")}
    return [".notdef"] + sorted(names - {".notdef"})


def compile_tables(text, order):
    fb = FontBuilder(1000, isTTF=True)
    fb.setupGlyphOrder(order)
    font = fb.font
    addOpenTypeFeaturesFromString(font, text)
    return {t: font[t].compile(font) for t in TABLES if t in font}


TIME_LIMIT = 120  # seconds per compile; feaLib can take an hour on a huge GPOS


class CompileTimeout(Exception):
    pass


def try_compile(text, order):
    import signal

    def give_up(signum, frame):
        raise CompileTimeout()

    signal.signal(signal.SIGALRM, give_up)
    signal.alarm(TIME_LIMIT)
    try:
        return compile_tables(text, order), None
    except CompileTimeout:
        return None, "TIMEOUT"
    except Exception as e:  # feaLib raises many error types; record the first line
        return None, f"{type(e).__name__}: {str(e).splitlines()[0][:200]}"
    finally:
        signal.alarm(0)


def check(entry):
    binary, line = entry
    parts = line.rstrip("\n").split("\t")
    path, glyphs = parts[0], parts[1] if len(parts) > 1 else None
    before = open(path, encoding="utf-8", errors="replace").read()
    fmt = subprocess.run([binary, "fmt", path, "--no-config"], capture_output=True, text=True)
    if fmt.returncode != 0:
        return {"path": path, "status": "fmt-failed", "detail": fmt.stderr[-300:]}
    after = fmt.stdout
    extra = open(glyphs).read().split() if glyphs else []
    order = glyph_order(before, extra)

    a, err = try_compile(before, order)
    wrapped = False
    if a is None:
        wrap = lambda s: f"feature zzzz {{\n{s}\n}} zzzz;\n"
        a2, _ = try_compile(wrap(before), order)
        if a2 is not None:
            a, err, wrapped = a2, None, True
            before, after = wrap(before), wrap(after)
    if err == "TIMEOUT":
        return {"path": path, "status": "fealib-timeout"}
    if a is None:
        return {"path": path, "status": "source-does-not-compile", "detail": err}
    b, err_after = try_compile(after, order)
    if err_after == "TIMEOUT":
        return {"path": path, "status": "fealib-timeout"}
    if b is None:
        return {"path": path, "status": "FORMATTED-DOES-NOT-COMPILE", "detail": err_after}
    return {
        "path": path,
        "status": "identical" if a == b else "DIFFERENT",
        "wrapped": wrapped,
        "bytes": sum(len(v) for v in a.values()),
        "tables": sorted(a),
    }


def main():
    import multiprocessing

    binary, listing, out = sys.argv[1:4]
    entries = [(binary, l) for l in open(listing) if l.strip()]
    results = []
    with multiprocessing.Pool(8) as pool:
        for i, r in enumerate(pool.imap_unordered(check, entries, chunksize=4)):
            results.append(r)
            if r["status"] not in ("identical", "source-does-not-compile", "fealib-timeout"):
                print(f"{r['status']}: {r['path']} {r.get('detail', '')}", flush=True)
            if (i + 1) % 250 == 0:
                print(f"{i + 1}/{len(entries)}", flush=True)
    counts = {}
    for r in results:
        counts[r["status"]] = counts.get(r["status"], 0) + 1
    json.dump({"counts": counts, "results": results}, open(out, "w"), indent=1)
    print(counts)


if __name__ == "__main__":
    main()
