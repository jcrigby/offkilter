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

test("sketch trim, offset and mirror", async ({ page }) => {
  page.on("dialog", (d) => d.accept(d.type() === "prompt" ? (d.message().startsWith("Fillet") ? "1.5" : "-1") : d.defaultValue()));
  await page.goto("/");
  await ready(page);
  await page.click("#btn-new");
  await page.waitForTimeout(200);
  const regions = () => page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    return app.summary.sketches[String(app.sketcher.sketchId)].profiles.length as number;
  });
  await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    app.apply({ type: "add_sketch", plane: { type: "standard", base: "top", offset: 0 }, name: "Sketch 1" });
    const id = app.summary.features[app.summary.features.length - 1].id;
    app.apply({ type: "sketch", id, op: { type: "add_rectangle", a: { x: 0, y: 0 }, b: { x: 10, y: 6 } } });
    app.apply({ type: "sketch", id, op: { type: "add_line", a: { x: -2, y: 3 }, b: { x: 12, y: 3 } } });
    app.editSketch(id);
  });
  // The crossing line splits the rectangle in two.
  expect(await regions()).toBe(2);
  await page.getByRole("button", { name: "Trim" }).click();
  await expect(page.locator("#status-text")).toContainText("Trim:");
  // Trim the middle piece of the crossing line: one region again.
  await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    const id = app.sketcher.sketchId;
    const line = app.summary.features.find((f: any) => f.id === id).kind.sketch.entities.filter((e: any) => e.type === "line").pop();
    app.apply({ type: "sketch", id, op: { type: "trim", entity: line.id, at: { x: 5, y: 3 } } });
  });
  expect(await regions()).toBe(1);
  // Select the rectangle and offset it outward through the panel button.
  await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    const id = app.sketcher.sketchId;
    const lines = app.summary.features.find((f: any) => f.id === id).kind.sketch.entities.filter((e: any) => e.type === "line").slice(0, 4);
    app.sketcher.selection = new Set(lines.map((l: any) => l.id));
    app.selectionChanged();
  });
  await page.getByRole("button", { name: "Offset…" }).click();
  await page.waitForTimeout(300);
  // Inner rectangle plus the ring, which the two stubs of the trimmed
  // line cut into a top and a bottom half.
  expect(await regions()).toBe(3);
  // Mirror the rectangle across a construction line at x = 20: a copy at 30..40.
  await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    const id = app.sketcher.sketchId;
    const lines = app.summary.features.find((f: any) => f.id === id).kind.sketch.entities.filter((e: any) => e.type === "line").slice(0, 4);
    const r = app.applyRaw({ type: "sketch", id, op: { type: "add_line", a: { x: 20, y: -5 }, b: { x: 20, y: 10 } } });
    app.applyRaw({ type: "sketch", id, op: { type: "set_construction", id: r.entities[0], construction: true } });
    app.apply({ type: "sketch", id, op: { type: "mirror", entities: lines.map((l: any) => l.id), axis: r.entities[0] } });
  });
  await page.waitForTimeout(300);
  expect(await regions()).toBe(4);
  await expect(page.locator("#detail-body")).toContainText("symmetric");
  // Undo the mirror.
  await page.click("#viewport");
  await page.keyboard.press("Control+z");
  await page.waitForTimeout(300);
  expect(await regions()).toBe(3);
  // Fillet: select the rectangle's bottom and right lines (they meet at a
  // corner) and round it with the panel button; an arc appears and the
  // sketch still solves.
  await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    const id = app.sketcher.sketchId;
    const sk = app.summary.features.find((f: any) => f.id === id).kind.sketch;
    const pos = (pid: number) => sk.entities.find((e: any) => e.id === pid).pos;
    const lines = sk.entities.filter((e: any) => e.type === "line");
    const bottom = lines.find((l: any) => pos(l.start).y === 0 && pos(l.end).y === 0 && pos(l.start).x >= 0 && pos(l.end).x >= 0);
    const right = lines.find((l: any) => pos(l.start).x === 10 && pos(l.end).x === 10);
    app.sketcher.selection = new Set([bottom.id, right.id]);
    app.selectionChanged();
  });
  const arcsBefore = await page.evaluate(() => (window as unknown as { offkilter: any }).offkilter.summary.features.find((f: any) => f.id === (window as any).offkilter.sketcher.sketchId).kind.sketch.entities.filter((e: any) => e.type === "arc").length as number);
  await page.getByRole("button", { name: "Fillet…" }).click();
  await page.waitForTimeout(200);
  const after = await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    const id = app.sketcher.sketchId;
    const sk = app.summary.features.find((f: any) => f.id === id).kind.sketch;
    return { arcs: sk.entities.filter((e: any) => e.type === "arc").length, tangents: sk.constraints.filter((c: any) => c.type === "tangent").length, status: app.summary.sketches[String(id)].solve.status };
  });
  expect(after.arcs).toBe(arcsBefore + 1);
  expect(after.tangents).toBeGreaterThanOrEqual(2);
  expect(after.status).not.toBe("inconsistent");
});

