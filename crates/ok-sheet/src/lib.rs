//! Shop drawing sheets. The bodies of a tab are projected into the
//! standard views (front, top, right, isometric) with hidden lines
//! removed, laid out third-angle at the largest standard scale that fits
//! the sheet, given overall dimensions, diameter callouts for holes and
//! bosses seen end-on, a balloon per part and a parts list for an
//! assembly, and a title block; then written as a vector PDF.
//!
//! The layout matches the web client's drawing dialog, so a sheet asked
//! for from a script or a model looks like the one a person downloads.

mod pdf;

use ok_brep::{project_view, Solid, Surface, View, ViewArc, ViewLines};
use ok_math::{Vec2, Vec3};
use ok_model::{Document, TabId};
pub use pdf::Anchor;

/// Sheet sizes, landscape, millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetSize {
    A4,
    A3,
    A2,
    Letter,
}

impl SheetSize {
    pub fn parse(name: &str) -> Result<SheetSize, String> {
        match name.trim().to_lowercase().as_str() {
            "" | "a4" => Ok(SheetSize::A4),
            "a3" => Ok(SheetSize::A3),
            "a2" => Ok(SheetSize::A2),
            "letter" => Ok(SheetSize::Letter),
            other => Err(format!(
                "unknown sheet size {other:?}: use A4, A3, A2 or Letter"
            )),
        }
    }

    pub fn size(self) -> (f64, f64) {
        match self {
            SheetSize::A4 => (297.0, 210.0),
            SheetSize::A3 => (420.0, 297.0),
            SheetSize::A2 => (594.0, 420.0),
            SheetSize::Letter => (279.4, 215.9),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            SheetSize::A4 => "A4",
            SheetSize::A3 => "A3",
            SheetSize::A2 => "A2",
            SheetSize::Letter => "Letter",
        }
    }
}

/// What to put on the sheet.
#[derive(Debug, Clone)]
pub struct Options {
    /// View names in `front`, `top`, `right`, `iso`; the layout places
    /// the first three third-angle and the rest to the right.
    pub views: Vec<String>,
    pub sheet: SheetSize,
    /// The title block's first line.
    pub title: String,
    /// A second line for the title block's right half (a date, an
    /// author); the kernel has no clock, so the caller supplies it.
    pub note: String,
    /// Balloons and a parts list.
    pub parts: bool,
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
        }
    }
}

/// A body on the sheet: its solid, what the parts list calls it, and a
/// key that groups identical parts into one row with a quantity.
pub struct Part<'a> {
    pub name: String,
    pub material: String,
    pub solid: &'a Solid,
    pub key: (u32, usize),
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

struct Placed {
    name: String,
    lines: ViewLines,
    callouts: Vec<Callout>,
    /// Item balloons: number and anchor in view coordinates.
    balloons: Vec<(usize, Vec2)>,
    dx: f64,
    dy: f64,
    b: Bounds,
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
const STANDARD_SCALES: [f64; 10] = [10.0, 5.0, 2.0, 1.0, 0.5, 0.2, 0.1, 0.05, 0.02, 0.01];
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
    } else {
        format!("1:{}", (1.0 / s).round() as i64)
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
}

