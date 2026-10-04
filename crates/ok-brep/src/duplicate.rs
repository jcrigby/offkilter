//! Whether a body can be copied on a duplicator: a pilot traces the
//! master from above while a bit of the same shape cuts the copy beside
//! it, so the copy is what a bit coming down from above can make of the
//! master's top surface. The check samples that surface as a height
//! field from the tessellation, rolls the bit over it (a morphological
//! closing with the bit's floor, a disc or a ball) to get the surface
//! the bit can leave, and reports what the two leave out: material under
//! overhangs the pilot never sees, concave corners tighter than the bit
//! and depths beyond the bit's reach.

use crate::Solid;
use ok_math::Vec3;

/// The bit's floor: a straight bit ends flat, a ball-nose bit in a
/// hemisphere of its radius.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitShape {
    Flat,
    Ball,
}

/// The bit (and the pilot, which has the same shape).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bit {
    pub diameter: f64,
    /// How far below the master's highest point the bit can cut: its
    /// cutting length less what the pilot needs to stay in contact.
    pub reach: f64,
    pub shape: BitShape,
}

/// The master seen from above: the highest point of the body over each
/// cell of a grid, `None` where the body has no material.
#[derive(Debug, Clone)]
pub struct HeightField {
    /// The grid's lower-left corner (the body's bounds).
    pub origin: (f64, f64),
    pub pitch: f64,
    pub cols: usize,
    pub rows: usize,
    /// Row-major, `rows` rows of `cols`.
    pub heights: Vec<Option<f64>>,
    /// The body's lowest and highest z.
    pub floor: f64,
    pub top: f64,
}

impl HeightField {
    /// The body's top surface over cells `pitch` apart, from its
    /// tessellation: every triangle's z over the cell centres it covers,
    /// the highest kept.
    pub fn of(solid: &Solid, pitch: f64) -> Option<HeightField> {
        let (lo, hi) = solid.bounds()?;
        let pitch = pitch.max(1e-3);
        let cols = ((hi.x - lo.x) / pitch).ceil().max(1.0) as usize;
        let rows = ((hi.y - lo.y) / pitch).ceil().max(1.0) as usize;
        let mut heights = vec![None; cols * rows];
        let mesh = crate::tessellate(solid);
        let p = |i: u32| {
            let i = i as usize * 3;
            Vec3::new(
                mesh.positions[i] as f64,
                mesh.positions[i + 1] as f64,
                mesh.positions[i + 2] as f64,
            )
        };
        for t in mesh.indices.chunks(3) {
            let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
            let det = (b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y);
            if det.abs() < 1e-12 {
                continue; // vertical: nothing to see from above
            }
            let (x0, x1) = (a.x.min(b.x).min(c.x), a.x.max(b.x).max(c.x));
            let (y0, y1) = (a.y.min(b.y).min(c.y), a.y.max(b.y).max(c.y));
            let i0 = (((x0 - lo.x) / pitch - 0.5).floor().max(0.0)) as usize;
            let i1 = (((x1 - lo.x) / pitch - 0.5).ceil().max(0.0) as usize).min(cols - 1);
            let j0 = (((y0 - lo.y) / pitch - 0.5).floor().max(0.0)) as usize;
            let j1 = (((y1 - lo.y) / pitch - 0.5).ceil().max(0.0) as usize).min(rows - 1);
            for j in j0..=j1 {
                let y = lo.y + (j as f64 + 0.5) * pitch;
                for i in i0..=i1 {
                    let x = lo.x + (i as f64 + 0.5) * pitch;
                    // Barycentric coordinates in plan, with a hair of slack so
                    // shared edges leave no gaps.
                    let u = ((b.x - x) * (c.y - y) - (c.x - x) * (b.y - y)) / det;
                    let v = ((c.x - x) * (a.y - y) - (a.x - x) * (c.y - y)) / det;
                    let w = 1.0 - u - v;
                    if u < -1e-9 || v < -1e-9 || w < -1e-9 {
                        continue;
                    }
                    let z = u * a.z + v * b.z + w * c.z;
                    let cell = &mut heights[j * cols + i];
                    if cell.is_none_or(|h| z > h) {
                        *cell = Some(z);
                    }
                }
            }
        }
        Some(HeightField {
            origin: (lo.x, lo.y),
            pitch,
            cols,
            rows,
            heights,
            floor: lo.z,
            top: hi.z,
        })
    }

