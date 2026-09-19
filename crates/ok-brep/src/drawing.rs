//! Orthographic drawing views with hidden-line removal.
//!
//! A view projects the edges of polyhedral solids onto a plane and splits
//! every edge into the parts a viewer would see and the parts hidden
//! behind faces. Everything is exact for the faceted geometry we have:
//! an edge is hidden wherever its projection lies inside (or on the
//! outline of) the projection of a face that faces the viewer and is
//! nearer than the edge there. Silhouette seams of curved surfaces (a facet facing the viewer
//! next to one facing away) are drawn like edges, so faceted cylinders
//! get their outline.

use crate::Solid;
use ok_math::{Vec2, Vec3};

/// Segments of a view in view coordinates (x right, y up, millimetres).
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ViewLines {
    pub visible: Vec<[Vec2; 2]>,
    pub hidden: Vec<[Vec2; 2]>,
}

/// Frame of a view: the viewer looks along `dir`; `up` is the screen's up.
#[derive(Debug, Clone, Copy)]
pub struct View {
    pub dir: Vec3,
    pub up: Vec3,
}

impl View {
    /// The screen basis (right, up, depth): depth grows away from the viewer.
    fn basis(&self) -> Option<(Vec3, Vec3, Vec3)> {
        let d = self.dir.normalized()?;
        let u = d.cross(self.up).normalized()?;
        let v = u.cross(d);
        Some((u, v, d))
    }
}

struct Occluder {
    loops: Vec<Vec<Vec2>>,
    min: Vec2,
    max: Vec2,
    /// Depth as a function of view position: `depth = a·x + b·y + c`.
    depth: (f64, f64, f64),
    solid: usize,
    face: usize,
}

