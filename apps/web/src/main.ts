import { Kernel } from "./kernel";
import type { Constraint, FeatureSummary, Op, PlaneSpec, SketchData, SketchOp, Summary } from "./kernel";
import { Viewer } from "./viewer";

const $ = <T extends HTMLElement>(sel: string): T => {
  const el = document.querySelector<T>(sel);
  if (!el) throw new Error(`missing element ${sel}`);
  return el;
};

const STORAGE_KEY = "offkilter.partstudio";

class App {
  kernel: Kernel;
  summary!: Summary;
  selected: number | null = null;
  viewer = new Viewer($("#viewport"));

  constructor(kernel: Kernel) {
    this.kernel = kernel;
    this.regenerate();
    this.viewer.fitAll();
  }

  // ------------------------------------------------------------ document

  replace(kernel: Kernel): void {
    this.kernel.dispose();
    this.kernel = kernel;
    this.selected = null;
    this.regenerate();
    this.viewer.fitAll();
  }

  apply(op: Op): void {
    try {
      this.kernel.apply(op);
    } catch (e) {
      this.setStatus(`error: ${(e as Error).message}`);
      return;
    }
    this.regenerate();
  }

  regenerate(): void {
    const t0 = performance.now();
    this.summary = this.kernel.regenerate();
    const dt = performance.now() - t0;
    this.viewer.setBodies(this.kernel.bodyMeshes());
    this.viewer.setSketches(this.summary.sketches, this.selected);
    this.renderFeatures();
    this.renderDetail();
    const faces = this.summary.bodies.reduce((n, b) => n + b.faces, 0);
    const volume = this.summary.bodies.reduce((n, b) => n + b.volume, 0);
    const errors = this.summary.features.filter((f) => f.error).length;
    this.setStatus(
      `${this.summary.bodies.length} ${this.summary.bodies.length === 1 ? "body" : "bodies"} · ${faces} faces · ${volume.toFixed(1)} mm³ · regen ${dt.toFixed(1)} ms` +
        (errors ? ` · ${errors} feature error${errors > 1 ? "s" : ""}` : "") +
        ` · kernel v${Kernel.version()}`,
    );
    ($("#studio-name") as HTMLInputElement).value = this.summary.name;
    try {
      localStorage.setItem(STORAGE_KEY, this.kernel.toJson());
    } catch {
      /* storage unavailable */
    }
  }

  setStatus(text: string): void {
    $("#status-text").textContent = text;
  }

  select(id: number | null): void {
    this.selected = id;
    this.viewer.setSketches(this.summary.sketches, this.selected);
    this.renderFeatures();
    this.renderDetail();
  }

  feature(id: number | null): FeatureSummary | undefined {
    return this.summary.features.find((f) => f.id === id);
  }

  // ------------------------------------------------------------ feature list

  renderFeatures(): void {
    const ul = $("#feature-list");
    ul.innerHTML = "";
    for (const f of this.summary.features) {
      const li = document.createElement("li");
      li.className = (f.id === this.selected ? "selected " : "") + (f.suppressed ? "suppressed" : "");
      const dot = document.createElement("span");
      dot.className = "dot " + (f.suppressed ? "off" : f.error ? "err" : this.sketchWarn(f) ? "warn" : "");
      dot.title = f.error ?? (this.sketchWarn(f) ? "sketch is under-constrained" : "ok");
      const icon = document.createElement("span");
      icon.className = "icon";
      icon.textContent = f.kind.type === "sketch" ? "✎" : "⬒";
      const name = document.createElement("span");
      name.className = "name";
      name.textContent = f.name;
      li.append(dot, icon, name);
      li.onclick = () => this.select(f.id);
      li.ondblclick = () => {
        const n = prompt("Rename feature", f.name);
        if (n && n !== f.name) this.apply({ type: "rename_feature", id: f.id, name: n });
      };
      ul.appendChild(li);
    }
    if (this.summary.features.length === 0) {
      const li = document.createElement("li");
      li.className = "note";
      li.textContent = "No features yet. Add a sketch to begin.";
      ul.appendChild(li);
    }
    const sel = this.feature(this.selected);
    ($("#btn-add-extrude") as HTMLButtonElement).disabled = !(sel && sel.kind.type === "sketch");
  }

