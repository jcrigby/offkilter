//! Measuring fields: the measuring sheet grown past one page and made
//! to survive being covered. A field is a lattice of bullseye marks
//! `PITCH` mm apart, each carrying its column and row as two short rows
//! of code dots, printed as landscape tiles that overlap by a whole
//! mark so they tape together mark over mark. A photograph needs any
//! four marks, not the corners: a part can lie across some, a
//! turntable's edge can cut the corners off, and every mark that reads
//! goes into one fit, so a taped seam that is a little off is averaged
//! out rather than trusted.

use super::{
    apply, components, invert, scene_at, Comp, Scene, SheetSize, DOT_R, H, ORIGIN_RING_HOLE_R,
    ORIGIN_RING_R, RING_HOLE_R, RING_R,
};
use ok_render::{Image, Rgb};
use ok_sheet::pdf::{document, Anchor, Page};
use serde::Serialize;
use std::f64::consts::PI;

/// Marks sit on this lattice, in millimetres.
pub const PITCH: f64 = 50.0;
/// Where the field's origin mark sits on tile (0, 0): in from the
/// page's left and bottom edges.
const X0: f64 = 30.0;
const Y0: f64 = 36.0;
/// What a printer leaves blank at the paper's edge.
const MARGIN: f64 = 6.0;
/// The code: two rows of `SLOTS` slots `SLOT_PITCH` apart, `CODE_DY`
/// below and above the mark's centre. The lower row's end slots are
/// always dots (the anchors, which also give the row's direction); its
/// middle four are the column, the upper row's middle four the row,
/// most significant first, a dot for a one.
const CODE_DY: f64 = 11.0;
const SLOT_PITCH: f64 = 4.5;
const SLOTS: usize = 6;
const CODE_R: f64 = 1.3;
const CODE_BITS: usize = 4;
/// How far a mark's ink reaches from its centre, each way.
const INK_X: f64 = (SLOTS as f64 - 1.0) / 2.0 * SLOT_PITCH + CODE_R + 0.2;
const INK_Y: f64 = CODE_DY + CODE_R + 0.2;
/// A tile's cut line sits this far before its first mark column or row.
const CUT: f64 = 20.0;

/// Where slot `k` of a code row sits along the row, from the mark's centre.
fn slot_u(k: usize) -> f64 {
    (k as f64 - (SLOTS as f64 - 1.0) / 2.0) * SLOT_PITCH
}

/// Whether slot `k` of the lower (`upper == false`) or upper row of
/// the mark at column `i`, row `j` carries a dot.
fn slot_set(i: usize, j: usize, upper: bool, k: usize) -> bool {
    if k == 0 || k == SLOTS - 1 {
        return !upper;
    }
    let value = if upper { j } else { i };
    (value >> (CODE_BITS - k)) & 1 == 1
}

/// A field: `nx` by `ny` tiles of one sheet size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
    pub size: SheetSize,
    pub nx: usize,
    pub ny: usize,
}

impl Field {
    pub fn new(size: SheetSize, nx: usize, ny: usize) -> Result<Field, String> {
        if nx == 0 || ny == 0 {
            return Err("a field needs at least one tile each way".into());
        }
        let f = Field { size, nx, ny };
        let (c, r) = (f.columns(), f.rows());
        if c > 1 << CODE_BITS || r > 1 << CODE_BITS {
            return Err(format!(
                "a {} field of {nx} x {ny} tiles has {c} x {r} marks; the marks' codes count to {} each way",
                size.name(),
                1 << CODE_BITS
            ));
        }
        Ok(f)
    }

    /// `"2x2"`: tiles across by tiles up.
    pub fn parse(size: SheetSize, tiles: &str) -> Result<Field, String> {
        let say = || format!("tiles {tiles:?}: say how many across and up, like \"2x2\"");
        let t = tiles.trim().to_lowercase();
        let (a, b) = t.split_once(['x', '×', '*']).ok_or_else(say)?;
        let n = |s: &str| s.trim().parse::<usize>().map_err(|_| say());
        Field::new(size, n(a)?, n(b)?)
    }

