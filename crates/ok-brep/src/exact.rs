//! Exact geometry recovered from a polyhedral solid's surface tags.
//!
//! Every edge lies between two faces; where both faces' surfaces are
//! analytic the edge's exact curve follows from the pair (a line, a
//! circle, an ellipse, or the quartic where two cylinders meet), and a
//! vertex's exact position is the point on all of its analytic surfaces.
//! Nothing here changes the solid: callers ask for exact positions and
//! curve samples where they need them (STEP export, refitting).

use crate::{edge_key, BrepError, EdgeKey, Solid, Surface};
use ok_math::{Plane, Vec3};
use std::collections::HashMap;

/// The exact curve along an edge between two surfaces.
#[derive(Debug, Clone, PartialEq)]
pub enum Curve {
    Line {
        point: Vec3,
        dir: Vec3,
    },
    Circle {
        center: Vec3,
        axis: Vec3,
        radius: f64,
    },
    /// The section of a cylinder by an oblique plane: `axis` is the
    /// plane's normal, `major` the unit direction of the long axis in
    /// the plane, `a` and `b` the semi-axes (`b` is the cylinder's radius).
    Ellipse {
        center: Vec3,
        axis: Vec3,
        major: Vec3,
        a: f64,
        b: f64,
    },
    /// Two cylinders meeting: evaluated by projecting onto both.
    Quartic,
    /// No analytic form (a revolved or ruled surface is involved, or the
    /// surfaces are tangent along the edge): the facet polyline stands.
    Polyline,
}

/// Whether a surface has an exact form to project onto.
pub fn is_analytic(s: &Surface) -> bool {
    matches!(s, Surface::Plane { .. } | Surface::Cylinder { .. })
}

/// The nearest point of the surface to `p` (planes and cylinders; other
/// surfaces return `p`).
pub fn project(s: &Surface, p: Vec3) -> Vec3 {
    match *s {
        Surface::Plane { normal, offset } => p - normal * (normal.dot(p) - offset),
        Surface::Cylinder {
            origin,
            axis,
            radius,
        } => {
            let d = p - origin;
            let along = origin + axis * d.dot(axis);
            match (p - along).normalized() {
                Some(radial) => along + radial * radius,
                None => along + perpendicular(axis) * radius,
            }
        }
        _ => p,
    }
}

/// The point on all the given surfaces nearest `p`, by alternating
/// projection; exact for planes meeting at a point, and within `1e-12`
/// of the model's size otherwise unless the surfaces are tangent there.
pub fn project_all(surfaces: &[&Surface], p: Vec3) -> Vec3 {
    let mut q = p;
    let scale = p.length().max(1.0);
    for _ in 0..200 {
        let before = q;
        for s in surfaces {
            q = project(s, q);
        }
        if q.distance(before) <= 1e-13 * scale {
            break;
        }
    }
    q
}

/// A unit vector perpendicular to `v`.
pub fn perpendicular(v: Vec3) -> Vec3 {
    let hint = if v.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
    (hint - v * hint.dot(v)).normalized().unwrap_or(Vec3::X)
}

/// A cylinder's in-plane frame `(x, y)` with `y = axis × x`, the angle
/// origin its facets are measured from.
pub fn cylinder_frame(axis: Vec3) -> (Vec3, Vec3) {
    let x = perpendicular(axis);
    (x, axis.cross(x))
}

/// The angle of `p` about a cylinder, in `(-π, π]`.
pub fn cylinder_angle(origin: Vec3, axis: Vec3, p: Vec3) -> f64 {
    let (x, y) = cylinder_frame(axis);
    let d = p - origin;
    d.dot(y).atan2(d.dot(x))
}

/// The exact curve where two surfaces meet.
pub fn edge_curve(a: &Surface, b: &Surface) -> Curve {
    match (a, b) {
        (
            Surface::Plane {
                normal: na,
                offset: da,
            },
            Surface::Plane {
                normal: nb,
                offset: db,
            },
        ) => {
            let Some(dir) = na.cross(*nb).normalized() else {
                return Curve::Polyline;
            };
            // A point on both planes: solve in the plane spanned by the normals.
            let (n1, n2) = (*na, *nb);
            let dot = n1.dot(n2);
            let det = 1.0 - dot * dot;
            let (c1, c2) = ((da - db * dot) / det, (db - da * dot) / det);
            Curve::Line {
                point: n1 * c1 + n2 * c2,
                dir,
            }
        }
        (Surface::Plane { normal, offset }, Surface::Cylinder { .. }) => {
            plane_cylinder(*normal, *offset, b)
        }
        (Surface::Cylinder { .. }, Surface::Plane { normal, offset }) => {
            plane_cylinder(*normal, *offset, a)
        }
        (Surface::Cylinder { axis: a1, .. }, Surface::Cylinder { axis: a2, .. }) => {
            if a1.cross(*a2).length() < 1e-9 {
                Curve::Polyline // coaxial: tangent or parallel
            } else {
                Curve::Quartic
            }
        }
        _ => Curve::Polyline,
    }
}

