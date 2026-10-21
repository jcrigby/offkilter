//! The Spectre as a jigsaw: a patch of the curved-edge aperiodic monotile
//! (Smith, Myers, Kaplan and Goodman-Strauss, *A chiral aperiodic
//! monotile*, 2023) laid under a rectangular board, every tile that
//! reaches the board eroded by half the gap and clipped to the board's
//! edges, so what is left between the pieces is one lattice of curves
//! that a bit the size of the gap can follow.
//!
//! The tile's fourteen vertices, the curve on each edge and the
//! substitution rules are Craig Kaplan's (`cs.uwaterloo.ca/~csk/spectre`).
//! Each edge is a cubic whose control points sit a third and two thirds
//! along it, `bulge` edge lengths to one side, the side alternating
//! round the tile; neighbouring tiles traverse a shared edge in opposite
//! directions with opposite parity, so the curve is the same from both
//! sides and the curved tiles still tile. Every vertex of the curved tile
//! keeps the straight tile's angle (90°, 120°, 180°, 240° or 270°),
//! because both curves at a vertex are turned by the same amount.
//!
//! Every tile in a patch has the same handedness; the substitution
//! mirrors each generation, so odd and even depths are mirror images,
//! which a board does not mind. The pairs of tiles that stand in for the
//! ninth supertile ("mystics") are the `dark` pieces: two woods cannot
//! alternate on this tiling (three tiles meet at a vertex) and three
//! cannot colour it properly either (a tile with seven neighbours round
//! it needs four), and on a gapped top the gaps outline every piece
//! anyway.

use crate::{point_in_polygon, signed_area, Profile, ProfileOptions, Sketch};
use ok_math::Vec2;
use std::collections::HashMap;

/// The tile with unit edges, counter-clockwise.
const SPECTRE: [(f64, f64); 14] = [
    (0.0, 0.0),
    (1.0, 0.0),
    (1.5, -0.866_025_403_784_438_6),
    (2.366_025_403_784_439, -0.366_025_403_784_438_65),
    (2.366_025_403_784_439, 0.633_974_596_215_561_4),
    (3.366_025_403_784_439, 0.633_974_596_215_561_4),
    (3.866_025_403_784_439, 1.5),
    (3.0, 2.0),
    (2.133_974_596_215_561, 1.5),
    (1.633_974_596_215_561_4, 2.366_025_403_784_439_3),
    (0.633_974_596_215_561_4, 2.366_025_403_784_439_3),
    (-0.366_025_403_784_438_6, 2.366_025_403_784_439_3),
    (-0.866_025_403_784_439, 1.5),
    (0.0, 1.0),
];
/// The vertices that place tiles in a supertile.
const KEYS: [usize; 4] = [3, 5, 7, 11];
/// Area of the tile with unit edges (the curves add and take away the
/// same amount), `3 + 3√3`.
pub mod rips;

pub const TILE_AREA: f64 = 8.196_152_422_706_632;

/// Kaplan's curve: the control points `0.6` edge lengths off the edge.
pub const KAPLAN_BULGE: f64 = 0.6;

/// Everything that shapes the puzzle.
#[derive(Debug, Clone, PartialEq)]
pub struct Params {
    /// The board, mm: pieces are clipped to `0..width` by `0..height`.
    pub width: f64,
    pub height: f64,
    /// The tile's edge length, mm.
    pub edge: f64,
    /// Gap between pieces, mm (0 for a tight fit).
    pub gap: f64,
    /// The shape of each edge's curve.
    pub curve: Curve,
    /// How far the curve stands off the straight edge, as a fraction of
    /// it: the control points' offset for a cubic, the sagitta for an
    /// arc; 0 gives the straight-sided tile.
    pub bulge: f64,
    /// Diameter of the router bit each piece is cut with, mm; zero for
    /// none. With a bit, the tile's four inside corners are filleted to
    /// its radius and the convex corners that meet them rounded to
    /// match, so the pieces still nest, and a curve tighter than the
    /// bit is refused.
    pub bit: f64,
    /// Slides the tiling under the board, mm: with zero the patch's
    /// most interior point sits at the board's centre.
    pub shift: Vec2,
    /// Turns the tiling under the board, degrees anticlockwise about
    /// that point, before the shift.
    pub angle: f64,
    /// Pieces smaller than this area, mm², are left to the resin: the
    /// board's edge cuts slivers off the outer tiles wherever it falls.
    pub scrap: f64,
    /// How the pieces get their woods.
    pub scheme: Scheme,
    /// Seed of the random schemes.
    pub seed: u32,
    /// Rips: cuts across the straight tile, in tile units, each along a
    /// 30° line, that split every tile into strip profiles (see
    /// `rips`); only with the straight tile (`bulge` 0).
    pub rips: Vec<rips::Cut>,
}

/// How the pieces get their woods: a wood slot (0 to 3, light, dark,
/// third, fourth) per tile, both pieces of a tile the board's edge cuts
/// getting the same one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Scheme {
    /// The mystic pairs dark, every other tile light.
    #[default]
    Mystics,
    /// Every tile light.
    One,
    /// Light or dark at random.
    TwoRandom,
    /// Light or dark by the tile's turn: tiles at even multiples of
    /// 30° light, odd ones dark. Only a mystic's partner is odd, so this
    /// is the mystic scheme with one of each pair light.
    TwoTurn,
    /// Light, dark or the third wood at random.
    ThreeRandom,
    /// Four woods with no two neighbours alike, a map colouring, the
    /// woods used about equally.
    FourMap,
    /// Four woods with no two neighbours alike, the light wood favoured
    /// and the fourth rare.
    Mix,
    /// By the strip profile the rips make, in the kit's order (largest
    /// first): a piece of the first profile light, the second dark, and
    /// so on. Without rips, every piece light.
    Profiles,
}

/// The shape of an edge's curve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Curve {
    /// Kaplan's cubic: control points a third and two thirds along the
    /// edge, `bulge` edge lengths to the side; tightest at its apex.
    #[default]
    Cubic,
    /// A circular arc of sagitta `bulge` edge lengths: one radius all
    /// along, `(1/4 + bulge²) / (2 bulge)` edges, kinder to a router bit.
    Arc,
}

/// One piece: a tile (or the part of one the board's edge leaves)
/// eroded by half the gap.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Piece {
    /// Index of the tile in the patch.
    pub tile: u32,
    /// One of a pair standing in for the ninth supertile: a dark piece
    /// under the mystic scheme.
    pub mystic: bool,
    /// The wood slot the scheme gave the tile: 0 light, 1 dark, 2 third,
    /// 3 fourth.
    #[serde(default)]
    pub wood: u8,
    /// With rips, which strip profile of the kit this piece is cut from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<u8>,
    /// Counter-clockwise outline, the gap taken off and the board's
    /// edges applied.
    pub outline: Vec<Vec2>,
}

/// The built puzzle.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Patch {
    pub pieces: Vec<Piece>,
    pub width: f64,
    pub height: f64,
    /// Substitution depth of the patch that covers the board.
    pub depth: u32,
    /// Tiles the board reaches (a tile the board's edge cuts in two is
    /// one tile, two pieces).
    pub tiles: usize,
}

// -- affine transforms, as Kaplan's [a b tx c d ty] ----------------------

/// An affine transform, as Kaplan's `[a b tx c d ty]`.
pub type Xf = [f64; 6];
const IDENT: Xf = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];

fn mul(a: &Xf, b: &Xf) -> Xf {
    [
        a[0] * b[0] + a[1] * b[3],
        a[0] * b[1] + a[1] * b[4],
        a[0] * b[2] + a[1] * b[5] + a[2],
        a[3] * b[0] + a[4] * b[3],
        a[3] * b[1] + a[4] * b[4],
        a[3] * b[2] + a[4] * b[5] + a[5],
    ]
}

fn trot(ang: f64) -> Xf {
    let (s, c) = ang.sin_cos();
    [c, -s, 0.0, s, c, 0.0]
}

fn ttrans(t: Vec2) -> Xf {
    [1.0, 0.0, t.x, 0.0, 1.0, t.y]
}

fn apply(t: &Xf, p: Vec2) -> Vec2 {
    Vec2::new(
        t[0] * p.x + t[1] * p.y + t[2],
        t[3] * p.x + t[4] * p.y + t[5],
    )
}

fn det(t: &Xf) -> f64 {
    t[0] * t[4] - t[1] * t[3]
}

// -- the substitution ----------------------------------------------------

/// Labels: the eight tiles, the mystic pair's two halves, and the pair.
const GAMMA: usize = 0;
const DELTA: usize = 1;
const GAMMA1: usize = 9;
const GAMMA2: usize = 10;

