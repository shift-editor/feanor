"""Build the data for the "what ships" scenes on the landing page.

For each scene, compile each version of its feature code into a subset of a
real font, shape the sample text with HarfBuzz, and save each glyph's outline,
position and cluster. The page only animates between the results, so what it
shows is HarfBuzz's real output. A version can be compiled by fontTools (the
default) or by fea-rs, through `feanor dev compile`.

A scene can instead use a font exactly as it ships, with its own layout
tables: then the fixed version is that font with the fix applied to its
compiled tables (`patch`), and the code shown is an excerpt of its source.

Martel Bold (for the i-matra scene) is
https://github.com/google/fonts/raw/main/ofl/martel/Martel-Bold.ttf
Idiqlat Regular (for the Syriac scene) is
https://github.com/google/fonts/raw/main/ofl/idiqlat/Idiqlat-Regular.ttf
and Coiny Regular (for the kerning scene) is
https://github.com/google/fonts/raw/main/ofl/coiny/Coiny-Regular.ttf

The joining scene uses two builds of Playwrite AR from its current source, with
and without its designer's 2023 fix ("Fix zx cnct"); ~/feanor-tools/scenes/
playwrite_zx.py makes them.

Usage (from site/):
  nix shell --impure --expr '(import<nixpkgs>{}).python3.withPackages(p:[p.fonttools p.uharfbuzz])' \\
    --command python3 scenes/build.py \\
      --inter "$(nix build --no-link --print-out-paths nixpkgs#inter)/share/fonts/truetype/InterVariable.ttf" \\
      --martel Martel-Bold.ttf \\
      --idiqlat Idiqlat-Regular.ttf \\
      --coiny Coiny-Regular.ttf \\
      --extra playwrite-before=playwrite-ar-before.ttf --extra playwrite-after=playwrite-ar-after.ttf \\
      --feanor ../target/release/feanor

Writes src/scenes/<scene>.json.
"""

import argparse
import io
import json
import pathlib
import subprocess
import tempfile

import uharfbuzz as hb
from fontTools.feaLib.builder import addOpenTypeFeaturesFromString
from fontTools.pens.boundsPen import BoundsPen
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.subset import Options, Subsetter
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

OUT = pathlib.Path(__file__).resolve().parent.parent / "src" / "scenes"

INTER = "Inter Bold, © 2016 The Inter Project Authors, SIL Open Font License 1.1"
MARTEL = "Martel Bold, © 2015 Dan Reynolds, © 2010–2015 Sorkin Type Co, SIL Open Font License 1.1"
IDIQLAT = ("Idiqlat Regular, © 2025–2026 SIL Global, original fonts © 1997–2025 Sparksoft Systems, "
           "SIL Open Font License 1.1")
COINY = "Coiny Regular, © 2015 Marcelo Magalhaes, SIL Open Font License 1.1"
PLAYWRITE = "Playwrite AR, © 2023 The Playwrite Project Authors, SIL Open Font License 1.1"