    pub fn name(&self) -> String {
        format!("{} {}x{}", self.size.name(), self.nx, self.ny)
    }

    /// Marks along one tile's x and y: the first at the tile's own
    /// origin, the last the furthest whose ring still prints.
    pub fn per_tile(&self) -> (usize, usize) {
        let (w, h) = self.size.size();
        (
            ((w - MARGIN - X0 - RING_R) / PITCH).floor() as usize + 1,
            ((h - MARGIN - Y0 - RING_R) / PITCH).floor() as usize + 1,
        )
    }

    /// How far apart the tiles' origins are: the next tile starts on
    /// this tile's last mark column and row.
    pub fn tile_pitch(&self) -> (f64, f64) {
        let (c, r) = self.per_tile();
        ((c - 1) as f64 * PITCH, (r - 1) as f64 * PITCH)
    }

    pub fn columns(&self) -> usize {
        (self.per_tile().0 - 1) * self.nx + 1
    }

    pub fn rows(&self) -> usize {
        (self.per_tile().1 - 1) * self.ny + 1
    }

    /// The printed extent of tile (a, b) in field millimetres: x0, y0,
    /// x1, y1.
    pub fn tile_bounds(&self, a: usize, b: usize) -> [f64; 4] {
        let (w, h) = self.size.size();
        let (tx, ty) = self.tile_pitch();
        let (ox, oy) = (tx * a as f64, ty * b as f64);
        [
            ox - (X0 - MARGIN),
            oy - (Y0 - MARGIN),
            ox + w - X0 - MARGIN,
            oy + h - Y0 - MARGIN,
        ]
    }

    /// The taped field's extent: x0, y0, x1, y1.
    pub fn bounds(&self) -> [f64; 4] {
        let a = self.tile_bounds(0, 0);
        let b = self.tile_bounds(self.nx - 1, self.ny - 1);
        [a[0], a[1], b[2], b[3]]
    }

    pub fn mark(i: usize, j: usize) -> [f64; 2] {
        [i as f64 * PITCH, j as f64 * PITCH]
    }

    /// Whether the mark at (i, j) prints whole on tile (a, b).
    fn mark_on_tile(&self, i: usize, j: usize, a: usize, b: usize) -> bool {
        let [x0, y0, x1, y1] = self.tile_bounds(a, b);
        let [mx, my] = Field::mark(i, j);
        mx - INK_X >= x0 && mx + INK_X <= x1 && my - INK_Y >= y0 && my + INK_Y <= y1
    }

