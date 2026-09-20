import type { Vec2 } from "./kernel";

/** Sketch geometry read from a DXF file: straight segments, full circles and counter-clockwise arcs. */
export type DxfGeometry = { lines: [Vec2, Vec2][]; circles: { center: Vec2; radius: number }[]; arcs: { center: Vec2; start: Vec2; end: Vec2 }[] };

/**
 * Reads the ENTITIES section of a DXF (ASCII) file: LINE, CIRCLE, ARC,
 * LWPOLYLINE and POLYLINE/VERTEX, with polyline bulges turned into arcs.
 * Coordinates are taken as millimetres in the drawing's own frame (z is
 * dropped); everything else in the file is ignored.
 */
export function parseDxf(text: string): DxfGeometry {
  const out: DxfGeometry = { lines: [], circles: [], arcs: [] };
  const rows = text.split(/\r?\n/);
  const pairs: [number, string][] = [];
  for (let i = 0; i + 1 < rows.length; i += 2) {
    const code = Number(rows[i]!.trim());
    if (!Number.isFinite(code)) throw new Error(`bad group code at line ${i + 1}`);
    pairs.push([code, rows[i + 1]!.trim()]);
  }
  // Split into entities: each starts with a (0, TYPE) pair.
  let entities: [number, string][][] | null = null;
  let current: [number, string][] | null = null;
  for (const [code, value] of pairs) {
    if (code === 0) {
      if (value === "SECTION") { current = null; continue; }
      if (current) entities?.push(current);
      current = [[code, value]];
      if (value === "ENDSEC") { current = null; continue; }
      continue;
    }
    if (code === 2 && value === "ENTITIES" && !current) { entities = []; continue; }
    if (current) current.push([code, value]);
  }
  if (current && entities) entities.push(current);
  if (!entities) throw new Error("no ENTITIES section");
  const num = (e: [number, string][], code: number, def = 0): number => {
    const p = e.find(([c]) => c === code);
    return p ? Number(p[1]) : def;
  };
  const polyline = (vertices: { p: Vec2; bulge: number }[], closed: boolean) => {
    const n = vertices.length;
    const count = closed ? n : n - 1;
    for (let i = 0; i < count; i++) {
      const a = vertices[i]!, b = vertices[(i + 1) % n]!;
      if (Math.hypot(b.p.x - a.p.x, b.p.y - a.p.y) < 1e-9) continue;
      if (Math.abs(a.bulge) < 1e-12) {
        out.lines.push([a.p, b.p]);
        continue;
      }
      // A bulge is tan(θ/4) of the included angle; positive bulges turn counter-clockwise.
      const d = Math.hypot(b.p.x - a.p.x, b.p.y - a.p.y);
      const bl = a.bulge;
      // Centre distance from the chord's midpoint (negative past a half circle).
      const h = (d * (1 - bl * bl)) / (4 * Math.abs(bl));
      const mid = { x: (a.p.x + b.p.x) / 2, y: (a.p.y + b.p.y) / 2 };
      const left = { x: -(b.p.y - a.p.y) / d, y: (b.p.x - a.p.x) / d };
      const sign = bl > 0 ? 1 : -1;
      const center = { x: mid.x + left.x * h * sign, y: mid.y + left.y * h * sign };
      if (bl > 0) out.arcs.push({ center, start: a.p, end: b.p });
      else out.arcs.push({ center, start: b.p, end: a.p });
    }
  };
  let poly: { vertices: { p: Vec2; bulge: number }[]; closed: boolean } | null = null;
  for (const e of entities) {
    const type = e[0]![1];
    switch (type) {
      case "LINE":
        out.lines.push([{ x: num(e, 10), y: num(e, 20) }, { x: num(e, 11), y: num(e, 21) }]);
        break;
      case "CIRCLE":
        out.circles.push({ center: { x: num(e, 10), y: num(e, 20) }, radius: num(e, 40) });
        break;
      case "ARC": {
        const c = { x: num(e, 10), y: num(e, 20) }, r = num(e, 40);
        const a0 = (num(e, 50) * Math.PI) / 180, a1 = (num(e, 51) * Math.PI) / 180;
        out.arcs.push({ center: c, start: { x: c.x + r * Math.cos(a0), y: c.y + r * Math.sin(a0) }, end: { x: c.x + r * Math.cos(a1), y: c.y + r * Math.sin(a1) } });
        break;
      }
      case "LWPOLYLINE": {
        const vertices: { p: Vec2; bulge: number }[] = [];
        for (const [code, value] of e) {
          if (code === 10) vertices.push({ p: { x: Number(value), y: 0 }, bulge: 0 });
          else if (code === 20 && vertices.length) vertices[vertices.length - 1]!.p.y = Number(value);
          else if (code === 42 && vertices.length) vertices[vertices.length - 1]!.bulge = Number(value);
        }
        polyline(vertices, (num(e, 70) & 1) === 1);
        break;
      }
      case "POLYLINE":
        poly = { vertices: [], closed: (num(e, 70) & 1) === 1 };
        break;
      case "VERTEX":
        if (poly) poly.vertices.push({ p: { x: num(e, 10), y: num(e, 20) }, bulge: num(e, 42) });
        break;
      case "SEQEND":
        if (poly) polyline(poly.vertices, poly.closed);
        poly = null;
        break;
      default:
        break;
    }
  }
  return out;
}