SCENES = {
    "fixed-join": {
        # built from the font's current source (its designer's fix in place), and with
        # the line that fix removed put back: see --extra
        "font": "playwrite-after",
        "shipped": True,
        "text": "zx",
        "focus": [0, 2],
        "features": {},
        "variants": {
            "written": {
                "label": "Before the fix",
                "font": "playwrite-before",
                "fea": "@zful_SRC = [z.cnt.ini z.cnt.med];\n@xmod_TGT = [x.mod.fin x.mod.med];\n@xful_TGT = [x.ful.fin x.ful.med];\n\n"
                "feature calt {\n    sub @zful_SRC' lookup before_x @xful_TGT;\n"
                "    sub @zful_SRC' lookup before_x @xmod_TGT;\n"
                "    sub @zful_SRC' lookup before_xful @xful_TGT;\n} calt;\n",
                "note": "The first rule picks the connector from z to most forms of x. The rule for this "
                "form of x comes later, so its connector, drawn to meet the x's diagonal, never appears.",
            },
            "meant": {
                "label": "After the fix",
                "font": "playwrite-after",
                "fea": "@zful_SRC = [z.cnt.ini z.cnt.med];\n@xmod_TGT = [x.mod.fin x.mod.med];\n@xful_TGT = [x.ful.fin x.ful.med];\n\n"
                "feature calt {\n    sub @zful_SRC' lookup before_x @xmod_TGT;\n"
                "    sub @zful_SRC' lookup before_xful @xful_TGT;\n} calt;\n",
                "note": "The designer removed the line that caught it first. The connector now meets the "
                "x where it was drawn to.",
            },
        },
    },
    "duplicate-pair": {
        "font": "coiny",
        # the font as it ships: its kern feature lists three pairs twice, with different values
        "shipped": True,
        "text": "Face",
        "glyphs": ["F", "a"],
        "focus": [0, 1],
        "features": {},
        "variants": {
            # an excerpt of the source's kern feature
            "written": {
                "label": "As shipped",
                "fea": "feature kern {\n    pos F a -30;\n    pos F a -55;\n    pos F c -55;\n    pos F c -40;\n"
                "    pos F e -55;\n    pos F e -37;\n} kern;\n",
                "note": "Each pair is written twice, a line apart, with two values. Compilers keep the "
                "first and ignore the second, so F a ships at -30.",
            },
            "meant": {
                "label": "Later values",
                "patch": "later-pairs",
                "fea": "feature kern {\n    pos F a -55;\n    pos F c -40;\n    pos F e -37;\n} kern;\n",
                "note": "With the later values, F a closes up by 25 units. Only the designer knows which "
                "value was meant; Feanor's automatic fix removes the ignored line, which keeps the font "
                "as it ships.",
            },
        },
    },
    "unreachable-rule": {
        "font": "inter",
        "text": "T-Rex x-Ray",
        "glyphs": ["T", "R", "e", "x", "a", "y", "hyphen", "hyphen.case", "space"],
        "features": {"calt": True},
        "variants": {
            # the exception for a lowercase letter comes after the rule it should stop
            "written": {
                "label": "As written",
                "fea": "@upper = [R T];\n@lower = [a e x y];\n\nfeature calt {\n"
                "    sub hyphen' @upper by hyphen.case;\n"
                "    ignore sub @lower hyphen' @upper;\n} calt;\n",
                "note": "A hyphen before a capital is raised to the middle of the capitals. The "
                "exception for one after a lowercase letter never runs, so in x-Ray it floats above the x.",
            },
            "meant": {
                "label": "Fixed",
                "fea": "@upper = [R T];\n@lower = [a e x y];\n\nfeature calt {\n"
                "    ignore sub @lower hyphen' @upper;\n"
                "    sub hyphen' @upper by hyphen.case;\n} calt;\n",
                "note": "The exception goes first. Within a lookup the first rule that matches wins, "
                "so an ignore after the rule it should stop never runs.",
            },
        },
    },
    "unreachable-imatra": {
        "font": "martel",
        # the font as it ships: its own GSUB does the Devanagari shaping
        "shipped": True,
        "text": "सिद्धि",
        "features": {},
        "variants": {
            # an excerpt of the source's pres feature; the real class lists 113 glyphs
            "written": {
                "label": "As shipped",
                "fea": "feature pres {\n    sub iMatra-deva' [ta-deva d-deva sa-deva na-deva] by iMatra-deva;\n"
                "    sub iMatra-deva' d-deva dha-deva by iMatra-deva.10;\n} pres;\n",
                "note": "The first rule keeps the short i-matra before most consonants, and its class "
                "includes half da. So before द्ध the longer i-matra drawn for it never appears.",
            },
            "meant": {
                "label": "Fixed",
                "patch": "imatra-first",
                "fea": "feature pres {\n    sub iMatra-deva' d-deva dha-deva by iMatra-deva.10;\n"
                "    sub iMatra-deva' [ta-deva d-deva sa-deva na-deva] by iMatra-deva;\n} pres;\n",
                "note": "The specific rule goes first, and the i-matra reaches over the whole conjunct.",
            },
        },
    },
    "syriac-nun": {
        "font": "idiqlat",
        # the font as it ships: its own GPOS spaces nun from a vowel below the alaph
        "shipped": True,
        "text": "ܐܷܢ ܐܸܢ",
        "rtl": True,
        "features": {},
        "variants": {
            # an excerpt of the source; the real class lists 18 marks below
            "written": {
                "label": "As shipped",
                "fea": "@below = [\n    zlamadottedhorizontal-syriac\n    rbasabelow-syriac\n    pthahabelow-syriac\n];\n\n"
                "lookup kern_nun_1 {\n    lookupflag RightToLeft;\n    pos nun-syriac.cv18 405;\n} kern_nun_1;\n\n"
                "feature kern {\n    pos alaph-syriac @below\n        nun-syriac' lookup kern_nun_1;\n} kern;\n",
                "note": "After an alaph, nun has already become its nun-syriac.cv18 form by the time "
                "positioning runs. The lookup only has a rule for that form, so the rule that calls it "
                "on nun-syriac never adds the space, and the vowel below the alaph crowds the nun.",
            },
            "meant": {
                "label": "Fixed",
                "patch": "nun-cv18",
                "fea": "@below = [\n    zlamadottedhorizontal-syriac\n    rbasabelow-syriac\n    pthahabelow-syriac\n];\n\n"
                "lookup kern_nun_1 {\n    lookupflag RightToLeft;\n    pos nun-syriac.cv18 405;\n} kern_nun_1;\n\n"
                "feature kern {\n    pos alaph-syriac @below\n        nun-syriac.cv18' lookup kern_nun_1;\n} kern;\n",
                "note": "The rule marks the form the nun has at that point, and the lookup's 405 units "
                "clear the vowel.",
            },
        },
    },
}