fn plane_cylinder(normal: Vec3, offset: f64, cyl: &Surface) -> Curve {
    let Surface::Cylinder {
        origin,
        axis,
        radius,
    } = *cyl
    else {
        return Curve::Polyline;
    };
    let cos = normal.dot(axis);
    if cos.abs() < 1e-9 {
        // Parallel to the axis: the edge runs along a ruling.
        let d = normal.dot(origin) - offset;
        if d.abs() > radius + 1e-9 {
            return Curve::Polyline;
        }
        let foot = origin - normal * d;
        let side = axis.cross(normal);
        let half = (radius * radius - d * d).max(0.0).sqrt();
        // Two rulings in general; the caller picks the one its run is on
        // by the run's own points, so report the line through the first.
        return Curve::Line {
            point: foot + side * half,
            dir: axis,
        };
    }
    // Where the axis meets the plane.
    let t = (offset - normal.dot(origin)) / cos;
    let center = origin + axis * t;
    let sin = normal.cross(axis).length();
    if sin < 1e-9 {
        return Curve::Circle {
            center,
            axis: normal,
            radius,
        };
    }
    let major = (axis - normal * cos)
        .normalized()
        .unwrap_or_else(|| perpendicular(normal));
    Curve::Ellipse {
        center,
        axis: normal,
        major,
        a: radius / cos.abs(),
        b: radius,
    }
}

/// A constraint a vertex's exact position satisfies: one of its
/// surfaces, or the ruling of a cylinder it was made on (the line two of
/// its facets on that cylinder meet along).
enum Constraint<'a> {
    On(&'a Surface),
    Line { point: Vec3, dir: Vec3 },
}

impl Constraint<'_> {
    fn project(&self, p: Vec3) -> Vec3 {
        match *self {
            Constraint::On(s) => project(s, p),
            Constraint::Line { point, dir } => point + dir * (p - point).dot(dir),
        }
    }
}

/// The exact position of a vertex: the point on all of its analytic
/// surfaces (a vertex touching a non-analytic surface keeps its place).
/// A vertex on a ruling of a cylinder (where two of its facets on the
/// cylinder meet) stays on that ruling, so that vertices keep their order
/// along a curve and two facet corners that are one exact point meet.
pub fn vertex_position(solid: &Solid, vertex_faces: &[Vec<usize>], v: u32) -> Vec3 {
    let p = solid.vertices[v as usize];
    let faces = &vertex_faces[v as usize];
    let mut surfaces: Vec<usize> = faces.iter().map(|&f| solid.faces[f].surface).collect();
    surfaces.sort_unstable();
    surfaces.dedup();
    if surfaces.iter().any(|&s| !is_analytic(&solid.surfaces[s])) {
        return p;
    }
    let mut constraints: Vec<Constraint> = surfaces
        .iter()
        .map(|&s| Constraint::On(&solid.surfaces[s]))
        .collect();
    for &s in &surfaces {
        let Surface::Cylinder { axis, .. } = solid.surfaces[s] else {
            continue;
        };
        // The pair of facets on this cylinder meeting at the widest angle;
        // their planes meet along a ruling when it is parallel to the axis.
        let planes: Vec<&ok_math::Plane> = faces
            .iter()
            .filter(|&&f| solid.faces[f].surface == s)
            .map(|&f| &solid.faces[f].plane)
            .collect();
        let mut best: Option<(f64, &ok_math::Plane, &ok_math::Plane)> = None;
        for (i, a) in planes.iter().enumerate() {
            for b in &planes[i + 1..] {
                let sin = a.normal.cross(b.normal).length();
                if best.is_none_or(|(s, _, _)| sin > s) {
                    best = Some((sin, a, b));
                }
            }
        }
        let Some((sin, a, b)) = best else {
            continue;
        };
        if sin < 1e-6 {
            continue;
        }
        let dir = a.normal.cross(b.normal) * (1.0 / sin);
        if dir.dot(axis).abs() < 1.0 - 1e-9 {
            continue;
        }
        // The point of the line nearest `p`: p + x·na + y·nb on both planes.
        let (oa, ob) = (a.normal.dot(a.origin), b.normal.dot(b.origin));
        let c = a.normal.dot(b.normal);
        let (ra, rb) = (oa - a.normal.dot(p), ob - b.normal.dot(p));
        let det = 1.0 - c * c;
        let (x, y) = ((ra - c * rb) / det, (rb - c * ra) / det);
        let point = p + a.normal * x + b.normal * y;
        constraints.push(Constraint::Line { point, dir });
    }
    let scale = p.length().max(1.0);
    let lines: Vec<(Vec3, Vec3)> = constraints
        .iter()
        .filter_map(|c| match *c {
            Constraint::Line { point, dir } => Some((point, dir)),
            Constraint::On(_) => None,
        })
        .collect();
    let mut q = p;
    match lines[..] {
        // On one ruling: where it meets the other surfaces, solved
        // directly (alternating projection creeps along a line that meets
        // a surface at a shallow angle).
        [(point, dir)] => {
            let mut hits: Vec<Vec3> = Vec::new();
            for &s in &surfaces {
                let mut ts: Vec<f64> = Vec::new();
                match solid.surfaces[s] {
                    Surface::Plane { normal, offset } => {
                        let cos = normal.dot(dir);
                        if cos.abs() > 1e-9 {
                            ts.push((offset - normal.dot(point)) / cos);
                        }
                    }
                    Surface::Cylinder {
                        origin,
                        axis,
                        radius,
                    } => {
                        let w = point - origin;
                        let w = w - axis * w.dot(axis);
                        let d = dir - axis * dir.dot(axis);
                        let (a, b, c) = (d.dot(d), 2.0 * w.dot(d), w.dot(w) - radius * radius);
                        if a > 1e-18 {
                            let disc = b * b - 4.0 * a * c;
                            if disc >= 0.0 {
                                let r = disc.sqrt();
                                ts.push((-b - r) / (2.0 * a));
                                ts.push((-b + r) / (2.0 * a));
                            }
                        }
                    }
                    _ => {}
                }
                if let Some(t) = ts.into_iter().min_by(|x, y| x.abs().total_cmp(&y.abs())) {
                    hits.push(point + dir * t);
                }
            }
            if let Some(h) = hits
                .iter()
                .min_by(|x, y| x.distance(p).total_cmp(&y.distance(p)))
            {
                q = *h;
            }
        }
        // On rulings of two cylinders: the one point both pass through,
        // if they do meet; otherwise the rulings are not to be trusted.
        [(p1, d1), (p2, d2)] => {
            let n = d1.cross(d2);
            let n2 = n.dot(n);
            if n2 > 1e-18 {
                let w = p2 - p1;
                let t1 = w.cross(d2).dot(n) / n2;
                let t2 = w.cross(d1).dot(n) / n2;
                let (a, b) = (p1 + d1 * t1, p2 + d2 * t2);
                if a.distance(b) <= 1e-7 * scale {
                    q = (a + b) * 0.5;
                } else {
                    constraints.retain(|c| matches!(c, Constraint::On(_)));
                }
            } else {
                constraints.retain(|c| matches!(c, Constraint::On(_)));
            }
        }
        _ => {}
    }
    for _ in 0..200 {
        let before = q;
        for c in &constraints {
            q = c.project(q);
        }
        if q.distance(before) <= 1e-13 * scale {
            break;
        }
    }
    q
}

