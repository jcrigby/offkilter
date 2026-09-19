import { Kernel } from "./kernel";
import { to3mf, toDxf, toStl } from "./export";
import type { Axis, BlendKind, Connector, Constraint, DocOp, DocOpResult, EdgeRef, ExtrudeDirection, ExtrudeEnd, FaceRef, FeatureSummary, InstanceSummary, MateKind, MateSummary, Op, OpResult, PatternKind, PlaneRef, ProfileSelection, ProjectionSource, RevolveAxis, SketchData, SketchOp, StandardPlane, Summary, Vec3 } from "./kernel";
import { Viewer } from "./viewer";
import type { EdgePick, FacePick } from "./viewer";
import { Sketcher } from "./sketcher";
import type { SketchHost, Tool } from "./sketcher";
import { Sync } from "./sync";
import type { DocMeta, UserInfo, VersionMeta } from "./sync";

const $ = <T extends HTMLElement>(sel: string): T => {
  const el = document.querySelector<T>(sel);
  if (!el) throw new Error(`missing element ${sel}`);
  return el;
};

const STORAGE_KEY = "offkilter.partstudio";

class App implements SketchHost {
  kernel: Kernel;
  summary!: Summary;
  selected: number | null = null;
  sketcher = new Sketcher(this);
  sync = new Sync(
    {
      remoteOp: (op, base) => this.applyRemote(op, base),
      localHash: () => this.kernel.structuralHash(),
      loadDocument: (json) => this.loadDocument(json),
      presence: (clients, names) => this.showPresence(clients, names),
      status: (text) => this.setStatus(text),
    },
    localStorage.getItem("offkilter.user") ?? `user-${Math.floor(Math.random() * 1000)}`,
  );
  /** Face selected in the viewport, if any. */
  selectedFace: FaceRef | null = null;
  /** Pending face request from a panel: receives the picked face. */
  facePicker: ((face: FaceRef) => void) | null = null;
  /** Blend feature currently collecting edges; the view rolls back to before it. */
  edgePicking: number | null = null;
  /** Feature-list rollback: number of features shown, or null for all. */
  rollbackCount: number | null = null;
  /** Active tab id; null until the first regeneration picks the first part studio. */
  tab: number | null = null;
  /** Assembly selection. */
  selectedInstance: number | null = null;
  selectedMate: number | null = null;
  /** "+ Mate" in progress: the first connector once picked. */
  matePick: { a: Connector | null } | null = null;
  viewer = new Viewer($("#viewport"));

  constructor(kernel: Kernel) {
    this.kernel = kernel;
    this.viewer.onPick = (pick) => this.onViewportPick(pick);
    this.viewer.onEdgePick = (pick) => this.onEdgePick(pick);
    this.regenerate();
    this.viewer.fitAll();
  }

  // ------------------------------------------------------------ edges

  edgeRefOf(pick: EdgePick): EdgeRef | null {
    const faces = this.summary.bodies[pick.body]?.faces;
    const a = faces?.[pick.faces[0]]?.origin;
    const b = faces?.[pick.faces[1]]?.origin;
    return a && b ? { a, b } : null;
  }

  /** Locates an edge reference among the current bodies (first match). */
  findEdge(ref: EdgeRef): { body: number; faces: [number, number] } | null {
    for (let b = 0; b < this.summary.bodies.length; b++) {
      const body = this.summary.bodies[b]!;
      for (let s = 0; s < body.faces.length; s++) {
        for (let t = s + 1; t < body.faces.length; t++) {
          const o1 = body.faces[s]!.origin, o2 = body.faces[t]!.origin;
          const same = (x: FaceRef, y: FaceRef) => x.feature === y.feature && x.local === y.local;
          if ((same(o1, ref.a) && same(o2, ref.b)) || (same(o1, ref.b) && same(o2, ref.a))) return { body: b, faces: [s, t] };
        }
      }
    }
    return null;
  }

  describeEdge(ref: EdgeRef): string {
    const name = (f: FaceRef) => `${this.feature(f.feature)?.name ?? `feature ${f.feature}`}·${f.local}`;
    return `${name(ref.a)} / ${name(ref.b)}`;
  }

  sameEdge(x: EdgeRef, y: EdgeRef): boolean {
    const same = (a: FaceRef, b: FaceRef) => a.feature === b.feature && a.local === b.local;
    return (same(x.a, y.a) && same(x.b, y.b)) || (same(x.a, y.b) && same(x.b, y.a));
  }

  onEdgePick(pick: EdgePick | null): void {
    if (this.edgePicking === null) return;
    const f = this.feature(this.edgePicking);
    if (!f || f.kind.type !== "blend") return;
    const ref = pick ? this.edgeRefOf(pick) : null;
    if (!ref) return;
    const edges = f.kind.edges.some((e) => this.sameEdge(e, ref)) ? f.kind.edges.filter((e) => !this.sameEdge(e, ref)) : [...f.kind.edges, ref];
    this.apply({ type: "set_blend", id: f.id, edges });
  }

  beginEdgePick(featureId: number): void {
    this.endFacePick();
    this.edgePicking = featureId;
    this.viewer.edgePickMode = true;
    this.regenerate();
    this.setStatus("Click edges to add or remove them (Esc when done). The view shows the part before this feature.");
  }

  endEdgePick(): void {
    if (this.edgePicking === null) return;
    this.edgePicking = null;
    this.viewer.edgePickMode = false;
    this.regenerate();
  }

  /** Rollback count for regeneration: edge picking wins, then the rollback bar. */
  private rollback(): number | null {
    if (this.edgePicking !== null) {
      const i = this.summary?.features.findIndex((f) => f.id === this.edgePicking) ?? -1;
      return i < 0 ? null : i;
    }
    if (this.sketcher.active && this.sketcher.tool === "use") {
      // Only bodies that exist before the sketch can be projected into it.
      const i = this.summary?.features.findIndex((f) => f.id === this.sketcher.sketchId) ?? -1;
      return i < 0 ? null : i + 1;
    }
    return this.rollbackCount;
  }

  /** "Use" tool: projects the picked body edge (or face) into the sketch being edited. */
  projectAt(e: PointerEvent): void {
    const id = this.sketcher.sketchId;
    if (id === null) return;
    const pick = this.viewer.pickEdgeOrFace(e);
    if (!pick) return;
    const source: ProjectionSource | null =
      "edge" in pick
        ? (() => {
            const edge = this.edgeRefOf(pick.edge);
            return edge ? { type: "edge", edge } : null;
          })()
        : (() => {
            const face = this.faceRefOf(pick.face);
            return face ? { type: "face", face } : null;
          })();
    if (!source) return;
    this.snapshot();
    try {
      this.applyRaw({ type: "sketch", id, op: { type: "project", source } });
    } catch (err) {
      this.setStatus(`error: ${(err as Error).message}`);
      return;
    }
    this.regenerate();
    this.viewer.hoverEdgeOrFace(null);
  }

  describeProjection(p: ProjectionSource): string {
    return p.type === "face" ? `Face: ${this.describeFace(p.face)}` : `Edge: ${this.describeFace(p.edge.a)} / ${this.describeFace(p.edge.b)}`;
  }

  setRollback(count: number | null): void {
    this.rollbackCount = count;
    this.regenerate();
  }

  // ------------------------------------------------------------ faces

  faceRefOf(pick: FacePick): FaceRef | null {
    return this.summary.bodies[pick.body]?.faces[pick.face]?.origin ?? null;
  }

  /** Locates a face reference among the current bodies. */
  findFace(ref: FaceRef): FacePick | null {
    for (let b = 0; b < this.summary.bodies.length; b++) {
      const faces = this.summary.bodies[b]!.faces;
      for (let f = 0; f < faces.length; f++) {
        const o = faces[f]!.origin;
        if (o.feature === ref.feature && o.local === ref.local) return { body: b, face: f };
      }
    }
    return null;
  }

  describeFace(ref: FaceRef): string {
    const feature = this.feature(ref.feature);
    const pick = this.findFace(ref);
    const surface = pick ? this.summary.bodies[pick.body]!.faces[pick.face]!.surface : "missing";
    return `${feature?.name ?? `feature ${ref.feature}`} · face ${ref.local} (${surface})`;
  }

  /** The instance shown as body `bodyIndex` in an assembly tab. */
  instanceAt(bodyIndex: number): InstanceSummary | undefined {
    return this.summary.instances.find((i) => i.body_indices.includes(bodyIndex));
  }

  instance(id: number | null): InstanceSummary | undefined {
    return this.summary.instances.find((i) => i.id === id);
  }

  mate(id: number | null): MateSummary | undefined {
    return this.summary.mates.find((m) => m.id === id);
  }

  onViewportPick(pick: FacePick | null): void {
    const ref = pick ? this.faceRefOf(pick) : null;
    if (this.summary.kind === "assembly") {
      const inst = pick ? this.instanceAt(pick.body) : undefined;
      if (this.matePick) {
        if (!inst || !ref) return;
        const connector: Connector = { instance: inst.id, face: ref };
        if (!this.matePick.a) {
          this.matePick.a = connector;
          this.viewer.setSelectedFace(pick);
          const f = this.kernel.connectorFrame(inst.id, ref);
          this.viewer.setFrames(f ? [f] : []);
          this.setStatus(`First connector: ${inst.name}. Now click a face on the other instance (Esc cancels).`);
          return;
        }
        if (this.matePick.a.instance === inst.id) {
          this.setStatus("Pick a face on a different instance (Esc cancels).");
          return;
        }
        const a = this.matePick.a;
        this.endMatePick();
        this.applyDoc({ type: "assembly", tab: this.tab!, op: { type: "add_mate", kind: "fastened", a, b: connector, name: null } });
        this.selectedMate = this.summary.mates[this.summary.mates.length - 1]?.id ?? null;
        this.selectedInstance = null;
        this.renderAssembly();
        this.renderDetail();
        return;
      }
      this.selectedInstance = inst?.id ?? null;
      this.selectedMate = null;
      this.viewer.setSelectedFace(pick);
      this.renderAssembly();
      this.renderDetail();
      this.setStatus(inst && ref ? `${inst.name} · face ${ref.local} of feature ${ref.feature}` : this.statusLine());
      return;
    }
    if (this.facePicker) {
      if (ref) {
        const cb = this.facePicker;
        this.endFacePick();
        cb(ref);
      }
      return;
    }
    this.selectedFace = ref;
    this.viewer.setSelectedFace(pick);
    this.setStatus(ref ? `Face: ${this.describeFace(ref)}` : this.statusLine());
  }

  beginFacePick(cb: (face: FaceRef) => void): void {
    this.facePicker = cb;
    this.viewer.pickMode = true;
    this.setStatus("Click a planar face in the viewport (Esc to cancel)");
  }

  endFacePick(): void {
    this.facePicker = null;
    this.viewer.pickMode = false;
    this.setStatus(this.statusLine());
  }

