import { Kernel } from "./kernel";
import { detailView, dimensionOffsetFor, drawingFrame, snapDrawingPoint, to3mf, toBom, toDrawingDxf, toDrawingSvg, toDxf, toStl, type DrawingFrame, type DrawingView, type SheetSize, type UserDimension } from "./export";
import { parseObj, parseStl } from "./stl";
import type { Axis, BlendKind, BooleanOp, Connector, Constraint, CopyOp, Placement, DocOp, DocOpResult, EdgeRef, ExtrudeDirection, ExtrudeEnd, FaceRef, FeatureSummary, InstanceSummary, MateKind, MateSummary, Op, OpResult, PatternKind, PlaneRef, ProfileSelection, ProjectionSource, RevolveAxis, SketchData, SketchOp, StandardPlane, Summary, Vec2, Vec3 } from "./kernel";
import { Viewer } from "./viewer";
import type { EdgePick, FacePick } from "./viewer";
import { Sketcher } from "./sketcher";
import type { SketchHost, Tool } from "./sketcher";
import { Sync } from "./sync";
import type { DocMeta, Team, UserInfo, VersionMeta } from "./sync";

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
      readOnly: (ro) => this.setReadOnly(ro),
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
  /** A running mate animation: display only, nothing is written to the document. */
  mateAnimation: { id: number; raf: number; start: number; angle: number; offset: number; spin: boolean; travel: number } | null = null;
  viewer = new Viewer($("#viewport"));

  constructor(kernel: Kernel) {
    this.kernel = kernel;
    this.viewer.onPick = (pick, e) => this.onViewportPick(pick, e);
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

  /** The connector under the pointer: a corner when snapped to one, else an edge, else the picked face. */
  connectorAt(e: PointerEvent | undefined, pick: FacePick | null): { connector: Connector; pick: FacePick; edge?: EdgePick } | null {
    const inst = pick ? this.instanceAt(pick.body) : undefined;
    const ref = pick ? this.faceRefOf(pick) : null;
    if (!e || !pick || !inst || !ref) return null;
    const corner = this.viewer.pickVertex(e);
    if (corner && corner.body === pick.body) {
      const refs = corner.faces.map((f) => this.summary.bodies[corner.body]!.faces[f]!.origin);
      const distinct = refs.filter((r, i) => refs.findIndex((q) => q.feature === r.feature && q.local === r.local) === i);
      const others = distinct.filter((r) => r.feature !== ref.feature || r.local !== ref.local);
      if (others.length >= 2) return { connector: { instance: inst.id, face: ref, anchor: { type: "vertex", others: [others[0]!, others[1]!] } }, pick };
    }
    const edgeOrFace = this.viewer.pickEdgeOrFace(e);
    if (edgeOrFace && "edge" in edgeOrFace && edgeOrFace.edge.body === pick.body) {
      const edge = this.edgeRefOf(edgeOrFace.edge);
      if (edge) {
        const face = edge.a.feature === ref.feature && edge.a.local === ref.local ? edge.a : edge.b;
        const other = face === edge.a ? edge.b : edge.a;
        return { connector: { instance: inst.id, face, anchor: { type: "edge", other } }, pick, edge: edgeOrFace.edge };
      }
    }
    return { connector: { instance: inst.id, face: ref }, pick };
  }

  describeConnector(c: Connector): string {
    const name = this.instance(c.instance)?.name ?? "?";
    const a = c.anchor ?? { type: "face" };
    if (a.type === "edge") return `${name} · edge ${c.face.local}/${a.other.local} of feature ${c.face.feature}`;
    if (a.type === "vertex") return `${name} · corner ${c.face.local}/${a.others[0].local}/${a.others[1].local} of feature ${c.face.feature}`;
    return `${name} · face ${c.face.local} of feature ${c.face.feature}`;
  }

  onViewportPick(pick: FacePick | null, e?: PointerEvent): void {
    const ref = pick ? this.faceRefOf(pick) : null;
    if (this.summary.kind === "assembly") {
      const inst = pick ? this.instanceAt(pick.body) : undefined;
      if (this.matePick) {
        const hit = this.connectorAt(e, pick);
        if (!hit || !inst) return;
        const connector = hit.connector;
        if (!this.matePick.a) {
          this.matePick.a = connector;
          this.viewer.setSelectedFace(hit.edge ? null : hit.pick);
          this.viewer.setSelectedEdges(hit.edge ? [hit.edge] : []);
          const f = this.kernel.connectorFrame(connector);
          this.viewer.setFrames(f ? [f] : []);
          this.setStatus(`First connector: ${this.describeConnector(connector)}. Now click a face, edge or corner on the other instance (Esc cancels).`);
          return;
        }
        if (this.matePick.a.instance === inst.id) {
          this.setStatus("Pick a face, edge or corner on a different instance (Esc cancels).");
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
    this.viewer.pickConnectors = true;
    this.setStatus("Mate: click a face, edge or corner on the first instance (Esc cancels)");
  }

  endMatePick(): void {
    this.matePick = null;
    this.viewer.pickMode = false;
    this.viewer.pickConnectors = false;
    this.viewer.setSelectedFace(null);
    this.viewer.setSelectedEdges([]);
    this.viewer.hoverEdgeOrFace(null);
    this.viewer.setFrames([]);
  }

  // ------------------------------------------------------------ mate animation

  /** Sweeps a mate's free motion for display: a full turn every 4 s for spinning mates, a there-and-back slide for sliders. */
  startMateAnimation(id: number): void {
    const m = this.mate(id);
    if (!m || this.summary.kind !== "assembly" || this.tab === null) return;
    this.stopMateAnimation();
    const spin = m.kind === "revolute" || m.kind === "cylindrical";
    const slide = m.kind === "slider";
    if (!spin && !slide) return;
    const bounds = this.viewer.bodyBounds();
    const size = bounds ? Math.hypot(bounds.max.x - bounds.min.x, bounds.max.y - bounds.min.y, bounds.max.z - bounds.min.z) : 20;
    const anim = { id, raf: 0, start: performance.now(), angle: m.angle, offset: m.offset, spin, travel: slide ? size / 4 : 0 };
    this.mateAnimation = anim;
    const frame = (now: number) => {
      if (this.mateAnimation !== anim) return;
      const t = (now - anim.start) / 4000;
      const angle = anim.spin ? anim.angle + (t * 360) % 360 : anim.angle;
      const offset = anim.travel ? anim.offset + anim.travel * Math.sin(t * 2 * Math.PI) : anim.offset;
      const deltas = this.kernel.matePreview(this.tab!, anim.id, angle, offset);
      this.viewer.setBodyTransforms(deltas);
      this.setStatus(deltas ? `Animating ${m.name}: ${anim.spin ? `${angle.toFixed(0)}°` : `${offset.toFixed(1)} mm`} (Esc stops)` : `Animating ${m.name}: cannot resolve this frame`);
      anim.raf = requestAnimationFrame(frame);
    };
    anim.raf = requestAnimationFrame(frame);
    this.renderDetail();
  }

  stopMateAnimation(): void {
    const anim = this.mateAnimation;
    if (!anim) return;
    cancelAnimationFrame(anim.raf);
    this.mateAnimation = null;
    this.viewer.setBodyTransforms(null);
    this.setStatus(this.statusLine());
  }

  // ------------------------------------------------------------ instance drag

  /** Drag mode for free instances in an assembly tab. */
  instanceDrag: { active: { id: number; start: Placement; origin: Vec3; from: Vec3 } | null } | null = null;

  beginInstanceDrag(): void {
    if (this.summary.kind !== "assembly") return;
    this.endMatePick();
    this.endMeasure();
    this.instanceDrag = { active: null };
    ($("#btn-move-instance") as HTMLButtonElement).classList.add("active");
    this.viewer.pointerHandler = {
      down: (e) => this.instanceDragDown(e),
      move: (e) => this.instanceDragMove(e),
      up: () => {
        if (this.instanceDrag) this.instanceDrag.active = null;
      },
      cancel: () => this.endInstanceDrag(),
    };
    this.setStatus("Move: drag an instance that is not fixed or mated. Esc stops.");
  }

  private instanceDragDown(e: PointerEvent): void {
    if (!this.instanceDrag) return;
    const body = this.viewer.pickBodyIndex(e);
    const inst = body === null ? undefined : this.instanceAt(body);
    if (!inst) return;
    if (inst.fixed) {
      this.setStatus(`${inst.name} is fixed; untick Fixed in its panel to move it.`);
      return;
    }
    if (this.summary.mates.some((m) => !m.error && (m.a.instance === inst.id || m.b.instance === inst.id))) {
      this.setStatus(`${inst.name} is positioned by its mates; edit the mate instead.`);
      return;
    }
    const origin = inst.transform?.t ?? inst.placement.position;
    const from = this.viewer.pickOnViewPlane(e, origin);
    if (!from) return;
    this.snapshot();
    this.instanceDrag.active = { id: inst.id, start: { position: { ...inst.placement.position }, rotation: { ...inst.placement.rotation } }, origin, from };
    this.selectedInstance = inst.id;
    this.selectedMate = null;
  }

  private instanceDragMove(e: PointerEvent): void {
    const active = this.instanceDrag?.active;
    if (!active) return;
    const to = this.viewer.pickOnViewPlane(e, active.origin);
    if (!to) return;
    const position = { x: active.start.position.x + to.x - active.from.x, y: active.start.position.y + to.y - active.from.y, z: active.start.position.z + to.z - active.from.z };
    try {
      this.applyDocRaw({ type: "assembly", tab: this.tab!, op: { type: "set_instance", id: active.id, placement: { position, rotation: active.start.rotation } } });
    } catch (err) {
      this.setStatus(`error: ${(err as Error).message}`);
      return;
    }
    this.regenerate();
  }

  endInstanceDrag(): void {
    if (!this.instanceDrag) return;
    this.instanceDrag = null;
    this.viewer.pointerHandler = null;
    ($("#btn-move-instance") as HTMLButtonElement).classList.remove("active");
    this.setStatus(this.statusLine());
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
    this.endInstanceDrag();
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
      const note = document.createElement("p");
      note.className = "note";
      note.textContent = `A: ${this.describeConnector(m.a)}\nB: ${this.describeConnector(m.b)}. B moves onto A (or A onto B when only B is placed); connector frames meet with z axes opposed unless flipped (a face's z is its normal, an edge's runs along it, a corner's follows its face).`;
      body.appendChild(note);
      body.appendChild(field("Offset", numberInput(m.offset, (v) => this.applyDoc({ type: "assembly", tab, op: { type: "set_mate", id: m.id, offset: v } }))));
      body.appendChild(field("Angle°", numberInput(m.angle, (v) => this.applyDoc({ type: "assembly", tab, op: { type: "set_mate", id: m.id, angle: v } }))));
      body.appendChild(field("Flip", checkbox(m.flip, (v) => this.applyDoc({ type: "assembly", tab, op: { type: "set_mate", id: m.id, flip: v } }))));
      if (m.kind === "revolute" || m.kind === "cylindrical" || m.kind === "slider") {
        const running = this.mateAnimation?.id === m.id;
        const b = button(running ? "Stop animation" : "Animate", () => {
          if (running) this.stopMateAnimation();
          else this.startMateAnimation(m.id);
          this.renderDetail();
        });
        b.id = "btn-animate-mate";
        b.title = m.kind === "slider" ? "Slide back and forth along the mate axis (display only)" : "Spin a full turn about the mate axis (display only)";
        body.appendChild(field("", b));
      }
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
    intro.textContent = "Pick a part studio and one of its bodies. The first instance is fixed; mate the rest to it with “+ Mate”: click a face, edge or corner on each of two instances and their connector frames meet.";
    body.appendChild(intro);
    this.renderInsertForm(body);
  }

  // ------------------------------------------------------------ document

  replace(kernel: Kernel): void {
    this.sketcher.exit();
    this.setReadOnly(false);
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

  /** Front, top, right and isometric views of the current bodies with hidden lines removed. */
  drawingViews(): DrawingView[] {
    const z = { x: 0, y: 0, z: 1 };
    const views: DrawingView[] = [
      { name: "front", lines: this.kernel.drawingView({ x: 0, y: 1, z: 0 }, z) },
      { name: "top", lines: this.kernel.drawingView({ x: 0, y: 0, z: -1 }, { x: 0, y: 1, z: 0 }) },
      { name: "right", lines: this.kernel.drawingView({ x: -1, y: 0, z: 0 }, z) },
      { name: "iso", lines: this.kernel.drawingView({ x: -0.6, y: 0.7, z: -0.5 }, z) },
    ];
    const section = this.drawingSectionView();
    if (section) views.push(section);
    const detail = this.drawingDetailView(views);
    if (detail) views.push(detail);
    return views;
  }

  /**
   * DETAIL B: the neighbourhood of the selected face, enlarged 2:1, taken
   * from whichever standard view faces it best. Nothing without a selected face.
   */
  drawingDetailView(views: DrawingView[]): DrawingView | null {
    if (!this.selectedFace || this.summary.kind === "assembly") return null;
    const pick = this.findFace(this.selectedFace);
    if (!pick) return null;
    const info = this.summary.bodies[pick.body]?.faces[pick.face];
    const fb = this.viewer.faceBounds(pick.body, pick.face);
    if (!info || !fb) return null;
    // View frames as the kernel projects them: u = dir × up, v = u × dir.
    const frames: Record<string, { dir: Vec3; up: Vec3 }> = { front: { dir: { x: 0, y: 1, z: 0 }, up: { x: 0, y: 0, z: 1 } }, top: { dir: { x: 0, y: 0, z: -1 }, up: { x: 0, y: 1, z: 0 } }, right: { dir: { x: -1, y: 0, z: 0 }, up: { x: 0, y: 0, z: 1 } } };
    const cross = (a: Vec3, b: Vec3): Vec3 => ({ x: a.y * b.z - a.z * b.y, y: a.z * b.x - a.x * b.z, z: a.x * b.y - a.y * b.x });
    const dot = (a: Vec3, b: Vec3) => a.x * b.x + a.y * b.y + a.z * b.z;
    // The view whose direction opposes the face normal most sees the face head on.
    let best: { name: string; score: number } | null = null;
    for (const [name, f] of Object.entries(frames)) {
      if (!views.some((v) => v.name === name)) continue;
      const score = -dot(f.dir, info.normal);
      if (!best || score > best.score) best = { name, score };
    }
    if (!best) return null;
    const source = views.find((v) => v.name === best!.name)!;
    const f = frames[best.name]!;
    const u = cross(f.dir, f.up);
    const v = cross(u, f.dir);
    const centre = { x: dot(fb.centre, u), y: dot(fb.centre, v) };
    const size = Math.hypot(fb.max.x - fb.min.x, fb.max.y - fb.min.y, fb.max.z - fb.min.z);
    // A detail of a face that spans the whole view is no detail: cap the circle at a third of the view.
    let ext = 0;
    for (const [a, b] of [...source.lines.visible, ...source.lines.hidden]) ext = Math.max(ext, Math.abs(a.x - centre.x), Math.abs(a.y - centre.y), Math.abs(b.x - centre.x), Math.abs(b.y - centre.y));
    const radius = Math.min(Math.max(size * 0.6, 2), Math.max(ext * 0.35, 2));
    return detailView(source, centre, radius, 2, "B");
  }

  /**
   * Section A-A: the viewport's section plane when one is shown, else a cut through the
   * middle of the model parallel to the front view. The removed side is the one the
   * viewport removes (the axis coordinate beyond the offset, or before it when flipped).
   */
  drawingSectionView(): DrawingView | null {
    const bounds = this.viewer.bodyBounds();
    if (!bounds) return null;
    const sec = this.section ?? { axis: "y" as const, t: 0.5, flip: true };
    const at = bounds.min[sec.axis] + (bounds.max[sec.axis] - bounds.min[sec.axis]) * sec.t;
    const unit = { x: sec.axis === "x" ? 1 : 0, y: sec.axis === "y" ? 1 : 0, z: sec.axis === "z" ? 1 : 0 };
    const sign = sec.flip ? -1 : 1;
    const normal = { x: unit.x * sign, y: unit.y * sign, z: unit.z * sign };
    // Look at the cut from the removed side: along -normal, with z (or y for a horizontal cut) up.
    const dir = { x: -normal.x, y: -normal.y, z: -normal.z };
    const up = sec.axis === "z" ? { x: 0, y: 1, z: 0 } : { x: 0, y: 0, z: 1 };
    const origin = { x: unit.x * at, y: unit.y * at, z: unit.z * at };
    const lines = this.kernel.drawingSection(dir, up, origin, normal);
    if (lines.cut.length === 0) return null;
    // Where the cutting plane shows edge-on: a horizontal trace on the top view for a
    // y cut (top view y = model y), vertical on the top view for an x cut, and a
    // horizontal trace on the front view for a z cut (front view y = model z).
    const trace = sec.axis === "y" ? { on: "top", horizontal: true, at } : sec.axis === "x" ? { on: "top", horizontal: false, at } : { on: "front", horizontal: true, at };
    return { name: "section", lines, cut: lines.cut, trace: { ...trace, label: "A", towards: sign } };
  }

  /** Which views the drawing sheet shows (all by default) and its sheet size. */
  drawingOptions: { views: Set<string>; sheet: SheetSize } = { views: new Set(["front", "top", "right", "iso", "section", "detail"]), sheet: "A4" };

  /** The dimensions placed on this tab's drawing; they live in the document. */
  drawingDimensions(): UserDimension[] {
    return this.summary.tabs.find((t) => t.id === this.summary.tab)?.drawing ?? [];
  }

  /** Where the chosen views sit on the sheet, for picking points in the preview. */
  drawingFrame(): DrawingFrame {
    return drawingFrame(this.chosenDrawingViews(), this.drawingOptions.sheet, this.drawingDimensions());
  }

  /**
   * Places a dimension between two points of a view (view coordinates); the
   * dimension line goes on the side away from the view's middle. One undo
   * step. Returns the measured value.
   */
  addDrawingDimension(view: string, a: Vec2, b: Vec2): number {
    const v = this.chosenDrawingViews().find((x) => x.name === view);
    if (!v) throw new Error(`no view ${view} on the sheet`);
    const dims = [...this.drawingDimensions(), { view, a, b, offset: dimensionOffsetFor(v, a, b) }];
    this.applyDoc({ type: "set_drawing_dimensions", tab: this.summary.tab, dims });
    return Math.hypot(b.x - a.x, b.y - a.y);
  }

  /** Removes every placed dimension of this tab's drawing (one undo step). */
  clearDrawingDimensions(): void {
    if (this.drawingDimensions().length > 0) this.applyDoc({ type: "set_drawing_dimensions", tab: this.summary.tab, dims: [] });
  }

  /** The chosen drawing views only. */
  chosenDrawingViews(): DrawingView[] {
    return this.drawingViews().filter((v) => this.drawingOptions.views.has(v.name));
  }

  /** A drawing sheet of the current bodies as SVG. */
  toDrawingSvg(): string {
    return toDrawingSvg(this.chosenDrawingViews(), this.summary.name, this.drawingOptions.sheet, this.drawingDimensions());
  }

  /** The drawing views as DXF lines at 1:1. */
  toDrawingDxf(): string {
    return toDrawingDxf(this.chosenDrawingViews(), this.drawingDimensions());
  }

  /** A bill of materials of the current tab as CSV. */
  toBom(): string {
    return toBom(this.summary);
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

  /** The open server document is shared with this account read-only. */
  readOnly = false;

  setReadOnly(readOnly: boolean): void {
    this.readOnly = readOnly;
    document.body.classList.toggle("read-only", readOnly);
    ($("#read-only") as HTMLElement).hidden = !readOnly;
  }

  applyDocRaw(op: DocOp): DocOpResult {
    if (this.readOnly) throw new Error("this document is shared with you read-only");
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
    this.stopMateAnimation();
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
      const icons: Record<string, string> = { sketch: "✎", extrude: "⬒", revolve: "◑", blend: "◜", mirror: "⇔", pattern: "⁝⁝", variable: "#", hole: "◎", sweep: "↝", loft: "⋀", boolean: "∪", shell: "◱", move_face: "⇥", draft: "◿", mesh: "▲", split: "⫽" };
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
    ($("#btn-add-split") as HTMLButtonElement).disabled = !hasBody;
    ($("#btn-add-boolean") as HTMLButtonElement).disabled = this.summary.bodies.length < 2;
    ($("#btn-add-shell") as HTMLButtonElement).disabled = !hasBody;
    ($("#btn-add-move-face") as HTMLButtonElement).disabled = !hasBody;
    ($("#btn-add-draft") as HTMLButtonElement).disabled = !hasBody;
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
      const mass = b.mass !== undefined ? ` · ${b.mass >= 1000 ? `${(b.mass / 1000).toFixed(3)} kg` : `${b.mass.toFixed(1)} g`}` : "";
      stats.textContent = `${b.volume.toFixed(1)} mm³ · ${b.area.toFixed(1)} mm²${mass}`;
      stats.title = (c ? `centre of mass (${c.x.toFixed(2)}, ${c.y.toFixed(2)}, ${c.z.toFixed(2)}) · ${b.face_count} faces` : "") + (b.material ? ` · ${b.material.name} ${b.material.density} g/cm³` : "");
      li.append(eye, name, stats);
      if (this.summary.kind !== "assembly") {
        const mat = document.createElement("select");
        mat.className = "pmaterial";
        mat.title = "Material (sets the density, so the part gets a mass)";
        const current = b.material ? MATERIALS.find((m) => m.name === b.material!.name && m.density === b.material!.density) : undefined;
        const options: [string, string][] = [["", "no material"], ...MATERIALS.map((m) => [m.name, `${m.name} (${m.density})`] as [string, string]), ["custom", "custom…"]];
        if (b.material && !current) options.splice(1, 0, [b.material.name, `${b.material.name} (${b.material.density})`]);
        for (const [value, label] of options) {
          const o = document.createElement("option");
          o.value = value;
          o.textContent = label;
          mat.appendChild(o);
        }
        mat.value = b.material ? b.material.name : "";
        mat.onchange = () => {
          const v = mat.value;
          if (v === "") this.apply({ type: "set_part_material", source: b.source, material: null });
          else if (v === "custom") {
            const answer = prompt("Material name and density in g/cm³, e.g. \"Oak 0.75\"", b.material ? `${b.material.name} ${b.material.density}` : "");
            if (!answer) {
              this.renderParts(target);
              return;
            }
            const parts = answer.trim().split(/\s+/);
            const density = Number(parts[parts.length - 1]);
            const mname = parts.slice(0, -1).join(" ") || "Custom";
            if (!Number.isFinite(density) || density <= 0) {
              this.setStatus("A material needs a positive density in g/cm³.");
              this.renderParts(target);
              return;
            }
            this.apply({ type: "set_part_material", source: b.source, material: { name: mname, density } });
          } else {
            const m = MATERIALS.find((x) => x.name === v) ?? (b.material && b.material.name === v ? b.material : undefined);
            if (m) this.apply({ type: "set_part_material", source: b.source, material: m });
          }
        };
        li.appendChild(mat);
      }
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
    else if (f.kind.type === "boolean") this.renderBooleanDetail(f, body);
    else if (f.kind.type === "shell") this.renderShellDetail(f, body);
    else if (f.kind.type === "move_face") this.renderMoveFaceDetail(f, body);
    else if (f.kind.type === "draft") this.renderDraftDetail(f, body);
    else if (f.kind.type === "mesh") this.renderMeshDetail(f, body);
    else if (f.kind.type === "split") this.renderSplitDetail(f, body);
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
      for (const [tool, label, key] of [["select", "Select", "S"], ["line", "Line", "L"], ["rectangle", "Rectangle", "R"], ["circle", "Circle", "C"], ["arc", "Arc", "A"], ["spline", "Spline", "B"], ["trim", "Trim", "T"], ["use", "Use", "U"]] as [Tool, string, string][]) {
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
        if (this.sketcher.filletCandidates()) {
          const fb = button("Fillet…", () => this.sketcher.filletSelection());
          fb.title = "Round the corner where the two selected lines meet with a tangent arc (I)";
          row.appendChild(fb);
        }
        const m = button(this.sketcher.awaitingMirrorAxis ? "Click a line…" : "Mirror…", () => this.sketcher.beginMirror());
        m.title = "Mirror the selection across a line you click next (M)";
        row.appendChild(m);
        row.appendChild(button("Delete selected", () => this.sketcher.deleteSelection(), "danger"));
        body.appendChild(row);
      }
    }

    const plane = kind.plane;
    this.planeFieldsFor(f, body, plane, (p) => this.apply({ type: "set_sketch_plane", id: f.id, plane: p }));

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
      "radius", "diameter", "equal", "parallel", "perpendicular", "angle", "point_on_line", "point_on_circle", "point_on_spline", "midpoint", "tangent",
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
    const preset = select(["custom", ...HOLE_PRESETS.map((p) => p.label)], HOLE_PRESETS.find((p) => p.diameter === k.diameter && (p.counterbore ? k.counterbore?.diameter === p.counterbore.diameter && k.counterbore?.depth === p.counterbore.depth : !k.counterbore))?.label ?? "custom", (v) => {
      const p = HOLE_PRESETS.find((x) => x.label === v);
      if (p) this.apply({ type: "set_hole", id: f.id, diameter: p.diameter, counterbore: p.counterbore ?? null });
    });
    preset.title = "ISO metric clearance holes, tapping drills and socket head cap screw counterbores";
    body.appendChild(field("Standard", preset));
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
    const planeValue = plane.type === "standard" ? plane.base : plane.type === "rotated" ? "angled" : "face";
    body.appendChild(
      field("Plane", select(["top", "front", "right", "angled", "face"], planeValue, (v) => {
        if (v === "face") this.beginFacePick((face) => onChange({ type: "face", face, offset: plane.offset }));
        else if (v === "angled") onChange({ type: "rotated", base: plane.type === "standard" ? plane.base : "top", axis: "x", angle: 30, offset: plane.offset });
        else onChange({ type: "standard", base: v as StandardPlane, offset: plane.offset });
      })),
    );
    if (plane.type === "rotated") {
      body.appendChild(field("Base plane", select(["top", "front", "right"], plane.base, (v) => onChange({ ...plane, base: v as StandardPlane }))));
      body.appendChild(field("About axis", select(["x", "y", "z"], plane.axis, (v) => onChange({ ...plane, axis: v as Axis }))));
      body.appendChild(field("Angle °", numberInput(plane.angle, (v) => onChange({ ...plane, angle: v }))));
    }
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
    if (plane.type === "rotated" && f.kind.type === "sketch") {
      // The angle takes expressions on sketches (the kernel exposes plane.angle there).
      const fields = [...body.querySelectorAll(".field")] as HTMLElement[];
      const angleRow = fields.find((el) => el.querySelector("label")?.textContent === "Angle °");
      angleRow?.replaceWith(field("Angle °", this.exprInput(f, "plane.angle", plane.angle, (v) => onChange({ ...plane, angle: v }))));
    }
  }

  renderShellDetail(f: FeatureSummary, body: HTMLElement): void {
    if (f.kind.type !== "shell") return;
    const k = f.kind;
    body.appendChild(field("Thickness", this.exprInput(f, "thickness", k.thickness, (v) => this.apply({ type: "set_shell", id: f.id, thickness: v }))));
    this.faceListFields(body, k.faces, "Add open face", "No open faces: every body becomes a closed hollow.", (faces) => this.apply({ type: "set_shell", id: f.id, faces }));
    const note = document.createElement("p");
    note.className = "note";
    note.textContent = "Walls keep this thickness inside every face; open faces are removed so the cavity is reachable. A face on a curved surface opens the whole surface.";
    body.appendChild(note);
  }

  renderMoveFaceDetail(f: FeatureSummary, body: HTMLElement): void {
    if (f.kind.type !== "move_face") return;
    const k = f.kind;
    body.appendChild(field("Distance", this.exprInput(f, "distance", k.distance, (v) => this.apply({ type: "set_move_face", id: f.id, distance: v }))));
    this.faceListFields(body, k.faces, "Add face", "No faces yet. Pick planar faces to move.", (faces) => this.apply({ type: "set_move_face", id: f.id, faces }));
    const note = document.createElement("p");
    note.className = "note";
    note.textContent = "Moves each face along its normal (negative pushes into the body); the faces around it stretch to follow.";
    body.appendChild(note);
  }

  renderSplitDetail(f: FeatureSummary, body: HTMLElement): void {
    if (f.kind.type !== "split") return;
    const k = f.kind;
    this.planeFieldsFor(f, body, k.plane, (plane) => this.apply({ type: "set_split", id: f.id, plane }));
    const chosen = k.bodies ?? [];
    const candidates = f.candidates ?? [];
    if (candidates.length > 1) {
      const ul = document.createElement("ul");
      ul.className = "body-pick";
      for (const [src, name] of candidates) {
        const li = document.createElement("li");
        const on = chosen.length === 0 || chosen.includes(src);
        const cb = checkbox(on, (v) => {
          const all = candidates.map(([s]) => s);
          const next = v ? [...new Set([...(chosen.length === 0 ? all : chosen), src])] : (chosen.length === 0 ? all : chosen).filter((s) => s !== src);
          this.apply({ type: "set_split", id: f.id, bodies: next.length === all.length ? [] : next });
        });
        const text = document.createElement("span");
        text.textContent = name;
        li.append(cb, text);
        ul.appendChild(li);
      }
      body.appendChild(field("Bodies", ul));
    }
    const note = document.createElement("p");
    note.className = "note";
    note.textContent = "Each body the plane crosses becomes two: the part against the plane's normal keeps the body's name, the other becomes a new part.";
    body.appendChild(note);
  }

  renderMeshDetail(f: FeatureSummary, body: HTMLElement): void {
    if (f.kind.type !== "mesh") return;
    const note = document.createElement("p");
    note.className = "note";
    note.textContent = `Imported mesh: ${f.kind.vertices.length} vertices, ${f.kind.triangles.length} triangles. Coplanar triangles are merged into faces, so later features can reference them like any body.`;
    body.appendChild(note);
  }

  renderDraftDetail(f: FeatureSummary, body: HTMLElement): void {
    if (f.kind.type !== "draft") return;
    const k = f.kind;
    body.appendChild(field("Angle °", this.exprInput(f, "angle", k.angle, (v) => this.apply({ type: "set_draft", id: f.id, angle: v }))));
    this.planeFieldsFor(f, body, k.neutral, (neutral) => this.apply({ type: "set_draft", id: f.id, neutral }));
    this.faceListFields(body, k.faces, "Add face", "No faces yet. Pick the planar faces to tilt.", (faces) => this.apply({ type: "set_draft", id: f.id, faces }));
    const note = document.createElement("p");
    note.className = "note";
    note.textContent = "Each face tilts about the line where it meets the neutral plane, so the body tapers towards the plane's normal (the pull direction).";
    body.appendChild(note);
  }

  /** A pick-to-add list of face references with per-face removal and a Clear button. */
  private faceListFields(body: HTMLElement, faces: FaceRef[], addLabel: string, emptyNote: string, setFaces: (faces: FaceRef[]) => void): void {
    const picking = this.facePicker !== null;
    const row = document.createElement("div");
    row.className = "row";
    row.appendChild(button(picking ? "Click a face…" : addLabel, () => {
      if (picking) return;
      this.beginFacePick((face) => {
        if (faces.some((x) => x.feature === face.feature && x.local === face.local)) return;
        setFaces([...faces, face]);
      });
      this.renderDetail();
    }, picking ? "primary" : ""));
    if (faces.length > 0) row.appendChild(button("Clear", () => setFaces([])));
    body.appendChild(row);
    const ul = document.createElement("ul");
    ul.className = "edge-list";
    for (const face of faces) {
      const li = document.createElement("li");
      const label = document.createElement("span");
      label.textContent = this.describeFace(face);
      li.appendChild(label);
      li.appendChild(button("×", () => setFaces(faces.filter((x) => x !== face)), "danger"));
      ul.appendChild(li);
    }
    if (faces.length === 0) {
      const li = document.createElement("li");
      li.className = "note";
      li.textContent = emptyNote;
      ul.appendChild(li);
    }
    body.appendChild(ul);
  }

  renderBooleanDetail(f: FeatureSummary, body: HTMLElement): void {
    if (f.kind.type !== "boolean") return;
    const k = f.kind;
    body.appendChild(field("Operation", select(["union", "subtract", "intersect"], k.op, (v) => this.apply({ type: "set_boolean", id: f.id, op: v as BooleanOp }))));
    // Bodies that existed before this feature, plus any referenced ones that no longer do.
    const candidates: [number, string][] = [...(f.candidates ?? [])];
    for (const src of [...k.targets, ...k.tools]) {
      if (!candidates.some(([s]) => s === src)) candidates.push([src, `${this.feature(src)?.name ?? `feature ${src}`} (missing)`]);
    }
    const pickList = (label: string, chosen: number[], other: number[], set: (ids: number[]) => void) => {
      const ul = document.createElement("ul");
      ul.className = "body-pick";
      for (const [src, name] of candidates) {
        const li = document.createElement("li");
        const cb = checkbox(chosen.includes(src), (on) => set(on ? [...chosen, src] : chosen.filter((s) => s !== src)));
        cb.disabled = other.includes(src);
        const text = document.createElement("span");
        text.textContent = name;
        li.append(cb, text);
        ul.appendChild(li);
      }
      body.appendChild(field(label, ul));
    };
    pickList("Targets", k.targets, k.tools, (targets) => this.apply({ type: "set_boolean", id: f.id, targets }));
    pickList("Tools", k.tools, k.targets, (tools) => this.apply({ type: "set_boolean", id: f.id, tools }));
    body.appendChild(field("Keep tools", checkbox(k.keep_tools, (v) => this.apply({ type: "set_boolean", id: f.id, keep_tools: v }))));
    const note = document.createElement("p");
    note.className = "note";
    note.textContent = k.op === "union" ? "Merges the targets and tools into one body." : k.op === "subtract" ? "Removes every tool from each target." : "Keeps only where each target overlaps every tool.";
    body.appendChild(note);
  }

  /** "Copies" mode plus the list of solid features a pattern or mirror may replay instead of whole bodies. */
  private copyScopeFields(f: FeatureSummary, body: HTMLElement, chosen: number[], op: CopyOp, setFeatures: (ids: number[]) => void, setOp: (op: CopyOp) => void, chosenBodies: number[] = [], setBodies?: (ids: number[]) => void): void {
    const copies = select(["add", "new"], op, (v) => setOp(v as CopyOp));
    copies.disabled = chosen.length > 0;
    body.appendChild(field("Copies", copies));
    // Whole-body copies can be limited to some of the bodies that exist before this feature.
    if (chosen.length === 0 && setBodies && (f.candidates ?? []).length > 1) {
      const ul = document.createElement("ul");
      ul.className = "body-pick";
      for (const [src, name] of f.candidates!) {
        const li = document.createElement("li");
        const cb = checkbox(chosenBodies.length === 0 || chosenBodies.includes(src), (on) => {
          const all = f.candidates!.map(([s]) => s);
          const current = chosenBodies.length === 0 ? all : chosenBodies;
          const next = on ? [...current, src] : current.filter((s) => s !== src);
          setBodies(next.length === all.length ? [] : next);
        });
        const text = document.createElement("span");
        text.textContent = name;
        li.append(cb, text);
        ul.appendChild(li);
      }
      body.appendChild(field("Bodies", ul));
    }
    const pos = this.summary.features.findIndex((g) => g.id === f.id);
    const solidKinds = new Set(["extrude", "revolve", "hole", "sweep", "loft"]);
    const candidates = this.summary.features.filter((g, i) => i < pos && solidKinds.has(g.kind.type) && !g.suppressed);
    if (candidates.length === 0) return;
    const ul = document.createElement("ul");
    ul.className = "body-pick";
    for (const g of candidates) {
      const li = document.createElement("li");
      const cb = checkbox(chosen.includes(g.id), (on) => setFeatures(on ? [...chosen, g.id] : chosen.filter((id) => id !== g.id)));
      const text = document.createElement("span");
      text.textContent = g.name;
      li.append(cb, text);
      ul.appendChild(li);
    }
    body.appendChild(field("Features", ul));
  }

  renderMirrorDetail(f: FeatureSummary, body: HTMLElement): void {
    if (f.kind.type !== "mirror") return;
    const k = f.kind;
    this.planeFieldsFor(f, body, k.plane, (plane) => this.apply({ type: "set_mirror", id: f.id, plane }));
    this.copyScopeFields(f, body, k.features ?? [], k.op, (features) => this.apply({ type: "set_mirror", id: f.id, features }), (op) => this.apply({ type: "set_mirror", id: f.id, op }), k.bodies ?? [], (bodies) => this.apply({ type: "set_mirror", id: f.id, bodies }));
    const note = document.createElement("p");
    note.className = "note";
    note.textContent = "Mirrors across the plane. With no features ticked every body is copied: “Add” unions each copy with its original, “New” keeps copies separate. Ticked features are replayed mirrored with their own add / remove operation.";
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
    this.copyScopeFields(f, body, k.features ?? [], k.op, (features) => this.apply({ type: "set_pattern", id: f.id, features }), (op) => this.apply({ type: "set_pattern", id: f.id, op }), k.bodies ?? [], (bodies) => this.apply({ type: "set_pattern", id: f.id, bodies }));
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

/** Common materials with densities in g/cm³. */
const MATERIALS: { name: string; density: number }[] = [
  { name: "Steel", density: 7.85 },
  { name: "Stainless steel", density: 8.0 },
  { name: "Aluminium", density: 2.7 },
  { name: "Brass", density: 8.5 },
  { name: "Copper", density: 8.96 },
  { name: "Titanium", density: 4.43 },
  { name: "ABS", density: 1.04 },
  { name: "PLA", density: 1.24 },
  { name: "Nylon", density: 1.15 },
  { name: "Acrylic", density: 1.18 },
  { name: "Plywood", density: 0.6 },
];

/** Standard hole sizes (ISO metric): clearance, tapping drill, and counterbored clearance for socket head cap screws. */
const HOLE_PRESETS: { label: string; diameter: number; counterbore?: { diameter: number; depth: number } }[] = [
  ...[3, 4, 5, 6, 8, 10, 12].map((m) => ({ label: `M${m} clearance`, diameter: { 3: 3.4, 4: 4.5, 5: 5.5, 6: 6.6, 8: 9, 10: 11, 12: 13.5 }[m]! })),
  ...[3, 4, 5, 6, 8, 10, 12].map((m) => ({ label: `M${m} tap drill`, diameter: { 3: 2.5, 4: 3.3, 5: 4.2, 6: 5, 8: 6.8, 10: 8.5, 12: 10.2 }[m]! })),
  ...[3, 4, 5, 6, 8, 10, 12].map((m) => ({
    label: `M${m} socket head c'bore`,
    diameter: { 3: 3.4, 4: 4.5, 5: 5.5, 6: 6.6, 8: 9, 10: 11, 12: 13.5 }[m]!,
    counterbore: { diameter: { 3: 6.5, 4: 8, 5: 10, 6: 11, 8: 15, 10: 18, 12: 20 }[m]!, depth: { 3: 3.4, 4: 4.4, 5: 5.4, 6: 6.5, 8: 8.6, 10: 10.6, 12: 12.7 }[m]! },
  })),
];

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
    case "point_on_spline": return ["point", "spline"];
    case "tangent": return ["line", "entity"];
    case "symmetric": return ["a", "b", "line"];
    case "rotated": return ["a", "b", "center"];
    default: return [];
  }
}

function constraintHasValue(t: string): boolean {
  return ["distance", "horizontal_distance", "vertical_distance", "length", "radius", "diameter", "angle", "rotated"].includes(t);
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
      if (pendingInvite) {
        await acceptPendingInvite();
        return;
      }
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
    let myTeams: Team[] = [];
    if (user) {
      try {
        myTeams = await Sync.listTeams();
      } catch {
        myTeams = [];
      }
    }
    renderTeams(myTeams);
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
      const viewers = d.viewers ?? [];
      const teamShares = d.teams ?? [];
      owner.textContent = (d.parent ? `branch of ${d.parent.doc_name}${d.parent.version_name ? ` at ${d.parent.version_name}` : ""} · ` : "") + (d.owner
        ? (mine ? "yours" : `by ${d.owner.name}`) +
          (shared.length ? ` · shared with ${shared.map((c) => c.name).join(", ")}` : "") +
          (viewers.length ? ` · read-only: ${viewers.map((c) => c.name).join(", ")}` : "") +
          (teamShares.length ? ` · teams: ${teamShares.map((t) => `${t.name}${t.role === "viewer" ? " (read-only)" : ""}`).join(", ")}` : "")
        : "open to all");
      const when = document.createElement("span");
      when.className = "dwhen";
      when.textContent = new Date(d.updated * 1000).toLocaleString();
      li.append(name, owner, when);
      if (mine) {
        const share = button("Share…", async () => {
          const who = prompt(`Share "${d.name}" with which account name? Add " viewer" for read-only access.`);
          if (!who) return;
          const parts = who.trim().split(/\s+/);
          const role = parts.length > 1 && parts[parts.length - 1]!.toLowerCase() === "viewer" ? "viewer" : "editor";
          const name = role === "viewer" ? parts.slice(0, -1).join(" ") : who.trim();
          try {
            await Sync.shareDoc(d.id, name, role);
            await renderDocs();
          } catch (e) {
            note.textContent = `Could not share: ${(e as Error).message}`;
          }
        });
        share.className = "dshare";
        li.appendChild(share);
        const invite = button("Invite link…", async () => {
          const answer = prompt(`Invite link for "${d.name}": anyone signed in who opens it joins as an editor or a viewer. Which role?`, "editor");
          if (!answer) return;
          const role = answer.trim().toLowerCase().startsWith("v") ? "viewer" : "editor";
          try {
            const inv = await Sync.createInvite(d.id, role);
            const url = Sync.inviteUrl(d.id, inv.token);
            await renderDocs();
            let copied = "";
            try {
              await navigator.clipboard.writeText(url);
              copied = " (copied)";
            } catch {
              // No clipboard access: the link is on screen.
            }
            note.textContent = `Invite link (${role}): ${url}${copied}`;
          } catch (e) {
            note.textContent = `Could not create the link: ${(e as Error).message}`;
          }
        });
        invite.className = "dshare";
        li.appendChild(invite);
        for (const inv of d.invites ?? []) {
          const url = Sync.inviteUrl(d.id, inv.token);
          const un = button(`− link (${inv.role})`, async () => {
            if (!confirm(`Withdraw this ${inv.role} invitation link? Accounts that already joined keep their access.`)) return;
            await Sync.revokeInvite(d.id, inv.token);
            await renderDocs();
          });
          un.className = "dshare";
          un.title = url;
          li.appendChild(un);
        }
        if (myTeams.length > 0) {
          const st = button("Share with team…", async () => {
            const answer = prompt(`Share "${d.name}" with which team? (${myTeams.map((t) => t.name).join(", ")}) Add " viewer" for read-only access.`, myTeams[0]!.name);
            if (!answer) return;
            const parts = answer.trim().split(/\s+/);
            const role = parts.length > 1 && parts[parts.length - 1]!.toLowerCase() === "viewer" ? "viewer" : "editor";
            const tname = (role === "viewer" ? parts.slice(0, -1).join(" ") : answer.trim()).toLowerCase();
            const team = myTeams.find((t) => t.name.toLowerCase() === tname);
            if (!team) {
              note.textContent = `No team called "${tname}".`;
              return;
            }
            try {
              await Sync.shareDocWithTeam(d.id, team.id, role);
              await renderDocs();
            } catch (e) {
              note.textContent = `Could not share: ${(e as Error).message}`;
            }
          });
          st.className = "dshare";
          li.appendChild(st);
        }
        for (const c of [...shared, ...viewers]) {
          const un = button(`− ${c.name}`, async () => {
            await Sync.unshareDoc(d.id, c.id);
            await renderDocs();
          });
          un.className = "dshare";
          un.title = `Stop sharing with ${c.name}`;
          li.appendChild(un);
        }
        for (const t of teamShares) {
          const un = button(`− team ${t.name}`, async () => {
            await Sync.unshareTeam(d.id, t.id);
            await renderDocs();
          });
          un.className = "dshare";
          un.title = `Stop sharing with the team ${t.name}`;
          li.appendChild(un);
        }
      }
      const branch = button("Branch", async () => {
        const bname = prompt(`Name for the new document branched from "${d.name}"`, `${d.name} (branch)`);
        if (!bname) return;
        try {
          const meta = await Sync.branchDoc(d.id, bname);
          dialog.close();
          openDoc(meta.id);
        } catch (e) {
          note.textContent = `Could not branch: ${(e as Error).message}`;
        }
      });
      branch.className = "dshare";
      branch.title = "Copy this document into a new one of your own, as it is now";
      li.appendChild(branch);
      if (d.parent) {
        // Merges between a branch and its origin: plan first, then confirm.
        const merge = (target: string, from: string, what: string) => async () => {
          try {
            const plan = await Sync.mergeDoc(target, from, true);
            const summary = `${plan.changes} change${plan.changes === 1 ? "" : "s"}` + (plan.conflicts.length ? `, ${plan.conflicts.length} left alone:\n${plan.conflicts.join("\n")}` : "");
            if (plan.changes === 0) {
              note.textContent = `Nothing to merge ${what}: ${summary}.`;
              return;
            }
            if (!confirm(`Merge ${what}? ${summary}`)) return;
            const done = await Sync.mergeDoc(target, from);
            const message = `Merged ${done.changes} change${done.changes === 1 ? "" : "s"} ${what}` + (done.conflicts.length ? `; left alone: ${done.conflicts.join("; ")}` : ".");
            await renderDocs();
            note.textContent = message;
          } catch (e) {
            note.textContent = `Could not merge: ${(e as Error).message}`;
          }
        };
        const up = button("Merge into origin", merge(d.parent.doc, d.id, `from "${d.name}" into "${d.parent.doc_name}"`));
        up.className = "dshare";
        up.title = `Apply this branch's changes to ${d.parent.doc_name}`;
        const down = button("Pull origin", merge(d.id, d.parent.doc, `from "${d.parent.doc_name}" into "${d.name}"`));
        down.className = "dshare";
        down.title = `Bring ${d.parent.doc_name}'s later changes into this branch`;
        li.append(up, down);
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
  /** The Teams section of the Docs dialog: the signed-in account's teams with their members. */
  const renderTeams = (teams: Team[]) => {
    const box = $("#teams");
    const list = $("#teams-list");
    list.innerHTML = "";
    box.hidden = !user;
    if (!user) return;
    if (teams.length === 0) {
      const li = document.createElement("li");
      li.textContent = "No teams yet.";
      list.appendChild(li);
    }
    for (const t of teams) {
      const li = document.createElement("li");
      const name = document.createElement("span");
      name.className = "dname";
      name.textContent = t.name;
      const mineTeam = t.owner.id === user!.id;
      const who = document.createElement("span");
      who.className = "downer";
      who.textContent = (mineTeam ? "yours" : `by ${t.owner.name}`) + (t.members.length ? ` · ${t.members.map((m) => m.name).join(", ")}` : " · no members yet");
      li.append(name, who);
      if (mineTeam) {
        const add = button("+ member…", async () => {
          const n = prompt(`Add which account to "${t.name}"?`);
          if (!n) return;
          try {
            await Sync.addTeamMember(t.id, n.trim());
            await renderDocs();
          } catch (e) {
            $("#docs-note").textContent = `Could not add: ${(e as Error).message}`;
          }
        });
        add.className = "dshare";
        li.appendChild(add);
        for (const m of t.members) {
          const un = button(`− ${m.name}`, async () => {
            await Sync.removeTeamMember(t.id, m.id);
            await renderDocs();
          });
          un.className = "dshare";
          li.appendChild(un);
        }
        li.appendChild(button("×", async () => {
          if (!confirm(`Delete the team "${t.name}"? Documents shared with it stay with their owners.`)) return;
          await Sync.deleteTeam(t.id);
          await renderDocs();
        }, "danger"));
      }
      list.appendChild(li);
    }
  };
  $("#teams-create").onclick = async () => {
    const n = prompt("Team name");
    if (!n) return;
    try {
      await Sync.createTeam(n.trim());
      await renderDocs();
    } catch (e) {
      $("#docs-note").textContent = `Could not create the team: ${(e as Error).message}`;
    }
  };

  /** Lists the features that differ between a saved version and the document now, per tab. */
  const compareWithVersion = (versionJson: string): string[] => {
    const then = Kernel.fromJson(versionJson);
    const now = Kernel.fromJson(app.kernel.toJson());
    const thenTabs = then.regenerate(null).tabs;
    const nowTabs = now.regenerate(null).tabs;
    const lines: string[] = [];
    const key = (f: FeatureSummary) => JSON.stringify({ name: f.name, suppressed: f.suppressed, kind: f.kind, bindings: f.bindings });
    const seen = new Set<number>();
    for (const tab of [...nowTabs, ...thenTabs]) {
      if (seen.has(tab.id)) continue;
      seen.add(tab.id);
      const inNow = nowTabs.some((t) => t.id === tab.id), inThen = thenTabs.some((t) => t.id === tab.id);
      if (!inThen) { lines.push(`+ tab ${tab.name} (new)`); continue; }
      if (!inNow) { lines.push(`− tab ${tab.name} (removed)`); continue; }
      const before = new Map(then.regenerate(tab.id).features.map((f) => [f.id, f] as const));
      const after = new Map(now.regenerate(tab.id).features.map((f) => [f.id, f] as const));
      for (const [id, f] of after) {
        const b = before.get(id);
        if (!b) lines.push(`+ ${tab.name}: ${f.name} added`);
        else if (key(b) !== key(f)) lines.push(`~ ${tab.name}: ${f.name} changed${b.name !== f.name ? ` (was ${b.name})` : ""}`);
      }
      for (const [id, f] of before) if (!after.has(id)) lines.push(`− ${tab.name}: ${f.name} removed`);
    }
    return lines;
  };
  const showDiff = (v: VersionMeta, lines: string[]) => {
    const list = $("#versions-diff");
    list.innerHTML = "";
    list.hidden = false;
    const head = document.createElement("li");
    head.textContent = lines.length === 0 ? `No changes since version "${v.name}".` : `Since version "${v.name}": ${lines.length} change${lines.length === 1 ? "" : "s"}`;
    list.appendChild(head);
    for (const line of lines) {
      const li = document.createElement("li");
      li.className = line.startsWith("+") ? "added" : line.startsWith("−") ? "removed" : "changed";
      li.textContent = line;
      list.appendChild(li);
    }
  };
  const renderVersions = async () => {
    ($("#versions-diff") as HTMLElement).hidden = true;
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
      const branchV = button("Branch", async () => {
        const bname = prompt(`Name for the new document branched from version "${v.name}"`, `${app.summary.name} (${v.name})`);
        if (!bname) return;
        try {
          const meta = await Sync.branchDoc(id, bname, v.id);
          dialog.close();
          openDoc(meta.id);
        } catch (e) {
          showDiff(v, [`Could not branch: ${(e as Error).message}`]);
        }
      });
      branchV.className = "dshare";
      branchV.title = "Copy this version into a new document of your own";
      const compare = button("Compare", async () => {
        try {
          showDiff(v, compareWithVersion(await Sync.getVersion(id, v.id)));
        } catch (e) {
          showDiff(v, [`Could not compare: ${(e as Error).message}`]);
        }
      });
      compare.className = "dshare";
      compare.title = "What changed since this version";
      li.append(name, when, branchV, compare);
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
    url.searchParams.delete("invite");
    history.replaceState(null, "", url.toString());
  };
  let pendingInvite: { doc: string; token: string } | null = null;
  const acceptPendingInvite = async () => {
    const inv = pendingInvite;
    if (!inv) return;
    pendingInvite = null;
    try {
      const meta = await Sync.acceptInvite(inv.doc, inv.token);
      app.setStatus(`Joined "${meta.name}" through an invitation link.`);
      openDoc(inv.doc);
    } catch (e) {
      app.setStatus(`Invitation not accepted: ${(e as Error).message}`);
      const url = new URL(location.href);
      url.searchParams.delete("invite");
      url.searchParams.delete("doc");
      history.replaceState(null, "", url.toString());
    }
  };
  $("#btn-docs").onclick = async () => {
    await renderDocs();
    dialog.showModal();
  };
  const helpDialog = $("#help-dialog") as HTMLDialogElement;
  $("#btn-help").onclick = () => helpDialog.showModal();
  $("#help-close").onclick = () => helpDialog.close();
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
  const startUrl = new URL(location.href);
  const docParam = startUrl.searchParams.get("doc");
  const inviteParam = startUrl.searchParams.get("invite");
  if (docParam && inviteParam) {
    // An invitation link: accept it once signed in, then open the document.
    pendingInvite = { doc: docParam, token: inviteParam };
    if (user) await acceptPendingInvite();
    else {
      $("#account-note").textContent = "Sign in or register to accept the invitation.";
      accountDialog.showModal();
    }
  } else if (docParam) openDoc(docParam);
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
  $("#btn-move-instance").onclick = () => (app.instanceDrag ? app.endInstanceDrag() : app.beginInstanceDrag());
  ($("#explode") as HTMLInputElement).oninput = (e) => app.viewer.setExplode((Number((e.target as HTMLInputElement).value) / 100) * 1.5);
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
  const drawingDialog = $("#drawing-dialog") as HTMLDialogElement;
  const renderDrawingPreview = () => {
    const views = ["front", "top", "right", "iso", "section", "detail"].filter((n) => ($(`#dv-${n}`) as HTMLInputElement).checked);
    ($("#dv-detail-note") as HTMLElement).hidden = !!app.selectedFace;
    app.drawingOptions = { views: new Set(views), sheet: ($("#dv-sheet") as HTMLSelectElement).value as SheetSize };
    $("#drawing-preview").innerHTML = app.toDrawingSvg();
    ($("#dv-clear-dims") as HTMLButtonElement).disabled = app.drawingDimensions().length === 0;
  };
  // Placing dimensions: two clicks on line endpoints of one view in the preview.
  let dimensionMode = false;
  let dimensionPick: { view: string; p: Vec2 } | null = null;
  const setDimensionMode = (on: boolean) => {
    dimensionMode = on;
    dimensionPick = null;
    $("#dv-dimension").classList.toggle("active", on);
    $("#dv-dim-note").textContent = on ? "Click two line endpoints of one view." : "";
    $("#drawing-preview").classList.toggle("picking", on);
  };
  const openDrawingDialog = () => {
    setDimensionMode(false);
    renderDrawingPreview();
    drawingDialog.showModal();
  };
  $("#dv-dimension").onclick = () => setDimensionMode(!dimensionMode);
  $("#dv-clear-dims").onclick = () => {
    app.clearDrawingDimensions();
    renderDrawingPreview();
  };
  $("#drawing-preview").onclick = (e) => {
    if (!dimensionMode) return;
    const svg = $("#drawing-preview").querySelector("svg") as SVGSVGElement | null;
    const ctm = svg?.getScreenCTM();
    if (!svg || !ctm) return;
    const pt = new DOMPoint(e.clientX, e.clientY).matrixTransform(ctm.inverse());
    const frame = app.drawingFrame();
    const views = app.chosenDrawingViews();
    // The clicked point in each view's own coordinates; snap to the nearest endpoint within 3 sheet mm.
    let best: { view: string; p: Vec2; d: number } | null = null;
    for (const f of frame.views) {
      const v = views.find((x) => x.name === f.name);
      if (!v) continue;
      const local = { x: (pt.x - frame.ox) / frame.scale - f.dx, y: (frame.oy - pt.y) / frame.scale - f.dy };
      const snap = snapDrawingPoint(v, local, 3 / frame.scale);
      if (!snap) continue;
      const d = Math.hypot(snap.x - local.x, snap.y - local.y);
      if (!best || d < best.d) best = { view: f.name, p: snap, d };
    }
    if (!best) {
      $("#dv-dim-note").textContent = "No line endpoint there; click closer to a corner.";
      return;
    }
    if (!dimensionPick || dimensionPick.view !== best.view) {
      dimensionPick = { view: best.view, p: best.p };
      const f = frame.views.find((x) => x.name === best!.view)!;
      const mark = document.createElementNS("http://www.w3.org/2000/svg", "circle");
      mark.setAttribute("class", "pick");
      mark.setAttribute("cx", String(frame.ox + (best.p.x + f.dx) * frame.scale));
      mark.setAttribute("cy", String(frame.oy - (best.p.y + f.dy) * frame.scale));
      mark.setAttribute("r", "1.5");
      mark.setAttribute("fill", "none");
      mark.setAttribute("stroke", "#e33");
      mark.setAttribute("stroke-width", "0.4");
      svg.appendChild(mark);
      $("#dv-dim-note").textContent = `First point on the ${best.view} view; now click the second.`;
      return;
    }
    if (Math.hypot(best.p.x - dimensionPick.p.x, best.p.y - dimensionPick.p.y) < 1e-9) return;
    const value = app.addDrawingDimension(best.view, dimensionPick.p, best.p);
    dimensionPick = null;
    renderDrawingPreview();
    $("#dv-dim-note").textContent = `Dimension ${value.toFixed(2).replace(/\.?0+$/, "")} mm placed on the ${best.view} view. Click two more points, or Add dimension again to stop.`;
  };
  for (const n of ["front", "top", "right", "iso", "section", "detail"]) ($(`#dv-${n}`) as HTMLInputElement).onchange = renderDrawingPreview;
  ($("#dv-sheet") as HTMLSelectElement).onchange = renderDrawingPreview;
  $("#drawing-svg").onclick = () => app.download(new Blob([app.toDrawingSvg()], { type: "image/svg+xml" }), "svg", `${app.summary.name}-drawing`);
  $("#drawing-dxf").onclick = () => app.download(new Blob([app.toDrawingDxf()], { type: "application/dxf" }), "dxf", `${app.summary.name}-drawing`);
  $("#drawing-close").onclick = () => {
    setDimensionMode(false);
    drawingDialog.close();
  };
  const exportSelect = $("#export") as HTMLSelectElement;
  exportSelect.onchange = () => {
    const what = exportSelect.value;
    exportSelect.value = "";
    if (what === "png") app.viewer.snapshot().then((blob) => app.download(blob, "png"), (e) => app.setStatus(`error: ${(e as Error).message}`));
    else if (what === "stl") app.download(app.toStl(), "stl");
    else if (what === "3mf") app.download(app.to3mf(), "3mf");
    else if (what === "svg-drawing") app.download(new Blob([app.toDrawingSvg()], { type: "image/svg+xml" }), "svg", `${app.summary.name}-drawing`);
    else if (what === "dxf-drawing") app.download(new Blob([app.toDrawingDxf()], { type: "application/dxf" }), "dxf", `${app.summary.name}-drawing`);
    else if (what === "bom") app.download(new Blob([app.toBom()], { type: "text/csv" }), "csv", `${app.summary.name}-bom`);
    else if (what === "drawing") openDrawingDialog();
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
  const stlInput = $("#stl-input") as HTMLInputElement;
  $("#btn-import").onclick = () => {
    if (app.summary.kind !== "part_studio") {
      app.setStatus("Switch to a part studio tab to import a mesh.");
      return;
    }
    stlInput.click();
  };
  stlInput.onchange = async () => {
    const file = stlInput.files?.[0];
    stlInput.value = "";
    if (!file) return;
    try {
      const mesh = /\.obj$/i.test(file.name) ? parseObj(await file.text()) : parseStl(await file.arrayBuffer());
      app.apply({ type: "add_mesh", vertices: mesh.vertices, triangles: mesh.triangles, name: file.name.replace(/\.(stl|obj)$/i, "") || null });
    } catch (e) {
      app.setStatus(`could not import ${file.name}: ${(e as Error).message}`);
    }
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
  $("#btn-add-split").onclick = () => {
    app.sketcher.exit();
    // Default: a right plane through the middle of the bodies.
    const b = app.viewer.bodyBounds();
    const offset = b ? (b.min.x + b.max.x) / 2 : 0;
    app.apply({ type: "add_split", plane: { type: "standard", base: "right", offset }, name: app.autoName("Split") });
    app.select(app.summary.features[app.summary.features.length - 1]?.id ?? null);
  };
  $("#btn-add-boolean").onclick = () => {
    app.sketcher.exit();
    // Default to the two most recent bodies: the newest is the tool.
    const bodies = app.summary.bodies;
    const tool = bodies[bodies.length - 1]?.source;
    const target = bodies.find((b) => b.source !== tool)?.source;
    app.apply({ type: "add_boolean", op: "subtract", targets: target === undefined ? [] : [target], tools: tool === undefined ? [] : [tool], name: app.autoName("Boolean") });
    app.select(app.summary.features[app.summary.features.length - 1]?.id ?? null);
  };
  $("#btn-add-shell").onclick = () => {
    app.sketcher.exit();
    // A face selected in the viewport becomes the first open face.
    const faces = app.selectedFace ? [app.selectedFace] : [];
    app.apply({ type: "add_shell", thickness: 2, faces, name: app.autoName("Shell") });
    app.select(app.summary.features[app.summary.features.length - 1]?.id ?? null);
  };
  $("#btn-add-move-face").onclick = () => {
    app.sketcher.exit();
    const faces = app.selectedFace ? [app.selectedFace] : [];
    app.apply({ type: "add_move_face", faces, distance: 5, name: app.autoName("Move face") });
    app.select(app.summary.features[app.summary.features.length - 1]?.id ?? null);
  };
  $("#btn-add-draft").onclick = () => {
    app.sketcher.exit();
    const faces = app.selectedFace ? [app.selectedFace] : [];
    app.apply({ type: "add_draft", faces, neutral: { type: "standard", base: "top", offset: 0 }, angle: 5, name: app.autoName("Draft") });
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
    if (e.key === "?") {
      const d = $("#help-dialog") as HTMLDialogElement;
      if (d.open) d.close();
      else d.showModal();
      return;
    }
    if (!app.sketcher.active) {
      const view = ({ "1": "top", "2": "front", "3": "right", "0": "iso" } as Record<string, "top" | "front" | "right" | "iso">)[e.key];
      if (view) app.viewer.setStandardView(view);
    }
    if (e.key === "Escape") {
      if (app.mateAnimation) {
        app.stopMateAnimation();
        app.renderDetail();
      } else if (app.measure) app.endMeasure();
      else if (app.instanceDrag) app.endInstanceDrag();
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
      const tool = ({ s: "select", l: "line", r: "rectangle", c: "circle", a: "arc", b: "spline", p: "polygon", n: "slot", t: "trim", u: "use" } as Record<string, Tool>)[e.key.toLowerCase()];
      if (tool) app.sketcher.setTool(tool);
      if (e.key === "Enter") app.sketcher.finish();
      if (e.key.toLowerCase() === "o") app.sketcher.offsetSelection();
      if (e.key.toLowerCase() === "i") app.sketcher.filletSelection();
      if (e.key.toLowerCase() === "m") app.sketcher.beginMirror();
      if (e.key.toLowerCase() === "y") app.sketcher.patternSelection();
      if (e.key.toLowerCase() === "q") app.sketcher.toggleConstruction();
      if (e.key === "Delete" || e.key === "Backspace") app.sketcher.deleteSelection();
    }
  });
}

main().catch((e) => {
  $("#status-text").textContent = `failed to start: ${(e as Error).message}`;
  console.error(e);
});