/// Moves every vertex onto its exact surfaces and splits the facets that
/// bend by that into planar triangles, so the solid is a tessellation of
/// its exact trimmed faces whatever the resolution of the tools that
/// made it: a boolean's intersection vertices lie where facet planes
/// met, and this puts them on the curves the surfaces meet on. Returns
/// `None` when nothing moves, and an error when the moved mesh does not
/// close or its volume changes by more than a chord's worth (the caller
/// keeps the solid as it was).
pub fn refit(solid: &Solid) -> Result<Option<Solid>, BrepError> {
    refit_within(solid, None)
}

/// [`refit`] for the vertices inside `region` (a box) alone, when the
/// rest are known to be exact already: a boolean changes nothing outside
/// the overlap of its operands' boxes.
pub fn refit_within(
    solid: &Solid,
    region: Option<(Vec3, Vec3)>,
) -> Result<Option<Solid>, BrepError> {
    if !solid
        .surfaces
        .iter()
        .any(|s| matches!(s, Surface::Cylinder { .. }))
    {
        // Vertices of planar facets already sit where their planes meet.
        return Ok(None);
    }
    let Some((lo, hi)) = solid.bounds() else {
        return Ok(None);
    };
    let diag = (hi - lo).length();
    let tol = crate::merge_tolerance(diag);
    // A vertex that would move further than this sits where the facets
    // met but the surfaces do not (tangencies, near-misses): left alone.
    let limit = 0.02 * diag;
    let vf = vertex_faces(solid);
    let mut moves: Vec<(usize, Vec3)> = Vec::new();
    let inside = |p: Vec3| match region {
        Some((lo, hi)) => {
            p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y && p.z >= lo.z && p.z <= hi.z
        }
        None => true,
    };
    for v in 0..solid.vertices.len() {
        if vf[v].is_empty() || !inside(solid.vertices[v]) {
            continue;
        }
        // A vertex of planar facets alone sits where their planes meet.
        if !vf[v].iter().any(|&f| {
            matches!(
                solid.surfaces[solid.faces[f].surface],
                Surface::Cylinder { .. }
            )
        }) {
            continue;
        }
        let p = vertex_position(solid, &vf, v as u32);
        let d = p.distance(solid.vertices[v]);
        if d > 1e-9 * diag.max(1.0) && d <= limit {
            moves.push((v, p));
        }
    }
    if moves.is_empty() {
        return Ok(None);
    }
    let mut out = solid.clone();
    let mut moved = vec![false; solid.vertices.len()];
    for &(v, p) in &moves {
        out.vertices[v] = p;
        moved[v] = true;
    }
    // Two vertices of a run closer together than their moves can change
    // places along the curve, folding the run back on itself; a vertex
    // that only the run's two surfaces hold is then put onto its
    // neighbour, and the assembly below welds the pair.
    let counts: Vec<usize> = vf
        .iter()
        .map(|faces| {
            let mut s: Vec<usize> = faces.iter().map(|&f| solid.faces[f].surface).collect();
            s.sort_unstable();
            s.dedup();
            s.len()
        })
        .collect();
    for run in edge_runs(solid) {
        if !matches!(solid.surfaces[run.surfaces.0], Surface::Cylinder { .. })
            && !matches!(solid.surfaces[run.surfaces.1], Surface::Cylinder { .. })
        {
            continue;
        }
        let vs = &run.vertices;
        for _ in 0..vs.len() {
            let mut folded = None;
            for i in 1..vs.len().saturating_sub(1) {
                let (a, b, c) = (vs[i - 1] as usize, vs[i] as usize, vs[i + 1] as usize);
                let (pa, pb, pc) = (out.vertices[a], out.vertices[b], out.vertices[c]);
                if (pb - pa).dot(pc - pb) < 0.0 && pb.distance(pa) > 0.0 && pc.distance(pb) > 0.0 {
                    folded = Some((a, b, c));
                    break;
                }
            }
            let Some((a, b, c)) = folded else {
                break;
            };
            let (victim, target) = if counts[b] == 2 {
                (
                    b,
                    if out.vertices[b].distance(out.vertices[a])
                        <= out.vertices[b].distance(out.vertices[c])
                    {
                        a
                    } else {
                        c
                    },
                )
            } else if counts[a] == 2 {
                (a, b)
            } else if counts[c] == 2 {
                (c, b)
            } else {
                break;
            };
            out.vertices[victim] = out.vertices[target];
            moved[victim] = true;
        }
    }

    // Vertices that now coincide (two facet corners that are one exact
    // point, or a merged pair above) become one.
    let cell = 4.0 * tol;
    let key = |p: Vec3| {
        (
            (p.x / cell).floor() as i64,
            (p.y / cell).floor() as i64,
            (p.z / cell).floor() as i64,
        )
    };
    let mut cells: HashMap<(i64, i64, i64), Vec<u32>> = HashMap::new();
    for (v, p) in out.vertices.iter().enumerate() {
        if !vf[v].is_empty() {
            cells.entry(key(*p)).or_default().push(v as u32);
        }
    }
    let mut canon: Vec<u32> = (0..out.vertices.len() as u32).collect();
    for v in 0..out.vertices.len() {
        if !moved[v] {
            continue;
        }
        let p = out.vertices[v];
        let k = key(p);
        let mut best: Option<u32> = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let Some(ids) = cells.get(&(k.0 + dx, k.1 + dy, k.2 + dz)) else {
                        continue;
                    };
                    for &u in ids {
                        if (u as usize) < v
                            && out.vertices[u as usize].distance(p) <= tol
                            && best.is_none_or(|b| u < b)
                        {
                            best = Some(u);
                        }
                    }
                }
            }
        }
        if let Some(u) = best {
            canon[v] = canon[u as usize];
        }
    }
    let merged = canon.iter().enumerate().any(|(v, &c)| c as usize != v);
    if merged {
        for f in &mut out.faces {
            for l in &mut f.loops {
                for v in l.iter_mut() {
                    *v = canon[*v as usize];
                }
                l.dedup();
                while l.len() > 1 && l.first() == l.last() {
                    l.pop();
                }
            }
            f.loops.retain(|l| l.len() >= 3);
        }
        out.faces.retain(|f| !f.loops.is_empty());
    }
    // Faces with moved vertices get their plane fitted again: a planar
    // face keeps its surface's plane, a facet the plane its loop now
    // spans (bent ones are split into triangles below).
    for f in &mut out.faces {
        if !f.loops.iter().flatten().any(|&v| moved[v as usize]) {
            continue;
        }
        let pts: Vec<Vec3> = f.loops[0]
            .iter()
            .map(|&v| out.vertices[v as usize])
            .collect();
        let centroid = pts.iter().fold(Vec3::ZERO, |a, &p| a + p) * (1.0 / pts.len() as f64);
        let (normal, origin) = match out.surfaces[f.surface] {
            Surface::Plane { normal, offset } => {
                let n = if normal.dot(f.plane.normal) < 0.0 {
                    -normal
                } else {
                    normal
                };
                let off = if n == normal { offset } else { -offset };
                (n, centroid - n * (n.dot(centroid) - off))
            }
            _ => {
                let Some(n) = crate::revolve::newell_normal(&pts).normalized() else {
                    continue;
                };
                if n.dot(f.plane.normal) <= 0.0 {
                    return Err(BrepError::Degenerate("refit turned a facet over".into()));
                }
                (n, centroid)
            }
        };
        let x_axis = (f.plane.x_axis - normal * f.plane.x_axis.dot(normal))
            .normalized()
            .unwrap_or_else(|| perpendicular(normal));
        f.plane = Plane {
            origin,
            x_axis,
            y_axis: normal.cross(x_axis),
            normal,
        };
    }
    out.split_nonplanar_faces(tol);
    out.remove_spikes(tol);
    out.remove_degenerate_faces();
    out.validate()?;
    out.remove_unused_vertices();
    Ok(Some(out))
}

