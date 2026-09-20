//! Orthographic drawing views with hidden-line removal.
//!
//! A view projects the edges of polyhedral solids onto a plane and splits
//! every edge into the parts a viewer would see and the parts hidden
//! behind faces. Everything is exact for the faceted geometry we have:
//! an edge is hidden wherever its projection lies inside (or on the
//! outline of) the projection of a face that faces the viewer and is
//! nearer than the edge there. Silhouette seams of curved surfaces (a
//! facet facing the viewer next to one facing away) are drawn like
//! edges, so faceted cylinders get their outline. Edges on an exact
//! circle or ellipse (a cylinder's rims) come back as arcs of the
//! ellipse they project to rather than as their facet chords: the
//! hidden-line work is done on the chords, and the visible and hidden
//! pieces are then joined into arcs.

use crate::exact::{self, Curve};
use crate::{edge_key, EdgeKey, Solid};
use ok_math::{Plane, Vec2, Vec3};
use std::collections::HashMap;
use std::f64::consts::{PI, TAU};

/// A piece of an ellipse in view coordinates: a circle or ellipse edge
/// seen obliquely. Its points are `center + major·cos t + minor·sin t`,
/// where `minor` is `major` turned a quarter turn counter-clockwise and
/// scaled by `ratio`, for `t` from `start` to `end` (radians; `end` is
/// past `start` by at most a full turn, which is a whole ellipse).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ViewArc {
    pub center: Vec2,
    pub major: Vec2,
    pub ratio: f64,
    pub start: f64,
    pub end: f64,
}

/// Segments and arcs of a view in view coordinates (x right, y up,
/// millimetres).
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ViewLines {
    pub visible: Vec<[Vec2; 2]>,
    pub hidden: Vec<[Vec2; 2]>,
    #[serde(default)]
    pub visible_arcs: Vec<ViewArc>,
    #[serde(default)]
    pub hidden_arcs: Vec<ViewArc>,
}