test("assembly tab: insert two instances and mate them face to face", async ({ page }) => {
  page.on("dialog", (d) => d.accept(d.defaultValue()));
  await page.goto("/");
  await ready(page);
  await page.click("#btn-new");
  await page.waitForTimeout(200);
  // A 10x10x5 block in the part studio.
  const extrude = await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    app.apply({ type: "add_sketch", plane: { type: "standard", base: "top", offset: 0 }, name: "Sketch 1" });
    const s = app.summary.features[app.summary.features.length - 1].id;
    app.apply({ type: "sketch", id: s, op: { type: "add_rectangle", a: { x: 0, y: 0 }, b: { x: 10, y: 10 } } });
    app.apply({ type: "add_extrude", sketch: s, depth: 5, name: "Block" });
    return app.summary.features[app.summary.features.length - 1].id as number;
  });
  // A new assembly tab through the tab bar.
  await page.getByRole("button", { name: "+ Assembly" }).click();
  await expect(page.locator("#tabbar button.active")).toContainText("Assembly 1");
  await expect(page.locator("#assembly-panel")).toBeVisible();
  await expect(page.locator("#detail-title")).toHaveText("Insert instance");
  // Insert the block twice through the form, then mate B's bottom onto A's top.
  await page.getByRole("button", { name: "Insert", exact: true }).click();
  await expect(page.locator("#instance-list li")).toHaveCount(1);
  await page.click("#btn-insert-instance");
  await page.getByRole("button", { name: "Insert", exact: true }).click();
  await expect(page.locator("#instance-list li")).toHaveCount(2);
  const count = () => page.evaluate(() => (window as unknown as { offkilter: any }).offkilter.summary.bodies.length as number);
  expect(await count()).toBe(2);
  await page.evaluate((e) => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    const [a, b] = app.summary.instances.map((i: any) => i.id);
    app.applyDoc({ type: "assembly", tab: app.tab, op: { type: "add_mate", kind: "fastened", a: { instance: a, face: { feature: e, local: 1 } }, b: { instance: b, face: { feature: e, local: 0 } }, offset: 0, angle: 0, flip: false, name: null } });
  }, extrude);
  await expect(page.locator("#mate-list li")).toHaveCount(1);
  // The assembly's bill of materials lists both instances with their source tab.
  const bom: string = await page.evaluate(() => (window as unknown as { offkilter: any }).offkilter.toBom());
  expect(bom.split("\n")[0]).toBe("instance,source,body,material,mass_g,volume_mm3,surface_mm2,size_mm");
  expect(bom.trim().split("\n").length).toBe(3);
  expect(bom).toContain("Part Studio 1");
  expect(bom).toContain("500.000");
  const top = () => page.evaluate(() => (window as unknown as { offkilter: any }).offkilter.summary.bodies[1].bounds[0].z as number);
  expect(await top()).toBeCloseTo(5, 6);
  // Editing the mate offset through the panel moves the instance.
  await page.click("#mate-list li");
  const offset = page.locator("#detail-body .field input[type=number]").first();
  await offset.fill("3");
  await offset.press("Enter");
  await page.waitForTimeout(300);
  expect(await top()).toBeCloseTo(8, 6);
  expect(await page.$$eval("#mate-list .dot.err, #instance-list .dot.err", (els) => els.length)).toBe(0);
  // Touching blocks do not interfere; sinking B by 1 mm overlaps 10x10x1.
  await page.click("#btn-interference");
  await expect(page.locator("#detail-body")).toContainText("No overlapping instances");
  await page.click("#mate-list li");
  await offset.fill("-1");
  await offset.press("Enter");
  await page.waitForTimeout(300);
  await page.click("#btn-interference");
  await expect(page.locator("#detail-body")).toContainText("100.00 mm³ overlap");
  // Undo the last offset edit (back to 3); the part studio tab still holds the block.
  await page.click("#viewport");
  await page.keyboard.press("Control+z");
  await page.waitForTimeout(300);
  expect(await top()).toBeCloseTo(8, 6);
  await page.getByRole("button", { name: /Part Studio 1/ }).click();
  await expect(page.locator("#feature-list")).toContainText("Block");
});

