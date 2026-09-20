// Interactive sketch mode: drawing tools, snapping, dragging, selection.
//
// Everything here turns pointer input into ops (`add_line`, `add_constraint`,
// `move_point`, ...); the kernel solves and the viewer redraws from the regen
// summary. Snapping to an existing point adds a coincident constraint, and
// nearly horizontal/vertical lines get the matching constraint, which is how
// most CAD sketchers infer intent while drawing.

import type { Constraint, Entity, Op, OpResult, PlaneFrame, SketchCurve, SketchData, SketchOp, Summary, Vec2, Vec3 } from "./kernel";
import type { PointerHandler, Viewer } from "./viewer";

export type Tool = "select" | "line" | "rectangle" | "circle" | "arc" | "spline" | "polygon" | "slot" | "trim" | "use";

/** Catmull–Rom spline through `pts` (matches the kernel's `spline_polyline`), for previews. */
export function splinePolyline(pts: Vec2[], pieces: number): Vec2[] {
  if (pts.length < 2) return pts.slice();
  const n = pts.length;
  const tangent = (i: number): Vec2 => {
    if (i === 0) return { x: pts[1]!.x - pts[0]!.x, y: pts[1]!.y - pts[0]!.y };
    if (i === n - 1) return { x: pts[n - 1]!.x - pts[n - 2]!.x, y: pts[n - 1]!.y - pts[n - 2]!.y };
    return { x: (pts[i + 1]!.x - pts[i - 1]!.x) / 2, y: (pts[i + 1]!.y - pts[i - 1]!.y) / 2 };
  };
  const out: Vec2[] = [];
  for (let i = 0; i < n - 1; i++) {
    const p0 = pts[i]!, p1 = pts[i + 1]!, m0 = tangent(i), m1 = tangent(i + 1);
    for (let k = 0; k < pieces; k++) {
      const t = k / pieces, t2 = t * t, t3 = t2 * t;
      const h00 = 2 * t3 - 3 * t2 + 1, h10 = t3 - 2 * t2 + t, h01 = -2 * t3 + 3 * t2, h11 = t3 - t2;
      out.push({ x: p0.x * h00 + m0.x * h10 + p1.x * h01 + m1.x * h11, y: p0.y * h00 + m0.y * h10 + p1.y * h01 + m1.y * h11 });
    }
  }
  out.push(pts[n - 1]!);
  return out;
}

export interface SketchHost {
  summary: Summary;
  viewer: Viewer;
  /** Applies an op without regenerating; throws on failure. */
  applyRaw(op: Op): OpResult;
  regenerate(): void;
  /** Re-renders the panels after the sketch selection changed. */
  selectionChanged(): void;
  setStatus(text: string): void;
  /** Records an undo point before a user-level edit. */
  snapshot(): void;
  /** Projects the body edge or face under the pointer into the sketch ("Use" tool). */
  projectAt(e: PointerEvent): void;
}

const SNAP_PX = 10;
const CURVE_PX = 6;
const INFER_TAN = Math.tan((5 * Math.PI) / 180);

export class Sketcher implements PointerHandler {
  sketchId: number | null = null;
  tool: Tool = "select";
  selection = new Set<number>();
  private pending: Vec2[] = [];
  private pendingIds: (number | null)[] = [];
  private drag: { entity: number; moved: boolean } | null = null;
  private regenQueued = false;
  /** Set while "Mirror" waits for the user to click the axis line. */
  awaitingMirrorAxis = false;

  constructor(private host: SketchHost) {}

  get active(): boolean {
    return this.sketchId !== null;
  }

  enter(sketchId: number): void {
    if (this.sketchId === sketchId) return;
    this.exit();
    this.sketchId = sketchId;
    this.tool = "select";
    this.host.viewer.pointerHandler = this;
    this.host.viewer.setSketchMouse(true);
    const plane = this.plane();
    if (plane) this.host.viewer.lookAtPlane(plane);
    this.host.setStatus("Sketch mode · L line · R rectangle · C circle · A arc · B spline · P polygon · N slot · T trim · O offset · I fillet · M mirror · Y pattern · U use · S select · Q construction · right-drag orbits · Esc finishes");
  }

  exit(): void {
    if (!this.active) return;
    this.cancel();
    this.sketchId = null;
    this.selection.clear();
    this.host.viewer.pointerHandler = null;
    this.host.viewer.setSketchMouse(false);
    this.host.viewer.setPreview([]);
    this.host.viewer.resetUp();
    this.host.regenerate(); // refreshes the status line and selection colours
  }