# every scene without its own languagesystem gets the default one
DEFAULT_SYSTEM = "languagesystem DFLT dflt;\n"


def subset(source: TTFont, glyphs: list[str]) -> TTFont:
    """A copy of `source` with only `glyphs` and no layout tables."""
    buffer = io.BytesIO()
    source.save(buffer)
    buffer.seek(0)
    font = TTFont(buffer)
    options = Options()
    options.layout_features = []
    options.glyph_names = True
    options.notdef_outline = True
    subsetter = Subsetter(options)
    subsetter.populate(glyphs=glyphs)
    subsetter.subset(font)
    for table in ("GSUB", "GPOS", "GDEF", "kern"):
        if table in font:
            del font[table]
    return font


def imatra_first(font: TTFont) -> None:
    """Move the rule for i-matra before half da + dha ahead of the general rule
    that catches it first, in the font's compiled `pres` lookup."""
    gsub = font["GSUB"].table
    lookups = {i for r in gsub.FeatureList.FeatureRecord if r.FeatureTag == "pres" for i in r.Feature.LookupListIndex}

    def target(subtable):
        out = set()
        for record in subtable.SubstLookupRecord:
            for single in gsub.LookupList.Lookup[record.LookupListIndex].SubTable:
                out.add(getattr(single, "mapping", {}).get("uni093F"))
        return out

    for index in sorted(lookups):
        lookup = gsub.LookupList.Lookup[index]
        if lookup.LookupType != 6:
            continue
        subtables = lookup.SubTable
        general = specific = None
        for i, st in enumerate(subtables):
            if st.Format != 3 or "uni093F" not in st.InputCoverage[0].glyphs:
                continue
            ahead = [c.glyphs for c in st.LookAheadCoverage]
            if len(ahead) == 1 and "uni0926094D" in ahead[0] and target(st) == {"uni093F"}:
                general = i
            if ahead == [["uni0926094D"], ["uni0927"]] and target(st) == {"uni093F.10"}:
                specific = i
        if general is not None and specific is not None:
            assert general < specific, "the specific rule already comes first"
            subtables.insert(general, subtables.pop(specific))
            lookup.SubTableCount = len(subtables)
            return
    raise SystemExit("the i-matra rules weren't found in this font's pres feature")