/// Which label fills each of a supertile's eight slots, by the
/// supertile's label (Gamma first); the third slot of Gamma is empty.
const RULES: [[Option<usize>; 8]; 9] = {
    const T: usize = 2; // Theta
    const L: usize = 3; // Lambda
    const X: usize = 4; // Xi
    const P: usize = 5; // Pi
    const S: usize = 6; // Sigma
    const F: usize = 7; // Phi
    const Y: usize = 8; // Psi
    const D: usize = DELTA;
    const G: usize = GAMMA;
    [
        [
            Some(P),
            Some(D),
            None,
            Some(T),
            Some(S),
            Some(X),
            Some(F),
            Some(G),
        ],
        [
            Some(X),
            Some(D),
            Some(X),
            Some(F),
            Some(S),
            Some(P),
            Some(F),
            Some(G),
        ],
        [
            Some(Y),
            Some(D),
            Some(P),
            Some(F),
            Some(S),
            Some(P),
            Some(F),
            Some(G),
        ],
        [
            Some(Y),
            Some(D),
            Some(X),
            Some(F),
            Some(S),
            Some(P),
            Some(F),
            Some(G),
        ],
        [
            Some(Y),
            Some(D),
            Some(P),
            Some(F),
            Some(S),
            Some(Y),
            Some(F),
            Some(G),
        ],
        [
            Some(Y),
            Some(D),
            Some(X),
            Some(F),
            Some(S),
            Some(Y),
            Some(F),
            Some(G),
        ],
        [
            Some(X),
            Some(D),
            Some(X),
            Some(F),
            Some(S),
            Some(P),
            Some(L),
            Some(G),
        ],
        [
            Some(Y),
            Some(D),
            Some(Y),
            Some(F),
            Some(S),
            Some(P),
            Some(F),
            Some(G),
        ],
        [
            Some(Y),
            Some(D),
            Some(Y),
            Some(F),
            Some(S),
            Some(Y),
            Some(F),
            Some(G),
        ],
    ]
};

/// A shape of one generation: a tile, or children of an earlier
/// generation placed by transforms.
#[derive(Clone)]
struct Shape {
    quad: [Vec2; 4],
    children: Vec<(usize, usize, Xf)>, // (generation, label, placement)
    mystic: bool,
}

type System = Vec<Shape>; // by label, 11 entries

fn base() -> System {
    let quad = KEYS.map(|k| Vec2::new(SPECTRE[k].0, SPECTRE[k].1));
    let leaf = |mystic| Shape {
        quad,
        children: Vec::new(),
        mystic,
    };
    let mut sys: System = (0..11).map(|_| leaf(false)).collect();
    sys[GAMMA1] = leaf(true);
    sys[GAMMA2] = leaf(true);
    let v8 = Vec2::new(SPECTRE[8].0, SPECTRE[8].1);
    sys[GAMMA] = Shape {
        quad,
        children: vec![
            (0, GAMMA1, IDENT),
            (
                0,
                GAMMA2,
                mul(&ttrans(v8), &trot(std::f64::consts::PI / 6.0)),
            ),
        ],
        mystic: false,
    };
    sys
}