  setTool(tool: Tool): void {
    this.cancel();
    const wasUse = this.tool === "use";
    this.tool = tool;
    if (wasUse) this.host.viewer.hoverEdgeOrFace(null);
    // Entering or leaving "use" changes which bodies are shown.
    if (wasUse !== (tool === "use")) this.host.regenerate();
    else this.host.selectionChanged();
    if (tool === "use") this.host.setStatus("Use: click a body edge or face to project it into the sketch (only bodies before this sketch are shown)");
    if (tool === "trim") this.host.setStatus("Trim: click the piece of a line, arc or circle to remove (between its crossings)");
    if (tool === "spline") this.host.setStatus("Spline: click the points the curve passes through; click the last point again or press Enter to finish");
  }

  /** Finishes a multi-point tool (Enter): the spline through the points clicked so far. */
  finish(): void {
    if (this.tool === "spline") this.finishSpline();
  }

  /** Offsets the selected chain by a distance asked from the user (O). */
  offsetSelection(): void {
    const entities = [...this.selection].filter((id) => this.entity(id)?.type !== "point");
    if (entities.length === 0) return;
    const text = prompt("Offset distance (negative for the other side)", "1");
    if (text === null) return;
    const distance = Number(text);
    if (!Number.isFinite(distance) || distance === 0) return;
    this.host.snapshot();
    try {
      const r = this.sketchOp({ type: "offset", entities, distance });
      this.selection = new Set(r.entities);
    } catch (err) {
      this.host.setStatus(`error: ${(err as Error).message}`);
    }
    this.host.regenerate();
  }

  /** The two selected lines that share a corner, if the selection is exactly that. */
  filletCandidates(): [number, number] | null {
    const lines = [...this.selection].filter((id) => this.entity(id)?.type === "line");
    if (lines.length !== 2 || this.selection.size !== 2) return null;
    const ends = (id: number): number[] => {
      const e = this.entity(id);
      return e && e.type === "line" ? [e.start, e.end] : [];
    };
    const [a, b] = lines as [number, number];
    const pos = (id: number) => {
      const e = this.entity(id);
      return e && e.type === "point" ? e.pos : null;
    };
    for (const pa of ends(a)) {
      for (const pb of ends(b)) {
        const p = pos(pa), q = pos(pb);
        if (p && q && Math.hypot(p.x - q.x, p.y - q.y) < 1e-6) return [a, b];
      }
    }
    return null;
  }

  /** Fillet the corner between the two selected lines (I): a prompt takes the radius. */
  filletSelection(): void {
    const pair = this.filletCandidates();
    if (!pair) {
      this.host.setStatus("Select two lines that meet at a corner to fillet it.");
      return;
    }
    const text = prompt("Fillet radius", "1");
    if (text === null) return;
    const radius = Number(text);
    if (!Number.isFinite(radius) || radius <= 0) return;
    this.host.snapshot();
    try {
      const r = this.sketchOp({ type: "fillet", a: pair[0], b: pair[1], radius });
      this.selection = new Set(r.entities);
    } catch (err) {
      this.host.setStatus(`error: ${(err as Error).message}`);
    }
    this.host.regenerate();
  }

  /** Pattern the selection (Y): a prompt takes "linear COUNT,DX,DY" or "circular COUNT,CX,CY,ANGLE". */
  patternSelection(): void {
    const entities = [...this.selection];
    if (entities.length === 0) return;
    const text = prompt("Pattern the selection: linear COUNT,DX,DY  or  circular COUNT,CX,CY,ANGLE", "linear 3,20,0");
    if (text === null) return;
    const m = text.trim().match(/^(linear|circular)\s+([-\d.,\s]+)$/i);
    const nums = m ? m[2]!.split(",").map((x) => Number(x.trim())) : [];
    let op: SketchOp | null = null;
    if (m && m[1]!.toLowerCase() === "linear" && nums.length === 3 && nums.every(Number.isFinite)) {
      op = { type: "pattern_linear", entities, count: Math.round(nums[0]!), step: { x: nums[1]!, y: nums[2]! } };
    } else if (m && m[1]!.toLowerCase() === "circular" && nums.length === 4 && nums.every(Number.isFinite)) {
      op = { type: "pattern_circular", entities, count: Math.round(nums[0]!), center: { x: nums[1]!, y: nums[2]! }, angle: nums[3]! };
    }
    if (!op) {
      this.host.setStatus('Pattern: use "linear 3,20,0" (count, dx, dy) or "circular 6,0,0,60" (count, centre x, centre y, angle).');
      return;
    }
    this.host.snapshot();
    try {
      const r = this.sketchOp(op);
      this.selection = new Set(r.entities);
    } catch (err) {
      this.host.setStatus(`error: ${(err as Error).message}`);
    }
    this.host.regenerate();
  }

