//! Planar cross-sections of solids.
//!
//! The section of a solid by a plane is a set of oriented 2D loops in the
//! plane's coordinates, counter-clockwise around solid material. Vertices
//! that lie exactly on the plane are treated as being on one side, chosen
//! by `zero_is_above`; this is the classic simulation-of-simplicity trick
//! and lets callers ask for the section "just above" or "just below" a
//! plane without perturbing coordinates. Crossing points are computed once
//! per edge, so the loops chain together exactly by edge identity.
//! Vertices within `eps` of the plane are snapped onto it first, so that
//! faces built to be coplanar are treated as exactly coplanar.

use crate::fasthash::HashMap;
use crate::{edge_key, BrepError, EdgeKey, Solid};
use ok_math::{Plane, Vec2, Vec3};

pub struct Section {
    /// Loops in plane coordinates; CCW encloses material.
    pub loops: Vec<Vec<Vec2>>,
}

#[cfg(test)]
impl Section {
    pub fn is_empty(&self) -> bool {
        self.loops.is_empty()
    }
}

struct Crossing {
    point: Vec3,
}

#[cfg(test)]
pub fn section(
    solid: &Solid,
    plane: &Plane,
    zero_is_above: bool,
    eps: f64,
) -> Result<Section, BrepError> {
    let index = FaceIndex::new(solid);
    section_with_index(solid, &index, plane, zero_is_above, eps)
}

/// The faces of a solid in a bounding-volume tree, for repeated sections
/// of one solid: a plane query visits only the subtrees it cuts.
pub struct FaceIndex {
    boxes: Vec<(Vec3, Vec3)>,
    /// The box of every loop of every face, so a local section can skip
    /// the loops of a large face that stay clear of its window.
    loop_boxes: Vec<Vec<(Vec3, Vec3)>>,
    nodes: Vec<Node>,
    /// Face indices, grouped so each leaf owns a contiguous range.
    order: Vec<usize>,
    /// Faces around every vertex, for faces pulled onto a plane.
    vertex_faces: Vec<Vec<usize>>,
}

struct Node {
    lo: Vec3,
    hi: Vec3,
    /// Range into `order` (leaf) or the two children (inner).
    range: (usize, usize),
    children: Option<(usize, usize)>,
}

const LEAF: usize = 8;

impl FaceIndex {
    pub fn new(solid: &Solid) -> FaceIndex {
        let boxes: Vec<(Vec3, Vec3)> = solid
            .faces
            .iter()
            .map(|f| {
                crate::bounds_of(
                    f.loops
                        .iter()
                        .flatten()
                        .map(|&v| solid.vertices[v as usize]),
                )
                .unwrap_or((Vec3::ZERO, Vec3::ZERO))
            })
            .collect();
        let loop_boxes: Vec<Vec<(Vec3, Vec3)>> = solid
            .faces
            .iter()
            .map(|f| {
                f.loops
                    .iter()
                    .map(|l| {
                        crate::bounds_of(l.iter().map(|&v| solid.vertices[v as usize]))
                            .unwrap_or((Vec3::ZERO, Vec3::ZERO))
                    })
                    .collect()
            })
            .collect();
        let mut vertex_faces = vec![Vec::new(); solid.vertices.len()];
        for (fi, f) in solid.faces.iter().enumerate() {
            for &v in f.loops.iter().flatten() {
                vertex_faces[v as usize].push(fi);
            }
        }
        let mut order: Vec<usize> = (0..boxes.len()).collect();
        let mut nodes = Vec::new();
        if !order.is_empty() {
            build(&boxes, &mut order, 0, boxes.len(), &mut nodes);
        }
        FaceIndex {
            boxes,
            loop_boxes,
            nodes,
            order,
            vertex_faces,
        }
    }