  sketchWarn(f: FeatureSummary): boolean {
    if (f.kind.type !== "sketch") return false;
    const r = this.summary.sketches[String(f.id)];
    return !!r && r.solve.status === "under_constrained";
  }

  // ------------------------------------------------------------ detail panel

  renderDetail(): void {
    const f = this.feature(this.selected);
    const title = $("#detail-title");
    const body = $("#detail-body");
    body.innerHTML = "";
    if (!f) {
      title.textContent = "Nothing selected";
      body.innerHTML = `<p class="note">Select a feature to edit it. Drag to orbit, scroll to zoom, right-drag to pan.</p>`;
      return;
    }
    title.textContent = f.name;
    if (f.error) {
      const e = document.createElement("div");
      e.className = "error";
      e.textContent = f.error;
      body.appendChild(e);
    }
    if (f.kind.type === "sketch") this.renderSketchDetail(f, body);
    else this.renderExtrudeDetail(f, body);

    const row = document.createElement("div");
    row.className = "row";
    row.append(
      button(f.suppressed ? "Unsuppress" : "Suppress", () => this.apply({ type: "set_suppressed", id: f.id, suppressed: !f.suppressed })),
      button("Move up", () => this.moveFeature(f.id, -1)),
      button("Move down", () => this.moveFeature(f.id, +1)),
      button("Delete", () => {
        if (confirm(`Delete ${f.name}?`)) {
          this.selected = null;
          this.apply({ type: "delete_feature", id: f.id });
        }
      }, "danger"),
    );
    body.appendChild(row);
  }

  moveFeature(id: number, delta: number): void {
    const i = this.summary.features.findIndex((f) => f.id === id);
    const j = i + delta;
    if (i < 0 || j < 0 || j >= this.summary.features.length) return;
    this.apply({ type: "move_feature", id, index: j });
  }

  renderSketchDetail(f: FeatureSummary, body: HTMLElement): void {
    if (f.kind.type !== "sketch") return;
    const kind = f.kind;
    const result = this.summary.sketches[String(f.id)];

    body.appendChild(
      field("Plane", select(["top", "front", "right"], kind.plane.base, (v) =>
        this.apply({ type: "set_sketch_plane", id: f.id, plane: { base: v as PlaneSpec["base"], offset: kind.plane.offset } }),
      )),
    );
    body.appendChild(
      field("Offset", numberInput(kind.plane.offset, (v) =>
        this.apply({ type: "set_sketch_plane", id: f.id, plane: { base: kind.plane.base, offset: v } }),
      )),
    );

    if (result) {
      const s = result.solve;
      const cls = s.status === "fully_constrained" ? "ok" : s.status === "under_constrained" ? "warn" : "err";
      const label = s.status === "fully_constrained" ? "fully constrained" : s.status === "under_constrained" ? `${s.dof} degrees of freedom` : "inconsistent";
      const p = document.createElement("p");
      p.className = "note";
      p.innerHTML = `Solver: <span class="badge ${cls}">${label}</span> · ${s.equations} equations, ${s.parameters} parameters · ${result.profiles.length} closed region${result.profiles.length === 1 ? "" : "s"}`;
      body.appendChild(p);
    }

    body.appendChild(heading("Geometry"));
    const ents = document.createElement("p");
    ents.className = "note";
    const counts: Record<string, number> = {};
    for (const e of kind.sketch.entities) counts[e.type] = (counts[e.type] ?? 0) + 1;
    ents.textContent = Object.entries(counts).map(([k, n]) => `${n} ${k}${n === 1 ? "" : "s"}`).join(", ") || "empty";
    body.appendChild(ents);

    body.appendChild(this.geometryForm(f.id));

    body.appendChild(heading("Constraints"));
    const ul = document.createElement("ul");
    ul.className = "constraint-list";
    for (const c of kind.sketch.constraints) {
      const li = document.createElement("li");
      const k = document.createElement("span");
      k.className = "kind";
      k.innerHTML = `${c.type.replace(/_/g, " ")}<br><span class="refs">${describeRefs(c, kind.sketch)}</span>`;
      li.appendChild(k);
      if ("value" in c) {
        li.appendChild(numberInput(c.value, (v) => this.apply({ type: "sketch", id: f.id, op: { type: "set_constraint_value", id: c.id, value: v } })));
      } else {
        li.appendChild(document.createElement("span"));
      }
      li.appendChild(button("×", () => this.apply({ type: "sketch", id: f.id, op: { type: "remove_constraint", id: c.id } }), "danger"));
      ul.appendChild(li);
    }
    if (kind.sketch.constraints.length === 0) {
      const li = document.createElement("li");
      li.className = "note";
      li.textContent = "no constraints";
      ul.appendChild(li);
    }
    body.appendChild(ul);
    body.appendChild(this.constraintForm(f.id, kind.sketch));
  }

