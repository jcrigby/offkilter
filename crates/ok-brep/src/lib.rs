//! Boundary representation solids.
//!
//! A [`Solid`] is a closed, oriented set of planar polygonal faces sharing
//! vertices. Curved surfaces (cylinders from extruded arcs) are represented
//! by many planar facets that all reference the same analytic [`Surface`],
//! so they shade smoothly, select as one face, and can be replaced by exact
//! geometry later without changing the topology model.
//!
//! Faces are stored as loops of vertex indices: loop 0 is the outer boundary,
//! counter-clockwise when viewed from the face normal; later loops are holes,
//! clockwise. Edges are implicit (consecutive loop vertices) and every edge
//! of a valid solid is shared by exactly two faces in opposite directions.

mod blend;
mod boolean;
mod extrude;
mod revolve;
mod section;
mod tessellate;
mod transform;

pub use blend::{blend_edges, BlendKind};
pub use boolean::{boolean, BoolOp};
pub use extrude::extrude;
pub use revolve::revolve;
pub use tessellate::{display_edges, tessellate, tessellate_with_faces, DisplayEdge};
pub use transform::Transform;

use ok_math::{Plane, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, thiserror::Error)]
pub enum BrepError {
    #[error("result is not a closed solid: {0}")]
    NonManifold(String),
    #[error("cross-section of solid is not closed ({0})")]
    OpenSection(String),
    #[error("{0}")]
    Degenerate(String),
}

/// Analytic surface a face lies on.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Surface {
    Plane {
        normal: Vec3,
        offset: f64,
    },
    /// Infinite cylinder about the line through `origin` along unit `axis`.
    Cylinder {
        origin: Vec3,
        axis: Vec3,
        radius: f64,
    },
    /// A surface of revolution about the line through `origin` along unit
    /// `axis`; facets sharing it are shaded with averaged normals.
    Revolved {
        origin: Vec3,
        axis: Vec3,
    },
}

impl Surface {
    /// Whether facets on this surface should shade smoothly.
    pub fn is_smooth(&self) -> bool {
        !matches!(self, Surface::Plane { .. })
    }
}

/// Where a face came from, for persistent naming across regenerations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FaceOrigin {
    pub feature: u32,
    pub local: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Face {
    /// Right-handed frame on the face; `normal` points out of the solid.
    pub plane: Plane,
    /// Vertex loops; `loops[0]` is the outer boundary.
    pub loops: Vec<Vec<u32>>,
    pub surface: usize,
    pub origin: FaceOrigin,
}

/// A polygon in 3D used to build solids: loops of positions plus metadata.
#[derive(Debug, Clone)]
pub struct Polygon {
    pub plane: Plane,
    pub loops: Vec<Vec<Vec3>>,
    pub surface: usize,
    pub origin: FaceOrigin,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Solid {
    pub vertices: Vec<Vec3>,
    pub faces: Vec<Face>,
    pub surfaces: Vec<Surface>,
}

/// Undirected edge key.
pub type EdgeKey = (u32, u32);

pub fn edge_key(a: u32, b: u32) -> EdgeKey {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

/// Positional tolerance used when merging vertices; scaled by model size.
pub fn merge_tolerance(diag: f64) -> f64 {
    (diag * 1e-9).max(1e-6)
}

impl Solid {
    pub fn is_empty(&self) -> bool {
        self.faces.is_empty()
    }

    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        bounds_of(self.vertices.iter().copied())
    }

    /// Iterates every directed edge as `(from, to, face index)`.
    pub fn directed_edges(&self) -> impl Iterator<Item = (u32, u32, usize)> + '_ {
        self.faces.iter().enumerate().flat_map(|(fi, f)| {
            f.loops.iter().flat_map(move |l| {
                let n = l.len();
                (0..n).map(move |i| (l[i], l[(i + 1) % n], fi))
            })
        })
    }

