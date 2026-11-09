//! The curvy Spectre as convex prisms: the straight tile cut into
//! convex pieces by straight cuts between points of its 30° grid, each
//! piece a stick a table saw rips and a router table curves, the sticks
//! glued into one tile stick and sliced into pieces. Every curve then
//! sits on a convex stick: a flute (an edge bulging in) is one pass of
//! a core box bit of the arc's radius, a bump (bulging out) one pass of
//! a bullnose of that radius, or at the quarter circle a 45° gable
//! rounded over. Either cutter's shoulder leaves the arc's end tangent
//! to it, at the arc's end angle `theta` to the chord, so the prism's
//! corner where a bump ends must be no sharper than `180 - theta` or the
//! shoulder takes the next face: the shoulder rule. A flute's cutter
//! stays inside the chord and asks nothing. For the S curve (`ogee`)
//! only the end where the convex half starts is such a corner.
//!
//! `decompose` reads a cut set: the prisms, their faces (bump, flute,
//! S, glue), convexity, the rule, and the congruence classes (a mirror
//! image is the same stick turned end for end). `search` finds the
//! fewest prisms for a curve: every corner the rule or convexity makes
//! cut needs a cut leaving it within the rule, a vertex diagonal serving
//! both its ends or a grid ray that stops at the first thing it meets;
//! among the arrangements with the fewest cuts, the valid one whose
//! smallest prism is largest.

use super::{curved_tile, pieces_per_edge, rips, vertex, Curve, ANGLES};
use ok_math::Vec2;

/// A cut: two points in tile units, inside the tile.
pub type Cut = [Vec2; 2];

const EPS: f64 = 1e-7;

fn cross(a: Vec2, b: Vec2) -> f64 {
    a.x * b.y - a.y * b.x
}

fn same(a: Vec2, b: Vec2) -> bool {
    a.distance(b) < 1e-6
}

fn area(p: &[Vec2]) -> f64 {
    let n = p.len();
    (0..n).map(|i| cross(p[i], p[(i + 1) % n])).sum::<f64>() / 2.0
}

fn deg(v: Vec2) -> f64 {
    v.y.atan2(v.x).to_degrees().rem_euclid(360.0)
}

/// Interior angle at vertex `i` of a counter-clockwise polygon, degrees.
fn interior(p: &[Vec2], i: usize) -> f64 {
    let n = p.len();
    let (a, b, c) = (p[(i + n - 1) % n], p[i], p[(i + 1) % n]);
    let (u, v) = (a - b, c - b);
    (v.x * u.y - v.y * u.x)
        .atan2(v.x * u.x + v.y * u.y)
        .to_degrees()
        .rem_euclid(360.0)
}

fn point_in(p: Vec2, poly: &[Vec2]) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn on_segment(p: Vec2, a: Vec2, b: Vec2) -> bool {
    let d = b - a;
    if cross(d, p - a).abs() / d.length() > 1e-6 {
        return false;
    }
    let t = (p - a).dot(d) / d.dot(d);
    t > -1e-9 && t < 1.0 + 1e-9
}

fn on_edge(p: Vec2, poly: &[Vec2]) -> bool {
    let n = poly.len();
    (0..n).any(|i| on_segment(p, poly[i], poly[(i + 1) % n]))
}