    fn tile_page(&self, a: usize, b: usize) -> Page {
        let (w, h) = self.size.size();
        let tb = self.tile_bounds(a, b);
        let (tx, ty) = self.tile_pitch();
        // Page = field + (ox, oy).
        let (ox, oy) = (X0 - tx * a as f64, Y0 - ty * b as f64);
        let mut page = Page::new(w, h);
        // The grid, light so photographs threshold it away: 10 mm
        // lines, heavier every 50, over the whole printable tile.
        let mut fine = Vec::new();
        let mut coarse = Vec::new();
        let mut x = (tb[0] / 10.0).ceil() * 10.0;
        while x <= tb[2] + 1e-9 {
            let seg = [(ox + x, oy + tb[1]), (ox + x, oy + tb[3])];
            if (x % 50.0).abs() < 1e-9 {
                coarse.push(seg);
            } else {
                fine.push(seg);
            }
            x += 10.0;
        }
        let mut y = (tb[1] / 10.0).ceil() * 10.0;
        while y <= tb[3] + 1e-9 {
            let seg = [(ox + tb[0], oy + y), (ox + tb[2], oy + y)];
            if (y % 50.0).abs() < 1e-9 {
                coarse.push(seg);
            } else {
                fine.push(seg);
            }
            y += 10.0;
        }
        page.gray(0.78);
        page.lines(&fine, 0.15, None);
        page.gray(0.55);
        page.lines(&coarse, 0.3, None);
        // The marks that print whole on this tile, each with its code
        // and its position in small print beside it.
        for j in 0..self.rows() {
            for i in 0..self.columns() {
                if !self.mark_on_tile(i, j, a, b) {
                    continue;
                }
                let [mx, my] = Field::mark(i, j);
                let (cx, cy) = (ox + mx, oy + my);
                if i == 0 && j == 0 {
                    page.gray(0.0);
                    page.dot(cx, cy, ORIGIN_RING_R);
                    page.gray(1.0);
                    page.dot(cx, cy, ORIGIN_RING_HOLE_R);
                }
                page.gray(0.0);
                page.dot(cx, cy, RING_R);
                page.gray(1.0);
                page.dot(cx, cy, RING_HOLE_R);
                page.gray(0.0);
                page.dot(cx, cy, DOT_R);
                for upper in [false, true] {
                    let v = if upper { CODE_DY } else { -CODE_DY };
                    for k in 0..SLOTS {
                        if slot_set(i, j, upper, k) {
                            page.dot(cx + slot_u(k), cy + v, CODE_R);
                        }
                    }
                }
                page.gray(0.45);
                page.text(
                    cx + 10.0,
                    cy + 15.5,
                    2.0,
                    &format!("{mx:.0}, {my:.0}"),
                    Anchor::Left,
                    0.0,
                    false,
                );
            }
        }
        // Cut lines, with the band beyond them the waste: the tile to
        // the right and the tile above lie over this one's last mark
        // column and row, which both tiles print.
        page.gray(0.3);
        if a >= 1 {
            let x = ox + tx * a as f64 - CUT;
            page.lines(&[[(x, oy + tb[1]), (x, oy + tb[3])]], 0.3, Some((3.0, 2.0)));
            page.text(
                x + 3.5,
                oy + (tb[1] + tb[3]) / 2.0,
                2.0,
                "cut along the dashed line and lay this edge over the tile to the left, its marks on that tile's",
                Anchor::Middle,
                90.0,
                false,
            );
        }
        if b >= 1 {
            let y = oy + ty * b as f64 - CUT;
            page.lines(&[[(ox + tb[0], y), (ox + tb[2], y)]], 0.3, Some((3.0, 2.0)));
            page.text(
                ox + (tb[0] + tb[2]) / 2.0,
                y + 3.5,
                2.0,
                "cut along the dashed line and lay this edge over the tile below, its marks on that tile's",
                Anchor::Middle,
                0.0,
                false,
            );
        }
        // The title band under the first mark row, and the print-scale
        // bar: on the bottom tiles it stays; above, it goes with the
        // waste once the tile is cut.
        page.gray(0.0);
        let band = oy + ty * b as f64;
        page.text(
            ox + tx * a as f64 - 20.0,
            band - 22.5,
            2.8,
            &format!(
                "offkilter measuring field · {} · tile {} of {}, column {} row {} · print at 100 % · the bar is 100 mm · origin at the double-ringed mark, x right, y up · each mark's dots say where it is",
                self.name(),
                b * self.nx + a + 1,
                self.nx * self.ny,
                a + 1,
                b + 1
            ),
            Anchor::Left,
            0.0,
            false,
        );
        let (bx, by) = (ox + tx * a as f64 - 20.0, band - 27.0);
        page.lines(
            &[
                [(bx, by), (bx + 100.0, by)],
                [(bx, by - 1.5), (bx, by + 1.5)],
                [(bx + 100.0, by - 1.5), (bx + 100.0, by + 1.5)],
            ],
            0.4,
            None,
        );
        page
    }
}

/// The field as one PDF, a page per tile, tile (0, 0) first and then
/// along each row.
pub fn field_pdf(field: &Field) -> Vec<u8> {
    let mut pages = Vec::new();
    for b in 0..field.ny {
        for a in 0..field.nx {
            pages.push(field.tile_page(a, b));
        }
    }
    document(pages)
}

