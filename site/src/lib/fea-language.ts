// Syntax highlighting for OpenType feature files, as a CodeMirror stream language.
import { HighlightStyle, StreamLanguage, syntaxHighlighting } from "@codemirror/language";
import { tags } from "@lezer/highlight";

const KEYWORDS = new Set(
  (
    "feature lookup sub substitute rsub reversesub pos position enum enumerate ignore by from " +
    "script language languagesystem lookupflag markClass table include anchor anchorDef " +
    "valueRecordDef useExtension exclude_dflt include_dflt excludeDFLT includeDFLT " +
    "cursive base ligature mark ligComponent featureNames name sizemenuname parameters " +
    "cvParameters FeatUILabelNameID FeatUITooltipTextNameID SampleTextNameID " +
    "ParamUILabelNameID Character conditionset variation contourpoint device NULL"
  ).split(" "),
);
const FLAGS = new Set(
  "RightToLeft IgnoreBaseGlyphs IgnoreLigatures IgnoreMarks MarkAttachmentType UseMarkFilteringSet".split(" "),
);

export const fea = StreamLanguage.define<{ afterFeature: boolean }>({
  name: "fea",
  startState: () => ({ afterFeature: false }),
  token(stream, state) {
    if (stream.eatSpace()) return null;
    if (stream.match(/^#.*/)) return "comment";
    if (stream.match(/^"[^"]*"?/)) return "string";
    if (stream.match(/^@[\w.\-]+/)) return "className";
    if (stream.match(/^-?\d+(\.\d+)?/)) return "number";
    if (stream.match(/^[{}\[\]<>();,=']/)) return "punctuation";
    if (stream.match(/^\$[\[{]?/)) return "operator";
    if (stream.match(/^\\?[\w.\-*+:^|~]+/)) {
      const word = stream.current().replace(/^\\/, "");
      if (state.afterFeature) {
        state.afterFeature = false;
        return "tagName";
      }
      if (KEYWORDS.has(word)) {
        state.afterFeature = word === "feature" || word === "script" || word === "language";
        return "keyword";
      }
      if (FLAGS.has(word)) return "atom";
      return "variableName";
    }
    stream.next();
    return null;
  },
  languageData: { commentTokens: { line: "#" } },
});

export const feaHighlighting = syntaxHighlighting(
  HighlightStyle.define([
    { tag: tags.comment, color: "#8a8a85", fontStyle: "italic" },
    { tag: tags.keyword, color: "#0c6fc9", fontWeight: "600" },
    { tag: tags.className, color: "#a2470f" },
    { tag: tags.tagName, color: "#0a7d5a", fontWeight: "600" },
    { tag: tags.number, color: "#8b3fb5" },
    { tag: tags.string, color: "#0a7d5a" },
    { tag: tags.atom, color: "#8b3fb5" },
    { tag: tags.operator, color: "#a2470f" },
    { tag: tags.punctuation, color: "#737373" },
  ]),
);
