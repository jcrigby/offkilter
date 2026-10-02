//! Shop drawing sheets. The bodies of a tab are projected into the
//! standard views (front, top, right, isometric) with hidden lines
//! removed, laid out third-angle at the largest standard scale that fits
//! the sheet, given overall dimensions, diameter callouts for holes and
//! bosses seen end-on, a balloon per part and a parts list for an
//! assembly, and a title block; then written as a vector PDF.
//!
//! The layout matches the web client's drawing dialog, so a sheet asked
//! for from a script or a model looks like the one a person downloads.

pub mod pdf;

use ok_brep::{project_view, Solid, Surface, View, ViewArc, ViewLines};
use ok_math::{Plane, Vec2, Vec3};
use ok_model::{Document, TabId};
pub use pdf::Anchor;

/// Sheet sizes, landscape, millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetSize {
    A4,
    A3,
    A2,
    Letter,
    Tabloid,
}

impl SheetSize {
    pub fn parse(name: &str) -> Result<SheetSize, String> {
        match name.trim().to_lowercase().as_str() {
            "" | "a4" => Ok(SheetSize::A4),
            "a3" => Ok(SheetSize::A3),
            "a2" => Ok(SheetSize::A2),
            "letter" => Ok(SheetSize::Letter),
            "tabloid" | "ledger" | "11x17" => Ok(SheetSize::Tabloid),
            other => Err(format!(
                "unknown sheet size {other:?}: use A4, A3, A2, Letter or Tabloid"
            )),
        }
    }

    pub fn size(self) -> (f64, f64) {
        match self {
            SheetSize::A4 => (297.0, 210.0),
            SheetSize::A3 => (420.0, 297.0),
            SheetSize::A2 => (594.0, 420.0),
            SheetSize::Letter => (279.4, 215.9),
            SheetSize::Tabloid => (431.8, 279.4),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            SheetSize::A4 => "A4",
            SheetSize::A3 => "A3",
            SheetSize::A2 => "A2",
            SheetSize::Letter => "Letter",
            SheetSize::Tabloid => "Tabloid",
        }
    }
}

/// What to put on the sheet.
#[derive(Debug, Clone)]
pub struct Options {
    /// View names in `front`, `top`, `right`, `iso`, `section` (a cut
    /// parallel to the front view) and `section-side` (parallel to the
    /// right view), the two sections taking `@<mm>` for where the cut
    /// goes (through the middle of the bodies otherwise); the layout
    /// places the first three third-angle and the rest to the right.
    pub views: Vec<String>,
    pub sheet: SheetSize,
    /// The title block's first line.
    pub title: String,
    /// A second line for the title block's right half (a date, an
    /// author); the kernel has no clock, so the caller supplies it.
    pub note: String,
    /// Balloons and a parts list.
    pub parts: bool,
    /// Hidden lines, dashed. Absent: on a sheet of one part, off on a
    /// sheet of several (an assembly), where they only clutter.
    pub hidden: Option<bool>,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            views: ["front", "top", "right", "iso"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            sheet: SheetSize::A4,
            title: String::new(),
            note: String::new(),
            parts: true,
            hidden: None,
        }
    }
}

/// An item on the sheet: its solids (one for a body, every placed body
/// for a sub-assembly instance), what the parts list calls it, and a key
/// that groups identical items into one row with a quantity.
pub struct Part<'a> {
    pub name: String,
    pub material: String,
    pub solids: Vec<&'a Solid>,
    pub key: (u32, usize),
}

/// Volume-weighted centroid of several solids (a balloon's anchor).
fn centroid_of(solids: &[&Solid]) -> Option<Vec3> {
    let mut sum = Vec3::ZERO;
    let mut total = 0.0;
    for s in solids {
        if let Some(c) = s.centroid() {
            let v = s.volume().abs();
            sum += c * v;
            total += v;
        }
    }
    (total > 0.0).then(|| sum * (1.0 / total))
}

/// The view directions the client uses: the viewer looks along `dir`.
pub fn standard_view(name: &str) -> Option<View> {
    let z = Vec3::Z;
    match name {
        "front" => Some(View {
            dir: Vec3::new(0.0, 1.0, 0.0),
            up: z,
        }),
        "top" => Some(View {
            dir: Vec3::new(0.0, 0.0, -1.0),
            up: Vec3::new(0.0, 1.0, 0.0),
        }),
        "right" => Some(View {
            dir: Vec3::new(-1.0, 0.0, 0.0),
            up: z,
        }),
        "iso" => Some(View {
            dir: Vec3::new(-0.6, 0.7, -0.5),
            up: z,
        }),
        _ => None,
    }
}

/// A section view: which axis the cutting plane is normal to, and
/// where along it. `section` cuts parallel to the front view (a plane
/// y = at, the near side removed, seen from the front); `section-side`
/// parallel to the right view (x = at, the +x side removed, seen from
/// the right). `@<mm>` places the cut; without it the cut goes through
/// the middle of the bodies.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SectionSpec {
    pub axis: char,
    pub at: Option<f64>,
}

pub fn section_spec(name: &str) -> Result<Option<SectionSpec>, String> {
    let (base, at) = match name.split_once('@') {
        Some((b, a)) => {
            let at: f64 = a.trim().parse().map_err(|_| {
                format!("view {name:?}: the cut position after @ must be a number of millimetres")
            })?;
            (b, Some(at))
        }
        None => (name, None),
    };
    Ok(match base {
        "section" => Some(SectionSpec { axis: 'y', at }),
        "section-side" => Some(SectionSpec { axis: 'x', at }),
        _ => None,
    })
}

impl SectionSpec {
    /// The view direction of the section, and the cutting plane for
    /// `ok_brep::section_view` (its normal side is removed) at `at`.
    fn view_and_plane(self, at: f64) -> (View, Plane) {
        match self.axis {
            'y' => (
                standard_view("front").unwrap(),
                Plane::from_origin_normal(Vec3::new(0.0, at, 0.0), Vec3::new(0.0, -1.0, 0.0))
                    .unwrap(),
            ),
            _ => (
                standard_view("right").unwrap(),
                Plane::from_origin_normal(Vec3::new(at, 0.0, 0.0), Vec3::X).unwrap(),
            ),
        }
    }
}

/// The screen basis of a view as the kernel projects it.
fn frame(view: View) -> (Vec3, Vec3) {
    let d = view.dir.normalized().unwrap_or(Vec3::Y);
    let u = d.cross(view.up).normalized().unwrap_or(Vec3::X);
    (u, u.cross(d))
}

#[derive(Debug, Clone)]
struct Callout {
    centre: Vec2,
    radius: f64,
    count: usize,
    hole: bool,
    angle: f64,
}

#[derive(Debug, Clone, Copy)]
struct Bounds {
    minx: f64,
    miny: f64,
    maxx: f64,
    maxy: f64,
}

#[derive(Debug, Clone)]
struct Placed {
    name: String,
    lines: ViewLines,
    callouts: Vec<Callout>,
    /// Item balloons: number and anchor in view coordinates.
    balloons: Vec<(usize, Vec2)>,
    /// A section's cut faces (hatched), its letter, and where its
    /// cutting plane shows edge-on: the view, whether the trace runs
    /// horizontally there, its coordinate, and which way the arrows
    /// point (the direction of sight) as a unit vector in that view.
    cut: Vec<Vec<Vec2>>,
    section: Option<SectionMark>,
    /// A caption under the view (a mechanism's position).
    caption: Option<String>,
    dx: f64,
    dy: f64,
    b: Bounds,
}

#[derive(Debug, Clone)]
struct SectionMark {
    letter: char,
    on: String,
    horizontal: bool,
    at: f64,
    sight: Vec2,
}

struct Dimension {
    a: Vec2,
    b: Vec2,
    offset: f64,
    value: f64,
}

struct Row {
    item: usize,
    name: String,
    qty: usize,
    material: String,
    /// Index into the parts of the row's first body.
    body: usize,
}

const DIM_OFFSET: f64 = 10.0;
const DIM_OVERSHOOT: f64 = 2.0;
const TABLE_COLS: [(&str, f64); 4] = [
    ("ITEM", 12.0),
    ("PART", 60.0),
    ("QTY", 12.0),
    ("MATERIAL", 36.0),
];
const TABLE_ROW: f64 = 6.0;
const BALLOON_R: f64 = 3.5;
/// Sheet mm between a view's box and the balloon rims around it.
const BALLOON_GAP: f64 = 8.0;
const STANDARD_SCALES: [f64; 11] = [10.0, 5.0, 2.0, 1.0, 0.5, 0.4, 0.2, 0.1, 0.05, 0.02, 0.01];
/// Sheet millimetres kept around the views for a section's caption
/// below it and the trace letters beside the view it is drawn on.
const SECTION_ROOM: f64 = 8.0;
const MARGIN: f64 = 10.0;
const BLOCK: f64 = 24.0;

/// A point of a view arc at parameter `t`.
fn arc_point(a: &ViewArc, t: f64) -> Vec2 {
    let minor = Vec2::new(-a.major.y * a.ratio, a.major.x * a.ratio);
    a.center + a.major * t.cos() + minor * t.sin()
}

