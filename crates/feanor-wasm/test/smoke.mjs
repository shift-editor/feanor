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
  assert.equal(rules().length, 11);
  assert.match(explain("G001"), /^# G001 glyph-not-in-font/);
  assert.equal(explain("nope"), undefined);
  assert.match(version(), /^\d+\.\d+\.\d+/);
});

test("rejects bad options with a message", () => {
  assert.throws(() => format("", { indent: "wide" }), /indent must be/);
  assert.throws(() => lint("", { glyph: ["a"] }), /invalid lint options/);
});
