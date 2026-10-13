"""Lint accuracy by mutation: plant known defects in files that compile, and check
that the right rule fires at the right place. Unmutated files measure false
positives (an error-level diagnostic on a file feaLib compiles cleanly).

For defects that are compile errors, feaLib must also reject the mutated file;
that confirms the planted defect really is one.

Usage: lint_mutations.py FEA_BINARY ORACLE_JSON WORK_DIR OUT_JSON [MAX_PER_KIND]
ORACLE_JSON is fealib_equiv.py's output; files with status "identical" are used.
"""
import json
import pathlib
import random
import re
import subprocess
import sys

from fontTools.feaLib.builder import addOpenTypeFeaturesFromString
from fontTools.fontBuilder import FontBuilder

binary, oracle, work, out = sys.argv[1:5]
per_kind = int(sys.argv[5]) if len(sys.argv) > 5 else 3
work = pathlib.Path(work)
work.mkdir(parents=True, exist_ok=True)
rng = random.Random(7)
WORD = re.compile(r"[A-Za-z0-9._\-+*:^|~]+")


def names_in(text):
    return {n for n in WORD.findall(text) if not n[0].isdigit() and not n.startswith("-")}


def fealib_ok(text, order, wrapped):
    if wrapped:
        text = f"feature zzzz {{\n{text}\n}} zzzz;\n"
    fb = FontBuilder(1000, isTTF=True)
    fb.setupGlyphOrder([".notdef"] + sorted(order - {".notdef"}))
    try:
        addOpenTypeFeaturesFromString(fb.font, text)
        return True
    except Exception:
        return False


def lint(path, glyphs, project=False):
    if project:
        r = subprocess.run([binary, "project", str(path), "--glyphs", str(glyphs), "--format", "json", "--no-config"], capture_output=True, text=True)
        return json.loads(r.stdout or "[]")
    r = subprocess.run([binary, "lint", str(path), "--glyphs", str(glyphs), "--format", "json", "--no-config"], capture_output=True, text=True)
    return json.loads(r.stdout or "[]")


def tokens(path, text):
    """Tokens with start/end converted from UTF-8 byte offsets to str indices."""
    r = subprocess.run([binary, "dev", "tokens", str(path)], capture_output=True, text=True)
    toks = [json.loads(l) for l in r.stdout.splitlines()]
    byte_to_char = {}
    at = 0
    for i, ch in enumerate(text):
        byte_to_char[at] = i
        at += len(ch.encode("utf-8"))
    byte_to_char[at] = len(text)
    for t in toks:
        t["start"], t["end"] = byte_to_char[t["start"]], byte_to_char[t["end"]]
    return toks


def offset_of(text, line, character):
    # line/character are zero-based, character counts UTF-16 units
    start = 0
    for _ in range(line):
        start = text.index("\n", start) + 1
    col = 0
    i = start
    while col < character:
        col += 2 if ord(text[i]) > 0xFFFF else 1
        i += 1
    return i


def hit(diags, text, rule, at):
    for d in diags:
        if d["rule"] != rule:
            continue
        if "range" in d:
            pos = offset_of(text, d["range"]["start"]["line"], d["range"]["start"]["character"])
        else:
            pos = offset_of(text, d["line"], d["character"])
        if pos == at:
            return True
    return False


# ---------------------------------------------------------------- mutations
# each returns (mutated_text, expected_rule, expected_offset, is_compile_error) or None

def m_glyph(text, toks):
    picks = [t for t in toks if t["kind"] == "GlyphName" and (t["parent"] or "").startswith(("Gsub", "Gpos", "Context", "Backtrack", "Lookahead", "GlyphClass"))]
    if not picks:
        return None
    t = rng.choice(picks)
    return text[: t["end"]] + "zq" + text[t["end"]:], "glyph-not-in-font", t["start"], True


def m_duplicate(text, toks):
    picks = [t for t in toks if t["kind"] == "GlyphName" and t["parent"] == "GlyphClass" and t["grandparent"] == "GlyphClassDefNode"]
    # skip classes used on a replacement side, where repeats are meaningful
    replacement_side = {c for tail in re.findall(r"\b(?:by|from)\b([^;]*);", text) for c in re.findall(r"@[\w.\-]+", tail)}
    defs = {}
    for t in toks:
        if t["kind"] == "NamedGlyphClass" and t["parent"] == "GlyphClassDefNode":
            defs.setdefault(t["start"], t["text"])
    def owner(t):
        before = [s for s in defs if s < t["start"]]
        return defs[max(before)] if before else None
    picks = [t for t in picks if owner(t) not in replacement_side]
    if not picks:
        return None
    t = rng.choice(picks)
    return text[: t["end"]] + " " + t["text"] + text[t["end"]:], "duplicate-glyph-in-class", t["end"] + 1, False


def m_invisible(text, toks):
    picks = [t for t in toks if t["kind"] == "Whitespace" and t["text"] == " " and (t["parent"] or "").startswith(("Gsub", "Gpos"))]
    if not picks:
        return None
    t = rng.choice(picks)
    return text[: t["start"]] + " " + text[t["end"]:], "invisible-character", t["start"], True