  geometryForm(sketchId: number): HTMLElement {
    const wrap = document.createElement("div");
    const shape = select(["rectangle", "circle", "line"], "rectangle", () => render());
    wrap.appendChild(field("Add", shape));
    const inputs = document.createElement("div");
    wrap.appendChild(inputs);
    const render = () => {
      inputs.innerHTML = "";
      const vals: Record<string, HTMLInputElement> = {};
      const spec = shape.value === "circle" ? ["cx", "cy", "radius"] : ["x0", "y0", "x1", "y1"];
      const defaults: Record<string, number> = { cx: 0, cy: 0, radius: 10, x0: 0, y0: 0, x1: 40, y1: 20 };
      for (const k of spec) {
        vals[k] = numberInput(defaults[k] ?? 0, () => {});
        inputs.appendChild(field(k, vals[k]));
      }
      const row = document.createElement("div");
      row.className = "row";
      row.appendChild(
        button("Add geometry", () => {
          const n = (k: string) => Number(vals[k]?.value ?? 0);
          let op: SketchOp;
          if (shape.value === "circle") op = { type: "add_circle", center: { x: n("cx"), y: n("cy") }, radius: n("radius") };
          else if (shape.value === "line") op = { type: "add_line", a: { x: n("x0"), y: n("y0") }, b: { x: n("x1"), y: n("y1") } };
          else op = { type: "add_rectangle", a: { x: n("x0"), y: n("y0") }, b: { x: n("x1"), y: n("y1") } };
          this.apply({ type: "sketch", id: sketchId, op });
        }, "primary"),
      );
      inputs.appendChild(row);
    };
    render();
    return wrap;
  }

  constraintForm(sketchId: number, sketch: SketchData): HTMLElement {
    const wrap = document.createElement("div");
    const types = [
      "coincident", "fixed", "horizontal", "vertical", "distance", "horizontal_distance", "vertical_distance", "length",
      "radius", "diameter", "equal", "parallel", "perpendicular", "angle", "point_on_line", "point_on_circle", "midpoint", "tangent",
    ];
    const type = select(types, "length", () => render());
    wrap.appendChild(field("Add", type));
    const inputs = document.createElement("div");
    wrap.appendChild(inputs);
    const entityOptions = sketch.entities.map((e) => `${e.id}: ${e.type}`);
    const render = () => {
      inputs.innerHTML = "";
      const t = type.value;
      const refs = constraintRefFields(t);
      const sels: Record<string, HTMLSelectElement> = {};
      for (const r of refs) {
        sels[r] = select(entityOptions, entityOptions[0] ?? "", () => {});
        inputs.appendChild(field(r, sels[r]));
      }
      let valueInput: HTMLInputElement | null = null;
      if (constraintHasValue(t)) {
        valueInput = numberInput(10, () => {});
        inputs.appendChild(field("value", valueInput));
      }
      const row = document.createElement("div");
      row.className = "row";
      row.appendChild(
        button("Add constraint", () => {
          const c: Record<string, unknown> = { type: t };
          for (const r of refs) c[r] = Number(sels[r]?.value.split(":")[0]);
          if (valueInput) c.value = Number(valueInput.value);
          this.apply({ type: "sketch", id: sketchId, op: { type: "add_constraint", constraint: c as unknown as Constraint } });
        }, "primary"),
      );
      inputs.appendChild(row);
    };
    render();
    return wrap;
  }