  beginMatePick(): void {
    if (this.summary.instances.length < 2) {
      this.setStatus("Insert at least two instances before mating them.");
      return;
    }
    this.matePick = { a: null };
    this.viewer.pickMode = true;
    this.setStatus("Mate: click a face on the first instance (Esc cancels)");
  }

  endMatePick(): void {
    this.matePick = null;
    this.viewer.pickMode = false;
    this.viewer.setSelectedFace(null);
    this.viewer.setFrames([]);
  }

  // ------------------------------------------------------------ measure

  /** Measure mode: the first picked point, or null while waiting for it. */
  measure: { a: { point: Vec3; body: number; face: number } | null } | null = null;

  beginMeasure(): void {
    if (this.sketcher.active) this.sketcher.exit();
    this.endFacePick();
    this.endEdgePick();
    this.endMatePick();
    this.measure = { a: null };
    ($("#btn-measure") as HTMLButtonElement).classList.add("active");
    let down: { x: number; y: number } | null = null;
    this.viewer.pointerHandler = {
      down: (e) => {
        down = { x: e.clientX, y: e.clientY };
      },
      move: () => {},
      up: (e) => {
        const d = down;
        down = null;
        if (!d || Math.hypot(e.clientX - d.x, e.clientY - d.y) > 4) return;
        this.measureClick(e);
      },
      cancel: () => this.endMeasure(),
    };
    this.setStatus("Measure: click a point on a body (corners snap), then a second point. Esc stops.");
  }

  private measureClick(e: PointerEvent): void {
    if (!this.measure) return;
    const hit = this.viewer.pickPoint(e);
    if (!hit) return;
    const fmt = (p: Vec3): string => `(${p.x.toFixed(3)}, ${p.y.toFixed(3)}, ${p.z.toFixed(3)})`;
    if (!this.measure.a) {
      this.measure.a = hit;
      this.viewer.setMeasure({ a: hit.point, b: null, text: fmt(hit.point) });
      const body = this.summary.bodies[hit.body];
      const face = body?.faces[hit.face];
      const what = face ? `${face.surface} face of ${body!.name}` : "point";
      this.setStatus(`Measure: ${what} at ${fmt(hit.point)}${hit.snapped ? " (corner)" : ""}. Click the second point.`);
      return;
    }
    const a = this.measure.a.point;
    const b = hit.point;
    const dx = b.x - a.x;
    const dy = b.y - a.y;
    const dz = b.z - a.z;
    const dist = Math.hypot(dx, dy, dz);
    this.viewer.setMeasure({ a, b, text: `${dist.toFixed(3)} mm` });
    this.setStatus(`Distance ${dist.toFixed(3)} mm · dx ${dx.toFixed(3)} · dy ${dy.toFixed(3)} · dz ${dz.toFixed(3)} · from ${fmt(a)} to ${fmt(b)}. Click to measure again, Esc stops.`);
    this.measure.a = null;
  }

  endMeasure(): void {
    if (!this.measure) return;
    this.measure = null;
    this.viewer.pointerHandler = null;
    this.viewer.setMeasure(null);
    ($("#btn-measure") as HTMLButtonElement).classList.remove("active");
    this.setStatus(this.statusLine());
  }

  // ------------------------------------------------------------ section view

  /** Section view state: clipping axis, position as a fraction of the model extent, and side. */
  section: { axis: "x" | "y" | "z"; t: number; flip: boolean } | null = null;

  setSection(section: { axis: "x" | "y" | "z"; t: number; flip: boolean } | null): void {
    this.section = section;
    ($("#section-offset") as HTMLInputElement).hidden = !section;
    ($("#section-flip") as HTMLButtonElement).hidden = !section;
    this.applySection();
  }

  /** Re-applies the section plane against the current body extents. */
  applySection(): void {
    const sec = this.section;
    if (!sec) {
      this.viewer.setSection(null);
      return;
    }
    const bounds = this.viewer.bodyBounds();
    const lo = bounds ? bounds.min[sec.axis] : -50;
    const hi = bounds ? bounds.max[sec.axis] : 50;
    const offset = lo + (hi - lo) * sec.t;
    this.viewer.setSection({ axis: sec.axis, offset, flip: sec.flip });
  }

  // ------------------------------------------------------------ tabs

  switchTab(id: number): void {
    if (id === this.tab) return;
    this.sketcher.exit();
    this.endFacePick();
    this.endEdgePick();
    this.endMatePick();
    this.endMeasure();
    this.viewer.showAllBodies();
    this.selected = null;
    this.selectedFace = null;
    this.selectedInstance = null;
    this.selectedMate = null;
    this.rollbackCount = null;
    this.tab = id;
    this.regenerate();
    this.viewer.fitAll();
  }

  renderTabs(): void {
    const bar = $("#tabbar");
    bar.innerHTML = "";
    for (const t of this.summary.tabs) {
      const b = button(`${t.kind === "assembly" ? "⚙ " : "◧ "}${t.name}`, () => this.switchTab(t.id), t.id === this.summary.tab ? "active" : "");
      b.title = `${t.kind === "assembly" ? "Assembly" : "Part studio"} · double-click to rename`;
      b.ondblclick = () => {
        const name = prompt("Tab name", t.name);
        if (name && name !== t.name) this.applyDoc({ type: "rename_tab", tab: t.id, name });
      };
      bar.appendChild(b);
    }
    const addStudio = button("+ Part Studio", () => {
      this.applyDoc({ type: "add_part_studio", name: null });
      this.switchTab(this.summary.tabs[this.summary.tabs.length - 1]!.id);
    }, "add");
    const addAsm = button("+ Assembly", () => {
      this.applyDoc({ type: "add_assembly", name: null });
      this.switchTab(this.summary.tabs[this.summary.tabs.length - 1]!.id);
    }, "add");
    bar.append(addStudio, addAsm);
    if (this.summary.tabs.length > 1) {
      const del = button("×", () => {
        if (confirm(`Delete tab "${this.summary.tab_name}"?`)) {
          const gone = this.summary.tab;
          this.applyDoc({ type: "delete_tab", tab: gone });
          this.tab = this.summary.tabs.find((t) => t.id !== gone)?.id ?? null;
          this.regenerate();
        }
      }, "add danger");
      del.title = "Delete this tab";
      bar.appendChild(del);
    }
  }

  // ------------------------------------------------------------ assembly panel

  renderAssembly(): void {
    const ul = $("#instance-list");
    ul.innerHTML = "";
    for (const i of this.summary.instances) {
      const li = document.createElement("li");
      li.className = i.id === this.selectedInstance ? "selected" : "";
      const dot = document.createElement("span");
      dot.className = "dot " + (i.error ? "err" : "");
      dot.title = i.error ?? (i.fixed ? "fixed" : "placed by mates");
      const icon = document.createElement("span");
      icon.className = "icon";
      icon.textContent = i.fixed ? "⚓" : "◧";
      const name = document.createElement("span");
      name.className = "name";
      name.textContent = i.name;
      li.append(dot, icon, name);
      li.onclick = () => {
        this.selectedInstance = i.id;
        this.selectedMate = null;
        this.viewer.setSelectedFace(null);
        this.renderAssembly();
        this.renderDetail();
      };
      ul.appendChild(li);
    }
    if (this.summary.instances.length === 0) {
      const li = document.createElement("li");
      li.className = "note";
      li.textContent = "No instances yet. “+ Insert” places a body from a part studio.";
      ul.appendChild(li);
    }
    const ml = $("#mate-list");
    ml.innerHTML = "";
    for (const m of this.summary.mates) {
      const li = document.createElement("li");
      li.className = m.id === this.selectedMate ? "selected" : "";
      const dot = document.createElement("span");
      dot.className = "dot " + (m.error ? "err" : "");
      dot.title = m.error ?? "ok";
      const icon = document.createElement("span");
      icon.className = "icon";
      icon.textContent = { fastened: "⊠", revolute: "↻", slider: "↔", cylindrical: "⟳", planar: "▱", ball: "●" }[m.kind];
      const name = document.createElement("span");
      name.className = "name";
      name.textContent = `${m.name} · ${this.instance(m.a.instance)?.name ?? "?"} ↔ ${this.instance(m.b.instance)?.name ?? "?"}`;
      li.append(dot, icon, name);
      li.onclick = () => {
        this.selectedMate = m.id;
        this.selectedInstance = null;
        this.renderAssembly();
        this.renderDetail();
      };
      ml.appendChild(li);
    }
    if (this.summary.mates.length === 0) {
      const li = document.createElement("li");
      li.className = "note";
      li.textContent = "No mates. “+ Mate” joins two instances face to face.";
      ml.appendChild(li);
    }
    this.renderParts("#asm-part-list");
    ($("#btn-add-mate") as HTMLButtonElement).disabled = this.summary.instances.length < 2;
    ($("#btn-interference") as HTMLButtonElement).disabled = this.summary.instances.length < 2;
    this.viewer.setSelectedBodies(new Set(this.instance(this.selectedInstance)?.body_indices ?? []));
    const m = this.mate(this.selectedMate);
    this.viewer.setFrames(m ? [m.frame_a, m.frame_b].filter((f): f is NonNullable<typeof f> => f !== null) : []);
  }

  /** Runs the interference check and shows the overlapping pairs in the detail panel. */
  checkInterference(): void {
    const r = this.kernel.interferences();
    const title = $("#detail-title");
    const body = $("#detail-body");
    this.selectedInstance = null;
    this.selectedMate = null;
    this.renderAssembly();
    body.innerHTML = "";
    title.textContent = "Interference";
    const name = (id: number) => this.instance(id)?.name ?? `instance ${id}`;
    if (r.overlaps.length === 0 && r.failed.length === 0) {
      body.innerHTML = `<p class="note">No overlapping instances.</p>`;
      return;
    }
    const ul = document.createElement("ul");
    ul.className = "constraint-list";
    for (const o of r.overlaps) {
      const li = document.createElement("li");
      li.innerHTML = `<span class="kind">${name(o.a)} ∩ ${name(o.b)}<br><span class="refs">${o.volume.toFixed(2)} mm³ overlap</span></span>`;
      ul.appendChild(li);
    }
    for (const [a, b] of r.failed) {
      const li = document.createElement("li");
      li.innerHTML = `<span class="kind">${name(a)} ∩ ${name(b)}<br><span class="refs">could not be checked</span></span>`;
      ul.appendChild(li);
    }
    body.appendChild(ul);
    this.viewer.setSelectedBodies(new Set(r.overlaps.flatMap((o) => [...(this.instance(o.a)?.body_indices ?? []), ...(this.instance(o.b)?.body_indices ?? [])])));
  }

