// Mesh readers: STL (binary or ASCII) and Wavefront OBJ, welding shared
// vertices so the kernel sees a connected mesh (STL repeats every corner
// per triangle).

import type { Vec3 } from "./kernel";

export type Mesh = { vertices: Vec3[]; triangles: [number, number, number][] };

/** Parses an STL file (binary or ASCII) into welded vertices and triangles. */
export function parseStl(data: ArrayBuffer): Mesh {
  const bytes = new Uint8Array(data);
  const head = new TextDecoder("latin1").decode(bytes.subarray(0, Math.min(bytes.length, 512)));
  // Binary files carry a triangle count that matches their size; ASCII ones start with "solid".
  if (bytes.length >= 84) {
    const count = new DataView(data).getUint32(80, true);
    if (84 + count * 50 === bytes.length) return weld(readBinary(data, count));
  }
  if (head.trimStart().startsWith("solid")) return weld(readAscii(new TextDecoder("latin1").decode(bytes)));
  throw new Error("not an STL file");
}

function readBinary(data: ArrayBuffer, count: number): Vec3[] {
  const view = new DataView(data);
  const pts: Vec3[] = [];
  let at = 84;
  for (let i = 0; i < count; i++) {
    at += 12; // normal
    for (let k = 0; k < 3; k++) {
      pts.push({ x: view.getFloat32(at, true), y: view.getFloat32(at + 4, true), z: view.getFloat32(at + 8, true) });
      at += 12;
    }
    at += 2; // attribute byte count
  }
  return pts;
}

function readAscii(text: string): Vec3[] {
  const pts: Vec3[] = [];
  const re = /vertex\s+([-+\d.eE]+)\s+([-+\d.eE]+)\s+([-+\d.eE]+)/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) pts.push({ x: Number(m[1]), y: Number(m[2]), z: Number(m[3]) });
  if (pts.length % 3 !== 0) throw new Error("ASCII STL with a partial facet");
  return pts;
}

/** Merges corners closer than a millionth of the mesh size; drops degenerate triangles. */
function weld(corners: Vec3[]): Mesh {
  let size = 0;
  for (const p of corners) size = Math.max(size, Math.abs(p.x), Math.abs(p.y), Math.abs(p.z));
  const q = Math.max(size, 1) * 1e-6;
  const index = new Map<string, number>();
  const vertices: Vec3[] = [];
  const idOf = (p: Vec3) => {
    const key = `${Math.round(p.x / q)},${Math.round(p.y / q)},${Math.round(p.z / q)}`;
    let id = index.get(key);
    if (id === undefined) {
      id = vertices.length;
      vertices.push(p);
      index.set(key, id);
    }
    return id;
  };
  const triangles: [number, number, number][] = [];
  for (let i = 0; i + 2 < corners.length; i += 3) {
    const t: [number, number, number] = [idOf(corners[i]!), idOf(corners[i + 1]!), idOf(corners[i + 2]!)];
    if (t[0] !== t[1] && t[1] !== t[2] && t[0] !== t[2]) triangles.push(t);
  }
  if (triangles.length === 0) throw new Error("the STL has no triangles");
  return { vertices, triangles };
}

/** Parses a Wavefront OBJ: `v` lines and `f` faces (polygons are fanned into triangles; texture and normal indices are ignored). */
export function parseObj(text: string): Mesh {
  const verts: Vec3[] = [];
  const corners: Vec3[] = [];
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim();
    if (line.startsWith("v ")) {
      const [x, y, z] = line.slice(2).trim().split(/\s+/).map(Number);
      verts.push({ x: x ?? 0, y: y ?? 0, z: z ?? 0 });
    } else if (line.startsWith("f ")) {
      const idx = line.slice(2).trim().split(/\s+/).map((tok) => {
        const i = Number(tok.split("/")[0]);
        return i < 0 ? verts.length + i : i - 1;
      });
      for (let k = 1; k + 1 < idx.length; k++) {
        for (const i of [idx[0]!, idx[k]!, idx[k + 1]!]) {
          const v = verts[i];
          if (!v) throw new Error("OBJ face refers to a missing vertex");
          corners.push(v);
        }
      }
    }
  }
  if (corners.length === 0) throw new Error("the OBJ has no faces");
  return weld(corners);
}