/// The faces around every vertex.
pub fn vertex_faces(solid: &Solid) -> Vec<Vec<usize>> {
    let mut out = vec![Vec::new(); solid.vertices.len()];
    for (i, f) in solid.faces.iter().enumerate() {
        for &v in f.loops.iter().flatten() {
            if out[v as usize].last() != Some(&i) {
                out[v as usize].push(i);
            }
        }
    }
    out
}

/// A chain of facet edges between one pair of surfaces: a piece of the
/// pair's exact curve.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    /// The two surfaces, lower index first.
    pub surfaces: (usize, usize),
    /// The vertices along the run; for a closed run the first is repeated
    /// at the end.
    pub vertices: Vec<u32>,
    pub closed: bool,
}

/// Every edge run of the solid: facet edges between two different
/// surfaces, chained through vertices where the same pair continues.
/// Seams between facets of one surface are not edges.
pub fn edge_runs(solid: &Solid) -> Vec<Run> {
    // Directed by the lower-surface face's traversal so a run is oriented
    // consistently, as `blend_edges` orients its segments.
    let mut segments: Vec<(u32, u32, (usize, usize))> = Vec::new();
    // Every undirected edge with the faces using it and the direction
    // each traverses it in.
    type Use = (usize, (u32, u32));
    let mut uses: HashMap<EdgeKey, Vec<Use>> = HashMap::new();
    for (a, b, f) in solid.directed_edges() {
        uses.entry(edge_key(a, b)).or_default().push((f, (a, b)));
    }
    for faces in uses.values() {
        let [(fa, da), (fb, db)] = faces[..] else {
            continue;
        };
        let (sa, sb) = (solid.faces[fa].surface, solid.faces[fb].surface);
        if sa == sb {
            continue;
        }
        let (a, b) = if sa < sb { da } else { db };
        segments.push((a, b, (sa.min(sb), sa.max(sb))));
    }
    segments.sort_unstable();
    let mut used = vec![false; segments.len()];
    let mut by_start: HashMap<(u32, (usize, usize)), Vec<usize>> = HashMap::new();
    let mut by_end: HashMap<(u32, (usize, usize)), Vec<usize>> = HashMap::new();
    for (i, &(a, b, pair)) in segments.iter().enumerate() {
        by_start.entry((a, pair)).or_default().push(i);
        by_end.entry((b, pair)).or_default().push(i);
    }
    let mut runs = Vec::new();
    for start in 0..segments.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let pair = segments[start].2;
        let mut chain = vec![start];
        loop {
            let last = segments[*chain.last().unwrap()].1;
            let next = by_start
                .get(&(last, pair))
                .and_then(|v| v.iter().copied().find(|&j| !used[j]));
            match next {
                Some(j) => {
                    used[j] = true;
                    chain.push(j);
                }
                None => break,
            }
        }
        let closed = segments[*chain.last().unwrap()].1 == segments[start].0;
        if !closed {
            let mut before: Vec<usize> = Vec::new();
            loop {
                let first = segments[*before.last().unwrap_or(&chain[0])].0;
                let prev = by_end
                    .get(&(first, pair))
                    .and_then(|v| v.iter().copied().find(|&j| !used[j]));
                match prev {
                    Some(j) => {
                        used[j] = true;
                        before.push(j);
                    }
                    None => break,
                }
            }
            before.reverse();
            before.extend(chain);
            chain = before;
        }
        let mut vertices: Vec<u32> = chain.iter().map(|&i| segments[i].0).collect();
        vertices.push(segments[*chain.last().unwrap()].1);
        runs.push(Run {
            surfaces: pair,
            vertices,
            closed,
        });
    }
    runs
}