/// What is printed at (x, y) printed millimetres of the field: the
/// grid, a mark's ink, or the paper (None).
pub fn field_colour(field: &Field, x: f64, y: f64) -> Option<Rgb> {
    let b = field.bounds();
    if x < b[0] || x > b[2] || y < b[1] || y > b[3] {
        return None;
    }
    let mut c = None;
    if ((x / 10.0).round() * 10.0 - x).abs() < 0.12 || ((y / 10.0).round() * 10.0 - y).abs() < 0.12
    {
        c = Some(Rgb(200, 200, 200));
    }
    let (cols, rows) = (field.columns() as i64, field.rows() as i64);
    let (i, j) = ((x / PITCH).round() as i64, (y / PITCH).round() as i64);
    if (0..cols).contains(&i) && (0..rows).contains(&j) {
        let (mx, my) = (i as f64 * PITCH, j as f64 * PITCH);
        let r = ((x - mx).powi(2) + (y - my).powi(2)).sqrt();
        let origin = i == 0 && j == 0;
        let black = r <= DOT_R
            || (RING_HOLE_R..=RING_R).contains(&r)
            || (origin && (ORIGIN_RING_HOLE_R..=ORIGIN_RING_R).contains(&r));
        let white =
            (r < RING_HOLE_R && r > DOT_R) || (origin && r > RING_R && r < ORIGIN_RING_HOLE_R);
        if black {
            c = Some(Rgb(0, 0, 0));
        } else if white {
            c = Some(Rgb(255, 255, 255));
        }
        for upper in [false, true] {
            let v = if upper { CODE_DY } else { -CODE_DY };
            for k in 0..SLOTS {
                if slot_set(i as usize, j as usize, upper, k)
                    && ((x - mx - slot_u(k)).powi(2) + (y - my - v).powi(2)).sqrt() <= CODE_R
                {
                    c = Some(Rgb(0, 0, 0));
                }
            }
        }
    }
    c
}

/// The taped field as a raster at `s` px per true mm, y down, as a
/// scanner would see it, with the scene lying on it: the field's own
/// features in printed millimetres (`scene.print`, `scene.stretch`),
/// the parts in true ones. The origin mark sits at field (0, 0).
pub fn field_image(field: &Field, s: f64, scene: &Scene) -> Image {
    let b = field.bounds();
    let (px, py) = (scene.print * scene.stretch, scene.print);
    let (pw, ph) = (
        ((b[2] - b[0]) * px * s) as usize,
        ((b[3] - b[1]) * py * s) as usize,
    );
    let mut img = Image::new(pw, ph, Rgb(255, 255, 255));
    for jj in 0..ph {
        for ii in 0..pw {
            let xt = b[0] * px + (ii as f64 + 0.5) / s;
            let yt = b[3] * py - (jj as f64 + 0.5) / s;
            let mut c = field_colour(field, xt / px, yt / py).unwrap_or(Rgb(255, 255, 255));
            if let Some(part) = scene_at(scene, xt, yt) {
                c = part;
            }
            img.set(ii, jj, c);
        }
    }
    img
}

/// A mark read in a picture: its lattice position, where its centre
/// is in pixels, and the picture's scale there.
#[derive(Debug, Clone, Copy)]
pub(super) struct MarkPx {
    pub i: usize,
    pub j: usize,
    pub x: f64,
    pub y: f64,
}