  renderExtrudeDetail(f: FeatureSummary, body: HTMLElement): void {
    if (f.kind.type !== "extrude") return;
    const k = f.kind;
    const sketchNames = this.summary.features.filter((s) => s.kind.type === "sketch").map((s) => `${s.id}: ${s.name}`);
    const current = sketchNames.find((n) => n.startsWith(`${k.sketch}:`)) ?? `${k.sketch}: (missing)`;
    body.appendChild(field("Sketch", (() => {
      const s = select(sketchNames.includes(current) ? sketchNames : [current, ...sketchNames], current, () => {});
      s.disabled = true;
      return s;
    })()));
    body.appendChild(field("Depth", numberInput(k.depth, (v) => this.apply({ type: "set_extrude", id: f.id, depth: v }))));
    body.appendChild(
      field("Direction", select(["normal", "reverse", "symmetric"], k.direction, (v) =>
        this.apply({ type: "set_extrude", id: f.id, direction: v as typeof k.direction }),
      )),
    );
    const regions = this.summary.sketches[String(k.sketch)]?.profiles.length ?? 0;
    const profileChoices = ["all", "largest", ...Array.from({ length: regions }, (_, i) => `region ${i}`)];
    const profileValue = k.profiles.type === "indices" ? `region ${k.profiles.indices[0] ?? 0}` : k.profiles.type;
    body.appendChild(
      field("Regions", select(profileChoices, profileValue, (v) => {
        const profiles = v === "all" ? { type: "all" as const } : v === "largest" ? { type: "largest" as const } : { type: "indices" as const, indices: [Number(v.slice(7))] };
        this.apply({ type: "set_extrude", id: f.id, profiles });
      })),
    );
    body.appendChild(
      field("Result", select(["new", "add", "remove", "intersect"], k.op, (v) => this.apply({ type: "set_extrude", id: f.id, op: v as typeof k.op }))),
    );
    const note = document.createElement("p");
    note.className = "note";
    note.textContent = "New creates a body. Add, remove and intersect apply to every existing body the extrusion touches.";
    body.appendChild(note);
  }
}

// ---------------------------------------------------------------- helpers

function button(label: string, onClick: () => void, cls = ""): HTMLButtonElement {
  const b = document.createElement("button");
  b.textContent = label;
  b.className = cls;
  b.onclick = onClick;
  return b;
}

function field(label: string, control: HTMLElement): HTMLElement {
  const d = document.createElement("div");
  d.className = "field";
  const l = document.createElement("label");
  l.textContent = label;
  d.append(l, control);
  return d;
}

function heading(text: string): HTMLElement {
  const h = document.createElement("h4");
  h.textContent = text;
  return h;
}

function numberInput(value: number, onCommit: (v: number) => void): HTMLInputElement {
  const i = document.createElement("input");
  i.type = "number";
  i.step = "any";
  i.value = String(Number(value.toFixed(6)));
  const commit = () => {
    const v = Number(i.value);
    if (Number.isFinite(v) && v !== value) onCommit(v);
  };
  i.onchange = commit;
  i.onkeydown = (e) => {
    if (e.key === "Enter") commit();
  };
  return i;
}

