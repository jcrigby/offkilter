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

  // Both add a sketch at the same moment: ids come from each client's own
  // range, so both replicas and the server converge without a resync.
  const ids = (p: typeof a) => p.evaluate(() => (window as unknown as { offkilter: any }).offkilter.summary.features.map((f: any) => f.id));
  const addSketch = (p: typeof a) =>
    p.evaluate(() => {
      const app = (window as unknown as { offkilter: any }).offkilter;
      app.apply({ type: "add_sketch", plane: { type: "standard", base: "top", offset: 0 }, name: app.autoName("Sketch") });
    });
  await Promise.all([addSketch(a), addSketch(b)]);
  await expect.poll(async () => (await ids(a)).length, { timeout: 10_000 }).toBe(9);
  await expect.poll(async () => (await ids(b)).length, { timeout: 10_000 }).toBe(9);
  const [idsA, idsB] = await Promise.all([ids(a), ids(b)]);
  expect(new Set(idsA)).toEqual(new Set(idsB));
  const docId = new URL(a.url()).searchParams.get("doc");
  const server = (await (await fetch(`${SERVER}/api/docs/${docId}`)).json()) as { tabs: { kind: { features: { id: number }[] } }[] };
  expect(new Set(server.tabs[0]!.kind.features.map((f) => f.id))).toEqual(new Set(idsA));
  // The two new sketches got ids from different prefixes (>= 1 << 20).
  const fresh = idsA.filter((id: number) => id >= 1 << 20);
  expect(fresh.length).toBeGreaterThanOrEqual(2);
  expect(new Set(fresh.map((id: number) => id >> 20)).size).toBe(2);

  // Undo is per user: A adds a variable, B adds one, A undoes. Only A's
  // goes, on both replicas, and B's edit survives. (The simultaneous adds
  // above may have resynced once to agree on feature order; from here on
  // replicas must agree with the server without any resync.)
  const resyncs = (p: typeof a) => p.evaluate(() => (window as unknown as { offkilter: any }).offkilter.sync.resyncs as number);
  const [resyncA, resyncB] = [await resyncs(a), await resyncs(b)];
  const addVar = (p: typeof a, name: string) =>
    p.evaluate((n) => (window as unknown as { offkilter: any }).offkilter.apply({ type: "add_variable", name: n, expression: "1" }), name);
  await addVar(a, "ua");
  await expect.poll(async () => featureNames(b), { timeout: 10_000 }).toContain("#ua");
  await addVar(b, "ub");
  await expect.poll(async () => featureNames(a), { timeout: 10_000 }).toContain("#ub");
  await a.click("#viewport");
  await a.keyboard.press("Control+z");
  await expect.poll(async () => featureNames(a), { timeout: 10_000 }).not.toContain("#ua");
  expect(await featureNames(a)).toContain("#ub");
  await expect.poll(async () => featureNames(b), { timeout: 10_000 }).not.toContain("#ua");
  expect(await featureNames(b)).toContain("#ub");
  // Redo brings it back everywhere.
  await a.keyboard.press("Control+y");
  await expect.poll(async () => featureNames(b), { timeout: 10_000 }).toContain("#ua");
  expect(await featureNames(a)).toEqual(await featureNames(b));
  expect(await resyncs(a)).toBe(resyncA);
  expect(await resyncs(b)).toBe(resyncB);
});

