//! The straight Spectre on a table saw: a stick of it, the section a
//! stack of blanks is cut to as one prism, made by rips alone. Every
//! face of the straight tile is a plane along the stick, and a vertical
//! blade makes any plane the stick can present standing up: a face the
//! tile lies wholly to one side of is a through rip, a face that ends
//! at an inside corner is a rabbet, the blade coming up from the open
//! end and stopping at the corner, and no face lies between two inside
//! corners. Two of the through faces are parallel and a third is square
//! to them, so a blank planed to the tile's thickness between them and
//! ripped to its width has three faces to begin with; the rest is ten
//! rips, two through and four pairs of rabbets.
//!
//! The station is a sled with the blade at one height for good and a
//! pocket that takes a cassette per cut. Each cassette is the negative
//! of the stick as the earlier cuts left it, turned so the face stands
//! vertical with the open end down, and placed so the face's line falls
//! on the blade's near side; the thickness of its floor puts the inside
//! corner at the blade's top. The stick only fits the cassette the way
//! the cut wants it, so nothing is set or measured at the saw.
//!
//! [`program`] simulates the sequence with a polygon clipper and
//! reports every cut with the stock before and after it, what came off,
//! the envelopes the cassette's base and lid are the negatives of, and
//! at the end how far the section is from the tile.

use super::vertex;
use crate::{point_in_polygon, signed_area};
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::single::SingleFloatOverlay;
use ok_math::Vec2;

/// The faces of the tile with unit edges, by the vertex each starts at.
/// Faces 5 and 11 are parallel (the blank's thickness), 6 is square to
/// them (its end); 2 and 9 (10 lies on its line) are the through rips;
/// the rest meet an inside corner.
const THICKNESS_FACES: [usize; 2] = [5, 11];
const END_FACE: usize = 6;
/// The cuts in order: the through rips, then the inside corners' pairs,
/// each with its corner at vertex 1, 4, 8 and 13.
const ORDER: [usize; 10] = [2, 9, 0, 1, 3, 4, 7, 8, 12, 13];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    /// The tile's edge, mm.
    pub edge: f64,
    /// The gap between the pieces of the top, mm; the stick is the tile
    /// eroded by half of it.
    pub gap: f64,
    /// The blade's kerf, mm.
    pub kerf: f64,
    /// Blank past the tile's notched end, mm: the waste the first cuts
    /// come through.
    pub margin: f64,
    /// The thinnest floor a cassette gets, mm: the through cuts sit on
    /// it, and the blade's height is set from it.
    pub floor: f64,
    /// How far past an inside corner a rabbet's blade is aimed, mm. A
    /// blade stopping short of the corner leaves a sliver of waste in it
    /// the size of the shortfall; one going past nicks the far face by as
    /// much, a kerf wide, under the gap. Aim past by the setup's error.
    pub bias: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            edge: 9.0,
            gap: 0.7,
            kerf: 3.175,
            margin: 2.0,
            floor: 4.0,
            bias: 0.1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Through,
    Rabbet,
}