function select(options: string[], value: string, onChange: (v: string) => void): HTMLSelectElement {
  const s = document.createElement("select");
  for (const o of options) {
    const opt = document.createElement("option");
    opt.value = o;
    opt.textContent = o;
    s.appendChild(opt);
  }
  s.value = value;
  s.onchange = () => onChange(s.value);
  return s;
}

function constraintRefFields(t: string): string[] {
  switch (t) {
    case "coincident": case "equal": case "parallel": case "perpendicular":
    case "distance": case "horizontal_distance": case "vertical_distance": case "angle":
      return ["a", "b"];
    case "fixed": return ["point"];
    case "horizontal": case "vertical": case "length": return ["line"];
    case "radius": case "diameter": return ["entity"];
    case "point_on_line": case "midpoint": return ["point", "line"];
    case "point_on_circle": return ["point", "entity"];
    case "tangent": return ["line", "entity"];
    default: return [];
  }
}

function constraintHasValue(t: string): boolean {
  return ["distance", "horizontal_distance", "vertical_distance", "length", "radius", "diameter", "angle"].includes(t);
}

function describeRefs(c: Constraint & { id: number }, sketch: SketchData): string {
  const refs = constraintRefFields(c.type).map((k) => (c as unknown as Record<string, number>)[k]);
  return refs
    .map((id) => {
      const e = sketch.entities.find((e) => e.id === id);
      return e ? `${e.type} ${id}` : `#${id}`;
    })
    .join(", ");
}

// ---------------------------------------------------------------- bootstrap

async function main(): Promise<void> {
  await Kernel.load();
  let kernel: Kernel | null = null;
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved) kernel = Kernel.fromJson(saved);
  } catch {
    kernel = null;
  }
  const app = new App(kernel ?? Kernel.demo());

  $("#btn-new").onclick = () => {
    if (confirm("Start a new empty part studio? Unsaved work is lost.")) app.replace(Kernel.empty());
  };
  $("#btn-demo").onclick = () => app.replace(Kernel.demo());
  $("#btn-save").onclick = () => {
    const blob = new Blob([app.kernel.toJson()], { type: "application/json" });
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = `${app.summary.name.replace(/[^\w.-]+/g, "_")}.okpart`;
    a.click();
    URL.revokeObjectURL(a.href);
  };
  const fileInput = $("#file-input") as HTMLInputElement;
  $("#btn-open").onclick = () => fileInput.click();
  fileInput.onchange = async () => {
    const file = fileInput.files?.[0];
    if (!file) return;
    try {
      app.replace(Kernel.fromJson(await file.text()));
    } catch (e) {
      app.setStatus(`could not open file: ${(e as Error).message}`);
    }
    fileInput.value = "";
  };
  ($("#studio-name") as HTMLInputElement).onchange = (e) => {
    app.apply({ type: "rename_studio", name: (e.target as HTMLInputElement).value });
  };
  $("#btn-add-sketch").onclick = () => {
    const base = (prompt("Sketch plane: top, front or right", "top") ?? "").trim().toLowerCase();
    if (!["top", "front", "right"].includes(base)) return;
    const r = app.kernel.apply({ type: "add_sketch", plane: { base: base as PlaneSpec["base"], offset: 0 }, name: null });
    app.selected = r.feature;
    app.regenerate();
  };
  $("#btn-add-extrude").onclick = () => {
    const f = app.feature(app.selected);
    if (!f || f.kind.type !== "sketch") return;
    const r = app.kernel.apply({ type: "add_extrude", sketch: f.id, depth: 10, profiles: { type: "all" }, name: null });
    app.selected = r.feature;
    app.regenerate();
  };
  window.addEventListener("keydown", (e) => {
    if (e.key === "f" && !(e.target instanceof HTMLInputElement)) app.viewer.fitAll();
  });
}

main().catch((e) => {
  $("#status-text").textContent = `failed to start: ${(e as Error).message}`;
  console.error(e);
});