/// The exact points along a run: its vertices at their exact positions.
/// With `extra_rulings`, the run is also sampled where it crosses those
/// rulings (angles about the given cylinder surface) between its
/// vertices, so a cylinder can be re-facetted at any resolution.
pub fn run_points(
    solid: &Solid,
    vertex_faces: &[Vec<usize>],
    run: &Run,
    extra_rulings: Option<(usize, &[f64])>,
) -> Vec<Vec3> {
    let exact: Vec<Vec3> = run
        .vertices
        .iter()
        .map(|&v| vertex_position(solid, vertex_faces, v))
        .collect();
    let Some((cyl, rulings)) = extra_rulings else {
        return exact;
    };
    let Surface::Cylinder {
        origin,
        axis,
        radius,
    } = solid.surfaces[cyl]
    else {
        return exact;
    };
    let other = if run.surfaces.0 == cyl {
        run.surfaces.1
    } else {
        run.surfaces.0
    };
    let other = &solid.surfaces[other];
    let (x, y) = cylinder_frame(axis);
    let mut out = Vec::with_capacity(exact.len() * 2);
    for w in exact.windows(2) {
        let (p, q) = (w[0], w[1]);
        out.push(p);
        let t0 = cylinder_angle(origin, axis, p);
        let mut t1 = cylinder_angle(origin, axis, q);
        while t1 - t0 > std::f64::consts::PI {
            t1 -= std::f64::consts::TAU;
        }
        while t0 - t1 > std::f64::consts::PI {
            t1 += std::f64::consts::TAU;
        }
        if (t1 - t0).abs() < 1e-12 {
            continue;
        }
        let (lo, hi) = (t0.min(t1), t0.max(t1));
        let mut crossings: Vec<(f64, Vec3)> = Vec::new();
        for &r in rulings {
            for k in -1..=1 {
                let rho = r + k as f64 * std::f64::consts::TAU;
                // A ruling right next to a vertex adds nothing but a
                // near-duplicate point.
                if rho <= lo + 1e-4 || rho >= hi - 1e-4 {
                    continue;
                }
                let frac = (rho - t0) / (t1 - t0);
                let guess = p + (q - p) * frac;
                let on_ruling = origin + (x * rho.cos() + y * rho.sin()) * radius;
                if let Some(point) = ruling_hit(on_ruling, axis, other, guess) {
                    crossings.push((frac, point));
                }
            }
        }
        crossings.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        out.extend(crossings.into_iter().map(|c| c.1));
    }
    out.push(*exact.last().unwrap());
    out
}

