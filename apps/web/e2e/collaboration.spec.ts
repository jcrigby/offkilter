import { expect, test } from "@playwright/test";
import { featureNames, ready } from "./helpers";

const SERVER = process.env.OK_SERVER ?? "http://localhost:8080";

test("two clients edit one document live", async ({ browser }) => {
  let up = false;
  try {
    up = (await fetch(`${SERVER}/api/health`)).ok;
  } catch {
    up = false;
  }
  test.skip(!up, `no document server at ${SERVER}`);

  const a = await (await browser.newContext()).newPage();
  const b = await (await browser.newContext()).newPage();
  for (const p of [a, b]) p.on("dialog", (d) => d.accept(d.type() === "prompt" ? "width" : d.defaultValue()));
  await a.goto(`${SERVER}/`);
  await ready(a);
  await a.click("#btn-demo");
  await a.click("#btn-docs");
  await a.click("#docs-upload");
  await a.waitForFunction(() => new URL(location.href).searchParams.get("doc") !== null);
  await expect(a.locator("#presence")).toContainText("1 online", { timeout: 10_000 });
  await b.goto(a.url());
  await ready(b);
  await expect(a.locator("#presence")).toContainText("2 online", { timeout: 10_000 });
  await expect(b.locator("#presence")).toContainText("2 online", { timeout: 10_000 });

  // A suppresses the slot; B sees the lighter body.
  await a.click("#feature-list li:nth-child(6)");
  await a.getByRole("button", { name: "Suppress" }).click();
  await expect(b.locator("#status-text")).toContainText("223 faces", { timeout: 10_000 });

  // B adds a variable; A sees the new feature.
  await b.click("#btn-add-variable");
  await expect.poll(async () => featureNames(a), { timeout: 10_000 }).toContain("#width");
});
