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

Usage (from site/):
  nix shell --impure --expr '(import<nixpkgs>{}).python3.withPackages(p:[p.fonttools p.uharfbuzz])' \\
    --command python3 scenes/build.py \\
      --inter "$(nix build --no-link --print-out-paths nixpkgs#inter)/share/fonts/truetype/InterVariable.ttf" \\
      --martel Martel-Bold.ttf \\
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
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.subset import Options, Subsetter
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

OUT = pathlib.Path(__file__).resolve().parent.parent / "src" / "scenes"

INTER = "Inter Bold, © 2016 The Inter Project Authors, SIL Open Font License 1.1"
MARTEL = "Martel Bold, © 2015 Dan Reynolds, © 2010–2015 Sorkin Type Co, SIL Open Font License 1.1"

SCENES = {
    "duplicate-pair": {
        "font": "inter",
        "text": "Tokyo",
        "glyphs": ["T", "o", "k", "y"],
        "features": {"kern": True},
        "variants": {
            # the designer corrected T o further down; compilers keep the first value
            "written": {
                "label": "As written",
                "fea": "feature kern {\n    pos T o -180;\n    pos T y -80;\n    pos T o -60;\n} kern;\n",
            },
            "meant": {
                "label": "Fixed",
                "fea": "feature kern {\n    pos T y -80;\n    pos T o -60;\n} kern;\n",
                "note": "The later value was the correction, so the first one goes. Feanor's automatic "
                "fix removes the ignored line instead, which keeps the font as it ships.",
            },
        },
    },
    "unreachable-rule": {
        "font": "inter",
        "text": "Lettuce",
        "glyphs": ["L", "e", "t", "u", "c", "t.1"],
        "features": {"calt": True},
        "variants": {
            # the exception comes after the rule it should stop
            "written": {
                "label": "As written",
                "fea": "feature calt {\n    sub t' by t.1;\n    ignore sub t' t;\n} calt;\n",
            },
            "meant": {
                "label": "Fixed",
                "fea": "feature calt {\n    ignore sub t' t;\n    sub t' by t.1;\n} calt;\n",
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


PATCHES = {"imatra-first": imatra_first}


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


def main() -> None:
    args = argparse.ArgumentParser()
    args.add_argument("--inter", required=True)
    args.add_argument("--martel", required=True)
    args.add_argument("--feanor", default="feanor")
    opts = args.parse_args()

    inter = instantiateVariableFont(TTFont(opts.inter), {"wght": 700, "opsz": 32}, inplace=False)
    fonts = {"inter": (inter, INTER), "martel": (TTFont(opts.martel), MARTEL)}
    OUT.mkdir(parents=True, exist_ok=True)

    for name, scene in SCENES.items():
        source, credit = fonts[scene["font"]]
        variants = {}
        for key, variant in scene["variants"].items():
            fea = variant["fea"]
            if "languagesystem" not in fea:
                fea = DEFAULT_SYSTEM + fea
            if scene.get("shipped"):
                font = copy(source)
                if "patch" in variant:
                    PATCHES[variant["patch"]](font)
            else:
                font = subset(source, scene["glyphs"])
                if variant.get("compiler") == "fea-rs":
                    compile_with_fea_rs(font, fea, opts.feanor)
                else:
                    compile_with_fontTools(font, fea)
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
            "descender": hhea.descent,
            "font": credit,
            **variants,
        }
        (OUT / f"{name}.json").write_text(json.dumps(result, indent=1) + "\n")
        summary = {k: " ".join(g["name"] for g in v["glyphs"]) for k, v in variants.items()}
        print(f"{name}: {summary}")


if __name__ == "__main__":
    main()