/// Projects `solids` into the view and removes hidden lines.
pub fn project_view(solids: &[&Solid], view: View) -> ViewLines {
    let Some((u, v, d)) = view.basis() else {
        return ViewLines::default();
    };
    let to2 = |p: Vec3| Vec2::new(p.dot(u), p.dot(v));
    let depth = |p: Vec3| p.dot(d);
    let scale = solids
        .iter()
        .filter_map(|s| s.bounds())
        .map(|(lo, hi)| (hi - lo).length())
        .fold(1.0, f64::max);
    let eps = 1e-7 * scale;

    // Faces that face the viewer hide what lies behind them.
    let mut occluders: Vec<Occluder> = Vec::new();
    for (si, s) in solids.iter().enumerate() {
        for (fi, f) in s.faces.iter().enumerate() {
            let n = f.plane.normal;
            if n.dot(d) >= -1e-9 {
                continue; // faces away from or edge-on to the viewer
            }
            let loops: Vec<Vec<Vec2>> = f
                .loops
                .iter()
                .map(|l| l.iter().map(|&i| to2(s.vertices[i as usize])).collect())
                .collect();
            let (mut min, mut max) = (Vec2::new(f64::MAX, f64::MAX), Vec2::new(f64::MIN, f64::MIN));
            for p in loops.iter().flatten() {
                min = Vec2::new(min.x.min(p.x), min.y.min(p.y));
                max = Vec2::new(max.x.max(p.x), max.y.max(p.y));
            }
            // depth(p) for p on the plane: n·p = n·o, with p = x u + y v + t d:
            // t = (n·o - x n·u - y n·v) / n·d.
            let nd = n.dot(d);
            let no = n.dot(f.plane.origin);
            occluders.push(Occluder {
                loops,
                min,
                max,
                depth: (-n.dot(u) / nd, -n.dot(v) / nd, no / nd),
                solid: si,
                face: fi,
            });
        }
    }

    let mut out = ViewLines::default();
    for (si, s) in solids.iter().enumerate() {
        let mut edges: Vec<_> = s.edge_faces().into_iter().collect();
        edges.sort_by_key(|(k, _)| *k);
        for ((a, b), faces) in edges {
            let show = match faces.as_slice() {
                [f0, f1] => {
                    let (fa, fb) = (&s.faces[*f0], &s.faces[*f1]);
                    let front = |f: &crate::Face| f.plane.normal.dot(d) < -1e-9;
                    let silhouette = front(fa) != front(fb);
                    let real_edge = if fa.surface == fb.surface {
                        false
                    } else {
                        let coplanar = fa.plane.normal.dot(fb.plane.normal) > 1.0 - 1e-9;
                        let both_planar = !s.surfaces[fa.surface].is_smooth()
                            && !s.surfaces[fb.surface].is_smooth();
                        !(coplanar && both_planar)
                    };
                    real_edge || silhouette
                }
                _ => true,
            };
            if !show {
                continue;
            }
            let (pa, pb) = (s.vertices[a as usize], s.vertices[b as usize]);
            let (qa, qb) = (to2(pa), to2(pb));
            if qa.distance(qb) <= eps {
                continue; // seen end-on
            }
            let (za, zb) = (depth(pa), depth(pb));
            let mut hidden: Vec<(f64, f64)> = Vec::new();
            let (smin, smax) = (
                Vec2::new(qa.x.min(qb.x), qa.y.min(qb.y)),
                Vec2::new(qa.x.max(qb.x), qa.y.max(qb.y)),
            );
            for occ in &occluders {
                if occ.solid == si && faces.contains(&occ.face) {
                    continue;
                }
                if occ.max.x < smin.x - eps
                    || occ.min.x > smax.x + eps
                    || occ.max.y < smin.y - eps
                    || occ.min.y > smax.y + eps
                {
                    continue;
                }
                for (t0, t1) in inside_intervals(qa, qb, &occ.loops, eps) {
                    // Depth of the edge minus depth of the face, linear in t.
                    let at = |t: f64| {
                        let p = qa + (qb - qa) * t;
                        let z = za + (zb - za) * t;
                        z - (occ.depth.0 * p.x + occ.depth.1 * p.y + occ.depth.2)
                    };
                    let (d0, d1) = (at(t0), at(t1));
                    // Hidden where the edge is behind the face (positive).
                    match (d0 > eps, d1 > eps) {
                        (true, true) => hidden.push((t0, t1)),
                        (false, false) => {}
                        (h0, _) => {
                            let tc = t0 + (t1 - t0) * (d0 / (d0 - d1));
                            if h0 {
                                hidden.push((t0, tc));
                            } else {
                                hidden.push((tc, t1));
                            }
                        }
                    }
                }
            }
            let hidden = merge_intervals(hidden, 1e-9);
            let seg = |t0: f64, t1: f64| [qa + (qb - qa) * t0, qa + (qb - qa) * t1];
            let mut cursor = 0.0;
            for &(h0, h1) in &hidden {
                if h0 > cursor + 1e-9 {
                    out.visible.push(seg(cursor, h0));
                }
                out.hidden.push(seg(h0, h1));
                cursor = h1;
            }
            if cursor < 1.0 - 1e-9 {
                out.visible.push(seg(cursor, 1.0));
            }
        }
    }
    // Edges that project onto one another (two edges of a box seen square
    // on, both hidden behind a nearer part) would be drawn twice, and a
    // hidden edge behind a visible one would put dashes over a solid line:
    // keep one copy of overlapping collinear runs, visible winning.
    out.visible = dedupe_collinear(&out.visible, eps);
    out.hidden = dedupe_collinear(&out.hidden, eps);
    out.hidden = subtract_collinear(&out.hidden, &out.visible, eps);
    out.visible.retain(|s| s[0].distance(s[1]) > eps);
    out.hidden.retain(|s| s[0].distance(s[1]) > eps);
    out
}

/// Keeps one copy of every collinear overlap among `segments`.
fn dedupe_collinear(segments: &[[Vec2; 2]], eps: f64) -> Vec<[Vec2; 2]> {
    let mut kept: Vec<[Vec2; 2]> = Vec::new();
    for seg in segments {
        let pieces = subtract_collinear(std::slice::from_ref(seg), &kept, eps);
        kept.extend(pieces);
    }
    kept
}