fn supertiles(prev: &System, generation: usize) -> System {
    let quad = prev[DELTA].quad;
    let mirror: Xf = [-1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
    let t_rules: [(f64, usize, usize); 7] = [
        (60.0, 3, 1),
        (0.0, 2, 0),
        (60.0, 3, 1),
        (60.0, 3, 1),
        (0.0, 2, 0),
        (60.0, 3, 1),
        (-120.0, 3, 3),
    ];
    let mut ts: Vec<Xf> = vec![IDENT];
    let mut total = 0.0;
    let mut rot = IDENT;
    let mut tquad = quad;
    for (ang, from, to) in t_rules {
        total += ang;
        if ang != 0.0 {
            rot = trot(total.to_radians());
            tquad = quad.map(|q| apply(&rot, q));
        }
        let p = apply(ts.last().unwrap(), quad[from]);
        ts.push(mul(&ttrans(p - tquad[to]), &rot));
    }
    for t in &mut ts {
        *t = mul(&mirror, t);
    }
    let super_quad = [
        apply(&ts[6], quad[2]),
        apply(&ts[5], quad[1]),
        apply(&ts[3], quad[2]),
        apply(&ts[0], quad[1]),
    ];
    let mut next = prev.clone();
    for (label, rule) in RULES.iter().enumerate() {
        let children = rule
            .iter()
            .zip(&ts)
            .filter_map(|(slot, t)| slot.map(|lab| (generation - 1, lab, *t)))
            .collect();
        next[label] = Shape {
            quad: super_quad,
            children,
            mystic: false,
        };
    }
    next
}

fn leaves(gens: &[System], g: usize, label: usize, xf: Xf, out: &mut Vec<(bool, Xf)>) {
    let shape = &gens[g][label];
    if shape.children.is_empty() {
        out.push((shape.mystic, xf));
    } else {
        for (cg, cl, ct) in &shape.children {
            leaves(gens, *cg, *cl, mul(&xf, ct), out);
        }
    }
}

/// Every tile of the Delta supertile of the given depth, as (mystic,
/// placement) in tile units.
pub fn tiles(depth: u32) -> Vec<(bool, Xf)> {
    let mut gens = vec![base()];
    for g in 1..=depth as usize {
        let next = supertiles(&gens[g - 1], g);
        gens.push(next);
    }
    let mut out = Vec::new();
    leaves(&gens, depth as usize, DELTA, IDENT, &mut out);
    out
}

// -- the curved tile -----------------------------------------------------

pub(crate) fn vertex(i: usize) -> Vec2 {
    Vec2::new(SPECTRE[i % 14].0, SPECTRE[i % 14].1)
}

/// The curved tile with unit edges in its own frame, counter-clockwise,
/// `per_edge` straight pieces per edge, the first vertex first.
pub fn curved_tile(curve: Curve, bulge: f64, per_edge: usize) -> Vec<Vec2> {
    let n = per_edge.max(1);
    let mut out = Vec::with_capacity(14 * n);
    let mut sign = 1.0;
    for i in 0..14 {
        let a = vertex(i + 13);
        let b = vertex(i);
        let v = b - a;
        match curve {
            Curve::Cubic => {
                let w = v.perp() * (sign * bulge);
                let c1 = a + v * 0.33 + w;
                let c2 = a + v * 0.67 + w;
                for k in 1..=n {
                    let t = k as f64 / n as f64;
                    let u = 1.0 - t;
                    out.push(
                        a * (u * u * u)
                            + c1 * (3.0 * u * u * t)
                            + c2 * (3.0 * u * t * t)
                            + b * (t * t * t),
                    );
                }
            }
            Curve::Arc if bulge.abs() > 1e-9 => {
                // The circle through both ends whose apex stands `bulge`
                // off the edge's middle, swept the short way.
                let radius = (0.25 + bulge * bulge) / (2.0 * bulge);
                let centre = (a + b) * 0.5 - v.perp() * (sign * (radius - bulge));
                let a0 = (a - centre).angle();
                let mut sweep = (b - centre).angle() - a0;
                while sweep > std::f64::consts::PI {
                    sweep -= std::f64::consts::TAU;
                }
                while sweep < -std::f64::consts::PI {
                    sweep += std::f64::consts::TAU;
                }
                for k in 1..=n {
                    let t = k as f64 / n as f64;
                    out.push(centre + Vec2::from_angle(a0 + sweep * t) * radius);
                }
            }
            Curve::Arc => {
                for k in 1..=n {
                    out.push(a + v * (k as f64 / n as f64));
                }
            }
        }
        sign = -sign;
    }
    // Start at vertex 0: the loop above ends each edge on its vertex.
    out.rotate_left(n - 1);
    out
}

/// Straight pieces per edge at the given facet angle: the curve turns
/// through twice the angle its ends make with the edge.
pub fn pieces_per_edge(curve: Curve, bulge: f64, facet: f64) -> usize {
    let end_angle = match curve {
        Curve::Cubic => bulge.abs().atan2(0.33),
        Curve::Arc => 2.0 * (2.0 * bulge.abs()).atan(),
    };
    ((2.0 * end_angle / facet.max(1e-3)).ceil() as usize).clamp(4, 64)
}

/// The tightest radius along an edge's curve, in edge lengths: what a
/// router bit following the concave side has to get round.
pub fn tightest_radius(curve: Curve, bulge: f64) -> f64 {
    if bulge.abs() < 1e-9 {
        return f64::INFINITY;
    }
    match curve {
        Curve::Arc => (0.25 + bulge * bulge) / (2.0 * bulge),
        Curve::Cubic => {
            // One edge from (0, 0) to (1, 0), sampled finely: the apex
            // at t = 1/2 is the tightest.
            let (c1, c2) = (Vec2::new(0.33, bulge), Vec2::new(0.67, bulge));
            let at = |t: f64| {
                let u = 1.0 - t;
                c1 * (3.0 * u * u * t) + c2 * (3.0 * u * t * t) + Vec2::new(t * t * t, 0.0)
            };
            let mut best = f64::INFINITY;
            for i in 1..400 {
                let (a, b, c) = (
                    at((i - 1) as f64 / 400.0),
                    at(i as f64 / 400.0),
                    at((i + 1) as f64 / 400.0),
                );
                let cross = (b - a).cross(c - b).abs();
                if cross > 1e-15 {
                    best = best.min(a.distance(b) * b.distance(c) * a.distance(c) / (2.0 * cross));
                }
            }
            best
        }
    }
}

/// Interior angles of the straight tile at its vertices, degrees.
const ANGLES: [u32; 14] = [
    90, 240, 90, 120, 270, 120, 90, 120, 270, 120, 180, 120, 90, 240,
];

// -- a router bit's corners ----------------------------------------------

/// Closest point on the polyline `pts` to `p`, as (distance, index of
/// the segment's first point, parameter along it).
fn nearest_on(pts: &[Vec2], p: Vec2) -> (f64, usize, f64) {
    let mut best = (f64::INFINITY, 0, 0.0);
    for i in 0..pts.len().saturating_sub(1) {
        let (a, b) = (pts[i], pts[i + 1]);
        let d = b - a;
        let len2 = d.length_squared();
        let t = if len2 > 0.0 {
            ((p - a).dot(d) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let dist = p.distance(a + d * t);
        if dist < best.0 {
            best = (dist, i, t);
        }
    }
    best
}

/// The point `s` along a polyline from its start, and the unit
/// direction of the piece it lies on.
fn along_polyline(pts: &[Vec2], s: f64) -> (Vec2, Vec2) {
    let mut left = s;
    for i in 0..pts.len() - 1 {
        let d = pts[i + 1] - pts[i];
        let len = d.length();
        if left <= len || i + 2 == pts.len() {
            let t = if len > 0.0 {
                (left / len).min(1.0)
            } else {
                0.0
            };
            return (pts[i] + d * t, d.normalized().unwrap_or(Vec2::X));
        }
        left -= len;
    }
    (*pts.last().unwrap(), Vec2::X)
}

/// Replaces the corner at `at` of the counter-clockwise polygon by the
/// arc of radius `r` tangent to both curves meeting there: outside the
/// polygon for an inside corner (what a bit of that radius leaves), or
/// inside it for a convex corner (what its mate then needs). The arc is
/// sampled at `facet`.
fn round_corner(
    poly: &[Vec2],
    at: usize,
    r: f64,
    outside: bool,
    facet: f64,
) -> Result<Vec<Vec2>, String> {
    let n = poly.len();
    // Rotate so the corner sits in the middle and nothing wraps.
    let shift = (at + n - n / 2) % n;
    let p: Vec<Vec2> = (0..n).map(|i| poly[(i + shift) % n]).collect();
    let at = n / 2;
    // The curve into the corner, walked back from it, and the curve out.
    let back: Vec<Vec2> = (0..=at).rev().map(|i| p[i]).collect();
    let fwd: Vec<Vec2> = (at..n).map(|i| p[i]).collect();
    let side = if outside { -1.0 } else { 1.0 };
    // Circle centre from a cut `s` back along the incoming curve: its
    // distance to the outgoing curve must be r too.
    let centre = |s: f64| {
        let (a, dir) = along_polyline(&back, s);
        // `dir` runs away from the corner; travel along the polygon is
        // the other way, so the left normal of travel is -dir.perp().
        (a, a + dir.perp() * (-side * r))
    };
    let gap = |s: f64| nearest_on(&fwd, centre(s).1).0 - r;
    let reach: f64 = back.windows(2).map(|w| w[0].distance(w[1])).sum::<f64>() * 0.45;
    let (mut lo, mut hi) = (1e-6, reach.min(4.0 * r));
    if gap(lo) > 0.0 || gap(hi) < 0.0 {
        return Err(format!("a bit of radius {r:.2} does not fit the corner"));
    }
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if gap(mid) < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let s = 0.5 * (lo + hi);
    let (a, c) = centre(s);
    let (_, j, t) = nearest_on(&fwd, c);
    let b = fwd[j] + (fwd[j + 1] - fwd[j]) * t;
    let (_, i, u) = nearest_on(&back, a);
    // The arc from a to b about c, the short way.
    let a0 = (a - c).angle();
    let mut sweep = (b - c).angle() - a0;
    while sweep > std::f64::consts::PI {
        sweep -= std::f64::consts::TAU;
    }
    while sweep < -std::f64::consts::PI {
        sweep += std::f64::consts::TAU;
    }
    let steps = ((sweep.abs() / facet.max(1e-3)).ceil() as usize).max(1);
    let mut out: Vec<Vec2> = Vec::with_capacity(n + steps);
    // Up to the cut on the incoming curve: `a` lies between back[i + 1]
    // and back[i], which are p[at - i - 1] and p[at - i].
    let _ = u;
    out.extend_from_slice(&p[..at - i]);
    for k in 0..=steps {
        out.push(c + Vec2::from_angle(a0 + sweep * k as f64 / steps as f64) * r);
    }
    // From the cut on the outgoing curve: `b` lies between fwd[j] and
    // fwd[j + 1], which are p[at + j] and p[at + j + 1].
    out.extend_from_slice(&p[(at + j + 1).min(n)..]);
    // Drop near-duplicates the cuts may have left.
    let mut clean: Vec<Vec2> = Vec::with_capacity(out.len());
    for q in out {
        if clean.last().is_none_or(|l| l.distance(q) > 1e-7) {
            clean.push(q);
        }
    }
    if clean.len() > 3 && clean[0].distance(*clean.last().unwrap()) <= 1e-7 {
        clean.pop();
    }
    Ok(clean)
}

// -- offsets, clipping ---------------------------------------------------

/// The polygon moved `d` inwards (outwards for negative `d`): corners
/// that would open are rounded with arcs about the corner, sampled at
/// `facet`, the others mitred. The polygon must run counter-clockwise.
pub fn offset(poly: &[Vec2], d: f64, facet: f64) -> Vec<Vec2> {
    let n = poly.len();
    if n < 3 || d == 0.0 {
        return poly.to_vec();
    }
    let mut out = Vec::with_capacity(n + 16);
    for i in 0..n {
        let p = poly[i];
        let e0 = (p - poly[(i + n - 1) % n]).normalized().unwrap_or(Vec2::X);
        let e1 = (poly[(i + 1) % n] - p).normalized().unwrap_or(Vec2::X);
        // Inward normals (the interior is on the left of a ccw edge).
        let (n0, n1) = (e0.perp(), e1.perp());
        let turn = e0.cross(e1);
        if turn * d >= 0.0 {
            // Convex and shrinking, or reflex and growing: the offset
            // edges cross at the mitre point.
            let bis = n0 + n1;
            let denom = 1.0 + n0.dot(n1);
            if denom > 1e-9 {
                out.push(p + bis * (d / denom));
            } else {
                out.push(p + n0 * d);
                out.push(p + n1 * d);
            }
        } else {
            // The offset edges part: an arc about the corner joins them.
            let a0 = n0.angle();
            let delta = n0.cross(n1).atan2(n0.dot(n1));
            let steps = ((delta.abs() / facet.max(1e-3)).ceil() as usize).max(1);
            for k in 0..=steps {
                let a = a0 + delta * (k as f64 / steps as f64);
                out.push(p + Vec2::from_angle(a) * d);
            }
        }
    }
    out
}

fn segments_cross(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> bool {
    let r = b - a;
    let s = d - c;
    let denom = r.cross(s);
    if denom.abs() < 1e-14 {
        return false;
    }
    let t = (c - a).cross(s) / denom;
    let u = (c - a).cross(r) / denom;
    let eps = 1e-9;
    t > eps && t < 1.0 - eps && u > eps && u < 1.0 - eps
}

/// Whether any two non-adjacent edges of the polygon cross.
pub fn self_intersects(poly: &[Vec2]) -> bool {
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        for j in i + 2..n {
            if i == 0 && j == n - 1 {
                continue;
            }
            if segments_cross(a, b, poly[j], poly[(j + 1) % n]) {
                return true;
            }
        }
    }
    false
}

/// Liang–Barsky: the part of `a..b` inside the board, its ends snapped
/// onto the edges they land on.
fn clip(a: Vec2, b: Vec2, w: f64, h: f64) -> Option<(Vec2, Vec2)> {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-d.x, a.x), (d.x, w - a.x), (-d.y, a.y), (d.y, h - a.y)] {
        if p.abs() < 1e-15 {
            if q < 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
        }
    }
    if t0 > t1 {
        return None;
    }
    let snap = |v: f64, lim: f64| {
        if v.abs() < 1e-9 {
            0.0
        } else if (v - lim).abs() < 1e-9 {
            lim
        } else {
            v
        }
    };
    let at = |t: f64| {
        let p = a + d * t;
        Vec2::new(snap(p.x, w), snap(p.y, h))
    };
    Some((at(t0), at(t1)))
}

/// The four board edges as (origin, direction, length), each running
/// counter-clockwise round the board.
fn board_edges(w: f64, h: f64) -> [(Vec2, Vec2, f64); 4] {
    [
        (Vec2::ZERO, Vec2::X, w),
        (Vec2::new(w, 0.0), Vec2::Y, h),
        (Vec2::new(w, h), -Vec2::X, w),
        (Vec2::new(0.0, h), -Vec2::Y, h),
    ]
}

/// Where along a board edge a point lies, if it lies on it.
fn along(edge: &(Vec2, Vec2, f64), p: Vec2) -> Option<f64> {
    let (o, dir, len) = *edge;
    let r = p - o;
    let s = r.dot(dir);
    (r.cross(dir).abs() < 1e-9 && (-1e-9..=len + 1e-9).contains(&s)).then_some(s.clamp(0.0, len))
}

/// The parts of a counter-clockwise polygon inside the board, each a
/// counter-clockwise outline: the polygon's edges clipped to the board
/// plus the stretches of the board's edges that run through the polygon,
/// closed by the region finder.
fn clip_polygon(poly: &[Vec2], w: f64, h: f64, opts: &ProfileOptions) -> Vec<Vec<Vec2>> {
    let mut sk = Sketch::new();
    let mut on_edges: [Vec<f64>; 4] = Default::default();
    let edges = board_edges(w, h);
    let n = poly.len();
    for i in 0..n {
        if let Some((a, b)) = clip(poly[i], poly[(i + 1) % n], w, h) {
            if a.distance(b) > 1e-9 {
                sk.add_line(a, b);
                for (k, e) in edges.iter().enumerate() {
                    for p in [a, b] {
                        if let Some(s) = along(e, p) {
                            on_edges[k].push(s);
                        }
                    }
                }
            }
        }
    }
    for (k, e) in edges.iter().enumerate() {
        let (o, dir, len) = *e;
        let mut stops = std::mem::take(&mut on_edges[k]);
        stops.push(0.0);
        stops.push(len);
        stops.sort_by(f64::total_cmp);
        stops.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        for pair in stops.windows(2) {
            let mid = o + dir * (0.5 * (pair[0] + pair[1]));
            if point_in_polygon(mid, poly) {
                sk.add_line(o + dir * pair[0], o + dir * pair[1]);
            }
        }
    }
    sk.profiles(opts)
        .into_iter()
        .filter(|p| p.holes.is_empty())
        .map(|p| p.outer.points)
        .collect()
}

// -- the patch -----------------------------------------------------------

/// A point rounded to a millionth, for matching shared vertices.
type Key = (i64, i64);
/// An edge by its two rounded ends, the lesser first.
type EdgeKey = (Key, Key);
/// A patch that covers the board: its depth, its tiles and its centre.
type Placed = (u32, Vec<(bool, Xf)>, Vec2);

fn key(p: Vec2) -> Key {
    ((p.x * 1e6).round() as i64, (p.y * 1e6).round() as i64)
}

fn edge_key(a: Vec2, b: Vec2) -> EdgeKey {
    let (ka, kb) = (key(a), key(b));
    if ka <= kb {
        (ka, kb)
    } else {
        (kb, ka)
    }
}

fn bbox(pts: &[Vec2]) -> (Vec2, Vec2) {
    let mut lo = Vec2::new(f64::INFINITY, f64::INFINITY);
    let mut hi = -lo;
    for p in pts {
        lo = Vec2::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Vec2::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    (lo, hi)
}

/// Builds the puzzle: the shallowest patch whose tiles cover the board
/// once its most interior point (the tile centre furthest from the
/// patch's boundary) is put at the board's centre and shifted, every
/// tile the board reaches eroded by half the gap and clipped to the
/// board. `facet` is the angle per straight piece of the curves and
/// the rounded corners.
pub fn build(p: &Params, facet: f64) -> Result<Patch, String> {
    for (name, v) in [("width", p.width), ("height", p.height), ("edge", p.edge)] {
        if !(v.is_finite() && v > 0.0) {
            return Err(format!("{name} must be positive"));
        }
    }
    if !(p.gap.is_finite() && p.gap >= 0.0) {
        return Err("gap must be zero or positive".into());
    }
    if p.gap > 0.25 * p.edge {
        return Err(format!(
            "gap {:.2} is over a quarter of the edge {:.2}: the pieces would lose their lobes",
            p.gap, p.edge
        ));
    }
    if !(p.bulge.is_finite() && (0.0..=1.0).contains(&p.bulge)) {
        return Err("bulge must be between 0 (straight edges) and 1".into());
    }
    if p.curve == Curve::Arc && p.bulge > 0.5 {
        return Err("an arc's sagitta must be at most half the edge (a semicircle)".into());
    }
    if !(p.bit.is_finite() && p.bit >= 0.0) {
        return Err("bit must be zero or positive".into());
    }
    if !p.rips.is_empty() {
        if p.bulge != 0.0 {
            return Err("rips need the straight tile: set the bulge to 0".into());
        }
        if let Some(c) = p.rips.iter().find(|c| !rips::cut_ok(c[0], c[1])) {
            return Err(format!(
                "rip from ({:.3}, {:.3}) to ({:.3}, {:.3}) is not a cut along a 30° line through the tile",
                c[0].x, c[0].y, c[1].x, c[1].y
            ));
        }
    }
    if p.bit > 0.0 {
        let tightest = tightest_radius(p.curve, p.bulge) * p.edge;
        if tightest < p.bit / 2.0 {
            return Err(format!(
                "the curve's tightest radius {tightest:.2} mm is under the bit's {:.2}: a flatter curve or a longer edge",
                p.bit / 2.0
            ));
        }
        if p.bit > p.edge {
            return Err(format!(
                "bit {:.2} is over the edge {:.2}: its fillets would meet",
                p.bit, p.edge
            ));
        }
    }
    if !(p.shift.x.is_finite() && p.shift.y.is_finite()) {
        return Err("shift must be finite".into());
    }
    if !p.angle.is_finite() {
        return Err("angle must be finite".into());
    }
    let turn = Vec2::from_angle(p.angle.to_radians());
    let spin = move |v: Vec2| Vec2::new(v.x * turn.x - v.y * turn.y, v.x * turn.y + v.y * turn.x);
    if !(p.scrap.is_finite() && p.scrap >= 0.0) {
        return Err("scrap must be zero or positive".into());
    }
    // The curves and the rounded corners want no finer than 10°: the
    // pieces are polylines, and a top is a few hundred of them.
    let facet = facet.max(10f64.to_radians());
    let per_edge = pieces_per_edge(p.curve, p.bulge, facet);
    let local = curved_tile(p.curve, p.bulge, per_edge);
    let margin = p.bulge.abs().max(0.1);
    let board = (Vec2::ZERO, Vec2::new(p.width, p.height));
    let centre = Vec2::new(p.width, p.height) * 0.5 + p.shift;
    // The depth that covers the board, its tiles and the patch's centre.
    let mut placed: Option<Placed> = None;
    let half_diagonal = Vec2::new(p.width, p.height).length() / (2.0 * p.edge);
    for depth in 1..=5u32 {
        let all = tiles(depth);
        let verts_of = |xf: &Xf| -> Vec<Vec2> { (0..14).map(|i| apply(xf, vertex(i))).collect() };
        // The patch's boundary: the edges with one tile. The board goes
        // round the tile centre furthest from them, the patch's most
        // interior point, since a patch is a blob with deep bays and its
        // bounding box's centre can sit in one.
        let mut owners: HashMap<EdgeKey, (u32, Vec2)> = HashMap::new();
        for (_, xf) in &all {
            let verts = verts_of(xf);
            for i in 0..14 {
                let (a, b) = (verts[i], verts[(i + 1) % 14]);
                let e = owners.entry(edge_key(a, b)).or_insert((0, (a + b) * 0.5));
                e.0 += 1;
            }
        }
        let boundary: Vec<Vec2> = owners
            .values()
            .filter(|(n, _)| *n == 1)
            .map(|(_, m)| *m)
            .collect();
        let mut pole = (f64::NEG_INFINITY, Vec2::ZERO);
        for (_, xf) in &all {
            let verts = verts_of(xf);
            let c = verts.iter().fold(Vec2::ZERO, |s, v| s + *v) * (1.0 / 14.0);
            let d = boundary
                .iter()
                .map(|m| m.distance(c))
                .fold(f64::INFINITY, f64::min);
            if d > pole.0 {
                pole = (d, c);
            }
        }
        // A board whose corners reach the boundary cannot fit.
        if pole.0 + p.shift.length() / p.edge < half_diagonal {
            continue;
        }
        let patch_centre = pole.1;
        let place = |q: Vec2| spin((q - patch_centre) * p.edge) + centre;
        // Covered: the board's corners each lie in a tile and no edge
        // of the patch's boundary crosses the board.
        let mut corners_in = [false; 4];
        let corners = [
            board.0,
            Vec2::new(board.1.x, 0.0),
            board.1,
            Vec2::new(0.0, board.1.y),
        ];
        for (_, xf) in &all {
            let verts = verts_of(xf);
            // The placed tile's box, from its placed vertices (the tiling may
            // be turned), a curve's bulge wider.
            let placed_verts: Vec<Vec2> = verts.iter().map(|v| place(*v)).collect();
            let (tlo, thi) = bbox(&placed_verts);
            let pad = Vec2::new(margin, margin) * p.edge;
            let (tlo, thi) = (tlo - pad, thi + pad);
            if corners
                .iter()
                .any(|c| c.x >= tlo.x && c.x <= thi.x && c.y >= tlo.y && c.y <= thi.y)
            {
                let poly: Vec<Vec2> = local.iter().map(|q| place(apply(xf, *q))).collect();
                for (k, c) in corners.iter().enumerate() {
                    if point_in_polygon(*c, &poly) {
                        corners_in[k] = true;
                    }
                }
            }
        }
        let mut covered = corners_in.iter().all(|&c| c);
        if covered {
            'tiles: for (_, xf) in &all {
                let verts = verts_of(xf);
                for i in 0..14 {
                    if owners[&edge_key(verts[i], verts[(i + 1) % 14])].0 == 1 {
                        // A boundary edge: its curve must stay off the board.
                        let (a, b) = (verts[i], verts[(i + 1) % 14]);
                        let (lo, hi) = bbox(&[place(a), place(b)]);
                        let lo = lo - Vec2::new(margin, margin) * p.edge;
                        let hi = hi + Vec2::new(margin, margin) * p.edge;
                        if lo.x <= board.1.x && hi.x >= 0.0 && lo.y <= board.1.y && hi.y >= 0.0 {
                            // Vertex i sits at local[per_edge * i]; the
                            // curve to vertex i + 1 is the pieces after it.
                            let at = |j: usize| place(apply(xf, local[j % (14 * per_edge)]));
                            for j in per_edge * i..per_edge * (i + 1) {
                                if clip(at(j), at(j + 1), p.width, p.height).is_some() {
                                    covered = false;
                                    break 'tiles;
                                }
                            }
                        }
                    }
                }
            }
        }
        if covered {
            placed = Some((depth, all, patch_centre));
            break;
        }
    }
    let Some((depth, all, patch_centre)) = placed else {
        return Err(
            "no patch up to depth 5 covers the board: a smaller board or a longer edge".into(),
        );
    };
    let place = |q: Vec2| spin((q - patch_centre) * p.edge) + centre;
    let opts = ProfileOptions {
        arc_segment_angle: facet,
        merge_tolerance: 1e-6,
    };
    // How many tiles meet at each vertex: a convex corner at a vertex of
    // two tiles meets an inside corner, and takes its rounding.
    let mut at_vertex: HashMap<Key, u32> = HashMap::new();
    if p.bit > 0.0 {
        for (_, xf) in &all {
            for i in 0..14 {
                *at_vertex.entry(key(apply(xf, vertex(i)))).or_insert(0) += 1;
            }
        }
    }
    let woods = colour(&all, p.scheme, p.seed);
    let mut pieces = Vec::new();
    let mut touched = 0;
    for (k, (mystic, xf)) in all.iter().enumerate() {
        let mut poly: Vec<Vec2> = local.iter().map(|q| place(apply(xf, *q))).collect();
        if det(xf) < 0.0 {
            poly.reverse();
        }
        let (lo, hi) = bbox(&poly);
        if lo.x >= p.width || hi.x <= 0.0 || lo.y >= p.height || hi.y <= 0.0 {
            continue;
        }
        if p.bit > 0.0 {
            // Inside corners get the bit's fillet; a convex corner that
            // meets one (two tiles at the vertex) is rounded to match.
            // The corners are found by their points, since each rounding
            // renumbers the polygon.
            let corners: Vec<(Vec2, bool)> = (0..14)
                .filter_map(|i| {
                    let q = apply(xf, vertex(i));
                    let inside = ANGLES[i] > 180;
                    let mate = ANGLES[i] < 180 && at_vertex.get(&key(q)) == Some(&2);
                    (inside || mate).then(|| (place(q), inside))
                })
                .collect();
            for (q, inside) in corners {
                let at = (0..poly.len())
                    .min_by(|&a, &b| poly[a].distance(q).total_cmp(&poly[b].distance(q)))
                    .unwrap();
                poly = round_corner(&poly, at, p.bit / 2.0, inside, facet)
                    .map_err(|e| format!("tile {}: {e}", k + 1))?;
            }
        }
        let eroded = offset(&poly, p.gap / 2.0, facet);
        if self_intersects(&eroded) {
            return Err(format!(
                "the gap is too wide for the edge: tile {} loses a lobe to it",
                k + 1
            ));
        }
        let mut kept = false;
        for outline in clip_polygon(&eroded, p.width, p.height, &opts) {
            if signed_area(&outline) < p.scrap {
                continue;
            }
            kept = true;
            pieces.push(Piece {
                tile: k as u32,
                mystic: *mystic,
                wood: woods[k],
                profile: None,
                outline,
            });
        }
        touched += kept as usize;
    }
    if !p.rips.is_empty() {
        // Every piece splits by the strip profiles the rips make,
        // mapped through its tile's placement; a profile's pieces take
        // its wood under the profiles scheme.
        let faces = rips::pieces(&p.rips);
        let profiles = rips::profile_of(&faces);
        let mut split = Vec::new();
        for piece in pieces {
            let xf = &all[piece.tile as usize].1;
            let parts = rips::split_by(&piece.outline, &faces, |q| place(apply(xf, q)));
            for (j, outline) in parts {
                if signed_area(&outline) < p.scrap {
                    continue;
                }
                split.push(Piece {
                    tile: piece.tile,
                    mystic: piece.mystic,
                    wood: if p.scheme == Scheme::Profiles {
                        profiles[j].min(3)
                    } else {
                        piece.wood
                    },
                    profile: Some(profiles[j]),
                    outline,
                });
            }
        }
        pieces = split;
    }
    if pieces.is_empty() {
        return Err("no tile reaches the board".into());
    }
    Ok(Patch {
        pieces,
        width: p.width,
        height: p.height,
        depth,
        tiles: touched,
    })
}

/// A wood slot for every tile of a patch under a scheme. The tiles are
/// the whole patch, so a board gets the same woods wherever it sits on
/// it and however the random schemes fall.
pub fn colour(all: &[(bool, Xf)], scheme: Scheme, seed: u32) -> Vec<u8> {
    let n = all.len();
    let mut rng = Rng(seed ^ 0x9e37_79b9);
    let turn_odd = |xf: &Xf| -> bool {
        // The tile's turn in 30° steps, from where its x axis points.
        let deg = xf[3].atan2(xf[0]).to_degrees();
        ((deg / 30.0).round() as i64).rem_euclid(2) == 1
    };
    match scheme {
        Scheme::Mystics => all.iter().map(|(m, _)| *m as u8).collect(),
        Scheme::One | Scheme::Profiles => vec![0; n],
        Scheme::TwoRandom => (0..n).map(|_| (rng.next() % 2) as u8).collect(),
        Scheme::TwoTurn => all.iter().map(|(_, xf)| turn_odd(xf) as u8).collect(),
        Scheme::ThreeRandom => (0..n).map(|_| (rng.next() % 3) as u8).collect(),
        Scheme::FourMap | Scheme::Mix => {
            // Neighbours: tiles sharing an edge.
            let mut owners: HashMap<EdgeKey, Vec<usize>> = HashMap::new();
            for (k, (_, xf)) in all.iter().enumerate() {
                for i in 0..14 {
                    let (a, b) = (apply(xf, vertex(i)), apply(xf, vertex(i + 1)));
                    owners.entry(edge_key(a, b)).or_default().push(k);
                }
            }
            let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
            for ks in owners.values() {
                if let [a, b] = ks[..] {
                    adj[a].push(b);
                    adj[b].push(a);
                }
            }
            let weights: [u32; 4] = if scheme == Scheme::Mix {
                [8, 4, 2, 1]
            } else {
                [1, 1, 1, 1]
            };
            // DSatur: the tile with the most distinct neighbouring woods
            // next, ties by degree then at random; a wood from those its
            // neighbours lack, drawn by weight. A patch is a map, so
            // four always suffice in practice; a few retries cover the
            // order's bad luck.
            let mut best: Option<(usize, Vec<u8>)> = None;
            for _ in 0..24 {
                let mut wood: Vec<Option<u8>> = vec![None; n];
                let mut clashes = 0;
                let noise: Vec<u32> = (0..n).map(|_| rng.next()).collect();
                for _ in 0..n {
                    let k = (0..n)
                        .filter(|&k| wood[k].is_none())
                        .max_by_key(|&k| {
                            let sat = adj[k]
                                .iter()
                                .filter_map(|&j| wood[j])
                                .collect::<std::collections::BTreeSet<u8>>()
                                .len();
                            (sat, adj[k].len(), noise[k])
                        })
                        .unwrap();
                    let used: Vec<u8> = adj[k].iter().filter_map(|&j| wood[j]).collect();
                    let free: Vec<u8> = (0..4u8).filter(|w| !used.contains(w)).collect();
                    let pick = if free.is_empty() {
                        clashes += 1;
                        (rng.next() % 4) as u8
                    } else {
                        let total: u32 = free.iter().map(|&w| weights[w as usize]).sum();
                        let mut r = rng.next() % total;
                        let mut chosen = free[0];
                        for &w in &free {
                            if r < weights[w as usize] {
                                chosen = w;
                                break;
                            }
                            r -= weights[w as usize];
                        }
                        chosen
                    };
                    wood[k] = Some(pick);
                }
                let result: Vec<u8> = wood.into_iter().map(|w| w.unwrap()).collect();
                if best.as_ref().is_none_or(|(c, _)| clashes < *c) {
                    best = Some((clashes, result));
                }
                if clashes == 0 {
                    break;
                }
            }
            best.unwrap().1
        }
    }
}

/// A small seeded generator (mulberry32), so a seed draws the same
/// woods everywhere.
struct Rng(u32);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_add(0x6d2b_79f5);
        let mut z = self.0;
        z = (z ^ (z >> 15)).wrapping_mul(z | 1);
        z ^= z.wrapping_add((z ^ (z >> 7)).wrapping_mul(z | 61));
        z ^ (z >> 14)
    }
}

