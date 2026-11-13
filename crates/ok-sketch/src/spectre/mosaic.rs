//! The hex-edged tile as a bundle of lattice triangles.
//!
//! With half a hexagon on every edge (`Curve::Hex`) every corner of the
//! tile lies on a triangular lattice of side half an edge, so the tile
//! can be built from sawn triangular sticks and no curve is routed: a
//! stick per piece, the sticks bundled end-on in a comb at the pattern,
//! resin poured through the bundle, and the bundle sliced into lids.
//! The tile's fourteen edges fall into two families, eight on the 60°
//! lattice (directions 0°, 60°, 120°) and six on the 30° one, in three
//! runs each, and the runs do not share an offset: a 30° edge moves the
//! 60° lattice by a fraction of a step. So no one lattice covers the
//! tile, and this module covers as much of it as it can from the six
//! lattices the runs define, with four shapes of stick: the lattice
//! triangle (equilateral, side 1/2), its half (30-60-90, sides 1/4, √3/4,
//! 1/2), the half of the quarter-size triangle (sides 1/8, √3/8, 1/4),
//! and the trapezoid a half triangle leaves when that small half is
//! taken off its apex. What the shapes leave, a fraction of a percent
//! in slivers along the seams between lattices, the resin fills.
//!
//! The cover is a search: every placement of every shape on every
//! lattice that lies inside the tile is a candidate; a greedy pass
//! takes the biggest first, boundary first; then ruin-and-recreate
//! (clear a disc, refill it in a jittered order, keep the result if it
//! covers no less) for a fixed number of rounds from a fixed seed, so
//! the answer is the same every time; then every two halves that make
//! a whole lattice triangle become one.

use super::{curved_tile, vertex, Curve};
use crate::{point_in_polygon, signed_area};
use ok_math::Vec2;

const SQRT3: f64 = 1.732_050_807_568_877_2;
/// The lattice's step: half an edge.
const STEP: f64 = 0.5;
/// How far a candidate's vertices are pulled in toward its centroid for
/// the inside test, so a piece on the boundary counts as inside.
const PULL: f64 = 1e-6;

/// The four sticks, as fractions of the tile's edge.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    /// Equilateral, side 1/2.
    Triangle,
    /// Half of it: 30-60-90, sides 1/4, √3/4 and 1/2.
    Half,
    /// Half of the quarter-size triangle: sides 1/8, √3/8 and 1/4.
    Small,
    /// A half with the small half off its apex: parallel sides 1/4 and
    /// 1/8, √3/8 apart, one slant of 1/4.
    Trapezoid,
    /// The quarter-size triangle: equilateral, side 1/4, the lattice
    /// triangle with its sides' midpoints joined. Not in the default
    /// set; the same saw setting as the triangle at half the width.
    Quarter,
}

impl Shape {
    pub const ALL: [Shape; 5] = [
        Shape::Triangle,
        Shape::Half,
        Shape::Small,
        Shape::Trapezoid,
        Shape::Quarter,
    ];
    /// The set the search uses unless told otherwise.
    pub const DEFAULT: [Shape; 4] = [Shape::Triangle, Shape::Half, Shape::Small, Shape::Trapezoid];

    pub fn name(self) -> &'static str {
        match self {
            Shape::Triangle => "triangle",
            Shape::Half => "half",
            Shape::Small => "small",
            Shape::Trapezoid => "trapezoid",
            Shape::Quarter => "quarter",
        }
    }

    /// Area in edge units squared.
    pub fn area(self) -> f64 {
        let t = SQRT3 / 16.0;
        match self {
            Shape::Triangle => t,
            Shape::Half => t / 2.0,
            Shape::Small => t / 8.0,
            Shape::Trapezoid => t * 3.0 / 8.0,
            Shape::Quarter => t / 4.0,
        }
    }

    /// The order the search places them in: biggest first.
    fn rank(self) -> usize {
        match self {
            Shape::Triangle => 0,
            Shape::Half => 1,
            Shape::Trapezoid => 2,
            Shape::Quarter => 3,
            Shape::Small => 4,
        }
    }
}

/// One stick's place in the tile.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Piece {
    pub shape: Shape,
    /// Counter-clockwise, in the tile's frame, unit edges.
    pub outline: Vec<Vec2>,
}

/// The cover of one tile.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Mosaic {
    pub pieces: Vec<Piece>,
    /// The tile's area the pieces cover, as a fraction.
    pub coverage: f64,
    /// Pieces of each shape, in `Shape::ALL`'s order.
    pub counts: [usize; 5],
}