test("accounts own documents and share them", async ({ browser }) => {
  // Several argon2 registrations and page loads: allow three times the usual budget.
  test.slow();
  let up = false;
  try {
    up = (await fetch(`${SERVER}/api/health`)).ok;
  } catch {
    up = false;
  }
  test.skip(!up, `no document server at ${SERVER}`);
  const stamp = Date.now().toString(36);
  const alice = `alice_${stamp}`;
  const bob = `bob_${stamp}`;

  const a = await (await browser.newContext()).newPage();
  const b = await (await browser.newContext()).newPage();
  // Prompts answer the share dialog with bob's name (as an editor first, then
  // read-only); confirms are accepted.
  let shares = 0;
  for (const p of [a, b]) p.on("dialog", (d) => d.accept(d.type() === "prompt" ? (d.message().startsWith("Share") ? (shares++ === 0 ? bob : `${bob} viewer`) : d.message().startsWith("Invite") ? "editor" : "Private part") : d.defaultValue()));

  // Alice creates an account through the dialog.
  await a.goto(`${SERVER}/`);
  await ready(a);
  await a.click("#btn-account");
  await a.fill("#account-name", alice);
  await a.fill("#account-password", "correct horse battery");
  await a.click("#account-register");
  await expect(a.locator("#btn-account")).toHaveText(alice);
  // The session survives a reload.
  await a.reload();
  await ready(a);
  await expect(a.locator("#btn-account")).toHaveText(alice);

  // She creates a document: it is hers and shows her account name in presence.
  await a.click("#btn-docs");
  await a.click("#docs-create");
  await a.waitForFunction(() => new URL(location.href).searchParams.get("doc") !== null);
  await expect(a.locator("#presence")).toContainText("1 online", { timeout: 10_000 });
  await expect(a.locator("#presence")).toHaveAttribute("title", alice);
  const docUrl = a.url();

  // Bob registers; the document is invisible to him until it is shared.
  await b.goto(`${SERVER}/`);
  await ready(b);
  await b.click("#btn-account");
  await b.fill("#account-name", bob);
  await b.fill("#account-password", "bobs long password");
  await b.click("#account-register");
  await expect(b.locator("#btn-account")).toHaveText(bob);
  await b.click("#btn-docs");
  await expect(b.locator("#docs-list")).not.toContainText("Private part");
  await b.click("#docs-close");

  await a.click("#btn-docs");
  await expect(a.locator("#docs-list")).toContainText("yours");
  await a.getByRole("button", { name: "Share…" }).first().click();
  await expect(a.locator("#docs-list")).toContainText(`shared with ${bob}`);
  await a.click("#docs-close");

  await b.click("#btn-docs");
  await expect(b.locator("#docs-list")).toContainText("Private part");
  await expect(b.locator("#docs-list")).toContainText(`by ${alice}`);
  await b.click("#docs-close");
  await b.goto(docUrl);
  await ready(b);
  await expect(a.locator("#presence")).toContainText("2 online", { timeout: 10_000 });
  await expect(a.locator("#presence")).toHaveAttribute("title", `${alice}, ${bob}`);

  // Shared again as a viewer, Bob sees a read-only badge and his edits are refused.
  await a.click("#btn-docs");
  await a.getByRole("button", { name: "Share…" }).first().click();
  await expect(a.locator("#docs-list")).toContainText(`read-only: ${bob}`);
  await a.click("#docs-close");
  await b.goto(docUrl);
  await ready(b);
  await expect(b.locator("#read-only")).toBeVisible();
  await b.click("#btn-add-variable");
  await b.waitForTimeout(300);
  expect(await b.evaluate(() => (window as unknown as { offkilter: any }).offkilter.readOnly)).toBe(true);
  expect(await featureNames(b)).not.toContain("#width");
  await expect(a.locator("#read-only")).toBeHidden();

  // An editor invitation link from Alice makes Bob an editor when he opens it.
  await a.click("#btn-docs");
  await a.getByRole("button", { name: "Invite link…" }).first().click();
  await expect(a.locator("#docs-note")).toContainText("Invite link (editor):");
  const link = (await a.locator("#docs-note").textContent())!.match(/https?:\/\/\S+/)![0];
  expect(link).toContain("invite=");
  await expect(a.getByRole("button", { name: "− link (editor)" })).toHaveCount(1);
  await a.click("#docs-close");
  await b.goto(link);
  await ready(b);
  await expect(b.locator("#status-text")).toContainText("Joined", { timeout: 10_000 });
  await expect(b.locator("#read-only")).toBeHidden();
  expect(await b.evaluate(() => (window as unknown as { offkilter: any }).offkilter.readOnly)).toBe(false);
  expect(b.url()).not.toContain("invite=");
  await a.click("#btn-docs");
  await expect(a.locator("#docs-list")).toContainText(`shared with ${bob}`);
  // Withdrawing the link keeps Bob's access.
  await a.getByRole("button", { name: "− link (editor)" }).click();
  await expect(a.getByRole("button", { name: "− link (editor)" })).toHaveCount(0);
  await expect(a.locator("#docs-list")).toContainText(`shared with ${bob}`);
  await a.click("#docs-close");

  // Signing out closes the live document.
  await b.click("#btn-account");
  await expect(b.locator("#btn-account")).toHaveText("Sign in");
  await expect(a.locator("#presence")).toContainText("1 online", { timeout: 10_000 });
});
