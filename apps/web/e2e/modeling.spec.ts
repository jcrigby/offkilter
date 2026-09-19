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
  page.on("dialog", (d) => d.accept(d.type() === "prompt" ? "-1" : d.defaultValue()));
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

test("standard views, measure, section and part rename", async ({ page }) => {
  await openDemo(page);
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
});

test("export bodies as STL and 3MF and a sketch as DXF", async ({ page }) => {
  await openDemo(page);
  const sizes = await page.evaluate(async () => {
    const app = (window as any).offkilter;
    const stl = app.toStl();
    const mf = app.to3mf();
    const head = new Uint8Array(await mf.slice(0, 2).arrayBuffer());
    return { stl: stl.size, mf: mf.size, sig: String.fromCharCode(...head), dxfWithoutSketch: app.toDxf() };
  });
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