test("assembly tab: edge and corner mate connectors", async ({ page }) => {
  page.on("dialog", (d) => d.accept(d.defaultValue()));
  await page.goto("/");
  await ready(page);
  await page.click("#btn-new");
  await page.waitForTimeout(200);
  const extrude = await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    app.apply({ type: "add_sketch", plane: { type: "standard", base: "top", offset: 0 }, name: "Sketch 1" });
    const s = app.summary.features[app.summary.features.length - 1].id;
    app.apply({ type: "sketch", id: s, op: { type: "add_rectangle", a: { x: 0, y: 0 }, b: { x: 10, y: 10 } } });
    app.apply({ type: "add_extrude", sketch: s, depth: 5, name: "Block" });
    return app.summary.features[app.summary.features.length - 1].id as number;
  });
  await page.getByRole("button", { name: "+ Assembly" }).click();
  await page.getByRole("button", { name: "Insert", exact: true }).click();
  await page.click("#btn-insert-instance");
  await page.getByRole("button", { name: "Insert", exact: true }).click();
  await expect(page.locator("#instance-list li")).toHaveCount(2);
  const bounds = (i: number) => page.evaluate((i) => (window as unknown as { offkilter: any }).offkilter.summary.bodies[i].bounds, i);

  // A hinge: B's bottom-front edge on A's top-front edge. Frames meet with
  // z (along the edge) opposed, so B folds under, beside A (y < 0).
  await page.evaluate((e) => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    const [a, b] = app.summary.instances.map((i: any) => i.id);
    const f = (local: number) => ({ feature: e, local });
    app.applyDoc({ type: "assembly", tab: app.tab, op: { type: "add_mate", kind: "revolute", a: { instance: a, face: f(1), anchor: { type: "edge", other: f(2) } }, b: { instance: b, face: f(0), anchor: { type: "edge", other: f(2) } }, offset: 0, angle: 0, flip: false, name: null } });
  }, extrude);
  await expect(page.locator("#mate-list li")).toHaveCount(1);
  expect(await page.$$eval("#mate-list .dot.err", (els) => els.length)).toBe(0);
  let b = await bounds(1);
  expect(b[0].z).toBeCloseTo(0, 5);
  expect(b[1].z).toBeCloseTo(5, 5);
  expect(b[1].y).toBeCloseTo(0, 5);
  expect(b[0].y).toBeCloseTo(-10, 5);
  // Animating the hinge moves B on screen only: no ops, no change to the mate.
  await page.click("#mate-list li");
  await expect(page.locator("#detail-body")).toContainText("edge 1/2");
  const opCount = () => page.evaluate(() => (window as unknown as { offkilter: any }).offkilter.history.length as number);
  const before = await opCount();
  await page.click("#btn-animate-mate");
  await page.waitForTimeout(700);
  expect(await status(page)).toMatch(/Animating .*°/);
  const during = await page.evaluate(() => (window as any).offkilter.viewer.bodyMatrix(1));
  const identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
  expect(Math.max(...during.map((v: number, i: number) => Math.abs(v - identity[i]!)))).toBeGreaterThan(0.05);
  expect(await page.evaluate(() => (window as any).offkilter.viewer.bodyMatrix(0))).toEqual(identity);
  await expect(page.locator("#btn-animate-mate")).toHaveText("Stop animation");
  await page.keyboard.press("Escape");
  await expect(page.locator("#btn-animate-mate")).toHaveText("Animate");
  expect(await page.evaluate(() => (window as any).offkilter.viewer.bodyMatrix(1))).toEqual(identity);
  expect(await opCount()).toBe(before);
  expect(await page.evaluate(() => (window as unknown as { offkilter: any }).offkilter.summary.mates[0].angle)).toBe(0);
  // Opening the hinge by 90° stands B up along the shared edge.
  await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    app.applyDoc({ type: "assembly", tab: app.tab, op: { type: "set_mate", id: app.summary.mates[0].id, angle: 90 } });
  });
  b = await bounds(1);
  expect(b[1].z - b[0].z).toBeCloseTo(10, 5);
  expect(b[1].y - b[0].y).toBeCloseTo(5, 5);
  expect(b[1].x - b[0].x).toBeCloseTo(10, 5);

  // Corner connectors picked in the viewport: remove the hinge, park B
  // beside A, then click near a top corner of each block.
  await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    app.applyDoc({ type: "assembly", tab: app.tab, op: { type: "remove_mate", id: app.summary.mates[0].id } });
    app.applyDoc({ type: "assembly", tab: app.tab, op: { type: "set_instance", id: app.summary.instances[1].id, placement: { position: { x: 20, y: 0, z: 0 }, rotation: { x: 0, y: 0, z: 0 } } } });
  });
  await page.keyboard.press("1");
  await page.keyboard.press("f");
  await page.waitForTimeout(300);
  await page.click("#btn-add-mate");
  expect(await status(page)).toContain("corner");
  const vp = (await page.locator("#viewport").boundingBox())!;
  const clickNearCorner = async (corner: { x: number; y: number; z: number }, centre: { x: number; y: number; z: number }) => {
    const [c, m] = await page.evaluate(([c, m]) => {
      const v = (window as any).offkilter.viewer;
      return [v.toScreen(c), v.toScreen(m)];
    }, [corner, centre]);
    const d = Math.hypot(m.x - c.x, m.y - c.y);
    const x = c.x + ((m.x - c.x) / d) * 3, y = c.y + ((m.y - c.y) / d) * 3;
    await page.mouse.move(vp.x + x, vp.y + y);
    await page.mouse.down();
    await page.mouse.up();
    await page.waitForTimeout(200);
  };
  await clickNearCorner({ x: 10, y: 0, z: 5 }, { x: 5, y: 5, z: 5 });
  expect(await status(page)).toMatch(/First connector: .* corner 1\//);
  await clickNearCorner({ x: 20, y: 0, z: 5 }, { x: 25, y: 5, z: 5 });
  await expect(page.locator("#mate-list li")).toHaveCount(1);
  const mate = await page.evaluate(() => (window as unknown as { offkilter: any }).offkilter.summary.mates[0]);
  expect(mate.a.anchor.type).toBe("vertex");
  expect(mate.b.anchor.type).toBe("vertex");
  expect(mate.error ?? null).toBeNull();
  // Corners coincide with the top normals opposed: B sits upside down on A's corner.
  b = await bounds(1);
  expect(b[0].z).toBeCloseTo(5, 5);
  expect(b[1].z).toBeCloseTo(10, 5);
  expect(Math.min(Math.abs(b[0].x - 10), Math.abs(b[1].x - 10))).toBeLessThan(1e-5);
  expect(Math.min(Math.abs(b[0].y), Math.abs(b[1].y))).toBeLessThan(1e-5);
});