/// An arc as chords at most 5° apart.
fn arc_chords(a: &ViewArc) -> Vec<[Vec2; 2]> {
    let n = (((a.end - a.start) / (std::f64::consts::PI / 36.0)).ceil() as usize).max(1);
    let mut out = Vec::with_capacity(n);
    let mut prev = arc_point(a, a.start);
    for i in 1..=n {
        let p = arc_point(a, a.start + (a.end - a.start) * i as f64 / n as f64);
        out.push([prev, p]);
        prev = p;
    }
    out
}

/// Hatch lines at 45 degrees, `spacing` apart, filling the polygons by
/// the even-odd rule (a face with holes comes as several loops), in
/// the polygons' own coordinates.
fn hatch(loops: &[Vec<(f64, f64)>], spacing: f64) -> Vec<[(f64, f64); 2]> {
    // Lines x - y = c; along a line, t = x + y.
    let mut out = Vec::new();
    let (mut cmin, mut cmax) = (f64::INFINITY, f64::NEG_INFINITY);
    for l in loops {
        for &(x, y) in l {
            cmin = cmin.min(x - y);
            cmax = cmax.max(x - y);
        }
    }
    if !cmin.is_finite() {
        return out;
    }
    let step = spacing * std::f64::consts::SQRT_2;
    let mut c = (cmin / step).floor() * step + step / 2.0;
    while c < cmax {
        let mut ts: Vec<f64> = Vec::new();
        for l in loops {
            let n = l.len();
            for i in 0..n {
                let (p, q) = (l[i], l[(i + 1) % n]);
                let (cp, cq) = (p.0 - p.1 - c, q.0 - q.1 - c);
                if (cp < 0.0) == (cq < 0.0) {
                    continue; // both sides equal, or both on: no crossing
                }
                let f = cp / (cp - cq);
                let x = p.0 + (q.0 - p.0) * f;
                let y = p.1 + (q.1 - p.1) * f;
                ts.push(x + y);
            }
        }
        ts.sort_by(f64::total_cmp);
        for pair in ts.chunks(2) {
            if pair.len() == 2 && pair[1] - pair[0] > 1e-6 {
                let at = |t: f64| ((t + c) / 2.0, (t - c) / 2.0);
                out.push([at(pair[0]), at(pair[1])]);
            }
        }
        c += step;
    }
    out
}

fn bounds_of(v: &ViewLines) -> Bounds {
    let mut b = Bounds {
        minx: f64::INFINITY,
        miny: f64::INFINITY,
        maxx: f64::NEG_INFINITY,
        maxy: f64::NEG_INFINITY,
    };
    let arcs: Vec<[Vec2; 2]> = v
        .visible_arcs
        .iter()
        .chain(&v.hidden_arcs)
        .flat_map(arc_chords)
        .collect();
    for [p, q] in v.visible.iter().chain(&v.hidden).chain(&arcs) {
        for r in [p, q] {
            b.minx = b.minx.min(r.x);
            b.miny = b.miny.min(r.y);
            b.maxx = b.maxx.max(r.x);
            b.maxy = b.maxy.max(r.y);
        }
    }
    if b.minx.is_finite() {
        b
    } else {
        Bounds {
            minx: 0.0,
            miny: 0.0,
            maxx: 0.0,
            maxy: 0.0,
        }
    }
}

/// Diameter callouts of a view: every cylinder seen end-on as a circle,
/// alike sizes counted on one callout, concentric ones fanned out.
fn callouts(solids: &[&Solid], view: View) -> Vec<Callout> {
    let dir = view.dir.normalized().unwrap_or(Vec3::Y);
    let (u, v) = frame(view);
    let mut circles: Vec<(Vec2, f64, bool)> = Vec::new();
    for solid in solids {
        for (si, surface) in solid.surfaces.iter().enumerate() {
            let Surface::Cylinder {
                origin,
                axis,
                radius,
            } = surface
            else {
                continue;
            };
            if axis.dot(dir).abs() < 0.999 {
                continue;
            }
            let Some(face) = solid.faces.iter().find(|f| f.surface == si) else {
                continue;
            };
            let pts: Vec<Vec3> = face.loops[0]
                .iter()
                .map(|&k| solid.vertices[k as usize])
                .collect();
            let c = pts.iter().fold(Vec3::ZERO, |a, &p| a + p) * (1.0 / pts.len().max(1) as f64);
            let d = c - *origin;
            let radial = d - *axis * d.dot(*axis);
            let hole = face.plane.normal.dot(radial) < 0.0;
            let centre = Vec2::new(origin.dot(u), origin.dot(v));
            if circles
                .iter()
                .any(|(o, r, _)| (r - radius).abs() < 1e-6 && o.distance(centre) < 1e-6)
            {
                continue;
            }
            circles.push((centre, *radius, hole));
        }
    }
    let mut out: Vec<Callout> = Vec::new();
    for (centre, radius, hole) in circles {
        if let Some(same) = out
            .iter_mut()
            .find(|o| (o.radius - radius).abs() < 1e-6 && o.hole == hole)
        {
            same.count += 1;
            if centre.y < same.centre.y - 1e-9
                || ((centre.y - same.centre.y).abs() < 1e-9 && centre.x < same.centre.x)
            {
                same.centre = centre;
            }
        } else {
            out.push(Callout {
                centre,
                radius,
                count: 1,
                hole,
                angle: 45.0,
            });
        }
    }
    out.sort_by(|a, b| a.radius.total_cmp(&b.radius));
    let angles = [45.0, 135.0, -45.0, -135.0];
    for i in 0..out.len() {
        let near = out[..i]
            .iter()
            .filter(|o| o.centre.distance(out[i].centre) < o.radius + out[i].radius + 12.0)
            .count();
        out[i].angle = angles[near % angles.len()];
    }
    out
}

fn dim_text(v: f64) -> String {
    let s = format!("{:.2}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() {
        "0".into()
    } else {
        s.into()
    }
}

fn scale_label(s: f64) -> String {
    if s >= 1.0 {
        format!("{}:1", s.round() as i64)
    } else if ((1.0 / s) - (1.0 / s).round()).abs() < 1e-9 {
        format!("1:{}", (1.0 / s).round() as i64)
    } else {
        format!("1:{:.1}", 1.0 / s)
    }
}

/// The drawing of some parts: views placed, dimensions and the sheet
/// frame worked out. `write_pdf` renders it.
pub struct Sheet {
    size: SheetSize,
    placed: Vec<Placed>,
    dims: Vec<Dimension>,
    rows: Vec<Row>,
    scale: f64,
    /// Sheet = (ox + x * scale, oy + y * scale), y up.
    ox: f64,
    oy: f64,
    title: String,
    note: String,
    hidden: bool,
}