    /// The volume under the top surface down to the floor: what the copy
    /// has, every overhang's hollow filled.
    pub fn volume(&self) -> f64 {
        self.heights
            .iter()
            .flatten()
            .map(|h| (h - self.floor).max(0.0))
            .sum::<f64>()
            * self.pitch
            * self.pitch
    }

    /// The surface a bit leaves when it is lowered onto the field
    /// everywhere it can go: a morphological closing with the bit's
    /// floor, a flat disc or a ball of its radius. Where the body has no
    /// material the floor stands in.
    pub fn cut_by(&self, bit: &Bit) -> Vec<f64> {
        let r = bit.diameter / 2.0;
        // The structuring element: the bit's floor as a height over each
        // cell offset it covers, zero for a flat bit, the ball's underside
        // (highest at the centre) for a ball. A flat floor's cell counts
        // when it lies inside the floor, not merely its centre: taking
        // every cell the centre reaches makes the sampled floor half a
        // pitch too wide each way, and a bit the size of a groove then
        // never fits it. A ball's rim is what meets a wall, so its cells
        // run to the full radius, where its height is nothing anyway.
        let inside = match bit.shape {
            BitShape::Flat => (r - self.pitch / 2.0).max(0.0),
            BitShape::Ball => r,
        };
        let k = (inside / self.pitch).floor() as i64;
        let mut element: Vec<(i64, i64, f64)> = Vec::new();
        for dj in -k..=k {
            for di in -k..=k {
                let rho = ((di * di + dj * dj) as f64).sqrt() * self.pitch;
                if rho <= inside + 1e-9 {
                    let s = match bit.shape {
                        BitShape::Flat => 0.0,
                        BitShape::Ball => (r * r - rho * rho).max(0.0).sqrt(),
                    };
                    element.push((di, dj, s));
                }
            }
        }
        let h = |i: i64, j: i64| -> f64 {
            if i < 0 || j < 0 || i >= self.cols as i64 || j >= self.rows as i64 {
                return self.floor;
            }
            self.heights[j as usize * self.cols + i as usize].unwrap_or(self.floor)
        };
        // The bit's lowest centre over each cell: touching the field
        // somewhere under its floor (dilation).
        let mut centre = vec![self.floor; self.cols * self.rows];
        for j in 0..self.rows as i64 {
            for i in 0..self.cols as i64 {
                let mut z = f64::NEG_INFINITY;
                for &(di, dj, s) in &element {
                    z = z.max(h(i + di, j + dj) + s);
                }
                centre[j as usize * self.cols + i as usize] = z;
            }
        }
        // The floor's underside over each cell: the lowest it gets from
        // any centre within the radius (erosion).
        let c = |i: i64, j: i64| -> f64 {
            if i < 0 || j < 0 || i >= self.cols as i64 || j >= self.rows as i64 {
                return self.floor + r;
            }
            centre[j as usize * self.cols + i as usize]
        };
        let mut cut = vec![self.floor; self.cols * self.rows];
        for j in 0..self.rows as i64 {
            for i in 0..self.cols as i64 {
                let mut z = f64::INFINITY;
                for &(di, dj, s) in &element {
                    z = z.min(c(i + di, j + dj) - s);
                }
                cut[j as usize * self.cols + i as usize] = z.max(h(i, j));
            }
        }
        cut
    }
}

/// What a duplicator with `bit` makes of the master and leaves out.
#[derive(Debug, Clone)]
pub struct DuplicateReport {
    /// The bit the check was made with.
    pub bit: Bit,
    pub field: HeightField,
    /// The surface the bit leaves, row-major like the field's heights.
    pub cut: Vec<f64>,
    /// Over each cell, how much the cut surface stands above the master's:
    /// the material the bit cannot reach into (concave corners tighter
    /// than its radius).
    pub residual: Vec<f64>,
    pub residual_max: f64,
    /// Area (mm²) where the residual is more than a few hundredths.
    pub residual_area: f64,
    /// How far below the master's highest point its lowest seen point is.
    pub depth_max: f64,
    /// Area (mm²) deeper than the bit reaches.
    pub too_deep_area: f64,
    /// The master's volume and what the copy would have: the difference
    /// is material under overhangs the pilot never sees.
    pub volume: f64,
    pub copy_volume: f64,
    pub undercut_volume: f64,
    /// One line per thing the duplicator cannot do; empty when the copy
    /// would be the master.
    pub problems: Vec<String>,
}