    /// Faces whose box reaches within `eps` of the plane, in index order.
    fn straddling(&self, n: Vec3, d0: f64, eps: f64) -> Vec<usize> {
        let mut out = Vec::new();
        if self.nodes.is_empty() {
            return out;
        }
        let straddles = |lo: Vec3, hi: Vec3| -> bool {
            let pick = |c: f64, l: f64, h: f64| if c >= 0.0 { (h, l) } else { (l, h) };
            let (xh, xl) = pick(n.x, lo.x, hi.x);
            let (yh, yl) = pick(n.y, lo.y, hi.y);
            let (zh, zl) = pick(n.z, lo.z, hi.z);
            let dmax = n.x * xh + n.y * yh + n.z * zh - d0;
            let dmin = n.x * xl + n.y * yl + n.z * zl - d0;
            dmin <= eps && dmax >= -eps
        };
        let mut stack = vec![0usize];
        while let Some(ni) = stack.pop() {
            let node = &self.nodes[ni];
            if !straddles(node.lo, node.hi) {
                continue;
            }
            match node.children {
                Some((l, r)) => {
                    stack.push(r);
                    stack.push(l);
                }
                None => {
                    for &fi in &self.order[node.range.0..node.range.1] {
                        let (lo, hi) = self.boxes[fi];
                        if straddles(lo, hi) {
                            out.push(fi);
                        }
                    }
                }
            }
        }
        out.sort_unstable();
        out
    }

    /// Faces whose box, grown by `tol`, contains `p`, in index order.
    pub fn faces_near_point(&self, p: Vec3, tol: f64) -> Vec<usize> {
        let mut out = Vec::new();
        if self.nodes.is_empty() {
            return out;
        }
        let holds = |lo: Vec3, hi: Vec3| -> bool {
            p.x >= lo.x - tol
                && p.x <= hi.x + tol
                && p.y >= lo.y - tol
                && p.y <= hi.y + tol
                && p.z >= lo.z - tol
                && p.z <= hi.z + tol
        };
        let mut stack = vec![0usize];
        while let Some(ni) = stack.pop() {
            let node = &self.nodes[ni];
            if !holds(node.lo, node.hi) {
                continue;
            }
            match node.children {
                Some((l, r)) => {
                    stack.push(r);
                    stack.push(l);
                }
                None => {
                    for &fi in &self.order[node.range.0..node.range.1] {
                        let (lo, hi) = self.boxes[fi];
                        if holds(lo, hi) {
                            out.push(fi);
                        }
                    }
                }
            }
        }
        out.sort_unstable();
        out
    }

    /// `straddling` limited to faces whose box also overlaps `within`
    /// (grown by `eps`).
    pub fn straddling_within(
        &self,
        n: Vec3,
        d0: f64,
        eps: f64,
        within: (Vec3, Vec3),
    ) -> Vec<usize> {
        let mut out = Vec::new();
        if self.nodes.is_empty() {
            return out;
        }
        let (wlo, whi) = (
            within.0 - Vec3::new(eps, eps, eps),
            within.1 + Vec3::new(eps, eps, eps),
        );
        let wanted = |lo: Vec3, hi: Vec3| -> bool {
            if hi.x < wlo.x
                || lo.x > whi.x
                || hi.y < wlo.y
                || lo.y > whi.y
                || hi.z < wlo.z
                || lo.z > whi.z
            {
                return false;
            }
            let pick = |c: f64, l: f64, h: f64| if c >= 0.0 { (h, l) } else { (l, h) };
            let (xh, xl) = pick(n.x, lo.x, hi.x);
            let (yh, yl) = pick(n.y, lo.y, hi.y);
            let (zh, zl) = pick(n.z, lo.z, hi.z);
            let dmax = n.x * xh + n.y * yh + n.z * zh - d0;
            let dmin = n.x * xl + n.y * yl + n.z * zl - d0;
            dmin <= eps && dmax >= -eps
        };
        let mut stack = vec![0usize];
        while let Some(ni) = stack.pop() {
            let node = &self.nodes[ni];
            if !wanted(node.lo, node.hi) {
                continue;
            }
            match node.children {
                Some((l, r)) => {
                    stack.push(r);
                    stack.push(l);
                }
                None => {
                    for &fi in &self.order[node.range.0..node.range.1] {
                        let (lo, hi) = self.boxes[fi];
                        if wanted(lo, hi) {
                            out.push(fi);
                        }
                    }
                }
            }
        }
        out.sort_unstable();
        out
    }

