// Screenshots of the client for the README, at a desktop size and a
// phone size, from the ramp example: the assembly, a part with a feature
// selected, and the Docs dialog. Needs the document server on port 8080
// serving the built app (see "Running the document server" in the
// README) and Playwright's Chromium (PW_CHROMIUM for a pre-installed
// one). Run from apps/web:
//
//     node scripts/screenshots.cjs ../../docs/screenshots
const path = require("node:path");
const { chromium, devices } = require("@playwright/test");

const OUT = process.argv[2] ?? path.join(__dirname, "../../../docs/screenshots");
const DOC = path.join(__dirname, "../../../examples/ramp/out/ramp.okpart");
const SERVER = process.env.OK_SERVER ?? "http://localhost:8080";

const ready = (page) => page.waitForFunction(() => Boolean(window.offkilter && window.offkilter.summary), null, { timeout: 60_000 });
const click = (page, id) => page.evaluate((id) => document.getElementById(id).click(), id);
const fit = (page) => page.evaluate(() => window.offkilter.viewer.fitAll());

async function open(page) {
  page.on("dialog", (d) => d.accept(d.defaultValue()));
  await page.goto(`${SERVER}/`);
  await ready(page);
  await page.setInputFiles("#file-input", DOC);
  await page.waitForTimeout(3000);
}

/** Switches to the tab whose name ends in `name`, iso view, fitted. */
async function tab(page, name) {
  await page.evaluate((name) => {
    for (const b of document.querySelectorAll("#tabbar button")) if (b.textContent.trim().endsWith(name)) { b.click(); break; }
  }, name);
  await page.waitForTimeout(2500);
  await click(page, "btn-view-iso");
  await fit(page);
  await page.waitForTimeout(800);
}

async function selectFeature(page, index) {
  const items = page.locator("#feature-list li");
  await items.nth(Math.min(index, (await items.count()) - 1)).click();
  await page.waitForTimeout(800);
}

async function docs(page) {
  await click(page, "btn-docs");
  await page.waitForTimeout(600);
  if ((await page.locator("#docs-list li").count()) === 0) {
    await click(page, "docs-upload");
    await page.waitForTimeout(3000);
  }
}

(async () => {
  const browser = await chromium.launch({
    executablePath: process.env.PW_CHROMIUM,
    args: ["--use-gl=swiftshader", "--enable-unsafe-swiftshader"],
  });
  let ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 });
  let page = await ctx.newPage();
  await open(page);
  await tab(page, "ramp");
  await page.screenshot({ path: `${OUT}/desktop-ramp.png` });
  await tab(page, "lug_1");
  await selectFeature(page, 6);
  await page.screenshot({ path: `${OUT}/desktop-part.png` });
  await tab(page, "ramp");
  await docs(page);
  await page.screenshot({ path: `${OUT}/desktop-docs.png` });
  await ctx.close();

  ctx = await browser.newContext({ ...devices["iPhone 14"], viewport: { width: 390, height: 844 }, deviceScaleFactor: 2 });
  page = await ctx.newPage();
  await open(page);
  await tab(page, "ramp");
  await page.screenshot({ path: `${OUT}/phone-ramp.png` });
  await tab(page, "lug_1");
  await selectFeature(page, 6);
  await page.screenshot({ path: `${OUT}/phone-part.png` });
  await docs(page);
  await page.screenshot({ path: `${OUT}/phone-docs.png` });
  await ctx.close();
  await browser.close();
  console.log(`wrote 6 screenshots to ${OUT}`);
})().catch((e) => { console.error(e); process.exit(1); });