def nun_cv18(font: TTFont) -> None:
    """Mark the nun.cv18 form in the compiled rule that spaces nun from a vowel below
    the alaph before it, as the lookup it calls only has a rule for that form."""
    for lookup in font["GPOS"].table.LookupList.Lookup:
        for subtable in lookup.SubTable:
            st = getattr(subtable, "ExtSubTable", subtable)
            if getattr(st, "Format", 0) != 3 or not hasattr(st, "InputCoverage"):
                continue
            if st.InputCoverage[0].glyphs == ["uni0722"] and "uni0710" in st.BacktrackCoverage[-1].glyphs:
                st.InputCoverage[0].glyphs = ["uni0722.cv18"]
                return
    raise SystemExit("the nun rule wasn't found in this font's GPOS")


def later_pairs(font: TTFont) -> None:
    """Drop the first of each pair the kern lookup lists twice, so the later value applies."""
    dropped = 0
    for lookup in font["GPOS"].table.LookupList.Lookup:
        if lookup.LookupType != 2:
            continue
        for subtable in lookup.SubTable:
            if subtable.Format != 1:
                continue
            for pairs in subtable.PairSet:
                records = pairs.PairValueRecord
                later = {r.SecondGlyph: i for i, r in enumerate(records)}
                kept = [r for i, r in enumerate(records) if later[r.SecondGlyph] == i]
                dropped += len(records) - len(kept)
                pairs.PairValueRecord = kept
                pairs.PairValueCount = len(kept)
    if not dropped:
        raise SystemExit("no pair is listed twice in this font's GPOS")



PATCHES = {"imatra-first": imatra_first, "nun-cv18": nun_cv18, "later-pairs": later_pairs}


def copy(source: TTFont) -> TTFont:
    buffer = io.BytesIO()
    source.save(buffer)
    buffer.seek(0)
    return TTFont(buffer)


def compile_with_fontTools(font: TTFont, fea: str) -> None:
    addOpenTypeFeaturesFromString(font, fea)


def compile_with_fea_rs(font: TTFont, fea: str, feanor: str) -> None:
    """Compile with fea-rs and put its layout tables into `font`."""
    with tempfile.TemporaryDirectory() as tmp:
        tmp = pathlib.Path(tmp)
        (tmp / "features.fea").write_text(fea)
        # the font's glyph order first, so fea-rs's glyph ids match the font's
        (tmp / "glyphs.txt").write_text("\n".join(font.getGlyphOrder()) + "\n")
        subprocess.run(
            [feanor, "dev", "compile", str(tmp / "features.fea"), "--glyphs", str(tmp / "glyphs.txt"),
             "--out", str(tmp / "out.bin"), "--exact-order"],
            check=True,
            capture_output=True,
        )
        compiled = TTFont(str(tmp / "out.bin"))
        for table in ("GSUB", "GPOS", "GDEF"):
            if table in compiled:
                font[table] = compiled[table]


def shape(font: TTFont, text: str, features: dict, script: str | None) -> list[dict]:
    """HarfBuzz's glyphs and positions for `text`, with each glyph's outline."""
    data = io.BytesIO()
    font.save(data)
    hb_font = hb.Font(hb.Face(data.getvalue()))
    buf = hb.Buffer()
    buf.add_str(text)
    buf.guess_segment_properties()
    if script:
        buf.script = script
    hb.shape(hb_font, buf, features)
    order = font.getGlyphOrder()
    glyph_set = font.getGlyphSet()
    out = []
    x = 0
    for info, pos in zip(buf.glyph_infos, buf.glyph_positions):
        name = order[info.codepoint]
        pen = SVGPathPen(glyph_set)
        glyph_set[name].draw(pen)
        out.append({"name": name, "d": pen.getCommands(), "x": x + pos.x_offset, "y": pos.y_offset,
                    "advance": pos.x_advance, "cluster": info.cluster})
        x += pos.x_advance
    return out