/// Draws an outline into a sketch as lines.
pub fn draw(outline: &[Vec2], sk: &mut Sketch) {
    let n = outline.len();
    for i in 0..n {
        sk.add_line(outline[i], outline[(i + 1) % n]);
    }
}

/// A piece's outline as the one region it closes.
pub fn piece_profile(piece: &Piece, opts: &ProfileOptions) -> Result<Profile, String> {
    let mut sk = Sketch::new();
    draw(&piece.outline, &mut sk);
    let mut profiles = sk.profiles(opts);
    if profiles.len() != 1 {
        return Err(format!(
            "piece of tile {} does not close into one region ({} found)",
            piece.tile + 1,
            profiles.len()
        ));
    }
    Ok(profiles.remove(0))
}

/// The regions between the pieces, largest first: every outline plus
/// the stretches of the board's edges between pieces, from which the
/// region finder yields the pieces and the gaps. The first is the
/// lattice proper; any others are gaps the board's edge closes off in a
/// piece's concavity, where the piece beyond the edge had a lobe.
pub fn lattice(patch: &Patch, opts: &ProfileOptions) -> Result<Vec<Profile>, String> {
    let mut sk = Sketch::new();
    let mut piece_area = 0.0;
    let edges = board_edges(patch.width, patch.height);
    let mut runs: [Vec<(f64, f64)>; 4] = Default::default();
    let mut outlines: std::collections::HashSet<Vec<Key>> = Default::default();
    for piece in &patch.pieces {
        draw(&piece.outline, &mut sk);
        piece_area += signed_area(&piece.outline).abs();
        outlines.insert(outline_key(&piece.outline));
        let n = piece.outline.len();
        for i in 0..n {
            let (a, b) = (piece.outline[i], piece.outline[(i + 1) % n]);
            for (k, e) in edges.iter().enumerate() {
                if let (Some(s), Some(t)) = (along(e, a), along(e, b)) {
                    runs[k].push((s.min(t), s.max(t)));
                }
            }
        }
    }
    for (k, e) in edges.iter().enumerate() {
        let (o, dir, len) = *e;
        let mut r = std::mem::take(&mut runs[k]);
        r.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut at = 0.0;
        for (lo, hi) in r {
            if lo - at > 1e-9 {
                sk.add_line(o + dir * at, o + dir * lo);
            }
            at = at.max(hi);
        }
        if len - at > 1e-9 {
            sk.add_line(o + dir * at, o + dir * len);
        }
    }
    let expected = patch.width * patch.height - piece_area;
    let mut gaps: Vec<Profile> = sk
        .profiles(opts)
        .into_iter()
        .filter(|p| !outlines.contains(&outline_key(&p.outer.points)))
        .collect();
    gaps.sort_by(|a, b| b.area().total_cmp(&a.area()));
    let total: f64 = gaps.iter().map(|p| p.area()).sum();
    if gaps.is_empty() || (total - expected).abs() > 0.05 * expected.max(1.0) {
        return Err(format!(
            "the gaps do not add up: {total:.1} mm² in {} regions against {expected:.1} between the pieces",
            gaps.len()
        ));
    }
    Ok(gaps)
}

