//! The curvy Spectre's prisms as sticks: how each is cut. A prism's
//! straight faces are glue lines; its curved faces are bumps and
//! flutes, arcs of one radius. The stick is sawn first and routed
//! after. On the saw the blade stands at 90° for good and a cradle on
//! a one-sided sled tilts the stick so the face to cut stands vertical
//! with the stick on the blade's near side: a convex section has every
//! face as a through rip, the offcut falling away on the far side, so
//! the sled never straddles the blade. A flute is sawn at its chord; a
//! bump is sawn as a gable, two flats at the arc's end angle θ to the
//! chord meeting at a ridge over the apex, each tangent to the arc at
//! its end. The sled carries nothing past the blade, so the kept stock
//! is the part on the near side, resting in its cassette, and the stick
//! goes through as it is or end for end, whichever leaves the offcut
//! above the kept stock, to fall away rather than wedge under it.
//!
//! On the router table a flute is one pass of a core box bit of the
//! arc's radius, the stick lying chord down over the bit, the bit up
//! by the arc's sagitta (plus the gap, for the clearance between the
//! pieces). A bump is two passes of a roundover bit of the arc's
//! radius, one from each gable face: with the gable face standing
//! against the fence and the bit's arc starting at the face's foot (the
//! arc's end, where the face is tangent to it), the roundover sweeps a
//! quarter circle, past the apex by 90° − θ, which lies on the other
//! half of the same circle; its shoulder flat then slits off what is
//! left of the ridge, and the second pass from the other face finishes
//! the arc. That wants 45° ≤ θ ≤ 90°: under 45° the quarter circle
//! would run past the far end of the arc. A half-round bullnose of the
//! same radius will not do instead: its body continues the semicircle
//! past the arc's ends, and at a 120° corner, which the tile has at
//! most bump ends, it would cut 1.7 mm into the face next door.
//!
//! [`plan`] simulates every cut with a polygon clipper and reports, per
//! stick, the blank, the saw cuts in order with the cassette envelopes
//! (as [`super::saw`] does for the straight tile) and the router passes
//! with their cradles and settings.

use super::prisms::{self, FaceKind};
use super::saw::{
    difference, envelope, intersection, least_width, mirror_symmetric, rotate, transform, TOL,
};
use super::Curve;
use crate::{point_in_polygon, signed_area};
use ok_math::Vec2;

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub curve: Curve,
    pub bulge: f64,
    /// The glue lines, tile units; empty for the fewest the search finds.
    pub cuts: Vec<prisms::Cut>,
    /// The tile's edge, mm.
    pub edge: f64,
    /// The clearance between pieces, mm: the flutes are cut this much
    /// deeper than the bumps stand, since bits of one radius mate tight.
    pub gap: f64,
    /// The blade's kerf, mm.
    pub kerf: f64,
    /// The thinnest cassette floor, mm; the blade's height is set from it.
    pub floor: f64,
    /// Blank past the section on every side but the datum, mm: the cuts
    /// come through waste.
    pub margin: f64,
    /// The roundover bit's shoulder flat, mm, past the arc: what slits
    /// the ridge off.
    pub shoulder: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            curve: Curve::Arc,
            bulge: 0.267_3,
            cuts: Vec::new(),
            edge: 10.56,
            gap: 0.2,
            kerf: 3.175,
            floor: 4.0,
            margin: 2.0,
            shoulder: 3.0,
        }
    }
}

/// A face of the sawn section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SawnKind {
    /// A glue line: finished off the saw.
    Glue,
    /// A flute's chord: the core box cuts the flute into it.
    Chord,
    /// One flat of a bump's gable: the roundover rides it.
    Gable,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct SawnFace {
    pub kind: SawnKind,
    /// The prism face (as `prisms::Prism::faces` counts them) this
    /// came from.
    pub prism_face: usize,
    /// For a gable, which flat: 0 from the arc's first end to the
    /// ridge, 1 from the ridge to the second.
    pub side: Option<u8>,
    /// Length, mm.
    pub len: f64,
}