/// A mark's ink as elements, each `[cx, cy, r_in, r_out]` in printed
/// millimetres (a dot's inner radius is 0), `slack` mm bigger than
/// printed: the centre dot, the ring, the origin's extra ring, and the
/// code dots. For a mark that read, its own code dots; for one that
/// did not, every slot, since erasing bare paper costs nothing.
pub(super) fn ink_elements(
    mx: f64,
    my: f64,
    origin: bool,
    code: Option<(usize, usize)>,
    slack: f64,
) -> Vec<[f64; 4]> {
    let mut v = vec![
        [mx, my, 0.0, DOT_R + slack],
        [mx, my, RING_HOLE_R - slack, RING_R + slack],
    ];
    if origin {
        v.push([mx, my, ORIGIN_RING_HOLE_R - slack, ORIGIN_RING_R + slack]);
    }
    for upper in [false, true] {
        let dy = if upper { CODE_DY } else { -CODE_DY };
        for k in 0..SLOTS {
            let set = match code {
                Some((i, j)) => slot_set(i, j, upper, k),
                None => true,
            };
            if set {
                v.push([mx + slot_u(k), my + dy, 0.0, CODE_R + slack]);
            }
        }
    }
    v
}

/// The second moment of the ring about its centre, per axis, in mm2:
/// (R2² + R1²) / 4 for a uniform annulus. A ring seen as an ellipse
/// has moments this times the square of the pixels per millimetre
/// along each of its axes.
const RING_VARIANCE: f64 = (RING_R * RING_R + RING_HOLE_R * RING_HOLE_R) / 4.0;