    /// The faces the ray `p + t d` crosses, as `(t, face)` for `t >= -eps`
    /// (so a face through `p` is reported at `t ≈ 0`), unsorted; `None`
    /// when a crossing is too close to call (within `eps` of an edge, or
    /// a face the ray runs along).
    pub fn hits_along(
        &self,
        solid: &Solid,
        p: Vec3,
        d: Vec3,
        eps: f64,
    ) -> Option<Vec<(f64, usize)>> {
        let mut hits = Vec::new();
        if self.nodes.is_empty() {
            return Some(hits);
        }
        let inv = Vec3::new(1.0 / d.x, 1.0 / d.y, 1.0 / d.z);
        let ray_hits_box = |lo: Vec3, hi: Vec3| -> bool {
            let (mut t0, mut t1) = (-eps, f64::INFINITY);
            for axis in 0..3 {
                let (l, h, o, i) = match axis {
                    0 => (lo.x, hi.x, p.x, inv.x),
                    1 => (lo.y, hi.y, p.y, inv.y),
                    _ => (lo.z, hi.z, p.z, inv.z),
                };
                let (mut a, mut b) = ((l - eps - o) * i, (h + eps - o) * i);
                if a > b {
                    std::mem::swap(&mut a, &mut b);
                }
                t0 = t0.max(a);
                t1 = t1.min(b);
                if t0 > t1 {
                    return false;
                }
            }
            true
        };
        let mut stack = vec![0usize];
        while let Some(ni) = stack.pop() {
            let node = &self.nodes[ni];
            if !ray_hits_box(node.lo, node.hi) {
                continue;
            }
            match node.children {
                Some((l, r)) => {
                    stack.push(r);
                    stack.push(l);
                }
                None => {
                    for &fi in &self.order[node.range.0..node.range.1] {
                        let (lo, hi) = self.boxes[fi];
                        if !ray_hits_box(lo, hi) {
                            continue;
                        }
                        let f = &solid.faces[fi];
                        let denom = f.plane.normal.dot(d);
                        let dist = f.plane.normal.dot(f.plane.origin - p);
                        if denom.abs() < 1e-9 {
                            if dist.abs() <= eps {
                                return None;
                            }
                            continue;
                        }
                        let t = dist / denom;
                        if t < -eps {
                            continue;
                        }
                        let q = f.plane.to_plane(p + d * t);
                        match point_in_face(solid, f, q, eps) {
                            Containment::Inside => hits.push((t, fi)),
                            Containment::Outside => {}
                            Containment::Boundary => return None,
                        }
                    }
                }
            }
        }
        Some(hits)
    }
}

enum Containment {
    Inside,
    Outside,
    Boundary,
}

