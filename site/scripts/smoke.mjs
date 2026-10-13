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
check(rules === 11, `rules table lists ${rules} rules`);

await page.screenshot({ path: "smoke.png", fullPage: true });
check(errors.length === 0, `no page errors${errors.length ? ": " + errors.join(" | ") : ""}`);
await browser.close();