/// One rip: the station's frame has the blade's near face at x = 0, the
/// stick on its left, y up from the sled's plate. The sled is on the
/// left too: nothing holds the offcut.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SawCut {
    /// The sawn section's face this makes.
    pub face: usize,
    pub kind: SawnKind,
    /// Degrees the stick is turned anticlockwise from the blank's frame
    /// (datum down) to stand the face vertical.
    pub turn: f64,
    /// The section is mirrored after the turn: the stick goes through
    /// end for end. Chosen, cut by cut, so the offcut is above the kept
    /// stock.
    pub mirrored: bool,
    /// The cradle's tilt: the datum's angle from the plate after the
    /// turn, degrees, positive rising to the right.
    pub tilt: f64,
    /// Where the blank frame's origin lands in the station's.
    pub shift: Vec2,
    /// The stick's lowest point sits this far over the plate.
    pub bed: f64,
    /// The blade's top over the plate: the station's one height.
    pub top: f64,
    pub before: Vec<Vec2>,
    pub after: Vec<Vec2>,
    pub offcuts: Vec<Vec<Vec2>>,
    /// Lower and upper envelopes of `before`, left to right: the
    /// cassette's base and lid are their negatives.
    pub lower: Vec<Vec2>,
    pub upper: Vec<Vec2>,
    /// The offcut's least width, mm.
    pub thin: f64,
    /// `before` is its own mirror image: the cassette would take the
    /// stick end for end.
    pub symmetric: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PassKind {
    /// The core box, the stick chord down over it.
    Flute,
    /// The roundover, a gable face standing against the fence.
    Bump,
}