/// Places the views (third angle, the rest to the right of the
/// elevations or, with `below`, in a row under them), works out the
/// overall dimensions, and fits the sheet at the largest standard scale
/// that keeps the views in the free area above the title block and off
/// the parts list of `rows`. Returns the placed views, the dimensions,
/// the scale and the sheet origin.
fn arrange(
    mut views: Vec<Placed>,
    rows: &[Row],
    sheet: SheetSize,
    below: bool,
) -> (Vec<Placed>, Vec<Dimension>, f64, f64, f64) {
    // Third-angle layout in model millimetres: front at the origin, top
    // above it, right to its right, the rest to the right of those.
    let gap = 15.0;
    let dim_gap = DIM_OFFSET + 8.0;
    let fb = views
        .iter()
        .find(|v| v.name == "front")
        .map(|v| v.b)
        .unwrap_or(Bounds {
            minx: 0.0,
            miny: 0.0,
            maxx: 0.0,
            maxy: 0.0,
        });
    let has_top = views.iter().any(|v| v.name == "top");
    let mut cursor_x = f64::NEG_INFINITY;
    for v in views.iter_mut() {
        match v.name.as_str() {
            "front" => {}
            "top" => v.dy = fb.maxy + gap + dim_gap - v.b.miny,
            "right" => v.dx = fb.maxx + gap - v.b.minx,
            _ => {}
        }
        if matches!(v.name.as_str(), "front" | "top" | "right") {
            cursor_x = cursor_x.max(v.b.maxx + v.dx);
        }
    }
    let mut cursor_x = if cursor_x.is_finite() {
        cursor_x + gap
    } else {
        0.0
    };
    // The row under the elevations starts at the front view's left
    // edge, below its width dimension.
    let row_top = fb.miny - gap - dim_gap;
    let mut row_x = fb.minx;
    for v in views.iter_mut() {
        if matches!(v.name.as_str(), "front" | "top" | "right") {
            continue;
        }
        if below {
            v.dx = row_x - v.b.minx;
            v.dy = row_top - v.b.maxy;
            row_x += v.b.maxx - v.b.minx + gap;
            continue;
        }
        v.dx = cursor_x - v.b.minx;
        v.dy = if v.name == "iso" && has_top {
            fb.maxy + gap + dim_gap - v.b.miny
        } else {
            -v.b.miny
        };
        cursor_x += v.b.maxx - v.b.minx + gap;
    }
    // Overall dimensions: width below the front view, height left of
    // it, depth left of the top view.
    let mut dims = Vec::new();
    for p in &views {
        let (w, h) = (p.b.maxx - p.b.minx, p.b.maxy - p.b.miny);
        let at = |x: f64, y: f64| Vec2::new(x + p.dx, y + p.dy);
        if p.name == "front" && w > 0.0 {
            dims.push(Dimension {
                a: at(p.b.minx, p.b.miny),
                b: at(p.b.maxx, p.b.miny),
                offset: -DIM_OFFSET,
                value: w,
            });
            if h > 0.0 {
                dims.push(Dimension {
                    a: at(p.b.minx, p.b.miny),
                    b: at(p.b.minx, p.b.maxy),
                    offset: DIM_OFFSET,
                    value: h,
                });
            }
        }
        if p.name == "top" && h > 0.0 {
            dims.push(Dimension {
                a: at(p.b.minx, p.b.miny),
                b: at(p.b.minx, p.b.maxy),
                offset: DIM_OFFSET,
                value: h,
            });
        }
    }
    let mut min = Vec2::new(f64::INFINITY, f64::INFINITY);
    let mut max = Vec2::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
    for p in &views {
        min.x = min.x.min(p.b.minx + p.dx);
        min.y = min.y.min(p.b.miny + p.dy);
        max.x = max.x.max(p.b.maxx + p.dx);
        max.y = max.y.max(p.b.maxy + p.dy);
    }
    if !min.x.is_finite() {
        min = Vec2::ZERO;
        max = Vec2::ZERO;
    }
    if !dims.is_empty() {
        min -= Vec2::new(dim_gap, dim_gap);
    }
    // Fit the sheet at the largest standard scale where the views sit
    // in the free area above the title block without covering the
    // parts list, which takes the bottom-right corner of that area:
    // centred when they can be, otherwise pushed to the top left.
    let (sw, sh) = sheet.size();
    let table = (!rows.is_empty()).then(|| {
        let w: f64 = TABLE_COLS.iter().map(|c| c.1).sum();
        (
            sw - MARGIN - w,
            MARGIN + BLOCK + (rows.len() as f64 + 1.0) * TABLE_ROW,
        )
    });
    // Balloons hang outside their view by a leader and a circle.
    let pad = if views.iter().any(|v| !v.balloons.is_empty()) {
        BALLOON_GAP + 2.0 * BALLOON_R
    } else {
        0.0
    };
    // Sections and captioned views need sheet room below and beside.
    let room = if views
        .iter()
        .any(|v| v.section.is_some() || v.caption.is_some())
    {
        SECTION_ROOM
    } else {
        0.0
    };
    let avail_w = sw - 2.0 * MARGIN - 2.0 * pad - 2.0 * room;
    let avail_h = sh - 2.0 * MARGIN - BLOCK - 2.0 * pad - 2.0 * room;
    let extent_w = (max.x - min.x).max(1e-9);
    let extent_h = (max.y - min.y).max(1e-9);
    // Whether a view's box, placed at scale `s` with offset (ox, oy),
    // stays off the parts list.
    let view_clear = |p: &Placed, s: f64, ox: f64, oy: f64| {
        let Some((tx, ty)) = table else { return true };
        let pad = if matches!(p.name.as_str(), "front" | "top") {
            dim_gap
        } else {
            0.0
        };
        let x1 = (p.b.maxx + p.dx) * s + ox + 2.0;
        let y0 = (p.b.miny + p.dy - pad) * s + oy - 2.0;
        x1 <= tx || y0 >= ty
    };
    let top_left = |s: f64| {
        (
            MARGIN + pad + room - min.x * s,
            sh - MARGIN - pad - room - max.y * s,
        )
    };
    let iso = views.iter().position(|v| v.name == "iso");
    let mut fit = None;
    'scales: for &s in &STANDARD_SCALES {
        if extent_w * s > avail_w || extent_h * s > avail_h {
            continue;
        }
        let centred = (
            MARGIN + pad + room + (avail_w - extent_w * s) / 2.0 - min.x * s,
            MARGIN + BLOCK + pad + room + (avail_h - extent_h * s) / 2.0 - min.y * s,
        );
        for (ox, oy) in [centred, top_left(s)] {
            if views.iter().all(|p| view_clear(p, s, ox, oy)) {
                fit = Some((s, ox, oy, 0.0));
                break 'scales;
            }
            // The iso's height is free: lift it off the list when it
            // is the only view in the way and the sheet has the room.
            let Some(k) = iso else { continue };
            let others = views
                .iter()
                .enumerate()
                .all(|(i, p)| i == k || view_clear(p, s, ox, oy));
            let Some((_, ty)) = table else { continue };
            let v = &views[k];
            let y0 = (v.b.miny + v.dy) * s + oy - 2.0;
            let lift = (ty - y0) / s;
            let top = (v.b.maxy + v.dy + lift) * s + oy;
            if others && lift > 0.0 && top <= sh - MARGIN - pad {
                fit = Some((s, ox, oy, lift));
                break 'scales;
            }
        }
    }
    let (scale, ox, oy, lift) = fit.unwrap_or_else(|| {
        let s = STANDARD_SCALES[STANDARD_SCALES.len() - 1];
        let (ox, oy) = top_left(s);
        (s, ox, oy, 0.0)
    });
    if let Some(k) = iso {
        views[k].dy += lift;
    }
    (views, dims, scale, ox, oy)
}