  /** Starts a mirror of the selection: the next clicked line is the axis (M). */
  beginMirror(): void {
    if (this.selection.size === 0) return;
    this.cancel();
    this.tool = "select";
    this.awaitingMirrorAxis = true;
    this.host.selectionChanged();
    this.host.setStatus("Mirror: click the line to mirror across (Esc cancels)");
  }

  private finishMirror(axis: number): void {
    const entities = [...this.selection].filter((id) => id !== axis);
    this.awaitingMirrorAxis = false;
    this.host.snapshot();
    try {
      const r = this.sketchOp({ type: "mirror", entities, axis });
      this.selection = new Set(r.entities);
    } catch (err) {
      this.host.setStatus(`error: ${(err as Error).message}`);
    }
    this.host.regenerate();
  }

  private clickTrim(e: PointerEvent): void {
    const id = this.nearestCurve(e);
    const plane = this.plane();
    if (id === null || !plane) return;
    const at = this.host.viewer.toPlane(e, plane);
    if (!at) return;
    this.host.snapshot();
    try {
      this.sketchOp({ type: "trim", entity: id, at });
    } catch (err) {
      this.host.setStatus(`error: ${(err as Error).message}`);
    }
    this.host.regenerate();
  }

  /** Entities that belong to a projection, with the projection's index. */
  projectionOf(entity: number): number | null {
    const f = this.host.summary.features.find((f) => f.id === this.sketchId);
    const list = f && f.kind.type === "sketch" ? f.kind.projections ?? [] : [];
    const i = list.findIndex((p) => p.entities.includes(entity));
    return i < 0 ? null : i;
  }

  // ------------------------------------------------------------ data access

  private plane(): PlaneFrame | null {
    return this.host.summary.sketches[String(this.sketchId)]?.plane ?? null;
  }

  private curves(): SketchCurve[] {
    return this.host.summary.sketches[String(this.sketchId)]?.curves ?? [];
  }

  sketchData(): SketchData | null {
    const f = this.host.summary.features.find((f) => f.id === this.sketchId);
    return f && f.kind.type === "sketch" ? f.kind.sketch : null;
  }

  entity(id: number): ({ id: number } & Entity) | undefined {
    return this.sketchData()?.entities.find((e) => e.id === id);
  }

  pointPos(id: number): Vec2 | null {
    const e = this.entity(id);
    return e && e.type === "point" ? e.pos : null;
  }

  private lift(p: Vec2): Vec3 {
    const pl = this.plane()!;
    return {
      x: pl.origin.x + pl.x_axis.x * p.x + pl.y_axis.x * p.y,
      y: pl.origin.y + pl.x_axis.y * p.x + pl.y_axis.y * p.y,
      z: pl.origin.z + pl.x_axis.z * p.x + pl.y_axis.z * p.y,
    };
  }

  // ------------------------------------------------------------ hit testing

  /** Nearest point entity to the pointer within the snap radius. */
  private nearestPoint(e: PointerEvent): { id: number; pos: Vec2 } | null {
    const px = this.host.viewer.eventPx(e);
    let best: { id: number; pos: Vec2; d: number } | null = null;
    for (const c of this.curves()) {
      if (c.kind !== "point") continue;
      const s = this.host.viewer.toScreen(c.points[0]!);
      const d = Math.hypot(s.x - px.x, s.y - px.y);
      if (d <= SNAP_PX && (!best || d < best.d)) {
        const pos = this.pointPos(c.entity);
        if (pos) best = { id: c.entity, pos, d };
      }
    }
    return best;
  }

  /** Nearest curve (line, circle, arc) within a few pixels. */
  private nearestCurve(e: PointerEvent): number | null {
    const px = this.host.viewer.eventPx(e);
    let best: { id: number; d: number } | null = null;
    for (const c of this.curves()) {
      if (c.kind === "point") continue;
      const pts = c.points.map((p) => this.host.viewer.toScreen(p));
      for (let i = 0; i + 1 < pts.length; i++) {
        const d = segmentDistance(px, pts[i]!, pts[i + 1]!);
        if (d <= CURVE_PX && (!best || d < best.d)) best = { id: c.entity, d };
      }
    }
    return best?.id ?? null;
  }

  /** Pointer position on the sketch plane, snapped to a nearby point or the origin. */
  private snap(e: PointerEvent): { pos: Vec2; id: number | null } | null {
    const near = this.nearestPoint(e);
    if (near) return { pos: near.pos, id: near.id };
    const plane = this.plane();
    if (!plane) return null;
    const raw = this.host.viewer.toPlane(e, plane);
    if (!raw) return null;
    const o = this.host.viewer.toScreen(this.lift({ x: 0, y: 0 }));
    const px = this.host.viewer.eventPx(e);
    if (Math.hypot(o.x - px.x, o.y - px.y) <= SNAP_PX) return { pos: { x: 0, y: 0 }, id: null };
    return { pos: raw, id: null };
  }

