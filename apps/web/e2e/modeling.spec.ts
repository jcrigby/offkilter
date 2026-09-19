import { expect, test } from "@playwright/test";
import { clickViewport, featureNames, openDemo, ready, status, volume } from "./helpers";

test("demo part regenerates with the expected volume", async ({ page }) => {
  await openDemo(page);
  expect(await featureNames(page)).toEqual(["Sketch 1", "Extrude 1", "Sketch 2", "Extrude 2", "Sketch 3", "Extrude 3"]);
  expect(await volume(page)).toBeCloseTo(19152.5, 0);
  expect(await page.$$eval("#feature-list li .dot.err", (els) => els.length)).toBe(0);
});

test("boolean result modes on the slot cut", async ({ page }) => {
  await openDemo(page);
  await page.click("#feature-list li:nth-child(6)");
  // Selects: sketch, end, direction, regions, result.
  const result = page.locator("#detail-body .field select").nth(4);
  await result.selectOption("intersect");
  await page.waitForTimeout(300);
  expect(await status(page)).toContain("2 bodies");
  expect(await volume(page)).toBeCloseTo(348.7, 0);
  await result.selectOption("remove");
  await page.waitForTimeout(300);
  expect(await volume(page)).toBeCloseTo(19152.5, 0);
});

test("sketch mode: draw, infer constraints, extrude", async ({ page }) => {
  page.on("dialog", (d) => d.accept(d.type() === "prompt" ? "top" : d.defaultValue()));
  await page.goto("/");
  await ready(page);
  await page.click("#btn-new");
  await page.click("#btn-add-sketch");
  await page.waitForTimeout(300);
  await page.keyboard.press("r");
  await clickViewport(page, 0.4, 0.4);
  await clickViewport(page, 0.6, 0.6);
  await page.keyboard.press("l");
  await clickViewport(page, 0.4, 0.6); // snaps to the rectangle corner
  await clickViewport(page, 0.45, 0.7);
  await clickViewport(page, 0.55, 0.702); // nearly horizontal
  await page.keyboard.press("Escape");
  await page.keyboard.press("c");
  await clickViewport(page, 0.5, 0.5);
  await clickViewport(page, 0.53, 0.5);
  // The kind cell's first text node is the constraint type.
  const constraints = await page.$$eval("#detail-body .constraint-list .kind", (els) => els.map((e) => (e.childNodes[0]?.textContent ?? "").trim()));
  expect(constraints.filter((c) => c === "coincident").length).toBeGreaterThanOrEqual(5);
  expect(constraints.filter((c) => c === "horizontal").length).toBeGreaterThanOrEqual(3);
  await page.getByRole("button", { name: "Done" }).click();
  await page.click("#btn-add-extrude");
  await page.waitForTimeout(300);
  expect(await status(page)).toContain("1 body");
});

test("face pick, sketch on face, fillet by edge pick", async ({ page }) => {
  await openDemo(page);
  await page.keyboard.press("f");
  await page.waitForTimeout(300);
  // Click somewhere on the plate top.
  let picked = "";
  for (const [fx, fy] of [[0.5, 0.62], [0.35, 0.6], [0.6, 0.55], [0.45, 0.7]] as const) {
    await clickViewport(page, fx, fy);
    picked = await status(page);
    if (picked.startsWith("Face:")) break;
  }
  expect(picked).toContain("Face: Extrude 1");
  await page.click("#btn-add-sketch");
  await page.waitForTimeout(300);
  expect(await page.locator("#detail-body .field .note").first().textContent()).toContain("Extrude 1 · face");
  await page.getByRole("button", { name: "Done" }).click();

  // Fillet: pick a vertical plate edge through the exposed app object.
  const before = await volume(page);
  await page.click("#btn-add-fillet");
  await page.waitForTimeout(300);
  const target = await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    const m = app.kernel.bodyMeshes()[0];
    const faces = app.summary.bodies[0].faces;
    for (let s = 0; s < m.edgeFaces.length / 2; s++) {
      const fa = faces[m.edgeFaces[2 * s]], fb = faces[m.edgeFaces[2 * s + 1]];
      if (fa.surface !== "plane" || fb.surface !== "plane") continue;
      const e = Array.from(m.edges.slice(6 * s, 6 * s + 6)) as number[];
      if (Math.abs(e[5]! - e[2]!) < 1e-6) continue;
      if (fa.origin.feature !== 2 || fb.origin.feature !== 2) continue;
      return app.viewer.toScreen({ x: (e[0]! + e[3]!) / 2, y: (e[1]! + e[4]!) / 2, z: (e[2]! + e[5]!) / 2 });
    }
    return null;
  });
  expect(target).not.toBeNull();
  const vp = (await page.locator("#viewport").boundingBox())!;
  await page.mouse.move(vp.x + target!.x, vp.y + target!.y);
  await page.mouse.down();
  await page.mouse.up();
  await page.waitForTimeout(300);
  expect(await page.$$eval("#detail-body .edge-list li", (els) => els.length)).toBe(1);
  await page.keyboard.press("Escape");
  await page.waitForTimeout(300);
  expect(await volume(page)).toBeLessThan(before);
});

