import { expect, type Page } from "@playwright/test";

/** Waits until the kernel has loaded and regenerated once. */
export async function ready(page: Page): Promise<void> {
  await page.waitForFunction(() => /bod(y|ies)/.test(document.querySelector("#status-text")?.textContent ?? ""), null, { timeout: 30_000 });
}

export async function status(page: Page): Promise<string> {
  return (await page.textContent("#status-text")) ?? "";
}

/** Volume in mm³ parsed from the status bar. */
export async function volume(page: Page): Promise<number> {
  const m = (await status(page)).match(/([\d.]+) mm³/);
  expect(m).not.toBeNull();
  return Number(m![1]);
}

export async function featureNames(page: Page): Promise<string[]> {
  return page.$$eval("#feature-list li .name", (els) => els.map((e) => e.textContent ?? ""));
}

/** Clicks in the viewport at a fraction of its size. */
export async function clickViewport(page: Page, fx: number, fy: number): Promise<void> {
  const vp = await page.locator("#viewport").boundingBox();
  if (!vp) throw new Error("no viewport");
  const x = vp.x + vp.width * fx;
  const y = vp.y + vp.height * fy;
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.up();
  await page.waitForTimeout(150);
}

/** Loads the demo part fresh. Dialogs are accepted; prompts get `promptAnswer` or their default. */
export async function openDemo(page: Page, promptAnswer?: string): Promise<void> {
  page.on("dialog", (d) => d.accept(d.type() === "prompt" ? (promptAnswer ?? d.defaultValue()) : d.defaultValue()));
  await page.goto("/");
  await ready(page);
  await page.click("#btn-demo");
  await page.waitForTimeout(300);
}