/// The hex tile in its own frame, 42 corners, counter-clockwise.
pub fn tile() -> Vec<Vec2> {
    curved_tile(Curve::Hex, 0.0, 3)
}

// -- geometry -------------------------------------------------------------

fn cross(a: Vec2, b: Vec2) -> f64 {
    a.x * b.y - a.y * b.x
}

fn centroid(p: &[Vec2]) -> Vec2 {
    p.iter().fold(Vec2::ZERO, |s, q| s + *q) * (1.0 / p.len() as f64)
}

fn bbox(p: &[Vec2]) -> (Vec2, Vec2) {
    let mut lo = Vec2::new(f64::INFINITY, f64::INFINITY);
    let mut hi = Vec2::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
    for q in p {
        lo = Vec2::new(lo.x.min(q.x), lo.y.min(q.y));
        hi = Vec2::new(hi.x.max(q.x), hi.y.max(q.y));
    }
    (lo, hi)
}

fn boxes_apart(a: &(Vec2, Vec2), b: &(Vec2, Vec2)) -> bool {
    a.0.x >= b.1.x - 1e-9 || b.0.x >= a.1.x - 1e-9 || a.0.y >= b.1.y - 1e-9 || b.0.y >= a.1.y - 1e-9
}

/// The part of convex `poly` on the left of the line a → b.
fn clip_left(poly: &[Vec2], a: Vec2, b: Vec2) -> Vec<Vec2> {
    let mut out = Vec::with_capacity(poly.len() + 2);
    let n = poly.len();
    for i in 0..n {
        let (p, q) = (poly[i], poly[(i + 1) % n]);
        let (sp, sq) = (cross(b - a, p - a), cross(b - a, q - a));
        if sp >= -1e-12 {
            out.push(p);
        }
        if (sp > 1e-12 && sq < -1e-12) || (sp < -1e-12 && sq > 1e-12) {
            let t = sp / (sp - sq);
            out.push(p + (q - p) * t);
        }
    }
    out
}

/// Area of the overlap of two convex counter-clockwise polygons.
fn overlap(p: &[Vec2], q: &[Vec2]) -> f64 {
    let mut r = p.to_vec();
    for i in 0..q.len() {
        r = clip_left(&r, q[i], q[(i + 1) % q.len()]);
        if r.len() < 3 {
            return 0.0;
        }
    }
    signed_area(&r).abs()
}