test("variables drive dimensions and undo restores", async ({ page }) => {
  await openDemo(page, "depth");
  await page.click("#btn-add-variable");
  await page.waitForTimeout(300);
  const expr = page.locator("#detail-body .field input").nth(1);
  await expr.fill("12");
  await expr.press("Enter");
  for (let i = 0; i < 7; i++) await page.getByRole("button", { name: "Move up" }).click();
  await page.click("#feature-list li:nth-child(3)"); // Extrude 1
  const depth = page.locator("#detail-body .field input[type=text]").first();
  await depth.fill("#depth");
  await depth.press("Enter");
  await page.waitForTimeout(300);
  const bound = await volume(page);
  expect(bound).toBeGreaterThan(19152.5);
  await page.click("#feature-list li:nth-child(1)");
  await page.keyboard.press("Control+z");
  await page.waitForTimeout(300);
  expect(await volume(page)).toBeCloseTo(19152.5, 0);
});

test("hole feature drills at sketch points on a face", async ({ page }) => {
  await openDemo(page);
  const before = await volume(page);
  // Select the plate top face, sketch two points on it via the geometry form.
  await page.keyboard.press("f");
  await page.waitForTimeout(300);
  let picked = "";
  for (const [fx, fy] of [[0.5, 0.62], [0.35, 0.6], [0.6, 0.55], [0.45, 0.7]] as const) {
    await clickViewport(page, fx, fy);
    picked = await status(page);
    if (picked.startsWith("Face:")) break;
  }
  expect(picked).toContain("Face: Extrude 1");
  await page.click("#btn-add-sketch");
  await page.waitForTimeout(300);
  await page.getByRole("button", { name: "Done" }).click();
  // Add two standalone points through ops exposed on the app object.
  await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    const id = app.selected;
    app.apply({ type: "sketch", id, op: { type: "add_point", pos: { x: 10, y: 10 } } });
    app.apply({ type: "sketch", id, op: { type: "add_point", pos: { x: 50, y: 10 } } });
  });
  await page.click("#btn-add-hole");
  await page.waitForTimeout(400);
  expect(await featureNames(page)).toContain("Hole 1");
  const drilled = await volume(page);
  // Two ⌀6 through holes in the 8 mm plate.
  expect(before - drilled).toBeCloseTo(2 * Math.PI * 9 * 8, -1);
  expect(await page.$$eval("#feature-list li .dot.err", (els) => els.length)).toBe(0);
});

