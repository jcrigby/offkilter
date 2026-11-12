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
fn touches(piece: &[Vec2], tile: &[Vec2]) -> bool {
    let n = tile.len();
    piece.iter().any(|p| {
        (0..n).any(|j| {
            let (a, b) = (tile[j], tile[(j + 1) % n]);
            let ab = b - a;
            let t = (*p - a).dot(ab) / ab.dot(ab);
            (0.0..=1.0).contains(&t) && (a + ab * t).distance(*p) < 1e-6
        })
    })
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
    let mut best_area = total(&best);
    let mut cur_area = best_area;
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
        if a >= cur_area - 1e-12 {
            chosen = trial;
            cur_area = a;
            if a > best_area + 1e-9 {
                best_area = a;
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
        for m in &mosaic.pieces {
            let poly = ccw(m.outline.iter().map(|q| super::apply(xf, *q)).collect());
            let k = poly.len();
            let mut cut = poly.clone();
            for i in 0..k {
                let (a, b) = (poly[i], poly[(i + 1) % k]);
                let on_edge = touches(&[a], &edge_line)
                    && touches(&[b], &edge_line)
                    && touches(&[(a + b) * 0.5], &edge_line);
                let d = if on_edge { gap / 2.0 } else { inner / 2.0 };
                // The side moved in by d: the left of a → b is the inside.
                let n = (b - a).perp().normalized().unwrap_or(Vec2::ZERO) * d;
                cut = clip_left(&cut, a + n, b + n);
                if cut.len() < 3 {
                    break;
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
            let laid = lay(&patch, &m, 0.2, 0.2, 0.0);
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
                for q in &l.outline {
                    assert!(
                        point_in_polygon(*q, &framed),
                        "whole {whole}: a stick outside its tile"
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
