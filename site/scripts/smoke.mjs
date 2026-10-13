// Load the built site in a headless browser and exercise each interactive part.
// Usage: pnpm build && pnpm start (in another shell), then node scripts/smoke.mjs [url]
import { chromium } from "playwright";

const url = process.argv[2] ?? "http://localhost:3000";
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
const errors = [];
page.on("pageerror", (e) => errors.push(e.message));
page.on("console", (m) => m.type() === "error" && errors.push(m.text()));

const check = (ok, what) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) process.exitCode = 1;
};

await page.goto(url, { waitUntil: "networkidle" });

// hero: lint markers appear once the WebAssembly module has loaded
await page.waitForSelector(".cm-lintRange", { timeout: 15000 });
const status = await page.locator("[aria-live=polite]").first().textContent();
check(/error/.test(status ?? ""), `playground reports problems: "${status}"`);

// hero: Show diff renders a diff, Format rewrites the editor
await page.getByRole("button", { name: "Show diff" }).click();
await page.waitForSelector("diffs-container, [data-diffs], pre", { timeout: 10000 });
check(true, "playground diff shown");
await page.getByRole("button", { name: "Format" }).click();
const firstEditorText = await page.locator(".cm-content").first().innerText();
check(firstEditorText.includes("@lowercase = [a b c d e];"), "Format rewrote the playground text");

// formatting demo: switch file and indent, diff stays rendered
await page.locator("select").first().selectOption("recursive");
await page.locator("select").nth(1).selectOption("4");
const formatNote = await page.locator("#formatting").innerText();
check(/Formatted in [\d.]+ ms/.test(formatNote), "format demo reports timing");

// lint demo: diagnostics listed, a safe fix applies
const lint = page.locator("#linting");
await lint.locator("li").first().waitFor();
const before = await lint.locator("li").count();
check(before >= 4, `lint demo lists ${before} diagnostics`);
await lint.getByRole("button", { name: /Remove the second/ }).click();
await page.waitForTimeout(400);
const after = await lint.locator("li").count();
check(after === before - 1, `safe fix removed one diagnostic (${before} -> ${after})`);
await lint.getByRole("button", { name: /C001 class-length-mismatch/ }).click();
check((await lint.innerText()).includes("Why it matters"), "rule explanation opens");

// rules: every rule listed, details expand
const rules = await page.locator("#rules summary").count();
check(rules === 21, `rules table lists ${rules} rules`);
const g001 = page.locator("#rules details").filter({ hasText: "glyph-not-in-font" });
await g001.locator("summary").click();
const feaSpans = await g001.locator(".code-block span[style*=color]").count();
check(feaSpans > 5, `rule docs highlight FEA (${feaSpans} coloured spans)`);

// the diffs highlight FEA too: keyword colour from Pierre's theme inside the diff
await page.locator("#formatting").scrollIntoViewIfNeeded();
const diffKeyword = await page.waitForFunction(() => {
  const roots = [...document.querySelectorAll("*")].filter((e) => e.shadowRoot).map((e) => e.shadowRoot);
  const spans = roots.flatMap((r) => [...r.querySelectorAll("span")]);
  // tokens are coloured through --diffs-token-light
  return spans.some((s) => ["sub", "feature", "lookup"].includes(s.textContent.trim()) && /#d32a61/i.test(s.getAttribute("style") ?? ""));
}, null, { timeout: 10000 }).then(() => true, () => false);
check(diffKeyword, "diffs highlight FEA keywords");

// consequences: a scene shaped by HarfBuzz, with the code switching on the toggle
const scene = page.locator("#consequences figure").first();
check((await scene.locator("svg path").count()) === 5, "scene draws the five glyphs of Tokyo");
await scene.hover();
await scene.getByRole("button", { name: "Fixed" }).click();
check(!(await scene.innerText()).includes("pos T o -180"), "fixed version shows the fixed code");
await scene.getByRole("button", { name: "As written" }).click();
check((await scene.innerText()).includes("R003 duplicate-pair"), "written version shows the R003 diagnostic");

// speed: two hatched charts, a tooltip on hover, a table view
const fast = page.locator("#fast");
const bars = await fast.locator("figure").count();
check(bars === 2, `speed section has ${bars} charts`);
await fast.getByText("Harmattan features.fea").first().hover();
check(await fast.getByRole("tooltip").isVisible(), "chart tooltip shows on hover");
check(!(await fast.getByText("Show as table").count()), "no visible table toggle");
check((await fast.locator(".sr-only table").first().textContent()).includes("65,379 lines"), "chart numbers available to screen readers");

// code samples are highlighted
const coloured = await page.locator("#ci .code-block span[style*=color], #anywhere .code-block span[style*=color]").count();
check(coloured > 20, `code blocks highlighted (${coloured} coloured spans)`);

// shift.graphics typography
const fonts = await page.evaluate(async () => {
  await document.fonts.ready;
  const h1 = getComputedStyle(document.querySelector("h1")).fontFamily;
  const lede = getComputedStyle(document.querySelector("#formatting p")).fontFamily;
  return { h1, lede, aktiv: document.fonts.check('700 54px "aktiv-grotesk"') && [...document.fonts].some((f) => f.family.includes("aktiv") && f.status === "loaded") };
});
check(fonts.h1.includes("aktiv-grotesk") && fonts.lede.includes("DM Sans"), `headings ${fonts.h1.split(",")[0]}, copy ${fonts.lede.split(",")[0]}`);
console.log(`info aktiv-grotesk loaded from Adobe Fonts here: ${fonts.aktiv}`);

await page.screenshot({ path: "smoke.png", fullPage: true });
check(errors.length === 0, `no page errors${errors.length ? ": " + errors.join(" | ") : ""}`);
await browser.close();
