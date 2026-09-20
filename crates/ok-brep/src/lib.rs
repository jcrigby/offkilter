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
mod drawing;
mod extrude;
mod loft;
mod revolve;
mod section;
mod shell;
mod sweep;
mod tessellate;
mod transform;

pub use blend::{blend_edges, BlendKind};
pub use boolean::{boolean, BoolOp};
pub use drawing::{project_view, section_view, split, split_tagged, SectionLines, View, ViewLines};
pub use extrude::extrude;
pub use loft::loft;
pub use revolve::revolve;
pub use shell::{draft_faces, move_faces, shell};
pub use sweep::{sweep, sweep_closed};
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
    /// A ruled or swept surface without a simple analytic form; facets
    /// sharing it are shaded with averaged normals.
    Ruled,
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

    /// Total surface area.
    pub fn surface_area(&self) -> f64 {
        let mesh = tessellate(self);
        let p = |i: u32| {
            let i = i as usize * 3;
            Vec3::new(
                mesh.positions[i] as f64,
                mesh.positions[i + 1] as f64,
                mesh.positions[i + 2] as f64,
            )
        };
        mesh.indices
            .chunks_exact(3)
            .map(|t| (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0])).length() * 0.5)
            .sum()
    }

    /// Centre of mass assuming uniform density, or `None` for an empty solid.
    pub fn centroid(&self) -> Option<Vec3> {
        let mesh = tessellate(self);
        let p = |i: u32| {
            let i = i as usize * 3;
            Vec3::new(
                mesh.positions[i] as f64,
                mesh.positions[i + 1] as f64,
                mesh.positions[i + 2] as f64,
            )
        };
        let mut volume = 0.0;
        let mut sum = Vec3::ZERO;
        for t in mesh.indices.chunks_exact(3) {
            let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
            let v = a.dot(b.cross(c)) / 6.0;
            volume += v;
            sum += (a + b + c) * (v / 4.0);
        }
        (volume.abs() > 1e-12).then(|| sum / volume)
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
        Self::from_polygons_with_tolerance(polys, surfaces, tol)
    }

    /// `from_polygons` with an explicit vertex-merge tolerance, for callers
    /// (the booleans) whose fragments were built at a known tolerance.
    pub fn from_polygons_with_tolerance(
        polys: Vec<Polygon>,
        surfaces: Vec<Surface>,
        tol: f64,
    ) -> Result<Solid, BrepError> {
        Self::assemble(polys, surfaces, tol, None)
    }

    /// `from_polygons_with_tolerance` that also closes small holes in the
    /// result: rings of unmatched edges no larger than `max_gap` across are
    /// filled with a fan of triangles. Offsetting uses this where the faces
    /// around a corner legitimately disagree by a little.
    pub fn from_polygons_closing_gaps(
        polys: Vec<Polygon>,
        surfaces: Vec<Surface>,
        tol: f64,
        max_gap: f64,
    ) -> Result<Solid, BrepError> {
        Self::assemble(polys, surfaces, tol, Some(max_gap))
    }

    fn assemble(
        polys: Vec<Polygon>,
        surfaces: Vec<Surface>,
        tol: f64,
        max_gap: Option<f64>,
    ) -> Result<Solid, BrepError> {
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
        // Splitting an edge at a T-junction can expose a short edge or an
        // open vertex, and merging vertices can create a new T-junction,
        // so alternate until stable (two rounds in practice).
        for _ in 0..4 {
            solid.repair_t_junctions(tol);
            let mut changed = solid.collapse_short_edges(tol * 10.0);
            changed |= solid.stitch_open_vertices(tol * 10.0);
            if !changed {
                break;
            }
        }
        if let Some(max_gap) = max_gap {
            if solid.close_small_gaps(max_gap) {
                solid.repair_t_junctions(tol);
            }
        }
        solid.split_nonplanar_faces(tol);
        solid.remove_spikes(tol);
        solid.remove_degenerate_faces();
        solid.compact_surfaces();
        solid.validate()?;
        Ok(solid)
    }

    /// Fills rings of unmatched edges no larger than `max_gap` across with
    /// fan triangles from the ring's centroid. Returns whether any were added.
    fn close_small_gaps(&mut self, max_gap: f64) -> bool {
        // Directed edges the faces still owe: the reverse of each unmatched one.
        let mut sum: HashMap<EdgeKey, (i32, usize)> = HashMap::new();
        for (a, b, f) in self.directed_edges() {
            let e = sum.entry(edge_key(a, b)).or_insert((0, f));
            e.0 += if a < b { 1 } else { -1 };
        }
        let mut owed: HashMap<u32, Vec<(u32, usize)>> = HashMap::new();
        for ((a, b), (s, f)) in &sum {
            match s.signum() {
                1 => owed.entry(*b).or_default().push((*a, *f)), // a->b present once more: owe b->a
                -1 => owed.entry(*a).or_default().push((*b, *f)),
                _ => {}
            }
        }
        if owed.is_empty() {
            return false;
        }
        let mut added = false;
        while let Some((&start, _)) = owed.iter().next() {
            // Walk head to tail until the ring closes.
            let mut ring = vec![start];
            let mut origin = None;
            let mut cur = start;
            loop {
                let Some(list) = owed.get_mut(&cur) else {
                    break;
                };
                let Some((next, f)) = list.pop() else {
                    owed.remove(&cur);
                    break;
                };
                if list.is_empty() {
                    owed.remove(&cur);
                }
                origin.get_or_insert(self.faces[f].origin);
                if next == start {
                    break;
                }
                ring.push(next);
                cur = next;
            }
            if ring.len() < 3 || ring.last() == Some(&start) {
                continue;
            }
            let pts: Vec<Vec3> = ring.iter().map(|&v| self.vertices[v as usize]).collect();
            let Some((min, max)) = bounds_of(pts.iter().copied()) else {
                continue;
            };
            if (max - min).length() > max_gap {
                continue;
            }
            let centre = pts.iter().fold(Vec3::ZERO, |a, &p| a + p) / pts.len() as f64;
            let c = self.vertices.len() as u32;
            self.vertices.push(centre);
            for k in 0..ring.len() {
                let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
                let (pa, pb) = (self.vertices[a as usize], self.vertices[b as usize]);
                let Some(normal) = (pa - centre).cross(pb - centre).normalized() else {
                    continue;
                };
                let Some(plane) = Plane::from_origin_normal(centre, normal) else {
                    continue;
                };
                self.surfaces.push(Surface::Plane {
                    normal,
                    offset: normal.dot(centre),
                });
                self.faces.push(Face {
                    plane,
                    loops: vec![vec![c, a, b]],
                    surface: self.surfaces.len() - 1,
                    origin: origin.unwrap_or(FaceOrigin {
                        feature: 0,
                        local: 0,
                    }),
                });
                added = true;
            }
        }
        added
    }

    /// Merges the endpoints of every edge shorter than `thr`. Grazing
    /// intersections leave the faces around a corner disagreeing by a sliver
    /// a few tolerances across; collapsing those edges brings the corner
    /// back to one vertex.
    fn collapse_short_edges(&mut self, thr: f64) -> bool {
        let mut pairs = Vec::new();
        for f in &self.faces {
            for l in &f.loops {
                for i in 0..l.len() {
                    let (a, b) = (l[i], l[(i + 1) % l.len()]);
                    if self.vertices[a as usize].distance(self.vertices[b as usize]) < thr {
                        pairs.push((a, b));
                    }
                }
            }
        }
        self.merge_vertex_pairs(&pairs, thr)
    }

    /// Merges vertices on open (unmatched) edges that lie within `thr` of
    /// each other: two faces that should share a corner but disagree on it
    /// by slightly more than the merge tolerance.
    fn stitch_open_vertices(&mut self, thr: f64) -> bool {
        let mut dir: HashMap<EdgeKey, (i32, usize)> = HashMap::new();
        for (a, b, _) in self.directed_edges() {
            let e = dir.entry(edge_key(a, b)).or_insert((0, 0));
            e.0 += if a < b { 1 } else { -1 };
            e.1 += 1;
        }
        let mut open: Vec<u32> = dir
            .iter()
            .filter(|(_, (sum, count))| *count % 2 != 0 || *sum != 0)
            .flat_map(|((a, b), _)| [*a, *b])
            .collect();
        open.sort_unstable();
        open.dedup();
        if open.len() < 2 {
            return false;
        }
        let mut pairs = Vec::new();
        for (i, &a) in open.iter().enumerate() {
            for &b in &open[i + 1..] {
                if self.vertices[a as usize].distance(self.vertices[b as usize]) < thr {
                    pairs.push((a, b));
                }
            }
        }
        self.merge_vertex_pairs(&pairs, thr)
    }

    /// Unifies the given vertex pairs (union-find), rewriting every loop.
    /// Clusters stay within `thr` of their representative so chains of
    /// close vertices never collapse into one point. Returns whether
    /// anything changed.
    fn merge_vertex_pairs(&mut self, pairs: &[(u32, u32)], thr: f64) -> bool {
        if pairs.is_empty() {
            return false;
        }
        let n = self.vertices.len();
        let mut parent: Vec<u32> = (0..n as u32).collect();
        fn find(parent: &mut [u32], mut i: u32) -> u32 {
            while parent[i as usize] != i {
                parent[i as usize] = parent[parent[i as usize] as usize];
                i = parent[i as usize];
            }
            i
        }
        let mut merged = false;
        for &(a, b) in pairs {
            let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
            if ra == rb {
                continue;
            }
            if self.vertices[ra as usize].distance(self.vertices[rb as usize]) < thr {
                parent[rb as usize] = ra;
                merged = true;
            }
        }
        if !merged {
            return false;
        }
        for f in &mut self.faces {
            for l in &mut f.loops {
                for v in l.iter_mut() {
                    *v = find(&mut parent, *v);
                }
                l.dedup();
                while l.len() > 1 && l.first() == l.last() {
                    l.pop();
                }
            }
            f.loops.retain(|l| l.len() >= 3);
        }
        self.faces.retain(|f| !f.loops.is_empty());
        true
    }

    /// Splits any face whose vertices stray from its plane by more than
    /// `tol` into triangles, each exactly planar, keeping the surface tag
    /// and origin. Stitching open vertices and collapsing short edges may
    /// move a vertex by several times the merge tolerance, which can leave
    /// a face non-planar by that much; later booleans section such a face
    /// by its plane and would get points off its true edges.
    fn split_nonplanar_faces(&mut self, tol: f64) {
        let mut out: Vec<Face> = Vec::with_capacity(self.faces.len());
        for f in std::mem::take(&mut self.faces) {
            let pts: Vec<Vec3> = f.loops[0]
                .iter()
                .map(|&v| self.vertices[v as usize])
                .collect();
            let Some(n) = revolve::newell_normal(&pts).normalized() else {
                out.push(f);
                continue;
            };
            let centroid = pts.iter().fold(Vec3::ZERO, |a, &p| a + p) * (1.0 / pts.len() as f64);
            let off = f
                .loops
                .iter()
                .flatten()
                .map(|&v| n.dot(self.vertices[v as usize] - centroid).abs())
                .fold(0.0, f64::max);
            if off <= tol {
                out.push(f);
                continue;
            }
            // Triangulate in the best-fit plane; every triangle is planar.
            let x = (f.plane.x_axis - n * f.plane.x_axis.dot(n))
                .normalized()
                .unwrap_or_else(|| {
                    let seed = if n.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
                    n.cross(seed).normalized().unwrap_or(Vec3::X)
                });
            let frame = ok_math::Plane {
                origin: centroid,
                x_axis: x,
                y_axis: n.cross(x),
                normal: n,
            };
            let mut flat: Vec<f64> = Vec::new();
            let mut holes: Vec<usize> = Vec::new();
            let mut ids: Vec<u32> = Vec::new();
            for (li, l) in f.loops.iter().enumerate() {
                if li > 0 {
                    holes.push(flat.len() / 2);
                }
                for &v in l {
                    let q = frame.to_plane(self.vertices[v as usize]);
                    flat.extend([q.x, q.y]);
                    ids.push(v);
                }
            }
            let tris = earcutr::earcut(&flat, &holes, 2).unwrap_or_default();
            if tris.len() < 3 {
                out.push(f);
                continue;
            }
            let mut any = false;
            for t in tris.chunks(3) {
                let (a, b, c) = (ids[t[0]], ids[t[1]], ids[t[2]]);
                let (pa, pb, pc) = (
                    self.vertices[a as usize],
                    self.vertices[b as usize],
                    self.vertices[c as usize],
                );
                let Some(tn) = (pb - pa).cross(pc - pa).normalized() else {
                    continue;
                };
                // Earcut winds with the frame; keep the outward sense of the face.
                let (loop_, tn) = if tn.dot(n) >= 0.0 {
                    (vec![a, b, c], tn)
                } else {
                    (vec![a, c, b], -tn)
                };
                let tx = (pb - pa).normalized().unwrap_or(x);
                out.push(Face {
                    plane: ok_math::Plane {
                        origin: pa,
                        x_axis: tx,
                        y_axis: tn.cross(tx),
                        normal: tn,
                    },
                    loops: vec![loop_],
                    surface: f.surface,
                    origin: f.origin,
                });
                any = true;
            }
            if !any {
                out.push(f);
            }
        }
        self.faces = out;
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
    /// Cells are sized to the model (about 1/32 of its extent, never
    /// below 64 tolerances) so a query along an edge touches a bounded
    /// number of cells however long the edge is.
    fn new(points: &[Vec3], tol: f64) -> Self {
        let extent = bounds_of(points.iter().copied())
            .map(|(lo, hi)| (hi - lo).length())
            .unwrap_or(1.0);
        let cell = (tol * 64.0).max(extent / 32.0).max(1e-3);
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

    /// Points in the cells the segment passes through, with a one-cell
    /// margin, so every point within a cell width of the segment is
    /// included (and some further away, which callers filter).
    fn candidates_near_segment(&self, a: Vec3, b: Vec3) -> Vec<u32> {
        if self.cells.len() <= 8 {
            return self.cells.values().flatten().copied().collect();
        }
        let steps = ((b - a).length() / self.cell).ceil().max(1.0) as usize;
        let mut seen: std::collections::HashSet<(i64, i64, i64)> = std::collections::HashSet::new();
        let mut out = Vec::new();
        for i in 0..=steps {
            let p = a + (b - a) * (i as f64 / steps as f64);
            let k = self.key(p);
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        let key = (k.0 + dx, k.1 + dy, k.2 + dz);
                        if seen.insert(key) {
                            if let Some(v) = self.cells.get(&key) {
                                out.extend_from_slice(v);
                            }
                        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_face_bent_by_more_than_tolerance_is_split_into_planar_triangles() {
        // A unit box whose top far corner is raised by 5e-3: the top is
        // non-planar by well over the merge tolerance.
        let lift = 5e-3;
        let p = |x: f64, y: f64, z: f64| Vec3::new(x, y, z);
        let quad = |pts: [Vec3; 4], normal: Vec3, local: u32| {
            let x = (pts[1] - pts[0]).normalized().unwrap();
            Polygon {
                plane: ok_math::Plane {
                    origin: pts[0],
                    x_axis: x,
                    y_axis: normal.cross(x),
                    normal,
                },
                loops: vec![pts.to_vec()],
                surface: local as usize,
                origin: FaceOrigin { feature: 1, local },
            }
        };
        let (nx, ny, nz) = (p(1.0, 0.0, 0.0), p(0.0, 1.0, 0.0), p(0.0, 0.0, 1.0));
        let polys = vec![
            quad(
                [
                    p(0.0, 0.0, 0.0),
                    p(0.0, 1.0, 0.0),
                    p(1.0, 1.0, 0.0),
                    p(1.0, 0.0, 0.0),
                ],
                -nz,
                0,
            ),
            quad(
                [
                    p(0.0, 0.0, 1.0),
                    p(1.0, 0.0, 1.0),
                    p(1.0, 1.0, 1.0 + lift),
                    p(0.0, 1.0, 1.0),
                ],
                nz,
                1,
            ),
            quad(
                [
                    p(0.0, 0.0, 0.0),
                    p(1.0, 0.0, 0.0),
                    p(1.0, 0.0, 1.0),
                    p(0.0, 0.0, 1.0),
                ],
                -ny,
                2,
            ),
            quad(
                [
                    p(1.0, 0.0, 0.0),
                    p(1.0, 1.0, 0.0),
                    p(1.0, 1.0, 1.0 + lift),
                    p(1.0, 0.0, 1.0),
                ],
                nx,
                3,
            ),
            quad(
                [
                    p(1.0, 1.0, 0.0),
                    p(0.0, 1.0, 0.0),
                    p(0.0, 1.0, 1.0),
                    p(1.0, 1.0, 1.0 + lift),
                ],
                ny,
                4,
            ),
            quad(
                [
                    p(0.0, 1.0, 0.0),
                    p(0.0, 0.0, 0.0),
                    p(0.0, 0.0, 1.0),
                    p(0.0, 1.0, 1.0),
                ],
                -nx,
                5,
            ),
        ];
        let surfaces: Vec<Surface> = polys
            .iter()
            .map(|q| Surface::Plane {
                normal: q.plane.normal,
                offset: q.plane.normal.dot(q.plane.origin),
            })
            .collect();
        let s = Solid::from_polygons(polys, surfaces).unwrap();
        s.validate().unwrap();
        // The bent top became two triangles; the five flat quads stay (the
        // raised corner keeps the x = 1 and y = 1 sides planar).
        assert_eq!(s.faces.len(), 7);
        assert_eq!(s.faces.iter().filter(|f| f.origin.local == 1).count(), 2);
        let tol = merge_tolerance(3f64.sqrt());
        for f in &s.faces {
            for &v in &f.loops[0] {
                let d = f
                    .plane
                    .normal
                    .dot(s.vertices[v as usize] - f.plane.origin)
                    .abs();
                assert!(d <= tol, "face still bent by {d}");
            }
        }
        let v = s.volume();
        assert!(v > 1.0 && v < 1.0 + lift, "volume {v}");
    }
}