/// Parameter intervals of the segment `a`→`b` that lie strictly inside the
/// polygon with holes `loops` (even-odd rule).
fn inside_intervals(a: Vec2, b: Vec2, loops: &[Vec<Vec2>], eps: f64) -> Vec<(f64, f64)> {
    let dir = b - a;
    let len2 = dir.length_squared();
    let mut cuts = vec![0.0, 1.0];
    for l in loops {
        for i in 0..l.len() {
            let (p, q) = (l[i], l[(i + 1) % l.len()]);
            let e = q - p;
            let den = dir.cross(e);
            if den.abs() <= 1e-12 * len2.max(1.0) {
                continue; // parallel
            }
            let t = (p - a).cross(e) / den;
            let s = (p - a).cross(dir) / den;
            if t > 0.0 && t < 1.0 && (0.0..=1.0).contains(&s) {
                cuts.push(t);
            }
        }
    }
    cuts.sort_by(|x, y| x.partial_cmp(y).unwrap());
    cuts.dedup_by(|x, y| (*x - *y).abs() <= 1e-12);
    let mut out = Vec::new();
    for w in cuts.windows(2) {
        let (t0, t1) = (w[0], w[1]);
        if (t1 - t0) * len2.sqrt() <= eps {
            continue;
        }
        let mid = a + dir * ((t0 + t1) / 2.0);
        if point_in_loops(mid, loops, eps) {
            out.push((t0, t1));
        }
    }
    out
}

/// Even-odd point-in-polygon over all loops; points within `eps` of an
/// edge count as inside, so an edge running along a nearer face's outline
/// (the back edges of a box seen square on) is covered by it.
fn point_in_loops(p: Vec2, loops: &[Vec<Vec2>], eps: f64) -> bool {
    let mut inside = false;
    for l in loops {
        for i in 0..l.len() {
            let (a, b) = (l[i], l[(i + 1) % l.len()]);
            let e = b - a;
            let len = e.length();
            if len > 0.0 {
                let t = ((p - a).dot(e) / (len * len)).clamp(0.0, 1.0);
                if (a + e * t).distance(p) <= eps {
                    return true;
                }
            }
            if (a.y > p.y) != (b.y > p.y) {
                let x = a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y);
                if x > p.x {
                    inside = !inside;
                }
            }
        }
    }
    inside
}

fn merge_intervals(mut v: Vec<(f64, f64)>, eps: f64) -> Vec<(f64, f64)> {
    v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mut out: Vec<(f64, f64)> = Vec::new();
    for (s, e) in v {
        if let Some(last) = out.last_mut() {
            if s <= last.1 + eps {
                last.1 = last.1.max(e);
                continue;
            }
        }
        out.push((s, e));
    }
    out
}