    /// Map from undirected edge to the faces using it.
    pub fn edge_faces(&self) -> HashMap<EdgeKey, Vec<usize>> {
        let mut m: HashMap<EdgeKey, Vec<usize>> = HashMap::new();
        for (a, b, f) in self.directed_edges() {
            m.entry(edge_key(a, b)).or_default().push(f);
        }
        m
    }

    /// Checks that the faces form a closed surface: every edge is used an
    /// even number of times with balanced orientation. Normally that is
    /// exactly twice; lumps that touch along an edge share it four times.
    pub fn validate(&self) -> Result<(), BrepError> {
        let mut dir: HashMap<EdgeKey, (i32, usize)> = HashMap::new();
        for (a, b, _) in self.directed_edges() {
            if a == b {
                return Err(BrepError::NonManifold(format!(
                    "degenerate edge at vertex {a}"
                )));
            }
            let e = dir.entry(edge_key(a, b)).or_insert((0, 0));
            e.0 += if a < b { 1 } else { -1 };
            e.1 += 1;
        }
        let mut bad = 0;
        let mut example = None;
        for (k, (sum, count)) in &dir {
            if *count % 2 != 0 || *sum != 0 {
                bad += 1;
                if example.is_none() {
                    example = Some((*k, *count, *sum));
                }
            }
        }
        if bad > 0 {
            let (k, count, sum) = example.unwrap();
            return Err(BrepError::NonManifold(format!(
                "{bad} bad edge(s); e.g. edge {:?} used {count} time(s), orientation sum {sum}, from {:?} to {:?}, faces {:?}",
                k,
                self.vertices[k.0 as usize],
                self.vertices[k.1 as usize],
                self.directed_edges().filter(|(a, b, _)| edge_key(*a, *b) == k).map(|(a, b, f)| (a, b, self.faces[f].origin, self.faces[f].plane.normal)).collect::<Vec<_>>()
            )));
        }
        for f in &self.faces {
            if f.loops.is_empty() || f.loops[0].len() < 3 {
                return Err(BrepError::NonManifold(
                    "face with fewer than three vertices".into(),
                ));
            }
        }
        Ok(())
    }

    /// Signed volume by the divergence theorem (positive for outward normals).
    pub fn volume(&self) -> f64 {
        let mesh = tessellate(self);
        mesh.signed_volume()
    }

    /// Builds a solid from polygons, merging coincident vertices, inserting
    /// vertices that lie on edges of other polygons (T-junctions), dropping
    /// degenerate loops, and validating closure.
    pub fn from_polygons(polys: Vec<Polygon>, surfaces: Vec<Surface>) -> Result<Solid, BrepError> {
        let all = polys.iter().flat_map(|p| p.loops.iter().flatten().copied());
        let Some((min, max)) = bounds_of(all) else {
            return Ok(Solid::default());
        };
        let tol = merge_tolerance((max - min).length());
        let mut merger = VertexMerger::new(tol);
        let mut faces = Vec::new();
        for p in polys {
            let mut loops = Vec::new();
            for l in &p.loops {
                let mut ids: Vec<u32> = Vec::with_capacity(l.len());
                for &v in l {
                    let id = merger.insert(v);
                    if ids.last() != Some(&id) {
                        ids.push(id);
                    }
                }
                while ids.len() > 1 && ids.first() == ids.last() {
                    ids.pop();
                }
                if ids.len() >= 3 {
                    loops.push(ids);
                }
            }
            if !loops.is_empty() {
                faces.push(Face {
                    plane: p.plane,
                    loops,
                    surface: p.surface,
                    origin: p.origin,
                });
            }
        }
        let mut solid = Solid {
            vertices: merger.points,
            faces,
            surfaces,
        };
        solid.repair_t_junctions(tol);
        solid.remove_spikes(tol);
        solid.remove_degenerate_faces();
        solid.compact_surfaces();
        solid.validate()?;
        Ok(solid)
    }

