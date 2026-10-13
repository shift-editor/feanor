"""Build the data for the "what your font does" scenes on the landing page.

For each scene, compile its feature code twice into a subset of a real font,
"as written" and "as meant", shape the sample text with HarfBuzz, and save each
glyph's outline and position. The page only animates between the two results,
so what it shows is HarfBuzz's real output.

Usage (from site/):
  nix shell --impure --expr '(import<nixpkgs>{}).python3.withPackages(p:[p.fonttools p.uharfbuzz])' \
    --command python3 scenes/build.py "$(nix build --no-link --print-out-paths nixpkgs#inter)/share/fonts/truetype/InterVariable.ttf"

Writes src/scenes/<scene>.json.
"""

import io
import json
import pathlib
import sys

import uharfbuzz as hb
from fontTools.feaLib.builder import addOpenTypeFeaturesFromString
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.subset import Options, Subsetter
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

OUT = pathlib.Path(__file__).resolve().parent.parent / "src" / "scenes"

SCENES = {
    "duplicate-pair": {
        "text": "Tokyo",
        "glyphs": ["T", "o", "k", "y"],
        # the designer corrected T o further down; compilers keep the first value
        "written": "languagesystem DFLT dflt;\nfeature kern {\n    pos T o -180;\n    pos T y -80;\n    pos T o -60;\n} kern;\n",
        "meant": "languagesystem DFLT dflt;\nfeature kern {\n    pos T y -80;\n    pos T o -60;\n} kern;\n",
    },
}


def font_for(source: TTFont, glyphs: list[str], fea: str) -> TTFont:
    """A subset of `source` with only `glyphs`, its layout replaced by `fea`."""
    font = TTFont()
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
    addOpenTypeFeaturesFromString(font, fea)
    return font


def shape(font: TTFont, text: str) -> list[dict]:
    """HarfBuzz's glyphs and positions for `text`, with each glyph's outline."""
    data = io.BytesIO()
    font.save(data)
    face = hb.Face(data.getvalue())
    hb_font = hb.Font(face)
    buf = hb.Buffer()
    buf.add_str(text)
    buf.guess_segment_properties()
    hb.shape(hb_font, buf, {"kern": True, "liga": True})
    order = font.getGlyphOrder()
    glyph_set = font.getGlyphSet()
    out = []
    x = 0
    for info, pos in zip(buf.glyph_infos, buf.glyph_positions):
        name = order[info.codepoint]
        pen = SVGPathPen(glyph_set)
        glyph_set[name].draw(pen)
        out.append(
            {
                "name": name,
                "d": pen.getCommands(),
                "x": x + pos.x_offset,
                "y": pos.y_offset,
                "advance": pos.x_advance,
            }
        )
        x += pos.x_advance
    return out


def main() -> None:
    source = TTFont(sys.argv[1])
    instance = instantiateVariableFont(source, {"wght": 700, "opsz": 32}, inplace=False)
    OUT.mkdir(parents=True, exist_ok=True)
    for name, scene in SCENES.items():
        variants = {}
        for variant in ("written", "meant"):
            font = font_for(instance, scene["glyphs"], scene[variant])
            variants[variant] = {"fea": scene[variant], "glyphs": shape(font, scene["text"])}
        head = instance["head"]
        hhea = instance["hhea"]
        hmtx = instance["hmtx"]
        result = {
            "text": scene["text"],
            # each glyph's own advance, before kerning
            "widths": {g: hmtx[g][0] for g in scene["glyphs"]},
            "unitsPerEm": head.unitsPerEm,
            "ascender": hhea.ascent,
            "descender": hhea.descent,
            "font": "Inter Bold, © 2016 The Inter Project Authors, SIL Open Font License 1.1",
            **variants,
        }
        (OUT / f"{name}.json").write_text(json.dumps(result, indent=1) + "\n")
        print(f"{name}: written {[g['x'] for g in variants['written']['glyphs']]}, "
              f"meant {[g['x'] for g in variants['meant']['glyphs']]}")


if __name__ == "__main__":
    main()
