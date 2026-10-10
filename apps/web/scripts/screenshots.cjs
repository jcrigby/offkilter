// Screenshots of the client for the README, at a desktop size and a
// phone size, from the example documents: the duplicator, the cart,
// the router lift and the puzzle top as assemblies or studios, the
// ramp's lug with its extrude selected, and the Docs dialog with the
// examples on the server. Needs the document server on port 8080
// serving the built app (see "Running the document server" in the
// README) and Playwright's Chromium (PW_CHROMIUM for a pre-installed
// one). Run from apps/web:
//
//     node scripts/screenshots.cjs ../../docs/screenshots
const path = require("node:path");
const { chromium, devices } = require("@playwright/test");

const ROOT = path.join(__dirname, "../../..");
const OUT = process.argv[2] ?? path.join(ROOT, "docs/screenshots");
const SERVER = process.env.OK_SERVER ?? "http://localhost:8080";
const doc = (example, file) => path.join(ROOT, "examples", example, "out", file);

/** What to shoot: the document, the tab to show (null keeps the first) and the file stem. */
const SHOTS = [
  { doc: doc("duplicator", "duplicator.okpart"), tab: "Duplicator", stem: "duplicator" },
  { doc: doc("ego-cart", "ego_cart.okpart"), tab: "Cart", stem: "cart" },
  { doc: doc("router-lift", "router_lift.okpart"), tab: "Lift assembly", stem: "lift" },
  { doc: doc("puzzle-top", "puzzle_top.okpart"), tab: null, stem: "puzzle" },
];
const RAMP = doc("ramp", "ramp.okpart");

const ready = (page) => page.waitForFunction(() => Boolean(window.offkilter && window.offkilter.summary), null, { timeout: 60_000 });
const click = (page, id) => page.evaluate((id) => document.getElementById(id).click(), id);
const fit = (page) => page.evaluate(() => window.offkilter.viewer.fitAll());

async function open(page, file) {
  await page.goto(`${SERVER}/`);
  await ready(page);
  await page.setInputFiles("#file-input", file);
  await page.waitForTimeout(3000);
}

/** Switches to the tab whose name ends in `name` (or stays), iso view, fitted. */
async function tab(page, name) {
  if (name) {
    await page.evaluate((name) => {
      for (const b of document.querySelectorAll("#tabbar button")) if (b.textContent.trim().endsWith(name)) { b.click(); break; }
    }, name);
    await page.waitForTimeout(4000);
  }
  await click(page, "btn-view-iso");
  await fit(page);
  await page.waitForTimeout(1000);
}

async function selectFeature(page, index) {
  const items = page.locator("#feature-list li");
  await items.nth(Math.min(index, (await items.count()) - 1)).click();
  await page.waitForTimeout(800);
}

/** Opens the Docs dialog and waits for its list. */
async function openDocs(page) {
  for (let i = 0; i < 3; i++) {
    await click(page, "btn-docs");
    await page.waitForTimeout(800);
    if (await page.evaluate(() => document.getElementById("docs-dialog").open)) return;
  }
  throw new Error("the Docs dialog did not open");
}

/** Puts the open document on the server unless one of that name is there. */
async function upload(page) {
  await openDocs(page);
  const name = await page.inputValue("#studio-name");
  const there = await page.locator("#docs-list li", { hasText: name }).count();
  if (there === 0) {
    await click(page, "docs-upload");
    await page.waitForTimeout(3000);
  }
  await click(page, "docs-close");
  await page.waitForTimeout(300);
}

(async () => {
  const browser = await chromium.launch({
    executablePath: process.env.PW_CHROMIUM,
    args: ["--use-gl=swiftshader", "--enable-unsafe-swiftshader"],
  });
  let ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 });
  let page = await ctx.newPage();
  page.on("dialog", (d) => d.accept(d.defaultValue()));
  for (const s of SHOTS) {
    await open(page, s.doc);
    await tab(page, s.tab);
    await page.screenshot({ path: `${OUT}/desktop-${s.stem}.png` });
    await upload(page);
  }
  await open(page, RAMP);
  await tab(page, "lug_1");
  await selectFeature(page, 6);
  await page.screenshot({ path: `${OUT}/desktop-part.png` });
  await upload(page);
  await tab(page, "ramp");
  await openDocs(page);
  await page.screenshot({ path: `${OUT}/desktop-docs.png` });
  await ctx.close();

  ctx = await browser.newContext({ ...devices["iPhone 14"], viewport: { width: 390, height: 844 }, deviceScaleFactor: 2 });
  page = await ctx.newPage();
  page.on("dialog", (d) => d.accept(d.defaultValue()));
  await open(page, SHOTS[1].doc);
  await tab(page, SHOTS[1].tab);
  await page.screenshot({ path: `${OUT}/phone-cart.png` });
  await open(page, RAMP);
  await tab(page, "lug_1");
  await selectFeature(page, 6);
  await page.screenshot({ path: `${OUT}/phone-part.png` });
  await openDocs(page);
  await page.screenshot({ path: `${OUT}/phone-docs.png` });
  await ctx.close();
  await browser.close();
  console.log(`wrote the screenshots to ${OUT}`);
})().catch((e) => { console.error(e); process.exit(1); });