/// Removes from `from` the parts that overlap a collinear segment of `by`.
fn subtract_collinear(from: &[[Vec2; 2]], by: &[[Vec2; 2]], eps: f64) -> Vec<[Vec2; 2]> {
    let mut out = Vec::new();
    for seg in from {
        let (a, b) = (seg[0], seg[1]);
        let dir = b - a;
        let len = dir.length();
        let mut covered: Vec<(f64, f64)> = Vec::new();
        for other in by {
            let (p, q) = (other[0], other[1]);
            // Collinear: both endpoints within eps of the line through a-b.
            let off = |x: Vec2| ((x - a).cross(dir) / len).abs();
            if off(p) > eps || off(q) > eps {
                continue;
            }
            let tp = (p - a).dot(dir) / (len * len);
            let tq = (q - a).dot(dir) / (len * len);
            let (t0, t1) = (tp.min(tq).max(0.0), tp.max(tq).min(1.0));
            if t1 > t0 {
                covered.push((t0, t1));
            }
        }
        let covered = merge_intervals(covered, 1e-9);
        let mut cursor = 0.0;
        for (c0, c1) in covered {
            if c0 > cursor + 1e-9 {
                out.push([a + dir * cursor, a + dir * c0]);
            }
            cursor = c1;
        }
        if cursor < 1.0 - 1e-9 {
            out.push([a + dir * cursor, b]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{boolean, extrude, BoolOp};
    use ok_math::Plane;
    use ok_sketch::{ProfileOptions, Sketch};

    fn block(w: f64, d: f64, h: f64) -> Solid {
        let mut s = Sketch::new();
        s.add_rectangle(Vec2::ZERO, Vec2::new(w, d));
        let p = s.profiles(&ProfileOptions::default()).remove(0);
        extrude(&p, &Plane::XY, 0.0, h, 1).unwrap()
    }

    const FRONT: View = View {
        dir: Vec3 {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        },
        up: Vec3::Z,
    };
    const TOP: View = View {
        dir: Vec3 {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        },
        up: Vec3::Y,
    };

    fn total_length(v: &[[Vec2; 2]]) -> f64 {
        v.iter().map(|s| s[0].distance(s[1])).sum()
    }

    #[test]
    fn box_seen_square_on_is_its_outline() {
        let b = block(30.0, 20.0, 10.0);
        let lines = project_view(&[&b], FRONT);
        // Four visible outline edges (front face), nothing dashed: the back
        // edges coincide with the front ones.
        assert_eq!(lines.visible.len(), 4, "{:?}", lines.visible);
        assert!(lines.hidden.is_empty(), "{:?}", lines.hidden);
        assert!((total_length(&lines.visible) - 2.0 * (30.0 + 10.0)).abs() < 1e-9);
        // The view's x is world x and its y is world z.
        let xs: Vec<f64> = lines.visible.iter().flatten().map(|p| p.x).collect();
        let ys: Vec<f64> = lines.visible.iter().flatten().map(|p| p.y).collect();
        assert!(xs
            .iter()
            .all(|&x| (x - 0.0).abs() < 1e-9 || (x - 30.0).abs() < 1e-9));
        assert!(ys
            .iter()
            .all(|&y| (y - 0.0).abs() < 1e-9 || (y - 10.0).abs() < 1e-9));
    }

    #[test]
    fn box_in_isometric_view_shows_nine_edges_and_hides_three() {
        let b = block(30.0, 20.0, 10.0);
        let iso = View {
            dir: Vec3::new(-0.6, 0.7, -0.5),
            up: Vec3::Z,
        };
        let lines = project_view(&[&b], iso);
        assert_eq!(lines.visible.len(), 9, "{:?}", lines.visible);
        assert_eq!(lines.hidden.len(), 3, "{:?}", lines.hidden);
    }

    #[test]
    fn through_hole_appears_as_hidden_lines_from_the_side() {
        let plate = block(40.0, 30.0, 8.0);
        let mut s = Sketch::new();
        s.add_circle(Vec2::new(20.0, 15.0), 5.0);
        let p = s.profiles(&ProfileOptions::default()).remove(0);
        let drill = extrude(&p, &Plane::XY, -1.0, 9.0, 2).unwrap();
        let plate = boolean(&plate, &drill, BoolOp::Difference).unwrap();
        let front = project_view(&[&plate], FRONT);
        // Outline plus two dashed verticals at the hole's extremes.
        assert_eq!(front.visible.len(), 4, "{:?}", front.visible);
        let mut xs: Vec<f64> = front.hidden.iter().map(|s| s[0].x).collect();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        xs.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        assert_eq!(xs.len(), 2, "hidden {:?}", front.hidden);
        assert!((xs[0] - 15.0).abs() < 1e-6 && (xs[1] - 25.0).abs() < 1e-6);
        assert!((total_length(&front.hidden) - 16.0).abs() < 1e-6);
        // From above the rim is visible (one facet edge per segment) and
        // nothing is hidden.
        let top = project_view(&[&plate], TOP);
        assert!(top.hidden.is_empty(), "{:?}", top.hidden);
        assert!(top.visible.len() > 4 + 30);
    }

    #[test]
    fn a_block_in_front_hides_part_of_another() {
        let back = block(40.0, 10.0, 10.0);
        let front_block = block(10.0, 10.0, 20.0)
            .transformed(&crate::Transform::translation(Vec3::new(15.0, -20.0, 0.0)));
        let lines = project_view(&[&back, &front_block], FRONT);
        // The back block's top edge is split around the front block.
        let top_edges: Vec<&[Vec2; 2]> = lines
            .visible
            .iter()
            .filter(|s| (s[0].y - 10.0).abs() < 1e-9 && (s[1].y - 10.0).abs() < 1e-9)
            .collect();
        let visible_top: f64 = top_edges.iter().map(|s| s[0].distance(s[1])).sum();
        assert!((visible_top - 30.0).abs() < 1e-9, "{top_edges:?}");
        let hidden_top: f64 = lines
            .hidden
            .iter()
            .filter(|s| (s[0].y - 10.0).abs() < 1e-9)
            .map(|s| s[0].distance(s[1]))
            .sum();
        assert!((hidden_top - 10.0).abs() < 1e-9, "{:?}", lines.hidden);
    }
}