  renderInsertForm(body: HTMLElement): void {
    const studios = this.summary.tabs.filter((t) => t.id !== this.tab);
    if (studios.length === 0) {
      body.innerHTML = `<p class="note">Add a part studio tab first.</p>`;
      return;
    }
    let studio = studios[0]!.id;
    let bodyIndex = 0;
    const bodySel = document.createElement("select");
    const fillBodies = () => {
      bodySel.innerHTML = "";
      const source = studios.find((t) => t.id === studio)!;
      const names = source.kind === "assembly" ? ["(whole assembly, as one rigid group)"] : this.kernel.studioBodies(studio);
      names.forEach((n, i) => {
        const o = document.createElement("option");
        o.value = String(i);
        o.textContent = n;
        bodySel.appendChild(o);
      });
      if (names.length === 0) {
        const o = document.createElement("option");
        o.textContent = "(no bodies)";
        o.value = "-1";
        bodySel.appendChild(o);
      }
      bodyIndex = names.length > 0 ? 0 : -1;
    };
    bodySel.onchange = () => (bodyIndex = Number(bodySel.value));
    body.appendChild(
      field("Source tab", select(studios.map((t) => t.name), studios[0]!.name, (v) => {
        studio = studios.find((t) => t.name === v)!.id;
        fillBodies();
      })),
    );
    fillBodies();
    body.appendChild(field("Body", bodySel));
    body.appendChild(
      field("", button("Insert", () => {
        if (bodyIndex < 0) return;
        this.applyDoc({ type: "assembly", tab: this.tab!, op: { type: "add_instance", studio, body: bodyIndex, name: null, fixed: this.summary.instances.length === 0 } });
        this.selectedInstance = this.summary.instances[this.summary.instances.length - 1]?.id ?? null;
        this.renderAssembly();
        this.renderDetail();
      }, "primary")),
    );
  }

  renderAssemblyDetail(): void {
    const title = $("#detail-title");
    const body = $("#detail-body");
    body.innerHTML = "";
    const tab = this.tab!;
    const inst = this.instance(this.selectedInstance);
    const m = this.mate(this.selectedMate);
    if (inst) {
      title.textContent = inst.name;
      if (inst.error) {
        const e = document.createElement("div");
        e.className = "error";
        e.textContent = inst.error;
        body.appendChild(e);
      }
      const src = this.summary.tabs.find((t) => t.id === inst.studio);
      const note = document.createElement("p");
      note.className = "note";
      note.textContent = `${src?.kind === "assembly" ? "Sub-assembly" : `Body ${inst.body + 1}`} of ${src?.name ?? `tab ${inst.studio}`}. ${inst.fixed ? "Fixed: anchors mate chains." : "Positioned by mates when mated, else by the placement below."}`;
      body.appendChild(note);
      body.appendChild(field("Name", textInput(inst.name, (v) => this.applyDoc({ type: "assembly", tab, op: { type: "set_instance", id: inst.id, name: v } }))));
      body.appendChild(field("Fixed", checkbox(inst.fixed, (v) => this.applyDoc({ type: "assembly", tab, op: { type: "set_instance", id: inst.id, fixed: v } }))));
      const p = inst.placement;
      const setPlacement = (k: "position" | "rotation", axis: "x" | "y" | "z", v: number) => {
        const next = { position: { ...p.position }, rotation: { ...p.rotation } };
        next[k][axis] = v;
        this.applyDoc({ type: "assembly", tab, op: { type: "set_instance", id: inst.id, placement: next } });
      };
      for (const axis of ["x", "y", "z"] as const) {
        body.appendChild(field(`Position ${axis}`, numberInput(p.position[axis], (v) => setPlacement("position", axis, v))));
      }
      for (const axis of ["x", "y", "z"] as const) {
        body.appendChild(field(`Rotation ${axis}°`, numberInput(p.rotation[axis], (v) => setPlacement("rotation", axis, v))));
      }
      body.appendChild(field("", button("Remove instance", () => {
        this.applyDoc({ type: "assembly", tab, op: { type: "remove_instance", id: inst.id } });
        this.selectedInstance = null;
        this.renderAssembly();
        this.renderDetail();
      }, "danger")));
      return;
    }
    if (m) {
      title.textContent = m.name;
      if (m.error) {
        const e = document.createElement("div");
        e.className = "error";
        e.textContent = m.error;
        body.appendChild(e);
      }
      body.appendChild(field("Name", textInput(m.name, (v) => this.applyDoc({ type: "assembly", tab, op: { type: "set_mate", id: m.id, name: v } }))));
      body.appendChild(field("Kind", select(["fastened", "revolute", "slider", "cylindrical", "planar", "ball"], m.kind, (v) => this.applyDoc({ type: "assembly", tab, op: { type: "set_mate", id: m.id, kind: v as MateKind } }))));
      const describe = (c: Connector) => `${this.instance(c.instance)?.name ?? "?"} · face ${c.face.local} of feature ${c.face.feature}`;
      const note = document.createElement("p");
      note.className = "note";
      note.textContent = `A: ${describe(m.a)}\nB: ${describe(m.b)}. B moves onto A (or A onto B when only B is placed); faces meet with normals opposed unless flipped.`;
      body.appendChild(note);
      body.appendChild(field("Offset", numberInput(m.offset, (v) => this.applyDoc({ type: "assembly", tab, op: { type: "set_mate", id: m.id, offset: v } }))));
      body.appendChild(field("Angle°", numberInput(m.angle, (v) => this.applyDoc({ type: "assembly", tab, op: { type: "set_mate", id: m.id, angle: v } }))));
      body.appendChild(field("Flip", checkbox(m.flip, (v) => this.applyDoc({ type: "assembly", tab, op: { type: "set_mate", id: m.id, flip: v } }))));
      body.appendChild(field("", button("Remove mate", () => {
        this.applyDoc({ type: "assembly", tab, op: { type: "remove_mate", id: m.id } });
        this.selectedMate = null;
        this.renderAssembly();
        this.renderDetail();
      }, "danger")));
      return;
    }
    title.textContent = "Insert instance";
    const intro = document.createElement("p");
    intro.className = "note";
    intro.textContent = "Pick a part studio and one of its bodies. The first instance is fixed; mate the rest to it with “+ Mate”: click a face on each of two instances and they meet face to face.";
    body.appendChild(intro);
    this.renderInsertForm(body);
  }

  // ------------------------------------------------------------ document

  replace(kernel: Kernel): void {
    this.sketcher.exit();
    this.kernel.dispose();
    this.kernel = kernel;
    this.selected = null;
    this.selectedFace = null;
    this.history = [];
    this.future = [];
    this.updateHistoryButtons();
    this.regenerate();
    this.viewer.fitAll();
  }

  /** Applies a part-studio op to the active tab as one undo step. */
  apply(op: Op): void {
    if (this.summary?.kind !== "part_studio") {
      this.setStatus("switch to a part studio tab for that");
      return;
    }
    this.applyDoc({ type: "studio", tab: this.tab!, op });
  }

  applyDoc(op: DocOp): void {
    this.snapshot();
    try {
      this.applyDocRaw(op);
    } catch (e) {
      this.setStatus(`error: ${(e as Error).message}`);
      if (this.history[this.history.length - 1]?.length === 0) this.history.pop();
      this.updateHistoryButtons();
      return;
    }
    this.regenerate();
  }

  /** An op from another client: apply without touching undo history. */
  applyRemote(op: DocOp, base: number | null): void {
    try {
      this.kernel.apply(op, base);
    } catch (e) {
      this.setStatus(`remote edit failed locally: ${(e as Error).message}`);
      return;
    }
    this.regenerate();
  }

  /** Replaces the document from the server (welcome or resync). */
  loadDocument(json: string): void {
    const fresh = Kernel.fromJson(json);
    this.sketcher.exit();
    this.kernel.dispose();
    this.kernel = fresh;
    this.history = [];
    this.future = [];
    this.updateHistoryButtons();
    if (!this.summary || !this.feature(this.selected)) this.selected = null;
    this.regenerate();
  }

  showPresence(clients: number, names: string[]): void {
    const el = $("#presence");
    el.hidden = !this.sync.connected;
    el.textContent = `● ${clients} online`;
    el.title = names.join(", ");
  }

  // ------------------------------------------------------------ undo / redo

  // Undo is op-based: every applied op comes back with the ops that undo
  // it, and undoing applies those as ordinary (synced) ops. In a shared
  // document this reverts only our own edit and leaves everyone else's in
  // place. A history entry is one user-level edit: the inverses of every op
  // applied since the last `snapshot()`, in application order.
  private history: DocOp[][] = [];
  private future: DocOp[][] = [];
  private static readonly HISTORY_LIMIT = 200;

  /** Starts a new undo step; the ops applied next are undone together. */
  snapshot(): void {
    if (this.history.length > 0 && this.history[this.history.length - 1]!.length === 0) return;
    this.history.push([]);
    if (this.history.length > App.HISTORY_LIMIT) this.history.shift();
    this.future = [];
    this.updateHistoryButtons();
  }

  private recordInverse(inverse: DocOp[] | undefined): void {
    if (!inverse || inverse.length === 0) return;
    if (this.history.length === 0) this.history.push([]);
    this.history[this.history.length - 1]!.push(...inverse);
  }

  undo(): void {
    while (this.history.length > 0 && this.history[this.history.length - 1]!.length === 0) this.history.pop();
    const step = this.history.pop();
    if (step === undefined) {
      this.updateHistoryButtons();
      return;
    }
    const redo = this.applyStep(step);
    if (redo.length > 0) this.future.push(redo);
    this.afterHistoryStep();
  }

  redo(): void {
    const step = this.future.pop();
    if (step === undefined) return;
    const undo = this.applyStep(step);
    if (undo.length > 0) this.history.push(undo);
    this.afterHistoryStep();
  }

  /** Applies a step's ops last-first and returns the ops that reverse it. */
  private applyStep(step: DocOp[]): DocOp[] {
    const reverse: DocOp[] = [];
    for (let i = step.length - 1; i >= 0; i--) {
      const op = step[i]!;
      const base = this.sync.nextBase();
      try {
        const r = this.kernel.apply(op, base);
        reverse.push(...(r.inverse ?? []));
      } catch (e) {
        // Someone else changed what this op touches; skip just that op.
        this.setStatus(`undo skipped an edit: ${(e as Error).message}`);
        continue;
      }
      this.sync.send(op, base);
    }
    return reverse;
  }

  private afterHistoryStep(): void {
    const sketchId = this.sketcher.active ? this.sketcher.sketchId : null;
    this.sketcher.cancel();
    this.sketcher.selection.clear();
    this.regenerate();
    if (!this.feature(this.selected)) this.selected = null;
    if (sketchId !== null && !this.feature(sketchId)) this.sketcher.exit();
    this.renderFeatures();
    this.renderDetail();
    this.updateHistoryButtons();
  }

  private updateHistoryButtons(): void {
    ($("#btn-undo") as HTMLButtonElement).disabled = !this.history.some((s) => s.length > 0);
    ($("#btn-redo") as HTMLButtonElement).disabled = this.future.length === 0;
  }

  // ------------------------------------------------------------ export

  /** All bodies as one binary STL file. */
  toStl(): Blob {
    return toStl(this.kernel.bodyMeshes());
  }

  /** All bodies as a 3MF package with part names. */
  to3mf(): Blob {
    return to3mf(this.summary.bodies, this.kernel.bodyMeshes(), this.summary.name);
  }