test("standard views, measure, section and part rename", async ({ page }) => {
  await openDemo(page);
  // The shortcuts dialog opens with ? and from the toolbar.
  await page.keyboard.press("?");
  await expect(page.locator("#help-dialog")).toBeVisible();
  await expect(page.locator("#help-dialog")).toContainText("Spline");
  await page.click("#help-close");
  await expect(page.locator("#help-dialog")).toBeHidden();
  // Standard views look straight down an axis.
  await page.keyboard.press("1");
  await page.waitForTimeout(200);
  let dir = await page.evaluate(() => (window as any).offkilter.viewer.viewDirection());
  expect(dir.z).toBeCloseTo(-1, 6);
  await page.click("#btn-view-front");
  dir = await page.evaluate(() => (window as any).offkilter.viewer.viewDirection());
  expect(dir.y).toBeCloseTo(1, 6);
  await page.keyboard.press("f");
  await page.keyboard.press("1");
  await page.waitForTimeout(200);

  // Measure across the plate's top face, near two opposite corners (seen from above).
  const bounds = await page.evaluate(() => (window as any).offkilter.summary.bodies[0].bounds);
  const plateTop = await page.evaluate(() => {
    const faces = (window as any).offkilter.summary.bodies[0].faces as { origin: { feature: number }; normal: { z: number } }[];
    return faces.some((f) => f.origin.feature === 2 && f.normal.z > 0.99);
  });
  expect(plateTop).toBe(true);
  await page.click("#btn-measure");
  expect(await status(page)).toContain("Measure:");
  const vp = (await page.locator("#viewport").boundingBox())!;
  const corners = [
    { x: bounds[0].x + 1.5, y: bounds[0].y + 1.5, z: bounds[0].z + 1 },
    { x: bounds[1].x - 1.5, y: bounds[1].y - 1.5, z: bounds[0].z + 1 },
  ];
  for (const c of corners) {
    const s = await page.evaluate((c) => (window as any).offkilter.viewer.toScreen(c), c);
    await page.mouse.move(vp.x + s.x, vp.y + s.y);
    await page.mouse.down();
    await page.mouse.up();
    await page.waitForTimeout(150);
  }
  const measured = await status(page);
  const m = measured.match(/Distance ([\d.]+) mm · dx (-?[\d.]+) · dy (-?[\d.]+) · dz (-?[\d.]+)/);
  expect(m, measured).not.toBeNull();
  const diag = Math.hypot(bounds[1].x - bounds[0].x, bounds[1].y - bounds[0].y);
  // Both clicks landed on the plate top (dz = 0) roughly a diagonal apart.
  expect(Number(m![1])).toBeGreaterThan(diag * 0.8);
  expect(Number(m![1])).toBeCloseTo(Math.hypot(Number(m![2]), Number(m![3]), Number(m![4])), 2);
  expect(Math.abs(Number(m![4]))).toBeLessThan(1e-6);
  await page.keyboard.press("Escape");
  expect(await status(page)).not.toContain("Distance");

  // Section view exposes the slider; turning it off hides it again.
  await page.selectOption("#section-axis", "z");
  await expect(page.locator("#section-offset")).toBeVisible();
  expect(await page.evaluate(() => (window as any).offkilter.section.axis)).toBe("z");
  await page.selectOption("#section-axis", "");
  await expect(page.locator("#section-offset")).toBeHidden();

  // Hide and show the part from the parts list.
  await page.click("#part-list li .eye");
  expect(await page.evaluate(() => (window as any).offkilter.viewer.hiddenBodies().size)).toBe(1);
  await page.click("#part-list li .eye");
  expect(await page.evaluate(() => (window as any).offkilter.viewer.hiddenBodies().size)).toBe(0);

  // Rename the part; undo restores the default name.
  const original = (await page.textContent("#part-list li .pname")) ?? "";
  await page.dblclick("#part-list li .pname");
  await page.fill("#part-list li .pname-edit", "Plate");
  await page.keyboard.press("Enter");
  await page.waitForTimeout(200);
  expect(await page.textContent("#part-list li .pname")).toBe("Plate");
  await page.keyboard.press("Control+z");
  await page.waitForTimeout(200);
  expect(await page.textContent("#part-list li .pname")).toBe(original);
  // A material gives the part a mass: volume × density.
  const vol = await volume(page);
  await page.selectOption("#part-list li .pmaterial", "Aluminium");
  await page.waitForTimeout(200);
  const stats = (await page.textContent("#part-list li .pstats")) ?? "";
  const grams = vol * 2.7 / 1000;
  expect(stats).toContain(grams >= 1000 ? `${(grams / 1000).toFixed(3)} kg` : `${grams.toFixed(1)} g`);
  const bom: string = await page.evaluate(() => (window as unknown as { offkilter: any }).offkilter.toBom());
  expect(bom.split("\n")[0]).toBe("part,material,mass_g,volume_mm3,surface_mm2,size_mm,faces");
  expect(bom).toContain("Aluminium");
  await page.keyboard.press("Control+z");
  await page.waitForTimeout(200);
  expect((await page.textContent("#part-list li .pstats")) ?? "").not.toContain(" g");
});

test("export bodies as STL and 3MF and a sketch as DXF", async ({ page }) => {
  await openDemo(page);
  const sizes = await page.evaluate(async () => {
    const app = (window as any).offkilter;
    const stl = app.toStl();
    const mf = app.to3mf();
    const head = new Uint8Array(await mf.slice(0, 2).arrayBuffer());
    const png = await app.viewer.snapshot();
    const pngHead = new Uint8Array(await png.slice(1, 4).arrayBuffer());
    return { stl: stl.size, mf: mf.size, sig: String.fromCharCode(...head), dxfWithoutSketch: app.toDxf(), png: png.size, pngSig: String.fromCharCode(...pngHead) };
  });
  expect(sizes.png).toBeGreaterThan(1000);
  expect(sizes.pngSig).toBe("PNG");
  expect(sizes.stl).toBe(84 + 50 * (await page.evaluate(() => (window as any).offkilter.summary.bodies.reduce((n: number, b: any) => n + b.triangles, 0))));
  expect(sizes.mf).toBeGreaterThan(1000);
  expect(sizes.sig).toBe("PK");
  expect(sizes.dxfWithoutSketch).toBeNull();
  await page.click("#feature-list li:nth-child(1)");
  const dxf: string = await page.evaluate(() => (window as any).offkilter.toDxf());
  expect(dxf).toContain("ENTITIES");
  expect((dxf.match(/\r\nLINE\r\n/g) ?? []).length).toBeGreaterThanOrEqual(4);
  expect(dxf.trimEnd().endsWith("EOF")).toBe(true);
});

test("boolean feature subtracts and unions separate bodies", async ({ page }) => {
  await openDemo(page);
  const plate = await volume(page);
  // A separate cylinder body through the plate.
  await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    app.apply({ type: "add_sketch", plane: { type: "standard", base: "top", offset: -5 }, name: "Pin" });
    const s = app.summary.features[app.summary.features.length - 1].id;
    app.apply({ type: "sketch", id: s, op: { type: "add_circle", center: { x: 10, y: 10 }, radius: 3 } });
    app.apply({ type: "add_extrude", sketch: s, depth: 30, op: "new", name: "Pin body" });
  });
  await page.waitForTimeout(300);
  expect(await page.$$eval("#part-list li .pname", (els) => els.length)).toBe(2);
  const both = await volume(page);
  const pin = both - plate;
  await page.click("#btn-add-boolean");
  await page.waitForTimeout(400);
  expect(await featureNames(page)).toContain("Boolean 1");
  // Subtract by default: the pin is consumed and its slice through the plate removed.
  expect(await page.$$eval("#part-list li .pname", (els) => els.length)).toBe(1);
  const cut = await volume(page);
  expect(cut).toBeLessThan(plate);
  expect(await page.$$eval("#detail-body .body-pick input", (els) => els.length)).toBe(4);
  // Switch to union: one body holding everything.
  await page.selectOption("#detail-body select", "union");
  await page.waitForTimeout(400);
  expect(await page.$$eval("#part-list li .pname", (els) => els.length)).toBe(1);
  const united = await volume(page);
  expect(united).toBeGreaterThan(plate);
  expect(united).toBeLessThan(plate + pin);
  expect(await page.$$eval("#feature-list li .dot.err", (els) => els.length)).toBe(0);
});