impl Sheet {
    /// Lays the parts out for the options.
    pub fn layout(parts: &[Part], opts: &Options) -> Result<Sheet, String> {
        let solids: Vec<&Solid> = parts.iter().map(|p| p.solid).collect();
        if solids.is_empty() {
            return Err("nothing to draw: the tab has no bodies".into());
        }
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
        for name in &opts.views {
            let Some(view) = standard_view(name) else {
                return Err(format!(
                    "unknown view {name:?}: use front, top, right or iso"
                ));
            };
            let lines = project_view(&solids, view);
            // Hole callouts belong on a part's own sheet: an assembly
            // view is a thicket of holes seen end-on.
            let callouts = if name == "iso" || rows.len() > 1 {
                Vec::new()
            } else {
                callouts(&solids, view)
            };
            let balloons = if name == "iso" {
                let (u, v) = frame(view);
                rows.iter()
                    .filter_map(|r| {
                        parts[r.body]
                            .solid
                            .centroid()
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
                dx: 0.0,
                dy: 0.0,
                b,
            });
        }
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
        for v in views.iter_mut() {
            if matches!(v.name.as_str(), "front" | "top" | "right") {
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
        let (sw, sh) = opts.sheet.size();
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
        let avail_w = sw - 2.0 * MARGIN - 2.0 * pad;
        let avail_h = sh - 2.0 * MARGIN - BLOCK - 2.0 * pad;
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
        let top_left = |s: f64| (MARGIN + pad - min.x * s, sh - MARGIN - pad - max.y * s);
        let iso = views.iter().position(|v| v.name == "iso");
        let mut fit = None;
        'scales: for &s in &STANDARD_SCALES {
            if extent_w * s > avail_w || extent_h * s > avail_h {
                continue;
            }
            let centred = (
                MARGIN + pad + (avail_w - extent_w * s) / 2.0 - min.x * s,
                MARGIN + BLOCK + pad + (avail_h - extent_h * s) / 2.0 - min.y * s,
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
            page.lines(&hidden, 0.25, Some((2.0, 1.0)));
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
                "Views: {} · third angle · mm · {}",
                names.join(", "),
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
pub type PartRecord = (String, String, (u32, usize), Solid);

/// The parts of a tab for a sheet: a part studio's bodies, or an
/// assembly's placed bodies grouped by the part they are instances of.
/// Regenerates the tab.
pub fn parts_of(doc: &mut Document, tab: TabId) -> Result<Vec<PartRecord>, String> {
    let kind = doc.tab(tab).map(|t| t.kind_name()).ok_or("no such tab")?;
    if kind == "assembly" {
        let r = doc.regenerate_assembly(tab).map_err(|e| e.to_string())?;
        let asm = doc.assembly(tab).map_err(|e| e.to_string())?;
        let keys: Vec<((u32, usize), String)> = r
            .bodies
            .iter()
            .zip(&r.placed)
            .filter_map(|(b, inst)| {
                let i = asm.instances.iter().find(|i| i.id == *inst)?;
                let material = b
                    .material
                    .as_ref()
                    .map(|m| m.name.clone())
                    .unwrap_or_default();
                Some(((i.studio.0, i.body), material))
            })
            .collect();
        let solids: Vec<Solid> = r.bodies.iter().map(|b| b.solid.clone()).collect();
        let mut out = Vec::new();
        for ((key, material), solid) in keys.into_iter().zip(solids) {
            out.push((part_name(doc, key)?, material, key, solid));
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
                    b.solid.clone(),
                )
            })
            .collect())
    }
}

/// What the parts list calls a body of a part studio: the studio's name,
/// plus the body's own name when the studio holds several (an assembly
/// names its placed bodies after the instances, which is not the part).
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
        .map(|(name, material, key, solid)| Part {
            name: name.clone(),
            material: material.clone(),
            solid,
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
        let text = String::from_utf8_lossy(pdf);
        let xref_at: usize = text
            .rsplit("startxref\n")
            .next()
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!(
            text[xref_at..].starts_with("xref\n"),
            "startxref points at the table"
        );
        let table = &text[xref_at..];
        for (i, line) in table
            .lines()
            .skip(2)
            .take_while(|l| l.len() == 18)
            .enumerate()
        {
            let offset: usize = line[..10].parse().unwrap();
            if i > 0 {
                assert!(
                    text[offset..].starts_with(&format!("{i} 0 obj")),
                    "object {i} at {offset}"
                );
            }
        }
    }

    #[test]
    fn a_block_gets_three_views_its_three_sizes_and_a_scale_that_fits() {
        let solid = block(40.0, 20.0, 10.0);
        let parts = [Part {
            name: "Block".into(),
            material: String::new(),
            solid: &solid,
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
        let text = String::from_utf8_lossy(&pdf);
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
            solid: &big,
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
        assert!(String::from_utf8_lossy(&sheet.to_pdf()).contains("/MediaBox [0 0 792 612]"));
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
                solid: &plate,
                key: (2, 0),
            },
            Part {
                name: "Peg".into(),
                material: String::new(),
                solid: &peg,
                key: (3, 0),
            },
            Part {
                name: "Peg".into(),
                material: String::new(),
                solid: &peg2,
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
        let text = String::from_utf8_lossy(&sheet.to_pdf()).to_string();
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
        let text = String::from_utf8_lossy(&sheet.to_pdf()).to_string();
        assert!(text.contains("(\\33010)"), "a diameter callout");
        assert!(
            text.contains("[2 1] 0 d"),
            "the hole is hidden lines from the front"
        );
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
                solid: s,
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