test("sweep and loft between sketches", async ({ page }) => {
  page.on("dialog", (d) => d.accept(d.defaultValue()));
  await page.goto("/");
  await ready(page);
  await page.click("#btn-new");
  await page.waitForTimeout(200);
  // Path: a single 10 mm line on the Top plane starting at the origin.
  // Profile: a 2x2 square centred on the origin of the Right plane (normal +X).
  await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    app.apply({ type: "add_sketch", plane: { type: "standard", base: "top", offset: 0 }, name: "Path" });
    const path = app.summary.features[app.summary.features.length - 1].id;
    app.apply({ type: "sketch", id: path, op: { type: "add_line", a: { x: 0, y: 0 }, b: { x: 10, y: 0 } } });
    app.apply({ type: "add_sketch", plane: { type: "standard", base: "right", offset: 0 }, name: "Profile" });
    const prof = app.summary.features[app.summary.features.length - 1].id;
    app.apply({ type: "sketch", id: prof, op: { type: "add_rectangle", a: { x: -1, y: -1 }, b: { x: 1, y: 1 } } });
    app.select(prof);
  });
  await page.click("#btn-add-sweep");
  await page.waitForTimeout(300);
  expect(await featureNames(page)).toContain("Sweep 1");
  expect(await volume(page)).toBeCloseTo(40, 0);
  // Loft from the profile square to a larger square 6 mm further along +X.
  await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    app.apply({ type: "add_sketch", plane: { type: "standard", base: "right", offset: 12 }, name: "Top square" });
    const b = app.summary.features[app.summary.features.length - 1].id;
    app.apply({ type: "sketch", id: b, op: { type: "add_rectangle", a: { x: -2, y: -2 }, b: { x: 2, y: 2 } } });
    const prof = app.summary.features.find((f: any) => f.name === "Profile").id;
    app.apply({ type: "add_loft", sketch: b, sketch_b: prof, op: "new", name: "Loft 1" });
  });
  await page.waitForTimeout(300);
  expect(await featureNames(page)).toContain("Loft 1");
  // Frustum between 2x2 and 4x4 squares, 12 mm tall: h/3 (A1 + A2 + sqrt(A1 A2)) = 4 (4 + 16 + 8) = 112.
  const total = await volume(page);
  expect(total).toBeCloseTo(40 + 112, 0);
  expect(await page.$$eval("#feature-list li .dot.err", (els) => els.length)).toBe(0);
});

test("use tool projects a face outline into a sketch", async ({ page }) => {
  page.on("dialog", (d) => d.accept(d.defaultValue()));
  await page.goto("/");
  await ready(page);
  await page.click("#btn-new");
  await page.waitForTimeout(200);
  await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    app.apply({ type: "add_sketch", plane: { type: "standard", base: "top", offset: 0 }, name: "Base" });
    const s = app.summary.features[app.summary.features.length - 1].id;
    app.apply({ type: "sketch", id: s, op: { type: "add_rectangle", a: { x: 0, y: 0 }, b: { x: 10, y: 6 } } });
    app.apply({ type: "add_extrude", sketch: s, depth: 5, name: "Extrude 1" });
    const e1 = app.summary.features[app.summary.features.length - 1].id;
    app.apply({ type: "add_sketch", plane: { type: "face", face: { feature: e1, local: 1 }, offset: 0 }, name: "On top" });
    app.editSketch(app.summary.features[app.summary.features.length - 1].id);
  });
  const bodyVolume = () => page.evaluate(() => (window as unknown as { offkilter: any }).offkilter.summary.bodies.reduce((n: number, b: any) => n + b.volume, 0));
  expect(await bodyVolume()).toBeCloseTo(300, 0);
  // The Use tool is offered while editing and hides later bodies.
  await page.getByRole("button", { name: "Use" }).click();
  await expect(page.locator("#status-text")).toContainText("Use:");
  await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    const e1 = app.summary.features.find((f: any) => f.name === "Extrude 1").id;
    app.apply({ type: "sketch", id: app.sketcher.sketchId, op: { type: "project", source: { type: "face", face: { feature: e1, local: 1 } } } });
  });
  await expect(page.locator("#detail-body")).toContainText("face outline");
  await expect(page.locator("#detail-body")).toContainText("fully constrained");
  await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    app.sketcher.exit();
    app.apply({ type: "add_extrude", sketch: app.selected, depth: 3, op: "new", name: "Extrude 2" });
  });
  await page.waitForTimeout(300);
  expect(await bodyVolume()).toBeCloseTo(480, 0);
  expect(await page.$$eval("#feature-list li .dot.err", (els) => els.length)).toBe(0);
});