/// Where a point in a face's plane coordinates falls with respect to the
/// face: inside its outer loop and outside its holes, within `eps` of an
/// edge, or outside.
fn point_in_face(solid: &Solid, f: &crate::Face, q: Vec2, eps: f64) -> Containment {
    let eps2 = eps * eps;
    let mut inside = false;
    for l in &f.loops {
        let n = l.len();
        for i in 0..n {
            let a = f.plane.to_plane(solid.vertices[l[i] as usize]);
            let b = f.plane.to_plane(solid.vertices[l[(i + 1) % n] as usize]);
            let d = b - a;
            let len2 = d.length_squared();
            let t = if len2 > 0.0 {
                ((q - a).dot(d) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            if (q - (a + d * t)).length_squared() <= eps2 {
                return Containment::Boundary;
            }
            if (a.y > q.y) != (b.y > q.y) {
                let x = a.x + (q.y - a.y) / (b.y - a.y) * (b.x - a.x);
                if q.x < x {
                    inside = !inside;
                }
            }
        }
    }
    if inside {
        Containment::Inside
    } else {
        Containment::Outside
    }
}

/// Builds the subtree over `order[start..end]` by splitting the longest
/// axis of the boxes' centroids at the median; returns the node index.
fn build(
    boxes: &[(Vec3, Vec3)],
    order: &mut [usize],
    start: usize,
    end: usize,
    nodes: &mut Vec<Node>,
) -> usize {
    let (mut lo, mut hi) = (
        Vec3::new(f64::MAX, f64::MAX, f64::MAX),
        Vec3::new(f64::MIN, f64::MIN, f64::MIN),
    );
    for &fi in &order[start..end] {
        let (l, h) = boxes[fi];
        lo = Vec3::new(lo.x.min(l.x), lo.y.min(l.y), lo.z.min(l.z));
        hi = Vec3::new(hi.x.max(h.x), hi.y.max(h.y), hi.z.max(h.z));
    }
    let index = nodes.len();
    nodes.push(Node {
        lo,
        hi,
        range: (start, end),
        children: None,
    });
    if end - start <= LEAF {
        return index;
    }
    let extent = hi - lo;
    let axis = if extent.x >= extent.y && extent.x >= extent.z {
        0
    } else if extent.y >= extent.z {
        1
    } else {
        2
    };
    let centre = |fi: usize| {
        let (l, h) = boxes[fi];
        match axis {
            0 => l.x + h.x,
            1 => l.y + h.y,
            _ => l.z + h.z,
        }
    };
    let mid = start + (end - start) / 2;
    order[start..end].select_nth_unstable_by(mid - start, |&a, &b| {
        centre(a)
            .partial_cmp(&centre(b))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let left = build(boxes, order, start, mid, nodes);
    let right = build(boxes, order, mid, end, nodes);
    nodes[index].children = Some((left, right));
    index
}

/// `section` with the faces indexed (`FaceIndex::new`): faces whose box
/// lies entirely on one side of the plane are never visited.
#[cfg(test)]
pub fn section_with_index(
    solid: &Solid,
    index: &FaceIndex,
    plane: &Plane,
    zero_is_above: bool,
    eps: f64,
) -> Result<Section, BrepError> {
    let prepared = prepare(solid, index, plane, eps);
    section_prepared(solid, plane, &prepared, zero_is_above)
}

/// The sections a hair above and a hair below the plane (as `section`
/// with `zero_is_above` false and true), sharing the candidate faces and
/// their distances between them.
pub fn sections_above_below(
    solid: &Solid,
    index: &FaceIndex,
    plane: &Plane,
    eps: f64,
) -> Result<(Section, Section), BrepError> {
    let prepared = prepare(solid, index, plane, eps);
    Ok((
        section_prepared(solid, plane, &prepared, false)?,
        section_prepared(solid, plane, &prepared, true)?,
    ))
}

/// A section restricted to the faces near a box: the loops that close
/// among those faces, and the chains that leave them (oriented like the
/// loops, with material on the left).
pub struct LocalSection {
    pub loops: Vec<Vec<Vec2>>,
    pub chains: Vec<Vec<Vec2>>,
}

/// The sections a hair above and below the plane, cut only from the
/// faces whose box reaches `within`; the chains are complete inside that
/// box (a chain leaves it only where the faces beyond were left out).
pub fn local_sections_above_below(
    solid: &Solid,
    index: &FaceIndex,
    plane: &Plane,
    eps: f64,
    within: (Vec3, Vec3),
) -> Result<(LocalSection, LocalSection), BrepError> {
    let n = plane.normal;
    let d0 = n.dot(plane.origin);
    let faces = index.straddling_within(n, d0, eps, within);
    let window = (
        within.0 - Vec3::new(eps, eps, eps),
        within.1 + Vec3::new(eps, eps, eps),
    );
    let prepared = prepare_faces(solid, index, plane, eps, faces, Some(window));
    Ok((
        chains_prepared(solid, plane, &prepared, false)?,
        chains_prepared(solid, plane, &prepared, true)?,
    ))
}

/// The faces a plane may cut, and the vertices pulled onto the plane so
/// parallel faces agree; every other vertex's distance is computed from
/// its position when asked (cheaper than caching it per call).
struct Prepared<'a> {
    faces: Vec<usize>,
    pulled: HashMap<u32, ()>,
    n: Vec3,
    d0: f64,
    eps: f64,
    /// For a local section: the box the chains must be complete in.
    /// Loops of a face whose box stays clear of it are skipped (a closed
    /// loop crosses the plane an even number of times, so the remaining
    /// crossings still pair up and the parity at the window holds), and
    /// crossing segments lying wholly outside it along the face's
    /// crossing line are left out.
    window: Option<(Vec3, Vec3)>,
    loop_boxes: &'a [Vec<(Vec3, Vec3)>],
}

impl Prepared<'_> {
    /// Whether loop `li` of face `fi` can reach the window.
    fn loop_in_window(&self, fi: usize, li: usize) -> bool {
        let Some((lo, hi)) = self.window else {
            return true;
        };
        let (blo, bhi) = self.loop_boxes[fi][li];
        !(bhi.x < lo.x
            || blo.x > hi.x
            || bhi.y < lo.y
            || blo.y > hi.y
            || bhi.z < lo.z
            || blo.z > hi.z)
    }
}

impl Prepared<'_> {
    #[inline]
    fn dist(&self, solid: &Solid, v: u32) -> f64 {
        if !self.pulled.is_empty() && self.pulled.contains_key(&v) {
            return 0.0;
        }
        let d = self.n.dot(solid.vertices[v as usize]) - self.d0;
        if d.abs() <= self.eps {
            0.0
        } else {
            d
        }
    }
}

fn prepare<'a>(solid: &Solid, index: &'a FaceIndex, plane: &Plane, eps: f64) -> Prepared<'a> {
    let n = plane.normal;
    let d0 = n.dot(plane.origin);
    let faces = index.straddling(n, d0, eps);
    prepare_faces(solid, index, plane, eps, faces, None)
}

