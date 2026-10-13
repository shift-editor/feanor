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
// keywords followed by a name: a tag, or a lookup's name
const NAMED = new Set(["feature", "lookup", "script", "language"]);
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
    if (stream.match(/^[{}\[\]<>();,=']/)) {
      // `} smcp;`: the name after a closing brace is the block's tag
      state.afterFeature = stream.current() === "}";
      return "punctuation";
    }
    if (stream.match(/^\$[\[{]?/)) return "operator";
    if (stream.match(/^\\?[\w.\-*+:^|~]+/)) {
      const word = stream.current().replace(/^\\/, "");
      if (state.afterFeature) {
        state.afterFeature = false;
        return "tagName";
      }
      if (KEYWORDS.has(word)) {
        state.afterFeature = NAMED.has(word);
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

// Pierre's light theme colours for the same scopes, so the editors match the
// diffs and the highlighted code blocks (see fea-grammar.ts).
export const feaHighlighting = syntaxHighlighting(
  HighlightStyle.define([
    { tag: tags.comment, color: "#737373" },
    { tag: tags.keyword, color: "#d32a61" },
    { tag: tags.className, color: "#d47628" },
    { tag: tags.tagName, color: "#693acf" },
    { tag: tags.number, color: "#1ca1c7" },
    { tag: tags.string, color: "#199f43" },
    { tag: tags.atom, color: "#1ca1c7" },
    { tag: tags.operator, color: "#636363" },
    { tag: tags.punctuation, color: "#636363" },
  ]),
);