    /// Inserts any vertex lying strictly inside an edge into that edge.
    fn repair_t_junctions(&mut self, tol: f64) {
        let grid = PointGrid::new(&self.vertices, tol);
        let verts = self.vertices.clone();
        for f in &mut self.faces {
            for l in &mut f.loops {
                let mut out: Vec<u32> = Vec::with_capacity(l.len());
                let n = l.len();
                for i in 0..n {
                    let a = l[i];
                    let b = l[(i + 1) % n];
                    out.push(a);
                    let (pa, pb) = (verts[a as usize], verts[b as usize]);
                    let d = pb - pa;
                    let len2 = d.length_squared();
                    if len2 == 0.0 {
                        continue;
                    }
                    let mut on_edge: Vec<(f64, u32)> = Vec::new();
                    for c in grid.candidates_near_segment(pa, pb) {
                        if c == a || c == b {
                            continue;
                        }
                        let pc = verts[c as usize];
                        let t = (pc - pa).dot(d) / len2;
                        if t <= 0.0 || t >= 1.0 {
                            continue;
                        }
                        let dist = (pc - (pa + d * t)).length();
                        if dist <= tol {
                            on_edge.push((t, c));
                        }
                    }
                    on_edge.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap());
                    for (_, c) in on_edge {
                        if out.last() != Some(&c) {
                            out.push(c);
                        }
                    }
                }
                out.dedup();
                while out.len() > 1 && out.first() == out.last() {
                    out.pop();
                }
                *l = out;
            }
        }
    }

    /// Removes zero-width excursions from loops: a vertex `b` between `a` and
    /// `c` where the path doubles back along itself (`c` on segment `a-b` or
    /// `a` on segment `b-c`). These arise when a clipped fragment carries a
    /// hairline sliver whose vertices merged with the main boundary.
    fn remove_spikes(&mut self, tol: f64) {
        let verts = &self.vertices;
        let on_segment = |p: Vec3, a: Vec3, b: Vec3| -> bool {
            let d = b - a;
            let len2 = d.length_squared();
            if len2 == 0.0 {
                return p.distance(a) <= tol;
            }
            let t = (p - a).dot(d) / len2;
            if !(-1e-9..=1.0 + 1e-9).contains(&t) {
                return false;
            }
            (p - (a + d * t)).length() <= tol
        };
        for f in &mut self.faces {
            for l in &mut f.loops {
                loop {
                    let n = l.len();
                    if n < 3 {
                        break;
                    }
                    let mut removed = false;
                    for i in 0..n {
                        let (a, b, c) = (l[(i + n - 1) % n], l[i], l[(i + 1) % n]);
                        let (pa, pb, pc) =
                            (verts[a as usize], verts[b as usize], verts[c as usize]);
                        let spike = a == c
                            || b == a
                            || b == c
                            || on_segment(pc, pa, pb)
                            || on_segment(pa, pb, pc);
                        if spike {
                            l.remove(i);
                            removed = true;
                            break;
                        }
                    }
                    if !removed {
                        break;
                    }
                }
            }
        }
    }

    fn remove_degenerate_faces(&mut self) {
        let verts = &self.vertices;
        self.faces.retain(|f| {
            let l = &f.loops[0];
            if l.len() < 3 {
                return false;
            }
            // Area via Newell's method.
            let mut n = Vec3::ZERO;
            for i in 0..l.len() {
                let a = verts[l[i] as usize];
                let b = verts[l[(i + 1) % l.len()] as usize];
                n += a.cross(b);
            }
            n.length() > 1e-14
        });
        for f in &mut self.faces {
            f.loops.retain(|l| l.len() >= 3);
        }
    }

    /// Merges planar faces that lie in the same plane (same normal) and
    /// share an edge, as booleans between flush bodies leave behind. The
    /// merged face keeps the surface and origin of its largest member.
    /// Vertices on the merged boundary that were T-junction splits stay in
    /// place so neighbouring faces remain matched.
    pub fn merge_coplanar_faces(&mut self) {
        let n = self.faces.len();
        if n == 0 {
            return;
        }
        let planar = |f: &Face| matches!(self.surfaces.get(f.surface), Some(Surface::Plane { .. }));
        // Union-find over faces joined by a shared edge with matching planes.
        let mut parent: Vec<usize> = (0..n).collect();
        fn find(p: &mut [usize], i: usize) -> usize {
            let mut r = i;
            while p[r] != r {
                r = p[r];
            }
            let mut c = i;
            while p[c] != r {
                let nx = p[c];
                p[c] = r;
                c = nx;
            }
            r
        }
        let coplanar = |a: &Face, b: &Face| -> bool {
            a.plane.normal.dot(b.plane.normal) > 1.0 - 1e-9
                && (a.plane.normal.dot(b.plane.origin - a.plane.origin)).abs() <= 1e-6
        };
        for (_, faces) in self.edge_faces() {
            if faces.len() != 2 {
                continue;
            }
            let (fa, fb) = (&self.faces[faces[0]], &self.faces[faces[1]]);
            if planar(fa) && planar(fb) && coplanar(fa, fb) {
                let (ra, rb) = (find(&mut parent, faces[0]), find(&mut parent, faces[1]));
                if ra != rb {
                    parent[ra] = rb;
                }
            }
        }
        let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
        for i in 0..n {
            let r = find(&mut parent, i);
            groups.entry(r).or_default().push(i);
        }
        if groups.values().all(|g| g.len() == 1) {
            return;
        }
        let mut new_faces: Vec<Face> = Vec::new();
        let mut merged_any = false;
        let mut sorted_groups: Vec<Vec<usize>> = groups.into_values().collect();
        sorted_groups.sort_by_key(|g| g[0]);
        for group in sorted_groups {
            if group.len() == 1 {
                new_faces.push(self.faces[group[0]].clone());
                continue;
            }
            // Directed edges of all member loops; internal shared edges
            // appear in both directions and cancel out.
            let mut directed: HashMap<(u32, u32), u32> = HashMap::new();
            for &fi in &group {
                for l in &self.faces[fi].loops {
                    for i in 0..l.len() {
                        let (a, b) = (l[i], l[(i + 1) % l.len()]);
                        *directed.entry((a, b)).or_insert(0) += 1;
                    }
                }
            }
            let mut boundary: HashMap<u32, Vec<u32>> = HashMap::new();
            for (&(a, b), &count) in &directed {
                let reverse = directed.get(&(b, a)).copied().unwrap_or(0);
                if count > reverse {
                    for _ in 0..(count - reverse) {
                        boundary.entry(a).or_default().push(b);
                    }
                }
            }
            // Chain into loops. At vertices with several outgoing edges
            // (a boundary touching itself) any consistent choice keeps the
            // loop closed; area sign then classifies outer vs hole.
            let mut loops: Vec<Vec<u32>> = Vec::new();
            while let Some((&start, _)) = boundary.iter().find(|(_, v)| !v.is_empty()) {
                let mut l = vec![start];
                let mut cur = start;
                loop {
                    let Some(next) = boundary.get_mut(&cur).and_then(|v| v.pop()) else {
                        break;
                    };
                    if next == start {
                        break;
                    }
                    l.push(next);
                    cur = next;
                    if l.len() > directed.len() + 1 {
                        break;
                    }
                }
                if l.len() >= 3 {
                    loops.push(l);
                }
            }
            let largest = *group
                .iter()
                .max_by(|&&a, &&b| {
                    self.face_area(a)
                        .partial_cmp(&self.face_area(b))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap();
            let template = &self.faces[largest];
            let area_of = |l: &Vec<u32>| -> f64 {
                let pts: Vec<ok_math::Vec2> = l
                    .iter()
                    .map(|&v| template.plane.to_plane(self.vertices[v as usize]))
                    .collect();
                ok_sketch::signed_area(&pts)
            };
            let mut outers: Vec<Vec<u32>> = Vec::new();
            let mut holes: Vec<Vec<u32>> = Vec::new();
            for l in loops {
                if area_of(&l) > 0.0 {
                    outers.push(l);
                } else {
                    holes.push(l);
                }
            }
            if outers.len() != 1 {
                // Unexpected topology (should not happen for edge-connected
                // coplanar faces); keep the originals untouched.
                for &fi in &group {
                    new_faces.push(self.faces[fi].clone());
                }
                continue;
            }
            let mut face_loops = vec![outers.remove(0)];
            face_loops.extend(holes);
            new_faces.push(Face {
                plane: template.plane,
                loops: face_loops,
                surface: template.surface,
                origin: template.origin,
            });
            merged_any = true;
        }
        if merged_any {
            self.faces = new_faces;
        }
    }

    fn face_area(&self, fi: usize) -> f64 {
        let f = &self.faces[fi];
        let l = &f.loops[0];
        let pts: Vec<ok_math::Vec2> = l
            .iter()
            .map(|&v| f.plane.to_plane(self.vertices[v as usize]))
            .collect();
        ok_sketch::signed_area(&pts).abs()
    }

    /// Drops unreferenced surfaces and renumbers.
    pub fn compact_surfaces(&mut self) {
        let mut map: HashMap<usize, usize> = HashMap::new();
        let mut surfaces = Vec::new();
        for f in &mut self.faces {
            let s = f.surface;
            let id = *map.entry(s).or_insert_with(|| {
                surfaces.push(self.surfaces[s]);
                surfaces.len() - 1
            });
            f.surface = id;
        }
        self.surfaces = surfaces;
    }

    /// Returns the same solid with all faces turned inside out.
    pub fn flipped(&self) -> Solid {
        let mut s = self.clone();
        for f in &mut s.faces {
            f.plane = flip_plane(&f.plane);
            for l in &mut f.loops {
                l.reverse();
            }
        }
        s
    }

    /// Concatenates two solids without any geometric interaction.
    pub fn merged(&self, other: &Solid) -> Solid {
        let mut s = self.clone();
        let voff = s.vertices.len() as u32;
        let soff = s.surfaces.len();
        s.vertices.extend_from_slice(&other.vertices);
        s.surfaces.extend_from_slice(&other.surfaces);
        for f in &other.faces {
            s.faces.push(Face {
                plane: f.plane,
                loops: f
                    .loops
                    .iter()
                    .map(|l| l.iter().map(|v| v + voff).collect())
                    .collect(),
                surface: f.surface + soff,
                origin: f.origin,
            });
        }
        s
    }

    /// Converts faces back to standalone polygons (used by booleans).
    pub fn polygons(&self) -> Vec<Polygon> {
        self.faces
            .iter()
            .map(|f| Polygon {
                plane: f.plane,
                loops: f
                    .loops
                    .iter()
                    .map(|l| l.iter().map(|&v| self.vertices[v as usize]).collect())
                    .collect(),
                surface: f.surface,
                origin: f.origin,
            })
            .collect()
    }

    /// Splits the solid into connected shells (separate lumps).
    pub fn shells(&self) -> Vec<Solid> {
        let n = self.faces.len();
        let mut parent: Vec<usize> = (0..n).collect();
        fn find(p: &mut [usize], i: usize) -> usize {
            let mut r = i;
            while p[r] != r {
                r = p[r];
            }
            let mut c = i;
            while p[c] != r {
                let nx = p[c];
                p[c] = r;
                c = nx;
            }
            r
        }
        for (_, faces) in self.edge_faces() {
            for w in faces.windows(2) {
                let (a, b) = (find(&mut parent, w[0]), find(&mut parent, w[1]));
                if a != b {
                    parent[a] = b;
                }
            }
        }
        let mut groups: HashMap<usize, Vec<Polygon>> = HashMap::new();
        let polys = self.polygons();
        for (i, p) in polys.into_iter().enumerate() {
            groups.entry(find(&mut parent, i)).or_default().push(p);
        }
        let mut out: Vec<Solid> = groups
            .into_values()
            .filter_map(|polys| Solid::from_polygons(polys, self.surfaces.clone()).ok())
            .collect();
        out.sort_by(|a, b| {
            b.volume()
                .partial_cmp(&a.volume())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        out
    }
}

pub(crate) fn flip_plane(p: &Plane) -> Plane {
    Plane {
        origin: p.origin,
        x_axis: p.y_axis,
        y_axis: p.x_axis,
        normal: -p.normal,
    }
}

pub(crate) fn bounds_of(points: impl Iterator<Item = Vec3>) -> Option<(Vec3, Vec3)> {
    let mut min = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut max = -min;
    let mut any = false;
    for p in points {
        any = true;
        min = Vec3::new(min.x.min(p.x), min.y.min(p.y), min.z.min(p.z));
        max = Vec3::new(max.x.max(p.x), max.y.max(p.y), max.z.max(p.z));
    }
    any.then_some((min, max))
}

pub(crate) fn bounds_overlap(a: (Vec3, Vec3), b: (Vec3, Vec3), tol: f64) -> bool {
    a.0.x <= b.1.x + tol
        && b.0.x <= a.1.x + tol
        && a.0.y <= b.1.y + tol
        && b.0.y <= a.1.y + tol
        && a.0.z <= b.1.z + tol
        && b.0.z <= a.1.z + tol
}

/// Uniform grid over points for tolerance queries.
struct PointGrid {
    cell: f64,
    cells: HashMap<(i64, i64, i64), Vec<u32>>,
}

impl PointGrid {
    fn new(points: &[Vec3], tol: f64) -> Self {
        let cell = (tol * 64.0).max(1e-3);
        let mut g = PointGrid {
            cell,
            cells: HashMap::new(),
        };
        for (i, p) in points.iter().enumerate() {
            g.cells.entry(g.key(*p)).or_default().push(i as u32);
        }
        g
    }

    fn key(&self, p: Vec3) -> (i64, i64, i64) {
        (
            (p.x / self.cell).floor() as i64,
            (p.y / self.cell).floor() as i64,
            (p.z / self.cell).floor() as i64,
        )
    }

    fn candidates_near_segment(&self, a: Vec3, b: Vec3) -> Vec<u32> {
        let (ka, kb) = (self.key(a), self.key(b));
        let (x0, x1) = (ka.0.min(kb.0) - 1, ka.0.max(kb.0) + 1);
        let (y0, y1) = (ka.1.min(kb.1) - 1, ka.1.max(kb.1) + 1);
        let (z0, z1) = (ka.2.min(kb.2) - 1, ka.2.max(kb.2) + 1);
        let span = (x1 - x0 + 1) * (y1 - y0 + 1) * (z1 - z0 + 1);
        if span > 4096 {
            // Long edge relative to the grid: fall back to scanning all cells.
            return self.cells.values().flatten().copied().collect();
        }
        let mut out = Vec::new();
        for x in x0..=x1 {
            for y in y0..=y1 {
                for z in z0..=z1 {
                    if let Some(v) = self.cells.get(&(x, y, z)) {
                        out.extend_from_slice(v);
                    }
                }
            }
        }
        out
    }
}

/// Merges points within a tolerance, keeping the first representative.
struct VertexMerger {
    tol: f64,
    cell: f64,
    points: Vec<Vec3>,
    cells: HashMap<(i64, i64, i64), Vec<u32>>,
}

impl VertexMerger {
    fn new(tol: f64) -> Self {
        Self {
            tol,
            cell: tol * 4.0,
            points: Vec::new(),
            cells: HashMap::new(),
        }
    }

    fn key(&self, p: Vec3) -> (i64, i64, i64) {
        (
            (p.x / self.cell).floor() as i64,
            (p.y / self.cell).floor() as i64,
            (p.z / self.cell).floor() as i64,
        )
    }

    fn insert(&mut self, p: Vec3) -> u32 {
        let k = self.key(p);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(ids) = self.cells.get(&(k.0 + dx, k.1 + dy, k.2 + dz)) {
                        for &id in ids {
                            if self.points[id as usize].distance(p) <= self.tol {
                                return id;
                            }
                        }
                    }
                }
            }
        }
        let id = self.points.len() as u32;
        self.points.push(p);
        self.cells.entry(k).or_default().push(id);
        id
    }
}
