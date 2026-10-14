// End-to-end test of the built package (run ./build.sh first): node test/smoke.mjs
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const pkg = new URL("../pkg/", import.meta.url);
const feanor = await import(new URL("feanor.js", pkg));
feanor.initSync({ module: readFileSync(new URL("feanor_bg.wasm", pkg)) });
const { format, lint, fix, rules, explain, version, Document } = feanor;

test("formats a file and keeps tokens", () => {
  const out = format("feature liga {\nsub f i by f_i;\n} liga;\n");
  assert.equal(out.text, "feature liga {\n    sub f i by f_i;\n} liga;\n");
  assert.deepEqual(out.untouched, []);
  assert.equal(out.featureBody, false);
});

test("takes format options", () => {
  const out = format("feature liga {\nsub f i by f_i;\n} liga;\n", { indent: 2 });
  assert.equal(out.text, "feature liga {\n  sub f i by f_i;\n} liga;\n");
  assert.equal(format("sub a   by b;\n").featureBody, true);
});

test("reports positions as UTF-16 offsets", () => {
  // the comment holds an emoji (two UTF-16 units, four UTF-8 bytes) before the problem
  const source = "# 😀 caps\n@lc = [a b a];\n";
  const [d] = lint(source, { glyphs: ["a", "b"] }).filter((d) => d.code === "C002");
  assert.equal(source.slice(d.from, d.to), "a");
  assert.equal(d.rule, "duplicate-glyph-in-class");
  assert.equal(d.severity, "warning");
  assert.deepEqual(d.start, { line: 1, character: 11 });
  assert.equal(d.fixes[0].applicability, "safe");
  assert.equal(source.slice(d.related.from, d.related.to), "a");
});

test("suggests glyph names and applies only safe fixes by default", () => {
  const source = "@lc = [a b b];\nfeature smcp { sub agrve by a; } smcp;\n";
  const glyphs = ["a", "b", "agrave"];
  const missing = lint(source, { glyphs }).find((d) => d.code === "G001");
  assert.match(missing.message, /did you mean 'agrave'/);
  assert.equal(fix(source, { glyphs }), "@lc = [a b];\nfeature smcp { sub agrve by a; } smcp;\n");
  assert.equal(fix(source, { glyphs, unsafe: true }), "@lc = [a b];\nfeature smcp { sub agrave by a; } smcp;\n");
});

test("selects rules and honours noqa", () => {
  const source = "@a = [x x]; # noqa\n@b = [y y];\n";
  assert.equal(lint(source).filter((d) => d.code === "C002").length, 1);
  assert.equal(lint(source, { noqa: false }).filter((d) => d.code === "C002").length, 2);
  // the bare `# noqa` also hides line 1's unused-class hint
  assert.deepEqual(lint(source, { select: ["C004"] }).map((d) => d.code), ["C004"]);
  assert.deepEqual(lint(source, { select: ["C004"], noqa: false }).map((d) => d.code), ["C004", "C004"]);
  assert.equal(lint(source, { ignore: ["C"] }).length, 0);
});

test("a document stays in step with lint() as it's edited", () => {
  const doc = new Document("# 😀\n@a = [x y];\n", {});
  doc.edit(14, 14, " x"); // after "y": "# " 2 + emoji 2 + "\n@a = [x y" 10
  assert.equal(doc.text, "# 😀\n@a = [x y x];\n");
  const live = doc.diagnostics().map((d) => [d.code, d.from, d.to]);
  const full = lint(doc.text).map((d) => [d.code, d.from, d.to]);
  assert.deepEqual(live, full);
  assert.ok(live.some(([code]) => code === "C002"));
  doc.edit(14, 16, "");
  assert.equal(doc.text, "# 😀\n@a = [x y];\n");
});

