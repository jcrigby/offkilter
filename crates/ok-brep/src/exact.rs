//! Exact geometry recovered from a polyhedral solid's surface tags.
//!
//! Every edge lies between two faces; where both faces' surfaces are
//! analytic the edge's exact curve follows from the pair (a line, a
//! circle, an ellipse, or the quartic where two cylinders meet), and a
//! vertex's exact position is the point on all of its analytic surfaces.
//! Nothing here changes the solid: callers ask for exact positions and
//! curve samples where they need them (STEP export, refitting).

use crate::{edge_key, EdgeKey, Solid, Surface};
use ok_math::Vec3;
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

/// The exact position of a vertex: the point on all of its analytic
/// surfaces (a vertex touching a non-analytic surface keeps its place).
pub fn vertex_position(solid: &Solid, vertex_faces: &[Vec<usize>], v: u32) -> Vec3 {
    let p = solid.vertices[v as usize];
    let mut surfaces: Vec<usize> = vertex_faces[v as usize]
        .iter()
        .map(|&f| solid.faces[f].surface)
        .collect();
    surfaces.sort_unstable();
    surfaces.dedup();
    if surfaces.iter().any(|&s| !is_analytic(&solid.surfaces[s])) {
        return p;
    }
    let refs: Vec<&Surface> = surfaces.iter().map(|&s| &solid.surfaces[s]).collect();
    project_all(&refs, p)
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
    for (key, faces) in solid.edge_faces() {
        if faces.len() != 2 {
            continue;
        }
        let (mut ia, mut ib) = (faces[0], faces[1]);
        if solid.faces[ia].surface > solid.faces[ib].surface {
            std::mem::swap(&mut ia, &mut ib);
        }
        let (sa, sb) = (solid.faces[ia].surface, solid.faces[ib].surface);
        if sa == sb {
            continue;
        }
        let mut dir = None;
        for l in &solid.faces[ia].loops {
            for i in 0..l.len() {
                let (a, b) = (l[i], l[(i + 1) % l.len()]);
                if edge_key(a, b) == key {
                    dir = Some((a, b));
                }
            }
        }
        if let Some((a, b)) = dir {
            segments.push((a, b, (sa, sb)));
        }
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
            loop {
                let first = segments[chain[0]].0;
                let prev = by_end
                    .get(&(first, pair))
                    .and_then(|v| v.iter().copied().find(|&j| !used[j]));
                match prev {
                    Some(j) => {
                        used[j] = true;
                        chain.insert(0, j);
                    }
                    None => break,
                }
            }
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
                if rho <= lo + 1e-9 || rho >= hi - 1e-9 {
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
        // The cylinder's region is one loop around: rim, then the bottom.
        let regions = surface_regions(&cut);
        let loops = &regions[&cyl];
        assert_eq!(loops.len(), 2, "top rim and bottom rim");
        assert!(loops.iter().all(|l| l.len() == 72));
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
            }
            // Sampling at a finer set of the wall's rulings adds points on both surfaces.
            let fine: Vec<f64> = (0..360)
                .map(|k| (k as f64).to_radians() - std::f64::consts::PI)
                .collect();
            let pts = run_points(&cut, &vf, run, Some((cyls[0], &fine)));
            assert!(pts.len() > run.vertices.len() * 2, "{} points", pts.len());
            for p in &pts {
                assert!(p.distance(project(wall, *p)) < 1e-9);
                assert!(p.distance(project(hole, *p)) < 1e-6, "{p:?}");
            }
        }
    }
}