test("feature pattern replays a hole instead of copying bodies", async ({ page }) => {
  await openDemo(page);
  const before = await volume(page);
  // A hole through the plate, then a pattern that names the hole feature.
  const hole = await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    app.apply({ type: "add_sketch", plane: { type: "standard", base: "top", offset: 0 }, name: "Hole points" });
    const s = app.summary.features[app.summary.features.length - 1].id;
    app.apply({ type: "sketch", id: s, op: { type: "add_point", pos: { x: 8, y: 8 } } });
    app.apply({ type: "add_hole", sketch: s, diameter: 4, through_all: true, direction: "normal", name: "Hole 1" });
    return app.summary.features[app.summary.features.length - 1].id;
  });
  await page.waitForTimeout(300);
  const oneHole = await volume(page);
  const holeVolume = before - oneHole;
  expect(holeVolume).toBeGreaterThan(50);
  await page.click("#btn-add-pattern");
  await page.waitForTimeout(300);
  // Tick the hole in the pattern's feature list.
  const labels = await page.$$eval("#detail-body .body-pick li span", (els) => els.map((e) => e.textContent));
  expect(labels).toContain("Hole 1");
  const index = labels.indexOf("Hole 1");
  await page.locator("#detail-body .body-pick input").nth(index).check();
  await page.waitForTimeout(400);
  expect(await page.evaluate((id) => (window as unknown as { offkilter: any }).offkilter.summary.features.at(-1).kind.features, hole)).toEqual([hole]);
  // Two holes 20 mm apart along X, still one body.
  expect(await page.$$eval("#part-list li .pname", (els) => els.length)).toBe(1);
  expect(before - (await volume(page))).toBeCloseTo(2 * holeVolume, 0);
  expect(await page.$$eval("#feature-list li .dot.err", (els) => els.length)).toBe(0);
});

test("shell hollows a box and opens a picked face", async ({ page }) => {
  await openDemo(page);
  await page.click("#btn-new");
  await page.waitForTimeout(300);
  await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    app.apply({ type: "add_sketch", plane: { type: "standard", base: "top", offset: 0 }, name: "Sketch 1" });
    const s = app.summary.features[app.summary.features.length - 1].id;
    app.apply({ type: "sketch", id: s, op: { type: "add_rectangle", a: { x: 0, y: 0 }, b: { x: 60, y: 40 } } });
    app.apply({ type: "add_extrude", sketch: s, depth: 20, op: "new", name: "Extrude 1" });
  });
  await page.waitForTimeout(300);
  const before = await volume(page);
  expect(before).toBeCloseTo(60 * 40 * 20, 3);
  // Select the top face by clicking it, then add a shell: the selected face opens.
  await page.keyboard.press("f");
  await page.waitForTimeout(300);
  let picked = "";
  for (const [fx, fy] of [[0.5, 0.45], [0.5, 0.4], [0.45, 0.5], [0.55, 0.42]] as const) {
    await clickViewport(page, fx, fy);
    picked = await status(page);
    if (picked.startsWith("Face:")) break;
  }
  expect(picked).toContain("Face: Extrude 1");
  await page.click("#btn-add-shell");
  await page.waitForTimeout(600);
  expect(await featureNames(page)).toContain("Shell 1");
  expect(await page.$$eval("#feature-list li .dot.err", (els) => els.length)).toBe(0);
  expect(await page.$$eval("#detail-body .edge-list li", (els) => els.length)).toBe(1);
  expect(await volume(page)).toBeCloseTo(60 * 40 * 20 - 56 * 36 * 18, 3);
  // Closing the face again gives a closed hollow.
  await page.getByRole("button", { name: "Clear" }).click();
  await page.waitForTimeout(600);
  expect(await volume(page)).toBeCloseTo(60 * 40 * 20 - 56 * 36 * 16, 3);
  // Undo twice restores the solid box.
  await page.keyboard.press("Control+z");
  await page.keyboard.press("Control+z");
  await page.waitForTimeout(500);
  expect(await volume(page)).toBeCloseTo(before, 3);
});

test("polygon and slot sketch tools", async ({ page }) => {
  page.on("dialog", (d) => d.accept(d.type() === "prompt" ? (d.defaultValue() || "top") : d.defaultValue()));
  await page.goto("/");
  await ready(page);
  await page.click("#btn-new");
  await page.click("#btn-add-sketch");
  await page.waitForTimeout(300);
  // Hexagon (the prompt's default of 6 sides is accepted): centre then a corner.
  await page.keyboard.press("p");
  await clickViewport(page, 0.35, 0.5);
  await clickViewport(page, 0.42, 0.5);
  // Slot: two centres and a width point.
  await page.keyboard.press("n");
  await clickViewport(page, 0.55, 0.45);
  await clickViewport(page, 0.7, 0.45);
  await clickViewport(page, 0.6, 0.5);
  const kinds = await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    const f = app.summary.features.find((x: any) => x.kind.type === "sketch");
    const counts: Record<string, number> = {};
    for (const e of f.kind.sketch.entities) counts[e.type] = (counts[e.type] ?? 0) + 1;
    return { counts, regions: app.summary.sketches[String(f.id)].profiles.length, dof: app.summary.sketches[String(f.id)].solve.dof };
  });
  expect(kinds.counts.line).toBe(6 + 2);
  expect(kinds.counts.arc).toBe(2);
  expect(kinds.regions).toBe(2);
  // Hexagon: centre, size, rotation (4); slot: two centres and the radius (5).
  expect(kinds.dof).toBe(9);
  await page.getByRole("button", { name: "Done" }).click();
  await page.click("#btn-add-extrude");
  await page.waitForTimeout(300);
  expect(await status(page)).toContain("1 body");
});