impl Sheet {
    /// Lays the parts out for the options.
    pub fn layout(parts: &[Part], opts: &Options) -> Result<Sheet, String> {
        let solids: Vec<&Solid> = parts
            .iter()
            .flat_map(|p| p.solids.iter().copied())
            .collect();
        if solids.is_empty() {
            return Err("nothing to draw: the tab has no bodies".into());
        }
        let mut keys: Vec<(u32, usize)> = parts.iter().map(|p| p.key).collect();
        keys.sort_unstable();
        keys.dedup();
        let one_part = keys.len() <= 1;
        let hidden = opts.hidden.unwrap_or(one_part);
        // Parts list rows: one per distinct part, in order of first appearance.
        let mut rows: Vec<Row> = Vec::new();
        if opts.parts && parts.len() > 1 {
            for (i, p) in parts.iter().enumerate() {
                if let Some(r) = rows.iter_mut().find(|r| parts[r.body].key == p.key) {
                    r.qty += 1;
                } else {
                    rows.push(Row {
                        item: rows.len() + 1,
                        name: p.name.clone(),
                        qty: 1,
                        material: p.material.clone(),
                        body: i,
                    });
                }
            }
        }
        // Views.
        let mut views: Vec<Placed> = Vec::new();
        let mut letters = 'A'..='Z';
        let (model_lo, model_hi) = {
            let mut lo = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
            let mut hi = -lo;
            for s in &solids {
                if let Some((a, b)) = s.bounds() {
                    lo = Vec3::new(lo.x.min(a.x), lo.y.min(a.y), lo.z.min(a.z));
                    hi = Vec3::new(hi.x.max(b.x), hi.y.max(b.y), hi.z.max(b.z));
                }
            }
            (lo, hi)
        };
        for name in &opts.views {
            if let Some(spec) = section_spec(name)? {
                let at = spec.at.unwrap_or(match spec.axis {
                    'y' => (model_lo.y + model_hi.y) / 2.0,
                    _ => (model_lo.x + model_hi.x) / 2.0,
                });
                let (view, plane) = spec.view_and_plane(at);
                let cut_lines = ok_brep::section_view(&solids, view, &plane);
                if cut_lines.cut.is_empty() {
                    return Err(format!(
                        "view {name:?}: the cut at {at} misses every body (they span {} to {} along {})",
                        if spec.axis == 'y' { model_lo.y } else { model_lo.x },
                        if spec.axis == 'y' { model_hi.y } else { model_hi.x },
                        spec.axis
                    ));
                }
                let lines = ViewLines {
                    visible: cut_lines.visible,
                    hidden: Vec::new(),
                    visible_arcs: cut_lines.visible_arcs,
                    hidden_arcs: Vec::new(),
                };
                let letter = letters.next().unwrap_or('Z');
                // Where the plane is edge-on: the top view by preference,
                // else the other elevation. Top view (u, v) = (x, y),
                // front (x, z), right (y, z).
                let has_top = opts.views.iter().any(|v| v == "top");
                let mark = match (spec.axis, has_top) {
                    ('y', true) => SectionMark {
                        letter,
                        on: "top".into(),
                        horizontal: true,
                        at,
                        sight: Vec2::new(0.0, 1.0),
                    },
                    ('y', false) => SectionMark {
                        letter,
                        on: "right".into(),
                        horizontal: false,
                        at,
                        sight: Vec2::new(1.0, 0.0),
                    },
                    (_, true) => SectionMark {
                        letter,
                        on: "top".into(),
                        horizontal: false,
                        at,
                        sight: Vec2::new(-1.0, 0.0),
                    },
                    (_, false) => SectionMark {
                        letter,
                        on: "front".into(),
                        horizontal: false,
                        at,
                        sight: Vec2::new(-1.0, 0.0),
                    },
                };
                let b = bounds_of(&lines);
                views.push(Placed {
                    name: name.clone(),
                    lines,
                    callouts: Vec::new(),
                    balloons: Vec::new(),
                    cut: cut_lines.cut,
                    section: Some(mark),
                    caption: None,
                    dx: 0.0,
                    dy: 0.0,
                    b,
                });
                continue;
            }
            let Some(view) = standard_view(name) else {
                return Err(format!(
                    "unknown view {name:?}: use front, top, right, iso, section or section-side (the sections take @<mm> for the cut)"
                ));
            };
            let lines = project_view(&solids, view);
            // Hole callouts belong on a part's own sheet: an assembly
            // view is a thicket of holes seen end-on.
            let callouts = if name == "iso" || !one_part {
                Vec::new()
            } else {
                callouts(&solids, view)
            };
            let balloons = if name == "iso" {
                let (u, v) = frame(view);
                rows.iter()
                    .filter_map(|r| {
                        centroid_of(&parts[r.body].solids)
                            .map(|c| (r.item, Vec2::new(c.dot(u), c.dot(v))))
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let b = bounds_of(&lines);
            views.push(Placed {
                name: name.clone(),
                lines,
                callouts,
                balloons,
                cut: Vec::new(),
                section: None,
                caption: None,
                dx: 0.0,
                dy: 0.0,
                b,
            });
        }
        Sheet::finish(views, rows, opts, hidden)
    }

    /// Lays out views projected by the caller, each with a name and an
    /// optional caption: a mechanism drawn at several positions.
    pub fn layout_views(
        views: Vec<(String, ViewLines, Option<String>)>,
        opts: &Options,
    ) -> Result<Sheet, String> {
        if views.is_empty() {
            return Err("nothing to draw".into());
        }
        let placed = views
            .into_iter()
            .map(|(name, lines, caption)| {
                let b = bounds_of(&lines);
                Placed {
                    name,
                    lines,
                    callouts: Vec::new(),
                    balloons: Vec::new(),
                    cut: Vec::new(),
                    section: None,
                    caption,
                    dx: 0.0,
                    dy: 0.0,
                    b,
                }
            })
            .collect();
        Sheet::finish(placed, Vec::new(), opts, opts.hidden.unwrap_or(false))
    }

    fn finish(
        views: Vec<Placed>,
        rows: Vec<Row>,
        opts: &Options,
        hidden: bool,
    ) -> Result<Sheet, String> {
        // The views beyond the elevations go to their right, or in a row
        // under them when that fits a larger scale (five views in one
        // row can force 1:5 where 1:2.5 fits two rows).
        let rest = views
            .iter()
            .filter(|v| !matches!(v.name.as_str(), "front" | "top" | "right"))
            .count();
        let right = arrange(views.clone(), &rows, opts.sheet, false);
        let (views, dims, scale, ox, oy) = if rest > 0 {
            let below = arrange(views, &rows, opts.sheet, true);
            if below.2 > right.2 {
                below
            } else {
                right
            }
        } else {
            right
        };
        Ok(Sheet {
            size: opts.sheet,
            placed: views,
            dims,
            rows,
            scale,
            ox,
            oy,
            title: opts.title.clone(),
            note: opts.note.clone(),
            hidden,
        })
    }

    pub fn scale(&self) -> f64 {
        self.scale
    }

    pub fn part_rows(&self) -> usize {
        self.rows.len()
    }

    /// The sheet as a PDF file.
    pub fn to_pdf(&self) -> Vec<u8> {
        let (sw, sh) = self.size.size();
        let mut page = pdf::Page::new(sw, sh);
        let s = self.scale;
        let sx = |x: f64| self.ox + x * s;
        let sy = |y: f64| self.oy + y * s;
        page.rect(
            MARGIN,
            MARGIN,
            sw - 2.0 * MARGIN,
            sh - 2.0 * MARGIN,
            0.5,
            false,
        );
        for p in &self.placed {
            let seg = |a: Vec2, b: Vec2| {
                [
                    (sx(a.x + p.dx), sy(a.y + p.dy)),
                    (sx(b.x + p.dx), sy(b.y + p.dy)),
                ]
            };
            let hidden: Vec<[(f64, f64); 2]> = p
                .lines
                .hidden
                .iter()
                .map(|[a, b]| seg(*a, *b))
                .chain(
                    p.lines
                        .hidden_arcs
                        .iter()
                        .flat_map(arc_chords)
                        .map(|[a, b]| seg(a, b)),
                )
                .collect();
            if self.hidden {
                page.lines(&hidden, 0.25, Some((2.0, 1.0)));
            }
            let visible: Vec<[(f64, f64); 2]> = p
                .lines
                .visible
                .iter()
                .map(|[a, b]| seg(*a, *b))
                .chain(
                    p.lines
                        .visible_arcs
                        .iter()
                        .flat_map(arc_chords)
                        .map(|[a, b]| seg(a, b)),
                )
                .collect();
            page.lines(&visible, 0.5, None);
            if let Some(caption) = &p.caption {
                page.text(
                    sx((p.b.minx + p.b.maxx) / 2.0 + p.dx),
                    sy(p.b.miny + p.dy) - 6.0,
                    3.5,
                    caption,
                    Anchor::Middle,
                    0.0,
                    false,
                );
            }
            if let Some(mark) = &p.section {
                // The cut faces hatched at 45 degrees, even-odd across
                // every loop so holes stay clear, and the caption.
                let loops: Vec<Vec<(f64, f64)>> = p
                    .cut
                    .iter()
                    .map(|l| l.iter().map(|q| (sx(q.x + p.dx), sy(q.y + p.dy))).collect())
                    .collect();
                page.lines(&hatch(&loops, 2.5), 0.18, None);
                page.text(
                    sx((p.b.minx + p.b.maxx) / 2.0 + p.dx),
                    sy(p.b.miny + p.dy) - 6.0,
                    3.5,
                    &format!("SECTION {0}-{0}", mark.letter),
                    Anchor::Middle,
                    0.0,
                    false,
                );
            }
        }
        // Cutting-plane traces: a chain line across the view the plane is
        // edge-on in, arrows for the direction of sight, the letter.
        for p in &self.placed {
            let Some(mark) = &p.section else { continue };
            let Some(on) = self.placed.iter().find(|q| q.name == mark.on) else {
                continue;
            };
            let ext = 5.0 / s;
            let (a, b) = if mark.horizontal {
                (
                    Vec2::new(on.b.minx - ext, mark.at),
                    Vec2::new(on.b.maxx + ext, mark.at),
                )
            } else {
                (
                    Vec2::new(mark.at, on.b.miny - ext),
                    Vec2::new(mark.at, on.b.maxy + ext),
                )
            };
            let pt = |q: Vec2| (sx(q.x + on.dx), sy(q.y + on.dy));
            page.lines(&[[pt(a), pt(b)]], 0.35, Some((6.0, 1.5)));
            // Arrowheads at both ends, pointing the way the section looks,
            // and the letter beyond each.
            let (head, half) = (3.0 / s, 1.2 / s);
            let d = mark.sight;
            let n = Vec2::new(-d.y, d.x);
            for end in [a, b] {
                let tip = end + d * head;
                let tri = [tip, end + n * half, end - n * half];
                let pts: Vec<(f64, f64)> = tri.iter().map(|q| pt(*q)).collect();
                page.polygon(&pts);
                page.lines(&[[pt(end - d * head), pt(end)]], 0.35, None);
                let label = end - d * (head + 1.5 / s);
                let (lx, ly) = pt(label);
                page.text(
                    lx,
                    ly - 1.2,
                    3.5,
                    &mark.letter.to_string(),
                    Anchor::Middle,
                    0.0,
                    false,
                );
            }
        }
        // Dimensions: extension lines, the line, arrowheads and the value.
        for d in &self.dims {
            let dv = d.b - d.a;
            let len = dv.length().max(1e-9);
            let u = dv * (1.0 / len);
            let n = Vec2::new(-u.y, u.x);
            let at = |p: Vec2, along: f64, across: f64| p + u * along + n * across;
            let over = d.offset + d.offset.signum() * DIM_OVERSHOOT;
            let lines = [
                [at(d.a, 0.0, 0.0), at(d.a, 0.0, over)],
                [at(d.b, 0.0, 0.0), at(d.b, 0.0, over)],
                [at(d.a, 0.0, d.offset), at(d.b, 0.0, d.offset)],
            ];
            let segs: Vec<[(f64, f64); 2]> = lines
                .iter()
                .map(|[a, b]| [(sx(a.x), sy(a.y)), (sx(b.x), sy(b.y))])
                .collect();
            page.lines(&segs, 0.18, None);
            let (head, half) = (2.5 / s, 0.8 / s);
            for tri in [
                [
                    at(d.a, 0.0, d.offset),
                    at(d.a, head, d.offset + half),
                    at(d.a, head, d.offset - half),
                ],
                [
                    at(d.b, 0.0, d.offset),
                    at(d.b, -head, d.offset + half),
                    at(d.b, -head, d.offset - half),
                ],
            ] {
                let pts: Vec<(f64, f64)> = tri.iter().map(|p| (sx(p.x), sy(p.y))).collect();
                page.polygon(&pts);
            }
            let mid = at(
                d.a,
                len / 2.0,
                d.offset + if d.offset < 0.0 { -1.2 / s } else { 1.2 / s },
            );
            let mut angle = u.y.atan2(u.x).to_degrees();
            if angle > 90.0 {
                angle -= 180.0;
            }
            if angle <= -90.0 {
                angle += 180.0;
            }
            if angle.abs() < 0.5 {
                angle = 0.0;
            }
            if (angle.abs() - 90.0).abs() < 0.5 {
                angle = 90.0;
            }
            page.text(
                sx(mid.x),
                sy(mid.y),
                3.0,
                &dim_text(d.value),
                Anchor::Middle,
                angle,
                false,
            );
        }
        // Diameter callouts: a leader from the rim, a shoulder, the text.
        for p in &self.placed {
            for c in &p.callouts {
                let a = c.angle.to_radians();
                let (ux, uy) = (a.cos(), a.sin());
                let left = ux < 0.0;
                let rim = Vec2::new(
                    c.centre.x + p.dx + c.radius * ux,
                    c.centre.y + p.dy + c.radius * uy,
                );
                let elbow = rim + Vec2::new(5.0 / s * ux, 5.0 / s * uy);
                let end = elbow + Vec2::new(if left { -4.0 / s } else { 4.0 / s }, 0.0);
                page.lines(
                    &[
                        [(sx(rim.x), sy(rim.y)), (sx(elbow.x), sy(elbow.y))],
                        [(sx(elbow.x), sy(elbow.y)), (sx(end.x), sy(end.y))],
                    ],
                    0.18,
                    None,
                );
                let back = rim + Vec2::new(ux, uy) * (2.5 / s);
                let nrm = Vec2::new(-uy, ux) * (0.8 / s);
                page.polygon(&[
                    (sx(rim.x), sy(rim.y)),
                    (sx(back.x + nrm.x), sy(back.y + nrm.y)),
                    (sx(back.x - nrm.x), sy(back.y - nrm.y)),
                ]);
                let d = dim_text(2.0 * c.radius);
                let text = if c.count > 1 {
                    format!("{}× Ø{}", c.count, d)
                } else {
                    format!("Ø{}", d)
                };
                let tx = sx(end.x) + if left { -0.8 } else { 0.8 };
                page.text(
                    tx,
                    sy(end.y),
                    3.0,
                    &text,
                    if left { Anchor::Right } else { Anchor::Left },
                    0.0,
                    false,
                );
            }
        }
        let table = (!self.rows.is_empty()).then(|| {
            let w: f64 = TABLE_COLS.iter().map(|c| c.1).sum();
            (
                sw - MARGIN - w,
                MARGIN + BLOCK + (self.rows.len() as f64 + 1.0) * TABLE_ROW,
            )
        });
        // Balloons: a circle outside the view on the ray from its middle
        // through the part, a leader to the part. Neighbouring balloons
        // are pushed apart around the view so none overlap.
        for p in &self.placed {
            if p.balloons.is_empty() {
                continue;
            }
            let c = Vec2::new(
                (p.b.minx + p.b.maxx) / 2.0 + p.dx,
                (p.b.miny + p.b.maxy) / 2.0 + p.dy,
            );
            let (hw, hh) = ((p.b.maxx - p.b.minx) / 2.0, (p.b.maxy - p.b.miny) / 2.0);
            let r = BALLOON_R / s;
            // Distance from the middle to a balloon centre along a direction.
            let dist = |d: Vec2| {
                let reach = (if d.x.abs() > 1e-9 {
                    hw / d.x.abs()
                } else {
                    f64::INFINITY
                })
                .min(if d.y.abs() > 1e-9 {
                    hh / d.y.abs()
                } else {
                    f64::INFINITY
                });
                reach + BALLOON_GAP / s + r
            };
            // The parts list corner is out of bounds for balloons: the
            // arc of directions that would land one on it is skipped.
            let tau = std::f64::consts::TAU;
            let on_table = |angle: f64| {
                let Some((tx, ty)) = table else { return false };
                let d = Vec2::new(angle.cos(), angle.sin());
                let centre = c + d * dist(d);
                sx(centre.x) + BALLOON_R > tx - 1.0 && sy(centre.y) - BALLOON_R < ty + 1.0
            };
            let steps = 360;
            let blocked: Vec<bool> = (0..steps)
                .map(|k| on_table(k as f64 * tau / steps as f64))
                .collect();
            // The allowed arc runs from the end of the blocked run to its
            // start, going counter-clockwise (None: no direction blocked).
            let arc = blocked
                .iter()
                .position(|b| *b)
                .filter(|_| !blocked.iter().all(|b| *b))
                .map(|_| {
                    let first_free_after_block = (0..steps)
                        .find(|&k| blocked[(k + steps - 1) % steps] && !blocked[k])
                        .unwrap_or(0);
                    let lo = first_free_after_block as f64 * tau / steps as f64;
                    let free = blocked.iter().filter(|b| !**b).count();
                    (lo, lo + free as f64 * tau / steps as f64)
                });
            let mut items: Vec<(usize, Vec2, f64)> = p
                .balloons
                .iter()
                .map(|(item, at)| {
                    let anchor = Vec2::new(at.x + p.dx, at.y + p.dy);
                    let d = anchor - c;
                    let mut angle = if d.length() < 1e-9 {
                        std::f64::consts::FRAC_PI_4
                    } else {
                        d.y.atan2(d.x)
                    };
                    if let Some((lo, hi)) = arc {
                        // Into the arc's range, then to its nearer end.
                        while angle < lo {
                            angle += tau;
                        }
                        while angle >= lo + tau {
                            angle -= tau;
                        }
                        if angle > hi {
                            angle = if angle - hi < lo + tau - angle {
                                hi
                            } else {
                                lo
                            };
                        }
                    }
                    (*item, anchor, angle)
                })
                .collect();
            items.sort_by(|a, b| a.2.total_cmp(&b.2));
            let n = items.len();
            for _ in 0..60 {
                let mut moved = false;
                for k in 0..n {
                    let (a, b) = (k, (k + 1) % n);
                    if a == b || (b == 0 && arc.is_some()) {
                        break;
                    }
                    let (ta, tb) = (items[a].2, items[b].2);
                    let gap = if b == 0 { tb + tau - ta } else { tb - ta };
                    let mid = (ta + tb) / 2.0;
                    let sep = (2.0 * r + 1.0 / s) / dist(Vec2::new(mid.cos(), mid.sin()));
                    if gap < sep - 1e-9 {
                        let push = (sep - gap) / 2.0;
                        items[a].2 -= push;
                        items[b].2 += push;
                        moved = true;
                    }
                }
                if let Some((lo, hi)) = arc {
                    // Squeeze back into the arc from whichever end spilled.
                    if items[0].2 < lo {
                        let shift = lo - items[0].2;
                        for it in items.iter_mut() {
                            it.2 += shift;
                        }
                    }
                    if items[n - 1].2 > hi {
                        let shift = items[n - 1].2 - hi;
                        for it in items.iter_mut() {
                            it.2 -= shift;
                        }
                    }
                }
                if !moved {
                    break;
                }
            }
            for (item, anchor, angle) in items {
                let d = Vec2::new(angle.cos(), angle.sin());
                let centre = c + d * dist(d);
                let to_anchor = anchor - centre;
                let l = to_anchor.length().max(1e-9);
                let rim = centre + to_anchor * (r / l);
                page.lines(
                    &[[(sx(rim.x), sy(rim.y)), (sx(anchor.x), sy(anchor.y))]],
                    0.18,
                    None,
                );
                page.dot(sx(anchor.x), sy(anchor.y), 0.6);
                page.circle(sx(centre.x), sy(centre.y), BALLOON_R, 0.35, true);
                page.text(
                    sx(centre.x),
                    sy(centre.y),
                    3.5,
                    &item.to_string(),
                    Anchor::Middle,
                    0.0,
                    false,
                );
            }
        }
        // Parts list above the title block, header on top.
        if !self.rows.is_empty() {
            let table_w: f64 = TABLE_COLS.iter().map(|c| c.1).sum();
            let table_h = (self.rows.len() as f64 + 1.0) * TABLE_ROW;
            let x0 = sw - MARGIN - table_w;
            let top = MARGIN + BLOCK + table_h;
            page.rect(x0, MARGIN + BLOCK, table_w, table_h, 0.5, true);
            let mut cx = x0;
            for col in &TABLE_COLS[..TABLE_COLS.len() - 1] {
                cx += col.1;
                page.lines(&[[(cx, MARGIN + BLOCK), (cx, top)]], 0.25, None);
            }
            let mut cells: Vec<Vec<String>> =
                vec![TABLE_COLS.iter().map(|c| c.0.to_string()).collect()];
            for r in &self.rows {
                cells.push(vec![
                    r.item.to_string(),
                    r.name.clone(),
                    r.qty.to_string(),
                    r.material.clone(),
                ]);
            }
            for (ri, row) in cells.iter().enumerate() {
                let y = top - ri as f64 * TABLE_ROW;
                if ri > 0 {
                    page.lines(&[[(x0, y), (x0 + table_w, y)]], 0.25, None);
                }
                let mut x = x0;
                for (ci, text) in row.iter().enumerate() {
                    page.text(
                        x + 2.0,
                        y - TABLE_ROW / 2.0,
                        3.0,
                        text,
                        Anchor::Left,
                        0.0,
                        ri == 0,
                    );
                    x += TABLE_COLS[ci].1;
                }
            }
        }
        // Title block along the bottom edge.
        page.rect(MARGIN, MARGIN, sw - 2.0 * MARGIN, BLOCK, 0.5, false);
        page.lines(
            &[[(sw / 2.0, MARGIN), (sw / 2.0, MARGIN + BLOCK)]],
            0.35,
            None,
        );
        let by = MARGIN + BLOCK;
        page.text(
            MARGIN + 4.0,
            by - 9.0,
            6.0,
            &self.title,
            Anchor::Left,
            0.0,
            false,
        );
        let names: Vec<&str> = self.placed.iter().map(|p| p.name.as_str()).collect();
        page.text(
            MARGIN + 4.0,
            by - 18.0,
            3.5,
            &format!(
                "Views: {} · third angle{} · mm · {}",
                names.join(", "),
                if self.hidden {
                    ""
                } else {
                    " · hidden lines omitted"
                },
                self.size.name()
            ),
            Anchor::Left,
            0.0,
            false,
        );
        page.text(
            sw / 2.0 + 4.0,
            by - 9.0,
            4.0,
            &format!("Scale {}", scale_label(s)),
            Anchor::Left,
            0.0,
            false,
        );
        let note = if self.note.is_empty() {
            "offkilter".to_string()
        } else {
            format!("{} · offkilter", self.note)
        };
        page.text(
            sw / 2.0 + 4.0,
            by - 18.0,
            3.5,
            &note,
            Anchor::Left,
            0.0,
            false,
        );
        page.finish()
    }
}

/// A body for a sheet: name, material, the key grouping identical parts,
/// and the solid.
pub type PartRecord = (String, String, (u32, usize), Vec<Solid>);

/// The parts of a tab for a sheet: a part studio's bodies, or an
/// assembly's placed bodies grouped by the part they are instances of.
/// Regenerates the tab.
pub fn parts_of(doc: &mut Document, tab: TabId) -> Result<Vec<PartRecord>, String> {
    let kind = doc.tab(tab).map(|t| t.kind_name()).ok_or("no such tab")?;
    if kind == "assembly" {
        let r = doc.regenerate_assembly(tab).map_err(|e| e.to_string())?;
        let asm = doc.assembly(tab).map_err(|e| e.to_string())?.clone();
        // One record per instance: a part's body, or every placed body of
        // a sub-assembly instance, which the list carries as one item.
        let mut out: Vec<PartRecord> = Vec::new();
        let mut last: Option<ok_model::InstanceId> = None;
        for (b, inst) in r.bodies.iter().zip(&r.placed) {
            let Some(i) = asm.instances.iter().find(|i| i.id == *inst) else {
                continue;
            };
            let material = b
                .material
                .as_ref()
                .map(|m| m.name.clone())
                .unwrap_or_default();
            if last == Some(*inst) {
                if let Some(rec) = out.last_mut() {
                    rec.3.push(b.solid.clone());
                    if rec.1 != material {
                        rec.1.clear();
                    }
                }
                continue;
            }
            last = Some(*inst);
            let sub = doc.tab(i.studio).map(|t| t.kind_name()) == Some("assembly");
            let key = if sub {
                (i.studio.0, usize::MAX)
            } else {
                (i.studio.0, i.body)
            };
            out.push((part_name(doc, key)?, material, key, vec![b.solid.clone()]));
        }
        Ok(out)
    } else {
        let r = doc
            .regenerate_studio(tab, None)
            .map_err(|e| e.to_string())?;
        Ok(r.bodies
            .iter()
            .enumerate()
            .map(|(k, b)| {
                (
                    b.name.clone(),
                    b.material
                        .as_ref()
                        .map(|m| m.name.clone())
                        .unwrap_or_default(),
                    (tab.0, k),
                    vec![b.solid.clone()],
                )
            })
            .collect())
    }
}

/// What the parts list calls an item: a part studio's name, plus the
/// body's own name when the studio holds several (an assembly names its
/// placed bodies after the instances, which is not the part); a
/// sub-assembly's name for a sub-assembly instance.
fn part_name(doc: &mut Document, (studio, body): (u32, usize)) -> Result<String, String> {
    let tab = TabId(studio);
    let studio_name = doc
        .tab(tab)
        .map(|t| t.name().to_string())
        .ok_or("instance of a missing studio")?;
    match doc.tab(tab).map(|t| t.kind_name()) {
        Some("part_studio") => {
            let r = doc
                .regenerate_studio(tab, None)
                .map_err(|e| e.to_string())?;
            Ok(match r.bodies.get(body) {
                Some(b) if r.bodies.len() > 1 => format!("{studio_name} \u{b7} {}", b.name),
                _ => studio_name,
            })
        }
        _ => Ok(studio_name),
    }
}

/// A tab's shop drawing as a PDF, regenerating the tab. The title
/// defaults to the document and tab names.
/// One position of a mechanism: its caption, and the mates to set as
/// (id, angle in degrees, offset in mm).
pub type Position = (String, Vec<(ok_model::MateId, f64, f64)>);

/// A mechanism at several positions of its mates on one sheet: one view
/// of the assembly per position, in a row, each captioned. `view` is a
/// standard view name.
pub fn motion_pdf(
    doc: &mut Document,
    tab: TabId,
    view: &str,
    positions: &[Position],
    opts: &Options,
) -> Result<Vec<u8>, String> {
    let v = standard_view(view)
        .ok_or_else(|| format!("unknown view {view:?}: use front, top, right or iso"))?;
    if positions.is_empty() {
        return Err("no positions to draw".into());
    }
    let mut views = Vec::new();
    for (caption, mates) in positions {
        let r = doc
            .preview_assembly_at(tab, mates)
            .map_err(|e| e.to_string())?;
        let solids: Vec<&Solid> = r.bodies.iter().map(|b| &b.solid).collect();
        views.push((
            format!("{view} · {caption}"),
            project_view(&solids, v),
            Some(caption.clone()),
        ));
    }
    let mut opts = opts.clone();
    if opts.title.is_empty() {
        let tab_name = doc
            .tab(tab)
            .map(|t| t.name().to_string())
            .unwrap_or_default();
        opts.title = format!("{} · {} · range of motion", doc.name, tab_name);
    }
    Ok(Sheet::layout_views(views, &opts)?.to_pdf())
}

pub fn drawing_pdf(doc: &mut Document, tab: TabId, opts: &Options) -> Result<Vec<u8>, String> {
    let parts = parts_of(doc, tab)?;
    let mut opts = opts.clone();
    if opts.title.is_empty() {
        let tab_name = doc
            .tab(tab)
            .map(|t| t.name().to_string())
            .unwrap_or_default();
        opts.title = format!("{} · {}", doc.name, tab_name);
    }
    let refs: Vec<Part> = parts
        .iter()
        .map(|(name, material, key, solids)| Part {
            name: name.clone(),
            material: material.clone(),
            solids: solids.iter().collect(),
            key: *key,
        })
        .collect();
    Ok(Sheet::layout(&refs, &opts)?.to_pdf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ok_math::Plane;

    fn block(w: f64, d: f64, h: f64) -> Solid {
        let mut sk = ok_sketch::Sketch::new();
        sk.add_rectangle(Vec2::ZERO, Vec2::new(w, d));
        let profile = sk.profiles(&ok_sketch::ProfileOptions::default()).remove(0);
        ok_brep::extrude(
            &profile,
            &Plane::from_origin_normal(Vec3::ZERO, Vec3::Z).unwrap(),
            0.0,
            h,
            1,
        )
        .unwrap()
    }

    fn objects_are_where_the_xref_says(pdf: &[u8]) {
        // Offsets are byte offsets, and the content stream is binary, so
        // this reads the file as bytes.
        let find = |needle: &[u8], from: usize| -> usize {
            pdf[from..]
                .windows(needle.len())
                .position(|w| w == needle)
                .map(|p| p + from)
                .unwrap_or_else(|| panic!("no {:?}", String::from_utf8_lossy(needle)))
        };
        let number = |from: usize| -> usize {
            let digits: String = pdf[from..]
                .iter()
                .take_while(|b| b.is_ascii_digit())
                .map(|&b| b as char)
                .collect();
            digits.parse().unwrap()
        };
        let sx = pdf.windows(10).rposition(|w| w == b"startxref\n").unwrap();
        let xref_at = number(sx + 10);
        assert!(
            pdf[xref_at..].starts_with(b"xref\n"),
            "startxref points at the table"
        );
        let count = number(find(b"\n0 ", xref_at) + 3);
        let first = find(b"0000000000 65535 f \n", xref_at) + 20;
        for i in 1..count {
            let line = first + (i - 1) * 20;
            let offset = number(line);
            assert!(
                pdf[offset..].starts_with(format!("{i} 0 obj").as_bytes()),
                "object {i} at {offset}"
            );
        }
        // The stream is deflated and inflates to the drawing's operators.
        let text = pdf::inflated(pdf);
        assert!(
            String::from_utf8_lossy(pdf).contains("/Filter /FlateDecode") && text.contains(" cm\n"),
            "a deflated content stream"
        );
    }

    #[test]
    fn a_block_gets_three_views_its_three_sizes_and_a_scale_that_fits() {
        let solid = block(40.0, 20.0, 10.0);
        let parts = [Part {
            name: "Block".into(),
            material: String::new(),
            solids: vec![&solid],
            key: (1, 0),
        }];
        let sheet = Sheet::layout(&parts, &Options::default()).unwrap();
        assert_eq!(sheet.placed.len(), 4);
        let mut values: Vec<f64> = sheet.dims.iter().map(|d| d.value).collect();
        values.sort_by(|a, b| a.total_cmp(b));
        assert_eq!(values, vec![10.0, 20.0, 40.0]);
        // With the wide isometric view A4 takes it at 1:1; without, 2:1 fits.
        assert!((sheet.scale() - 1.0).abs() < 1e-9, "{}", sheet.scale());
        let three = Sheet::layout(
            &parts,
            &Options {
                views: ["front", "top", "right"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                ..Options::default()
            },
        )
        .unwrap();
        assert!((three.scale() - 2.0).abs() < 1e-9, "{}", three.scale());
        assert_eq!(sheet.part_rows(), 0);
        let pdf = three.to_pdf();
        assert!(pdf.starts_with(b"%PDF-1.4"));
        let text = pdf::inflated(&pdf);
        assert!(
            text.contains("/MediaBox [0 0 841.89 595.276]"),
            "A4 landscape in points"
        );
        assert!(text.contains("(40) Tj") && text.contains("(20) Tj") && text.contains("(10) Tj"));
        assert!(text.contains("(Scale 2:1) Tj"));
        assert!(!text.contains("[2 1] 0 d"), "a plain block hides nothing");
        objects_are_where_the_xref_says(&pdf);
        // A big part drops to 1:5 and Letter is a different page.
        let big = block(800.0, 400.0, 300.0);
        let parts = [Part {
            name: "Slab".into(),
            material: String::new(),
            solids: vec![&big],
            key: (1, 0),
        }];
        let sheet = Sheet::layout(
            &parts,
            &Options {
                sheet: SheetSize::Letter,
                ..Options::default()
            },
        )
        .unwrap();
        assert!((sheet.scale() - 0.1).abs() < 1e-9, "{}", sheet.scale());
        assert!(pdf::inflated(&sheet.to_pdf()).contains("/MediaBox [0 0 792 612]"));
    }

    #[test]
    fn two_sections_cut_a_block_with_a_hole_and_are_traced_on_the_top_view() {
        // A 60 x 40 x 20 block with a 10 mm hole through it at (30, 20).
        let solid = {
            let mut sk = ok_sketch::Sketch::new();
            sk.add_rectangle(Vec2::ZERO, Vec2::new(60.0, 40.0));
            sk.add_circle(Vec2::new(30.0, 20.0), 5.0);
            let profiles = sk.profiles(&ok_sketch::ProfileOptions::default());
            let profile = profiles
                .iter()
                .max_by(|a, b| a.area().total_cmp(&b.area()))
                .unwrap();
            ok_brep::extrude(
                profile,
                &Plane::from_origin_normal(Vec3::ZERO, Vec3::Z).unwrap(),
                0.0,
                20.0,
                1,
            )
            .unwrap()
        };
        let parts = [Part {
            name: "Block".into(),
            material: String::new(),
            solids: vec![&solid],
            key: (1, 0),
        }];
        let opts = Options {
            views: ["front", "top", "section@20", "section-side"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            ..Options::default()
        };
        let sheet = Sheet::layout(&parts, &opts).unwrap();
        // Section A-A through the hole's axis (y = 20): the cut face is
        // the two walls beside the hole, so two loops, each 25 x 20.
        let a = sheet
            .placed
            .iter()
            .find(|p| p.name == "section@20")
            .unwrap();
        assert_eq!(a.cut.len(), 2, "{:?}", a.cut);
        for l in &a.cut {
            let (xs, ys): (Vec<f64>, Vec<f64>) = l.iter().map(|p| (p.x, p.y)).unzip();
            let w = xs.iter().cloned().fold(f64::MIN, f64::max)
                - xs.iter().cloned().fold(f64::MAX, f64::min);
            let h = ys.iter().cloned().fold(f64::MIN, f64::max)
                - ys.iter().cloned().fold(f64::MAX, f64::min);
            assert!(
                (w - 25.0).abs() < 1e-6 && (h - 20.0).abs() < 1e-6,
                "loop {w} x {h}"
            );
        }
        let mark = a.section.as_ref().unwrap();
        assert!(
            mark.letter == 'A'
                && mark.on == "top"
                && mark.horizontal
                && (mark.at - 20.0).abs() < 1e-9
        );
        // Section B-B through the middle (x = 30): the same two walls.
        let b = sheet
            .placed
            .iter()
            .find(|p| p.name == "section-side")
            .unwrap();
        assert_eq!(b.cut.len(), 2);
        let mark = b.section.as_ref().unwrap();
        assert!(
            mark.letter == 'B'
                && mark.on == "top"
                && !mark.horizontal
                && (mark.at - 30.0).abs() < 1e-9
        );
        // Sections sit to the right of the elevation views.
        let front = sheet.placed.iter().find(|p| p.name == "front").unwrap();
        assert!(a.b.minx + a.dx > front.b.maxx + front.dx && b.b.minx + b.dx > a.b.maxx + a.dx);
        // Hatching: even-odd over both loops; a line across one wall never
        // crosses the hole.
        let loops: Vec<Vec<(f64, f64)>> = a
            .cut
            .iter()
            .map(|l| l.iter().map(|p| (p.x, p.y)).collect())
            .collect();
        let lines = hatch(&loops, 2.5);
        assert!(lines.len() > 10, "{}", lines.len());
        for [(x0, _), (x1, _)] in &lines {
            assert!(
                !(x0.min(*x1) < 25.0 - 1e-6 && x0.max(*x1) > 35.0 + 1e-6),
                "a hatch line crossed the hole"
            );
        }
        let pdf = sheet.to_pdf();
        objects_are_where_the_xref_says(&pdf);
        let text = pdf::inflated(&pdf);
        assert!(
            text.contains("(SECTION A-A) Tj") && text.contains("(SECTION B-B) Tj"),
            "{text}"
        );
        assert!(text.contains("[6 1.5] 0 d"), "the trace is a chain line");
        assert!(
            text.contains("Views: front, top, section@20, section-side"),
            "{text}"
        );
        // A table-top sized part with five views: one row would need 1:5
        // on A3, two rows fit 1:2.5, so the sections go under the front.
        let top = block(400.0, 380.0, 38.0);
        let parts = [Part {
            name: "Top".into(),
            material: String::new(),
            solids: vec![&top],
            key: (1, 0),
        }];
        let sheet = Sheet::layout(
            &parts,
            &Options {
                views: ["front", "top", "right", "section", "section-side"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                sheet: SheetSize::A3,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(
            (sheet.scale() - 0.4).abs() < 1e-9,
            "scale {}",
            sheet.scale()
        );
        assert_eq!(scale_label(sheet.scale()), "1:2.5");
        let front = sheet.placed.iter().find(|p| p.name == "front").unwrap();
        for name in ["section", "section-side"] {
            let v = sheet.placed.iter().find(|p| p.name == name).unwrap();
            assert!(
                v.b.maxy + v.dy < front.b.miny + front.dy,
                "{name} sits under the front view"
            );
        }
        assert!(pdf::inflated(&sheet.to_pdf()).contains("(Scale 1:2.5) Tj"));
        // Bad cuts are refused with their reason.
        let err = Sheet::layout(
            &parts,
            &Options {
                views: vec!["section@abc".into()],
                ..Options::default()
            },
        )
        .err()
        .expect("refused");
        assert!(err.contains("millimetres"), "{err}");
        let err = Sheet::layout(
            &parts,
            &Options {
                views: vec!["section@2000".into()],
                ..Options::default()
            },
        )
        .err()
        .expect("refused");
        assert!(
            err.contains("misses every body") && err.contains("0 to 380 along y"),
            "{err}"
        );
        let err = Sheet::layout(
            &parts,
            &Options {
                views: vec!["behind".into()],
                ..Options::default()
            },
        )
        .err()
        .expect("refused");
        assert!(err.contains("section-side"), "{err}");
    }

    #[test]
    fn captioned_views_go_in_a_row_with_their_captions_under_them() {
        // A block at three "positions": the same view three times, as a
        // range-of-motion sheet would give it.
        let solid = block(40.0, 20.0, 10.0);
        let view = standard_view("front").unwrap();
        let lines = project_view(&[&solid], view);
        let views: Vec<(String, ViewLines, Option<String>)> = ["lowest", "working", "highest"]
            .iter()
            .map(|c| (format!("front · {c}"), lines.clone(), Some(c.to_string())))
            .collect();
        let sheet = Sheet::layout_views(views, &Options::default()).unwrap();
        assert_eq!(sheet.placed.len(), 3);
        // In a row, left to right, at one height.
        for w in sheet.placed.windows(2) {
            assert!(w[1].b.minx + w[1].dx > w[0].b.maxx + w[0].dx);
            assert!((w[1].dy - w[0].dy).abs() < 1e-9);
        }
        let text = pdf::inflated(&sheet.to_pdf()).to_string();
        for c in ["lowest", "working", "highest"] {
            assert!(text.contains(&format!("({c}) Tj")), "{c} captioned");
        }
        assert!(
            text.contains("Views: front \\267 lowest, front \\267 working, front \\267 highest"),
            "{text}"
        );
        assert!(Sheet::layout_views(Vec::new(), &Options::default()).is_err());
    }

    #[test]
    fn an_assembly_gets_balloons_and_a_parts_list_and_a_hole_a_callout() {
        let plate = {
            let mut sk = ok_sketch::Sketch::new();
            sk.add_rectangle(Vec2::ZERO, Vec2::new(60.0, 40.0));
            sk.add_circle(Vec2::new(30.0, 20.0), 5.0);
            let profiles = sk.profiles(&ok_sketch::ProfileOptions::default());
            let profile = profiles
                .iter()
                .max_by(|a, b| a.area().total_cmp(&b.area()))
                .unwrap();
            ok_brep::extrude(
                profile,
                &Plane::from_origin_normal(Vec3::ZERO, Vec3::Z).unwrap(),
                0.0,
                6.0,
                1,
            )
            .unwrap()
        };
        let peg = block(8.0, 8.0, 30.0);
        let peg2 = peg.transformed(&ok_brep::Transform::translation(Vec3::new(45.0, 0.0, 0.0)));
        let parts = [
            Part {
                name: "Plate".into(),
                material: "Maple".into(),
                solids: vec![&plate],
                key: (2, 0),
            },
            Part {
                name: "Peg".into(),
                material: String::new(),
                solids: vec![&peg],
                key: (3, 0),
            },
            Part {
                name: "Peg".into(),
                material: String::new(),
                solids: vec![&peg2],
                key: (3, 0),
            },
        ];
        let sheet = Sheet::layout(&parts, &Options::default()).unwrap();
        assert_eq!(sheet.part_rows(), 2, "two distinct parts");
        assert_eq!(sheet.rows[1].qty, 2);
        let iso = sheet.placed.iter().find(|p| p.name == "iso").unwrap();
        assert_eq!(iso.balloons.len(), 2);
        let top = sheet.placed.iter().find(|p| p.name == "top").unwrap();
        assert!(top.callouts.is_empty(), "no hole callouts on an assembly");
        // No view sits on the parts list in the bottom right corner.
        let table_x = 297.0 - MARGIN - TABLE_COLS.iter().map(|c| c.1).sum::<f64>();
        let table_y = MARGIN + BLOCK + 3.0 * TABLE_ROW;
        for p in &sheet.placed {
            let x1 = (p.b.maxx + p.dx) * sheet.scale + sheet.ox;
            let y0 = (p.b.miny + p.dy) * sheet.scale + sheet.oy;
            assert!(
                x1 <= table_x || y0 >= table_y,
                "{} covers the table",
                p.name
            );
        }
        let text = pdf::inflated(&sheet.to_pdf()).to_string();
        assert!(
            text.contains("(PART) Tj")
                && text.contains("(Plate) Tj")
                && text.contains("(Maple) Tj")
        );
        assert!(text.contains("(2) Tj"), "quantity two");
        assert!(text.contains("/F2 3 Tf"), "bold header");

        // The plate on its own sheet gets its hole called out.
        let sheet = Sheet::layout(&parts[..1], &Options::default()).unwrap();
        assert_eq!(sheet.part_rows(), 0, "no parts list for one part");
        let top = sheet.placed.iter().find(|p| p.name == "top").unwrap();
        assert_eq!(top.callouts.len(), 1, "{:?}", top.callouts);
        assert!(top.callouts[0].hole && (top.callouts[0].radius - 5.0).abs() < 1e-9);
        let text = pdf::inflated(&sheet.to_pdf()).to_string();
        assert!(text.contains("(\\33010)"), "a diameter callout");
        assert!(
            text.contains("[2 1] 0 d"),
            "the hole is hidden lines from the front"
        );
    }

    #[test]
    fn a_sub_assembly_is_one_item_with_its_bodies_and_the_sheet_omits_hidden_lines() {
        use ok_model::{
            AssemblyOp, BodyOp, DocOp, Document, ExtrudeDirection, ExtrudeEnd, Op, Placement,
            PlaneRef, ProfileSelection, SketchOp, StandardPlane, TabId,
        };
        // Block: a 10 x 10 x 5 part. Pair: two blocks 20 mm apart. Top:
        // two pairs and a block.
        let mut d = Document::new("t");
        let block = d.first_studio().unwrap();
        d.apply_with_base(
            DocOp::RenameTab {
                tab: block,
                name: "Block".into(),
            },
            None,
        )
        .unwrap();
        let apply = |d: &mut Document, tab: TabId, op: Op| {
            d.apply_with_base(DocOp::Studio { tab, op }, None)
                .unwrap()
                .studio
                .unwrap()
                .feature
        };
        let s = apply(
            &mut d,
            block,
            Op::AddSketch {
                plane: PlaneRef::standard(StandardPlane::Top),
                name: None,
            },
        )
        .unwrap();
        apply(
            &mut d,
            block,
            Op::Sketch {
                id: s,
                op: SketchOp::AddRectangle {
                    a: Vec2::ZERO,
                    b: Vec2::new(10.0, 10.0),
                },
            },
        );
        apply(
            &mut d,
            block,
            Op::AddExtrude {
                sketch: s,
                depth: 5.0,
                direction: ExtrudeDirection::Normal,
                end: ExtrudeEnd::Blind,
                profiles: ProfileSelection::All,
                op: BodyOp::New,
                name: None,
            },
        );
        let add_assembly = |d: &mut Document, name: &str| {
            d.apply_with_base(
                DocOp::AddAssembly {
                    name: Some(name.into()),
                },
                None,
            )
            .unwrap()
            .tab
            .unwrap()
        };
        let place = |d: &mut Document, asm: TabId, studio: TabId, x: f64, y: f64| {
            d.apply_with_base(
                DocOp::Assembly {
                    tab: asm,
                    op: AssemblyOp::AddInstance {
                        studio,
                        body: 0,
                        name: None,
                        fixed: true,
                        placement: Placement {
                            position: Vec3::new(x, y, 0.0),
                            rotation: Vec3::ZERO,
                        },
                    },
                },
                None,
            )
            .unwrap();
        };
        let pair = add_assembly(&mut d, "Pair");
        place(&mut d, pair, block, 0.0, 0.0);
        place(&mut d, pair, block, 20.0, 0.0);
        let top = add_assembly(&mut d, "Top");
        place(&mut d, top, pair, 0.0, 0.0);
        place(&mut d, top, pair, 0.0, 30.0);
        place(&mut d, top, block, 40.0, 15.0);

        let parts = parts_of(&mut d, top).unwrap();
        assert_eq!(parts.len(), 3, "one record per instance");
        assert_eq!(parts[0].0, "Pair");
        assert_eq!(parts[0].3.len(), 2, "both blocks of the pair");
        assert_eq!(parts[2].0, "Block");
        assert_eq!(parts[2].3.len(), 1);
        let refs: Vec<Part> = parts
            .iter()
            .map(|(name, material, key, solids)| Part {
                name: name.clone(),
                material: material.clone(),
                solids: solids.iter().collect(),
                key: *key,
            })
            .collect();
        let sheet = Sheet::layout(&refs, &Options::default()).unwrap();
        assert_eq!(sheet.part_rows(), 2);
        assert_eq!(
            (sheet.rows[0].name.as_str(), sheet.rows[0].qty),
            ("Pair", 2)
        );
        assert_eq!(
            (sheet.rows[1].name.as_str(), sheet.rows[1].qty),
            ("Block", 1)
        );
        // The pair's balloon anchors between its two blocks.
        let iso = sheet.placed.iter().find(|p| p.name == "iso").unwrap();
        assert_eq!(iso.balloons.len(), 2);
        let (u, _) = frame(standard_view("iso").unwrap());
        let mid = Vec3::new(15.0, 5.0, 2.5).dot(u);
        assert!(
            (iso.balloons[0].1.x - mid).abs() < 1e-9,
            "{:?}",
            iso.balloons[0]
        );
        let text = pdf::inflated(&sheet.to_pdf()).to_string();
        assert!(text.contains("(Pair) Tj") && text.contains("(Block) Tj"));
        assert!(
            !text.contains("[2 1] 0 d") && text.contains("hidden lines omitted"),
            "an assembly sheet has no hidden lines"
        );
        let forced = Sheet::layout(
            &refs,
            &Options {
                hidden: Some(true),
                ..Options::default()
            },
        )
        .unwrap();
        assert!(pdf::inflated(&forced.to_pdf()).contains("[2 1] 0 d"));
        // The pair's own sheet lists its blocks.
        let parts = parts_of(&mut d, pair).unwrap();
        assert_eq!(parts.len(), 2);
        assert!(parts.iter().all(|p| p.0 == "Block" && p.3.len() == 1));
    }

    #[test]
    fn a_long_parts_list_pushes_the_views_up_instead_of_shrinking_them() {
        // Twenty distinct small parts on A3: the list is 126 mm tall,
        // half the free height, but the three views fit beside it and the
        // iso above it at 1:1. Fitting above the list would give 1:2.
        let solids: Vec<Solid> =
            (0..20)
                .map(|k| {
                    block(15.0, 15.0, 10.0).transformed(&ok_brep::Transform::translation(
                        Vec3::new(20.0 * (k % 4) as f64, 20.0 * (k / 4) as f64, 0.0),
                    ))
                })
                .collect();
        let parts: Vec<Part> = solids
            .iter()
            .enumerate()
            .map(|(k, s)| Part {
                name: format!("Part {k}"),
                material: String::new(),
                solids: vec![s],
                key: (1, k),
            })
            .collect();
        let opts = Options {
            sheet: SheetSize::A3,
            ..Options::default()
        };
        let sheet = Sheet::layout(&parts, &opts).unwrap();
        assert_eq!(sheet.part_rows(), 20);
        assert!((sheet.scale - 1.0).abs() < 1e-9, "{}", sheet.scale);
        let table_x = 420.0 - MARGIN - TABLE_COLS.iter().map(|c| c.1).sum::<f64>();
        let table_y = MARGIN + BLOCK + 21.0 * TABLE_ROW;
        for p in &sheet.placed {
            let x1 = (p.b.maxx + p.dx) * sheet.scale + sheet.ox;
            let y0 = (p.b.miny + p.dy) * sheet.scale + sheet.oy;
            assert!(
                x1 <= table_x || y0 >= table_y,
                "{} covers the table",
                p.name
            );
        }
        let iso = sheet.placed.iter().find(|p| p.name == "iso").unwrap();
        assert!(iso.b.maxx + iso.dx > table_x, "the iso sits above the list");
    }
}
