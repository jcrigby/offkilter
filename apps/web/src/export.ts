// File exporters: STL and 3MF for bodies, DXF for sketches. All are
// written by hand so the client carries no extra dependencies.

import type { BodyMesh, BodySummary, SketchData, Vec2, ViewLines } from "./kernel";

/** All bodies as one binary STL file. */
export function toStl(meshes: BodyMesh[]): Blob {
  const count = meshes.reduce((n, m) => n + m.indices.length / 3, 0);
  const buf = new ArrayBuffer(84 + count * 50);
  const view = new DataView(buf);
  new Uint8Array(buf, 0, 80).set(new TextEncoder().encode("offkilter binary STL").subarray(0, 80));
  view.setUint32(80, count, true);
  let off = 84;
  for (const m of meshes) {
    for (let t = 0; t < m.indices.length; t += 3) {
      const idx = [m.indices[t]!, m.indices[t + 1]!, m.indices[t + 2]!];
      const p = idx.map((i) => [m.positions[3 * i]!, m.positions[3 * i + 1]!, m.positions[3 * i + 2]!]);
      const ux = p[1]![0]! - p[0]![0]!, uy = p[1]![1]! - p[0]![1]!, uz = p[1]![2]! - p[0]![2]!;
      const vx = p[2]![0]! - p[0]![0]!, vy = p[2]![1]! - p[0]![1]!, vz = p[2]![2]! - p[0]![2]!;
      let nx = uy * vz - uz * vy, ny = uz * vx - ux * vz, nz = ux * vy - uy * vx;
      const len = Math.hypot(nx, ny, nz) || 1;
      nx /= len; ny /= len; nz /= len;
      for (const v of [nx, ny, nz, ...p.flat()]) {
        view.setFloat32(off, v, true);
        off += 4;
      }
      view.setUint16(off, 0, true);
      off += 2;
    }
  }
  return new Blob([buf], { type: "model/stl" });
}