/// Every mark in the thresholded picture whose code reads: a bullseye
/// (a ring with a dot at its centre) with the lower code row's two
/// anchor dots the right distance apart and the right way off the
/// centre. The ring is an ellipse in an oblique picture; its moments
/// give the pixels per millimetre along its two axes, and every dot
/// round it is unwarped by that back onto the circle, where the
/// anchors and the centre give the mark's own frame (which perspective
/// barely bends over 11 mm) and each dot falls into a slot of it or is
/// ignored. Two marks reading the same place are both dropped.
pub(super) fn find_marks(dark: &[bool], w: usize, h: usize) -> Vec<MarkPx> {
    let comps = components(dark, w, h);
    let is_ring = |c: &Comp| {
        c.area >= 40 && {
            let aspect = c.width() as f64 / c.height() as f64;
            (0.2..=5.0).contains(&aspect) && (0.08..=0.8).contains(&c.fill())
        }
    };
    let is_dot = |c: &Comp| c.area >= 4 && c.fill() > 0.5;
    let concentric = |a: &Comp, b: &Comp| {
        a.contains(b)
            && ((a.cx - b.cx).powi(2) + (a.cy - b.cy).powi(2)).sqrt()
                < 0.2 * a.width().max(a.height()) as f64
    };
    // Bullseyes as (ring, dot); the origin's outer ring would claim the
    // same dot, so the smallest ring round a dot is the one kept.
    let mut bulls: Vec<(usize, usize)> = Vec::new();
    for (ai, a) in comps.iter().enumerate() {
        if !is_ring(a) {
            continue;
        }
        let dot = comps
            .iter()
            .enumerate()
            .filter(|(_, b)| is_dot(b) && concentric(a, b) && b.area * 2 < a.area)
            .max_by_key(|(_, b)| b.area);
        if let Some((di, _)) = dot {
            bulls.push((ai, di));
        }
    }
    bulls.sort_by_key(|&(ai, _)| comps[ai].area);
    let mut seen_dots = Vec::new();
    bulls.retain(|&(_, di)| {
        if seen_dots.contains(&di) {
            false
        } else {
            seen_dots.push(di);
            true
        }
    });
    let span = (SLOTS - 1) as f64 * SLOT_PITCH;
    let mut out: Vec<MarkPx> = Vec::new();
    for (ai, di) in bulls {
        let (ring, d) = (&comps[ai], &comps[di]);
        let c = [d.cx, d.cy];
        // The ring as an ellipse: pixels per millimetre along its major
        // and minor axes, and the angle of the major one. Unwarping by
        // that puts the mark's surroundings back on the circle, in
        // millimetres, orientation kept (the map does not flip).
        let (l1, l2, ang) = ring.axes();
        let (sa, sb) = ((l1 / RING_VARIANCE).sqrt(), (l2 / RING_VARIANCE).sqrt());
        if sb < 0.25 || sa < 0.25 {
            continue;
        }
        let (ca, sn) = (ang.cos(), ang.sin());
        let unwarp = |dx: f64, dy: f64| -> [f64; 2] {
            [(dx * ca + dy * sn) / sa, (-dx * sn + dy * ca) / sb]
        };
        let dot_area = PI * CODE_R * CODE_R * sa * sb;
        let at = |k: usize| unwarp(comps[k].cx - c[0], comps[k].cy - c[1]);
        let cands: Vec<usize> = comps
            .iter()
            .enumerate()
            .filter(|&(k, b)| {
                k != di && k != ai && b.fill() > 0.4 && {
                    let a = b.area as f64 / dot_area;
                    let [u, v] = unwarp(b.cx - c[0], b.cy - c[1]);
                    let r = (u * u + v * v).sqrt();
                    (0.2..=5.0).contains(&a) && r > 7.0 && r < 20.0
                }
            })
            .map(|(k, _)| k)
            .collect();
        // The anchors: the pair with the right span whose midpoint sits
        // the right distance off the centre, square to the pair.
        let mut best: Option<(f64, usize, usize)> = None;
        for p in 0..cands.len() {
            for q in p + 1..cands.len() {
                let (a, b) = (at(cands[p]), at(cands[q]));
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let dpq = (dx * dx + dy * dy).sqrt();
                let e1 = (dpq / span - 1.0).abs();
                if e1 > 0.2 {
                    continue;
                }
                let off = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
                let dm = (off[0] * off[0] + off[1] * off[1]).sqrt();
                let e2 = (dm / CODE_DY - 1.0).abs();
                if e2 > 0.3 {
                    continue;
                }
                let cosine = ((dx * off[0] + dy * off[1]) / (dpq * dm)).abs();
                if cosine > 0.35 {
                    continue;
                }
                let score = e1 + e2 + cosine;
                if best.is_none_or(|(sc, _, _)| score < sc) {
                    best = Some((score, cands[p], cands[q]));
                }
            }
        }
        let Some((_, p, q)) = best else {
            continue;
        };
        // The mark's frame on the unwarped circle: `ex` per millimetre
        // along the lower row, `ey` per millimetre from that row up
        // through the centre. Seen from above, x to y turns clockwise in
        // a picture (y down), and the unwarp keeps that, which fixes
        // which way along the row is +x.
        let (pa, pb) = (at(p), at(q));
        let m = [(pa[0] + pb[0]) / 2.0, (pa[1] + pb[1]) / 2.0];
        let mut ex = [(pb[0] - pa[0]) / span, (pb[1] - pa[1]) / span];
        let ey = [-m[0] / CODE_DY, -m[1] / CODE_DY];
        if ex[0] * ey[1] - ex[1] * ey[0] > 0.0 {
            ex = [-ex[0], -ex[1]];
        }
        let det = ex[0] * ey[1] - ex[1] * ey[0];
        let mut set = [[false; SLOTS]; 2];
        for &k in &cands {
            let o = at(k);
            let u = (o[0] * ey[1] - o[1] * ey[0]) / det;
            let v = (ex[0] * o[1] - ex[1] * o[0]) / det;
            let row = if (v + CODE_DY).abs() < 2.0 {
                0
            } else if (v - CODE_DY).abs() < 2.0 {
                1
            } else {
                continue;
            };
            let slot = ((u - slot_u(0)) / SLOT_PITCH).round();
            if slot < 0.0 || slot >= SLOTS as f64 {
                continue;
            }
            let slot = slot as usize;
            if (u - slot_u(slot)).abs() < 1.6 {
                set[row][slot] = true;
            }
        }
        if !(set[0][0] && set[0][SLOTS - 1]) || set[1][0] || set[1][SLOTS - 1] {
            continue;
        }
        // Every slot of both rows must lie inside the picture: a mark
        // cut by its edge could be missing dots.
        let inside = (0..SLOTS).all(|k| {
            [-CODE_DY, CODE_DY].iter().all(|&v| {
                let u = slot_u(k);
                let o = [u * ex[0] + v * ey[0], u * ex[1] + v * ey[1]];
                let (dx, dy) = (
                    o[0] * sa * ca - o[1] * sb * sn,
                    o[0] * sa * sn + o[1] * sb * ca,
                );
                let (px, py) = (c[0] + dx, c[1] + dy);
                px > 2.0 && py > 2.0 && px < w as f64 - 3.0 && py < h as f64 - 3.0
            })
        });
        if !inside {
            continue;
        }
        let value = |row: usize| (1..=CODE_BITS).fold(0, |v, k| (v << 1) | set[row][k] as usize);
        out.push(MarkPx {
            i: value(0),
            j: value(1),
            x: c[0],
            y: c[1],
        });
    }
    let ids: Vec<(usize, usize)> = out.iter().map(|m| (m.i, m.j)).collect();
    out.retain(|m| ids.iter().filter(|&&id| id == (m.i, m.j)).count() == 1);
    out
}