  // ------------------------------------------------------------ pointer handler

  down(e: PointerEvent): void {
    if (this.tool !== "select") return;
    if (this.awaitingMirrorAxis) {
      const line = this.nearestCurve(e);
      if (line !== null && this.entity(line)?.type === "line") this.finishMirror(line);
      else this.host.setStatus("Mirror: that is not a line; click a line or press Esc");
      return;
    }
    const point = this.nearestPoint(e);
    const hit = point?.id ?? this.nearestCurve(e);
    if (hit === null || hit === undefined) {
      if (!e.shiftKey) this.selection.clear();
    } else if (e.shiftKey) {
      if (this.selection.has(hit)) this.selection.delete(hit);
      else this.selection.add(hit);
    } else {
      if (!this.selection.has(hit)) {
        this.selection.clear();
        this.selection.add(hit);
      }
      if (point && this.projectionOf(point.id) === null) {
        this.drag = { entity: point.id, moved: false };
        this.host.snapshot();
      }
    }
    this.host.selectionChanged();
  }

  move(e: PointerEvent): void {
    if (this.drag) {
      const plane = this.plane();
      const pos = plane && this.host.viewer.toPlane(e, plane);
      if (!pos) return;
      this.drag.moved = true;
      try {
        this.host.applyRaw({ type: "sketch", id: this.sketchId!, op: { type: "move_point", id: this.drag.entity, pos } });
      } catch {
        return;
      }
      if (!this.regenQueued) {
        this.regenQueued = true;
        requestAnimationFrame(() => {
          this.regenQueued = false;
          this.host.regenerate();
        });
      }
      return;
    }
    if (this.tool === "use") {
      this.host.viewer.hoverEdgeOrFace(e);
      return;
    }
    if (this.tool !== "select" && this.pending.length > 0) {
      const s = this.snap(e);
      if (s) this.host.viewer.setPreview(this.previewFor(this.inferred(s).pos));
    }
  }

  up(e: PointerEvent): void {
    if (this.drag) {
      this.drag = null;
      return;
    }
    if (this.tool === "select") return;
    if (this.tool === "use") {
      this.host.projectAt(e);
      return;
    }
    if (this.tool === "trim") {
      this.clickTrim(e);
      return;
    }
    const s = this.snap(e);
    if (!s) return;
    switch (this.tool) {
      case "line":
        this.clickLine(s);
        break;
      case "rectangle":
        this.clickRectangle(s);
        break;
      case "circle":
        this.clickCircle(s);
        break;
      case "arc":
        this.clickArc(s);
        break;
      case "spline":
        this.clickSpline(s);
        break;
      case "polygon":
        this.clickPolygon(s);
        break;
      case "slot":
        this.clickSlot(s);
        break;
    }
  }

  cancel(): void {
    this.pending = [];
    this.pendingIds = [];
    this.drag = null;
    this.awaitingMirrorAxis = false;
    this.host.viewer.setPreview([]);
  }

  // ------------------------------------------------------------ tools

  /** Applies horizontal/vertical inference relative to the pending start point. */
  private inferred(s: { pos: Vec2; id: number | null }): { pos: Vec2; id: number | null; h: boolean; v: boolean } {
    if (this.tool !== "line" || this.pending.length === 0 || s.id !== null) return { ...s, h: false, v: false };
    const a = this.pending[0]!;
    const dx = s.pos.x - a.x;
    const dy = s.pos.y - a.y;
    if (Math.abs(dy) <= INFER_TAN * Math.abs(dx)) return { pos: { x: s.pos.x, y: a.y }, id: null, h: true, v: false };
    if (Math.abs(dx) <= INFER_TAN * Math.abs(dy)) return { pos: { x: a.x, y: s.pos.y }, id: null, h: false, v: true };
    return { ...s, h: false, v: false };
  }