/// A loop's vertices as a sorted, rounded set: the same from any start.
fn outline_key(outline: &[Vec2]) -> Vec<Key> {
    let mut k: Vec<Key> = outline.iter().map(|p| key(*p)).collect();
    k.sort_unstable();
    k
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    #[test]
    fn the_tile_has_unit_edges_and_the_known_area() {
        for i in 0..14 {
            assert!(
                near(vertex(i).distance(vertex(i + 1)), 1.0, 1e-12),
                "edge {i}"
            );
        }
        let straight: Vec<Vec2> = (0..14).map(vertex).collect();
        assert!(near(signed_area(&straight), TILE_AREA, 1e-12));
        // The curves add on one side what they take off the other.
        let curved = curved_tile(Curve::Cubic, KAPLAN_BULGE, 32);
        let arced = curved_tile(Curve::Arc, 0.3, 32);
        assert!(
            near(signed_area(&arced), TILE_AREA, 1e-3),
            "{}",
            signed_area(&arced)
        );
        assert!(near(
            tightest_radius(Curve::Arc, 0.3),
            (0.25 + 0.09) / 0.6,
            1e-12
        ));
        let apex = tightest_radius(Curve::Cubic, KAPLAN_BULGE);
        assert!(apex > 0.27 && apex < 0.29, "{apex}");
        assert!(
            near(signed_area(&curved), TILE_AREA, 1e-3),
            "{}",
            signed_area(&curved)
        );
        assert!(curved[0].distance(vertex(0)) < 1e-12);
        assert!(!self_intersects(&curved));
    }

    #[test]
    fn patches_grow_as_kaplans_do_and_share_their_edges() {
        assert_eq!(tiles(0).len(), 1);
        assert_eq!(tiles(1).len(), 9);
        assert_eq!(tiles(2).len(), 71);
        let all = tiles(2);
        // One pair in each of the eight supertiles of a Delta.
        assert_eq!(
            all.iter().filter(|(m, _)| *m).count(),
            2 * 8,
            "mystic pairs"
        );
        let mut owners: HashMap<_, u32> = HashMap::new();
        let mut mids: HashMap<_, Vec2> = HashMap::new();
        let local = curved_tile(Curve::Cubic, KAPLAN_BULGE, 2);
        for (_, xf) in &all {
            assert!(near(det(xf), 1.0, 1e-9));
            let verts: Vec<Vec2> = (0..14).map(|i| apply(xf, vertex(i))).collect();
            for i in 0..14 {
                let k = edge_key(verts[i], verts[(i + 1) % 14]);
                *owners.entry(k).or_insert(0) += 1;
                // The curve's midpoint is the same from both tiles.
                let mid = apply(xf, local[2 * i + 1]);
                let prev = mids.insert(k, mid);
                assert!(prev.is_none_or(|q| q.distance(mid) < 1e-9), "edge {k:?}");
            }
        }
        assert!(owners.values().all(|&n| n <= 2));
        assert_eq!(owners.values().filter(|&&n| n == 2).count(), 406);
    }

    #[test]
    fn offsets_shrink_and_grow_a_square_with_the_right_corners() {
        let sq = vec![
            Vec2::ZERO,
            Vec2::new(10.0, 0.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(0.0, 10.0),
        ];
        let inner = offset(&sq, 1.0, 0.1);
        assert_eq!(inner.len(), 4);
        assert!(near(signed_area(&inner), 64.0, 1e-9));
        let outer = offset(&sq, -1.0, 10f64.to_radians());
        // Rounded corners: four quarter discs of radius 1.
        let expect = 100.0 + 4.0 * 10.0 + std::f64::consts::PI;
        assert!(
            near(signed_area(&outer), expect, 0.05),
            "{}",
            signed_area(&outer)
        );
        // An L: its reflex corner is rounded when shrinking.
        let l = vec![
            Vec2::ZERO,
            Vec2::new(10.0, 0.0),
            Vec2::new(10.0, 5.0),
            Vec2::new(5.0, 5.0),
            Vec2::new(5.0, 10.0),
            Vec2::new(0.0, 10.0),
        ];
        let inner = offset(&l, 1.0, 10f64.to_radians());
        assert!(inner.len() > 6);
        // Two 8 x 3 arms overlapping in a 3 x 3 square, plus the corner
        // the arc about the reflex vertex leaves outside the quarter disc.
        let expect = 24.0 + 24.0 - 9.0 + 1.0 - std::f64::consts::PI / 4.0;
        assert!(
            near(signed_area(&inner), expect, 0.05),
            "{} vs {expect}",
            signed_area(&inner)
        );
    }

    #[test]
    fn a_square_clips_to_a_board_in_the_parts_the_board_leaves() {
        let opts = ProfileOptions::default();
        // A U across the board's right edge: two parts.
        let u = vec![
            Vec2::new(80.0, 10.0),
            Vec2::new(120.0, 10.0),
            Vec2::new(120.0, 40.0),
            Vec2::new(80.0, 40.0),
            Vec2::new(80.0, 30.0),
            Vec2::new(110.0, 30.0),
            Vec2::new(110.0, 20.0),
            Vec2::new(80.0, 20.0),
        ];
        let parts = clip_polygon(&u, 100.0, 50.0, &opts);
        assert_eq!(parts.len(), 2, "{parts:?}");
        let mut areas: Vec<f64> = parts.iter().map(|p| signed_area(p)).collect();
        areas.sort_by(f64::total_cmp);
        assert!(
            near(areas[0], 200.0, 1e-9) && near(areas[1], 200.0, 1e-9),
            "{areas:?}"
        );
        // Over a corner: one part with the corner as a vertex.
        let sq = vec![
            Vec2::new(-5.0, -5.0),
            Vec2::new(5.0, -5.0),
            Vec2::new(5.0, 5.0),
            Vec2::new(-5.0, 5.0),
        ];
        let parts = clip_polygon(&sq, 100.0, 50.0, &opts);
        assert_eq!(parts.len(), 1);
        assert!(near(signed_area(&parts[0]), 25.0, 1e-9));
        assert!(parts[0].iter().any(|p| p.distance(Vec2::ZERO) < 1e-9));
        // Wholly inside: unchanged.
        let inside: Vec<Vec2> = sq.iter().map(|p| *p + Vec2::new(50.0, 25.0)).collect();
        let parts = clip_polygon(&inside, 100.0, 50.0, &opts);
        assert_eq!(parts.len(), 1);
        assert!(near(signed_area(&parts[0]), 100.0, 1e-9));
    }

    #[test]
    fn a_board_gets_its_pieces_and_a_lattice_that_fill_it() {
        let params = Params {
            width: 150.0,
            height: 100.0,
            edge: 12.7,
            gap: 1.5,
            curve: Curve::Cubic,
            bulge: KAPLAN_BULGE,
            bit: 0.0,
            shift: Vec2::ZERO,
            angle: 0.0,
            scrap: 0.0,
            scheme: Scheme::Mystics,
            seed: 0,
            rips: Vec::new(),
        };
        let facet = 5f64.to_radians();
        let patch = build(&params, facet).unwrap();
        assert_eq!(patch.depth, 2, "depth {}", patch.depth);
        let area = params.width * params.height;
        let mut pieces = 0.0;
        let mut dark = 0;
        for piece in &patch.pieces {
            let a = signed_area(&piece.outline);
            assert!(a > 0.0, "clockwise piece of tile {}", piece.tile);
            assert!(!self_intersects(&piece.outline));
            let (lo, hi) = bbox(&piece.outline);
            assert!(
                lo.x >= -1e-9
                    && lo.y >= -1e-9
                    && hi.x <= params.width + 1e-9
                    && hi.y <= params.height + 1e-9
            );
            pieces += a;
            dark += piece.mystic as usize;
        }
        let whole = TILE_AREA * params.edge * params.edge;
        assert!(
            patch.pieces.len() as f64 > area / whole,
            "{} pieces",
            patch.pieces.len()
        );
        assert!(
            dark > 0 && dark < patch.pieces.len() / 2,
            "{dark} dark of {}",
            patch.pieces.len()
        );
        let opts = ProfileOptions {
            arc_segment_angle: facet,
            merge_tolerance: 1e-6,
        };
        let gaps = lattice(&patch, &opts).unwrap();
        let gap_area: f64 = gaps.iter().map(|g| g.area()).sum();
        assert!(
            near(gap_area + pieces, area, 1e-6 * area),
            "{gap_area} + {pieces} vs {area}"
        );
        // The lattice is about the gap wide along every shared edge, and
        // the pockets the board's edge closes off are small beside it.
        assert!(
            gaps[0].area() > 0.05 * area && gaps[0].area() < 0.3 * area,
            "{}",
            gaps[0].area()
        );
        assert!(
            gaps[1..].iter().all(|g| g.area() < 0.1 * gaps[0].area()),
            "{:?}",
            gaps.iter().map(|g| g.area()).collect::<Vec<_>>()
        );
        // Each piece is one region of its own.
        for piece in &patch.pieces {
            piece_profile(piece, &opts).unwrap();
        }
        // No gap: the pieces alone fill the board.
        let tight = build(
            &Params {
                gap: 0.0,
                ..params.clone()
            },
            facet,
        )
        .unwrap();
        let sum: f64 = tight.pieces.iter().map(|p| signed_area(&p.outline)).sum();
        assert!(near(sum, area, 1e-6 * area), "{sum} vs {area}");
        // Shifting the tiling changes which tiles the board gets.
        let shifted = build(
            &Params {
                shift: Vec2::new(7.0, -4.0),
                ..params.clone()
            },
            facet,
        )
        .unwrap();
        assert_ne!(shifted.pieces.len(), 0);
        assert!(shifted.pieces[0].outline != patch.pieces[0].outline);
        // Too wide a gap is refused with the reason.
        let err = build(
            &Params {
                gap: 4.0,
                ..params.clone()
            },
            facet,
        )
        .unwrap_err();
        assert!(err.contains("quarter"), "{err}");
        // Slivers under the scrap area are left to the resin: fewer
        // pieces, none small, and the gaps take up what they covered.
        let smallest = patch
            .pieces
            .iter()
            .map(|p| signed_area(&p.outline))
            .fold(f64::INFINITY, f64::min);
        assert!(smallest < 60.0, "smallest {smallest}");
        let scrapped = build(
            &Params {
                scrap: 60.0,
                ..params.clone()
            },
            facet,
        )
        .unwrap();
        assert!(scrapped.pieces.len() < patch.pieces.len());
        assert!(scrapped
            .pieces
            .iter()
            .all(|p| signed_area(&p.outline) >= 60.0));
        let kept: f64 = scrapped
            .pieces
            .iter()
            .map(|p| signed_area(&p.outline))
            .sum();
        let gaps: f64 = lattice(&scrapped, &opts)
            .unwrap()
            .iter()
            .map(|g| g.area())
            .sum();
        assert!(
            near(kept + gaps, area, 1e-6 * area),
            "{kept} + {gaps} vs {area}"
        );
    }

    #[test]
    fn a_bit_fillets_the_inside_corners_and_rounds_their_mates_so_the_pieces_still_nest() {
        let facet = 10f64.to_radians();
        let params = Params {
            width: 101.6,
            height: 101.6,
            edge: 9.0,
            gap: 0.0,
            curve: Curve::Arc,
            bulge: 0.3,
            bit: 4.7625,
            shift: Vec2::ZERO,
            angle: 0.0,
            scrap: 0.0,
            scheme: Scheme::Mystics,
            seed: 0,
            rips: Vec::new(),
        };
        let patch = build(&params, facet).unwrap();
        let area = params.width * params.height;
        let sum: f64 = patch.pieces.iter().map(|p| signed_area(&p.outline)).sum();
        // What a fillet adds to one piece its mate's rounding takes off.
        assert!(near(sum, area, 1e-4 * area), "{sum} vs {area}");
        // No inside corner is sharper than the sampled fillet arcs, and
        // no outline doubles back on itself.
        for piece in &patch.pieces {
            assert!(!self_intersects(&piece.outline), "tile {}", piece.tile);
            let n = piece.outline.len();
            for i in 0..n {
                let (a, b, c) = (
                    piece.outline[(i + n - 1) % n],
                    piece.outline[i],
                    piece.outline[(i + 1) % n],
                );
                let (u, v) = (b - a, c - b);
                let turn = u.cross(v).atan2(u.dot(v)).to_degrees();
                assert!(
                    turn > -(facet.to_degrees() + 3.0),
                    "tile {}: turn {turn:.1} at {b:?}",
                    piece.tile
                );
            }
        }
        // The same board without the bit has sharp inside corners.
        let sharp = build(
            &Params {
                bit: 0.0,
                ..params.clone()
            },
            facet,
        )
        .unwrap();
        let sharpest = sharp
            .pieces
            .iter()
            .flat_map(|p| {
                let n = p.outline.len();
                (0..n).map(move |i| {
                    let (a, b, c) = (
                        p.outline[(i + n - 1) % n],
                        p.outline[i],
                        p.outline[(i + 1) % n],
                    );
                    (b - a).cross(c - b).atan2((b - a).dot(c - b)).to_degrees()
                })
            })
            .fold(0.0, f64::min);
        assert!(sharpest < -55.0, "{sharpest}");
        // Kaplan's curve at an 8 mm edge is tighter than the bit, and says so.
        let err = build(
            &Params {
                curve: Curve::Cubic,
                bulge: KAPLAN_BULGE,
                edge: 8.0,
                ..params.clone()
            },
            facet,
        )
        .unwrap_err();
        assert!(err.contains("tightest radius"), "{err}");
        // Turned under the board, the pieces still fill it exactly, and a
        // full turn is no turn.
        let turned = build(
            &Params {
                angle: 37.0,
                ..params.clone()
            },
            facet,
        )
        .unwrap();
        let sum: f64 = turned.pieces.iter().map(|p| signed_area(&p.outline)).sum();
        assert!(near(sum, area, 1e-4 * area), "{sum} vs {area}");
        assert!(turned.pieces[0].outline != patch.pieces[0].outline);
        let full = build(
            &Params {
                angle: 360.0,
                ..params.clone()
            },
            facet,
        )
        .unwrap();
        assert_eq!(full.pieces.len(), patch.pieces.len());
        assert!(full.pieces[0].outline[0].distance(patch.pieces[0].outline[0]) < 1e-9);
    }

    #[test]
    fn the_schemes_give_every_tile_a_wood_and_the_map_ones_keep_neighbours_apart() {
        let all = tiles(2);
        // Neighbours by shared edge, as the colouring sees them.
        let mut owners: HashMap<EdgeKey, Vec<usize>> = HashMap::new();
        for (k, (_, xf)) in all.iter().enumerate() {
            for i in 0..14 {
                owners
                    .entry(edge_key(apply(xf, vertex(i)), apply(xf, vertex(i + 1))))
                    .or_default()
                    .push(k);
            }
        }
        let pairs: Vec<(usize, usize)> = owners
            .values()
            .filter_map(|ks| match ks[..] {
                [a, b] => Some((a, b)),
                _ => None,
            })
            .collect();
        assert_eq!(pairs.len(), 406);
        let mystic = colour(&all, Scheme::Mystics, 0);
        assert!(all.iter().zip(&mystic).all(|((m, _), w)| *w == *m as u8));
        assert!(colour(&all, Scheme::One, 7).iter().all(|&w| w == 0));
        // A mystic pair is one tile and its partner turned 30°: the turn
        // scheme gives them different woods, and every other tile the
        // light one.
        let turn = colour(&all, Scheme::TwoTurn, 0);
        for &(a, b) in &pairs {
            if all[a].0 && all[b].0 {
                assert_ne!(turn[a], turn[b], "pair {a} {b}");
            }
        }
        assert_eq!(
            turn.iter().filter(|&&w| w == 1).count(),
            all.iter().filter(|(m, _)| *m).count() / 2
        );
        for (scheme, woods) in [(Scheme::TwoRandom, 2), (Scheme::ThreeRandom, 3)] {
            let w = colour(&all, scheme, 3);
            assert!(w.iter().all(|&x| x < woods));
            assert!(
                (0..woods).all(|x| w.contains(&x)),
                "{scheme:?} uses every wood"
            );
            assert_eq!(w, colour(&all, scheme, 3), "the seed repeats");
            assert_ne!(w, colour(&all, scheme, 4), "another seed differs");
        }
        for scheme in [Scheme::FourMap, Scheme::Mix] {
            let w = colour(&all, scheme, 1);
            for &(a, b) in &pairs {
                assert_ne!(
                    w[a], w[b],
                    "{scheme:?}: tiles {a} and {b} share an edge and a wood"
                );
            }
            let counts: Vec<usize> = (0..4u8)
                .map(|x| w.iter().filter(|&&y| y == x).count())
                .collect();
            assert!(counts.iter().all(|&c| c > 0), "{scheme:?}: {counts:?}");
            if scheme == Scheme::Mix {
                assert!(
                    counts[0] > counts[3],
                    "the mix favours the light wood: {counts:?}"
                );
            } else {
                assert!(
                    counts[0] < 2 * counts[3],
                    "the map spreads the woods: {counts:?}"
                );
            }
        }
        // The pieces carry their tile's wood.
        let patch = build(
            &Params {
                width: 150.0,
                height: 100.0,
                edge: 12.7,
                gap: 1.5,
                curve: Curve::Cubic,
                bulge: KAPLAN_BULGE,
                bit: 0.0,
                shift: Vec2::ZERO,
                angle: 0.0,
                scrap: 0.0,
                scheme: Scheme::FourMap,
                seed: 1,
                rips: Vec::new(),
            },
            5f64.to_radians(),
        )
        .unwrap();
        let w = colour(&tiles(patch.depth), Scheme::FourMap, 1);
        assert!(patch.pieces.iter().all(|p| p.wood == w[p.tile as usize]));
    }

    #[test]
    fn rips_split_every_straight_tile_into_its_profiles() {
        let plain = Params {
            width: 150.0,
            height: 100.0,
            edge: 12.7,
            gap: 1.5,
            curve: Curve::Cubic,
            bulge: 0.0,
            bit: 0.0,
            shift: Vec2::ZERO,
            angle: 20.0,
            scrap: 0.0,
            scheme: Scheme::Profiles,
            seed: 0,
            rips: Vec::new(),
        };
        let facet = 10f64.to_radians();
        let whole = build(&plain, facet).unwrap();
        let best = rips::ranked().kit[0].clone();
        let ripped = build(
            &Params {
                rips: best.cuts.clone(),
                ..plain.clone()
            },
            facet,
        )
        .unwrap();
        // Six profiles per tile; the same wood as a whole.
        assert!(
            ripped.pieces.len() > 3 * whole.pieces.len(),
            "{} from {}",
            ripped.pieces.len(),
            whole.pieces.len()
        );
        let a: f64 = whole.pieces.iter().map(|p| signed_area(&p.outline)).sum();
        let b: f64 = ripped.pieces.iter().map(|p| signed_area(&p.outline)).sum();
        assert!((a - b).abs() < 1e-6 * a, "{a} vs {b}");
        assert!(ripped.pieces.iter().all(|p| p.profile.is_some()));
        // Two profiles, the first (the larger) light and the second dark.
        let profiles: std::collections::BTreeSet<u8> =
            ripped.pieces.iter().filter_map(|p| p.profile).collect();
        assert_eq!(profiles.len(), 2);
        assert!(ripped.pieces.iter().all(|p| p.wood == p.profile.unwrap()));
        let whole_tiles = ripped
            .pieces
            .iter()
            .filter(|p| p.outline.len() == 4)
            .count();
        assert!(whole_tiles > 0);
        // The curved tile cannot be ripped, nor can a cut off the grid.
        let err = build(
            &Params {
                bulge: KAPLAN_BULGE,
                rips: best.cuts.clone(),
                ..plain.clone()
            },
            facet,
        )
        .unwrap_err();
        assert!(err.contains("straight"), "{err}");
        let err = build(
            &Params {
                rips: vec![[vertex(8), vertex(4)]],
                ..plain.clone()
            },
            facet,
        )
        .unwrap_err();
        assert!(err.contains("30°"), "{err}");
    }
}