/// Where the ruling line through `base` along `axis` meets `other`,
/// nearest to `guess`.
fn ruling_hit(base: Vec3, axis: Vec3, other: &Surface, guess: Vec3) -> Option<Vec3> {
    match *other {
        Surface::Plane { normal, offset } => {
            let cos = normal.dot(axis);
            if cos.abs() < 1e-12 {
                return None;
            }
            let h = (offset - normal.dot(base)) / cos;
            Some(base + axis * h)
        }
        Surface::Cylinder {
            origin,
            axis: a2,
            radius,
        } => {
            let w = base - origin;
            let wp = w - a2 * w.dot(a2);
            let ap = axis - a2 * axis.dot(a2);
            let (qa, qb, qc) = (ap.dot(ap), 2.0 * wp.dot(ap), wp.dot(wp) - radius * radius);
            if qa < 1e-18 {
                return None;
            }
            let disc = qb * qb - 4.0 * qa * qc;
            if disc < 0.0 {
                return Some(project_all(
                    &[&Surface::Cylinder {
                        origin,
                        axis: a2,
                        radius,
                    }],
                    guess,
                ));
            }
            let roots = [
                (-qb - disc.sqrt()) / (2.0 * qa),
                (-qb + disc.sqrt()) / (2.0 * qa),
            ];
            roots
                .into_iter()
                .map(|h| base + axis * h)
                .min_by(|p, q| p.distance(guess).partial_cmp(&q.distance(guess)).unwrap())
        }
        _ => None,
    }
}

/// The angles of a cylindrical surface's facet seams: where its facets
/// meet one another, measured in `cylinder_frame`.
pub fn rulings(solid: &Solid, surface: usize) -> Vec<f64> {
    let Surface::Cylinder { origin, axis, .. } = solid.surfaces[surface] else {
        return Vec::new();
    };
    let mut angles: Vec<f64> = Vec::new();
    for (key, faces) in solid.edge_faces() {
        if faces.len() == 2
            && solid.faces[faces[0]].surface == surface
            && solid.faces[faces[1]].surface == surface
        {
            angles.push(cylinder_angle(origin, axis, solid.vertices[key.0 as usize]));
        }
    }
    angles.sort_by(|a, b| a.partial_cmp(b).unwrap());
    angles.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    angles
}

/// A boundary loop of a surface's region: each entry is a vertex and the
/// surface on the other side of the edge leaving it.
pub type RegionLoop = Vec<(u32, usize)>;

