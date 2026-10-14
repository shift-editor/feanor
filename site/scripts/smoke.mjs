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

/** Choose `option` in the Base UI select at `index` inside `scope`. */
async function choose(scope, index, option) {
  await scope.getByRole("combobox").nth(index).click();
  await page.getByRole("option", { name: option, exact: true }).click();
}

await page.goto(url, { waitUntil: "networkidle" });

// hero: lint markers appear once the WebAssembly module has loaded
await page.waitForSelector(".cm-lintRange", { timeout: 15000 });
const errorCount = await page.getByLabel(/^\d+ errors$/).first().getAttribute("aria-label");
check(/^[1-9]\d* errors$/.test(errorCount ?? ""), `playground reports problems: "${errorCount}"`);

// hero: Diff renders a diff, Format rewrites the editor
await page.getByRole("button", { name: "Diff", exact: true }).first().click();
await page.waitForSelector("diffs-container, [data-diffs]", { timeout: 10000 });
check(true, "playground diff shown");
await page.getByRole("button", { name: "Format", exact: true }).first().click();
const firstEditorText = await page.locator(".cm-content").first().innerText();
check(firstEditorText.includes("@lowercase = [a b c d e];"), "Format rewrote the playground text");

// formatting demo: every example formats in the browser, with each indent
const formatting = page.locator("#formatting");
await formatting.scrollIntoViewIfNeeded();
for (const example of ["Hand-edited over the years", "A table, half aligned", "Already tidy"]) {
  await choose(formatting, 0, example);
  await page.waitForTimeout(300);
  const note = await formatting.innerText();
  check(/Formatted in [\d.]+ ms/.test(note) && /\d+ → \d+ lines/.test(note), `format demo formats "${example}"`);
}
await choose(formatting, 1, "4 spaces");
check(/Formatted in [\d.]+ ms/.test(await formatting.innerText()), "format demo takes an indent");

// lint demo: problems listed, a safe fix applies, a rule explains itself
const problems = page.locator("#linting section[aria-label=Problems] > ul > li");
await problems.first().waitFor();
const before = await problems.count();
check(before >= 4, `lint demo lists ${before} problems`);
const duplicate = problems.filter({ hasText: "C002" }).first();
await duplicate.getByRole("button").first().click();
const opened = await duplicate.innerText();
check(
  (await duplicate.getByRole("button").first().getAttribute("aria-expanded")) === "true" && opened.includes("Remove the second"),
  "a problem opens with its fix and rule summary",
);
await page.locator("#linting").getByRole("button", { name: /^Remove the second/ }).first().click();
await page.waitForTimeout(400);
const after = await problems.count();
check(after === before - 1, `safe fix removed one problem (${before} -> ${after})`);

// consequences: scenes shaped by HarfBuzz, with the code switching on the toggle
const consequences = page.locator("#consequences");
const scenes = [
  ["A join its designer fixed", "R001", "Before the fix", "After the fix", 3],
  ["A ligature that adds a letter", "R002", "As shipped", "Fixed", 4],
  ["An i-matra that never reaches", "R001", "As shipped", "Fixed", 5],
  ["A gap that never opens", "R005", "As shipped", "Fixed", 7],
  ["Kerning written twice", "R003", "As shipped", "Later values", 4],
  ["An exception that comes too late", "R001", "As written", "Fixed", 11],
];
for (const [tab, code, written, fixed, glyphs] of scenes) {
  await consequences.getByRole("tab", { name: tab }).click();
  const scene = consequences.locator("figure").first();
  await scene.hover();
  // scenes switch versions on a timer; hovering pauses them, then show the written one
  const toggle = (name) => scene.getByRole("radio", { name }).or(scene.getByRole("button", { name })).first();
  await toggle(written).click();
  // both versions stay in the page for a steady layout; only the shown one is visible
  await scene.locator(`span:visible:text-is("${code}")`).first().waitFor({ timeout: 10000 });
  const drawn = await scene.locator("svg").first().locator("path").count();
  check(drawn === glyphs, `scene "${tab}" shows ${code} and draws ${drawn} glyphs`);
  check(/From a font|An illustration/.test(await scene.innerText()), `scene "${tab}" says where it comes from`);
  await toggle(fixed).click();
  check(!(await scene.locator(`span:visible:text-is("${code}")`).count()), `scene "${tab}" hides the diagnostic once fixed`);
}

// speed: hatched charts with a tooltip on hover, numbers for screen readers
const fast = page.locator("#fast");
await fast.getByText("Harmattan features.fea").first().hover();
check(await fast.getByRole("tooltip").first().isVisible(), "chart tooltip shows on hover");
check((await fast.locator(".sr-only table").first().textContent()).includes("65,379 lines"), "chart numbers available to screen readers");

// code samples are highlighted
const coloured = await page.locator("#ci span[style*=color], #anywhere span[style*=color]").count();
check(coloured > 20, `code blocks highlighted (${coloured} coloured spans)`);

await page.screenshot({ path: "smoke.png", fullPage: true });
check(errors.length === 0, `no page errors${errors.length ? ": " + errors.join(" | ") : ""}`);
await browser.close();