test("spline sketch tool draws a smooth curve that bounds a region", async ({ page }) => {
  page.on("dialog", (d) => d.accept(d.defaultValue() || "top"));
  await page.goto("/");
  await ready(page);
  await page.click("#btn-new");
  await page.click("#btn-add-sketch");
  await page.waitForTimeout(300);
  // Four points, Enter finishes; then a line back from the last point to the first.
  await page.keyboard.press("b");
  await clickViewport(page, 0.35, 0.5);
  await clickViewport(page, 0.45, 0.38);
  await clickViewport(page, 0.55, 0.38);
  await clickViewport(page, 0.65, 0.5);
  await page.keyboard.press("Enter");
  await page.waitForTimeout(200);
  await page.keyboard.press("l");
  await clickViewport(page, 0.65, 0.5);
  await clickViewport(page, 0.35, 0.5);
  const info = await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    const f = app.summary.features.find((x: any) => x.kind.type === "sketch");
    const counts: Record<string, number> = {};
    for (const e of f.kind.sketch.entities) counts[e.type] = (counts[e.type] ?? 0) + 1;
    const sk = app.summary.sketches[String(f.id)];
    return { counts, regions: sk.profiles.length, dof: sk.solve.dof, coincident: f.kind.sketch.constraints.filter((c: any) => c.type === "coincident").length };
  });
  expect(info.counts.spline).toBe(1);
  expect(info.counts.line).toBe(1);
  expect(info.counts.point).toBe(6);
  expect(info.coincident).toBe(2);
  expect(info.regions).toBe(1);
  // Four free spline points and nothing else: the line's ends are tied to them.
  expect(info.dof).toBe(8);
  await page.getByRole("button", { name: "Done" }).click();
  await page.click("#btn-add-extrude");
  await page.waitForTimeout(300);
  expect(await status(page)).toContain("1 body");
  // The spline wall is one smooth surface: bottom, top, spline, line.
  const surfaces = await page.evaluate(() => new Set((window as any).offkilter.kernel.studio.body_face_surfaces(0)).size);
  expect(surfaces).toBe(4);
});

test("sketch on an angled plane", async ({ page }) => {
  page.on("dialog", (d) => d.accept());
  await page.goto("/");
  await ready(page);
  await page.click("#btn-new");
  await page.waitForTimeout(200);
  await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    app.apply({ type: "add_sketch", plane: { type: "rotated", base: "top", axis: "x", angle: 30, offset: 0 }, name: "Tilted" });
    const s = app.summary.features[app.summary.features.length - 1].id;
    app.apply({ type: "sketch", id: s, op: { type: "add_rectangle", a: { x: 0, y: 0 }, b: { x: 10, y: 10 } } });
    app.apply({ type: "add_extrude", sketch: s, depth: 4, name: "Wedge" });
    app.select(s);
  });
  expect(await status(page)).toContain("1 body");
  // The extrusion's top face normal is tilted 30° from +Z towards -Y.
  const normal = await page.evaluate(() => (window as unknown as { offkilter: any }).offkilter.summary.bodies[0].faces.find((f: any) => f.origin.local === 1).normal);
  expect(normal.z).toBeCloseTo(Math.cos(Math.PI / 6), 5);
  expect(normal.y).toBeCloseTo(-Math.sin(Math.PI / 6), 5);
  // The sketch panel shows the angled plane's fields; changing the angle re-tilts the body.
  await expect(page.locator("#detail-body")).toContainText("About axis");
  const angle = page.locator("#detail-body .field").filter({ hasText: "Angle °" }).locator("input");
  await angle.fill("45");
  await angle.press("Enter");
  await page.waitForTimeout(300);
  const n2 = await page.evaluate(() => (window as unknown as { offkilter: any }).offkilter.summary.bodies[0].faces.find((f: any) => f.origin.local === 1).normal);
  expect(n2.z).toBeCloseTo(Math.cos(Math.PI / 4), 5);
});

test("split a body by a plane into two parts", async ({ page }) => {
  await openDemo(page);
  const before = await volume(page);
  await page.click("#btn-add-split");
  await page.waitForTimeout(300);
  expect(await status(page)).toContain("2 bodies");
  expect(await volume(page)).toBeCloseTo(before, 3);
  await expect(page.locator("#detail-title")).toContainText("Split");
  await expect(page.locator("#part-list li")).toHaveCount(2);
  // Undo removes the split again.
  await page.click("#viewport");
  await page.keyboard.press("Control+z");
  await page.waitForTimeout(300);
  expect(await status(page)).toContain("1 body");
});

test("import a binary STL box as a body", async ({ page }) => {
  page.on("dialog", (d) => d.accept());
  await page.goto("/");
  await ready(page);
  await page.click("#btn-new");
  await page.waitForTimeout(200);
  // A 10 x 20 x 5 box as twelve binary STL facets with repeated corners.
  const v = [[0, 0, 0], [10, 0, 0], [10, 20, 0], [0, 20, 0], [0, 0, 5], [10, 0, 5], [10, 20, 5], [0, 20, 5]];
  const quads = [[0, 3, 2, 1], [4, 5, 6, 7], [0, 1, 5, 4], [1, 2, 6, 5], [2, 3, 7, 6], [3, 0, 4, 7]];
  const tris = quads.flatMap((q) => [[q[0]!, q[1]!, q[2]!], [q[0]!, q[2]!, q[3]!]]);
  const buf = Buffer.alloc(84 + tris.length * 50);
  buf.write("offkilter test box", 0, "latin1");
  buf.writeUInt32LE(tris.length, 80);
  let at = 84;
  for (const t of tris) {
    at += 12;
    for (const i of t) {
      for (const c of v[i]!) {
        buf.writeFloatLE(c, at);
        at += 4;
      }
    }
    at += 2;
  }
  await page.locator("#stl-input").setInputFiles({ name: "box.stl", mimeType: "model/stl", buffer: buf });
  await page.waitForTimeout(400);
  expect(await status(page)).toContain("1 body");
  expect(await volume(page)).toBeCloseTo(1000, 3);
  expect(await featureNames(page)).toContain("box");
  const faces = await page.evaluate(() => (window as unknown as { offkilter: any }).offkilter.summary.bodies[0].faces.length as number);
  expect(faces).toBe(6);
  await page.click("#feature-list li");
  await expect(page.locator("#detail-body")).toContainText("8 vertices, 12 triangles");
});