def m_tag(text, toks):
    significant = [t for t in toks if t["kind"] not in ("Whitespace", "Comment")]
    opening = [t for prev, t in zip(significant, significant[1:]) if prev["kind"] == "FeatureKw" and t["kind"] == "Tag"]
    if not opening:
        return None
    t = rng.choice(opening)
    # the opening tag and the closing tag of the same block
    closing = [u for prev, u in zip(significant, significant[1:]) if prev["kind"] == "RBrace" and u["kind"] == "Tag" and u["text"] == t["text"] and u["start"] > t["start"]]
    same = [t] + closing[:1]
    mutated = text
    for u in sorted(same, key=lambda u: -u["start"]):
        mutated = mutated[: u["start"]] + "zzzx" + mutated[u["end"]:]
    first = min(u["start"] for u in same)
    return mutated, "unknown-feature-tag", first, False


def m_class_length(text, toks):
    # `sub @A by @B;` where @B is a plain list of glyphs: drop @B's last glyph
    pairs = re.finditer(r"\bsub\s+(@[\w.\-]+)\s+by\s+(@[\w.\-]+)\s*;", text)
    candidates = []
    for m in pairs:
        # skip rules inside comments: changing their classes isn't a planted error there
        line_start = text.rfind("\n", 0, m.start()) + 1
        if "#" in text[line_start:m.start()]:
            continue
        target = m.group(2)
        d = re.search(re.escape(target) + r"\s*=\s*\[([^\]#@$]*)\]\s*;", text)
        # a one-glyph replacement is valid FEA, so the shortened class must keep two or more
        if d and len(d.group(1).split()) > 2 and d.start() < m.start():
            candidates.append((m, d))
    if not candidates:
        return None
    m, d = rng.choice(candidates)
    members = d.group(1)
    last = list(re.finditer(r"\S+", members))[-1]
    cut_start, cut_end = d.start(1) + last.start(), d.start(1) + last.end()
    mutated = text[:cut_start].rstrip(" ") + text[cut_end:]
    shift = cut_end - cut_start + (len(text[:cut_start]) - len(text[:cut_start].rstrip(" ")))
    at = m.start(1) - shift if m.start(1) > cut_start else m.start(1)
    return mutated, "class-length-mismatch", at, True


def m_undefined(text, toks):
    defined = {t["text"] for t in toks if t["kind"] == "NamedGlyphClass" and t["parent"] == "GlyphClassDefNode"}
    uses = [t for t in toks if t["kind"] == "NamedGlyphClass" and t["parent"] != "GlyphClassDefNode" and t["parent"] != "MarkClassNode" and t["text"] in defined]
    if not uses:
        return None
    t = rng.choice(uses)
    return text[: t["start"]] + "@zqUndefined" + text[t["end"]:], "undefined-class", t["start"], True


MUTATIONS = {
    "glyph-not-in-font": m_glyph,
    "duplicate-glyph-in-class": m_duplicate,
    "invisible-character": m_invisible,
    "unknown-feature-tag": m_tag,
    "class-length-mismatch": m_class_length,
    "undefined-class": m_undefined,
}

files = [r for r in json.load(open(oracle))["results"] if r["status"] == "identical"]
report = {"false_positives": [], "mutations": {k: {"planted": 0, "found": 0, "compiler_agrees": 0, "misses": []} for k in MUTATIONS}}
checked_files = 0
for i, r in enumerate(files):
    src = pathlib.Path(r["path"])
    # newline="" keeps \r\n as written, so offsets match the bytes the tool sees
    text = open(src, encoding="utf-8", errors="replace", newline="").read()
    order = names_in(text)
    glyphs = work / f"{i}.glyphs.txt"
    glyphs.write_text("\n".join(sorted(order)) + "\n")
    toks = tokens(src, text)
    checked_files += 1

    # false positives: error-level diagnostics on a file feaLib compiles
    for d in lint(src, glyphs):
        if d["severity"] == "error":
            report["false_positives"].append({"file": str(src), "rule": d["rule"], "message": d["message"]})

    for kind, mutate in MUTATIONS.items():
        for _ in range(per_kind):
            m = mutate(text, toks)
            if m is None:
                break
            mutated, rule, at, compile_error = m
            path = work / f"{i}.{kind}.fea"
            open(path, "w", encoding="utf-8", newline="").write(mutated)
            diags = lint(path, glyphs, project=(kind == "undefined-class"))
            stats = report["mutations"][kind]
            stats["planted"] += 1
            if hit(diags, mutated, rule, at):
                stats["found"] += 1
            else:
                near = [d["rule"] for d in diags][:6]
                stats["misses"].append({"file": str(src), "at": at, "context": mutated[max(0, at - 40): at + 40], "got": near})
            if compile_error and not fealib_ok(mutated, order, r.get("wrapped")):
                stats["compiler_agrees"] += 1
    print(f"{i + 1}/{len(files)} {src.name}", flush=True)

report["files"] = checked_files
json.dump(report, open(out, "w"), indent=1)
print(f"files {checked_files}; false positives {len(report['false_positives'])}")
for k, v in report["mutations"].items():
    print(f"{k:28} planted {v['planted']:4}  found {v['found']:4}  feaLib also rejects {v['compiler_agrees']:4}")