  private previewFor(cur: Vec2): Vec3[][] {
    const a = this.pending[0]!;
    switch (this.tool) {
      case "line":
        return [[this.lift(a), this.lift(cur)]];
      case "rectangle":
        return [[this.lift(a), this.lift({ x: cur.x, y: a.y }), this.lift(cur), this.lift({ x: a.x, y: cur.y }), this.lift(a)]];
      case "circle": {
        const r = Math.hypot(cur.x - a.x, cur.y - a.y);
        const pts: Vec3[] = [];
        for (let i = 0; i <= 64; i++) {
          const t = (i / 64) * Math.PI * 2;
          pts.push(this.lift({ x: a.x + r * Math.cos(t), y: a.y + r * Math.sin(t) }));
        }
        return [pts];
      }
      case "arc": {
        if (this.pending.length === 1) return [[this.lift(a), this.lift(cur)]];
        const start = this.pending[1]!;
        const r = Math.hypot(start.x - a.x, start.y - a.y);
        const a0 = Math.atan2(start.y - a.y, start.x - a.x);
        let sweep = Math.atan2(cur.y - a.y, cur.x - a.x) - a0;
        while (sweep <= 0) sweep += Math.PI * 2;
        const n = Math.max(2, Math.ceil(sweep / (Math.PI / 36)));
        const pts: Vec3[] = [];
        for (let i = 0; i <= n; i++) {
          const t = a0 + (sweep * i) / n;
          pts.push(this.lift({ x: a.x + r * Math.cos(t), y: a.y + r * Math.sin(t) }));
        }
        return [[this.lift(a), this.lift(start)], pts];
      }
      case "spline":
        return [splinePolyline([...this.pending, cur], 12).map((p) => this.lift(p))];
      case "polygon": {
        const n = this.polygonSides;
        const pts: Vec3[] = [];
        for (let i = 0; i <= n; i++) {
          const t = (Math.PI * 2 * i) / n;
          const dx = cur.x - a.x, dy = cur.y - a.y;
          pts.push(this.lift({ x: a.x + dx * Math.cos(t) - dy * Math.sin(t), y: a.y + dx * Math.sin(t) + dy * Math.cos(t) }));
        }
        return [pts];
      }
      case "slot": {
        if (this.pending.length === 1) return [[this.lift(a), this.lift(cur)]];
        const b = this.pending[1]!;
        const r = slotHalfWidth(a, b, cur);
        if (r < 1e-9) return [[this.lift(a), this.lift(b)]];
        const len = Math.hypot(b.x - a.x, b.y - a.y);
        const ux = (b.x - a.x) / len, uy = (b.y - a.y) / len;
        const pts: Vec3[] = [];
        const t0 = Math.atan2(uy, ux);
        for (let i = 0; i <= 18; i++) {
          const t = t0 - Math.PI / 2 + (Math.PI * i) / 18;
          pts.push(this.lift({ x: b.x + r * Math.cos(t), y: b.y + r * Math.sin(t) }));
        }
        for (let i = 0; i <= 18; i++) {
          const t = t0 + Math.PI / 2 + (Math.PI * i) / 18;
          pts.push(this.lift({ x: a.x + r * Math.cos(t), y: a.y + r * Math.sin(t) }));
        }
        pts.push(pts[0]!);
        return [pts];
      }
      default:
        return [];
    }
  }

  /** Spline tool: every click adds a point; clicking the last point again finishes. */
  private clickSpline(s: { pos: Vec2; id: number | null }): void {
    const last = this.pending[this.pending.length - 1];
    if (last && Math.hypot(s.pos.x - last.x, s.pos.y - last.y) < 1e-9) {
      this.finishSpline();
      return;
    }
    this.pending.push(s.pos);
    this.pendingIds.push(s.id);
    this.host.setStatus(`Spline: ${this.pending.length} point${this.pending.length === 1 ? "" : "s"} · click more, click the last point again or press Enter to finish, Esc cancels`);
  }

  private finishSpline(): void {
    if (this.pending.length < 2) {
      this.cancel();
      this.host.setStatus("A spline needs at least two points.");
      return;
    }
    const points = this.pending.slice();
    const ids = this.pendingIds.slice();
    this.host.snapshot();
    try {
      const res = this.sketchOp({ type: "add_spline", points });
      const [, ...pointIds] = res.entities;
      ids.forEach((id, i) => {
        const p = pointIds[i];
        if (id !== null && id !== undefined && p !== undefined) this.constrain({ type: "coincident", a: p, b: id });
      });
    } catch (err) {
      this.host.setStatus(`error: ${(err as Error).message}`);
    }
    this.cancel();
    this.host.regenerate();
  }

  /** Sides for the polygon tool; asked for on the first click and remembered. */
  polygonSides = 6;

  /** Polygon tool: centre, then one corner. */
  private clickPolygon(s: { pos: Vec2; id: number | null }): void {
    if (this.pending.length === 0) {
      const answer = prompt("Number of sides", String(this.polygonSides));
      if (answer === null) return;
      const n = Math.round(Number(answer));
      if (!Number.isFinite(n) || n < 3 || n > 64) {
        this.host.setStatus("A polygon needs between 3 and 64 sides.");
        return;
      }
      this.polygonSides = n;
      this.pending = [s.pos];
      this.pendingIds = [s.id];
      return;
    }
    const c = this.pending[0]!;
    if (Math.hypot(s.pos.x - c.x, s.pos.y - c.y) < 1e-9) return;
    this.host.snapshot();
    try {
      this.sketchOp({ type: "add_polygon", center: c, vertex: s.pos, sides: this.polygonSides });
    } catch (err) {
      this.host.setStatus(`error: ${(err as Error).message}`);
    }
    this.cancel();
    this.host.regenerate();
  }