test("drawing views remove hidden lines and export as SVG and DXF", async ({ page }) => {
  await openDemo(page);
  const counts = await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    const views = app.drawingViews();
    return Object.fromEntries(views.map((v: any) => [v.name, { visible: v.lines.visible.length, hidden: v.lines.hidden.length }]));
  });
  // Every view has an outline; the front view hides the slot and boss behind the plate face.
  for (const name of ["front", "top", "right", "iso"]) expect(counts[name].visible).toBeGreaterThan(3);
  expect(counts.front.hidden).toBeGreaterThan(0);
  const svg: string = await page.evaluate(() => (window as unknown as { offkilter: any }).offkilter.toDrawingSvg());
  expect(svg.startsWith("<svg")).toBe(true);
  expect(svg).toContain('id="view-front"');
  expect(svg).toContain('id="view-iso"');
  expect(svg).toContain('class="hidden"');
  expect(svg).toContain("Scale 1:");
  // Overall dimensions of the 60 x 40 plate: width and height on the front view, depth on the top view.
  expect((svg.match(/class="dimension"/g) ?? []).length).toBe(3);
  expect(svg).toMatch(/>60<\/text>/);
  expect(svg).toMatch(/>40<\/text>/);
  // A section A-A through the middle of the plate: hatched cut faces and a trace on the top view.
  const section = await page.evaluate(() => {
    const v = (window as unknown as { offkilter: any }).offkilter.drawingViews().find((x: any) => x.name === "section");
    return v ? { cut: v.cut.length, visible: v.lines.visible.length, trace: v.trace } : null;
  });
  expect(section).not.toBeNull();
  expect(section!.cut).toBeGreaterThan(0);
  expect(section!.trace.on).toBe("top");
  expect(svg).toContain('id="view-section"');
  expect(svg).toContain('class="hatch"');
  expect(svg).toContain("SECTION A-A");
  expect(svg).toContain('class="trace"');
  // The drawing dialog previews the sheet and its options change what is drawn.
  await page.selectOption("#export", "drawing");
  await expect(page.locator("#drawing-dialog")).toBeVisible();
  await expect(page.locator("#drawing-preview svg")).toHaveCount(1);
  await page.uncheck("#dv-iso");
  await page.selectOption("#dv-sheet", "A3");
  await expect(page.locator("#drawing-preview svg")).toHaveAttribute("width", "420mm");
  await expect(page.locator('#drawing-preview svg g[id="view-iso"]')).toHaveCount(0);
  await expect(page.locator('#drawing-preview svg g[id="view-front"]')).toHaveCount(1);
  await page.click("#drawing-close");
  const svg2: string = await page.evaluate(() => (window as unknown as { offkilter: any }).offkilter.toDrawingSvg());
  expect(svg2).not.toContain('id="view-iso"');
  expect(svg2).toContain("A3");
  // Bill of materials for the part studio: one row per body.
  const bom: string = await page.evaluate(() => (window as unknown as { offkilter: any }).offkilter.toBom());
  expect(bom.split("\n")[0]).toBe("part,material,mass_g,volume_mm3,surface_mm2,size_mm,faces");
  expect(bom.trim().split("\n").length).toBe(2);
  const dxf: string = await page.evaluate(() => (window as unknown as { offkilter: any }).offkilter.toDrawingDxf());
  expect((dxf.match(/\r\nHIDDEN\r\n/g) ?? []).length).toBeGreaterThan(0);
  expect((dxf.match(/\r\nDIMENSIONS\r\n/g) ?? []).length).toBeGreaterThan(3);
  expect((dxf.match(/\r\nLINE\r\n/g) ?? []).length).toBeGreaterThan(20);
});

test("move face and draft edit a box directly", async ({ page }) => {
  await openDemo(page);
  await page.click("#btn-new");
  await page.waitForTimeout(300);
  const block: number = await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    app.apply({ type: "add_sketch", plane: { type: "standard", base: "top", offset: 0 }, name: "Sketch 1" });
    const s = app.summary.features[app.summary.features.length - 1].id;
    app.apply({ type: "sketch", id: s, op: { type: "add_rectangle", a: { x: 0, y: 0 }, b: { x: 60, y: 40 } } });
    app.apply({ type: "add_extrude", sketch: s, depth: 20, op: "new", name: "Extrude 1" });
    return app.summary.features[app.summary.features.length - 1].id;
  });
  await page.waitForTimeout(300);
  // Pick the top face, then Move face pulls it 5 mm by default.
  await page.keyboard.press("f");
  await page.waitForTimeout(300);
  let picked = "";
  for (const [fx, fy] of [[0.5, 0.45], [0.5, 0.4], [0.45, 0.5], [0.55, 0.42]] as const) {
    await clickViewport(page, fx, fy);
    picked = await status(page);
    if (picked.startsWith("Face:")) break;
  }
  expect(picked).toContain("Face: Extrude 1");
  await page.click("#btn-add-move-face");
  await page.waitForTimeout(500);
  expect(await featureNames(page)).toContain("Move face 1");
  expect(await volume(page)).toBeCloseTo(60 * 40 * 25, 3);
  expect(await page.$$eval("#feature-list li .dot.err", (els) => els.length)).toBe(0);
  // Draft the four walls 10° about the base: the box tapers upward.
  await page.evaluate((block) => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    const faces = [2, 3, 4, 5].map((local) => ({ feature: block, local }));
    app.apply({ type: "add_draft", faces, neutral: { type: "standard", base: "top", offset: 0 }, angle: 10, name: "Draft 1" });
  }, block);
  await page.waitForTimeout(500);
  const k = Math.tan((10 * Math.PI) / 180);
  let expected = 0;
  for (let i = 0; i < 2500; i++) {
    const z = (i + 0.5) / 100;
    expected += (60 - 2 * k * z) * (40 - 2 * k * z) * 0.01;
  }
  expect(await volume(page)).toBeCloseTo(expected, 0);
  expect(await page.$$eval("#feature-list li .dot.err", (els) => els.length)).toBe(0);
  await page.click("#feature-list li:last-child");
  await page.waitForTimeout(300);
  expect(await page.$$eval("#detail-body .edge-list li", (els) => els.length)).toBe(4);
});