  /** The selected sketch as DXF text, or null when no sketch is selected. */
  toDxf(): string | null {
    const f = this.feature(this.selected);
    if (!f || f.kind.type !== "sketch") return null;
    return toDxf(f.kind.sketch);
  }

  /** Triggers a browser download of `blob` named after the document. */
  download(blob: Blob, ext: string, stem = this.summary.name): void {
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = `${stem.replace(/[^\w.-]+/g, "_")}.${ext}`;
    a.click();
    URL.revokeObjectURL(a.href);
  }

  /** Applies a studio op on the active tab without regenerating; throws on failure. */
  applyRaw(op: Op): OpResult {
    const r = this.applyDocRaw({ type: "studio", tab: this.tab!, op });
    return r.studio ?? { feature: null, entities: [], constraint: null };
  }

  applyDocRaw(op: DocOp): DocOpResult {
    const base = this.sync.nextBase();
    const r = this.kernel.apply(op, base);
    this.sync.send(op, base);
    this.recordInverse(r.inverse);
    return r;
  }

  /** "Sketch 3"-style default name, decided here so replicas agree on it. */
  autoName(kind: string): string {
    const n = this.summary.features.filter((f) => f.name.startsWith(kind + " ")).length + 1;
    return `${kind} ${n}`;
  }

  selectionChanged(): void {
    this.viewer.setSketches(this.summary.sketches, this.selected, this.sketcher.selection);
    this.renderDetail();
  }

  regenerate(): void {
    const t0 = performance.now();
    this.summary = this.kernel.regenerate(this.tab, this.rollback());
    this.tab = this.summary.tab;
    const dt = performance.now() - t0;
    const assembly = this.summary.kind === "assembly";
    ($("#assembly-panel") as HTMLElement).hidden = !assembly;
    ($("#studio-panel") as HTMLElement).hidden = assembly;
    this.viewer.setBodies(this.kernel.bodyMeshes());
    this.applySection();
    this.viewer.setSketches(this.summary.sketches, this.selected, this.sketcher.selection);
    if (!assembly) this.viewer.setSelectedFace(this.selectedFace ? this.findFace(this.selectedFace) : null);
    this.highlightBlendEdges();
    this.updateDimensionLabels();
    this.renderTabs();
    if (assembly) this.renderAssembly();
    else {
      this.viewer.setFrames([]);
      this.renderFeatures();
    }
    this.renderDetail();
    this.lastRegenMs = dt;
    this.setStatus(this.statusLine());
    ($("#studio-name") as HTMLInputElement).value = this.summary.name;
    const quality = $("#quality") as HTMLSelectElement;
    const current = String(this.summary.settings.facet_angle);
    if ([...quality.options].some((o) => o.value === current)) quality.value = current;
    if (!this.sync.connected) {
      try {
        localStorage.setItem(STORAGE_KEY, this.kernel.toJson());
      } catch {
        /* storage unavailable */
      }
    }
  }

  lastRegenMs = 0;

  statusLine(): string {
    const faces = this.summary.bodies.reduce((n, b) => n + b.face_count, 0);
    const volume = this.summary.bodies.reduce((n, b) => n + b.volume, 0);
    const errors = this.summary.features.filter((f) => f.error).length + this.summary.instances.filter((i) => i.error).length + this.summary.mates.filter((m) => m.error).length;
    return (
      `${this.summary.bodies.length} ${this.summary.bodies.length === 1 ? "body" : "bodies"} · ${faces} faces · ${volume.toFixed(1)} mm³ · regen ${this.lastRegenMs.toFixed(1)} ms` +
      (errors ? ` · ${errors} feature error${errors > 1 ? "s" : ""}` : "") +
      ` · kernel v${Kernel.version()}`
    );
  }

  setStatus(text: string): void {
    $("#status-text").textContent = text;
  }

  /** Shows the selected sketch's dimensions as clickable labels in the viewport. */
  updateDimensionLabels(): void {
    const f = this.feature(this.selected);
    const result = f ? this.summary.sketches[String(f.id)] : undefined;
    if (!f || f.kind.type !== "sketch" || !result) {
      this.viewer.setLabels([]);
      return;
    }
    const sketch = f.kind.sketch;
    const plane = result.plane;
    const lift = (p: { x: number; y: number }) => ({
      x: plane.origin.x + plane.x_axis.x * p.x + plane.y_axis.x * p.y,
      y: plane.origin.y + plane.x_axis.y * p.x + plane.y_axis.y * p.y,
      z: plane.origin.z + plane.x_axis.z * p.x + plane.y_axis.z * p.y,
    });
    const pos = (id: number) => {
      const e = sketch.entities.find((e) => e.id === id);
      return e && e.type === "point" ? e.pos : null;
    };
    const lineEnds = (id: number) => {
      const e = sketch.entities.find((e) => e.id === id);
      if (!e || e.type !== "line") return null;
      const a = pos(e.start), b = pos(e.end);
      return a && b ? [a, b] : null;
    };
    const mid = (a: { x: number; y: number }, b: { x: number; y: number }) => ({ x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 });
    const labels = [];
    for (const c of sketch.constraints) {
      if (!("value" in c)) continue;
      let anchor: { x: number; y: number } | null = null;
      let prefix = "";
      switch (c.type) {
        case "length": {
          const e = lineEnds(c.line);
          if (e) anchor = mid(e[0]!, e[1]!);
          break;
        }
        case "distance": case "horizontal_distance": case "vertical_distance": {
          const a = pos(c.a), b = pos(c.b);
          if (a && b) anchor = mid(a, b);
          prefix = c.type === "horizontal_distance" ? "↔ " : c.type === "vertical_distance" ? "↕ " : "";
          break;
        }
        case "radius": case "diameter": {
          const e = sketch.entities.find((e) => e.id === c.entity);
          const center = e && (e.type === "circle" || e.type === "arc") ? pos(e.center) : null;
          const r = e?.type === "circle" ? e.radius : e?.type === "arc" ? (() => { const s0 = pos(e.start); return center && s0 ? Math.hypot(s0.x - center.x, s0.y - center.y) : 0; })() : 0;
          if (center) anchor = { x: center.x + r * Math.SQRT1_2, y: center.y + r * Math.SQRT1_2 };
          prefix = c.type === "radius" ? "R" : "⌀";
          break;
        }
        case "angle": {
          const a = lineEnds(c.a), b = lineEnds(c.b);
          if (a && b) anchor = mid(mid(a[0]!, a[1]!), mid(b[0]!, b[1]!));
          prefix = "∠";
          break;
        }
      }
      if (!anchor) continue;
      const field = `constraint.${c.id}`;
      const bound = f.bindings[field];
      const text = `${prefix}${Number(c.value.toFixed(3))}${c.type === "angle" ? "°" : ""}`;
      labels.push({
        position: lift(anchor),
        text,
        className: bound ? "bound" : "",
        title: bound ? `${bound} (click to edit)` : "Click to edit",
        onClick: () => {
          const input = prompt(`${c.type.replace(/_/g, " ")} value or expression`, bound ?? String(Number(c.value.toFixed(6))));
          if (input === null) return;
          const text = input.trim();
          const n = Number(text);
          if (text === "") return;
          if (Number.isFinite(n)) {
            if (bound) this.applyRaw({ type: "set_binding", id: f.id, field, expression: null });
            this.apply({ type: "sketch", id: f.id, op: { type: "set_constraint_value", id: c.id, value: n } });
          } else {
            this.apply({ type: "set_binding", id: f.id, field, expression: text });
          }
        },
      });
    }
    this.viewer.setLabels(labels);
  }

  /** Highlights the edges of the selected blend feature when they are visible. */
  highlightBlendEdges(): void {
    const f = this.feature(this.selected);
    if (!f || f.kind.type !== "blend") {
      this.viewer.setSelectedEdges([]);
      return;
    }
    const picks = f.kind.edges.map((e) => this.findEdge(e)).filter((p): p is { body: number; faces: [number, number] } => p !== null);
    this.viewer.setSelectedEdges(picks);
  }

  select(id: number | null): void {
    if (this.sketcher.active && this.sketcher.sketchId !== id) this.sketcher.exit();
    if (this.edgePicking !== null && this.edgePicking !== id) this.endEdgePick();
    this.selected = id;
    this.highlightBlendEdges();
    this.updateDimensionLabels();
    this.viewer.setSketches(this.summary.sketches, this.selected, this.sketcher.selection);
    this.renderFeatures();
    this.renderDetail();
  }

  editSketch(id: number): void {
    this.selected = id;
    this.sketcher.enter(id);
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
      const icons: Record<string, string> = { sketch: "✎", extrude: "⬒", revolve: "◑", blend: "◜", mirror: "⇔", pattern: "⁝⁝", variable: "#", hole: "◎", sweep: "↝", loft: "⋀" };
      icon.textContent = icons[f.kind.type] ?? "•";
      const name = document.createElement("span");
      name.className = "name";
      name.textContent = f.name;
      const index = this.summary.features.indexOf(f);
      if (this.rollbackCount !== null && index >= this.rollbackCount) li.classList.add("rolled");
      const roll = document.createElement("button");
      roll.className = "roll";
      roll.textContent = "⏶";
      roll.title = "Roll back to before this feature";
      roll.onclick = (e) => {
        e.stopPropagation();
        this.setRollback(index);
      };
      li.append(dot, icon, name, roll);
      if (this.rollbackCount === index) {
        const bar = document.createElement("li");
        bar.className = "rollbar";
        bar.title = "Rollback bar: features below are not applied. Click to roll to the end.";
        bar.onclick = () => this.setRollback(null);
        ul.appendChild(bar);
      }
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
    this.renderParts();
    const sel = this.feature(this.selected);
    ($("#btn-add-extrude") as HTMLButtonElement).disabled = !(sel && sel.kind.type === "sketch");
    ($("#btn-add-revolve") as HTMLButtonElement).disabled = !(sel && sel.kind.type === "sketch");
    ($("#btn-add-hole") as HTMLButtonElement).disabled = !(sel && sel.kind.type === "sketch");
    const sketchCount = this.summary.features.filter((f) => f.kind.type === "sketch").length;
    ($("#btn-add-sweep") as HTMLButtonElement).disabled = !(sel && sel.kind.type === "sketch" && sketchCount >= 2);
    ($("#btn-add-loft") as HTMLButtonElement).disabled = !(sel && sel.kind.type === "sketch" && sketchCount >= 2);
    const hasBody = this.summary.bodies.length > 0;
    ($("#btn-add-fillet") as HTMLButtonElement).disabled = !hasBody;
    ($("#btn-add-chamfer") as HTMLButtonElement).disabled = !hasBody;
    ($("#btn-add-mirror") as HTMLButtonElement).disabled = !hasBody;
    ($("#btn-add-pattern") as HTMLButtonElement).disabled = !hasBody;
  }