fn prepare_faces<'a>(
    solid: &Solid,
    index: &'a FaceIndex,
    plane: &Plane,
    eps: f64,
    faces: Vec<usize>,
    window: Option<(Vec3, Vec3)>,
) -> Prepared<'a> {
    let n = plane.normal;
    let d0 = n.dot(plane.origin);
    let mut prepared = Prepared {
        faces,
        pulled: HashMap::default(),
        n,
        d0,
        eps,
        window,
        loop_boxes: &index.loop_boxes,
    };
    // A face parallel to the section plane is skipped below (it has no
    // crossing line), so its vertices must agree on which side they are:
    // when any of them sits on the plane, they all do. Otherwise one
    // vertex a rounding error past the snap band would leave the
    // neighbouring faces producing crossings through this face that
    // nothing closes. Pulling a vertex brings every face around it into
    // play, so those join the candidates; repeated until stable.
    // A face subset (a local section) must still see the parallel faces
    // next to it, whose pulled vertices it shares; they are brought in by
    // vertex adjacency, which keeps the pull decisions the same as a
    // section of the whole solid makes near those faces.
    let straddles_box = |g: usize| -> bool {
        let (lo, hi) = index.boxes[g];
        let pick = |c: f64, l: f64, h: f64| if c >= 0.0 { (h, l) } else { (l, h) };
        let (xh, xl) = pick(n.x, lo.x, hi.x);
        let (yh, yl) = pick(n.y, lo.y, hi.y);
        let (zh, zl) = pick(n.z, lo.z, hi.z);
        let dmax = n.x * xh + n.y * yh + n.z * zh - d0;
        let dmin = n.x * xl + n.y * yl + n.z * zl - d0;
        dmin <= eps && dmax >= -eps
    };
    let mut checked = 0;
    while checked < prepared.faces.len() {
        let fi = prepared.faces[checked];
        checked += 1;
        let f = &solid.faces[fi];
        for (li, l) in f.loops.iter().enumerate() {
            if !prepared.loop_in_window(fi, li) {
                continue;
            }
            for &v in l {
                if prepared.dist(solid, v) != 0.0 {
                    continue;
                }
                for &g in &index.vertex_faces[v as usize] {
                    if g != fi
                        && n.cross(solid.faces[g].plane.normal).length() <= 1e-9
                        && !prepared.faces.contains(&g)
                        && straddles_box(g)
                    {
                        prepared.faces.push(g);
                    }
                }
            }
        }
        if n.cross(f.plane.normal).length() > 1e-9 {
            continue;
        }
        let verts = || f.loops.iter().flatten().copied();
        let any_on = verts().any(|v| prepared.dist(solid, v) == 0.0);
        let any_off = verts().any(|v| prepared.dist(solid, v) != 0.0);
        if any_on && any_off {
            for v in verts() {
                if prepared.dist(solid, v) != 0.0 {
                    prepared.pulled.insert(v, ());
                    for &g in &index.vertex_faces[v as usize] {
                        if !prepared.faces.contains(&g) {
                            prepared.faces.push(g);
                        }
                    }
                }
            }
            // A pulled vertex can change the verdict of parallel faces
            // already checked; look at them again.
            checked = 0;
        }
    }
    prepared
}

fn section_prepared(
    solid: &Solid,
    plane: &Plane,
    prepared: &Prepared,
    zero_is_above: bool,
) -> Result<Section, BrepError> {
    let local = chains_prepared(solid, plane, prepared, zero_is_above)?;
    if !local.chains.is_empty() {
        return Err(BrepError::OpenSection(format!(
            "{} chain(s) did not close",
            local.chains.len()
        )));
    }
    Ok(Section { loops: local.loops })
}