/// Residual below this (mm) is sampling noise, not a corner.
pub const RESIDUAL_TOLERANCE: f64 = 0.05;

/// Checks `solid` as a master for a duplicator with `bit`, sampling it
/// `pitch` apart (a quarter of the bit, within reason, when `None`).
pub fn check(solid: &Solid, bit: &Bit, pitch: Option<f64>) -> Result<DuplicateReport, String> {
    if bit.diameter <= 0.0 || bit.diameter.is_nan() {
        return Err("the bit needs a diameter".into());
    }
    let (lo, hi) = solid.bounds().ok_or("an empty body")?;
    let extent = (hi.x - lo.x).max(hi.y - lo.y);
    let pitch = pitch.unwrap_or_else(|| (bit.diameter / 4.0).max(extent / 600.0).max(0.1));
    let field = HeightField::of(solid, pitch).ok_or("an empty body")?;
    let cut = field.cut_by(bit);
    let cell = pitch * pitch;
    let (mut residual_max, mut residual_area, mut depth_max, mut too_deep_area) =
        (0.0f64, 0.0, 0.0f64, 0.0);
    let mut residual = vec![0.0; cut.len()];
    for (k, h) in field.heights.iter().enumerate() {
        let Some(h) = h else { continue };
        let r = (cut[k] - h).max(0.0);
        residual[k] = r;
        residual_max = residual_max.max(r);
        if r > RESIDUAL_TOLERANCE {
            residual_area += cell;
        }
        let depth = field.top - h;
        depth_max = depth_max.max(depth);
        if depth > bit.reach + 1e-9 {
            too_deep_area += cell;
        }
    }
    let volume = solid.volume();
    let copy_volume = field.volume();
    // The grid's own error: half a cell along every edge of the footprint.
    let sampling = 2.0 * (hi.x - lo.x + hi.y - lo.y) * pitch * 0.5 * (hi.z - lo.z);
    let undercut_volume = (copy_volume - volume).max(0.0);
    let mut problems = Vec::new();
    if undercut_volume > sampling.max(0.02 * volume) {
        problems.push(format!(
            "undercuts: {undercut_volume:.0} mm3 of the copy would be solid where the master is hollow ({:.1} % of its volume), under overhangs a pilot from above never sees",
            100.0 * undercut_volume / volume
        ));
    }
    if residual_area > 0.0 {
        problems.push(format!(
            "concave corners tighter than the {} mm bit: {residual_area:.0} mm2 left uncut, up to {residual_max:.2} mm high",
            bit.diameter
        ));
    }
    if too_deep_area > 0.0 {
        problems.push(format!(
            "deeper than the bit reaches: {too_deep_area:.0} mm2 lies {depth_max:.1} mm below the top, the bit reaches {}",
            bit.reach
        ));
    }
    Ok(DuplicateReport {
        bit: *bit,
        field,
        cut,
        residual,
        residual_max,
        residual_area,
        depth_max,
        too_deep_area,
        volume,
        copy_volume,
        undercut_volume,
        problems,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{boolean, extrude, BoolOp};
    use ok_math::{Plane, Vec2};
    use ok_sketch::{ProfileOptions, Sketch};

    fn block(x0: f64, y0: f64, x1: f64, y1: f64, z0: f64, z1: f64) -> Solid {
        let mut sk = Sketch::new();
        sk.add_rectangle(Vec2::new(x0, y0), Vec2::new(x1, y1));
        let profile = sk.profiles(&ProfileOptions::default()).remove(0);
        extrude(&profile, &Plane::XY, z0, z1, 0).unwrap()
    }

    fn bit(d: f64, shape: BitShape) -> Bit {
        Bit {
            diameter: d,
            reach: 12.0,
            shape,
        }
    }

    #[test]
    fn a_plain_block_copies_exactly() {
        let b = block(0.0, 0.0, 40.0, 30.0, 0.0, 10.0);
        let r = check(&b, &bit(3.0, BitShape::Flat), Some(0.5)).unwrap();
        assert!(r.problems.is_empty(), "{:?}", r.problems);
        assert!((r.copy_volume - r.volume).abs() < 0.01 * r.volume);
        assert!(r.residual_max < 1e-9 && r.depth_max.abs() < 1e-9);
    }

    #[test]
    fn a_slot_narrower_than_the_bit_is_left_uncut() {
        // A 2 mm slot 5 deep across a block: a 1 mm bit follows it, a
        // 3 mm bit leaves it, a 3 mm ball nose leaves it too.
        let b = block(0.0, 0.0, 40.0, 30.0, 0.0, 10.0);
        let slot = block(19.0, -1.0, 21.0, 31.0, 5.0, 11.0);
        let b = boolean(&b, &slot, BoolOp::Difference).unwrap();
        let fine = check(&b, &bit(1.0, BitShape::Flat), Some(0.25)).unwrap();
        assert!(fine.problems.is_empty(), "{:?}", fine.problems);
        let coarse = check(&b, &bit(3.0, BitShape::Flat), Some(0.25)).unwrap();
        assert_eq!(coarse.problems.len(), 1, "{:?}", coarse.problems);
        assert!(coarse.problems[0].starts_with("concave corners"));
        assert!(
            (coarse.residual_max - 5.0).abs() < 0.3,
            "{}",
            coarse.residual_max
        );
        assert!(
            (coarse.residual_area - 60.0).abs() < 8.0,
            "{}",
            coarse.residual_area
        );
        let ball = check(&b, &bit(3.0, BitShape::Ball), Some(0.25)).unwrap();
        assert!(ball.residual_area > 40.0, "{}", ball.residual_area);
        assert!((coarse.depth_max - 5.0).abs() < 1e-6);
    }

    #[test]
    fn a_ball_nose_leaves_the_fillet_a_flat_bit_cuts_square() {
        // A step: a flat bit cuts the inside corner square; a ball nose
        // leaves a fillet of its radius along it.
        let b = block(0.0, 0.0, 40.0, 30.0, 0.0, 10.0);
        let step = block(20.0, -1.0, 41.0, 31.0, 5.0, 11.0);
        let b = boolean(&b, &step, BoolOp::Difference).unwrap();
        let flat = check(&b, &bit(4.0, BitShape::Flat), Some(0.25)).unwrap();
        assert!(flat.residual_area < 1.0, "{}", flat.residual_area);
        let ball = check(&b, &bit(4.0, BitShape::Ball), Some(0.25)).unwrap();
        // A fillet of radius 2 along the 30 mm corner: its footprint, where
        // the ball's surface stands more than the tolerance above the
        // floor, is a strip a millimetre or two wide along the wall.
        assert!(
            (30.0..80.0).contains(&ball.residual_area),
            "{}",
            ball.residual_area
        );
        assert!(
            (ball.residual_max - 2.0).abs() < 0.3,
            "{}",
            ball.residual_max
        );
    }

    #[test]
    fn an_overhang_is_an_undercut_and_a_deep_pocket_is_out_of_reach() {
        // A post with a wider cap: the hollow under the cap is 10 mm of
        // overhang all round, which the copy would fill.
        let post = block(10.0, 10.0, 30.0, 20.0, 0.0, 20.0);
        let cap = block(0.0, 0.0, 40.0, 30.0, 20.0, 25.0);
        let t = boolean(&post, &cap, BoolOp::Union).unwrap();
        let r = check(&t, &bit(3.0, BitShape::Flat), Some(0.5)).unwrap();
        let hollow = (40.0 * 30.0 - 20.0 * 10.0) * 20.0;
        assert!(
            (r.undercut_volume - hollow).abs() < 0.03 * hollow,
            "{}",
            r.undercut_volume
        );
        assert!(
            r.problems.iter().any(|p| p.starts_with("undercuts")),
            "{:?}",
            r.problems
        );
        // A pocket 15 deep when the bit reaches 12.
        let b = block(0.0, 0.0, 40.0, 30.0, 0.0, 20.0);
        let pocket = block(10.0, 10.0, 30.0, 20.0, 5.0, 21.0);
        let b = boolean(&b, &pocket, BoolOp::Difference).unwrap();
        let r = check(&b, &bit(3.0, BitShape::Flat), Some(0.5)).unwrap();
        assert!((r.depth_max - 15.0).abs() < 1e-6);
        assert!(
            (r.too_deep_area - 200.0).abs() < 10.0,
            "{}",
            r.too_deep_area
        );
        assert!(
            r.problems.iter().any(|p| p.starts_with("deeper")),
            "{:?}",
            r.problems
        );
    }
}
