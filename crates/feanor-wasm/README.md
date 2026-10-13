# @shift-editor/feanor

[Feanor](https://github.com/shift-editor/feanor), the formatter and linter for
OpenType feature files (`.fea`), compiled to WebAssembly for browsers and Node.

```js
import init, { format, lint, Document } from "@shift-editor/feanor";

await init(); // fetches feanor_bg.wasm next to feanor.js

format("feature liga {\nsub f i by f_i;\n} liga;\n").text;
// "feature liga {\n    sub f i by f_i;\n} liga;\n"

lint("@lc = [a b a];\n", { glyphs: ["a", "b"] });
// [{ code: "C002", rule: "duplicate-glyph-in-class", severity: "warning",
//    message: "'a' is already in this class", from: 11, to: 12, ... }]

// live linting in an editor: apply each change, then read diagnostics
const doc = new Document(source, { glyphs });
doc.edit(from, to, insert);
doc.diagnostics();
```

Positions (`from`, `to`, edit ranges) are UTF-16 offsets into the string you
passed, as JavaScript strings and CodeMirror use them. Diagnostics also carry
zero-based `start` / `end` line and character positions.

In Node, pass the module bytes instead of fetching:

```js
import { readFileSync } from "node:fs";
import { initSync } from "@shift-editor/feanor";
initSync({ module: readFileSync(new URL("./node_modules/@shift-editor/feanor/feanor_bg.wasm", import.meta.url)) });
```

Also exported: `fix(source, options)` applies safe fixes (or all with
`unsafe: true`), `rules()` lists the rules, `explain("G001")` returns a rule's
documentation, `version()` the Feanor version.

Licensed under either Apache-2.0 or MIT, at your option.
