// A TextMate grammar for OpenType feature files, for Shiki: the code blocks in
// rule documentation and the diffs. Scopes are the common ones themes colour
// (keyword, entity.name.function, variable, constant, string, comment), so the
// highlighting follows the theme. It matches the CodeMirror language in
// fea-language.ts token for token.
import type { LanguageRegistration } from "shiki";

// a glyph name, tag or lookup name: anything FEA allows in a bare name
const NAME = "[A-Za-z0-9_.\\-*+:^|~]+";
// not part of a longer name on either side
const word = (alternatives: string) => `(?<![\\w.\\-\\\\])(?:${alternatives})(?![\\w.\\-])`;

const KEYWORDS = [
  "feature lookup sub substitute rsub reversesub pos position enum enumerate ignore by from",
  "script language languagesystem lookupflag markClass table include anchor anchorDef",
  "valueRecordDef useExtension exclude_dflt include_dflt excludeDFLT includeDFLT",
  "cursive base ligature mark ligComponent featureNames name sizemenuname parameters",
  "cvParameters FeatUILabelNameID FeatUITooltipTextNameID SampleTextNameID",
  "ParamUILabelNameID Character conditionset variation contourpoint device NULL",
]
  .join(" ")
  .split(" ");

const FLAGS = "RightToLeft IgnoreBaseGlyphs IgnoreLigatures IgnoreMarks MarkAttachmentType UseMarkFilteringSet";

export const feaGrammar: LanguageRegistration = {
  name: "fea",
  scopeName: "source.fea",
  displayName: "OpenType feature file",
  aliases: ["feature"],
  repository: {},
  patterns: [
    { name: "comment.line.number-sign.fea", match: "#.*$" },
    { name: "string.quoted.double.fea", match: '"[^"]*"?' },
    // `feature smcp {`, `lookup name {`, `script latn;`, `language TRK;`: the name after the keyword
    {
      match: `${word("feature|lookup|script|language")}(\\s+)(${NAME})`,
      captures: {
        0: { name: "keyword.control.fea" },
        2: { name: "entity.name.function.fea" },
      },
    },
    // `} smcp;`: the name closing a block
    {
      match: `(\\})(\\s*)(${NAME})(\\s*)(;)`,
      captures: {
        1: { name: "punctuation.section.block.end.fea" },
        3: { name: "entity.name.function.fea" },
        5: { name: "punctuation.terminator.fea" },
      },
    },
    { name: "variable.other.class.fea", match: "@[\\w.\\-]+" },
    { name: "keyword.control.fea", match: word(KEYWORDS.join("|")) },
    { name: "constant.language.flag.fea", match: word(FLAGS.split(" ").join("|")) },
    { name: "constant.numeric.fea", match: "(?<![\\w.\\-])-?\\d+(?:\\.\\d+)?(?![\\w.\\-])" },
    { name: "keyword.operator.mark.fea", match: "'" },
    { name: "keyword.operator.range.fea", match: "(?<=\\s)-(?=\\s)" },
    { name: "keyword.operator.expression.fea", match: "\\$[\\[{]?" },
    { name: "punctuation.fea", match: "[{}\\[\\]<>();,=]" },
  ],
};