/// One rip, in the station's frame: `x` across the sled with the blade's
/// near face (the one the kept stock touches) at zero, `y` up from the
/// sled's plate, the stick's length along the feed.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Cut {
    /// The tile face this cut makes, by the vertex it starts at.
    pub face: usize,
    pub kind: Kind,
    /// The inside corner a rabbet stops at, by vertex.
    pub corner: Option<usize>,
    /// Degrees the stick is turned anticlockwise from the blank's frame
    /// (its thickness faces flat) to stand the face vertical, open end
    /// down.
    pub turn: f64,
    /// Where the blank frame's origin lands in the station's, after the
    /// turn.
    pub shift: Vec2,
    /// The kerf is on the right of the blade's near face (`0..kerf`) or
    /// on its left.
    pub waste_right: bool,
    /// The cassette's floor: the stick's lowest point sits this far
    /// above the plate.
    pub bed: f64,
    /// The blade's top above the plate: the station's one height.
    pub top: f64,
    /// The stock before the cut, anticlockwise, station frame.
    pub before: Vec<Vec2>,
    /// What is left attached after it.
    pub after: Vec<Vec2>,
    /// What came free.
    pub offcuts: Vec<Vec<Vec2>>,
    /// The stock before the cut with the kerfs of the corners still open
    /// filled in: what this cut's cassette is the negative of. A nest
    /// does not go into a kerf.
    pub held: Vec<Vec2>,
    /// The lower envelope of `held`, left to right: the base's nest is
    /// everything under it, so the stick drops in.
    pub lower: Vec<Vec2>,
    /// The upper envelope, left to right: the lid comes down to it.
    pub upper: Vec<Vec2>,
    /// The thinnest offcut's least width, mm; infinite with none.
    pub thin: f64,
    /// `held` is its own mirror image: this cassette would take the
    /// stick turned end for end, which mirrors the section.
    pub symmetric: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Program {
    pub settings: SettingsOut,
    /// The stick's section, eroded by half the gap, in the blank's frame:
    /// its thickness faces on `y = 0` and `y = thickness`, its end face
    /// on `x = width`.
    pub tile: Vec<Vec2>,
    pub thickness: f64,
    pub width: f64,
    pub cuts: Vec<Cut>,
    /// The blade's top above the sled's plate, set once.
    pub height: f64,
    /// Area of the tile the cuts leave uncut, mm²: zero but for the
    /// rounding of the clipper.
    pub left: f64,
    /// Area of the tile the cuts take, mm²: the nicks past the corners.
    pub nicked: f64,
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct SettingsOut {
    pub edge: f64,
    pub gap: f64,
    pub kerf: f64,
    pub margin: f64,
    pub floor: f64,
    pub bias: f64,
}

type Contour = Vec<[f64; 2]>;
const TOL: f64 = 1e-6;

fn to_contour(p: &[Vec2]) -> Contour {
    p.iter().map(|q| [q.x, q.y]).collect()
}

/// Drops repeated and collinear points.
fn clean(p: &[Vec2]) -> Vec<Vec2> {
    let mut q: Vec<Vec2> = Vec::new();
    for &v in p {
        if q.last().is_none_or(|l| l.distance(v) > TOL) {
            q.push(v);
        }
    }
    while q.len() > 1 && q[0].distance(*q.last().unwrap()) <= TOL {
        q.pop();
    }
    let n = q.len();
    (0..n)
        .filter(|&i| {
            let (a, b, c) = (q[(i + n - 1) % n], q[i], q[(i + 1) % n]);
            let (u, v) = (b - a, c - b);
            u.cross(v).abs() > TOL * (u.length() + v.length())
        })
        .map(|i| q[i])
        .collect()
}

/// The outer loops of `subject` clipped by `clip` under `rule`,
/// anticlockwise, biggest first.
fn clip(subject: &[Vec2], clip: &[Vec2], rule: OverlayRule) -> Vec<Vec<Vec2>> {
    let shapes = vec![to_contour(subject)].overlay_as::<i64>(
        &vec![to_contour(clip)],
        rule,
        FillRule::NonZero,
    );
    let mut out: Vec<Vec<Vec2>> = shapes
        .iter()
        .filter_map(|shape| {
            let outer = shape.first()?;
            let mut p: Vec<Vec2> = outer.iter().map(|q| Vec2::new(q[0], q[1])).collect();
            if signed_area(&p) < 0.0 {
                p.reverse();
            }
            let p = clean(&p);
            (p.len() >= 3 && signed_area(&p) > TOL).then_some(p)
        })
        .collect();
    out.sort_by(|a, b| signed_area(b).total_cmp(&signed_area(a)));
    out
}

fn difference(subject: &[Vec2], tool: &[Vec2]) -> Vec<Vec<Vec2>> {
    clip(subject, tool, OverlayRule::Difference)
}

fn intersection(subject: &[Vec2], tool: &[Vec2]) -> Vec<Vec<Vec2>> {
    clip(subject, tool, OverlayRule::Intersect)
}

/// The union of the loops, as one outer loop if they join (they always
/// do here: a kerf is filled back into the stock it was cut from).
fn union(loops: &[Vec<Vec2>]) -> Vec<Vec<Vec2>> {
    let mut acc: Vec<Contour> = vec![to_contour(&loops[0])];
    for l in &loops[1..] {
        let shapes =
            acc.overlay_as::<i64>(&vec![to_contour(l)], OverlayRule::Union, FillRule::NonZero);
        acc = shapes
            .into_iter()
            .filter_map(|s| s.into_iter().next())
            .collect();
    }
    acc.iter()
        .map(|c| {
            let mut p: Vec<Vec2> = c.iter().map(|q| Vec2::new(q[0], q[1])).collect();
            if signed_area(&p) < 0.0 {
                p.reverse();
            }
            clean(&p)
        })
        .collect()
}

fn intersection_area(a: &[Vec2], b: &[Vec2]) -> f64 {
    intersection(a, b).iter().map(|p| signed_area(p)).sum()
}

fn rotate(p: Vec2, deg: f64) -> Vec2 {
    let (s, c) = deg.to_radians().sin_cos();
    Vec2::new(p.x * c - p.y * s, p.x * s + p.y * c)
}

fn transform(poly: &[Vec2], deg: f64, shift: Vec2) -> Vec<Vec2> {
    poly.iter().map(|&q| rotate(q, deg) + shift).collect()
}

/// The tile at `edge`, eroded by half the gap with mitred corners, in the
/// blank's frame: faces 5 and 11 on `y = 0` and `y = thickness`, face 6
/// on `x = width`, the notched end `margin` in from `x = 0`; with the
/// thickness, the width, and a point well inside the tile.
fn blank_tile(s: &Settings) -> (Vec<Vec2>, f64, f64, Vec2) {
    // Turn the unit tile so face 5 runs along +x.
    let d5 = vertex(6) - vertex(5);
    let turn = -d5.angle().to_degrees();
    let raw: Vec<Vec2> = (0..14).map(|i| rotate(vertex(i), turn) * s.edge).collect();
    // Each edge's line moved in by half the gap, adjacent lines met.
    let n = 14;
    let line = |i: usize| {
        let a = raw[i];
        let u = (raw[(i + 1) % n] - a).normalized().unwrap();
        let nin = u.perp();
        (a + nin * (s.gap / 2.0), u)
    };
    let eroded: Vec<Vec2> = (0..n)
        .map(|i| {
            let (a, u) = line((i + n - 1) % n);
            let (b, v) = line(i);
            // a + u t = b + v w  ->  t = cross(b - a, v) / cross(u, v)
            let t = (b - a).cross(v) / u.cross(v);
            a + u * t
        })
        .collect();
    let y0 = eroded[THICKNESS_FACES[0]].y;
    let y1 = eroded[THICKNESS_FACES[1]].y;
    let x1 = eroded[END_FACE].x;
    let x0 = eroded.iter().map(|q| q.x).fold(f64::INFINITY, f64::min);
    let shift = Vec2::new(s.margin - x0, -y0);
    let tile: Vec<Vec2> = eroded.iter().map(|&q| q + shift).collect();
    // (1, 1.5) in tile units is in the middle of the tile's body, the
    // gap or no.
    let inside = rotate(Vec2::new(1.0, 1.5), turn) * s.edge + shift;
    (tile, y1 - y0, x1 - x0 + s.margin, inside)
}

/// The lower (`-1`) or upper (`1`) envelope of a polygon, left to right,
/// with the vertical steps in it.
fn envelope(poly: &[Vec2], side: f64) -> Vec<Vec2> {
    let n = poly.len();
    let mut xs: Vec<f64> = poly.iter().map(|q| q.x).collect();
    xs.sort_by(f64::total_cmp);
    xs.dedup_by(|a, b| (*a - *b).abs() < TOL);
    let mut out: Vec<Vec2> = Vec::new();
    for w in xs.windows(2) {
        let (x0, x1) = (w[0], w[1]);
        let xm = (x0 + x1) / 2.0;
        let mut best: Option<(f64, Vec2, Vec2)> = None;
        for i in 0..n {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            if (a.x - b.x).abs() < TOL || a.x.min(b.x) > xm || a.x.max(b.x) < xm {
                continue;
            }
            let at = |x: f64| a.y + (b.y - a.y) * (x - a.x) / (b.x - a.x);
            let ym = at(xm) * side;
            if best.is_none_or(|(y, _, _)| ym > y) {
                best = Some((ym, Vec2::new(x0, at(x0)), Vec2::new(x1, at(x1))));
            }
        }
        let (_, p0, p1) = best.expect("a polygon spans its own x range");
        if out.last().is_none_or(|l| l.distance(p0) > TOL) {
            out.push(p0);
        }
        out.push(p1);
    }
    // Drop the points inside straight runs.
    let mut keep: Vec<Vec2> = Vec::new();
    for (i, &p) in out.iter().enumerate() {
        if i > 0 && i + 1 < out.len() {
            let (a, c) = (out[i - 1], out[i + 1]);
            if (p - a).cross(c - p).abs() <= TOL * ((p - a).length() + (c - p).length()) {
                continue;
            }
        }
        keep.push(p);
    }
    keep
}

/// The least extent of a polygon over the directions at 5° steps.
fn least_width(p: &[Vec2]) -> f64 {
    (0..36)
        .map(|k| {
            let d = Vec2::from_angle((k as f64 * 5.0).to_radians());
            let (lo, hi) = p
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), q| {
                    (lo.min(q.dot(d)), hi.max(q.dot(d)))
                });
            hi - lo
        })
        .fold(f64::INFINITY, f64::min)
}