/// The homography taking each `from` point to its `to` point, fitted
/// to all of them by least squares (both sets normalised first), for
/// four or more.
pub fn homography_fit(from: &[[f64; 2]], to: &[[f64; 2]]) -> Result<H, String> {
    if from.len() < 4 || from.len() != to.len() {
        return Err("a homography needs four or more point pairs".into());
    }
    let norm = |pts: &[[f64; 2]]| -> (H, Vec<[f64; 2]>) {
        let n = pts.len() as f64;
        let (cx, cy) = (
            pts.iter().map(|p| p[0]).sum::<f64>() / n,
            pts.iter().map(|p| p[1]).sum::<f64>() / n,
        );
        let mean_d = pts
            .iter()
            .map(|p| ((p[0] - cx).powi(2) + (p[1] - cy).powi(2)).sqrt())
            .sum::<f64>()
            / n;
        let k = if mean_d > 1e-12 {
            2f64.sqrt() / mean_d
        } else {
            1.0
        };
        let t = [k, 0.0, -k * cx, 0.0, k, -k * cy, 0.0, 0.0, 1.0];
        (t, pts.iter().map(|p| apply(&t, *p)).collect())
    };
    let (tf, nf) = norm(from);
    let (tt, nt) = norm(to);
    // Normal equations for the eight unknowns, h[8] = 1.
    let mut ata = [[0.0f64; 8]; 8];
    let mut atb = [0.0f64; 8];
    for (f, t) in nf.iter().zip(&nt) {
        let ([u, v], [x, y]) = (*f, *t);
        let rows = [
            ([u, v, 1.0, 0.0, 0.0, 0.0, -x * u, -x * v], x),
            ([0.0, 0.0, 0.0, u, v, 1.0, -y * u, -y * v], y),
        ];
        for (row, rhs) in rows {
            for r in 0..8 {
                for c in 0..8 {
                    ata[r][c] += row[r] * row[c];
                }
                atb[r] += row[r] * rhs;
            }
        }
    }
    let mut a: Vec<[f64; 9]> = (0..8)
        .map(|r| {
            let mut row = [0.0; 9];
            row[..8].copy_from_slice(&ata[r]);
            row[8] = atb[r];
            row
        })
        .collect();
    for col in 0..8 {
        let pivot = (col..8)
            .max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))
            .unwrap();
        if a[pivot][col].abs() < 1e-12 {
            return Err("the marks do not pin the picture down: they lie along a line".into());
        }
        a.swap(col, pivot);
        for row in 0..8 {
            if row != col {
                let f = a[row][col] / a[col][col];
                let pivot_row = a[col];
                for (x, p) in a[row].iter_mut().zip(pivot_row).skip(col) {
                    *x -= f * p;
                }
            }
        }
    }
    let mut hn = [0.0; 9];
    for (k, row) in a.iter().enumerate() {
        hn[k] = row[8] / row[k];
    }
    hn[8] = 1.0;
    // Back to the original frames: H = Tt^-1 · Hn · Tf.
    let h = mul3(&mul3(&invert(&tt)?, &hn), &tf);
    Ok(h.map(|v| v / h[8]))
}