def focus(fonts: dict, variants: dict, first: int, last: int) -> list[float]:
    """The extent of glyphs `first` to `last` in every version, in font units:
    left, right, bottom, top."""
    xs, ys = [], []
    for key, variant in variants.items():
        glyph_set = fonts[key].getGlyphSet()
        for g in variant["glyphs"][first : last + 1]:
            pen = BoundsPen(glyph_set)
            glyph_set[g["name"]].draw(pen)
            if pen.bounds:
                xs += [g["x"] + pen.bounds[0], g["x"] + pen.bounds[2]]
                ys += [g["y"] + pen.bounds[1], g["y"] + pen.bounds[3]]
    return [min(xs), max(xs), min(ys), max(ys)]


def main() -> None:
    args = argparse.ArgumentParser()
    args.add_argument("--inter", required=True)
    args.add_argument("--martel", required=True)
    args.add_argument("--idiqlat", required=True)
    args.add_argument("--coiny", required=True)
    args.add_argument("--extra", action="append", default=[], metavar="NAME=PATH",
                      help="a prebuilt font a scene or variant names, e.g. playwrite-before=pw.ttf")
    args.add_argument("--feanor", default="feanor")
    opts = args.parse_args()

    inter = instantiateVariableFont(TTFont(opts.inter), {"wght": 700, "opsz": 32}, inplace=False)
    fonts = {"inter": (inter, INTER), "martel": (TTFont(opts.martel), MARTEL),
             "idiqlat": (TTFont(opts.idiqlat), IDIQLAT), "coiny": (TTFont(opts.coiny), COINY)}
    for item in opts.extra:
        key, path = item.split("=", 1)
        fonts[key] = (TTFont(path), PLAYWRITE if key.startswith("playwrite") else path)
    OUT.mkdir(parents=True, exist_ok=True)

    for name, scene in SCENES.items():
        source, credit = fonts[scene["font"]]
        variants = {}
        fonts_used = {}
        for key, variant in scene["variants"].items():
            fea = variant["fea"]
            if "languagesystem" not in fea:
                fea = DEFAULT_SYSTEM + fea
            if scene.get("shipped"):
                font = copy(fonts[variant["font"]][0] if "font" in variant else source)
                if "patch" in variant:
                    PATCHES[variant["patch"]](font)
            else:
                font = subset(source, scene["glyphs"])
                if variant.get("compiler") == "fea-rs":
                    compile_with_fea_rs(font, fea, opts.feanor)
                else:
                    compile_with_fontTools(font, fea)
            fonts_used[key] = font
            variants[key] = {
                "label": variant["label"],
                "note": variant.get("note", ""),
                "fea": fea,
                "glyphs": shape(font, scene["text"], scene["features"], scene.get("script")),
            }
        hhea = source["hhea"]
        hmtx = source["hmtx"]
        result = {
            "text": scene["text"],
            "widths": {g: hmtx[g][0] for g in scene.get("glyphs", []) if g in hmtx.metrics},
            "unitsPerEm": source["head"].unitsPerEm,
            "ascender": hhea.ascent,
            # some fonts store the descender as a positive number
            "descender": -abs(hhea.descent),
            "font": credit,
            **variants,
        }
        if scene.get("rtl"):
            # right to left: both versions end at the same place, where the text starts
            ends = {k: v["glyphs"][-1]["x"] + v["glyphs"][-1]["advance"] for k, v in variants.items()}
            for k, v in variants.items():
                for g in v["glyphs"]:
                    g["x"] += max(ends.values()) - ends[k]
            result["rtl"] = True
        if "focus" in scene:
            result["focus"] = focus(fonts_used, variants, *scene["focus"])
        (OUT / f"{name}.json").write_text(json.dumps(result, indent=1) + "\n")
        summary = {k: " ".join(g["name"] for g in v["glyphs"]) for k, v in variants.items()}
        print(f"{name}: {summary}")


if __name__ == "__main__":
    main()