fn segments_cross(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> bool {
    let (d1, d2) = (cross(b - a, c - a), cross(b - a, d - a));
    let (d3, d4) = (cross(d - c, a - c), cross(d - c, b - c));
    let s = |x: f64, y: f64| (x > 1e-9 && y < -1e-9) || (x < -1e-9 && y > 1e-9);
    s(d1, d2) && s(d3, d4)
}

/// Whether a convex piece lies inside the tile (its boundary allowed).
fn inside(piece: &[Vec2], tile: &[Vec2]) -> bool {
    let c = centroid(piece);
    if !point_in_polygon(c, tile) {
        return false;
    }
    let pulled: Vec<Vec2> = piece.iter().map(|p| c + (*p - c) * (1.0 - PULL)).collect();
    if pulled.iter().any(|p| !point_in_polygon(*p, tile)) {
        return false;
    }
    let n = tile.len();
    for i in 0..pulled.len() {
        let (a, b) = (pulled[i], pulled[(i + 1) % pulled.len()]);
        for j in 0..n {
            if segments_cross(a, b, tile[j], tile[(j + 1) % n]) {
                return false;
            }
        }
    }
    true
}

/// Whether a piece has a vertex on the tile's outline.
/// Where a point `p` on the tile's outline, a corner of `stick` with
/// neither side there, sends the stick: lines to clip it by, as a → b
/// with the inside on the left, so that it keeps `d` from the outline
/// like the eroded tile (`offset`, which is the pocket it goes in).
/// Beside a segment the stick keeps off its offset line; a stick at a
/// corner of the tile that lies alongside one of the corner's segments
/// (every corner of the stick projects onto it) is clipped by that
/// segment's line, which is exact there and clears the corner too. A
/// stick out past both segments' ends, in the fan of a reflex corner,
/// faces the eroded tile's arc about the corner: a flat tangent to it,
/// square to the stick's own corner, takes the point off.
fn point_cuts(p: Vec2, tile: &[Vec2], d: f64, stick: &[Vec2]) -> Vec<(Vec2, Vec2)> {
    let n = tile.len();
    let line = |e: Vec2| (p + e.perp() * d, p + e.perp() * d + e);
    for j in 0..n {
        if tile[j].distance(p) < 1e-6 {
            let e0 = (tile[j] - tile[(j + n - 1) % n])
                .normalized()
                .unwrap_or(Vec2::X);
            let e1 = (tile[(j + 1) % n] - tile[j])
                .normalized()
                .unwrap_or(Vec2::X);
            // Along the segment before the corner: (q - p) · e0 <= 0;
            // along the one after: (q - p) · e1 >= 0.
            let before = stick.iter().all(|q| (*q - p).dot(e0) <= 1e-6);
            let after = stick.iter().all(|q| (*q - p).dot(e1) >= -1e-6);
            let mut cuts = Vec::new();
            if before {
                cuts.push(line(e0));
            }
            if after {
                cuts.push(line(e1));
            }
            if cuts.is_empty() {
                let k = stick.len();
                let i = (0..k).find(|&i| stick[i].distance(p) < 1e-9).unwrap_or(0);
                let u = (stick[(i + 1) % k] - p).normalized().unwrap_or(Vec2::X);
                let v = (stick[(i + k - 1) % k] - p).normalized().unwrap_or(Vec2::X);
                let b = (u + v).normalized().unwrap_or(u);
                cuts.push((p + b * d, p + b * d - b.perp()));
                // The flat clears both segments only while the stick's
                // corner opens away from them; where it leans over one,
                // that segment's line keeps it off.
                if b.dot(e0) < -1e-9 {
                    cuts.push(line(e0));
                }
                if b.dot(e1) > 1e-9 {
                    cuts.push(line(e1));
                }
            }
            return cuts;
        }
    }
    for j in 0..n {
        let (a, b) = (tile[j], tile[(j + 1) % n]);
        if touches(&[p], &[a, b]) {
            return vec![line((b - a).normalized().unwrap_or(Vec2::X))];
        }
    }
    Vec::new()
}

fn touches(piece: &[Vec2], tile: &[Vec2]) -> bool {
    let n = tile.len();
    piece.iter().any(|p| {
        (0..n).any(|j| {
            let (a, b) = (tile[j], tile[(j + 1) % n]);
            let ab = b - a;
            // To the segment, ends included: a point at its end comes
            // out a rounding error past it as often as not.
            let t = ((*p - a).dot(ab) / ab.dot(ab)).clamp(0.0, 1.0);
            (a + ab * t).distance(*p) < 1e-6
        })
    })
}

/// The length of a piece's sides that lie along the tile's outline:
/// what a caul's pocket wall presses on.
fn on_outline(piece: &[Vec2], tile: &[Vec2]) -> f64 {
    let n = tile.len();
    let on = |p: Vec2| {
        (0..n).any(|j| {
            let (a, b) = (tile[j], tile[(j + 1) % n]);
            let ab = b - a;
            let t = (p - a).dot(ab) / ab.dot(ab);
            (0.0..=1.0).contains(&t) && (a + ab * t).distance(p) < 1e-6
        })
    };
    (0..piece.len())
        .map(|i| {
            let (a, b) = (piece[i], piece[(i + 1) % piece.len()]);
            if on(a) && on(b) && on((a + b) * 0.5) {
                a.distance(b)
            } else {
                0.0
            }
        })
        .sum()
}

fn ccw(mut p: Vec<Vec2>) -> Vec<Vec2> {
    if signed_area(&p) < 0.0 {
        p.reverse();
    }
    p
}

// -- candidates ------------------------------------------------------------

/// The six lattices: a family (0 for 0°, 60°, 120°; 1 for 30°, 90°,
/// 150°) and an origin, one per run of the tile's edges in a family.
fn lattices() -> Vec<(u8, Vec2)> {
    let family = |i: usize| {
        let d = vertex(i + 1) - vertex(i);
        ((d.y.atan2(d.x).to_degrees() / 30.0).round() as i64).rem_euclid(2) as u8
    };
    let mut out = Vec::new();
    let mut i = 0;
    while i < 14 {
        let f = family(i);
        out.push((f, vertex(i)));
        while i < 14 && family(i) == f {
            i += 1;
        }
    }
    out
}

/// Every placement of every shape in `shapes` on every lattice inside the tile.
fn candidates(tile: &[Vec2], shapes: &[Shape]) -> Vec<Piece> {
    let (lo, hi) = bbox(tile);
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    let mut push = |shape: Shape, poly: Vec<Vec2>, out: &mut Vec<Piece>| {
        if !shapes.contains(&shape) {
            return;
        }
        let poly = ccw(poly);
        if !inside(&poly, tile) {
            return;
        }
        let mut key: Vec<(i64, i64)> = poly
            .iter()
            .map(|p| ((p.x * 1e6).round() as i64, (p.y * 1e6).round() as i64))
            .collect();
        key.sort();
        if seen.insert((shape, key)) {
            out.push(Piece {
                shape,
                outline: poly,
            });
        }
    };
    for (f, o) in lattices() {
        let ang = if f == 1 { 30f64 } else { 0.0 }.to_radians();
        let e1 = Vec2::from_angle(ang) * STEP;
        let e2 = Vec2::from_angle(ang + 60f64.to_radians()) * STEP;
        for m in -12..14 {
            for n in -12..14 {
                let p = o + e1 * m as f64 + e2 * n as f64;
                if p.x < lo.x - 1.0 || p.x > hi.x + 1.0 || p.y < lo.y - 1.0 || p.y > hi.y + 1.0 {
                    continue;
                }
                let up = [p, p + e1, p + e2];
                let down = [p + e1, p + e1 + e2, p + e2];
                for t in [up, down] {
                    push(Shape::Triangle, t.to_vec(), &mut out);
                    // The halves: each vertex to the opposite side's middle.
                    for k in 0..3 {
                        let (a, b, c) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
                        let m = (b + c) * 0.5;
                        for h in [[a, b, m], [a, m, c]] {
                            push(Shape::Half, h.to_vec(), &mut out);
                            // Across the half at mid height: the small half at
                            // the apex, the trapezoid at the base.
                            let (p1, p2) = ((a + h[1]) * 0.5, (a + h[2]) * 0.5);
                            push(Shape::Small, vec![a, p1, p2], &mut out);
                            push(Shape::Trapezoid, vec![p1, h[1], h[2], p2], &mut out);
                        }
                    }
                }
                // The quarter triangles on the half-step lattice, and the
                // small halves, wherever they fit.
                let (h1, h2) = (e1 * 0.5, e2 * 0.5);
                for q in [p, p + h1, p + h2, p + h1 + h2] {
                    for t in [[q, q + h1, q + h2], [q + h1, q + h1 + h2, q + h2]] {
                        push(Shape::Quarter, t.to_vec(), &mut out);
                        for k in 0..3 {
                            let (a, b, c) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
                            let m = (b + c) * 0.5;
                            push(Shape::Small, vec![a, b, m], &mut out);
                            push(Shape::Small, vec![a, m, c], &mut out);
                        }
                    }
                }
            }
        }
    }
    out
}

// -- the search -----------------------------------------------------------

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// The cover, from `rounds` of ruin-and-recreate after the greedy
/// start, seeded so the answer repeats. A thousand rounds reach 99.5 %
/// of the tile; the first hundred most of the way there.
pub fn mosaic(rounds: u32, seed: u64) -> Mosaic {
    mosaic_with(&Shape::DEFAULT, rounds, seed)
}

/// The cover from a chosen set of shapes.
pub fn mosaic_with(shapes: &[Shape], rounds: u32, seed: u64) -> Mosaic {
    let tile = tile();
    let cands = candidates(&tile, shapes);
    let n = cands.len();
    let areas: Vec<f64> = cands.iter().map(|c| signed_area(&c.outline)).collect();
    let boxes: Vec<(Vec2, Vec2)> = cands.iter().map(|c| bbox(&c.outline)).collect();
    let centres: Vec<Vec2> = cands.iter().map(|c| centroid(&c.outline)).collect();
    let edge_touch: Vec<bool> = cands.iter().map(|c| touches(&c.outline, &tile)).collect();
    let contact: Vec<f64> = cands
        .iter()
        .map(|c| on_outline(&c.outline, &tile))
        .collect();
    // Who overlaps whom, once.
    let mut conflicts: Vec<Vec<u32>> = vec![Vec::new(); n];
    for i in 0..n {
        for j in i + 1..n {
            if boxes_apart(&boxes[i], &boxes[j]) {
                continue;
            }
            if overlap(&cands[i].outline, &cands[j].outline) > 1e-9 {
                conflicts[i].push(j as u32);
                conflicts[j].push(i as u32);
            }
        }
    }
    let free = |i: usize, chosen: &[bool]| conflicts[i].iter().all(|&j| !chosen[j as usize]);
    let total = |chosen: &[bool]| -> f64 {
        chosen
            .iter()
            .zip(&areas)
            .filter(|(c, _)| **c)
            .map(|(_, a)| a)
            .sum()
    };
    // Greedy: biggest first, the boundary before the middle, left to right.
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| {
        (cands[a].shape.rank(), !edge_touch[a])
            .cmp(&(cands[b].shape.rank(), !edge_touch[b]))
            .then(centres[a].x.total_cmp(&centres[b].x))
            .then(centres[a].y.total_cmp(&centres[b].y))
    });
    let mut chosen = vec![false; n];
    for &i in &order {
        if free(i, &chosen) {
            chosen[i] = true;
        }
    }
    let mut best = chosen.clone();
    // Among covers of one area, the one with more stick sides along the
    // outline wins: a side on the outline is a face the caul presses.
    let along = |chosen: &[bool]| -> f64 {
        chosen
            .iter()
            .zip(&contact)
            .filter(|(c, _)| **c)
            .map(|(_, l)| l)
            .sum()
    };
    let mut best_area = total(&best);
    let mut best_along = along(&best);
    let mut cur_area = best_area;
    let mut cur_along = best_along;
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    for round in 0..rounds {
        let radius = [0.35, 0.5, 0.7][(round % 3) as usize];
        let picked: Vec<usize> = (0..n).filter(|&i| chosen[i]).collect();
        if picked.is_empty() {
            break;
        }
        let c = centres[picked[rng.below(picked.len())]];
        let mut trial = chosen.clone();
        for &i in &picked {
            if centres[i].distance(c) < radius {
                trial[i] = false;
            }
        }
        let mut near: Vec<(f64, usize)> = (0..n)
            .filter(|&i| !trial[i] && centres[i].distance(c) < radius + 0.5)
            .map(|i| (cands[i].shape.rank() as f64 + rng.unit() * 1.2, i))
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, i) in near {
            if free(i, &trial) {
                trial[i] = true;
            }
        }
        let a = total(&trial);
        let l = along(&trial);
        if a > cur_area + 1e-9 || (a >= cur_area - 1e-12 && l >= cur_along - 1e-9) {
            chosen = trial;
            cur_area = a;
            cur_along = l;
            if a > best_area + 1e-9 || ((a - best_area).abs() <= 1e-9 && l > best_along + 1e-9) {
                best_area = a;
                best_along = l;
                best = chosen.clone();
            }
        }
    }
    // Two halves that make a lattice triangle are one stick.
    for i in 0..n {
        if cands[i].shape != Shape::Triangle || best[i] {
            continue;
        }
        let halves: Vec<usize> = conflicts[i]
            .iter()
            .map(|&j| j as usize)
            .filter(|&j| best[j] && cands[j].shape == Shape::Half)
            .filter(|&j| overlap(&cands[i].outline, &cands[j].outline) > areas[j] - 1e-9)
            .collect();
        if halves.len() == 2 && free_but(i, &halves, &conflicts, &best) {
            for j in halves {
                best[j] = false;
            }
            best[i] = true;
        }
    }
    let pieces: Vec<Piece> = (0..n)
        .filter(|&i| best[i])
        .map(|i| cands[i].clone())
        .collect();
    let mut counts = [0usize; 5];
    for p in &pieces {
        counts[Shape::ALL.iter().position(|s| *s == p.shape).unwrap()] += 1;
    }
    let coverage = total(&best) / signed_area(&tile);
    Mosaic {
        pieces,
        coverage,
        counts,
    }
}

