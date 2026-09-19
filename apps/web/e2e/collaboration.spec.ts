import { expect, test } from "@playwright/test";
import { featureNames, ready, status } from "./helpers";

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
  await a.waitForTimeout(300);
  await b.goto(a.url());
  await ready(b);
  await b.waitForTimeout(300);
  expect(await a.textContent("#presence")).toContain("2 online");

  await a.click("#feature-list li:nth-child(6)");
  await a.getByRole("button", { name: "Suppress" }).click();
  await b.waitForTimeout(600);
  expect(await status(b)).toContain("223 faces");

  await b.click("#btn-add-variable");
  await a.waitForTimeout(600);
  expect(await featureNames(a)).toContain("#width");
});