test("explains rules", () => {
  assert.equal(rules().length, 21);
  assert.match(explain("G001"), /^# G001 glyph-not-in-font/);
  assert.equal(explain("nope"), undefined);
  assert.match(version(), /^\d+\.\d+\.\d+/);
});

test("rejects bad options with a message", () => {
  assert.throws(() => format("", { indent: "wide" }), /indent must be/);
  assert.throws(() => lint("", { glyph: ["a"] }), /invalid lint options/);
});

test("format options: line width, tab indent, hand alignment", () => {
  assert.equal(
    format("@a = [aaaa bbbb cccc dddd eeee];\n", { lineWidth: 20 }).text,
    "@a = [\n    aaaa bbbb cccc\n    dddd eeee\n];\n",
  );
  assert.equal(format("feature a {\nsub a by b;\n} a;\n", { indent: "tab" }).text, "feature a {\n\tsub a by b;\n} a;\n");
  const aligned = "feature a {\n  sub A    by A.c;\n  sub BB   by BB.c;\n} a;\n";
  assert.equal(format(aligned).text, "feature a {\n  sub A  by A.c;\n  sub BB by BB.c;\n} a;\n");
  assert.equal(format(aligned, { keepAlignment: false }).text, "feature a {\n  sub A by A.c;\n  sub BB by BB.c;\n} a;\n");
});

test("keeps the file's line endings", () => {
  const out = format("feature liga {\r\nsub f i by f_i;\r\n} liga;\r\n").text;
  assert.equal(out, "feature liga {\r\n    sub f i by f_i;\r\n} liga;\r\n");
});

test("reports what it left alone as UTF-16 spans", () => {
  const source = "# 😀\nfeature a {\n# fmt: off\nsub a   by b;\n# fmt: on\n} a;\n";
  const { untouched } = format(source);
  assert.equal(untouched.length, 1);
  assert.match(source.slice(untouched[0].from, untouched[0].to), /^# fmt: off\nsub a {3}by b;/);
  const broken = "# 😀\nfeature a {\nsub a by ;\n} a;\n";
  const out = format(broken);
  assert.equal(out.text, broken);
  assert.equal(broken.slice(out.untouched[0].from, out.untouched[0].to).startsWith("feature a {"), true);
});

test("never throws on half-typed code, and formatting twice changes nothing", () => {
  const source =
    "languagesystem DFLT dflt;\n# 😀 ö\n@lc = [a b c];\nfeature calt {\n  lookup L {\n    sub a' b by c;\n  } L;\n" +
    "  sub [a b]' lookup L c;\n  pos a b -20;\n} calt;\n";
  for (let cut = 0; cut <= source.length; cut++) {
    const text = source.slice(0, cut);
    const once = format(text).text;
    assert.equal(format(once).text, once, `not idempotent when cut at ${cut}`);
    lint(text, { glyphs: ["a", "b", "c"] });
  }
});

test("fix edits applied in JS give the same text as fix()", () => {
  const source = "# 😀\n@lc = [a b b];;\nfeature kern {\n  pos T o -180;\n  pos T o -60;\n} kern;\n";
  const safe = lint(source)
    .flatMap((d) => d.fixes.filter((f) => f.applicability === "safe"))
    .flatMap((f) => f.edits)
    .sort((a, b) => b.from - a.from);
  let text = source;
  for (const e of safe) text = text.slice(0, e.from) + e.insert + text.slice(e.to);
  assert.equal(text, fix(source));
  assert.equal(text, "# 😀\n@lc = [a b];\nfeature kern {\n  pos T o -180;\n} kern;\n");
});

test("rules about lookups reach JS with their fixes", () => {
  const kern = "feature kern {\n  pos T o -180;\n  pos T o -60;\n} kern;\n";
  const [pair] = lint(kern).filter((d) => d.code === "R003");
  assert.equal(pair.rule, "duplicate-pair");
  assert.equal(kern.slice(pair.related.from, pair.related.to), "pos T o -180;");

  const late = "languagesystem DFLT dflt;\nfeature a { sub a by b; } a;\nlanguagesystem latn dflt;\n";
  const [system] = lint(late).filter((d) => d.code === "E003");
  assert.deepEqual(system.fixes.map((f) => f.applicability), ["unsafe"]);
  assert.equal(fix(late), late);
  assert.equal(fix(late, { unsafe: true }), "languagesystem DFLT dflt;\nlanguagesystem latn dflt;\nfeature a { sub a by b; } a;\n");

  const conflict = "feature ss01 {\n  sub a by a.1;\n  sub a by a.sc;\n} ss01;\n";
  assert.deepEqual(lint(conflict).map((d) => [d.code, d.severity]), [["R002", "error"]]);
});

test("a bare include is an error, not a crash", () => {
  const [d] = lint("include foo.fea;\n");
  assert.equal(d.code, "E001");
  assert.match(d.message, /include\(path\/to\/file\.fea\)/);
});

test("project mode makes an undefined class an error", () => {
  assert.equal(lint("sub @X by a;\n").length, 0);
  assert.deepEqual(lint("sub @X by a;\n", { project: true }).map((d) => [d.code, d.severity]), [["C003", "error"]]);
});

test("a document stays in step with lint() through random edits", () => {
  // a fixed-seed generator, so a failure always replays the same edits
  let seed = 7;
  const random = (n) => {
    seed = (seed * 1103515245 + 12345) % 2147483648;
    return seed % n;
  };
  const pieces = [" ", "\n", "a", "b", ";", "[", "]", "@x", "😀", "ö", "sub ", " by ", "pos ", "feature ", "} ", "{ ", "#"];
  const options = { glyphs: ["a", "b"] };
  const doc = new Document("@x = [a b];\nfeature liga {\n  sub a b by a;\n} liga;\n", options);
  for (let step = 0; step < 400; step++) {
    const text = doc.text;
    // ends on character boundaries: never inside an emoji's surrogate pair
    const boundary = (i) => (i > 0 && i < text.length && /[\uDC00-\uDFFF]/.test(text[i]) ? i - 1 : i);
    const from = boundary(random(text.length + 1));
    const to = boundary(Math.min(text.length, from + random(4)));
    const insert = random(3) === 0 ? "" : pieces[random(pieces.length)];
    doc.edit(from, Math.max(from, to), insert);
    assert.equal(doc.text, text.slice(0, from) + insert + text.slice(Math.max(from, to)), `text at step ${step}`);
    if (step % 10 === 0) {
      const live = doc.diagnostics().map((d) => [d.code, d.from, d.to, d.message]);
      const full = lint(doc.text, options).map((d) => [d.code, d.from, d.to, d.message]);
      assert.deepEqual(live, full, `diagnostics at step ${step}`);
    }
  }
});

test("document edits are clamped to the text", () => {
  const doc = new Document("abc", {});
  doc.edit(2, 99, "x");
  assert.equal(doc.text, "abx");
  assert.throws(() => doc.edit(2, 1, ""), /after/);
});

test("every rule is explained by code and by name", () => {
  for (const rule of rules()) {
    assert.match(rule.code, /^[A-Z]\d{3}$/);
    assert.ok(rule.summary.length > 0, rule.code);
    assert.match(explain(rule.code), new RegExp(`^# ${rule.code} ${rule.name}`));
    assert.equal(explain(rule.name), explain(rule.code));
  }
});