const escapeXml = (s: string): string => s.replace(/[<>&"']/g, (c) => ({ "<": "&lt;", ">": "&gt;", "&": "&amp;", '"': "&quot;", "'": "&apos;" })[c]!);

/**
 * All bodies as a 3MF package (one mesh object per body, named, in
 * millimetres). 3MF keeps part names and exact shared vertices, which STL
 * loses.
 */
export function to3mf(bodies: BodySummary[], meshes: BodyMesh[], title: string): Blob {
  const objects: string[] = [];
  const items: string[] = [];
  meshes.forEach((m, i) => {
    const id = i + 1;
    const name = bodies[i]?.name ?? `Part ${id}`;
    const verts: string[] = [];
    for (let v = 0; v < m.positions.length; v += 3) {
      verts.push(`<vertex x="${fmt(m.positions[v]!)}" y="${fmt(m.positions[v + 1]!)}" z="${fmt(m.positions[v + 2]!)}"/>`);
    }
    const tris: string[] = [];
    for (let t = 0; t < m.indices.length; t += 3) {
      tris.push(`<triangle v1="${m.indices[t]}" v2="${m.indices[t + 1]}" v3="${m.indices[t + 2]}"/>`);
    }
    objects.push(`<object id="${id}" name="${escapeXml(name)}" type="model"><mesh><vertices>${verts.join("")}</vertices><triangles>${tris.join("")}</triangles></mesh></object>`);
    items.push(`<item objectid="${id}"/>`);
  });
  const model =
    `<?xml version="1.0" encoding="UTF-8"?>\n` +
    `<model unit="millimeter" xml:lang="en-US" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">` +
    `<metadata name="Title">${escapeXml(title)}</metadata><metadata name="Application">offkilter</metadata>` +
    `<resources>${objects.join("")}</resources><build>${items.join("")}</build></model>`;
  const contentTypes =
    `<?xml version="1.0" encoding="UTF-8"?>\n<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">` +
    `<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>` +
    `<Default Extension="model" ContentType="application/vnd.ms-package.3dmanufacturing-3dmodel+xml"/></Types>`;
  const rels =
    `<?xml version="1.0" encoding="UTF-8"?>\n<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">` +
    `<Relationship Target="/3D/3dmodel.model" Id="rel0" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"/></Relationships>`;
  return new Blob([zip([
    ["[Content_Types].xml", contentTypes],
    ["_rels/.rels", rels],
    ["3D/3dmodel.model", model],
  ])], { type: "model/3mf" });
}

function fmt(v: number): string {
  return Number.isInteger(v) ? String(v) : v.toPrecision(9).replace(/\.?0+$/, "");
}

// ---- minimal zip writer (stored entries, CRC-32)

const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

function crc32(data: Uint8Array): number {
  let c = 0xffffffff;
  for (const b of data) c = CRC_TABLE[(c ^ b) & 0xff]! ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

/** Packs text entries into a zip archive without compression. */
export function zip(entries: [string, string][]): Uint8Array<ArrayBuffer> {
  const enc = new TextEncoder();
  const locals: Uint8Array[] = [];
  const centrals: Uint8Array[] = [];
  let offset = 0;
  for (const [name, text] of entries) {
    const nameBytes = enc.encode(name);
    const data = enc.encode(text);
    const crc = crc32(data);
    const local = new Uint8Array(30 + nameBytes.length + data.length);
    const lv = new DataView(local.buffer);
    lv.setUint32(0, 0x04034b50, true);
    lv.setUint16(4, 20, true);
    lv.setUint16(6, 0x0800, true); // UTF-8 names
    lv.setUint16(8, 0, true); // stored
    lv.setUint16(10, 0, true);
    lv.setUint16(12, 0x21, true); // 1980-01-01
    lv.setUint32(14, crc, true);
    lv.setUint32(18, data.length, true);
    lv.setUint32(22, data.length, true);
    lv.setUint16(26, nameBytes.length, true);
    lv.setUint16(28, 0, true);
    local.set(nameBytes, 30);
    local.set(data, 30 + nameBytes.length);
    const central = new Uint8Array(46 + nameBytes.length);
    const cv = new DataView(central.buffer);
    cv.setUint32(0, 0x02014b50, true);
    cv.setUint16(4, 20, true);
    cv.setUint16(6, 20, true);
    cv.setUint16(8, 0x0800, true);
    cv.setUint16(10, 0, true);
    cv.setUint16(12, 0, true);
    cv.setUint16(14, 0x21, true);
    cv.setUint32(16, crc, true);
    cv.setUint32(20, data.length, true);
    cv.setUint32(24, data.length, true);
    cv.setUint16(28, nameBytes.length, true);
    cv.setUint32(42, offset, true);
    central.set(nameBytes, 46);
    locals.push(local);
    centrals.push(central);
    offset += local.length;
  }
  const centralSize = centrals.reduce((n, c) => n + c.length, 0);
  const end = new Uint8Array(22);
  const ev = new DataView(end.buffer);
  ev.setUint32(0, 0x06054b50, true);
  ev.setUint16(8, entries.length, true);
  ev.setUint16(10, entries.length, true);
  ev.setUint32(12, centralSize, true);
  ev.setUint32(16, offset, true);
  const out = new Uint8Array(new ArrayBuffer(offset + centralSize + 22));
  let o = 0;
  for (const part of [...locals, ...centrals, end]) {
    out.set(part, o);
    o += part.length;
  }
  return out;
}

/** DXF (R12 subset) header and layer table; `layers` are [name, colour, linetype]. */
function dxfHead(layers: [string, number, string][]): string[] {
  const lines: string[] = ["0", "SECTION", "2", "HEADER", "9", "$INSUNITS", "70", "4", "0", "ENDSEC", "0", "SECTION", "2", "TABLES", "0", "TABLE", "2", "LAYER", "70", String(layers.length)];
  for (const [name, colour, linetype] of layers) lines.push("0", "LAYER", "2", name, "70", "0", "62", String(colour), "6", linetype);
  lines.push("0", "ENDTAB", "0", "ENDSEC", "0", "SECTION", "2", "ENTITIES");
  return lines;
}

/**
 * A sketch as a DXF (R12 subset: LINE, CIRCLE, ARC, POINT) in sketch-plane
 * coordinates. Construction entities go on a separate layer.
 */
export function toDxf(sketch: SketchData): string {
  const points = new Map<number, { x: number; y: number }>();
  for (const e of sketch.entities) if (e.type === "point") points.set(e.id, e.pos);
  const construction = new Set(sketch.construction ?? []);
  const lines = dxfHead([["SKETCH", 7, "CONTINUOUS"], ["CONSTRUCTION", 8, "DASHED"]]);
  const layer = (id: number) => (construction.has(id) ? "CONSTRUCTION" : "SKETCH");
  const num = (v: number) => fmt(v);
  const deg = (r: number) => ((r * 180) / Math.PI + 360) % 360;
  const endpoints = new Set<number>();
  for (const e of sketch.entities) {
    if (e.type === "line") {
      endpoints.add(e.start);
      endpoints.add(e.end);
    } else if (e.type === "arc") {
      endpoints.add(e.center);
      endpoints.add(e.start);
      endpoints.add(e.end);
    } else if (e.type === "circle") endpoints.add(e.center);
  }
  for (const e of sketch.entities) {
    switch (e.type) {
      case "line": {
        const a = points.get(e.start), b = points.get(e.end);
        if (!a || !b) break;
        lines.push("0", "LINE", "8", layer(e.id), "10", num(a.x), "20", num(a.y), "30", "0", "11", num(b.x), "21", num(b.y), "31", "0");
        break;
      }
      case "circle": {
        const c = points.get(e.center);
        if (!c) break;
        lines.push("0", "CIRCLE", "8", layer(e.id), "10", num(c.x), "20", num(c.y), "30", "0", "40", num(e.radius));
        break;
      }
      case "arc": {
        const c = points.get(e.center), s = points.get(e.start), t = points.get(e.end);
        if (!c || !s || !t) break;
        const r = Math.hypot(s.x - c.x, s.y - c.y);
        lines.push("0", "ARC", "8", layer(e.id), "10", num(c.x), "20", num(c.y), "30", "0", "40", num(r), "50", num(deg(Math.atan2(s.y - c.y, s.x - c.x))), "51", num(deg(Math.atan2(t.y - c.y, t.x - c.x))));
        break;
      }
      case "point": {
        // Free points only; endpoints and centres are implied by their curves.
        if (endpoints.has(e.id)) break;
        lines.push("0", "POINT", "8", layer(e.id), "10", num(e.pos.x), "20", num(e.pos.y), "30", "0");
        break;
      }
    }
  }
  lines.push("0", "ENDSEC", "0", "EOF");
  return lines.join("\r\n") + "\r\n";
}

// ---- drawings

/** A named view with its lines, as the kernel projects it. */
/** Where a section's cutting plane shows edge-on in another view, as a line across that view at `at` (view millimetres). */
export type SectionTrace = { on: string; horizontal: boolean; at: number; label: string; /** +1 when the removed side is towards larger coordinates, -1 otherwise. */ towards: number };
export type DrawingView = { name: string; lines: ViewLines; /** Closed outlines of cut faces (section views), hatched on the sheet. */ cut?: Vec2[][]; trace?: SectionTrace };

/**
 * Hatch lines at 45° with the given spacing clipped to the polygons by the
 * even-odd rule, so holes in a cut face stay clear. Segments are in the
 * polygons' coordinates.
 */
export function hatch(polys: Vec2[][], spacing: number): [Vec2, Vec2][] {
  const out: [Vec2, Vec2][] = [];
  if (polys.length === 0 || spacing <= 0) return out;
  // Rotate by -45°: hatch lines become horizontal lines v = const.
  const c = Math.SQRT1_2;
  const rot = (p: Vec2): Vec2 => ({ x: (p.x + p.y) * c, y: (p.y - p.x) * c });
  const unrot = (p: Vec2): Vec2 => ({ x: (p.x - p.y) * c, y: (p.x + p.y) * c });
  const rp = polys.map((poly) => poly.map(rot));
  let minv = Infinity, maxv = -Infinity;
  for (const poly of rp) for (const p of poly) { minv = Math.min(minv, p.y); maxv = Math.max(maxv, p.y); }
  if (!Number.isFinite(minv)) return out;
  for (let v = Math.ceil(minv / spacing) * spacing; v < maxv; v += spacing) {
    const xs: number[] = [];
    for (const poly of rp) {
      for (let i = 0; i < poly.length; i++) {
        const a = poly[i]!, b = poly[(i + 1) % poly.length]!;
        // Half-open rule on y so a vertex exactly on the line counts once.
        if ((a.y <= v && b.y > v) || (b.y <= v && a.y > v)) xs.push(a.x + ((v - a.y) / (b.y - a.y)) * (b.x - a.x));
      }
    }
    xs.sort((p, q) => p - q);
    for (let i = 0; i + 1 < xs.length; i += 2) {
      if (xs[i + 1]! - xs[i]! > 1e-9) out.push([unrot({ x: xs[i]!, y: v }), unrot({ x: xs[i + 1]!, y: v })]);
    }
  }
  return out;
}

type Bounds = { minx: number; miny: number; maxx: number; maxy: number };
type Placed = DrawingView & { dx: number; dy: number; b: Bounds };

/** An overall dimension: a measured span with its dimension line offset from the view. */
type Dimension = {
  /** Endpoints of the measured span, in sheet millimetres (view coordinates plus offset). */
  a: { x: number; y: number };
  b: { x: number; y: number };
  /** Where the dimension line runs: a signed offset perpendicular to the span. */
  offset: number;
  value: number;
};

function boundsOf(v: ViewLines): Bounds {
  let minx = Infinity, miny = Infinity, maxx = -Infinity, maxy = -Infinity;
  for (const [a, b] of [...v.visible, ...v.hidden]) {
    for (const p of [a, b]) {
      minx = Math.min(minx, p.x); miny = Math.min(miny, p.y);
      maxx = Math.max(maxx, p.x); maxy = Math.max(maxy, p.y);
    }
  }
  return Number.isFinite(minx) ? { minx, miny, maxx, maxy } : { minx: 0, miny: 0, maxx: 0, maxy: 0 };
}

/** Distance from the view outline to a dimension line, and its extension lines' overshoot. */
const DIM_OFFSET = 10;
const DIM_OVERSHOOT = 2;

/**
 * Third-angle layout in model millimetres: front at the origin, top above
 * it, right to its right, isometric top right. Overall dimensions go
 * below and left of the front view (width, height) and left of the top
 * view (depth). Views are positioned by name; unknown names are stacked
 * to the right.
 */
function layout(views: DrawingView[], gap = 15): { placed: Placed[]; dims: Dimension[]; min: { x: number; y: number }; max: { x: number; y: number } } {
  const by = (name: string) => views.find((v) => v.name === name);
  const placed: Placed[] = [];
  const front = by("front");
  const fb = front ? boundsOf(front.lines) : { minx: 0, miny: 0, maxx: 0, maxy: 0 };
  // Room on the left of the front and top views for their vertical dimensions.
  const dimGap = DIM_OFFSET + 8;
  if (front) placed.push({ ...front, dx: 0, dy: 0, b: fb });
  const top = by("top");
  if (top) {
    const tb = boundsOf(top.lines);
    placed.push({ ...top, dx: 0, dy: fb.maxy + gap + dimGap - tb.miny, b: tb });
  }
  const right = by("right");
  if (right) {
    const rb = boundsOf(right.lines);
    placed.push({ ...right, dx: fb.maxx + gap - rb.minx, dy: 0, b: rb });
  }
  let cursorX = Math.max(fb.maxx, ...placed.map((p) => p.b.maxx + p.dx)) + gap;
  for (const v of views) {
    if (["front", "top", "right"].includes(v.name)) continue;
    const vb = boundsOf(v.lines);
    const dy = v.name === "iso" && top ? fb.maxy + gap + dimGap - vb.miny : -vb.miny;
    placed.push({ ...v, dx: cursorX - vb.minx, dy, b: vb });
    cursorX += vb.maxx - vb.minx + gap;
  }
  // Overall dimensions: width below the front view, height left of it,
  // depth left of the top view.
  const dims: Dimension[] = [];
  const span = (p: Placed) => ({ w: p.b.maxx - p.b.minx, h: p.b.maxy - p.b.miny });
  for (const p of placed) {
    if (p.name === "front" && span(p).w > 0) {
      dims.push({ a: { x: p.b.minx + p.dx, y: p.b.miny + p.dy }, b: { x: p.b.maxx + p.dx, y: p.b.miny + p.dy }, offset: -DIM_OFFSET, value: span(p).w });
      if (span(p).h > 0) dims.push({ a: { x: p.b.minx + p.dx, y: p.b.miny + p.dy }, b: { x: p.b.minx + p.dx, y: p.b.maxy + p.dy }, offset: DIM_OFFSET, value: span(p).h });
    }
    if (p.name === "top" && span(p).h > 0) {
      dims.push({ a: { x: p.b.minx + p.dx, y: p.b.miny + p.dy }, b: { x: p.b.minx + p.dx, y: p.b.maxy + p.dy }, offset: DIM_OFFSET, value: span(p).h });
    }
  }
  let min = { x: Infinity, y: Infinity }, max = { x: -Infinity, y: -Infinity };
  for (const p of placed) {
    min = { x: Math.min(min.x, p.b.minx + p.dx), y: Math.min(min.y, p.b.miny + p.dy) };
    max = { x: Math.max(max.x, p.b.maxx + p.dx), y: Math.max(max.y, p.b.maxy + p.dy) };
  }
  if (!Number.isFinite(min.x)) { min = { x: 0, y: 0 }; max = { x: 0, y: 0 }; }
  if (dims.length > 0) {
    // The dimensions sit up to an offset plus text height outside the views.
    min = { x: min.x - dimGap, y: min.y - dimGap };
  }
  return { placed, dims, min, max };
}

/** Cutting-plane traces in sheet coordinates: across the view they cut, extended past its outline. */
function traces(placed: Placed[]): { a: Vec2; b: Vec2; label: string; labelOffset: Vec2 }[] {
  const out: { a: Vec2; b: Vec2; label: string; labelOffset: Vec2 }[] = [];
  for (const p of placed) {
    const t = p.trace;
    if (!t) continue;
    const on = placed.find((q) => q.name === t.on);
    if (!on) continue;
    const ext = 5;
    if (t.horizontal) {
      const y = t.at + on.dy;
      out.push({ a: { x: on.b.minx + on.dx - ext, y }, b: { x: on.b.maxx + on.dx + ext, y }, label: t.label, labelOffset: { x: 0, y: 3 * t.towards } });
    } else {
      const x = t.at + on.dx;
      out.push({ a: { x, y: on.b.miny + on.dy - ext }, b: { x, y: on.b.maxy + on.dy + ext }, label: t.label, labelOffset: { x: 3 * t.towards, y: 0 } });
    }
  }
  return out;
}

/** Number text for a dimension: up to two decimals, no trailing zeros. */
function dimText(v: number): string {
  return v.toFixed(2).replace(/\.?0+$/, "");
}

/**
 * Geometry of one dimension in sheet millimetres: the two extension lines,
 * the dimension line, its arrowheads (as triangles) and the text anchor
 * with its rotation in degrees (0 for horizontal spans, 90 for vertical).
 */
function dimensionGeometry(d: Dimension): { lines: [{ x: number; y: number }, { x: number; y: number }][]; arrows: { x: number; y: number }[][]; text: { x: number; y: number; angle: number } } {
  const dx = d.b.x - d.a.x, dy = d.b.y - d.a.y;
  const len = Math.hypot(dx, dy) || 1;
  const u = { x: dx / len, y: dy / len };
  const n = { x: -u.y, y: u.x }; // left normal of the span
  const at = (p: { x: number; y: number }, along: number, across: number) => ({ x: p.x + u.x * along + n.x * across, y: p.y + u.y * along + n.y * across });
  const over = d.offset + Math.sign(d.offset) * DIM_OVERSHOOT;
  const lines: [{ x: number; y: number }, { x: number; y: number }][] = [
    [at(d.a, 0, 0), at(d.a, 0, over)],
    [at(d.b, 0, 0), at(d.b, 0, over)],
    [at(d.a, 0, d.offset), at(d.b, 0, d.offset)],
  ];
  const head = 2.5, half = 0.8;
  const arrows = [
    [at(d.a, 0, d.offset), at(d.a, head, d.offset + half), at(d.a, head, d.offset - half)],
    [at(d.b, 0, d.offset), at(d.b, -head, d.offset + half), at(d.b, -head, d.offset - half)],
  ];
  const mid = at(d.a, len / 2, d.offset + (d.offset < 0 ? -1.2 : 1.2));
  const angle = Math.abs(u.x) >= Math.abs(u.y) ? 0 : 90;
  return { lines, arrows, text: { x: mid.x, y: mid.y, angle } };
}

const STANDARD_SCALES = [10, 5, 2, 1, 1 / 2, 1 / 5, 1 / 10, 1 / 20, 1 / 50, 1 / 100];

function scaleLabel(s: number): string {
  return s >= 1 ? `${Math.round(s)}:1` : `1:${Math.round(1 / s)}`;
}

/**
 * An A4 landscape drawing sheet (SVG, millimetre units) with the views in
 * third-angle layout at the largest standard scale that fits, hidden
 * lines dashed, and a title block.
 */
/** Sheet sizes in millimetres, landscape. */
export const SHEETS = { A4: { w: 297, h: 210 }, A3: { w: 420, h: 297 }, A2: { w: 594, h: 420 }, Letter: { w: 279.4, h: 215.9 } } as const;
export type SheetSize = keyof typeof SHEETS;

export function toDrawingSvg(views: DrawingView[], title: string, size: SheetSize = "A4"): string {
  const sheet = { ...SHEETS[size], margin: 10, block: 24 };
  const { placed, dims, min, max } = layout(views);
  const availW = sheet.w - 2 * sheet.margin;
  const availH = sheet.h - 2 * sheet.margin - sheet.block;
  const extentW = Math.max(max.x - min.x, 1e-9), extentH = Math.max(max.y - min.y, 1e-9);
  const fit = Math.min(availW / extentW, availH / extentH);
  const scale = STANDARD_SCALES.find((s) => s <= fit) ?? STANDARD_SCALES[STANDARD_SCALES.length - 1]!;
  // Centre the drawing in the free area; SVG y points down.
  const ox = sheet.margin + (availW - extentW * scale) / 2 - min.x * scale;
  const oy = sheet.margin + (availH - extentH * scale) / 2 + max.y * scale;
  const X = (x: number) => (ox + x * scale).toFixed(3);
  const Y = (y: number) => (oy - y * scale).toFixed(3);
  const out: string[] = [];
  out.push(`<svg xmlns="http://www.w3.org/2000/svg" width="${sheet.w}mm" height="${sheet.h}mm" viewBox="0 0 ${sheet.w} ${sheet.h}">`);
  out.push(`<rect x="0" y="0" width="${sheet.w}" height="${sheet.h}" fill="white"/>`);
  out.push(`<rect x="${sheet.margin}" y="${sheet.margin}" width="${sheet.w - 2 * sheet.margin}" height="${sheet.h - 2 * sheet.margin}" fill="none" stroke="black" stroke-width="0.5"/>`);
  for (const p of placed) {
    out.push(`<g id="view-${escapeXml(p.name)}">`);
    const seg = (a: { x: number; y: number }, b: { x: number; y: number }) => `M${X(a.x + p.dx)} ${Y(a.y + p.dy)}L${X(b.x + p.dx)} ${Y(b.y + p.dy)}`;
    if (p.lines.hidden.length > 0) {
      out.push(`<path class="hidden" fill="none" stroke="black" stroke-width="0.25" stroke-dasharray="2 1" d="${p.lines.hidden.map(([a, b]) => seg(a, b)).join("")}"/>`);
    }
    if (p.lines.visible.length > 0) {
      out.push(`<path class="visible" fill="none" stroke="black" stroke-width="0.5" stroke-linecap="round" d="${p.lines.visible.map(([a, b]) => seg(a, b)).join("")}"/>`);
    }
    if (p.cut && p.cut.length > 0) {
      const lines = hatch(p.cut, 3 / scale);
      if (lines.length > 0) out.push(`<path class="hatch" fill="none" stroke="black" stroke-width="0.18" d="${lines.map(([a, b]) => seg(a, b)).join("")}"/>`);
    }
    if (p.name === "section") {
      const label = p.trace ? `SECTION ${p.trace.label}-${p.trace.label}` : "SECTION";
      out.push(`<text class="caption" x="${X((p.b.minx + p.b.maxx) / 2 + p.dx)}" y="${Y(p.b.miny + p.dy - 6)}" font-family="Helvetica, Arial, sans-serif" font-size="3.5" fill="black" text-anchor="middle">${label}</text>`);
    }
    out.push(`</g>`);
  }
  // Cutting-plane traces: a chain line across the view the plane cuts edge-on, lettered at both ends.
  for (const t of traces(placed)) {
    out.push(`<g class="trace">`);
    out.push(`<path fill="none" stroke="black" stroke-width="0.5" stroke-dasharray="6 1.5 1 1.5" d="M${X(t.a.x)} ${Y(t.a.y)}L${X(t.b.x)} ${Y(t.b.y)}"/>`);
    for (const e of [t.a, t.b]) out.push(`<text x="${X(e.x + t.labelOffset.x)}" y="${Y(e.y + t.labelOffset.y)}" font-family="Helvetica, Arial, sans-serif" font-size="3.5" fill="black" text-anchor="middle" dominant-baseline="middle">${t.label}</text>`);
    out.push(`</g>`);
  }
  // Overall dimensions.
  for (const d of dims) {
    const g = dimensionGeometry(d);
    out.push(`<g class="dimension">`);
    out.push(`<path fill="none" stroke="black" stroke-width="0.18" d="${g.lines.map(([a, b]) => `M${X(a.x)} ${Y(a.y)}L${X(b.x)} ${Y(b.y)}`).join("")}"/>`);
    for (const tri of g.arrows) out.push(`<polygon fill="black" points="${tri.map((p) => `${X(p.x)},${Y(p.y)}`).join(" ")}"/>`);
    const tx = X(g.text.x), ty = Y(g.text.y);
    out.push(`<text x="${tx}" y="${ty}" font-family="Helvetica, Arial, sans-serif" font-size="3" fill="black" text-anchor="middle" dominant-baseline="middle"${g.text.angle ? ` transform="rotate(-90 ${tx} ${ty})"` : ""}>${dimText(d.value)}</text>`);
    out.push(`</g>`);
  }
  // Title block along the bottom edge.
  const by = sheet.h - sheet.margin - sheet.block;
  out.push(`<rect x="${sheet.margin}" y="${by}" width="${sheet.w - 2 * sheet.margin}" height="${sheet.block}" fill="none" stroke="black" stroke-width="0.5"/>`);
  out.push(`<line x1="${sheet.w / 2}" y1="${by}" x2="${sheet.w / 2}" y2="${sheet.h - sheet.margin}" stroke="black" stroke-width="0.35"/>`);
  const t = (x: number, y: number, text: string, size = 4) => out.push(`<text x="${x}" y="${y}" font-family="Helvetica, Arial, sans-serif" font-size="${size}" fill="black">${escapeXml(text)}</text>`);
  t(sheet.margin + 4, by + 9, title, 6);
  t(sheet.margin + 4, by + 18, `Views: ${placed.map((p) => p.name).join(", ")} · third angle · mm · ${size}`, 3.5);
  t(sheet.w / 2 + 4, by + 9, `Scale ${scaleLabel(scale)}`);
  t(sheet.w / 2 + 4, by + 18, `${new Date().toISOString().slice(0, 10)} · offkilter`, 3.5);
  out.push(`</svg>`);
  return out.join("\n") + "\n";
}

/** The same layout as a DXF at 1:1 in millimetres, hidden lines on their own layer. */
export function toDrawingDxf(views: DrawingView[]): string {
  const { placed, dims } = layout(views);
  const lines = dxfHead([["VISIBLE", 7, "CONTINUOUS"], ["HIDDEN", 8, "DASHED"], ["DIMENSIONS", 3, "CONTINUOUS"], ["SECTION", 1, "CONTINUOUS"]]);
  for (const t of traces(placed)) {
    lines.push("0", "LINE", "8", "SECTION", "10", fmt(t.a.x), "20", fmt(t.a.y), "30", "0", "11", fmt(t.b.x), "21", fmt(t.b.y), "31", "0");
    for (const e of [t.a, t.b]) lines.push("0", "TEXT", "8", "SECTION", "10", fmt(e.x + t.labelOffset.x), "20", fmt(e.y + t.labelOffset.y), "30", "0", "40", "3.5", "72", "1", "11", fmt(e.x + t.labelOffset.x), "21", fmt(e.y + t.labelOffset.y), "31", "0", "1", t.label);
  }
  for (const d of dims) {
    const g = dimensionGeometry(d);
    for (const [a, b] of g.lines) lines.push("0", "LINE", "8", "DIMENSIONS", "10", fmt(a.x), "20", fmt(a.y), "30", "0", "11", fmt(b.x), "21", fmt(b.y), "31", "0");
    for (const tri of g.arrows) {
      for (let i = 0; i < 3; i++) {
        const a = tri[i]!, b = tri[(i + 1) % 3]!;
        lines.push("0", "LINE", "8", "DIMENSIONS", "10", fmt(a.x), "20", fmt(a.y), "30", "0", "11", fmt(b.x), "21", fmt(b.y), "31", "0");
      }
    }
    lines.push("0", "TEXT", "8", "DIMENSIONS", "10", fmt(g.text.x), "20", fmt(g.text.y), "30", "0", "40", "3", "50", String(g.text.angle), "72", "1", "11", fmt(g.text.x), "21", fmt(g.text.y), "31", "0", "1", dimText(d.value));
  }
  for (const p of placed) {
    const add = (layer: string, segs: [{ x: number; y: number }, { x: number; y: number }][]) => {
      for (const [a, b] of segs) lines.push("0", "LINE", "8", layer, "10", fmt(a.x + p.dx), "20", fmt(a.y + p.dy), "30", "0", "11", fmt(b.x + p.dx), "21", fmt(b.y + p.dy), "31", "0");
    };
    add("VISIBLE", p.lines.visible);
    add("HIDDEN", p.lines.hidden);
    if (p.cut && p.cut.length > 0) add("SECTION", hatch(p.cut, 3));
    if (p.name === "section") {
      const x = (p.b.minx + p.b.maxx) / 2 + p.dx, y = p.b.miny + p.dy - 6;
      lines.push("0", "TEXT", "8", "SECTION", "10", fmt(x), "20", fmt(y), "30", "0", "40", "3.5", "72", "1", "11", fmt(x), "21", fmt(y), "31", "0", "1", p.trace ? `SECTION ${p.trace.label}-${p.trace.label}` : "SECTION");
    }
  }
  lines.push("0", "ENDSEC", "0", "EOF");
  return lines.join("\r\n") + "\r\n";
}

/** A bill of materials for the current tab as CSV: one row per body (part studio) or per instance (assembly). */
export function toBom(summary: { kind: string; name: string; bodies: BodySummary[]; instances: { name: string; studio: number; body: number; body_indices: number[] }[]; tabs: { id: number; name: string }[] }): string {
  const esc = (v: string | number) => {
    const s = String(v);
    return /[",\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
  };
  const rows: (string | number)[][] = [];
  if (summary.kind === "assembly") {
    rows.push(["instance", "source", "body", "volume_mm3", "surface_mm2", "size_mm"]);
    for (const inst of summary.instances) {
      const tab = summary.tabs.find((t) => t.id === inst.studio)?.name ?? `tab ${inst.studio}`;
      for (const i of inst.body_indices) {
        const b = summary.bodies[i];
        if (b) rows.push([inst.name, tab, b.name, b.volume.toFixed(3), b.area.toFixed(3), sizeOf(b)]);
      }
      if (inst.body_indices.length === 0) rows.push([inst.name, tab, "", "", "", ""]);
    }
  } else {
    rows.push(["part", "volume_mm3", "surface_mm2", "size_mm", "faces"]);
    for (const b of summary.bodies) rows.push([b.name, b.volume.toFixed(3), b.area.toFixed(3), sizeOf(b), b.face_count]);
  }
  return rows.map((r) => r.map(esc).join(",")).join("\n") + "\n";
}

function sizeOf(b: BodySummary): string {
  if (!b.bounds) return "";
  const [lo, hi] = b.bounds;
  return [hi.x - lo.x, hi.y - lo.y, hi.z - lo.z].map((v) => v.toFixed(2)).join(" x ");
}