test("assembly explode slider and dragging a free instance", async ({ page }) => {
  page.on("dialog", (d) => d.accept(d.defaultValue()));
  await page.goto("/");
  await ready(page);
  await page.click("#btn-new");
  await page.waitForTimeout(200);
  await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    app.apply({ type: "add_sketch", plane: { type: "standard", base: "top", offset: 0 }, name: "Sketch 1" });
    const s = app.summary.features[app.summary.features.length - 1].id;
    app.apply({ type: "sketch", id: s, op: { type: "add_rectangle", a: { x: 0, y: 0 }, b: { x: 10, y: 10 } } });
    app.apply({ type: "add_extrude", sketch: s, depth: 5, name: "Block" });
  });
  await page.getByRole("button", { name: "+ Assembly" }).click();
  await expect(page.locator("#assembly-panel")).toBeVisible();
  await page.getByRole("button", { name: "Insert", exact: true }).click();
  await page.click("#btn-insert-instance");
  await page.getByRole("button", { name: "Insert", exact: true }).click();
  await expect(page.locator("#instance-list li")).toHaveCount(2);
  // Place the second instance 30 mm along X so the two are apart.
  await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    const b = app.summary.instances[1].id;
    app.applyDoc({ type: "assembly", tab: app.tab, op: { type: "set_instance", id: b, placement: { position: { x: 30, y: 0, z: 0 }, rotation: { x: 0, y: 0, z: 0 } } } });
  });
  await page.waitForTimeout(200);
  // Explode: bodies slide apart in the viewer only; the document is unchanged.
  await page.locator("#explode").fill("100");
  await page.locator("#explode").dispatchEvent("input");
  const offsets = await page.evaluate(() => {
    const v = (window as unknown as { offkilter: any }).offkilter.viewer;
    return [v.bodyOffset(0), v.bodyOffset(1)];
  });
  expect(offsets[0].x).toBeLessThan(-10);
  expect(offsets[1].x).toBeGreaterThan(10);
  expect(await page.evaluate(() => (window as unknown as { offkilter: any }).offkilter.summary.instances[1].placement.position.x)).toBe(30);
  await page.locator("#explode").fill("0");
  await page.locator("#explode").dispatchEvent("input");
  // Move mode: drag the second instance by 80 pixels; its placement follows.
  await page.keyboard.press("f");
  await page.waitForTimeout(300);
  await page.click("#btn-move-instance");
  const centre = await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    const [lo, hi] = app.summary.bodies[1].bounds;
    return app.viewer.toScreen({ x: (lo.x + hi.x) / 2, y: (lo.y + hi.y) / 2, z: hi.z });
  });
  const vp = (await page.locator("#viewport").boundingBox())!;
  await page.mouse.move(vp.x + centre.x, vp.y + centre.y);
  await page.mouse.down();
  await page.mouse.move(vp.x + centre.x + 40, vp.y + centre.y, { steps: 4 });
  await page.mouse.move(vp.x + centre.x + 80, vp.y + centre.y, { steps: 4 });
  await page.mouse.up();
  await page.waitForTimeout(300);
  const after = await page.evaluate(() => (window as unknown as { offkilter: any }).offkilter.summary.instances[1].placement.position);
  expect(Math.hypot(after.x - 30, after.y, after.z)).toBeGreaterThan(1);
  await page.keyboard.press("Escape");
  // One undo step reverts the whole drag.
  await page.keyboard.press("Control+z");
  await page.waitForTimeout(300);
  expect(await page.evaluate(() => (window as unknown as { offkilter: any }).offkilter.summary.instances[1].placement.position.x)).toBeCloseTo(30, 6);
});

test("sketch pattern copies the selection with constraints", async ({ page }) => {
  // The pattern prompt's default ("linear 3,20,0") is accepted as is.
  page.on("dialog", (d) => d.accept(d.defaultValue()));
  await page.goto("/");
  await ready(page);
  await page.click("#btn-new");
  await page.click("#btn-add-sketch");
  await page.waitForTimeout(300);
  await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    const id = app.selected;
    const r = app.applyRaw({ type: "sketch", id, op: { type: "add_rectangle", a: { x: 0, y: 0 }, b: { x: 10, y: 5 } } });
    app.sketcher.selection = new Set(r.entities);
    app.regenerate();
  });
  await page.keyboard.press("y");
  await page.waitForTimeout(400);
  const info = await page.evaluate(() => {
    const app = (window as unknown as { offkilter: any }).offkilter;
    const f = app.summary.features.find((x: any) => x.kind.type === "sketch");
    const r = app.summary.sketches[String(f.id)];
    return { lines: f.kind.sketch.entities.filter((e: any) => e.type === "line").length, regions: r.profiles.length, dof: r.solve.dof };
  });
  expect(info.lines).toBe(12);
  expect(info.regions).toBe(3);
  expect(info.dof).toBe(4);
  await page.getByRole("button", { name: "Done" }).click();
  await page.click("#btn-add-extrude");
  await page.waitForTimeout(300);
  expect(await volume(page)).toBeCloseTo(3 * 10 * 5 * 10, 3);
});