/// Whether candidate `i` conflicts with nothing chosen but `except`.
fn free_but(i: usize, except: &[usize], conflicts: &[Vec<u32>], chosen: &[bool]) -> bool {
    conflicts[i]
        .iter()
        .all(|&j| !chosen[j as usize] || except.contains(&(j as usize)))
}

// -- a lid's worth --------------------------------------------------------

/// A stick's place on a lid.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Laid {
    /// The tile, by its index in the patch.
    pub tile: u32,
    /// The tile's wood slot.
    pub wood: u8,
    pub shape: Shape,
    /// Counter-clockwise, mm, in the board's frame, the gaps taken off.
    pub outline: Vec<Vec2>,
}

/// Every tile's pieces laid on a board: the patch's whole tiles, each
/// carrying the mosaic through its frame, every piece pulled in by half
/// the `inner` gap on its sides and half the patch's gap where its side
/// is the tile's edge, and dropped when what is left is narrower than
/// `min` (all in mm).
pub fn lay(patch: &super::Patch, mosaic: &Mosaic, gap: f64, inner: f64, min: f64) -> Vec<Laid> {
    let tile = tile();
    let mut out = Vec::new();
    let mut done = std::collections::HashSet::new();
    for piece in &patch.pieces {
        if !done.insert(piece.tile) {
            continue;
        }
        let xf = &patch.frames[piece.tile as usize];
        let edge_line: Vec<Vec2> = tile.iter().map(|q| super::apply(xf, *q)).collect();
        let edge_ccw = ccw(edge_line.clone());
        for m in &mosaic.pieces {
            let poly = ccw(m.outline.iter().map(|q| super::apply(xf, *q)).collect());
            let k = poly.len();
            let on_edge: Vec<bool> = (0..k)
                .map(|i| {
                    let (a, b) = (poly[i], poly[(i + 1) % k]);
                    touches(&[a], &edge_line)
                        && touches(&[b], &edge_line)
                        && touches(&[(a + b) * 0.5], &edge_line)
                })
                .collect();
            let mut cut = poly.clone();
            for i in 0..k {
                let (a, b) = (poly[i], poly[(i + 1) % k]);
                let d = if on_edge[i] { gap / 2.0 } else { inner / 2.0 };
                // The side moved in by d: the left of a → b is the inside.
                let n = (b - a).perp().normalized().unwrap_or(Vec2::ZERO) * d;
                cut = clip_left(&cut, a + n, b + n);
                if cut.len() < 3 {
                    break;
                }
            }
            // A corner on the tile's edge: a point on the line, or the end
            // of a side on it at a corner of the tile. Pulled in by its
            // sides alone it can stand proud of the eroded tile, into the
            // line between tiles (a point by the inner line's worth, the
            // end of a side into the tile's other segment), so it keeps
            // off the eroded tile as well.
            for i in 0..k {
                if cut.len() < 3 || !touches(&[poly[i]], &edge_line) {
                    continue;
                }
                for (a, b) in point_cuts(poly[i], &edge_ccw, gap / 2.0, &poly) {
                    cut = clip_left(&cut, a, b);
                }
            }
            if cut.len() < 3 || super::saw::least_width(&cut) < min {
                continue;
            }
            out.push(Laid {
                tile: piece.tile,
                wood: piece.wood,
                shape: m.shape,
                outline: cut,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hex_tile_closes_on_the_lattice_with_the_straight_tiles_area() {
        let t = tile();
        assert_eq!(t.len(), 42);
        assert!((signed_area(&t) - super::super::TILE_AREA).abs() < 1e-9);
        assert!(!super::super::self_intersects(&t));
        // Every piece is half an edge long.
        for i in 0..42 {
            let d = t[(i + 1) % 42].distance(t[i]);
            assert!((d - 0.5).abs() < 1e-9, "piece {i}: {d}");
        }
        assert_eq!(lattices().len(), 6);
    }

    /// A stick whose point is on the tile's edge, at a reflex corner,
    /// gets its point taken off at the eroded tile: pulling its sides in
    /// by the inner line alone would leave the point in the line.
    #[test]
    fn a_point_on_the_edge_is_taken_off_at_the_eroded_tile() {
        let p = super::super::Params {
            width: 60.0,
            height: 60.0,
            edge: 10.0,
            gap: 1.0,
            curve: Curve::Hex,
            bulge: super::super::HEX_BULGE,
            bit: 0.0,
            shift: Vec2::new(2.0, -3.0),
            angle: 17.0,
            scrap: 0.0,
            scheme: Default::default(),
            seed: 0,
            rips: Vec::new(),
            whole: true,
            prisms: Vec::new(),
        };
        let patch = super::super::build(&p, 10f64.to_radians()).unwrap();
        let t = ccw(tile());
        let n = t.len();
        // A reflex corner of the tile, and a triangle with its point there
        // along the corner's bisector, inside the tile.
        let j = (0..n)
            .find(|&j| {
                let e0 = t[j] - t[(j + n - 1) % n];
                let e1 = t[(j + 1) % n] - t[j];
                e0.cross(e1) < -1e-9
            })
            .expect("a reflex corner");
        let e0 = (t[j] - t[(j + n - 1) % n]).normalized().unwrap();
        let e1 = (t[(j + 1) % n] - t[j]).normalized().unwrap();
        let axis = (e0.perp() + e1.perp()).normalized().unwrap();
        let v = t[j];
        let tri = vec![
            v,
            v + Vec2::from_angle(axis.angle() - 30f64.to_radians()) * 0.5,
            v + Vec2::from_angle(axis.angle() + 30f64.to_radians()) * 0.5,
        ];
        assert!(tri.iter().skip(1).all(|q| point_in_polygon(*q, &t)));
        let m = Mosaic {
            pieces: vec![Piece {
                shape: Shape::Triangle,
                outline: tri,
            }],
            coverage: 0.0,
            counts: [0; 5],
        };
        let (gap, inner) = (0.4, 0.1);
        let laid = lay(&patch, &m, gap, inner, 0.0);
        assert!(!laid.is_empty());
        for l in &laid {
            let framed = ccw(tile()
                .iter()
                .map(|q| super::super::apply(&patch.frames[l.tile as usize], *q))
                .collect());
            // Four corners: the point became a flat, and every corner is
            // at least half the gap from the outline.
            assert_eq!(l.outline.len(), 4, "{:?}", l.outline);
            for q in &l.outline {
                let d = (0..framed.len())
                    .map(|i| {
                        let (a, b) = (framed[i], framed[(i + 1) % framed.len()]);
                        let ab = b - a;
                        let t = ((*q - a).dot(ab) / ab.dot(ab)).clamp(0.0, 1.0);
                        (a + ab * t).distance(*q)
                    })
                    .fold(f64::INFINITY, f64::min);
                assert!(d >= gap / 2.0 - 1e-9, "{d}");
            }
            // The flat: the eroded sides' point would be inner/2 / sin 30°
            // along the axis, the chord is at gap/2, and the cap lost is
            // a 60° triangle of that height, in the frame's scale.
            let scale = (framed[1] - framed[0]).length() / (t[1] - t[0]).length();
            let h = (gap / 2.0 - inner) / scale;
            let cap = h * h * 30f64.to_radians().tan();
            let side = 0.5 - inner / scale * 3f64.sqrt();
            let expect = (side * side * 3f64.sqrt() / 4.0 - cap) * scale * scale;
            assert!(
                (signed_area(&l.outline) - expect).abs() < 1e-6,
                "{} vs {expect}",
                signed_area(&l.outline)
            );
        }
    }

    /// A patch's frames map the tile onto its piece: the eroded piece
    /// sits inside the framed tile, and the laid sticks inside the piece
    /// (gap and inner alike here), on a board kept whole and not.
    #[test]
    fn the_frames_put_the_tile_on_its_piece_and_the_sticks_inside_it() {
        for whole in [false, true] {
            let p = super::super::Params {
                width: 60.0,
                height: 60.0,
                edge: 10.0,
                gap: 1.0,
                curve: Curve::Hex,
                bulge: super::super::HEX_BULGE,
                bit: 0.0,
                shift: Vec2::new(2.0, -3.0),
                angle: 17.0,
                scrap: 0.0,
                scheme: Default::default(),
                seed: 0,
                rips: Vec::new(),
                whole,
                prisms: Vec::new(),
            };
            let patch = super::super::build(&p, 10f64.to_radians()).unwrap();
            assert!(patch.frames.len() > patch.tiles);
            let t = tile();
            for piece in &patch.pieces {
                let framed: Vec<Vec2> = t
                    .iter()
                    .map(|q| super::super::apply(&patch.frames[piece.tile as usize], *q))
                    .collect();
                let framed = ccw(framed);
                let c = centroid(&piece.outline);
                assert!(
                    point_in_polygon(c, &framed),
                    "whole {whole}: tile {} is not where its piece is",
                    piece.tile
                );
                // The piece is the tile eroded by half the gap: its corners
                // are all inside the framed tile.
                for q in &piece.outline {
                    assert!(point_in_polygon(*q, &framed));
                }
            }
            let m = mosaic(20, 1);
            let (gap, inner) = (0.4, 0.1);
            let laid = lay(&patch, &m, gap, inner, 0.0);
            let tiles = patch
                .pieces
                .iter()
                .map(|p| p.tile)
                .collect::<std::collections::HashSet<_>>();
            assert_eq!(laid.len(), m.pieces.len() * tiles.len());
            for l in &laid {
                let framed: Vec<Vec2> = t
                    .iter()
                    .map(|q| super::super::apply(&patch.frames[l.tile as usize], *q))
                    .collect();
                let framed = ccw(framed);
                // Inside the tile, and half the gap from its outline at
                // every corner: a side on the outline, a side partly on
                // it, a point on it, at a corner of the tile or not.
                for q in &l.outline {
                    assert!(
                        point_in_polygon(*q, &framed),
                        "whole {whole}: a stick outside its tile"
                    );
                    let d = (0..framed.len())
                        .map(|i| {
                            let (a, b) = (framed[i], framed[(i + 1) % framed.len()]);
                            let ab = b - a;
                            let t = ((*q - a).dot(ab) / ab.dot(ab)).clamp(0.0, 1.0);
                            (a + ab * t).distance(*q)
                        })
                        .fold(f64::INFINITY, f64::min);
                    assert!(
                        d >= gap / 2.0 - 1e-6,
                        "whole {whole}: a {:?} stick {d} from the outline at {q:?}",
                        l.shape
                    );
                }
            }
        }
    }

    #[test]
    fn the_shapes_have_their_sides() {
        let c = candidates(&tile(), &Shape::ALL);
        let sides = |p: &[Vec2]| -> Vec<f64> {
            let mut s: Vec<f64> = (0..p.len())
                .map(|i| p[(i + 1) % p.len()].distance(p[i]))
                .collect();
            s.sort_by(|a, b| a.total_cmp(b));
            s
        };
        for p in &c {
            let s = sides(&p.outline);
            let want: Vec<f64> = match p.shape {
                Shape::Triangle => vec![0.5, 0.5, 0.5],
                Shape::Half => vec![0.25, SQRT3 / 4.0, 0.5],
                Shape::Small => vec![0.125, SQRT3 / 8.0, 0.25],
                Shape::Trapezoid => vec![0.125, SQRT3 / 8.0, 0.25, 0.25],
                Shape::Quarter => vec![0.25, 0.25, 0.25],
            };
            assert_eq!(s.len(), want.len());
            for (a, b) in s.iter().zip(&want) {
                assert!((a - b).abs() < 1e-9, "{:?}: {s:?}", p.shape);
            }
            assert!((signed_area(&p.outline) - p.shape.area()).abs() < 1e-9);
        }
    }

    /// The full search, timed: `cargo test --release -p ok-sketch --lib mosaic -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn the_full_search_covers_the_tile() {
        let t0 = std::time::Instant::now();
        let rounds: u32 = std::env::var("ROUNDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1500);
        let seed: u64 = std::env::var("SEED")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1);
        // SHAPES=triangle,quarter picks the set; the default set otherwise.
        let shapes: Vec<Shape> = match std::env::var("SHAPES") {
            Ok(v) => v
                .split(',')
                .map(|n| {
                    *Shape::ALL
                        .iter()
                        .find(|s| s.name() == n.trim())
                        .expect("a shape name")
                })
                .collect(),
            Err(_) => Shape::DEFAULT.to_vec(),
        };
        println!(
            "{} candidates for {:?}",
            candidates(&tile(), &shapes).len(),
            shapes
        );
        let m = mosaic_with(&shapes, rounds, seed);
        if let Ok(path) = std::env::var("DUMP") {
            // One piece a line: the shape, then x y pairs.
            let text: String = m
                .pieces
                .iter()
                .map(|p| {
                    let pts: Vec<String> = p
                        .outline
                        .iter()
                        .map(|q| format!("{} {}", q.x, q.y))
                        .collect();
                    format!("{} {}\n", p.shape.name(), pts.join(" "))
                })
                .collect();
            std::fs::write(path, text).unwrap();
        }
        println!(
            "{:.2} % in {} pieces {:?} in {:.1} s",
            100.0 * m.coverage,
            m.pieces.len(),
            m.counts,
            t0.elapsed().as_secs_f64()
        );
    }

    #[test]
    fn a_short_search_covers_most_of_the_tile_with_no_overlaps() {
        let m = mosaic(60, 1);
        println!(
            "{:.2} % in {} pieces {:?}",
            100.0 * m.coverage,
            m.pieces.len(),
            m.counts
        );
        assert!(m.coverage > 0.95, "{}", m.coverage);
        let t = tile();
        for p in &m.pieces {
            assert!(inside(&p.outline, &t));
        }
        for i in 0..m.pieces.len() {
            for j in i + 1..m.pieces.len() {
                assert!(overlap(&m.pieces[i].outline, &m.pieces[j].outline) < 1e-9);
            }
        }
        let sum: f64 = m.pieces.iter().map(|p| p.shape.area()).sum();
        assert!((sum / signed_area(&t) - m.coverage).abs() < 1e-9);
    }
}