  renderParts(target = "#part-list"): void {
    const ul = $(target);
    ul.innerHTML = "";
    const hidden = this.viewer.hiddenBodies();
    this.summary.bodies.forEach((b, index) => {
      const li = document.createElement("li");
      li.dataset.body = String(index);
      const eye = document.createElement("button");
      eye.className = "eye" + (hidden.has(index) ? " off" : "");
      eye.title = hidden.has(index) ? "Show this part" : "Hide this part";
      eye.textContent = hidden.has(index) ? "○" : "●";
      eye.onclick = () => {
        this.viewer.setBodyVisible(index, hidden.has(index));
        this.applySection();
        this.renderParts(target);
      };
      const name = document.createElement("span");
      name.className = "pname";
      name.textContent = b.name;
      if (this.summary.kind !== "assembly") {
        name.title = "Double-click to rename";
        name.ondblclick = () => this.editPartName(li, name, b.source, b.name);
      }
      const stats = document.createElement("span");
      stats.className = "pstats";
      const c = b.centroid;
      stats.textContent = `${b.volume.toFixed(1)} mm³ · ${b.area.toFixed(1)} mm²`;
      stats.title = c ? `centre of mass (${c.x.toFixed(2)}, ${c.y.toFixed(2)}, ${c.z.toFixed(2)}) · ${b.face_count} faces` : "";
      li.append(eye, name, stats);
      ul.appendChild(li);
    });
    if (this.summary.bodies.length === 0) {
      const li = document.createElement("li");
      li.textContent = "no parts";
      ul.appendChild(li);
    }
  }

  /** Replaces a part's name span with an input; Enter or blur commits, Escape cancels. */
  private editPartName(li: HTMLElement, name: HTMLElement, source: number, current: string): void {
    const input = document.createElement("input");
    input.className = "pname-edit";
    input.value = current;
    let done = false;
    const finish = (commit: boolean) => {
      if (done) return;
      done = true;
      const value = input.value.trim();
      if (commit && value !== current) this.apply({ type: "rename_part", source, name: value === "" ? null : value });
      else this.renderParts(li.parentElement?.id ? `#${li.parentElement.id}` : "#part-list");
    };
    input.onkeydown = (e) => {
      if (e.key === "Enter") finish(true);
      if (e.key === "Escape") finish(false);
    };
    input.onblur = () => finish(true);
    li.replaceChild(input, name);
    input.focus();
    input.select();
  }

  sketchWarn(f: FeatureSummary): boolean {
    if (f.kind.type !== "sketch") return false;
    const r = this.summary.sketches[String(f.id)];
    return !!r && r.solve.status === "under_constrained";
  }

  // ------------------------------------------------------------ detail panel

