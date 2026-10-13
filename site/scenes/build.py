"""Build the data for the "what ships" scenes on the landing page.

For each scene, compile each version of its feature code into a subset of a
real font, shape the sample text with HarfBuzz, and save each glyph's outline,
position and cluster. The page only animates between the results, so what it
shows is HarfBuzz's real output. A version can be compiled by fontTools (the
default) or by fea-rs, through `feanor dev compile`.

Usage (from site/):
  nix shell --impure --expr '(import<nixpkgs>{}).python3.withPackages(p:[p.fonttools p.uharfbuzz])' \\
    --command python3 scenes/build.py \\
      --inter "$(nix build --no-link --print-out-paths nixpkgs#inter)/share/fonts/truetype/InterVariable.ttf" \\
      --garamond "$(nix build --no-link --print-out-paths nixpkgs#eb-garamond)/share/fonts/opentype/EBGaramond12-Regular.otf" \\
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
GARAMOND = "EB Garamond 12, © 2010–2013 Georg Duffner, SIL Open Font License 1.1"

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
    "duplicate-substitution": {
        "font": "inter",
        "text": "banana",
        "glyphs": ["b", "a", "n", "a.1", "a.sc"],
        "features": {"ss01": True},
        "variants": {
            # one glyph, two results: the compilers disagree
            "written": {
                "label": "Built with fea-rs",
                "compiler": "fea-rs",
                "fea": "feature ss01 {\n    sub a by a.1;\n    sub a by a.sc;\n} ss01;\n",
                "note": "fea-rs silently keeps the last rule. fontTools refuses to build the same file: "
                "“Already defined substitution for \"a\"”.",
            },
            "meant": {
                "label": "Fixed",
                "fea": "feature ss01 {\n    sub a by a.1;\n} ss01;\n",
                "note": "One substitution per glyph in a lookup, and both compilers build the same font.",
            },
        },
    },
    "languagesystem-after-feature": {
        "font": "garamond",
        "text": "office",
        "glyphs": ["o", "f", "i", "c", "e", "f_f_i", "f_i", "c_t", "t"],
        "features": {"liga": True},
        "script": "Latn",
        "variants": {
            "written": {
                "label": "As written",
                "fea": "languagesystem DFLT dflt;\nfeature liga {\n    sub f f i by f_f_i;\n    sub f i by f_i;\n} liga;\n"
                "languagesystem latn dflt;\nfeature dlig {\n    sub c t by c_t;\n} dlig;\n",
                "note": "liga is registered only for the default script. Once dlig registers Latin, "
                "shapers look only there for Latin text, and the ligature is gone.",
            },
            "meant": {
                "label": "Fixed",
                "fea": "languagesystem DFLT dflt;\nlanguagesystem latn dflt;\nfeature liga {\n    sub f f i by f_f_i;\n"
                "    sub f i by f_i;\n} liga;\nfeature dlig {\n    sub c t by c_t;\n} dlig;\n",
                "note": "Every languagesystem before the first feature, as the specification requires.",
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
    args.add_argument("--garamond", required=True)
    args.add_argument("--feanor", default="feanor")
    opts = args.parse_args()

    inter = instantiateVariableFont(TTFont(opts.inter), {"wght": 700, "opsz": 32}, inplace=False)
    fonts = {"inter": (inter, INTER), "garamond": (TTFont(opts.garamond), GARAMOND)}
    OUT.mkdir(parents=True, exist_ok=True)

    for name, scene in SCENES.items():
        source, credit = fonts[scene["font"]]
        variants = {}
        for key, variant in scene["variants"].items():
            fea = variant["fea"]
            if "languagesystem" not in fea:
                fea = DEFAULT_SYSTEM + fea
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
            "widths": {g: hmtx[g][0] for g in scene["glyphs"] if g in hmtx.metrics},
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