  /** Slot tool: one centre, the other centre, then a point setting the width. */
  private clickSlot(s: { pos: Vec2; id: number | null }): void {
    if (this.pending.length < 2) {
      if (this.pending.length === 1 && Math.hypot(s.pos.x - this.pending[0]!.x, s.pos.y - this.pending[0]!.y) < 1e-9) return;
      this.pending.push(s.pos);
      this.pendingIds.push(s.id);
      return;
    }
    const [a, b] = [this.pending[0]!, this.pending[1]!];
    const r = slotHalfWidth(a, b, s.pos);
    if (r < 1e-9) return;
    this.host.snapshot();
    try {
      const out = this.sketchOp({ type: "add_slot", a, b, width: 2 * r });
      // Snapped centres stay tied to what they snapped to: the arcs' centres are entities 1 and 3.
      const arcB = out.entities[1], arcA = out.entities[3];
      const centreOf = (arc: number | undefined) => {
        const e = this.host.summary.features.find((f) => f.id === this.sketchId)?.kind;
        if (!e || e.type !== "sketch" || arc === undefined) return null;
        const ent = e.sketch.entities.find((x) => x.id === arc);
        return ent && ent.type === "arc" ? ent.center : null;
      };
      const [idA, idB] = [this.pendingIds[0], this.pendingIds[1]];
      const ca = centreOf(arcA), cb = centreOf(arcB);
      if (idA !== null && idA !== undefined && ca !== null) this.constrain({ type: "coincident", a: ca, b: idA });
      if (idB !== null && idB !== undefined && cb !== null) this.constrain({ type: "coincident", a: cb, b: idB });
    } catch (err) {
      this.host.setStatus(`error: ${(err as Error).message}`);
    }
    this.cancel();
    this.host.regenerate();
  }

  private sketchOp(op: SketchOp): OpResult {
    return this.host.applyRaw({ type: "sketch", id: this.sketchId!, op });
  }

  private constrain(constraint: Constraint): void {
    this.sketchOp({ type: "add_constraint", constraint });
  }

  private clickLine(s: { pos: Vec2; id: number | null }): void {
    if (this.pending.length === 0) {
      this.pending = [s.pos];
      this.pendingIds = [s.id];
      return;
    }
    const end = this.inferred(s);
    const a = this.pending[0]!;
    if (Math.hypot(end.pos.x - a.x, end.pos.y - a.y) < 1e-9) return;
    this.host.snapshot();
    try {
      const r = this.sketchOp({ type: "add_line", a, b: end.pos });
      const [line, start, finish] = r.entities as [number, number, number];
      const startId = this.pendingIds[0];
      if (startId !== null && startId !== undefined) this.constrain({ type: "coincident", a: start, b: startId });
      if (end.id !== null) this.constrain({ type: "coincident", a: finish, b: end.id });
      if (end.h) this.constrain({ type: "horizontal", line });
      if (end.v) this.constrain({ type: "vertical", line });
      // Chain: the next segment starts at this end, unless we closed onto an existing point.
      if (end.id !== null) {
        this.pending = [];
        this.pendingIds = [];
        this.host.viewer.setPreview([]);
      } else {
        this.pending = [end.pos];
        this.pendingIds = [finish];
      }
    } catch (err) {
      this.host.setStatus(`error: ${(err as Error).message}`);
      this.cancel();
    }
    this.host.regenerate();
  }

  private clickRectangle(s: { pos: Vec2; id: number | null }): void {
    if (this.pending.length === 0) {
      this.pending = [s.pos];
      this.pendingIds = [s.id];
      return;
    }
    const a = this.pending[0]!;
    if (Math.abs(s.pos.x - a.x) < 1e-9 || Math.abs(s.pos.y - a.y) < 1e-9) return;
    this.host.snapshot();
    try {
      this.sketchOp({ type: "add_rectangle", a, b: s.pos });
    } catch (err) {
      this.host.setStatus(`error: ${(err as Error).message}`);
    }
    this.cancel();
    this.host.regenerate();
  }

  private clickCircle(s: { pos: Vec2; id: number | null }): void {
    if (this.pending.length === 0) {
      this.pending = [s.pos];
      this.pendingIds = [s.id];
      return;
    }
    const c = this.pending[0]!;
    const radius = Math.hypot(s.pos.x - c.x, s.pos.y - c.y);
    if (radius < 1e-9) return;
    this.host.snapshot();
    try {
      const r = this.sketchOp({ type: "add_circle", center: c, radius });
      const centerId = this.pendingIds[0];
      if (centerId !== null && centerId !== undefined) this.constrain({ type: "coincident", a: r.entities[1]!, b: centerId });
    } catch (err) {
      this.host.setStatus(`error: ${(err as Error).message}`);
    }
    this.cancel();
    this.host.regenerate();
  }