/// The regions of every surface: the union of its facets, as loops of
/// edges to other surfaces (seams inside the surface cancel out), grouped
/// by the surface index. Loops are oriented as the facets traverse them.
pub fn surface_regions(solid: &Solid) -> HashMap<usize, Vec<RegionLoop>> {
    let edge_faces = solid.edge_faces();
    let mut out: HashMap<usize, Vec<RegionLoop>> = HashMap::new();
    let mut by_surface: HashMap<usize, Vec<usize>> = HashMap::new();
    for (i, f) in solid.faces.iter().enumerate() {
        by_surface.entry(f.surface).or_default().push(i);
    }
    for (surface, faces) in by_surface {
        // Directed boundary edges with the neighbour across each.
        let mut next: HashMap<u32, Vec<(u32, usize)>> = HashMap::new();
        let mut count = 0;
        for &fi in &faces {
            for l in &solid.faces[fi].loops {
                for i in 0..l.len() {
                    let (a, b) = (l[i], l[(i + 1) % l.len()]);
                    let key: EdgeKey = edge_key(a, b);
                    let Some(pair) = edge_faces.get(&key) else {
                        continue;
                    };
                    let other = pair.iter().copied().find(|&g| g != fi);
                    let Some(other) = other else { continue };
                    if solid.faces[other].surface == surface {
                        continue; // a seam inside the surface
                    }
                    next.entry(a)
                        .or_default()
                        .push((b, solid.faces[other].surface));
                    count += 1;
                }
            }
        }
        let mut loops: Vec<RegionLoop> = Vec::new();
        while let Some((&start, _)) = next.iter().find(|(_, v)| !v.is_empty()) {
            let mut l: RegionLoop = Vec::new();
            let mut cur = start;
            loop {
                let Some((nxt, other)) = next.get_mut(&cur).and_then(|v| v.pop()) else {
                    break;
                };
                l.push((cur, other));
                cur = nxt;
                if cur == start || l.len() > count {
                    break;
                }
            }
            if l.len() >= 2 {
                loops.push(l);
            }
        }
        out.insert(surface, loops);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{boolean, extrude, BoolOp};
    use ok_math::{Plane, Vec2};
    use ok_sketch::{ProfileOptions, Sketch};

    fn cylinder(r: f64, h: f64, id: u32) -> Solid {
        let mut s = Sketch::new();
        s.add_circle(Vec2::ZERO, r);
        extrude(
            &s.profiles(&ProfileOptions::default()).remove(0),
            &Plane::XY,
            0.0,
            h,
            id,
        )
        .unwrap()
    }

    /// Every vertex lies on each of its analytic surfaces and every face
    /// is planar to a small multiple of the merge tolerance.
    fn assert_refitted(solid: &Solid) {
        let vf = vertex_faces(solid);
        for (v, faces) in vf.iter().enumerate() {
            let p = solid.vertices[v];
            for &f in faces {
                let s = &solid.surfaces[solid.faces[f].surface];
                if is_analytic(s) {
                    assert!(
                        p.distance(project(s, p)) < 1e-6,
                        "vertex {v} off its surface"
                    );
                }
            }
        }
        for f in &solid.faces {
            for &v in f.loops.iter().flatten() {
                let off = f
                    .plane
                    .normal
                    .dot(solid.vertices[v as usize] - f.plane.origin);
                assert!(off.abs() < 1e-6, "face bent by {off}");
            }
        }
        solid.validate().unwrap();
    }

    fn cut_by_tilted_plane(body: &Solid, angle_deg: f64, z: f64) -> (Solid, Vec3) {
        let a = angle_deg.to_radians();
        let normal = Vec3::new(0.0, -a.sin(), a.cos());
        let plane = Plane::from_origin_normal(Vec3::new(0.0, 0.0, z), normal).unwrap();
        let mut b = Sketch::new();
        b.add_rectangle(Vec2::new(-40.0, -40.0), Vec2::new(40.0, 40.0));
        let wedge = extrude(
            &b.profiles(&ProfileOptions::default()).remove(0),
            &plane,
            0.0,
            40.0,
            2,
        )
        .unwrap();
        (boolean(body, &wedge, BoolOp::Difference).unwrap(), normal)
    }

    #[test]
    fn a_tilted_cut_of_a_cylinder_is_an_ellipse_and_its_vertices_snap_to_it() {
        let (cut, normal) = cut_by_tilted_plane(&cylinder(10.0, 30.0, 1), 30.0, 20.0);
        let runs = edge_runs(&cut);
        // The rim: the one closed run between the cylinder and the cut plane.
        let cyl = cut
            .surfaces
            .iter()
            .position(|s| matches!(s, Surface::Cylinder { .. }))
            .unwrap();
        let top = cut
            .faces
            .iter()
            .find(|f| f.plane.normal.approx_eq(normal))
            .unwrap()
            .surface;
        let rim: Vec<&Run> = runs
            .iter()
            .filter(|r| r.surfaces == (cyl.min(top), cyl.max(top)))
            .collect();
        assert_eq!(rim.len(), 1);
        assert!(rim[0].closed);
        assert_eq!(rim[0].vertices.len(), 73, "72 facets round plus the repeat");
        match edge_curve(&cut.surfaces[top], &cut.surfaces[cyl]) {
            Curve::Ellipse {
                center,
                a,
                b,
                major,
                axis,
            } => {
                assert!(center.distance(Vec3::new(0.0, 0.0, 20.0)) < 1e-9);
                assert!((a - 10.0 / 30f64.to_radians().cos()).abs() < 1e-9);
                assert!((b - 10.0).abs() < 1e-9);
                assert!(major.dot(axis).abs() < 1e-9);
            }
            other => panic!("{other:?}"),
        }
        // Exact positions lie on both the cylinder and the plane.
        let vf = vertex_faces(&cut);
        for &v in &rim[0].vertices {
            let p = vertex_position(&cut, &vf, v);
            let radial = (p.x * p.x + p.y * p.y).sqrt();
            assert!((radial - 10.0).abs() < 1e-9, "radius {radial}");
            assert!((normal.dot(p) - normal.dot(Vec3::new(0.0, 0.0, 20.0))).abs() < 1e-9);
        }
        // The bottom rim is a circle; the wall's seams are 72 rulings.
        let bottom = cut
            .faces
            .iter()
            .find(|f| f.plane.normal.approx_eq(-Vec3::Z))
            .unwrap()
            .surface;
        assert!(
            matches!(edge_curve(&cut.surfaces[bottom], &cut.surfaces[cyl]), Curve::Circle { radius, .. } if (radius - 10.0).abs() < 1e-9)
        );
        assert_eq!(rulings(&cut, cyl).len(), 72);
        // Every vertex sits on its surfaces and every facet is planar.
        assert_refitted(&cut);
        // The cylinder's region is one loop around: rim, then the bottom.
        let regions = surface_regions(&cut);
        let loops = &regions[&cyl];
        assert_eq!(loops.len(), 2, "top rim and bottom rim");
        assert!(loops.iter().all(|l| l.len() == 72));
    }

    #[test]
    fn refit_survives_oblique_drills_unions_and_repeated_cuts() {
        let body = cylinder(10.0, 30.0, 1);
        // A drill tilted 35° from the wall's normal, off the axis.
        let a = 35f64.to_radians();
        let dir = Vec3::new(a.cos(), 0.0, a.sin());
        let plane = Plane::from_origin_normal(Vec3::new(0.0, 3.0, 12.0), dir).unwrap();
        let mut s = Sketch::new();
        s.add_circle(Vec2::ZERO, 3.5);
        let drill = extrude(
            &s.profiles(&ProfileOptions::default()).remove(0),
            &plane,
            -25.0,
            25.0,
            2,
        )
        .unwrap();
        let cut = boolean(&body, &drill, BoolOp::Difference).unwrap();
        assert_refitted(&cut);
        // A boss unioned across it, then a second drill through both.
        let mut s = Sketch::new();
        s.add_circle(Vec2::new(0.0, 20.0), 6.0);
        let yz = Plane {
            origin: Vec3::ZERO,
            x_axis: Vec3::Y,
            y_axis: Vec3::Z,
            normal: Vec3::X,
        };
        let boss = extrude(
            &s.profiles(&ProfileOptions::default()).remove(0),
            &yz,
            0.0,
            18.0,
            3,
        )
        .unwrap();
        let joined = boolean(&cut, &boss, BoolOp::Union).unwrap();
        assert_refitted(&joined);
        let mut s = Sketch::new();
        s.add_circle(Vec2::new(0.0, 20.0), 2.0);
        let through = extrude(
            &s.profiles(&ProfileOptions::default()).remove(0),
            &yz,
            -30.0,
            30.0,
            4,
        )
        .unwrap();
        let bored = boolean(&joined, &through, BoolOp::Difference).unwrap();
        assert_refitted(&bored);
        // The bore runs through the main cylinder (20 long at y = 0) and
        // on through the boss to x = 18: about 28 of radius 2.
        let exact_bore = std::f64::consts::PI * 4.0 * 28.0;
        let removed = joined.volume() - bored.volume();
        assert!(
            (removed - exact_bore).abs() < 0.02 * exact_bore,
            "bore removed {removed}, expected about {exact_bore}"
        );
    }

    #[test]
    fn a_cross_hole_meets_the_wall_on_a_quartic_sampled_at_both_rulings() {
        let body = cylinder(10.0, 30.0, 1);
        let mut s = Sketch::new();
        s.add_circle(Vec2::new(0.0, 15.0), 4.0);
        // A drill along X through the wall at z = 15 (sketch on the YZ plane,
        // its y axis along Z).
        let yz = Plane {
            origin: Vec3::ZERO,
            x_axis: Vec3::Y,
            y_axis: Vec3::Z,
            normal: Vec3::X,
        };
        let drill = extrude(
            &s.profiles(&ProfileOptions::default()).remove(0),
            &yz,
            -20.0,
            20.0,
            2,
        )
        .unwrap();
        let cut = boolean(&body, &drill, BoolOp::Difference).unwrap();
        assert_refitted(&cut);
        let cyls: Vec<usize> = cut
            .surfaces
            .iter()
            .enumerate()
            .filter(|(_, s)| matches!(s, Surface::Cylinder { .. }))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(cyls.len(), 2);
        assert_eq!(
            edge_curve(&cut.surfaces[cyls[0]], &cut.surfaces[cyls[1]]),
            Curve::Quartic
        );
        let runs = edge_runs(&cut);
        let quartics: Vec<&Run> = runs
            .iter()
            .filter(|r| r.surfaces == (cyls[0], cyls[1]))
            .collect();
        assert_eq!(
            quartics.len(),
            2,
            "one loop where the drill enters, one where it leaves"
        );
        let vf = vertex_faces(&cut);
        let wall = &cut.surfaces[cyls[0]];
        let hole = &cut.surfaces[cyls[1]];
        for run in &quartics {
            assert!(run.closed);
            for &v in &run.vertices {
                let p = vertex_position(&cut, &vf, v);
                assert!(p.distance(project(wall, p)) < 1e-9);
                assert!(p.distance(project(hole, p)) < 1e-9);
                // The boolean refitted its result: the vertex is already there.
                assert!(cut.vertices[v as usize].distance(p) < 1e-9);
            }
            // Sampling at a finer set of the wall's rulings adds points on both surfaces.
            let fine: Vec<f64> = (0..360)
                .map(|k| (k as f64).to_radians() - std::f64::consts::PI)
                .collect();
            let pts = run_points(&cut, &vf, run, Some((cyls[0], &fine)));
            // The loop spans about 47 degrees of the wall either way,
            // so it crosses well over sixty fine rulings that are not
            // facet edges.
            assert!(pts.len() > run.vertices.len() + 60, "{} points", pts.len());
            for p in &pts {
                assert!(p.distance(project(wall, *p)) < 1e-9);
                assert!(p.distance(project(hole, *p)) < 1e-6, "{p:?}");
            }
        }
    }
}