/// Chains the crossing segments of the prepared faces into closed loops
/// and, where the faces run out, open chains.
fn chains_prepared(
    solid: &Solid,
    plane: &Plane,
    prepared: &Prepared,
    zero_is_above: bool,
) -> Result<LocalSection, BrepError> {
    let n = plane.normal;
    let dist = |v: u32| prepared.dist(solid, v);
    let above = |v: u32| -> bool {
        let d = dist(v);
        if zero_is_above {
            d >= 0.0
        } else {
            d > 0.0
        }
    };

    // Crossing nodes. A crossing that lands exactly on a vertex is shared by
    // every edge through that vertex, so a face merely touching the plane at
    // a vertex contributes a zero-length (skipped) segment and chains pass
    // through the vertex consistently.
    let mut crossings: Vec<Crossing> = Vec::new();
    let mut by_edge: HashMap<EdgeKey, usize> = HashMap::default();
    let mut by_vertex: HashMap<u32, usize> = HashMap::default();
    let mut crossing_of = |a: u32, b: u32| -> Option<(usize, Vec3)> {
        if above(a) == above(b) {
            return None;
        }
        let key = edge_key(a, b);
        let (lo, hi) = key;
        let (dl, dh) = (dist(lo), dist(hi));
        let t = dl / (dl - dh);
        let id = if t <= 0.0 || t >= 1.0 {
            let v = if t <= 0.0 { lo } else { hi };
            *by_vertex.entry(v).or_insert_with(|| {
                crossings.push(Crossing {
                    point: solid.vertices[v as usize],
                });
                crossings.len() - 1
            })
        } else {
            *by_edge.entry(key).or_insert_with(|| {
                let p = solid.vertices[lo as usize]
                    + (solid.vertices[hi as usize] - solid.vertices[lo as usize]) * t;
                crossings.push(Crossing { point: p });
                crossings.len() - 1
            })
        };
        Some((id, crossings[id].point))
    };

    // Directed segments between crossing nodes: start -> ends.
    let mut next: HashMap<usize, Vec<usize>> = HashMap::default();
    let mut starts: Vec<usize> = Vec::new();
    let mut segment_count = 0usize;
    for &fi in &prepared.faces {
        let f = &solid.faces[fi];
        let Some(dir) = n.cross(f.plane.normal).normalized() else {
            continue; // parallel to the section plane
        };
        let mut hits: Vec<(f64, usize)> = Vec::new();
        for (li, l) in f.loops.iter().enumerate() {
            if !prepared.loop_in_window(fi, li) {
                continue;
            }
            let count = l.len();
            for i in 0..count {
                if let Some((c, p)) = crossing_of(l[i], l[(i + 1) % count]) {
                    hits.push((p.dot(dir), c));
                }
            }
        }
        if hits.is_empty() {
            continue;
        }
        if !hits.len().is_multiple_of(2) {
            return Err(BrepError::OpenSection(format!(
                "face crossed an odd number of times ({})",
                hits.len()
            )));
        }
        hits.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        // The window's extent along the crossing line: a segment whose
        // ends both lie before or both beyond it cannot reach the window.
        let span = prepared.window.map(|(lo, hi)| {
            let mut range = (f64::MAX, f64::MIN);
            for k in 0..8 {
                let c = Vec3::new(
                    if k & 1 == 0 { lo.x } else { hi.x },
                    if k & 2 == 0 { lo.y } else { hi.y },
                    if k & 4 == 0 { lo.z } else { hi.z },
                );
                let s = c.dot(dir);
                range = (range.0.min(s), range.1.max(s));
            }
            range
        });
        for pair in hits.chunks_exact(2) {
            let (s, e) = (pair[0].1, pair[1].1);
            if s == e {
                continue;
            }
            if let Some((lo, hi)) = span {
                if pair[1].0 < lo || pair[0].0 > hi {
                    continue;
                }
            }
            let list = next.entry(s).or_default();
            if list.is_empty() {
                starts.push(s);
            }
            list.push(e);
            segment_count += 1;
        }
    }
    let _ = &mut crossing_of;

    // Chain segments. In a complete section every node has as many
    // outgoing as incoming segments, so following unused outgoing segments
    // from any node returns to it. Where the faces were cut off, nodes
    // with more outgoing than incoming segments start open chains, which
    // are walked first so the closed loops among the rest come out whole.
    let mut incoming: HashMap<usize, usize> = HashMap::default();
    for ends in next.values() {
        for &e in ends {
            *incoming.entry(e).or_insert(0) += 1;
        }
    }
    let mut sources: Vec<usize> = starts
        .iter()
        .copied()
        .filter(|s| next.get(s).map_or(0, |v| v.len()) > incoming.get(s).copied().unwrap_or(0))
        .collect();
    sources.sort_unstable();
    let mut loops = Vec::new();
    let mut chains = Vec::new();
    let mut remaining = segment_count;
    let walk = |start: usize,
                next: &mut HashMap<usize, Vec<usize>>,
                remaining: &mut usize|
     -> (Vec<Vec2>, bool) {
        let mut poly = Vec::new();
        let mut cur = start;
        loop {
            poly.push(plane.to_plane(crossings[cur].point));
            let Some(nx) = next.get_mut(&cur).and_then(|v| v.pop()) else {
                return (poly, false);
            };
            *remaining -= 1;
            cur = nx;
            if cur == start {
                return (poly, true);
            }
            if poly.len() > segment_count + 1 {
                return (poly, false);
            }
        }
    };
    for s in sources {
        while next.get(&s).is_some_and(|v| !v.is_empty()) {
            let (poly, closed) = walk(s, &mut next, &mut remaining);
            if closed {
                if poly.len() >= 3 && ok_sketch::signed_area(&poly).abs() > 1e-18 {
                    loops.push(poly);
                }
            } else if poly.len() >= 2 {
                chains.push(poly);
            }
        }
    }
    let mut start_at = 0;
    while remaining > 0 {
        // The next node that still has an outgoing segment.
        let start = loop {
            let Some(&s) = starts.get(start_at) else {
                break None;
            };
            if next.get(&s).is_some_and(|v| !v.is_empty()) {
                break Some(s);
            }
            start_at += 1;
        };
        let Some(start) = start else {
            break;
        };
        let (poly, closed) = walk(start, &mut next, &mut remaining);
        if !closed {
            return Err(BrepError::OpenSection(format!(
                "chain broke after {} of {} segments",
                poly.len(),
                segment_count
            )));
        }
        if poly.len() >= 3 && ok_sketch::signed_area(&poly).abs() > 1e-18 {
            loops.push(poly);
        }
    }
    Ok(LocalSection { loops, chains })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extrude;
    use ok_math::Vec2;
    use ok_sketch::{ProfileOptions, Sketch};

    fn box_solid(w: f64, h: f64, d: f64) -> Solid {
        let mut s = Sketch::new();
        s.add_rectangle(Vec2::ZERO, Vec2::new(w, h));
        let p = s.profiles(&ProfileOptions::default()).remove(0);
        extrude(&p, &Plane::XY, 0.0, d, 1).unwrap()
    }

    #[test]
    fn mid_section_of_box_is_ccw_rectangle() {
        let b = box_solid(4.0, 2.0, 3.0);
        let plane = Plane::XY.offset(1.5);
        let sec = section(&b, &plane, true, 1e-9).unwrap();
        assert_eq!(sec.loops.len(), 1);
        assert!((ok_sketch::signed_area(&sec.loops[0]) - 8.0).abs() < 1e-9);
    }

    #[test]
    fn section_through_top_face_depends_on_side() {
        let b = box_solid(4.0, 2.0, 3.0);
        let top = Plane::XY.offset(3.0);
        // Just below the top: material.
        assert_eq!(section(&b, &top, true, 1e-9).unwrap().loops.len(), 1);
        // Just above the top: nothing.
        assert!(section(&b, &top, false, 1e-9).unwrap().is_empty());
    }

    #[test]
    fn vertical_section_is_ccw_about_material() {
        let b = box_solid(4.0, 2.0, 3.0);
        let plane = Plane::YZ.offset(2.0);
        let sec = section(&b, &plane, true, 1e-9).unwrap();
        assert_eq!(sec.loops.len(), 1);
        assert!((ok_sketch::signed_area(&sec.loops[0]) - 6.0).abs() < 1e-9);
    }
}