/// The cycle of (side, turn) a polygon reads as from each start, as
/// text. Its mirror image, read anticlockwise, is the same cycle
/// backwards, so `mirror` reads it the other way round with the turns
/// the other way.
fn cycle_keys(p: &[Vec2], mirror: bool) -> Vec<String> {
    let q: Vec<Vec2> = if mirror {
        p.iter().rev().copied().collect()
    } else {
        p.to_vec()
    };
    let n = q.len();
    let items: Vec<String> = (0..n)
        .map(|i| {
            let (a, b, c) = (q[i], q[(i + 1) % n], q[(i + 2) % n]);
            let (u, v) = (b - a, c - b);
            let turn = u.cross(v).atan2(u.dot(v)).to_degrees() * if mirror { -1.0 } else { 1.0 };
            format!("{:.2}@{:.0}", u.length(), turn + 0.0)
        })
        .collect();
    (0..n)
        .map(|s| {
            (0..n)
                .map(|i| items[(s + i) % n].as_str())
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect()
}

/// Whether a polygon can be turned onto its own mirror image.
fn mirror_symmetric(p: &[Vec2]) -> bool {
    let p = clean(p);
    let keys = cycle_keys(&p, false);
    let mirrored = cycle_keys(&p, true);
    mirrored.iter().any(|m| keys.contains(m))
}

/// The cuts that make the stick, with the blade's height they settle.
pub fn program(s: &Settings) -> Result<Program, String> {
    for (name, v) in [
        ("edge", s.edge),
        ("kerf", s.kerf),
        ("margin", s.margin),
        ("floor", s.floor),
    ] {
        if !(v.is_finite() && v > 0.0) {
            return Err(format!("{name} must be positive"));
        }
    }
    if !(s.gap.is_finite() && s.gap >= 0.0) {
        return Err("gap must be zero or positive".into());
    }
    if !(s.bias.is_finite() && s.bias >= 0.0) {
        return Err("bias must be zero or positive: short of the corner leaves a sliver".into());
    }
    let (tile, thickness, width, inside) = blank_tile(s);
    if signed_area(&tile) <= 0.0
        || super::self_intersects(&tile)
        || !point_in_polygon(inside, &tile)
    {
        return Err("the gap is too wide for the edge".into());
    }
    let blank = vec![
        Vec2::new(0.0, 0.0),
        Vec2::new(width, 0.0),
        Vec2::new(width, thickness),
        Vec2::new(0.0, thickness),
    ];
    // The blank frame's stock, the kerfs still open (the first cut of a
    // corner's pair, until the second), and the cuts.
    let mut stock = blank.clone();
    let mut open: Vec<(usize, Vec<Vec2>)> = Vec::new();
    let mut cuts: Vec<Cut> = Vec::new();
    let big = 10.0 * (width + thickness);
    for &face in &ORDER {
        let a = tile[face];
        let b = tile[(face + 1) % 14];
        let u = (b - a).normalized().unwrap();
        let corner_start = super::ANGLES[face] > 180;
        let corner_end = super::ANGLES[(face + 1) % 14] > 180;
        let (kind, corner, up) = match (corner_start, corner_end) {
            (false, false) => (Kind::Through, None, true),
            (true, false) => (Kind::Rabbet, Some(face), false),
            (false, true) => (Kind::Rabbet, Some((face + 1) % 14), true),
            (true, true) => return Err(format!("face {face} is a dado")),
        };
        // Turn so u points up (open end down, corner up) or down.
        let target = if up { 90.0 } else { -90.0 };
        let turn = (target - u.angle().to_degrees()).rem_euclid(360.0);
        let mut loops = vec![stock.clone()];
        loops.extend(open.iter().map(|(_, k)| k.clone()));
        let held_blank = union(&loops);
        if held_blank.len() != 1 {
            return Err(format!("the stock came apart before face {face}"));
        }
        let turned = transform(&stock, turn, Vec2::ZERO);
        let a_t = rotate(a, turn);
        // The tile is on the left of u: with u up, left is -x, the kerf
        // on the right; with u down, the kerf on the left.
        let waste_right = up;
        // Station frame: the blade's near face at x = 0, the stick's
        // lowest point at y = bed (bed set once the height is known, so
        // for now at 0; shifted afterwards).
        let ymin = turned.iter().map(|q| q.y).fold(f64::INFINITY, f64::min);
        let shift0 = Vec2::new(-a_t.x, -ymin);
        let before0 = transform(&stock, turn, shift0);
        let corner_y = corner.map(|c| rotate(tile[c], turn).y + shift0.y);
        let top0 = match corner_y {
            Some(y) => y + s.bias,
            None => big,
        };
        let (x0, x1) = if waste_right {
            (0.0, s.kerf)
        } else {
            (-s.kerf, 0.0)
        };
        let slot = vec![
            Vec2::new(x0, -big),
            Vec2::new(x1, -big),
            Vec2::new(x1, top0),
            Vec2::new(x0, top0),
        ];
        let inside_t = rotate(inside, turn) + shift0;
        let pieces = difference(&before0, &slot);
        let keep = pieces
            .iter()
            .position(|p| point_in_polygon(inside_t, p))
            .ok_or("the cut took the tile with it")?;
        let after0 = pieces[keep].clone();
        let offcuts0: Vec<Vec<Vec2>> = pieces
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != keep)
            .map(|(_, p)| p.clone())
            .collect();
        if signed_area(&before0)
            - signed_area(&after0)
            - offcuts0.iter().map(|p| signed_area(p)).sum::<f64>()
            < TOL
        {
            // Nothing came off: the face was already there (10 lies on 9's line).
            continue;
        }
        // Back to the blank frame for the next cut, and the open kerfs.
        let unshift =
            |p: &[Vec2]| -> Vec<Vec2> { p.iter().map(|&q| rotate(q - shift0, -turn)).collect() };
        stock = unshift(&after0);
        // What the blade itself took, for filling back in until the
        // corner's second cut frees it.
        let kerf_taken: Vec<Vec<Vec2>> = intersection(&before0, &slot)
            .iter()
            .map(|p| unshift(p))
            .collect();
        if let Some(c) = corner {
            if let Some(i) = open.iter().position(|(oc, _)| *oc == c) {
                open.remove(i);
            } else {
                for k in kerf_taken {
                    open.push((c, k));
                }
            }
        }
        let held0 = transform(&held_blank[0], turn, shift0);
        let thin = offcuts0
            .iter()
            .map(|p| least_width(p))
            .fold(f64::INFINITY, f64::min);
        cuts.push(Cut {
            face,
            kind,
            corner,
            turn,
            shift: shift0,
            waste_right,
            bed: 0.0,
            top: top0,
            before: before0,
            after: after0,
            offcuts: offcuts0,
            lower: envelope(&held0, -1.0),
            upper: envelope(&held0, 1.0),
            held: held0,
            thin,
            symmetric: mirror_symmetric(&held_blank[0]),
        });
    }
    // The blade's height: the thinnest floor under the tallest through
    // cut, and under every corner.
    let mut height = s.floor;
    for c in &cuts {
        let need = match c.kind {
            Kind::Through => c.before.iter().map(|q| q.y).fold(0.0, f64::max),
            Kind::Rabbet => c.top,
        };
        height = height.max(s.floor + need);
    }
    for c in cuts.iter_mut() {
        c.bed = match c.kind {
            Kind::Through => s.floor,
            Kind::Rabbet => height - c.top,
        };
        let lift = Vec2::new(0.0, c.bed);
        c.shift += lift;
        c.top = height;
        for p in [
            &mut c.before,
            &mut c.after,
            &mut c.held,
            &mut c.lower,
            &mut c.upper,
        ] {
            for q in p.iter_mut() {
                *q += lift;
            }
        }
        for o in c.offcuts.iter_mut() {
            for q in o.iter_mut() {
                *q += lift;
            }
        }
    }
    let final_area = signed_area(&stock);
    let both = intersection_area(&stock, &tile);
    Ok(Program {
        settings: SettingsOut {
            edge: s.edge,
            gap: s.gap,
            kerf: s.kerf,
            margin: s.margin,
            floor: s.floor,
            bias: s.bias,
        },
        tile: tile.clone(),
        thickness,
        width,
        cuts,
        height,
        left: final_area - both,
        nicked: signed_area(&tile) - both,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_area() -> f64 {
        3.0 + 3.0 * 3f64.sqrt()
    }

    #[test]
    fn the_blank_carries_three_faces_and_ten_rips_make_the_rest() {
        let s = Settings::default();
        let p = program(&s).unwrap();
        for c in &p.cuts {
            println!(
                "face {:2} {:?} corner {:?} turn {:4.0} waste {} bed {:5.2} offcuts {} thin {:.2} symmetric {}",
                c.face,
                c.kind,
                c.corner,
                c.turn,
                if c.waste_right { "right" } else { "left " },
                c.bed,
                c.offcuts.len(),
                c.thin,
                c.symmetric
            );
        }
        println!(
            "blank {:.2} x {:.2}, blade at {:.2}, left {:.4}, nicked {:.4}",
            p.width, p.thickness, p.height, p.left, p.nicked
        );
        assert_eq!(p.cuts.len(), 10);
        let through = p.cuts.iter().filter(|c| c.kind == Kind::Through).count();
        assert_eq!(through, 2);
        // Thickness: the tile's extent across faces 5 and 11, less the gap.
        let t = 1.5 * (1.0 + 3f64.sqrt()) * s.edge - s.gap;
        assert!((p.thickness - t).abs() < 1e-9, "{} vs {t}", p.thickness);
        // Every turn is on the 30° grid.
        for c in &p.cuts {
            assert!(
                (c.turn / 30.0 - (c.turn / 30.0).round()).abs() < 1e-9,
                "{}",
                c.turn
            );
            assert!(c.bed >= s.floor - 1e-9);
            assert!((c.top - p.height).abs() < 1e-9);
        }
        // The section comes out as the tile, less the nicks past the
        // corners: a kerf wide and the bias deep at each cut of the two
        // square corners, and next to nothing at the 120° ones, where
        // the overshoot runs under the far face.
        let tile_area = unit_area() * s.edge * s.edge;
        assert!(p.left < 1e-6, "left {}", p.left);
        let nicks = 4.0 * s.kerf * s.bias;
        assert!(
            p.nicked > nicks && p.nicked < 1.25 * nicks,
            "nicked {} vs {nicks}",
            p.nicked
        );
        let last = p.cuts.last().unwrap();
        assert!((signed_area(&last.after) - (signed_area(&p.tile))).abs() < nicks * 1.5);
        assert!(signed_area(&p.tile) < tile_area && signed_area(&p.tile) > 0.9 * tile_area);
    }

    #[test]
    fn a_blade_short_of_the_corner_leaves_a_sliver_and_past_it_a_nick() {
        let s = Settings {
            bias: 0.0,
            ..Settings::default()
        };
        let p = program(&s).unwrap();
        assert!(p.left < 1e-6 && p.nicked < 1e-6, "{} {}", p.left, p.nicked);
        let s = Settings {
            bias: 0.3,
            ..Settings::default()
        };
        let p = program(&s).unwrap();
        assert!(p.left < 1e-6);
        let nicks = 4.0 * s.kerf * 0.3;
        assert!(p.nicked > nicks && p.nicked < 1.25 * nicks, "{}", p.nicked);
    }

    #[test]
    fn every_cassette_but_the_first_refuses_a_stick_end_for_end() {
        let p = program(&Settings::default()).unwrap();
        let sym: Vec<bool> = p.cuts.iter().map(|c| c.symmetric).collect();
        println!("{sym:?}");
        assert!(sym[0]);
        assert!(!sym[1..].iter().any(|&s| s));
    }

    #[test]
    fn the_gap_and_the_bias_are_checked() {
        assert!(program(&Settings {
            gap: 30.0,
            ..Settings::default()
        })
        .is_err());
        assert!(program(&Settings {
            bias: -0.1,
            ..Settings::default()
        })
        .is_err());
    }
}