/// A view as a DXF (R2000: LINE, CIRCLE, ARC and ELLIPSE) at 1:1 in view
/// coordinates, millimetres: visible edges on layer VISIBLE and, when
/// `hidden`, hidden ones dashed on layer HIDDEN. A template to print or a
/// profile to cut.
pub fn view_dxf(lines: &ViewLines, hidden: bool) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut put = |items: &[&str]| out.extend(items.iter().map(|s| s.to_string()));
    put(&[
        "0",
        "SECTION",
        "2",
        "HEADER",
        "9",
        "$ACADVER",
        "1",
        "AC1015",
        "9",
        "$INSUNITS",
        "70",
        "4",
        "0",
        "ENDSEC",
        "0",
        "SECTION",
        "2",
        "TABLES",
        "0",
        "TABLE",
        "2",
        "LAYER",
        "70",
        "2",
        "0",
        "LAYER",
        "2",
        "VISIBLE",
        "70",
        "0",
        "62",
        "7",
        "6",
        "CONTINUOUS",
        "0",
        "LAYER",
        "2",
        "HIDDEN",
        "70",
        "0",
        "62",
        "8",
        "6",
        "DASHED",
        "0",
        "ENDTAB",
        "0",
        "ENDSEC",
        "0",
        "SECTION",
        "2",
        "ENTITIES",
    ]);
    type Layer<'a> = (&'a str, &'a [[Vec2; 2]], &'a [ViewArc]);
    let layers: &[Layer] = if hidden {
        &[
            ("VISIBLE", &lines.visible, &lines.visible_arcs),
            ("HIDDEN", &lines.hidden, &lines.hidden_arcs),
        ]
    } else {
        &[("VISIBLE", &lines.visible, &lines.visible_arcs)]
    };
    for (layer, segments, arcs) in layers {
        for [a, b] in segments.iter() {
            put(&[
                "0",
                "LINE",
                "8",
                layer,
                "10",
                &dxf_num(a.x),
                "20",
                &dxf_num(a.y),
                "30",
                "0",
                "11",
                &dxf_num(b.x),
                "21",
                &dxf_num(b.y),
                "31",
                "0",
            ]);
        }
        for arc in arcs.iter() {
            let r = arc.major.length();
            let full = arc.end - arc.start >= TAU - 1e-9;
            let (cx, cy) = (dxf_num(arc.center.x), dxf_num(arc.center.y));
            if (arc.ratio - 1.0).abs() < 1e-9 {
                if full {
                    put(&[
                        "0",
                        "CIRCLE",
                        "8",
                        layer,
                        "10",
                        &cx,
                        "20",
                        &cy,
                        "30",
                        "0",
                        "40",
                        &dxf_num(r),
                    ]);
                } else {
                    let base = arc.major.y.atan2(arc.major.x);
                    let deg = |t: f64| dxf_num(((base + t).to_degrees() % 360.0 + 360.0) % 360.0);
                    put(&[
                        "0",
                        "ARC",
                        "8",
                        layer,
                        "10",
                        &cx,
                        "20",
                        &cy,
                        "30",
                        "0",
                        "40",
                        &dxf_num(r),
                        "50",
                        &deg(arc.start),
                        "51",
                        &deg(arc.end),
                    ]);
                }
            } else {
                put(&[
                    "0",
                    "ELLIPSE",
                    "8",
                    layer,
                    "10",
                    &cx,
                    "20",
                    &cy,
                    "30",
                    "0",
                    "11",
                    &dxf_num(arc.major.x),
                    "21",
                    &dxf_num(arc.major.y),
                    "31",
                    "0",
                    "40",
                    &dxf_num(arc.ratio),
                    "41",
                    &dxf_num(if full { 0.0 } else { arc.start }),
                    "42",
                    &dxf_num(if full { TAU } else { arc.end }),
                ]);
            }
        }
    }
    put(&["0", "ENDSEC", "0", "EOF"]);
    out.join("\n") + "\n"
}

/// A DXF number: six decimals, trailing zeros dropped, `-0` avoided.
fn dxf_num(v: f64) -> String {
    let s = format!("{:.6}", if v.abs() < 5e-7 { 0.0 } else { v });
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() {
        "0".into()
    } else {
        s.into()
    }
}

/// A section view: the lines of what is left after cutting, plus the
/// outlines of the cut faces (each a closed polygon in view coordinates)
/// for hatching.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SectionLines {
    pub visible: Vec<[Vec2; 2]>,
    pub hidden: Vec<[Vec2; 2]>,
    #[serde(default)]
    pub visible_arcs: Vec<ViewArc>,
    #[serde(default)]
    pub hidden_arcs: Vec<ViewArc>,
    pub cut: Vec<Vec<Vec2>>,
}

/// Cuts every solid by `plane`, removing the material on its normal side,
/// and draws what remains as seen from `view` (normally looking along
/// `-plane.normal`, at the cut). Faces lying in the cut plane come back
/// as `cut` polygons. A solid the cut fails on is drawn whole.
pub fn section_view(solids: &[&Solid], view: View, plane: &Plane) -> SectionLines {
    let Some((u, v, _)) = view.basis() else {
        return SectionLines::default();
    };
    let to2 = |p: Vec3| Vec2::new(p.dot(u), p.dot(v));
    let halves: Vec<Solid> = solids.iter().map(|s| cut_solid(s, plane)).collect();
    let mut cut = Vec::new();
    let level = plane.normal.dot(plane.origin);
    for half in &halves {
        let eps = 1e-6
            * half
                .bounds()
                .map_or(1.0, |(lo, hi)| (hi - lo).length().max(1.0));
        for f in &half.faces {
            let on_plane = f.plane.normal.dot(plane.normal) > 1.0 - 1e-6
                && (f.plane.normal.dot(f.plane.origin) - level).abs() <= eps;
            if !on_plane {
                continue;
            }
            for l in &f.loops {
                cut.push(l.iter().map(|&i| to2(half.vertices[i as usize])).collect());
            }
        }
    }
    let refs: Vec<&Solid> = halves.iter().collect();
    let lines = project_view(&refs, view);
    SectionLines {
        visible: lines.visible,
        hidden: lines.hidden,
        visible_arcs: lines.visible_arcs,
        hidden_arcs: lines.hidden_arcs,
        cut,
    }
}