  renderDetail(): void {
    if (this.summary.kind === "assembly") {
      this.renderAssemblyDetail();
      return;
    }
    const f = this.feature(this.selected);
    const title = $("#detail-title");
    const body = $("#detail-body");
    body.innerHTML = "";
    if (!f) {
      title.textContent = "Nothing selected";
      body.innerHTML = `<p class="note">Select a feature to edit it. Click a face to select it, then “+ Sketch” sketches on that face. Drag to orbit, scroll to zoom, right-drag to pan, F to fit.</p>`;
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
    else if (f.kind.type === "extrude") this.renderExtrudeDetail(f, body);
    else if (f.kind.type === "revolve") this.renderRevolveDetail(f, body);
    else if (f.kind.type === "blend") this.renderBlendDetail(f, body);
    else if (f.kind.type === "mirror") this.renderMirrorDetail(f, body);
    else if (f.kind.type === "variable") this.renderVariableDetail(f, body);
    else if (f.kind.type === "hole") this.renderHoleDetail(f, body);
    else if (f.kind.type === "sweep") this.renderSweepDetail(f, body);
    else if (f.kind.type === "loft") this.renderLoftDetail(f, body);
    else this.renderPatternDetail(f, body);

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

    const editing = this.sketcher.active && this.sketcher.sketchId === f.id;
    const tools = document.createElement("div");
    tools.className = "row tools";
    if (!editing) {
      tools.appendChild(button("Edit sketch", () => this.editSketch(f.id), "primary"));
    } else {
      for (const [tool, label, key] of [["select", "Select", "S"], ["line", "Line", "L"], ["rectangle", "Rectangle", "R"], ["circle", "Circle", "C"], ["arc", "Arc", "A"], ["trim", "Trim", "T"], ["use", "Use", "U"]] as [Tool, string, string][]) {
        const b = button(label, () => this.sketcher.setTool(tool), this.sketcher.tool === tool ? "active" : "");
        b.title = `${label} (${key})`;
        tools.appendChild(b);
      }
      tools.appendChild(button("Done", () => { this.sketcher.exit(); this.renderDetail(); }, "primary"));
    }
    body.appendChild(tools);
    if (editing) {
      const sel = [...this.sketcher.selection];
      const p = document.createElement("p");
      p.className = "note";
      p.textContent = sel.length === 0 ? "Click entities to select them (shift for more). Drag points to move them. Delete removes the selection." : `Selected: ${sel.map((id) => `${this.sketcher.entity(id)?.type ?? "?"} ${id}`).join(", ")}`;
      body.appendChild(p);
      const quick = this.sketcher.quickConstraints();
      if (quick.length > 0) {
        const row = document.createElement("div");
        row.className = "row";
        for (const q of quick) row.appendChild(button(q.label, () => this.sketcher.applyQuick(q.build)));
        body.appendChild(row);
      }
      if (sel.length > 0) {
        const row = document.createElement("div");
        row.className = "row";
        if (sel.some((id) => this.sketcher.entity(id)?.type !== "point")) {
          const b = button("Construction", () => this.sketcher.toggleConstruction());
          b.title = "Toggle construction geometry (Q)";
          row.appendChild(b);
        }
        const curves = sel.filter((id) => this.sketcher.entity(id)?.type !== "point");
        if (curves.length > 0) {
          const o = button("Offset…", () => this.sketcher.offsetSelection());
          o.title = "Copy the selected chain at a distance (O); negative distances go to the other side";
          row.appendChild(o);
        }
        const m = button(this.sketcher.awaitingMirrorAxis ? "Click a line…" : "Mirror…", () => this.sketcher.beginMirror());
        m.title = "Mirror the selection across a line you click next (M)";
        row.appendChild(m);
        row.appendChild(button("Delete selected", () => this.sketcher.deleteSelection(), "danger"));
        body.appendChild(row);
      }
    }

    const plane = kind.plane;
    const planeValue = plane.type === "standard" ? plane.base : "face";
    body.appendChild(
      field("Plane", select(["top", "front", "right", "face"], planeValue, (v) => {
        if (v === "face") {
          this.beginFacePick((face) => this.apply({ type: "set_sketch_plane", id: f.id, plane: { type: "face", face, offset: plane.offset } }));
        } else {
          this.apply({ type: "set_sketch_plane", id: f.id, plane: { type: "standard", base: v as StandardPlane, offset: plane.offset } });
        }
      })),
    );
    if (plane.type === "face") {
      const row = document.createElement("div");
      row.className = "field";
      const l = document.createElement("label");
      l.textContent = "Face";
      const v = document.createElement("span");
      v.className = "note";
      v.textContent = this.describeFace(plane.face);
      row.append(l, v);
      body.appendChild(row);
      body.appendChild(
        field("", button("Pick another face", () =>
          this.beginFacePick((face) => this.apply({ type: "set_sketch_plane", id: f.id, plane: { type: "face", face, offset: plane.offset } })),
        )),
      );
    }
    body.appendChild(
      field("Offset", this.exprInput(f, "plane.offset", plane.offset, (v) => this.apply({ type: "set_sketch_plane", id: f.id, plane: { ...plane, offset: v } })),
    ));

    if (result) {
      const s = result.solve;
      const cls = s.status === "fully_constrained" ? "ok" : s.status === "under_constrained" ? "warn" : "err";
      const label = s.status === "fully_constrained" ? "fully constrained" : s.status === "under_constrained" ? `${s.dof} degrees of freedom` : "inconsistent";
      const p = document.createElement("p");
      p.className = "note";
      p.innerHTML = `Solver: <span class="badge ${cls}">${label}</span> · ${s.equations} equations, ${s.parameters} parameters · ${result.profiles.length} closed region${result.profiles.length === 1 ? "" : "s"}`;
      body.appendChild(p);
    }

    const projections = kind.projections ?? [];
    if (projections.length > 0) {
      body.appendChild(heading("Projected geometry"));
      const ul = document.createElement("ul");
      ul.className = "constraint-list";
      projections.forEach((p, index) => {
        const li = document.createElement("li");
        const k = document.createElement("span");
        k.className = "kind";
        k.innerHTML = `${p.source.type === "face" ? "face outline" : "edge"}<br><span class="refs">${this.describeProjection(p.source)} · ${p.entities.length} entities</span>`;
        li.appendChild(k);
        li.appendChild(document.createElement("span"));
        li.appendChild(button("×", () => this.apply({ type: "sketch", id: f.id, op: { type: "remove_projection", index } }), "danger"));
        ul.appendChild(li);
      });
      body.appendChild(ul);
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
        li.appendChild(this.exprInput(f, `constraint.${c.id}`, c.value, (v) => this.apply({ type: "sketch", id: f.id, op: { type: "set_constraint_value", id: c.id, value: v } })));
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

  /** A "Regions" field shared by solid features built from a sketch. */
  regionField(sketchId: number, current: ProfileSelection, onChange: (p: ProfileSelection) => void): HTMLElement {
    const regions = this.summary.sketches[String(sketchId)]?.profiles.length ?? 0;
    const choices = ["all", "largest", ...Array.from({ length: regions }, (_, i) => `region ${i}`)];
    const value = current.type === "indices" ? `region ${current.indices[0] ?? 0}` : current.type;
    return field("Regions", select(choices, value, (v) => {
      onChange(v === "all" ? { type: "all" } : v === "largest" ? { type: "largest" } : { type: "indices", indices: [Number(v.slice(7))] });
    }));
  }

  sketchField(sketchId: number): HTMLElement {
    const names = this.summary.features.filter((s) => s.kind.type === "sketch").map((s) => `${s.id}: ${s.name}`);
    const current = names.find((n) => n.startsWith(`${sketchId}:`)) ?? `${sketchId}: (missing)`;
    const s = select(names.includes(current) ? names : [current, ...names], current, () => {});
    s.disabled = true;
    return field("Sketch", s);
  }

  /**
   * A numeric field that also accepts an expression. A plain number sets the
   * value through `onNumber` and clears any binding; anything else becomes a
   * binding on `field`, evaluated at regeneration.
   */
  exprInput(f: FeatureSummary, field: string, value: number, onNumber: (v: number) => void): HTMLElement {
    const bound = f.bindings[field];
    const wrap = document.createElement("span");
    wrap.style.display = "flex";
    wrap.style.alignItems = "center";
    const i = document.createElement("input");
    i.type = "text";
    i.spellcheck = false;
    i.value = bound ?? String(Number(value.toFixed(6)));
    if (bound) i.classList.add("bound");
    if (bound && f.error?.startsWith(field + ":")) i.classList.add("bad");
    const commit = () => {
      const text = i.value.trim();
      if (text === "" ) return;
      const n = Number(text);
      if (Number.isFinite(n)) {
        if (bound) this.applyRaw({ type: "set_binding", id: f.id, field, expression: null });
        if (n !== value || bound) onNumber(n);
        else this.regenerate();
      } else if (text !== bound) {
        this.apply({ type: "set_binding", id: f.id, field, expression: text });
      }
    };
    i.onchange = commit;
    // Enter blurs, and the native change event then commits exactly once.
    i.onkeydown = (e) => {
      if (e.key === "Enter") i.blur();
    };
    i.title = "A number, or an expression such as #width / 2";
    wrap.appendChild(i);
    if (bound) {
      const v = document.createElement("span");
      v.className = "expr-value";
      v.textContent = `= ${Number(value.toFixed(4))}`;
      wrap.appendChild(v);
    }
    return wrap;
  }

  /** A select over the sketches before `before` (or all), labelled "id: name". */
  sketchChooser(current: number, before: number, exclude: number | null, onChange: (id: number) => void): HTMLSelectElement {
    const idx = this.summary.features.findIndex((f) => f.id === before);
    const names = this.summary.features
      .filter((s, i) => s.kind.type === "sketch" && (idx < 0 || i < idx) && s.id !== exclude)
      .map((s) => `${s.id}: ${s.name}`);
    const cur = names.find((n) => n.startsWith(`${current}:`)) ?? `${current}: (missing)`;
    return select(names.includes(cur) ? names : [cur, ...names], cur, (v) => onChange(Number(v.split(":")[0])));
  }

  renderSweepDetail(f: FeatureSummary, body: HTMLElement): void {
    if (f.kind.type !== "sweep") return;
    const k = f.kind;
    body.appendChild(this.sketchField(k.sketch));
    body.appendChild(field("Path sketch", this.sketchChooser(k.path, f.id, k.sketch, (id) => this.apply({ type: "set_sweep", id: f.id, path: id }))));
    body.appendChild(this.regionField(k.sketch, k.profiles, (profiles) => this.apply({ type: "set_sweep", id: f.id, profiles })));
    body.appendChild(field("Result", select(["new", "add", "remove", "intersect"], k.op, (v) => this.apply({ type: "set_sweep", id: f.id, op: v as typeof k.op }))));
    const note = document.createElement("p");
    note.className = "note";
    note.textContent = "The path is the open chain of lines and arcs in the path sketch. The profile's sketch origin is carried to the path's start, with the profile plane turned to face along the path.";
    body.appendChild(note);
  }

  renderLoftDetail(f: FeatureSummary, body: HTMLElement): void {
    if (f.kind.type !== "loft") return;
    const k = f.kind;
    body.appendChild(this.sketchField(k.sketch));
    body.appendChild(field("To sketch", this.sketchChooser(k.sketch_b, f.id, k.sketch, (id) => this.apply({ type: "set_loft", id: f.id, sketch_b: id }))));
    body.appendChild(field("Result", select(["new", "add", "remove", "intersect"], k.op, (v) => this.apply({ type: "set_loft", id: f.id, op: v as typeof k.op }))));
    const note = document.createElement("p");
    note.className = "note";
    note.textContent = "Lofts between the largest region of each sketch; both need the same number of holes.";
    body.appendChild(note);
  }

  renderHoleDetail(f: FeatureSummary, body: HTMLElement): void {
    if (f.kind.type !== "hole") return;
    const k = f.kind;
    body.appendChild(this.sketchField(k.sketch));
    body.appendChild(field("Diameter", this.exprInput(f, "diameter", k.diameter, (v) => this.apply({ type: "set_hole", id: f.id, diameter: v }))));
    body.appendChild(
      field("Depth", select(["through_all", "blind"], k.through_all ? "through_all" : "blind", (v) =>
        this.apply({ type: "set_hole", id: f.id, through_all: v === "through_all", depth: v === "blind" && k.depth <= 0 ? 5 : null }),
      )),
    );
    if (!k.through_all) {
      body.appendChild(field("Blind depth", this.exprInput(f, "depth", k.depth, (v) => this.apply({ type: "set_hole", id: f.id, depth: v }))));
    }
    body.appendChild(
      field("Direction", select(["reverse", "normal", "symmetric"], k.direction, (v) => this.apply({ type: "set_hole", id: f.id, direction: v as ExtrudeDirection }))),
    );
    body.appendChild(
      field("Counterbore", select(["none", "yes"], k.counterbore ? "yes" : "none", (v) =>
        this.apply({ type: "set_hole", id: f.id, counterbore: v === "yes" ? { diameter: k.diameter * 2, depth: 2 } : null }),
      )),
    );
    if (k.counterbore) {
      const cb = k.counterbore;
      body.appendChild(field("C'bore ⌀", this.exprInput(f, "cbore_diameter", cb.diameter, (v) => this.apply({ type: "set_hole", id: f.id, counterbore: { ...cb, diameter: v } }))));
      body.appendChild(field("C'bore depth", this.exprInput(f, "cbore_depth", cb.depth, (v) => this.apply({ type: "set_hole", id: f.id, counterbore: { ...cb, depth: v } }))));
    }
    const note = document.createElement("p");
    note.className = "note";
    note.textContent = "Drills at every standalone point of the sketch (points not used by lines, arcs or circles). “Reverse” drills into the face the sketch sits on.";
    body.appendChild(note);
  }

  renderVariableDetail(f: FeatureSummary, body: HTMLElement): void {
    if (f.kind.type !== "variable") return;
    const k = f.kind;
    const name = document.createElement("input");
    name.value = k.name;
    name.spellcheck = false;
    name.onchange = () => {
      const n = name.value.trim().replace(/^#/, "");
      if (n && n !== k.name) this.apply({ type: "set_variable", id: f.id, name: n });
    };
    body.appendChild(field("Name", name));
    const expr = document.createElement("input");
    expr.value = k.expression;
    expr.spellcheck = false;
    expr.classList.add("bound");
    expr.onchange = () => {
      if (expr.value !== k.expression) this.apply({ type: "set_variable", id: f.id, expression: expr.value });
    };
    expr.onkeydown = (e) => {
      if (e.key === "Enter") expr.blur();
    };
    body.appendChild(field("Expression", expr));
    const p = document.createElement("p");
    p.className = "note";
    p.textContent = f.value === null ? "Not evaluated." : `#${k.name} = ${Number(f.value.toFixed(6))}`;
    body.appendChild(p);
    const vars = Object.entries(this.summary.variables);
    if (vars.length > 0) {
      const list = document.createElement("p");
      list.className = "note";
      list.textContent = "Variables so far: " + vars.map(([n, v]) => `#${n} = ${Number(v.toFixed(4))}`).join(", ");
      body.appendChild(list);
    }
    const help = document.createElement("p");
    help.className = "note";
    help.textContent = "Expressions accept + - * / ^, parentheses, #names, pi, and sin cos tan asin acos atan sqrt abs floor ceil round min max (angles in degrees). Type an expression into any dimension field to bind it.";
    body.appendChild(help);
  }

  /** Plane chooser shared by sketches and mirrors. */
  planeFields(body: HTMLElement, plane: PlaneRef, onChange: (p: PlaneRef) => void): void {
    const planeValue = plane.type === "standard" ? plane.base : "face";
    body.appendChild(
      field("Plane", select(["top", "front", "right", "face"], planeValue, (v) => {
        if (v === "face") this.beginFacePick((face) => onChange({ type: "face", face, offset: plane.offset }));
        else onChange({ type: "standard", base: v as StandardPlane, offset: plane.offset });
      })),
    );
    if (plane.type === "face") {
      const row = document.createElement("div");
      row.className = "field";
      const l = document.createElement("label");
      l.textContent = "Face";
      const v = document.createElement("span");
      v.className = "note";
      v.textContent = this.describeFace(plane.face);
      row.append(l, v);
      body.appendChild(row);
      body.appendChild(field("", button("Pick another face", () => this.beginFacePick((face) => onChange({ type: "face", face, offset: plane.offset })))));
    }
    body.appendChild(field("Offset", numberInput(plane.offset, (v) => onChange({ ...plane, offset: v }))));
  }

  /** Same as `planeFields` but with an expression-capable offset. */
  planeFieldsFor(f: FeatureSummary, body: HTMLElement, plane: PlaneRef, onChange: (p: PlaneRef) => void): void {
    this.planeFields(body, plane, onChange);
    const last = body.lastElementChild as HTMLElement;
    last.replaceWith(field("Offset", this.exprInput(f, "plane.offset", plane.offset, (v) => onChange({ ...plane, offset: v }))));
  }

  renderMirrorDetail(f: FeatureSummary, body: HTMLElement): void {
    if (f.kind.type !== "mirror") return;
    const k = f.kind;
    this.planeFieldsFor(f, body, k.plane, (plane) => this.apply({ type: "set_mirror", id: f.id, plane }));
    body.appendChild(field("Copies", select(["add", "new"], k.op, (v) => this.apply({ type: "set_mirror", id: f.id, op: v as "add" | "new" }))));
    const note = document.createElement("p");
    note.className = "note";
    note.textContent = "Mirrors every body across the plane. “Add” unions each copy with its original; “New” keeps copies as separate bodies.";
    body.appendChild(note);
  }

  renderPatternDetail(f: FeatureSummary, body: HTMLElement): void {
    if (f.kind.type !== "pattern") return;
    const k = f.kind;
    const setKind = (kind: PatternKind) => this.apply({ type: "set_pattern", id: f.id, kind });
    body.appendChild(
      field("Type", select(["linear", "circular"], k.kind.type, (v) =>
        setKind(v === "linear" ? { type: "linear", axis: k.kind.axis, spacing: 10 } : { type: "circular", axis: k.kind.axis, angle: 360 }),
      )),
    );
    body.appendChild(field("Axis", select(["x", "y", "z"], k.kind.axis, (v) => setKind({ ...k.kind, axis: v as Axis }))));
    if (k.kind.type === "linear") {
      const kind = k.kind;
      body.appendChild(field("Spacing", this.exprInput(f, "spacing", kind.spacing, (v) => setKind({ ...kind, spacing: v }))));
    } else {
      const kind = k.kind;
      body.appendChild(field("Total angle (°)", this.exprInput(f, "angle", kind.angle, (v) => setKind({ ...kind, angle: v }))));
    }
    body.appendChild(field("Count", this.exprInput(f, "count", k.count, (v) => this.apply({ type: "set_pattern", id: f.id, count: Math.max(2, Math.round(v)) }))));
    body.appendChild(field("Copies", select(["add", "new"], k.op, (v) => this.apply({ type: "set_pattern", id: f.id, op: v as "add" | "new" }))));
    const note = document.createElement("p");
    note.className = "note";
    note.textContent = "Repeats every body along or about a world axis through the origin; the count includes the original.";
    body.appendChild(note);
  }

  renderBlendDetail(f: FeatureSummary, body: HTMLElement): void {
    if (f.kind.type !== "blend") return;
    const k = f.kind;
    body.appendChild(field(k.kind === "fillet" ? "Radius" : "Distance", this.exprInput(f, "size", k.size, (v) => this.apply({ type: "set_blend", id: f.id, size: v }))));
    const picking = this.edgePicking === f.id;
    const row = document.createElement("div");
    row.className = "row";
    row.appendChild(button(picking ? "Done picking" : "Pick edges", () => {
      if (picking) this.endEdgePick();
      else this.beginEdgePick(f.id);
      this.renderDetail();
    }, picking ? "primary" : ""));
    if (k.edges.length > 0) row.appendChild(button("Clear", () => this.apply({ type: "set_blend", id: f.id, edges: [] })));
    body.appendChild(row);
    const ul = document.createElement("ul");
    ul.className = "edge-list";
    for (const e of k.edges) {
      const li = document.createElement("li");
      const label = document.createElement("span");
      label.textContent = this.describeEdge(e);
      li.appendChild(label);
      li.appendChild(button("×", () => this.apply({ type: "set_blend", id: f.id, edges: k.edges.filter((x) => x !== e) }), "danger"));
      ul.appendChild(li);
    }
    if (k.edges.length === 0) {
      const li = document.createElement("li");
      li.className = "note";
      li.textContent = "No edges yet. Pick edges in the viewport.";
      ul.appendChild(li);
    }
    body.appendChild(ul);
    const note = document.createElement("p");
    note.className = "note";
    note.textContent = "Edges are remembered by the two faces that meet there, so they follow later edits.";
    body.appendChild(note);
  }

  renderRevolveDetail(f: FeatureSummary, body: HTMLElement): void {
    if (f.kind.type !== "revolve") return;
    const k = f.kind;
    body.appendChild(this.sketchField(k.sketch));
    const sketch = this.feature(k.sketch);
    const lines = sketch && sketch.kind.type === "sketch" ? sketch.kind.sketch.entities.filter((e) => e.type === "line").map((e) => `line ${e.id}`) : [];
    const axisValue = k.axis.type === "line" ? `line ${k.axis.line}` : k.axis.type;
    body.appendChild(
      field("Axis", select(["x_axis", "y_axis", ...lines], axisValue, (v) => {
        const axis: RevolveAxis = v.startsWith("line ") ? { type: "line", line: Number(v.slice(5)) } : { type: v as "x_axis" | "y_axis" };
        this.apply({ type: "set_revolve", id: f.id, axis });
      })),
    );
    body.appendChild(field("Angle (°)", this.exprInput(f, "angle", k.angle, (v) => this.apply({ type: "set_revolve", id: f.id, angle: v }))));
    body.appendChild(this.regionField(k.sketch, k.profiles, (profiles) => this.apply({ type: "set_revolve", id: f.id, profiles })));
    body.appendChild(field("Result", select(["new", "add", "remove", "intersect"], k.op, (v) => this.apply({ type: "set_revolve", id: f.id, op: v as typeof k.op }))));
    const note = document.createElement("p");
    note.className = "note";
    note.textContent = "The axis is in sketch coordinates: the sketch's own X or Y axis, or one of its lines. The profile must stay on one side of it.";
    body.appendChild(note);
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
    body.appendChild(
      field("End", select(["blind", "through_all", "up_to_face"], k.end.type, (v) => {
        if (v === "up_to_face") {
          this.beginFacePick((face) => this.apply({ type: "set_extrude", id: f.id, end: { type: "up_to_face", face } }));
        } else {
          this.apply({ type: "set_extrude", id: f.id, end: { type: v as "blind" | "through_all" } });
        }
      })),
    );
    if (k.end.type === "blind") {
      body.appendChild(field("Depth", this.exprInput(f, "depth", k.depth, (v) => this.apply({ type: "set_extrude", id: f.id, depth: v }))));
    }
    if (k.end.type === "up_to_face") {
      const end: ExtrudeEnd = k.end;
      const row = document.createElement("div");
      row.className = "field";
      const l = document.createElement("label");
      l.textContent = "Face";
      const v = document.createElement("span");
      v.className = "note";
      v.textContent = this.describeFace(end.face);
      row.append(l, v);
      body.appendChild(row);
      body.appendChild(
        field("", button("Pick another face", () =>
          this.beginFacePick((face) => this.apply({ type: "set_extrude", id: f.id, end: { type: "up_to_face", face } })),
        )),
      );
    }
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
    if (e.key === "Enter") i.blur();
  };
  return i;
}

function textInput(value: string, onCommit: (v: string) => void): HTMLInputElement {
  const i = document.createElement("input");
  i.type = "text";
  i.value = value;
  i.onchange = () => {
    const v = i.value.trim();
    if (v && v !== value) onCommit(v);
  };
  i.onkeydown = (e) => {
    if (e.key === "Enter") i.blur();
  };
  return i;
}

function checkbox(value: boolean, onChange: (v: boolean) => void): HTMLInputElement {
  const i = document.createElement("input");
  i.type = "checkbox";
  i.checked = value;
  i.onchange = () => onChange(i.checked);
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
    case "symmetric": return ["a", "b", "line"];
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
  // Exposed for debugging and end-to-end tests.
  (window as unknown as { offkilter: App }).offkilter = app;

  $("#btn-new").onclick = () => {
    if (confirm("Start a new empty part studio? Unsaved work is lost.")) app.replace(Kernel.empty());
  };
  $("#btn-demo").onclick = () => {
    app.sync.disconnect();
    app.showPresence(0, []);
    app.replace(Kernel.demo());
  };
  // ---- account
  let user: UserInfo | null = null;
  const accountDialog = $("#account-dialog") as HTMLDialogElement;
  const showUser = () => {
    const b = $("#btn-account") as HTMLButtonElement;
    b.textContent = user ? user.name : "Sign in";
    b.title = user ? `Signed in as ${user.name} (click to sign out)` : "Sign in to own and share documents";
  };
  const setUser = (u: UserInfo | null) => {
    user = u;
    showUser();
    if (u) localStorage.setItem("offkilter.user", u.name);
  };
  if (await Sync.available()) setUser(await Sync.me());
  showUser();
  const accountForm = async (register: boolean) => {
    const name = ($("#account-name") as HTMLInputElement).value.trim();
    const password = ($("#account-password") as HTMLInputElement).value;
    const note = $("#account-note");
    try {
      setUser(register ? await Sync.register(name, password) : await Sync.login(name, password));
      ($("#account-password") as HTMLInputElement).value = "";
      note.textContent = "";
      accountDialog.close();
      // Reconnect so presence shows the account name.
      if (app.sync.docId) openDoc(app.sync.docId);
    } catch (e) {
      note.textContent = (e as Error).message;
    }
  };
  $("#btn-account").onclick = async () => {
    if (user) {
      if (confirm(`Sign out ${user.name}?`)) {
        await Sync.logout();
        setUser(null);
        if (app.sync.docId) {
          app.sync.disconnect();
          app.showPresence(0, []);
          app.setStatus("signed out; the document was closed");
        }
      }
      return;
    }
    if (!(await Sync.available())) {
      alert("No document server at this origin, so there is nothing to sign in to.");
      return;
    }
    $("#account-note").textContent = "";
    accountDialog.showModal();
    ($("#account-name") as HTMLInputElement).focus();
  };
  ($("#account-form") as HTMLFormElement).onsubmit = (e) => {
    e.preventDefault();
    void accountForm(false);
  };
  $("#account-register").onclick = () => void accountForm(true);
  $("#account-close").onclick = () => accountDialog.close();

  // ---- documents on the server
  const dialog = $("#docs-dialog") as HTMLDialogElement;
  const renderDocs = async () => {
    const list = $("#docs-list");
    list.innerHTML = "";
    const note = $("#docs-note");
    if (!(await Sync.available())) {
      note.textContent = "No document server at this origin. Run `cargo run -p ok-server -- --static apps/web/dist` and open its address to share documents.";
      return;
    }
    note.textContent = (app.sync.docId ? `Editing document ${app.sync.docId} live. ` : "Open a document to edit it with others in real time. ") + (user ? `Signed in as ${user.name}; new documents are yours.` : "Not signed in; new documents are open to everyone here.");
    let docs: DocMeta[] = [];
    try {
      docs = await Sync.listDocs();
    } catch (e) {
      note.textContent = `Could not list documents: ${(e as Error).message}`;
      return;
    }
    if (docs.length === 0) {
      const li = document.createElement("li");
      li.textContent = "No documents yet.";
      list.appendChild(li);
    }
    await renderVersions();
    for (const d of docs) {
      const li = document.createElement("li");
      const name = document.createElement("span");
      name.className = "dname";
      name.textContent = d.name;
      name.onclick = () => {
        dialog.close();
        openDoc(d.id);
      };
      const owner = document.createElement("span");
      owner.className = "downer";
      const mine = !!user && d.owner?.id === user.id;
      const shared = d.collaborators ?? [];
      owner.textContent = d.owner ? (mine ? "yours" : `by ${d.owner.name}`) + (shared.length ? ` · shared with ${shared.map((c) => c.name).join(", ")}` : "") : "open to all";
      const when = document.createElement("span");
      when.className = "dwhen";
      when.textContent = new Date(d.updated * 1000).toLocaleString();
      li.append(name, owner, when);
      if (mine) {
        const share = button("Share…", async () => {
          const who = prompt(`Share "${d.name}" with which account name?`);
          if (!who) return;
          try {
            await Sync.shareDoc(d.id, who.trim());
            await renderDocs();
          } catch (e) {
            note.textContent = `Could not share: ${(e as Error).message}`;
          }
        });
        share.className = "dshare";
        li.appendChild(share);
        for (const c of shared) {
          const un = button(`− ${c.name}`, async () => {
            await Sync.unshareDoc(d.id, c.id);
            await renderDocs();
          });
          un.className = "dshare";
          un.title = `Stop sharing with ${c.name}`;
          li.appendChild(un);
        }
      }
      if (!d.owner || mine) {
        li.appendChild(button("×", async () => {
          if (confirm(`Delete "${d.name}" from the server?`)) {
            try {
              await Sync.deleteDoc(d.id);
            } catch (e) {
              note.textContent = `Could not delete: ${(e as Error).message}`;
            }
            if (app.sync.docId === d.id) app.sync.disconnect();
            await renderDocs();
          }
        }, "danger"));
      }
      list.appendChild(li);
    }
  };
  const renderVersions = async () => {
    const box = $("#versions");
    const list = $("#versions-list");
    list.innerHTML = "";
    const id = app.sync.docId;
    box.hidden = !id;
    if (!id) return;
    let versions: VersionMeta[] = [];
    try {
      versions = await Sync.listVersions(id);
    } catch {
      return;
    }
    if (versions.length === 0) {
      const li = document.createElement("li");
      li.textContent = "No saved versions.";
      list.appendChild(li);
    }
    for (const v of versions) {
      const li = document.createElement("li");
      const name = document.createElement("span");
      name.className = "dname";
      name.textContent = v.name;
      name.title = "Restore this version";
      name.onclick = async () => {
        if (!confirm(`Restore version "${v.name}"? Everyone editing this document gets it.`)) return;
        await Sync.restoreVersion(id, v.id);
        dialog.close();
      };
      const when = document.createElement("span");
      when.className = "dwhen";
      when.textContent = new Date(v.created * 1000).toLocaleString();
      li.append(name, when);
      list.appendChild(li);
    }
  };
  $("#versions-save").onclick = async () => {
    const id = app.sync.docId;
    if (!id) return;
    const name = prompt("Version name", `v${new Date().toISOString().slice(0, 16).replace("T", " ")}`);
    if (!name) return;
    await Sync.saveVersion(id, name);
    await renderVersions();
  };
  const openDoc = (id: string) => {
    app.sync.connect(id);
    const url = new URL(location.href);
    url.searchParams.set("doc", id);
    history.replaceState(null, "", url.toString());
  };
  $("#btn-docs").onclick = async () => {
    await renderDocs();
    dialog.showModal();
  };
  $("#docs-close").onclick = () => dialog.close();
  $("#docs-create").onclick = async () => {
    const name = prompt("Document name", "Untitled");
    if (!name) return;
    try {
      const meta = await Sync.createDoc(name);
      dialog.close();
      openDoc(meta.id);
    } catch (e) {
      $("#docs-note").textContent = `Could not create: ${(e as Error).message}`;
    }
  };
  $("#docs-upload").onclick = async () => {
    try {
      const meta = await Sync.createDoc(app.summary.name, app.kernel.toJson());
      dialog.close();
      openDoc(meta.id);
    } catch (e) {
      $("#docs-note").textContent = `Could not upload: ${(e as Error).message}`;
    }
  };
  const docParam = new URL(location.href).searchParams.get("doc");
  if (docParam) openDoc(docParam);
  ($("#quality") as HTMLSelectElement).onchange = (e) => {
    app.apply({ type: "set_settings", facet_angle: Number((e.target as HTMLSelectElement).value) });
    app.setStatus(app.statusLine() + " · quality changed");
  };
  $("#btn-undo").onclick = () => app.undo();
  $("#btn-redo").onclick = () => app.redo();
  $("#btn-view-top").onclick = () => app.viewer.setStandardView("top");
  $("#btn-view-front").onclick = () => app.viewer.setStandardView("front");
  $("#btn-view-right").onclick = () => app.viewer.setStandardView("right");
  $("#btn-view-iso").onclick = () => app.viewer.setStandardView("iso");
  $("#btn-measure").onclick = () => (app.measure ? app.endMeasure() : app.beginMeasure());
  const sectionAxis = $("#section-axis") as HTMLSelectElement;
  const sectionOffset = $("#section-offset") as HTMLInputElement;
  sectionAxis.onchange = () => {
    const axis = sectionAxis.value as "" | "x" | "y" | "z";
    if (axis === "") app.setSection(null);
    else app.setSection({ axis, t: Number(sectionOffset.value) / 1000, flip: app.section?.flip ?? false });
  };
  sectionOffset.oninput = () => {
    if (app.section) app.setSection({ ...app.section, t: Number(sectionOffset.value) / 1000 });
  };
  $("#section-flip").onclick = () => {
    if (app.section) app.setSection({ ...app.section, flip: !app.section.flip });
  };
  app.undo(); // no-op that initialises the button states
  const exportSelect = $("#export") as HTMLSelectElement;
  exportSelect.onchange = () => {
    const what = exportSelect.value;
    exportSelect.value = "";
    if (what === "stl") app.download(app.toStl(), "stl");
    else if (what === "3mf") app.download(app.to3mf(), "3mf");
    else if (what === "dxf") {
      const dxf = app.toDxf();
      if (dxf === null) {
        app.setStatus("Select a sketch in the feature list to export it as DXF.");
        return;
      }
      app.download(new Blob([dxf], { type: "application/dxf" }), "dxf", `${app.summary.name}-${app.feature(app.selected)!.name}`);
    }
  };
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
    app.applyDoc({ type: "rename_document", name: (e.target as HTMLInputElement).value });
  };
  $("#btn-insert-instance").onclick = () => {
    app.selectedInstance = null;
    app.selectedMate = null;
    app.renderAssembly();
    app.renderDetail();
  };
  $("#btn-add-mate").onclick = () => app.beginMatePick();
  $("#btn-interference").onclick = () => app.checkInterference();
  $("#btn-add-sketch").onclick = () => {
    const addOn = (plane: PlaneRef) => {
      app.selectedFace = null;
      app.viewer.setSelectedFace(null);
      app.apply({ type: "add_sketch", plane, name: app.autoName("Sketch") });
      const id = app.summary.features[app.summary.features.length - 1]?.id ?? null;
      app.selected = id;
      if (id !== null) app.editSketch(id);
    };
    if (app.selectedFace) {
      addOn({ type: "face", face: app.selectedFace, offset: 0 });
      return;
    }
    const base = (prompt("Sketch plane: top, front, right, or 'face' to pick one", "top") ?? "").trim().toLowerCase();
    if (base === "face") {
      app.beginFacePick((face) => addOn({ type: "face", face, offset: 0 }));
      return;
    }
    if (!["top", "front", "right"].includes(base)) return;
    addOn({ type: "standard", base: base as StandardPlane, offset: 0 });
  };
  $("#btn-add-extrude").onclick = () => {
    const f = app.feature(app.selected);
    if (!f || f.kind.type !== "sketch") return;
    app.sketcher.exit();
    app.apply({ type: "add_extrude", sketch: f.id, depth: 10, profiles: { type: "all" }, name: app.autoName("Extrude") });
    app.select(app.summary.features[app.summary.features.length - 1]?.id ?? null);
  };
  const addBlend = (kind: BlendKind) => {
    app.sketcher.exit();
    app.apply({ type: "add_blend", kind, edges: [], size: kind === "fillet" ? 2 : 1, name: app.autoName(kind === "fillet" ? "Fillet" : "Chamfer") });
    const id = app.summary.features[app.summary.features.length - 1]?.id ?? null;
    app.select(id);
    if (id !== null) {
      app.beginEdgePick(id);
      app.renderDetail();
    }
  };
  $("#btn-add-fillet").onclick = () => addBlend("fillet");
  $("#btn-add-variable").onclick = () => {
    const name = (prompt("Variable name (letters, digits, underscore)", "width") ?? "").trim().replace(/^#/, "");
    if (!/^\w+$/.test(name)) return;
    app.sketcher.exit();
    app.apply({ type: "add_variable", name, expression: "10" });
    app.select(app.summary.features[app.summary.features.length - 1]?.id ?? null);
  };
  $("#btn-add-mirror").onclick = () => {
    app.sketcher.exit();
    app.apply({ type: "add_mirror", plane: { type: "standard", base: "right", offset: 0 }, op: "add", name: app.autoName("Mirror") });
    app.select(app.summary.features[app.summary.features.length - 1]?.id ?? null);
  };
  $("#btn-add-pattern").onclick = () => {
    app.sketcher.exit();
    app.apply({ type: "add_pattern", kind: { type: "linear", axis: "x", spacing: 20 }, count: 2, op: "add", name: app.autoName("Pattern") });
    app.select(app.summary.features[app.summary.features.length - 1]?.id ?? null);
  };
  $("#btn-add-chamfer").onclick = () => addBlend("chamfer");
  $("#btn-add-hole").onclick = () => {
    const f = app.feature(app.selected);
    if (!f || f.kind.type !== "sketch") return;
    app.sketcher.exit();
    app.apply({ type: "add_hole", sketch: f.id, diameter: 6, through_all: true, direction: "reverse", name: app.autoName("Hole") });
    app.select(app.summary.features[app.summary.features.length - 1]?.id ?? null);
  };
  const otherSketch = (f: FeatureSummary) => app.summary.features.find((s) => s.kind.type === "sketch" && s.id !== f.id);
  $("#btn-add-sweep").onclick = () => {
    const f = app.feature(app.selected);
    const other = f && otherSketch(f);
    if (!f || f.kind.type !== "sketch" || !other) return;
    app.sketcher.exit();
    app.apply({ type: "add_sweep", sketch: f.id, path: other.id, profiles: { type: "all" }, name: app.autoName("Sweep") });
    app.select(app.summary.features[app.summary.features.length - 1]?.id ?? null);
  };
  $("#btn-add-loft").onclick = () => {
    const f = app.feature(app.selected);
    const other = f && otherSketch(f);
    if (!f || f.kind.type !== "sketch" || !other) return;
    app.sketcher.exit();
    app.apply({ type: "add_loft", sketch: f.id, sketch_b: other.id, name: app.autoName("Loft") });
    app.select(app.summary.features[app.summary.features.length - 1]?.id ?? null);
  };
  $("#btn-add-revolve").onclick = () => {
    const f = app.feature(app.selected);
    if (!f || f.kind.type !== "sketch") return;
    app.sketcher.exit();
    app.apply({ type: "add_revolve", sketch: f.id, axis: { type: "y_axis" }, angle: 360, profiles: { type: "all" }, name: app.autoName("Revolve") });
    app.select(app.summary.features[app.summary.features.length - 1]?.id ?? null);
  };
  window.addEventListener("keydown", (e) => {
    const typing = e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement || e.target instanceof HTMLTextAreaElement;
    if (typing) return;
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "z") {
      e.preventDefault();
      if (e.shiftKey) app.redo();
      else app.undo();
      return;
    }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "y") {
      e.preventDefault();
      app.redo();
      return;
    }
    if (e.key === "f") app.viewer.fitAll();
    if (!app.sketcher.active) {
      const view = ({ "1": "top", "2": "front", "3": "right", "0": "iso" } as Record<string, "top" | "front" | "right" | "iso">)[e.key];
      if (view) app.viewer.setStandardView(view);
    }
    if (e.key === "Escape") {
      if (app.measure) app.endMeasure();
      else if (app.matePick) {
        app.endMatePick();
        app.setStatus(app.statusLine());
      } else if (app.facePicker) app.endFacePick();
      else if (app.edgePicking !== null) {
        app.endEdgePick();
        app.renderDetail();
      } else if (app.sketcher.active) {
        app.sketcher.cancel();
        if (app.sketcher.tool !== "select") app.sketcher.setTool("select");
        else {
          app.sketcher.exit();
          app.renderDetail();
        }
      } else app.onViewportPick(null);
    }
    if (app.sketcher.active) {
      const tool = ({ s: "select", l: "line", r: "rectangle", c: "circle", a: "arc", t: "trim", u: "use" } as Record<string, Tool>)[e.key.toLowerCase()];
      if (tool) app.sketcher.setTool(tool);
      if (e.key.toLowerCase() === "o") app.sketcher.offsetSelection();
      if (e.key.toLowerCase() === "m") app.sketcher.beginMirror();
      if (e.key.toLowerCase() === "q") app.sketcher.toggleConstruction();
      if (e.key === "Delete" || e.key === "Backspace") app.sketcher.deleteSelection();
    }
  });
}

main().catch((e) => {
  $("#status-text").textContent = `failed to start: ${(e as Error).message}`;
  console.error(e);
});