  /** Arc tool: centre, start, then end (projected onto the radius; counter-clockwise from start). */
  private clickArc(s: { pos: Vec2; id: number | null }): void {
    if (this.pending.length < 2) {
      if (this.pending.length === 1 && Math.hypot(s.pos.x - this.pending[0]!.x, s.pos.y - this.pending[0]!.y) < 1e-9) return;
      this.pending.push(s.pos);
      this.pendingIds.push(s.id);
      return;
    }
    const c = this.pending[0]!;
    const start = this.pending[1]!;
    const r = Math.hypot(start.x - c.x, start.y - c.y);
    const ang = Math.atan2(s.pos.y - c.y, s.pos.x - c.x);
    const end = { x: c.x + r * Math.cos(ang), y: c.y + r * Math.sin(ang) };
    if (Math.hypot(end.x - start.x, end.y - start.y) < 1e-9) return;
    this.host.snapshot();
    try {
      const res = this.sketchOp({ type: "add_arc", center: c, start, end });
      const [, centerId, startId, endId] = res.entities as [number, number, number, number];
      const cId = this.pendingIds[0];
      const sId = this.pendingIds[1];
      if (cId !== null && cId !== undefined) this.constrain({ type: "coincident", a: centerId, b: cId });
      if (sId !== null && sId !== undefined) this.constrain({ type: "coincident", a: startId, b: sId });
      if (s.id !== null) this.constrain({ type: "coincident", a: endId, b: s.id });
    } catch (err) {
      this.host.setStatus(`error: ${(err as Error).message}`);
    }
    this.cancel();
    this.host.regenerate();
  }

  /** Toggles construction on the selected curves. */
  toggleConstruction(): void {
    const data = this.sketchData();
    if (!data || this.selection.size === 0) return;
    const construction = new Set(data.construction ?? []);
    this.host.snapshot();
    for (const id of this.selection) {
      const e = this.entity(id);
      if (!e || e.type === "point") continue;
      try {
        this.sketchOp({ type: "set_construction", id, construction: !construction.has(id) });
      } catch {
        /* ignore */
      }
    }
    this.host.regenerate();
  }

  // ------------------------------------------------------------ selection helpers

  deleteSelection(): void {
    if (this.selection.size === 0) return;
    this.host.snapshot();
    // Projected entities go with their projection (highest index first so
    // the remaining indices stay valid).
    const projections = new Set<number>();
    for (const id of this.selection) {
      const p = this.projectionOf(id);
      if (p !== null) projections.add(p);
    }
    for (const index of [...projections].sort((a, b) => b - a)) this.sketchOp({ type: "remove_projection", index });
    for (const id of this.selection) {
      if (this.projectionOf(id) !== null) continue;
      try {
        this.sketchOp({ type: "remove_entity", id });
      } catch {
        /* already gone */
      }
    }
    this.selection.clear();
    this.host.regenerate();
  }