/// The part of `solid` on the far side of `plane` (against its normal):
/// the solid minus a box covering the normal side. The solid itself when
/// the plane misses it or the boolean fails.
fn cut_solid(solid: &Solid, plane: &Plane) -> Solid {
    match split(solid, plane) {
        Ok(Some((below, _))) => below,
        // Entirely on the removed side: keep it drawn whole rather than
        // vanish (a section through empty air is a user slip).
        _ => solid.clone(),
    }
}

/// Splits `solid` by `plane` into the part against the plane's normal
/// and the part on its normal side, each a closed solid whose new faces
/// are tagged with `feature`. `None` when the plane misses the solid
/// (nothing to split). The cut is a boolean against a box covering one
/// side, so it inherits the boolean's handling of coincident faces.
pub fn split(solid: &Solid, plane: &Plane) -> Result<Option<(Solid, Solid)>, crate::BrepError> {
    split_tagged(solid, plane, u32::MAX)
}

/// `split` with the faces created by the cut tagged as `feature`.
pub fn split_tagged(
    solid: &Solid,
    plane: &Plane,
    feature: u32,
) -> Result<Option<(Solid, Solid)>, crate::BrepError> {
    let Some((lo, hi)) = solid.bounds() else {
        return Ok(None);
    };
    let extent = (hi - lo).length().max(1.0);
    let centre = (lo + hi) * 0.5;
    let level = plane.normal.dot(plane.origin);
    let (mut min_d, mut max_d) = (f64::MAX, f64::MIN);
    for v in &solid.vertices {
        let d = plane.normal.dot(*v) - level;
        min_d = min_d.min(d);
        max_d = max_d.max(d);
    }
    let eps = 1e-9 * extent;
    if max_d <= eps || min_d >= -eps {
        return Ok(None);
    }
    // A box on the normal side, big enough to cover the solid; its base
    // sits in the cut plane and is centred under the solid.
    let c2 = plane.to_plane(centre);
    let half = extent * 4.0;
    let mut sk = ok_sketch::Sketch::new();
    sk.add_rectangle(
        Vec2::new(c2.x - half, c2.y - half),
        Vec2::new(c2.x + half, c2.y + half),
    );
    let profile = sk
        .profiles(&ok_sketch::ProfileOptions::default())
        .into_iter()
        .next()
        .ok_or_else(|| crate::BrepError::Degenerate("split box".into()))?;
    let cutter = crate::extrude(&profile, plane, 0.0, half, feature)?;
    let below = crate::boolean(solid, &cutter, crate::BoolOp::Difference)?;
    let above = crate::boolean(solid, &cutter, crate::BoolOp::Intersection)?;
    Ok(Some((below, above)))
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

/// An ellipse in view coordinates: `center + a·cos t + b·sin t` with `a`
/// the major and `b` the minor half-axis, `b` a quarter turn
/// counter-clockwise from `a`, and `a` pointing into the right half-plane
/// (so two projections of one ellipse compare equal). Seen edge-on, `b`
/// is zero and the ellipse is the segment from `center - a` to
/// `center + a`, its pieces measured along `a`.
#[derive(Clone, Copy)]
struct Ellipse2 {
    center: Vec2,
    a: Vec2,
    b: Vec2,
}

impl Ellipse2 {
    /// The projection of the 3D ellipse `center + u1·cos t + u2·sin t`
    /// onto the view (`u1`, `u2` already projected); `None` when it is
    /// seen end-on (a point).
    fn new(center: Vec2, u1: Vec2, u2: Vec2, eps: f64) -> Option<Ellipse2> {
        // Conjugate diameters to principal axes: the parameter shift that
        // makes them perpendicular.
        let t0 = 0.5 * (2.0 * u1.dot(u2)).atan2(u1.length_squared() - u2.length_squared());
        let (mut a, mut b) = (u1 * t0.cos() + u2 * t0.sin(), u2 * t0.cos() - u1 * t0.sin());
        if a.length() < b.length() {
            (a, b) = (b, a);
        }
        if a.length() <= eps {
            return None;
        }
        if a.x < 0.0 || (a.x == 0.0 && a.y < 0.0) {
            a = -a;
        }
        if b.length() <= eps {
            b = Vec2::ZERO;
        } else if b.dot(a.perp()) < 0.0 {
            b = -b;
        }
        Some(Ellipse2 { center, a, b })
    }

    fn edge_on(&self) -> bool {
        self.b == Vec2::ZERO
    }

    /// The parameter of the point of the ellipse nearest `p` in the
    /// axes' frame, in `(-π, π]`; along `a` in `[-1, 1]` when edge-on.
    fn param(&self, p: Vec2) -> f64 {
        let d = p - self.center;
        if self.edge_on() {
            return (d.dot(self.a) / self.a.length_squared()).clamp(-1.0, 1.0);
        }
        (d.dot(self.b) / self.b.length_squared()).atan2(d.dot(self.a) / self.a.length_squared())
    }

    fn same(&self, o: &Ellipse2, eps: f64) -> bool {
        self.center.distance(o.center) <= eps
            && self.a.distance(o.a) <= eps
            && self.b.distance(o.b) <= eps
    }
}

/// The ellipses the circle and ellipse edges of `solid` project to, and
/// which edge lies on which (by index into the returned list).
fn projected_ellipses(
    solid: &Solid,
    to2: &dyn Fn(Vec3) -> Vec2,
    eps: f64,
) -> (Vec<Ellipse2>, HashMap<EdgeKey, usize>) {
    let mut ellipses: Vec<Ellipse2> = Vec::new();
    let mut of_edge: HashMap<EdgeKey, usize> = HashMap::new();
    let origin = to2(Vec3::ZERO);
    let vec2 = |v: Vec3| to2(v) - origin;
    let vf = exact::vertex_faces(solid);
    for run in exact::edge_runs(solid) {
        let curve = exact::run_curve(solid, &vf, &run);
        let (center, u1, u2) = match curve {
            Curve::Circle {
                center,
                axis,
                radius,
            } => {
                let (x, y) = exact::cylinder_frame(axis);
                (center, x * radius, y * radius)
            }
            Curve::Ellipse {
                center,
                axis,
                major,
                a,
                b,
            } => (center, major * a, axis.cross(major) * b),
            _ => continue,
        };
        let Some(e) = Ellipse2::new(to2(center), vec2(u1), vec2(u2), eps) else {
            continue;
        };
        let id = ellipses.len();
        ellipses.push(e);
        for w in run.vertices.windows(2) {
            of_edge.insert(edge_key(w[0], w[1]), id);
        }
    }
    (ellipses, of_edge)
}

/// Joins the parameter intervals `pieces` (each shorter than half a turn)
/// of one ellipse into arcs: overlapping or touching ones merge, and a set
/// covering the whole turn is one full arc from 0 to 2π.
fn merge_arcs(pieces: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut v: Vec<(f64, f64)> = pieces
        .iter()
        .map(|&(s, e)| {
            let s0 = s.rem_euclid(TAU);
            (s0, s0 + (e - s))
        })
        .collect();
    let mut merged = merge_intervals(v.split_off(0), 1e-9);
    // An interval running past 2π may continue into the first ones.
    if merged.len() > 1 {
        let last = merged[merged.len() - 1];
        if last.1 >= TAU + merged[0].0 - 1e-9 {
            let first = merged.remove(0);
            let n = merged.len();
            merged[n - 1].1 = last.1.max(first.1 + TAU);
        }
    }
    if let [only] = merged[..] {
        if only.1 - only.0 >= TAU - 1e-9 {
            return vec![(0.0, TAU)];
        }
    }
    merged
}

/// The parts of the arcs `from` (intervals of one ellipse) not covered by
/// the arcs `by` of the same ellipse.
fn subtract_arcs(from: &[(f64, f64)], by: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for &(s, e) in from {
        let mut covered: Vec<(f64, f64)> = Vec::new();
        for &(bs, be) in by {
            for k in [-TAU, 0.0, TAU] {
                let (c0, c1) = ((bs + k).max(s), (be + k).min(e));
                if c1 > c0 {
                    covered.push((c0, c1));
                }
            }
        }
        let covered = merge_intervals(covered, 1e-9);
        let mut cursor = s;
        for (c0, c1) in covered {
            if c0 > cursor + 1e-9 {
                out.push((cursor, c0));
            }
            cursor = cursor.max(c1);
        }
        if cursor < e - 1e-9 {
            out.push((cursor, e));
        }
    }
    out
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
    // Every ellipse any circle or ellipse edge projects to, with the
    // pieces of it found visible and hidden (parameter intervals).
    let mut ellipses: Vec<Ellipse2> = Vec::new();
    let mut arc_visible: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut arc_hidden: Vec<Vec<(f64, f64)>> = Vec::new();
    for (si, s) in solids.iter().enumerate() {
        let (own, of_edge) = projected_ellipses(s, &to2, eps);
        // Ellipses two runs project onto alike (both rims of a through
        // hole seen along it) are one.
        let ids: Vec<usize> = own
            .iter()
            .map(|e| match ellipses.iter().position(|k| k.same(e, eps)) {
                Some(id) => id,
                None => {
                    ellipses.push(*e);
                    arc_visible.push(Vec::new());
                    arc_hidden.push(Vec::new());
                    ellipses.len() - 1
                }
            })
            .collect();
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
            // A chord of an ellipse contributes the parameter interval
            // between its ends (the short way round) instead of a segment.
            let arc = of_edge.get(&edge_key(a, b)).map(|&k| ids[k]);
            let piece = |t0: f64, t1: f64| -> Option<(usize, (f64, f64))> {
                let id = arc?;
                let e = &ellipses[id];
                let (s0, s1) = (e.param(seg(t0, t1)[0]), e.param(seg(t0, t1)[1]));
                let (lo, hi) = (s0.min(s1), s0.max(s1));
                if e.edge_on() {
                    return Some((id, (lo, hi)));
                }
                Some((
                    id,
                    if hi - lo > PI {
                        (hi, lo + TAU)
                    } else {
                        (lo, hi)
                    },
                ))
            };
            let mut cursor = 0.0;
            for &(h0, h1) in &hidden {
                if h0 > cursor + 1e-9 {
                    match piece(cursor, h0) {
                        Some((id, p)) => arc_visible[id].push(p),
                        None => out.visible.push(seg(cursor, h0)),
                    }
                }
                match piece(h0, h1) {
                    Some((id, p)) => arc_hidden[id].push(p),
                    None => out.hidden.push(seg(h0, h1)),
                }
                cursor = h1;
            }
            if cursor < 1.0 - 1e-9 {
                match piece(cursor, 1.0) {
                    Some((id, p)) => arc_visible[id].push(p),
                    None => out.visible.push(seg(cursor, 1.0)),
                }
            }
        }
    }
    for (id, e) in ellipses.iter().enumerate() {
        if e.edge_on() {
            // The chords of a rim seen edge-on join into one line, which
            // then takes part in the collinear clean-up like any segment.
            let along = |x: f64| e.center + e.a * x;
            for (x0, x1) in merge_intervals(arc_visible[id].clone(), 1e-9) {
                out.visible.push([along(x0), along(x1)]);
            }
            for (x0, x1) in merge_intervals(arc_hidden[id].clone(), 1e-9) {
                out.hidden.push([along(x0), along(x1)]);
            }
            continue;
        }
        let visible = merge_arcs(&arc_visible[id]);
        let hidden = subtract_arcs(&merge_arcs(&arc_hidden[id]), &visible);
        let arc = |(start, end): (f64, f64)| ViewArc {
            center: e.center,
            major: e.a,
            ratio: e.b.length() / e.a.length(),
            start,
            end,
        };
        out.visible_arcs.extend(visible.into_iter().map(arc));
        out.hidden_arcs.extend(hidden.into_iter().map(arc));
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

    #[test]
    fn view_dxf_writes_lines_circles_arcs_and_ellipses() {
        let lines = ViewLines {
            visible: vec![[Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.5)]],
            hidden: vec![[Vec2::new(0.0, 0.0), Vec2::new(0.0, 5.0)]],
            visible_arcs: vec![
                ViewArc {
                    center: Vec2::new(5.0, 5.0),
                    major: Vec2::new(2.0, 0.0),
                    ratio: 1.0,
                    start: 0.0,
                    end: TAU,
                },
                ViewArc {
                    center: Vec2::new(5.0, 5.0),
                    major: Vec2::new(0.0, 3.0),
                    ratio: 1.0,
                    start: 0.0,
                    end: PI / 2.0,
                },
                ViewArc {
                    center: Vec2::ZERO,
                    major: Vec2::new(4.0, 0.0),
                    ratio: 0.5,
                    start: 0.0,
                    end: TAU,
                },
            ],
            hidden_arcs: vec![],
        };
        let dxf = view_dxf(&lines, false);
        let entities: Vec<&str> = dxf
            .split('\n')
            .collect::<Vec<_>>()
            .windows(2)
            .filter(|w| w[0] == "0")
            .map(|w| w[1])
            .filter(|e| matches!(*e, "LINE" | "CIRCLE" | "ARC" | "ELLIPSE"))
            .collect();
        assert_eq!(entities, ["LINE", "CIRCLE", "ARC", "ELLIPSE"]);
        assert!(dxf.contains("\n10\n0\n20\n0\n30\n0\n11\n10\n21\n0.5\n31\n0\n"));
        // The quarter arc starts along +y: 90° to 180°.
        assert!(dxf.contains("\n50\n90\n51\n180\n"), "{dxf}");
        assert!(dxf.contains("$ACADVER\n1\nAC1015"));
        assert!(
            !dxf.contains("HIDDEN\n10"),
            "no hidden entities unless asked"
        );
        let with_hidden = view_dxf(&lines, true);
        assert!(with_hidden.contains("LINE\n8\nHIDDEN\n"));
        assert_eq!(dxf_num(-0.0000001), "0");
        assert_eq!(dxf_num(12.5), "12.5");
        assert_eq!(dxf_num(-3.0), "-3");
    }

    fn block(w: f64, d: f64, h: f64) -> Solid {
        let mut s = Sketch::new();
        s.add_rectangle(Vec2::ZERO, Vec2::new(w, d));
        let p = s.profiles(&ProfileOptions::default()).remove(0);
        extrude(&p, &Plane::XY, 0.0, h, 1).unwrap()
    }

    fn cylinder(r: f64, h: f64) -> Solid {
        let mut s = Sketch::new();
        s.add_circle(Vec2::ZERO, r);
        let p = s.profiles(&ProfileOptions::default()).remove(0);
        extrude(&p, &Plane::XY, 0.0, h, 1).unwrap()
    }

    fn view(dir: Vec3, up: Vec3) -> View {
        View { dir, up }
    }

    #[test]
    fn a_cylinder_seen_along_its_axis_is_one_circle() {
        let c = cylinder(10.0, 5.0);
        let lines = project_view(&[&c], view(-Vec3::Z, Vec3::Y));
        assert!(lines.visible.is_empty(), "{:?}", lines.visible);
        assert!(lines.hidden.is_empty());
        // Both rims project onto the same circle: drawn once, whole.
        assert_eq!(lines.visible_arcs.len(), 1, "{:?}", lines.visible_arcs);
        assert!(lines.hidden_arcs.is_empty());
        let a = &lines.visible_arcs[0];
        assert!(a.center.distance(Vec2::ZERO) < 1e-9);
        assert!((a.major.length() - 10.0).abs() < 1e-9);
        assert!((a.ratio - 1.0).abs() < 1e-9);
        assert!((a.end - a.start - TAU).abs() < 1e-9);
    }

    #[test]
    fn a_cylinder_seen_from_the_side_has_no_arcs() {
        let c = cylinder(10.0, 5.0);
        let lines = project_view(&[&c], view(Vec3::Y, Vec3::Z));
        assert!(lines.visible_arcs.is_empty() && lines.hidden_arcs.is_empty());
        // Two silhouette rulings and the two rims seen edge-on, each one
        // line rather than a row of chords.
        assert_eq!(lines.visible.len(), 4, "{:?}", lines.visible);
        let rims: Vec<_> = lines
            .visible
            .iter()
            .filter(|s| (s[0].y - s[1].y).abs() < 1e-9)
            .collect();
        assert_eq!(rims.len(), 2);
        for r in rims {
            assert!((r[0].distance(r[1]) - 20.0).abs() < 1e-9, "{r:?}");
        }
    }

    #[test]
    fn a_cylinder_in_isometric_view_shows_its_rims_as_ellipses() {
        let c = cylinder(10.0, 20.0);
        let dir = Vec3::new(-1.0, -1.0, -1.0);
        let lines = project_view(&[&c], view(dir, Vec3::Z));
        // The top rim is a whole visible ellipse; the bottom rim is partly
        // hidden behind the wall, so it is a visible arc and a hidden arc
        // of the same ellipse that do not overlap.
        let whole: Vec<_> = lines
            .visible_arcs
            .iter()
            .filter(|a| (a.end - a.start - TAU).abs() < 1e-9)
            .collect();
        assert_eq!(whole.len(), 1, "{:?}", lines.visible_arcs);
        assert_eq!(lines.hidden_arcs.len(), 1, "{:?}", lines.hidden_arcs);
        let h = &lines.hidden_arcs[0];
        let ratio = (1.0f64 / 3.0).sqrt();
        assert!((h.ratio - ratio).abs() < 1e-6, "ratio {}", h.ratio);
        let partial: Vec<_> = lines
            .visible_arcs
            .iter()
            .filter(|a| a.center.distance(h.center) < 1e-9)
            .collect();
        assert_eq!(partial.len(), 1);
        let v = partial[0];
        let total = (v.end - v.start) + (h.end - h.start);
        assert!((total - TAU).abs() < 1e-6, "visible {v:?} hidden {h:?}");
        // The wall hides the far half of the bottom rim, to within the
        // facet the silhouette falls in.
        assert!((h.end - h.start - PI).abs() < 0.1, "{h:?}");
        // No chord of a rim is left as a segment: the only segments are
        // the two silhouette rulings.
        assert_eq!(lines.visible.len(), 2, "{:?}", lines.visible);
    }

    fn polygon_area(poly: &[Vec2]) -> f64 {
        let n = poly.len();
        (0..n)
            .map(|i| poly[i].cross(poly[(i + 1) % n]))
            .sum::<f64>()
            .abs()
            / 2.0
    }

    #[test]
    fn split_divides_a_block_into_two_closed_halves() {
        let b = block(20.0, 10.0, 5.0);
        let plane = Plane::from_origin_normal(Vec3::new(6.0, 0.0, 0.0), Vec3::X).unwrap();
        let (below, above) = split_tagged(&b, &plane, 9).unwrap().unwrap();
        below.validate().unwrap();
        above.validate().unwrap();
        assert!((below.volume() - 300.0).abs() < 1e-9, "{}", below.volume());
        assert!((above.volume() - 700.0).abs() < 1e-9, "{}", above.volume());
        // The cut faces carry the split feature's tag; the rest keep theirs.
        let cut = |s: &Solid| s.faces.iter().filter(|f| f.origin.feature == 9).count();
        assert_eq!(cut(&below), 1);
        assert_eq!(cut(&above), 1);
        assert_eq!(below.faces.len(), 6);
        // A plane past the block splits nothing.
        let miss = Plane::from_origin_normal(Vec3::new(30.0, 0.0, 0.0), Vec3::X).unwrap();
        assert!(split(&b, &miss).unwrap().is_none());
    }

    #[test]
    fn section_through_a_block_with_a_hole_shows_the_hole_in_the_cut() {
        // 20 x 10 x 5 block with a vertical 4 mm hole through its middle.
        let block = block(20.0, 10.0, 5.0);
        let mut s = Sketch::new();
        s.add_circle(Vec2::new(10.0, 5.0), 2.0);
        let p = s.profiles(&ProfileOptions::default()).remove(0);
        let drill = extrude(&p, &Plane::XY, -1.0, 6.0, 2).unwrap();
        let solid = boolean(&block, &drill, BoolOp::Difference).unwrap();
        // Cut at y = 5 through the hole's axis, removing the front (y < 5)
        // half, seen from the front (looking along +y).
        let plane = Plane::from_origin_normal(Vec3::new(0.0, 5.0, 0.0), -Vec3::Y).unwrap();
        let view = View {
            dir: Vec3::Y,
            up: Vec3::Z,
        };
        let sec = section_view(&[&solid], view, &plane);
        // The cut face is the 20 x 5 rectangle minus a 4 mm wide slot: two
        // 8 x 5 pieces (or one polygon with the slot taken out).
        let area: f64 = sec.cut.iter().map(|l| polygon_area(l)).sum();
        assert!((area - (20.0 * 5.0 - 4.0 * 5.0)).abs() < 1e-6, "{area}");
        assert!(!sec.visible.is_empty());
        // Nothing is hidden: everything behind the cut is solid.
        assert!(sec.hidden.is_empty(), "{:?}", sec.hidden);
        // The cut outline spans the full width and height in view space.
        let (mut lo, mut hi) = (Vec2::new(f64::MAX, f64::MAX), Vec2::new(f64::MIN, f64::MIN));
        for p in sec.cut.iter().flatten() {
            lo = Vec2::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = Vec2::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        assert!((hi.x - lo.x - 20.0).abs() < 1e-9 && (hi.y - lo.y - 5.0).abs() < 1e-9);
        // A plane that misses the block leaves it whole: no cut faces.
        let miss = Plane::from_origin_normal(Vec3::new(0.0, 50.0, 0.0), -Vec3::Y).unwrap();
        assert!(section_view(&[&solid], view, &miss).cut.is_empty());
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
        // From above the rim is one visible circle (both rims project
        // onto it) beside the outline, and nothing is hidden.
        let top = project_view(&[&plate], TOP);
        assert!(top.hidden.is_empty(), "{:?}", top.hidden);
        assert!(top.hidden_arcs.is_empty(), "{:?}", top.hidden_arcs);
        assert_eq!(top.visible.len(), 4, "{:?}", top.visible);
        assert_eq!(top.visible_arcs.len(), 1, "{:?}", top.visible_arcs);
        let a = &top.visible_arcs[0];
        assert!((a.major.length() - 5.0).abs() < 1e-9 && (a.ratio - 1.0).abs() < 1e-9);
        assert!((a.end - a.start - TAU).abs() < 1e-9);
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