fn proper_cross(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> bool {
    let (d1, d2) = (cross(b - a, c - a), cross(b - a, d - a));
    let (d3, d4) = (cross(d - c, a - c), cross(d - c, b - c));
    d1 * d2 < -1e-12 && d3 * d4 < -1e-12
}

/// Where the lines through `a..b` and `c..d` meet, as parameters along each.
fn line_meet(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> Option<(f64, f64)> {
    let (r, s) = (b - a, d - c);
    let den = cross(r, s);
    if den.abs() < 1e-12 {
        return None;
    }
    Some((cross(c - a, s) / den, cross(c - a, r) / den))
}

/// The straight tile, counter-clockwise, unit edges.
pub fn tile() -> Vec<Vec2> {
    rips::tile()
}

/// The points a cut may start or end at: the tile's vertices and the
/// crossings of the lines at every 30° through them, inside the tile.
pub fn grid() -> Vec<Vec2> {
    rips::nodes()
}

/// Whether `a`–`b` is a cut: distinct points, every point between them
/// strictly inside the tile (so a cut neither runs along the boundary
/// nor passes through a vertex, as a line from vertex 2 through vertex
/// 1 would). Any direction will do; a glue face can lie at any angle.
pub fn cut_ok(a: Vec2, b: Vec2) -> bool {
    if same(a, b) {
        return false;
    }
    let t = tile();
    for k in 1..48 {
        let p = a + (b - a) * (k as f64 / 48.0);
        if !point_in(p, &t) || on_edge(p, &t) {
            return false;
        }
    }
    (0..14).all(|i| !proper_cross(a, b, t[i], t[(i + 1) % 14]))
}

/// What a prism's face is, once the curves are on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FaceKind {
    /// A tile edge bulging out: a bullnose pass, or a gable rounded over.
    Bump,
    /// A tile edge bulging in: a core box pass.
    Flute,
    /// A whole S: one ogee pass.
    Ogee,
    /// A cut: a glue face.
    Glue,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Face {
    /// Length in edge units.
    pub len: f64,
    pub kind: FaceKind,
    /// A curved face a bit makes in one pass: the whole tile edge, or
    /// exactly one half of an S. A part of a curve is shared with the
    /// prism next door and has to register at the glue line.
    pub whole: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Prism {
    /// The straight outline, counter-clockwise, tile units.
    pub outline: Vec<Vec2>,
    /// The outline with the curves on its tile edges.
    pub curved: Vec<Vec2>,
    pub faces: Vec<Face>,
    /// Interior angles at the straight outline's corners, degrees.
    pub corners: Vec<f64>,
    pub convex: bool,
    /// Every corner where a convex arc ends is within the shoulder rule.
    pub shoulders: bool,
    /// A bump or an S cut part way along by a cut: no bit makes that.
    pub bump_split: bool,
    /// A flute shared across a cut: two core box passes to register.
    pub flute_split: bool,
    /// Area of the straight outline, square edges.
    pub area: f64,
    /// Congruence class, mirror images alike, in the order the classes
    /// first appear.
    pub shape: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Decomposition {
    pub cuts: Vec<Cut>,
    pub prisms: Vec<Prism>,
    /// The prisms of each congruence class.
    pub shapes: Vec<Vec<usize>>,
    /// The arc's end angle, degrees: what a cutter's shoulder leaves at.
    pub theta: f64,
    /// The widest corner allowed where a convex arc ends: 180 - theta.
    pub limit: f64,
    /// Radius of the curve's arcs, edge units.
    pub radius: f64,
    /// Every prism convex, within the rule and with no split bump.
    pub clean: bool,
}

/// The angle the curve's arcs make with their chord at an end, degrees.
pub fn theta(curve: Curve, bulge: f64) -> f64 {
    match curve {
        Curve::Arc => 2.0 * (2.0 * bulge).atan().to_degrees(),
        Curve::Ogee => 2.0 * (4.0 * bulge).atan().to_degrees(),
        // A cubic has no one radius; its end tangent stands at this.
        Curve::Cubic => bulge.atan2(0.33).to_degrees(),
    }
}

/// Radius of the curve's arcs, edge units (the cubic's tightest).
pub fn radius(curve: Curve, bulge: f64) -> f64 {
    match curve {
        Curve::Arc => (0.25 + bulge * bulge) / (2.0 * bulge),
        Curve::Ogee => (1.0 / 16.0 + bulge * bulge) / (2.0 * bulge),
        Curve::Cubic => super::tightest_radius(curve, bulge),
    }
}

/// Whether tile edge `i` (from vertex `i` to `i + 1`) bulges out.
fn bump_edge(i: usize) -> bool {
    i.is_multiple_of(2)
}

/// The tile edge a prism edge lies on: (edge, whole, half), `half` 1
/// for the first half from vertex `i`, 2 for the second, 0 otherwise.
fn tile_edge_of(a: Vec2, b: Vec2) -> Option<(usize, bool, u8)> {
    for i in 0..14 {
        let (p, q) = (vertex(i), vertex(i + 1));
        if on_segment(a, p, q) && on_segment(b, p, q) {
            let m = (p + q) * 0.5;
            let at = |x: Vec2| {
                if same(x, p) {
                    0
                } else if same(x, m) {
                    1
                } else if same(x, q) {
                    2
                } else {
                    -1
                }
            };
            let (ea, eb) = (at(a), at(b));
            let whole = (ea == 0 && eb == 2) || (ea == 2 && eb == 0);
            let half = if (ea == 0 && eb == 1) || (ea == 1 && eb == 0) {
                1
            } else if (ea == 1 && eb == 2) || (ea == 2 && eb == 1) {
                2
            } else {
                0
            };
            return Some((i, whole, half));
        }
    }
    None
}

fn face_kind(te: Option<(usize, bool, u8)>, curve: Curve) -> (FaceKind, bool) {
    let Some((i, whole, half)) = te else {
        return (FaceKind::Glue, true);
    };
    if curve != Curve::Ogee {
        let kind = if bump_edge(i) {
            FaceKind::Bump
        } else {
            FaceKind::Flute
        };
        return (kind, whole);
    }
    if whole {
        (FaceKind::Ogee, true)
    } else if half == 1 {
        (FaceKind::Bump, true)
    } else if half == 2 {
        (FaceKind::Flute, true)
    } else {
        (FaceKind::Ogee, false)
    }
}

/// Drops repeated and flat vertices.
fn clean(p: &[Vec2]) -> Vec<Vec2> {
    let mut q: Vec<Vec2> = Vec::new();
    for &v in p {
        if q.last().is_none_or(|l| !same(*l, v)) {
            q.push(v);
        }
    }
    while q.len() > 1 && same(q[0], *q.last().unwrap()) {
        q.pop();
    }
    let n = q.len();
    (0..n)
        .filter(|&i| {
            let (a, b, c) = (q[(i + n - 1) % n], q[i], q[(i + 1) % n]);
            cross(b - a, c - b).abs() > 1e-9
        })
        .map(|i| q[i])
        .collect()
}

/// The tile's own vertices put back where cleaning dropped them: the
/// 180° vertex is flat, and without it the two edges meeting there read
/// as one face that is no tile edge.
fn with_tile_vertices(outline: &[Vec2]) -> Vec<Vec2> {
    let mut out = outline.to_vec();
    for i in 0..14 {
        let v = vertex(i);
        if out.iter().any(|q| same(*q, v)) {
            continue;
        }
        let n = out.len();
        for k in 0..n {
            if on_segment(v, out[k], out[(k + 1) % n]) {
                out.insert(k + 1, v);
                break;
            }
        }
    }
    out
}

/// The curve of tile edge `i` from vertex `i` to `i + 1`, in tile units.
fn edge_curve(curve: Curve, bulge: f64, facet: f64) -> Vec<Vec<Vec2>> {
    let n = pieces_per_edge(curve, bulge, facet);
    let all = curved_tile(curve, bulge, n);
    (0..14)
        .map(|i| {
            (0..=n)
                .map(|k| all[(i * n + k) % all.len()])
                .collect::<Vec<_>>()
        })
        .collect()
}

/// A prism's outline with the curves put on the tile edges it has: a
/// whole edge takes the whole curve, a part of one the part over it.
fn curved_outline(outline: &[Vec2], curves: &[Vec<Vec2>]) -> Vec<Vec2> {
    let n = outline.len();
    let mut out = Vec::new();
    for k in 0..n {
        let (a, b) = (outline[k], outline[(k + 1) % n]);
        out.push(a);
        let Some((i, whole, _)) = tile_edge_of(a, b) else {
            continue;
        };
        let (p, q) = (vertex(i), vertex(i + 1));
        let c = &curves[i];
        if whole {
            if same(a, p) {
                out.extend(c[1..c.len() - 1].iter().copied());
            } else {
                out.extend(c[1..c.len() - 1].iter().rev().copied());
            }
            continue;
        }
        let along = |x: Vec2| (x - p).dot(q - p) / (q - p).dot(q - p);
        let (ta, tb) = (along(a), along(b));
        let (lo, hi) = (ta.min(tb), ta.max(tb));
        let mut part: Vec<Vec2> = c
            .iter()
            .copied()
            .filter(|x| {
                let t = along(*x);
                t > lo + 1e-6 && t < hi - 1e-6
            })
            .collect();
        if ta > tb {
            part.reverse();
        }
        out.extend(part);
    }
    out
}

/// A congruence key: the cyclic sequence of (length, kind, turn) read
/// both ways round, least first, so a mirror image is the same shape.
fn shape_key(faces: &[Face], corners: &[f64]) -> String {
    let n = faces.len();
    let tag = |f: &Face| {
        format!(
            "{:.3}{}{}",
            f.len,
            match f.kind {
                FaceKind::Bump => 'b',
                FaceKind::Flute => 'f',
                FaceKind::Ogee => 'o',
                FaceKind::Glue => 'g',
            },
            if f.whole { "" } else { "~" }
        )
    };
    let fwd: Vec<String> = (0..n)
        .map(|k| format!("{}/{:.1}", tag(&faces[k]), corners[(k + 1) % n]))
        .collect();
    let mut rev: Vec<String> = (0..n)
        .map(|k| format!("{}/{:.1}", tag(&faces[k]), corners[k]))
        .collect();
    rev.reverse();
    let mut best: Option<String> = None;
    for seq in [&fwd, &rev] {
        for k in 0..n {
            let s: String = seq[k..]
                .iter()
                .chain(seq[..k].iter())
                .cloned()
                .collect::<Vec<_>>()
                .join("|");
            if best.as_ref().is_none_or(|b| s < *b) {
                best = Some(s);
            }
        }
    }
    best.unwrap_or_default()
}

/// Reads a prism: its faces, the rule, and whether any curve is split.
fn read(
    outline: &[Vec2],
    curve: Curve,
    curves: &[Vec<Vec2>],
    limit: f64,
) -> (Vec<Face>, Vec<f64>, bool, bool, bool, bool, Vec<Vec2>) {
    let n = outline.len();
    let corners: Vec<f64> = (0..n).map(|k| interior(outline, k)).collect();
    let convex = corners.iter().all(|&a| a <= 180.0 + 1e-6);
    let mut faces = Vec::with_capacity(n);
    let (mut shoulders, mut bump_split, mut flute_split) = (true, false, false);
    for k in 0..n {
        let (a, b) = (outline[k], outline[(k + 1) % n]);
        let te = tile_edge_of(a, b);
        let (kind, whole) = face_kind(te, curve);
        faces.push(Face {
            len: a.distance(b),
            kind,
            whole,
        });
        if !whole {
            if kind == FaceKind::Flute {
                flute_split = true;
            } else {
                bump_split = true;
            }
            continue;
        }
        // The corners where a convex arc ends.
        let ends: Vec<usize> = match (curve, kind) {
            (Curve::Ogee, FaceKind::Ogee) => {
                let (i, _, _) = te.unwrap();
                vec![if same(a, vertex(i)) { k } else { (k + 1) % n }]
            }
            (Curve::Ogee, FaceKind::Bump) => {
                let (i, _, _) = te.unwrap();
                vec![if same(a, vertex(i)) { k } else { (k + 1) % n }]
            }
            (_, FaceKind::Bump) => vec![k, (k + 1) % n],
            _ => Vec::new(),
        };
        if ends.iter().any(|&e| corners[e] > limit + 1e-6) {
            shoulders = false;
        }
    }
    let curved = curved_outline(outline, curves);
    (
        faces,
        corners,
        convex,
        shoulders,
        bump_split,
        flute_split,
        curved,
    )
}

/// A glue face shorter than this, in edge units, is a sliver the search
/// avoids: a ray that stopped a hair short of a vertex.
pub const SLIVER: f64 = 0.3;
/// A prism corner under this many degrees is a sharp point the search
/// avoids: a fragile edge on a stick, and a nest that pinches it.
pub const SHARP: f64 = 45.0;

/// The prisms a cut set makes of the tile with this curve.
pub fn decompose(
    curve: Curve,
    bulge: f64,
    cuts: &[Cut],
    facet: f64,
) -> Result<Decomposition, String> {
    if let Some(c) = cuts.iter().find(|c| !cut_ok(c[0], c[1])) {
        return Err(format!(
            "cut from ({:.3}, {:.3}) to ({:.3}, {:.3}) does not lie in the tile",
            c[0].x, c[0].y, c[1].x, c[1].y
        ));
    }
    let th = theta(curve, bulge);
    let limit = 180.0 - th;
    let curves = edge_curve(curve, bulge, facet);
    let mut prisms = Vec::new();
    let mut keys: Vec<String> = Vec::new();
    let mut shapes: Vec<Vec<usize>> = Vec::new();
    for raw in rips::pieces(cuts) {
        let mut outline = with_tile_vertices(&clean(&raw));
        if area(&outline) < 0.0 {
            outline.reverse();
        }
        let (faces, corners, convex, shoulders, bump_split, flute_split, curved) =
            read(&outline, curve, &curves, limit);
        let key = shape_key(&faces, &corners);
        let shape = match keys.iter().position(|k| *k == key) {
            Some(j) => j,
            None => {
                keys.push(key);
                shapes.push(Vec::new());
                keys.len() - 1
            }
        };
        shapes[shape].push(prisms.len());
        prisms.push(Prism {
            area: area(&outline),
            outline,
            curved,
            faces,
            corners,
            convex,
            shoulders,
            bump_split,
            flute_split,
            shape,
        });
    }
    let clean_all = prisms
        .iter()
        .all(|p| p.convex && p.shoulders && !p.bump_split);
    Ok(Decomposition {
        cuts: cuts.to_vec(),
        prisms,
        shapes,
        theta: th,
        limit,
        radius: radius(curve, bulge),
        clean: clean_all,
    })
}

/// Splits a piece (a tile's outline in any frame) by the prism cuts in
/// that frame, and says which prism each part is, by a point just inside
/// the part against the prisms' curved outlines in the same frame.
pub fn split_piece(
    outline: &[Vec2],
    segs: &[Cut],
    placed: &[Vec<Vec2>],
) -> Vec<(usize, Vec<Vec2>)> {
    // A glue line that ends on the tile's boundary meets the eroded
    // outline a hair off its corner, and the half-edge walk closes a
    // sliver there a thousandth of a square millimetre across: that is
    // the gap's, not a piece.
    let whole = area(outline).abs();
    rips::faces_of(outline, segs)
        .into_iter()
        .filter(|part| area(part).abs() > 1e-4 * whole)
        .map(|part| {
            let probe = inner_point(&part);
            let j = placed.iter().position(|c| point_in(probe, c)).unwrap_or(0);
            (j, part)
        })
        .collect()
}

/// A point inside a simple polygon: the midpoint of its longest edge,
/// nudged a little to the left of it (the inside of a counter-clockwise
/// outline); a centroid can fall outside a concave one.
fn inner_point(poly: &[Vec2]) -> Vec2 {
    let n = poly.len();
    let longest = (0..n)
        .max_by(|&i, &j| {
            poly[i]
                .distance(poly[(i + 1) % n])
                .total_cmp(&poly[j].distance(poly[(j + 1) % n]))
        })
        .unwrap_or(0);
    let (a, b) = (poly[longest], poly[(longest + 1) % n]);
    let d = (b - a).normalized().unwrap_or(Vec2::X);
    (a + b) * 0.5 + d.perp() * (1e-3 * a.distance(b).max(1e-3))
}

/// The curved outlines of the prisms, in tile units, for splitting a
/// top's pieces by them.
pub fn curved_faces(curve: Curve, bulge: f64, cuts: &[Cut], facet: f64) -> Vec<Vec<Vec2>> {
    let curves = edge_curve(curve, bulge, facet);
    rips::pieces(cuts)
        .iter()
        .map(|raw| {
            let mut outline = with_tile_vertices(&clean(raw));
            if area(&outline) < 0.0 {
                outline.reverse();
            }
            curved_outline(&outline, &curves)
        })
        .collect()
}

/// The vertex a cut must leave, and how: the chord of the convex arc
/// that ends there, and which way round the inside lies.
struct Need {
    /// Interior angle at the vertex.
    angle: f64,
    /// Direction of the chord away from the vertex, degrees.
    chord: f64,
    /// +1 when the inside is anticlockwise from the chord, -1 clockwise.
    side: f64,
}

fn needs(curve: Curve) -> Vec<Need> {
    let t = tile();
    (0..14)
        .map(|v| {
            // The convex arc ending at v: for arcs the bump edge at v,
            // for the S the edge whose convex half starts at v.
            let leaving = match curve {
                Curve::Ogee => true,
                _ => bump_edge(v),
            };
            let far = if leaving {
                t[(v + 1) % 14]
            } else {
                t[(v + 13) % 14]
            };
            Need {
                angle: ANGLES[v] as f64,
                chord: deg(far - t[v]),
                side: if leaving { 1.0 } else { -1.0 },
            }
        })
        .collect()
}

/// The angle from the vertex's constrained chord round the inside to a
/// direction, degrees.
fn phi(need: &Need, dir: f64) -> f64 {
    ((dir - need.chord) * need.side).rem_euclid(360.0)
}

/// The fewest convex prisms for this curve, if any arrangement of
/// vertex diagonals and grid rays satisfies the rule.
pub fn search(curve: Curve, bulge: f64, facet: f64) -> Result<Decomposition, String> {
    let t = tile();
    let th = theta(curve, bulge);
    let limit = 180.0 - th;
    let need = needs(curve);
    let needed: Vec<usize> = (0..14).filter(|&v| need[v].angle > limit + 1e-6).collect();
    // A diagonal serves its end when it leaves within the rule and leaves the rest convex.
    let serves = |c: &[usize; 2], v: usize| -> bool {
        if c[0] != v && c[1] != v {
            return false;
        }
        let o = if c[0] == v { c[1] } else { c[0] };
        let p = phi(&need[v], deg(t[o] - t[v]));
        p <= limit + 1e-6 && need[v].angle - p <= 180.0 + 1e-6
    };
    let mut diags: Vec<[usize; 2]> = Vec::new();
    for i in 0..14 {
        for j in i + 1..14 {
            if (j - i) % 14 == 1 || (i + 14 - j) % 14 == 1 {
                continue;
            }
            if cut_ok(t[i], t[j]) {
                diags.push([i, j]);
            }
        }
    }
    // Grid rays from a needed vertex, at the grid angles within its rule, to the boundary.
    struct Ray {
        v: usize,
        end: Vec2,
    }
    let mut rays: Vec<Ray> = Vec::new();
    for &v in &needed {
        for p in [30.0, 60.0, 90.0, 120.0, 150.0] {
            if p > limit + 1e-6 || p > need[v].angle - 1e-6 || need[v].angle - p > 180.0 + 1e-6 {
                continue;
            }
            let a = (need[v].chord + need[v].side * p).to_radians();
            let d = Vec2::new(a.cos(), a.sin());
            let mut best: Option<(f64, Vec2)> = None;
            for k in 0..14 {
                if k == v || (k + 1) % 14 == v {
                    continue;
                }
                let (c, e) = (t[k], t[(k + 1) % 14]);
                if let Some((tt, u)) = line_meet(t[v], t[v] + d, c, e) {
                    if tt > EPS && u > -1e-9 && u < 1.0 + 1e-9 && best.is_none_or(|(bt, _)| tt < bt)
                    {
                        best = Some((tt, t[v] + d * tt));
                    }
                }
            }
            if let Some((_, end)) = best {
                rays.push(Ray { v, end });
            }
        }
    }
    let options: Vec<Vec<usize>> = (0..14)
        .map(|v| (0..diags.len()).filter(|&k| serves(&diags[k], v)).collect())
        .collect();
    let dcross = |a: &[usize; 2], b: &[usize; 2]| -> bool {
        !(a[0] == b[0] || a[0] == b[1] || a[1] == b[0] || a[1] == b[1])
            && proper_cross(t[a[0]], t[a[1]], t[b[0]], t[b[1]])
    };
    // Depth-first over the needed vertices: the fewest cuts, every
    // arrangement with that many kept (to a few hundred).
    struct Walk<'a> {
        needed: &'a [usize],
        diags: &'a [[usize; 2]],
        rays: &'a [Ray],
        options: &'a [Vec<usize>],
        serves: &'a dyn Fn(&[usize; 2], usize) -> bool,
        dcross: &'a dyn Fn(&[usize; 2], &[usize; 2]) -> bool,
        best_n: usize,
        sols: Vec<(Vec<usize>, Vec<usize>)>,
    }
    impl Walk<'_> {
        fn go(&mut self, ds: &mut Vec<usize>, rs: &mut Vec<usize>) {
            let n = ds.len() + rs.len();
            if n > self.best_n {
                return;
            }
            let rest: Vec<usize> = self
                .needed
                .iter()
                .copied()
                .filter(|&v| {
                    !ds.iter().any(|&d| (self.serves)(&self.diags[d], v))
                        && !rs.iter().any(|&r| self.rays[r].v == v)
                })
                .collect();
            if rest.is_empty() {
                if n < self.best_n {
                    self.best_n = n;
                    self.sols.clear();
                }
                if self.sols.len() < 400 {
                    self.sols.push((ds.clone(), rs.clone()));
                }
                return;
            }
            let v = *rest
                .iter()
                .min_by_key(|&&v| {
                    self.options[v].len() + self.rays.iter().filter(|r| r.v == v).count()
                })
                .unwrap();
            for k in self.options[v].clone() {
                if ds.contains(&k)
                    || ds
                        .iter()
                        .any(|&d| (self.dcross)(&self.diags[k], &self.diags[d]))
                {
                    continue;
                }
                ds.push(k);
                self.go(ds, rs);
                ds.pop();
            }
            for r in 0..self.rays.len() {
                if self.rays[r].v == v {
                    rs.push(r);
                    self.go(ds, rs);
                    rs.pop();
                }
            }
        }
    }
    let mut walk = Walk {
        needed: &needed,
        diags: &diags,
        rays: &rays,
        options: &options,
        serves: &serves,
        dcross: &dcross,
        best_n: usize::MAX,
        sols: Vec::new(),
    };
    walk.go(&mut Vec::new(), &mut Vec::new());
    let sols = walk.sols;
    if sols.is_empty() {
        return Err("no arrangement of vertex cuts and grid rays satisfies this curve".into());
    }
    // Build each: diagonals whole, rays stopped at the first cut they
    // meet; keep the valid ones; fewest prisms, then no shared flutes,
    // then the biggest smallest prism, then the fewest shapes.
    // Prisms, shared flutes, sliver glue faces, sharp corners, minus the smallest area, shapes: less is better.
    type Score = (usize, usize, usize, usize, i64, usize);
    let mut best: Option<(Decomposition, Score)> = None;
    for (ds, rs) in &sols {
        let mut cuts: Vec<Cut> = ds
            .iter()
            .map(|&k| [t[diags[k][0]], t[diags[k][1]]])
            .collect();
        for &r in rs {
            let (a, end) = (t[rays[r].v], rays[r].end);
            let mut tmin = 1.0;
            for c in &cuts {
                if let Some((tt, u)) = line_meet(a, end, c[0], c[1]) {
                    if tt > EPS && tt < tmin && u > -1e-9 && u < 1.0 + 1e-9 {
                        tmin = tt;
                    }
                }
            }
            cuts.push([a, a + (end - a) * tmin]);
        }
        let Ok(d) = decompose(curve, bulge, &cuts, facet) else {
            continue;
        };
        if !d.clean {
            continue;
        }
        let smallest = d
            .prisms
            .iter()
            .map(|p| p.area)
            .fold(f64::INFINITY, f64::min);
        // A glue face shorter than SLIVER is a stub where a ray stopped
        // just short of a vertex: a flat a millimetre or two wide on a
        // stick, which no one wants to rip. Fewer of those before a
        // bigger smallest prism.
        let slivers = d
            .prisms
            .iter()
            .flat_map(|p| p.faces.iter())
            .filter(|f| f.kind == FaceKind::Glue && f.len < SLIVER)
            .count();
        // A corner under SHARP degrees is a fragile edge on the stick
        // and a nest that pinches it: fewer of those too.
        let sharp = d
            .prisms
            .iter()
            .flat_map(|p| p.corners.iter())
            .filter(|&&c| c < SHARP)
            .count();
        let score = (
            d.prisms.len(),
            d.prisms.iter().filter(|p| p.flute_split).count(),
            slivers,
            sharp,
            -(smallest * 1e6) as i64,
            d.shapes.len(),
        );
        if best.as_ref().is_none_or(|(_, s)| score < *s) {
            best = Some((d, score));
        }
    }
    best.map(|(d, _)| d)
        .ok_or_else(|| "arrangements were found but none survives building".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spectre::TILE_AREA;

    const FACET: f64 = 10.0 * std::f64::consts::PI / 180.0;
    const QUARTER: f64 = 0.207_106_78;
    const FIVE_SIXTEENTHS: f64 = 0.267_3;

    fn near(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    #[test]
    fn the_rule_is_the_arcs_end_angle_off_a_straight_angle() {
        assert!(near(theta(Curve::Arc, QUARTER), 45.0, 0.01));
        assert!(near(theta(Curve::Arc, 0.5), 90.0, 1e-9));
        assert!(near(theta(Curve::Arc, 0.357), 71.0, 0.2));
        assert!(near(theta(Curve::Ogee, 0.103_553), 45.0, 0.01));
        assert!(near(radius(Curve::Arc, QUARTER), 0.5f64.sqrt(), 1e-6));
        assert!(near(
            radius(Curve::Ogee, 0.103_553),
            0.5f64.sqrt() / 2.0,
            1e-5
        ));
    }

    #[test]
    fn the_fewest_prisms_step_with_the_bulge() {
        // Shapes repeat less than they might: a split with no corner
        // under 45 degrees is taken before one with fewer shapes.
        for (bulge, prisms, shapes_at_most) in [
            (QUARTER, 4, 4),
            (FIVE_SIXTEENTHS, 5, 5),
            (0.357, 8, 8),
            (0.5, 9, 9),
        ] {
            let d = search(Curve::Arc, bulge, FACET).unwrap();
            let sharpest = d
                .prisms
                .iter()
                .flat_map(|p| p.corners.iter().copied())
                .fold(180.0, f64::min);
            println!(
                "bulge {bulge}: {} prisms in {} shapes {:?}, sharpest corner {sharpest:.1}, cuts {}",
                d.prisms.len(),
                d.shapes.len(),
                d.shapes.iter().map(|s| s.len()).collect::<Vec<_>>(),
                d.cuts.len()
            );
            assert_eq!(d.prisms.len(), prisms, "bulge {bulge}");
            assert!(d.clean);
            // the semicircle has no split without a 30 degree point
            if bulge < 0.5 {
                assert!(
                    sharpest >= SHARP - 1e-6,
                    "bulge {bulge}: a {sharpest:.1} degree point"
                );
            }
            assert!(
                d.shapes.len() <= shapes_at_most,
                "bulge {bulge}: {} shapes",
                d.shapes.len()
            );
            let total: f64 = d.prisms.iter().map(|p| p.area).sum();
            assert!(near(total, TILE_AREA, 1e-6), "bulge {bulge}: {total}");
            // every corner where a bump ends within the rule, by hand
            for p in &d.prisms {
                let n = p.outline.len();
                for k in 0..n {
                    if p.faces[k].kind == FaceKind::Bump {
                        assert!(
                            p.corners[k] <= d.limit + 1e-6
                                && p.corners[(k + 1) % n] <= d.limit + 1e-6
                        );
                    }
                }
            }
        }
        // the semicircle's nine share one flute across a cut; the others share none
        let semi = search(Curve::Arc, 0.5, FACET).unwrap();
        assert_eq!(semi.prisms.iter().filter(|p| p.flute_split).count(), 2);
        // five sixteenths: no stub of a glue face (the ray from vertex 13
        // once stopped 0.2 edge short of vertex 10 for a bigger smallest
        // prism; the diagonal to the vertex is preferred), no prism under
        // a square edge
        let five = search(Curve::Arc, FIVE_SIXTEENTHS, FACET).unwrap();
        assert!(five.prisms.iter().all(|p| !p.flute_split));
        for p in &five.prisms {
            for f in &p.faces {
                assert!(f.kind != FaceKind::Glue || f.len >= SLIVER, "{:?}", p.faces);
            }
        }
        assert!(
            five.prisms
                .iter()
                .map(|p| p.area)
                .fold(f64::INFINITY, f64::min)
                > 1.1
        );
    }

    #[test]
    fn the_s_curve_asks_one_end_of_each_edge() {
        let d = search(Curve::Ogee, 0.103_553, FACET).unwrap();
        assert_eq!(d.prisms.len(), 5);
        assert!(d.clean);
        // the straight tile uncut: every corner reads, the four reflex ones fail convexity
        let whole = decompose(Curve::Ogee, 0.103_553, &[], FACET).unwrap();
        assert_eq!(whole.prisms.len(), 1);
        assert!(!whole.prisms[0].convex);
        assert_eq!(whole.prisms[0].faces.len(), 14);
        assert!(whole.prisms[0]
            .faces
            .iter()
            .all(|f| f.kind == FaceKind::Ogee && f.whole));
    }

    #[test]
    fn a_cut_to_an_edges_midpoint_splits_an_s_into_a_bump_and_a_flute() {
        let t = tile();
        let m = (t[0] + t[1]) * 0.5;
        // from the midpoint of edge 0 up to vertex 13 (0, 1): inside the tile
        let d = decompose(Curve::Ogee, 0.1, &[[m, t[13]]], FACET).unwrap();
        let kinds: Vec<Vec<(FaceKind, bool)>> = d
            .prisms
            .iter()
            .map(|p| p.faces.iter().map(|f| (f.kind, f.whole)).collect())
            .collect();
        assert!(kinds.iter().any(|k| k.contains(&(FaceKind::Bump, true))));
        assert!(kinds.iter().any(|k| k.contains(&(FaceKind::Flute, true))));
        assert!(!d.prisms.iter().any(|p| p.bump_split));
        // the same cut on arcs splits bump 0 in two, which no bit makes
        let d = decompose(Curve::Arc, QUARTER, &[[m, t[13]]], FACET).unwrap();
        assert!(d.prisms.iter().any(|p| p.bump_split));
    }

    #[test]
    fn the_quarter_circles_four_prisms_by_hand_and_their_curves() {
        let t = tile();
        let cuts = [[t[1], t[8]], [t[4], t[8]], [t[10], t[13]]];
        let d = decompose(Curve::Arc, QUARTER, &cuts, FACET).unwrap();
        assert_eq!(d.prisms.len(), 4);
        assert!(d.clean);
        assert_eq!(d.shapes.len(), 4);
        // the curved outlines together have the straight tile's area:
        // seven bumps add what seven flutes take away
        let curved: f64 = d.prisms.iter().map(|p| area(&p.curved)).sum();
        assert!(near(curved, TILE_AREA, 1e-3), "{curved}");
        assert!(d.prisms.iter().all(|p| p.curved.len() > p.outline.len()));
        // a cut outside the tile is refused
        assert!(decompose(Curve::Arc, QUARTER, &[[t[0], Vec2::new(5.0, -3.0)]], FACET).is_err());
    }

    #[test]
    fn mirror_images_are_one_shape() {
        // The four right triangles cut off the 90 degree corners (a flute
        // on one leg, a bump on the other) are one shape, though going
        // round the tile some are the others' mirror images.
        let t = tile();
        let cuts = [[t[13], t[1]], [t[1], t[3]], [t[5], t[7]], [t[11], t[13]]];
        let d = decompose(Curve::Arc, 0.357, &cuts, FACET).unwrap();
        assert_eq!(d.prisms.len(), 5);
        assert_eq!(d.shapes.len(), 2, "{:?}", d.shapes);
        assert_eq!(d.shapes.iter().map(|s| s.len()).max().unwrap(), 4);
        let tri = &d.prisms[d.shapes.iter().find(|s| s.len() == 4).unwrap()[0]];
        assert!(near(tri.area, 0.5, 1e-9));
        assert!(
            tri.faces.iter().any(|f| f.kind == FaceKind::Bump)
                && tri.faces.iter().any(|f| f.kind == FaceKind::Flute)
        );
        // the search's answer for that pair has eight prisms and none of
        // the triangles: it ranks a split without a corner under 45
        // degrees above one that repeats a shape, and the triangles'
        // corners are 30
        let s = search(Curve::Arc, 0.357, FACET).unwrap();
        assert_eq!(s.prisms.len(), 8);
        assert!(s
            .prisms
            .iter()
            .all(|p| p.corners.iter().all(|&c| c >= SHARP - 1e-6)));
    }
}