fn mul3(a: &H, b: &H) -> H {
    let mut out = [0.0; 9];
    for r in 0..3 {
        for c in 0..3 {
            out[r * 3 + c] = (0..3).map(|k| a[r * 3 + k] * b[k * 3 + c]).sum();
        }
    }
    out
}

/// A homography scaled so its target frame counts `k` units per unit.
pub(super) fn scaled(h: &H, k: f64) -> H {
    let mut out = *h;
    for v in out.iter_mut().take(6) {
        *v *= k;
    }
    out
}

/// What reading a field in a picture gave.
#[derive(Debug, Clone, Serialize)]
pub struct FieldRead {
    /// The lattice pitch, in millimetres.
    pub pitch: f64,
    /// Marks whose codes read, and how many of those the fit kept.
    pub marks_read: usize,
    pub marks_used: usize,
    /// The marks kept: column, row, and where each sits in the photo.
    pub marks: Vec<MarkRead>,
    /// The field millimetres the rectified picture covers: x0, y0, x1, y1.
    pub bounds: [f64; 4],
}

#[derive(Debug, Clone, Serialize)]
pub struct MarkRead {
    pub column: usize,
    pub row: usize,
    pub pixel: [f64; 2],
    /// How far the fit puts it from where it was seen, in photo pixels.
    pub residual: f64,
}

/// Fits the read marks to the lattice, dropping the worst until all
/// agree: a mark whose code misread lands far from where its code
/// says. Returns the picture-to-field homography, the marks kept with
/// their residuals, and the fit's root mean square residual.
pub(super) fn fit_marks(marks: &[MarkPx], f: f64) -> Result<(H, Vec<MarkRead>, f64), String> {
    let mut used: Vec<usize> = (0..marks.len()).collect();
    loop {
        let from: Vec<[f64; 2]> = used
            .iter()
            .map(|&k| [marks[k].x * f, marks[k].y * f])
            .collect();
        let to: Vec<[f64; 2]> = used
            .iter()
            .map(|&k| Field::mark(marks[k].i, marks[k].j))
            .collect();
        let distinct = |pick: fn(&MarkPx) -> usize| {
            let mut v: Vec<usize> = used.iter().map(|&k| pick(&marks[k])).collect();
            v.sort_unstable();
            v.dedup();
            v.len()
        };
        if used.len() < 4 || distinct(|m| m.i) < 2 || distinct(|m| m.j) < 2 {
            return Err(format!(
                "only {} marks of the field read in agreement; four in at least two columns and two rows are needed",
                used.len()
            ));
        }
        let h = homography_fit(&from, &to)?;
        let back = invert(&h)?;
        let res: Vec<f64> = from
            .iter()
            .zip(&to)
            .map(|(p, m)| {
                let q = apply(&back, *m);
                ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2)).sqrt()
            })
            .collect();
        let mut sorted = res.clone();
        sorted.sort_by(f64::total_cmp);
        let median = sorted[sorted.len() / 2];
        let limit = (3.0 * median).max(2.0 * f);
        let (worst, &wr) = res
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap();
        if wr > limit && used.len() > 4 {
            used.remove(worst);
            continue;
        }
        let rms = (res.iter().map(|r| r * r).sum::<f64>() / res.len() as f64).sqrt();
        let kept = used
            .iter()
            .zip(&res)
            .map(|(&k, &r)| MarkRead {
                column: marks[k].i,
                row: marks[k].j,
                pixel: [marks[k].x * f, marks[k].y * f],
                residual: r,
            })
            .collect();
        return Ok((h, kept, rms));
    }
}
