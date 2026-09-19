// File exporters: STL and 3MF for bodies, DXF for sketches. All are
// written by hand so the client carries no extra dependencies.

import type { BodyMesh, BodySummary, SketchData } from "./kernel";

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

/**
 * A sketch as a DXF (R12 subset: LINE, CIRCLE, ARC, POINT) in sketch-plane
 * coordinates. Construction entities go on a separate layer.
 */
export function toDxf(sketch: SketchData): string {
  const points = new Map<number, { x: number; y: number }>();
  for (const e of sketch.entities) if (e.type === "point") points.set(e.id, e.pos);
  const construction = new Set(sketch.construction ?? []);
  const lines: string[] = ["0", "SECTION", "2", "HEADER", "9", "$INSUNITS", "70", "4", "0", "ENDSEC", "0", "SECTION", "2", "TABLES", "0", "TABLE", "2", "LAYER", "70", "2"];
  lines.push("0", "LAYER", "2", "SKETCH", "70", "0", "62", "7", "6", "CONTINUOUS");
  lines.push("0", "LAYER", "2", "CONSTRUCTION", "70", "0", "62", "8", "6", "DASHED");
  lines.push("0", "ENDTAB", "0", "ENDSEC", "0", "SECTION", "2", "ENTITIES");
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