/// One router pass. The frame has the table at y = 0; for a flute the
/// bit's axis at x = 0 and the chord on the table; for a bump the gable
/// face in the plane x = 0 with the bit on its left and the stick on its
/// right, the face's foot (the arc's end) at the top, where the
/// roundover's arc starts under its bearing, so the bearing (or the
/// fence) is flush with x = 0 and the arc descends into the gable.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Pass {
    pub kind: PassKind,
    /// The sawn section's face the bit works from.
    pub face: usize,
    pub prism_face: usize,
    pub side: Option<u8>,
    /// Degrees the stick is turned anticlockwise from the blank's frame.
    pub turn: f64,
    /// The datum's angle from the table after the turn, degrees.
    pub tilt: f64,
    pub shift: Vec2,
    /// The section is mirrored after the turn: the stick goes through
    /// end for end. A bump's second pass, so the bit stays on the left.
    pub mirrored: bool,
    /// The stick's lowest point sits this far over the table (a bump's
    /// flat must clear the table).
    pub bed: f64,
    /// The bit's height over the table: a flute's depth, or where a
    /// roundover's arc starts (the bearing's underside).
    pub height: f64,
    pub before: Vec<Vec2>,
    pub after: Vec<Vec2>,
    pub offcuts: Vec<Vec<Vec2>>,
    /// Envelopes of `before`: a flute's cradle comes down on the upper
    /// one, the stick on the table; a bump's cradle is under the lower
    /// one and comes down on the upper.
    pub lower: Vec<Vec2>,
    pub upper: Vec<Vec2>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Stick {
    /// The prism, as `prisms::Decomposition` numbers them, and its shape.
    pub prism: usize,
    pub shape: usize,
    /// The prism face the blank's datum is: the longest glue line.
    pub datum: usize,
    /// The sawn section in the blank's frame: the datum along +x from the
    /// origin, the section above it, mm.
    pub section: Vec<Vec2>,
    pub faces: Vec<SawnFace>,
    /// The blank's box in that frame: x from `blank.0` to `blank.1`, y
    /// from 0 to `blank.2`.
    pub blank: (f64, f64, f64),
    pub saw: Vec<SawCut>,
    pub router: Vec<Pass>,
    /// The section after every pass, blank frame.
    pub finished: Vec<Vec2>,
    /// The prism's curved outline as the tile has it, blank frame, and
    /// how far `finished` is from it in area (the gap's clearance and
    /// the facets).
    pub target: Vec<Vec2>,
    pub off: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Plan {
    pub edge: f64,
    pub gap: f64,
    pub kerf: f64,
    pub floor: f64,
    pub margin: f64,
    pub shoulder: f64,
    /// The arc's end angle and the corner limit, degrees.
    pub theta: f64,
    pub limit: f64,
    /// The arcs' radius, mm: the roundover's and the core box's.
    pub radius: f64,
    /// A bump's sagitta, mm, and a flute's cut depth (the sagitta plus
    /// the gap).
    pub sagitta: f64,
    pub flute_depth: f64,
    /// The gable's ridge over the chord, mm.
    pub ridge: f64,
    pub bits: String,
    /// The blade's top over the sled's plate: one height for every cut
    /// of every stick.
    pub height: f64,
    pub sticks: Vec<Stick>,
}

fn outward(a: Vec2, b: Vec2) -> Vec2 {
    let d = (b - a).normalized().unwrap_or(Vec2::X);
    Vec2::new(d.y, -d.x)
}

/// An arc of `r` about `c` from angle `a0` to `a1` (radians), `n` pieces.
fn arc(c: Vec2, r: f64, a0: f64, a1: f64, n: usize) -> Vec<Vec2> {
    (0..=n)
        .map(|i| {
            let t = a0 + (a1 - a0) * i as f64 / n as f64;
            c + Vec2::new(t.cos(), t.sin()) * r
        })
        .collect()
}

/// The cuts and passes that make every prism of the tile as a stick.
pub fn plan(s: &Settings) -> Result<Plan, String> {
    for (name, v) in [
        ("edge", s.edge),
        ("kerf", s.kerf),
        ("floor", s.floor),
        ("margin", s.margin),
        ("shoulder", s.shoulder),
    ] {
        if !(v.is_finite() && v > 0.0) {
            return Err(format!("{name} must be positive"));
        }
    }
    if !(s.gap.is_finite() && s.gap >= 0.0) {
        return Err("gap must be zero or positive".into());
    }
    if s.curve != Curve::Arc {
        return Err("the sticks are cut for arcs: a roundover and a core box of one radius".into());
    }
    let facet = 10f64.to_radians();
    let dec = if s.cuts.is_empty() {
        prisms::search(s.curve, s.bulge, facet)?
    } else {
        prisms::decompose(s.curve, s.bulge, &s.cuts, facet)?
    };
    if !dec.clean {
        return Err("the prisms are not clean: a corner where a bump ends is too wide for the cutter's shoulder, or a bump is split".into());
    }
    let theta = dec.theta;
    if !(45.0 - 1e-6..=90.0 + 1e-6).contains(&theta) {
        return Err(format!(
            "the arc meets its chord at {theta:.1}°: a roundover makes a bump in two passes between 45° (the quarter circle) and 90° (the semicircle)"
        ));
    }
    let r = dec.radius * s.edge;
    let th = theta.to_radians();
    let sagitta = r * (1.0 - th.cos());
    let flute_depth = sagitta + s.gap;
    let chord = s.edge;
    let ridge = chord / 2.0 * th.tan();
    let big = 40.0 * s.edge;
    let mut sticks = Vec::new();
    for (j, p) in dec.prisms.iter().enumerate() {
        // The sawn section: the straight outline in mm with every bump
        // gabled, and what each face is.
        let n = p.outline.len();
        let mut sawn: Vec<Vec2> = Vec::new();
        let mut faces: Vec<SawnFace> = Vec::new();
        for k in 0..n {
            let (a, b) = (p.outline[k] * s.edge, p.outline[(k + 1) % n] * s.edge);
            let f = &p.faces[k];
            match f.kind {
                FaceKind::Bump => {
                    let m = (a + b) * 0.5 + outward(a, b) * ridge;
                    sawn.push(a);
                    faces.push(SawnFace {
                        kind: SawnKind::Gable,
                        prism_face: k,
                        side: Some(0),
                        len: a.distance(m),
                    });
                    sawn.push(m);
                    faces.push(SawnFace {
                        kind: SawnKind::Gable,
                        prism_face: k,
                        side: Some(1),
                        len: m.distance(b),
                    });
                }
                FaceKind::Flute => {
                    sawn.push(a);
                    faces.push(SawnFace {
                        kind: SawnKind::Chord,
                        prism_face: k,
                        side: None,
                        len: a.distance(b),
                    });
                }
                FaceKind::Glue => {
                    sawn.push(a);
                    faces.push(SawnFace {
                        kind: SawnKind::Glue,
                        prism_face: k,
                        side: None,
                        len: a.distance(b),
                    });
                }
                FaceKind::Ogee => return Err("the sticks are cut for arcs, not the ogee".into()),
            }
        }
        // The datum: the longest glue line. The blank frame has it along
        // +x from the origin.
        let (d, _) = faces
            .iter()
            .enumerate()
            .filter(|(_, f)| f.kind == SawnKind::Glue)
            .max_by(|a, b| a.1.len.total_cmp(&b.1.len))
            .ok_or_else(|| format!("prism {j} has no glue line to stand on"))?;
        let m = sawn.len();
        let a0 = sawn[d];
        let u0 = (sawn[(d + 1) % m] - a0).normalized().unwrap();
        let turn0 = -u0.angle().to_degrees();
        let section: Vec<Vec2> = (0..m)
            .map(|i| rotate(sawn[(d + i) % m] - a0, turn0))
            .collect();
        let faces: Vec<SawnFace> = (0..m).map(|i| faces[(d + i) % m]).collect();
        let datum = faces[0].prism_face;
        let (minx, maxx, maxy) = section.iter().fold(
            (f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY),
            |(lo, hi, top), q| (lo.min(q.x), hi.max(q.x), top.max(q.y)),
        );
        let blank = (minx - s.margin, maxx + s.margin, maxy + s.margin);
        let inside = section.iter().fold(Vec2::ZERO, |acc, q| acc + *q) * (1.0 / m as f64);
        // The saw: every face but the datum, in order round the section.
        let mut stock = vec![
            Vec2::new(blank.0, 0.0),
            Vec2::new(blank.1, 0.0),
            Vec2::new(blank.1, blank.2),
            Vec2::new(blank.0, blank.2),
        ];
        let mut saw: Vec<SawCut> = Vec::new();
        for k in 1..m {
            let (a, b) = (section[k], section[(k + 1) % m]);
            let u = (b - a).normalized().unwrap();
            // Two ways to stand the face vertical with the stick on the
            // left: as it is, the face pointing up, or end for end (the
            // section mirrored), the face pointing down. Both are tried
            // and the one with the offcut higher is kept.
            let mut best: Option<(f64, SawCut, Vec<Vec2>)> = None;
            for mirrored in [false, true] {
                let target = if mirrored { -90.0 } else { 90.0 };
                let turn = (target - u.angle().to_degrees()).rem_euclid(360.0);
                let place = |q: Vec2| {
                    let t = rotate(q, turn);
                    if mirrored {
                        Vec2::new(-t.x, t.y)
                    } else {
                        t
                    }
                };
                let unplace = |q: Vec2| {
                    let t = if mirrored { Vec2::new(-q.x, q.y) } else { q };
                    rotate(t, -turn)
                };
                let placed: Vec<Vec2> = stock.iter().map(|&q| place(q)).collect();
                let a_t = place(a);
                let ymin = placed.iter().map(|q| q.y).fold(f64::INFINITY, f64::min);
                let shift = Vec2::new(-a_t.x, s.floor - ymin);
                let mut before: Vec<Vec2> = placed.iter().map(|&q| q + shift).collect();
                if signed_area(&before) < 0.0 {
                    before.reverse();
                }
                let slot = vec![
                    Vec2::new(0.0, -big),
                    Vec2::new(s.kerf, -big),
                    Vec2::new(s.kerf, big),
                    Vec2::new(0.0, big),
                ];
                let inside_t = place(inside) + shift;
                let pieces = difference(&before, &slot);
                let Some(keep) = pieces.iter().position(|q| point_in_polygon(inside_t, q)) else {
                    return Err(format!(
                        "prism {j}, face {k}: the cut took the stick with it"
                    ));
                };
                let after = pieces[keep].clone();
                let offcuts: Vec<Vec<Vec2>> = pieces
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != keep)
                    .map(|(_, q)| q.clone())
                    .collect();
                let centroid = |poly: &[Vec2]| -> f64 {
                    let n = poly.len() as f64;
                    poly.iter().map(|q| q.y).sum::<f64>() / n
                };
                // The offcut's height over the kept stock; with nothing
                // loose, the stock's own top as low as it goes.
                let rise = match offcuts.first() {
                    Some(o) => centroid(o) - centroid(&after),
                    None => -before.iter().map(|q| q.y).fold(0.0, f64::max),
                };
                let next: Vec<Vec2> = after.iter().map(|&q| unplace(q - shift)).collect();
                let thin = offcuts
                    .iter()
                    .map(|q| least_width(q))
                    .fold(f64::INFINITY, f64::min);
                let cut = SawCut {
                    face: k,
                    kind: faces[k].kind,
                    turn,
                    mirrored,
                    tilt: turn,
                    shift,
                    bed: s.floor,
                    top: 0.0,
                    lower: envelope(&before, -1.0),
                    upper: envelope(&before, 1.0),
                    symmetric: mirror_symmetric(&before),
                    before,
                    after,
                    offcuts,
                    thin,
                };
                if best.as_ref().is_none_or(|(r, _, _)| rise > *r) {
                    best = Some((rise, cut, next));
                }
            }
            let (_, cut, next) = best.unwrap();
            if signed_area(&cut.before)
                - signed_area(&cut.after)
                - cut.offcuts.iter().map(|q| signed_area(q)).sum::<f64>()
                < TOL
            {
                continue;
            }
            let mut next = next;
            if signed_area(&next) < 0.0 {
                next.reverse();
            }
            stock = next;
            saw.push(cut);
        }
        // The router: the stock is the sawn section now.
        let mut router: Vec<Pass> = Vec::new();
        for k in 0..m {
            let (a, b) = (section[k], section[(k + 1) % m]);
            let f = faces[k];
            match f.kind {
                SawnKind::Glue => {}
                SawnKind::Chord => {
                    // Chord down over the bit, its midpoint over the axis.
                    let u = (b - a).normalized().unwrap();
                    let turn = (-u.angle().to_degrees()).rem_euclid(360.0);
                    let mid_t = rotate((a + b) * 0.5, turn);
                    let shift = Vec2::new(-mid_t.x, -mid_t.y);
                    let before = transform(&stock, turn, shift);
                    let centre = Vec2::new(0.0, flute_depth - r);
                    let tool = arc(centre, r, 0.0, std::f64::consts::TAU, 72);
                    let pieces = difference(&before, &tool);
                    let inside_t = rotate(inside, turn) + shift;
                    let keep = pieces
                        .iter()
                        .position(|q| point_in_polygon(inside_t, q))
                        .ok_or_else(|| format!("prism {j}: the flute took the stick"))?;
                    let after = pieces[keep].clone();
                    let offcuts: Vec<Vec<Vec2>> = pieces
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| *i != keep)
                        .map(|(_, q)| q.clone())
                        .collect();
                    stock = after.iter().map(|&q| rotate(q - shift, -turn)).collect();
                    router.push(Pass {
                        kind: PassKind::Flute,
                        face: k,
                        prism_face: f.prism_face,
                        side: None,
                        turn,
                        tilt: turn,
                        shift,
                        mirrored: false,
                        bed: 0.0,
                        height: flute_depth,
                        lower: envelope(&before, -1.0),
                        upper: envelope(&before, 1.0),
                        before,
                        after,
                        offcuts,
                    });
                }
                SawnKind::Gable => {
                    // The gable face standing in x = 0 with its foot (the
                    // arc's end: a for side 0, b for side 1) at the top and
                    // the stick on the right, so the roundover, bearing
                    // flush with the face, starts its arc at the foot and
                    // descends into the gable. Side 0's face points down
                    // from a as the section goes; side 1's points up to b,
                    // so that pass has the stick end for end: the section
                    // mirrored, the face pointing down again.
                    let side = f.side.unwrap();
                    let u = (b - a).normalized().unwrap();
                    let (target, foot, mirrored) = if side == 0 {
                        (-90.0, a, false)
                    } else {
                        (90.0, b, true)
                    };
                    let turn = (target - u.angle().to_degrees()).rem_euclid(360.0);
                    let place = |q: Vec2| {
                        let t = rotate(q, turn);
                        if mirrored {
                            Vec2::new(-t.x, t.y)
                        } else {
                            t
                        }
                    };
                    let unplace = |q: Vec2| {
                        let t = if mirrored { Vec2::new(-q.x, q.y) } else { q };
                        rotate(t, -turn)
                    };
                    let placed: Vec<Vec2> = stock.iter().map(|&q| place(q)).collect();
                    let foot_p = place(foot);
                    let ymin = placed.iter().map(|q| q.y).fold(f64::INFINITY, f64::min);
                    // The flat lies r under the foot; it has to clear the table.
                    let bed = (r + 1.0 - (foot_p.y - ymin)).max(0.0);
                    let shift = Vec2::new(-foot_p.x, bed - ymin);
                    let mut before: Vec<Vec2> = placed.iter().map(|&q| q + shift).collect();
                    if signed_area(&before) < 0.0 {
                        before.reverse();
                    }
                    let foot_s = foot_p + shift;
                    let centre = Vec2::new(r, foot_s.y);
                    // Removed: between the face and the quarter circle
                    // down to the flat; and the flat's slit out to the
                    // shoulder, which frees what is under it if it reaches
                    // across.
                    let mut tool = arc(
                        centre,
                        r,
                        std::f64::consts::PI,
                        1.5 * std::f64::consts::PI,
                        24,
                    );
                    tool.push(Vec2::new(0.0, foot_s.y - r));
                    tool.reverse();
                    let slit = vec![
                        Vec2::new(r - TOL, foot_s.y - r - 0.01),
                        Vec2::new(r + s.shoulder, foot_s.y - r - 0.01),
                        Vec2::new(r + s.shoulder, foot_s.y - r + 0.01),
                        Vec2::new(r - TOL, foot_s.y - r + 0.01),
                    ];
                    let mut pieces: Vec<Vec<Vec2>> = Vec::new();
                    for part in difference(&before, &tool) {
                        pieces.extend(difference(&part, &slit));
                    }
                    let inside_t = place(inside) + shift;
                    let keep = pieces
                        .iter()
                        .position(|q| point_in_polygon(inside_t, q))
                        .ok_or_else(|| format!("prism {j}: the roundover took the stick"))?;
                    let after = pieces[keep].clone();
                    let offcuts: Vec<Vec<Vec2>> = pieces
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| *i != keep)
                        .map(|(_, q)| q.clone())
                        .collect();
                    stock = after.iter().map(|&q| unplace(q - shift)).collect();
                    if signed_area(&stock) < 0.0 {
                        stock.reverse();
                    }
                    router.push(Pass {
                        kind: PassKind::Bump,
                        face: k,
                        prism_face: f.prism_face,
                        side: Some(side),
                        turn,
                        tilt: turn,
                        shift,
                        mirrored,
                        bed,
                        height: foot_s.y,
                        lower: envelope(&before, -1.0),
                        upper: envelope(&before, 1.0),
                        before,
                        after,
                        offcuts,
                    });
                }
            }
        }
        let target: Vec<Vec2> = p
            .curved
            .iter()
            .map(|q| rotate(*q * s.edge - a0, turn0))
            .collect();
        let both: f64 = intersection(&stock, &target)
            .iter()
            .map(|q| signed_area(q))
            .sum();
        let off = (signed_area(&stock) - both) + (signed_area(&target) - both);
        sticks.push(Stick {
            prism: j,
            shape: p.shape,
            datum,
            section,
            faces,
            blank,
            saw,
            router,
            finished: stock,
            target,
            off,
        });
    }
    // The blade's one height: the floor under the tallest stick as it stands for any cut.
    let mut height = s.floor;
    for st in &sticks {
        for c in &st.saw {
            height = height.max(c.before.iter().map(|q| q.y).fold(0.0, f64::max) + s.floor);
        }
    }
    for st in sticks.iter_mut() {
        for c in st.saw.iter_mut() {
            c.top = height;
        }
    }
    let bits = format!(
        "a roundover of radius {r:.2} mm ({:.3} in) in two passes on a {theta:.1}° gable, and a core box of diameter {:.2} mm up {flute_depth:.2}",
        r / 25.4,
        2.0 * r
    );
    Ok(Plan {
        edge: s.edge,
        gap: s.gap,
        kerf: s.kerf,
        floor: s.floor,
        margin: s.margin,
        shoulder: s.shoulder,
        theta,
        limit: dec.limit,
        radius: r,
        sagitta,
        flute_depth,
        ridge,
        bits,
        height,
        sticks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_sticks_are_sawn_through_and_routed_to_the_prisms() {
        let s = Settings::default();
        let p = plan(&s).unwrap();
        assert_eq!(p.sticks.len(), 5);
        assert!((p.radius - 6.35).abs() < 0.02, "{}", p.radius);
        assert!((p.ridge - 7.89).abs() < 0.02, "{}", p.ridge);
        assert!((p.sagitta - 0.2673 * s.edge).abs() < 1e-6);
        for st in &p.sticks {
            let n = st.section.len();
            // the datum along +x, the section above it
            assert!(
                st.section[0].length() < 1e-9
                    && st.section[1].y.abs() < 1e-9
                    && st.section[1].x > 0.0
            );
            assert!(st.section.iter().all(|q| q.y > -1e-9));
            // a cut per face but the datum, each through with one offcut
            assert_eq!(st.saw.len(), n - 1, "prism {}", st.prism);
            println!(
                "prism {} shape {} datum face {}: blank {:.1} to {:.1} by {:.1}, {} cuts, {} passes, off {:.2}",
                st.prism, st.shape, st.datum, st.blank.0, st.blank.1, st.blank.2, st.saw.len(), st.router.len(), st.off
            );
            for c in &st.saw {
                println!(
                    "   saw face {} {:?} tilt {:6.1} end for end {} offcuts {} thin {:.1}",
                    c.face,
                    c.kind,
                    c.tilt,
                    c.mirrored,
                    c.offcuts.len(),
                    c.thin
                );
                // the offcut rides above the kept stock, to fall away
                if let Some(o) = c.offcuts.first() {
                    let mid = |p: &[Vec2]| p.iter().map(|q| q.y).sum::<f64>() / p.len() as f64;
                    assert!(
                        mid(o) > mid(&c.after) - 1e-9,
                        "prism {} face {}: the offcut is under the stock",
                        st.prism,
                        c.face
                    );
                }
                // at most one offcut: a sliver under a kerf wide is all blade
                assert!(c.offcuts.len() <= 1, "prism {} face {}", st.prism, c.face);
                let taken = signed_area(&c.before)
                    - signed_area(&c.after)
                    - c.offcuts.iter().map(|o| signed_area(o)).sum::<f64>();
                // what the blade took is a kerf wide along the face
                assert!(taken > 0.0 && taken < s.kerf * 60.0, "{taken}");
                assert!((c.top - p.height).abs() < 1e-9 && c.bed >= s.floor - 1e-9);
                let lowest = c.before.iter().map(|q| q.y).fold(f64::INFINITY, f64::min);
                assert!((lowest - s.floor).abs() < 1e-9);
            }
            // off the saw the stock is the sawn section
            let last = st.saw.last().unwrap();
            let sawn_area = signed_area(&st.section);
            assert!(
                (signed_area(&last.after) - sawn_area).abs() < 1e-6,
                "prism {}",
                st.prism
            );
            for q in &st.router {
                println!("   pass {:?} face {} side {:?} tilt {:6.1} mirrored {} bed {:.1} height {:.2} offcuts {}", q.kind, q.face, q.side, q.tilt, q.mirrored, q.bed, q.height, q.offcuts.len());
            }
            // a pass per chord, two per bump
            let chords = st
                .faces
                .iter()
                .filter(|f| f.kind == SawnKind::Chord)
                .count();
            let gables = st
                .faces
                .iter()
                .filter(|f| f.kind == SawnKind::Gable)
                .count();
            assert_eq!(
                st.router
                    .iter()
                    .filter(|q| q.kind == PassKind::Flute)
                    .count(),
                chords
            );
            assert_eq!(
                st.router
                    .iter()
                    .filter(|q| q.kind == PassKind::Bump)
                    .count(),
                gables
            );
            assert!(gables % 2 == 0);
            // and the result is the prism's curved outline, the flutes
            // the gap deeper and the arcs faceted
            let tile_area = signed_area(&st.target);
            assert!(
                st.off < 0.03 * tile_area,
                "prism {}: off by {} of {tile_area}",
                st.prism,
                st.off
            );
            assert!(
                signed_area(&st.finished) < tile_area,
                "prism {}: flutes are deeper",
                st.prism
            );
        }
        // the station's height clears every stick standing up
        assert!(
            p.height > s.floor + 20.0 && p.height < s.floor + 60.0,
            "{}",
            p.height
        );
    }

    #[test]
    fn the_quarter_circle_and_the_semicircle_plan_and_the_cubic_does_not() {
        let q = plan(&Settings {
            bulge: 0.207_107,
            edge: 8.98,
            ..Settings::default()
        })
        .unwrap();
        assert_eq!(q.sticks.len(), 4);
        assert!((q.theta - 45.0).abs() < 0.1);
        let semi = plan(&Settings {
            bulge: 0.5,
            edge: 9.0,
            ..Settings::default()
        })
        .unwrap();
        assert_eq!(semi.sticks.len(), 9);
        assert!(plan(&Settings {
            bulge: 0.15,
            ..Settings::default()
        })
        .is_err());
        assert!(plan(&Settings {
            curve: Curve::Cubic,
            bulge: 0.6,
            ..Settings::default()
        })
        .is_err());
    }
}