  /** Constraints that can be applied to the current selection, with a
   *  builder that may ask for a value. */
  quickConstraints(): { label: string; build: () => Constraint | null }[] {
    const ids = [...this.selection];
    const kinds = ids.map((id) => this.entity(id)?.type ?? "?");
    const out: { label: string; build: () => Constraint | null }[] = [];
    const ask = (label: string, def: number): number | null => {
      const v = Number(prompt(label, String(Number(def.toFixed(4)))));
      return Number.isFinite(v) ? v : null;
    };
    const pos = (id: number) => this.pointPos(id)!;
    const lineLen = (id: number) => {
      const l = this.entity(id);
      if (!l || l.type !== "line") return 0;
      const a = pos(l.start), b = pos(l.end);
      return Math.hypot(b.x - a.x, b.y - a.y);
    };
    const radiusOf = (id: number) => {
      const e = this.entity(id);
      if (!e) return 0;
      if (e.type === "circle") return e.radius;
      if (e.type === "arc") {
        const c = pos(e.center), s = pos(e.start);
        return Math.hypot(s.x - c.x, s.y - c.y);
      }
      return 0;
    };
    const is = (...want: string[]) => kinds.length === want.length && want.every((w, i) => (w === "round" ? kinds[i] === "circle" || kinds[i] === "arc" : kinds[i] === w));
    const sorted = (a: string, b: string): [number, number] | null => {
      if (kinds.length !== 2) return null;
      const ia = kinds.findIndex((k) => (a === "round" ? k === "circle" || k === "arc" : k === a));
      const ib = kinds.findIndex((k, i) => i !== ia && (b === "round" ? k === "circle" || k === "arc" : k === b));
      return ia >= 0 && ib >= 0 ? [ids[ia]!, ids[ib]!] : null;
    };
    if (is("line")) {
      const line = ids[0]!;
      out.push({ label: "Horizontal", build: () => ({ type: "horizontal", line }) });
      out.push({ label: "Vertical", build: () => ({ type: "vertical", line }) });
      out.push({ label: "Length", build: () => { const v = ask("Length", lineLen(line)); return v === null ? null : { type: "length", line, value: v }; } });
    } else if (is("line", "line")) {
      const [a, b] = ids as [number, number];
      out.push({ label: "Parallel", build: () => ({ type: "parallel", a, b }) });
      out.push({ label: "Perpendicular", build: () => ({ type: "perpendicular", a, b }) });
      out.push({ label: "Equal", build: () => ({ type: "equal", a, b }) });
      out.push({ label: "Angle", build: () => { const v = ask("Angle (degrees, from first to second)", 90); return v === null ? null : { type: "angle", a, b, value: v }; } });
    } else if (is("point")) {
      out.push({ label: "Fix", build: () => ({ type: "fixed", point: ids[0]! }) });
    } else if (is("point", "point")) {
      const [a, b] = ids as [number, number];
      const pa = pos(a), pb = pos(b);
      out.push({ label: "Coincident", build: () => ({ type: "coincident", a, b }) });
      out.push({ label: "Distance", build: () => { const v = ask("Distance", Math.hypot(pb.x - pa.x, pb.y - pa.y)); return v === null ? null : { type: "distance", a, b, value: v }; } });
      out.push({ label: "Horizontal dist.", build: () => { const v = ask("Horizontal distance", pb.x - pa.x); return v === null ? null : { type: "horizontal_distance", a, b, value: v }; } });
      out.push({ label: "Vertical dist.", build: () => { const v = ask("Vertical distance", pb.y - pa.y); return v === null ? null : { type: "vertical_distance", a, b, value: v }; } });
    } else if (sorted("point", "line")) {
      const [point, line] = sorted("point", "line")!;
      out.push({ label: "On line", build: () => ({ type: "point_on_line", point, line }) });
      out.push({ label: "Midpoint", build: () => ({ type: "midpoint", point, line }) });
    } else if (kinds.length === 3 && kinds.filter((k) => k === "point").length === 2 && kinds.includes("line")) {
      const [a, b] = ids.filter((id) => this.entity(id)?.type === "point") as [number, number];
      const line = ids.find((id) => this.entity(id)?.type === "line")!;
      out.push({ label: "Symmetric", build: () => ({ type: "symmetric", a, b, line }) });
    } else if (is("round")) {
      const entity = ids[0]!;
      out.push({ label: "Radius", build: () => { const v = ask("Radius", radiusOf(entity)); return v === null ? null : { type: "radius", entity, value: v }; } });
      out.push({ label: "Diameter", build: () => { const v = ask("Diameter", 2 * radiusOf(entity)); return v === null ? null : { type: "diameter", entity, value: v }; } });
    } else if (is("round", "round")) {
      out.push({ label: "Equal", build: () => ({ type: "equal", a: ids[0]!, b: ids[1]! }) });
    } else if (sorted("point", "round")) {
      const [point, entity] = sorted("point", "round")!;
      out.push({ label: "On circle", build: () => ({ type: "point_on_circle", point, entity }) });
    } else if (sorted("line", "round")) {
      const [line, entity] = sorted("line", "round")!;
      out.push({ label: "Tangent", build: () => ({ type: "tangent", line, entity }) });
      out.push({ label: "Equal", build: () => ({ type: "equal", a: line, b: entity }) });
    }
    return out;
  }

  applyQuick(build: () => Constraint | null): void {
    const c = build();
    if (!c) return;
    this.host.snapshot();
    try {
      this.constrain(c);
    } catch (err) {
      this.host.setStatus(`error: ${(err as Error).message}`);
    }
    this.host.regenerate();
  }
}

function segmentDistance(p: { x: number; y: number }, a: { x: number; y: number }, b: { x: number; y: number }): number {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const len2 = dx * dx + dy * dy;
  const t = len2 === 0 ? 0 : Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / len2));
  return Math.hypot(p.x - (a.x + dx * t), p.y - (a.y + dy * t));
}

/** Half the width of a slot from `a` to `b` whose edge passes through `p`. */
function slotHalfWidth(a: Vec2, b: Vec2, p: Vec2): number {
  const len = Math.hypot(b.x - a.x, b.y - a.y);
  if (len < 1e-9) return 0;
  return Math.abs(((b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)) / len);
}
