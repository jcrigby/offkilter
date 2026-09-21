//! Jigsaw puzzle geometry: a grid of pieces whose shared edges carry
//! interlocking tabs, as 2D lines and arcs ready to become a sketch.
//!
//! The pieces are meant to be pin-routed from solid boards, one board
//! per colour of a checkerboard, so every curve is made of straight
//! lines and tangent arcs whose radii the router bit can follow, and
//! [`build`] refuses a design the bit cannot cut. A gap between the
//! pieces (for a resin fill, or nothing) is an inward offset of every
//! outline, which keeps lines and arcs as lines and arcs.
//!
//! Grid nodes are `(i, j)` for `i` in `0..=cols`, `j` in `0..=rows` at
//! `pitch` spacing; interior nodes may be moved. Interior edges carry one
//! tab each: horizontal edges first (`j` in `1..rows`, `i` in `0..cols`,
//! index `(j - 1) * cols + i`), then vertical (`i` in `1..cols`, `j` in
//! `0..rows`, index `h + (i - 1) * rows + j`). A tab that is `out` bulges
//! towards the higher-index piece (up for a horizontal edge, right for a
//! vertical one), so the lower-index piece owns it.

use crate::Sketch;
use ok_math::Vec2;

/// One interlocking tab on an interior edge.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Tab {
    /// Bulges towards the higher-index piece.
    pub out: bool,
    /// Scales the whole tab (neck, shoulders, head); 1 is the default.
    #[serde(default = "one")]
    pub size: f64,
    /// Scales the neck width alone; 1 is the default.
    #[serde(default = "one")]
    pub width: f64,
    /// Where along the edge the tab sits, as a fraction of its length.
    #[serde(default = "half")]
    pub shift: f64,
}

fn one() -> f64 {
    1.0
}

fn half() -> f64 {
    0.5
}

impl Default for Tab {
    fn default() -> Tab {
        Tab {
            out: true,
            size: 1.0,
            width: 1.0,
            shift: 0.5,
        }
    }
}

/// Which way the wood grain runs across the puzzle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Grain {
    #[default]
    X,
    Y,
}

/// Everything that shapes the puzzle.
#[derive(Debug, Clone, PartialEq)]
pub struct Params {
    pub cols: u32,
    pub rows: u32,
    /// Nominal piece size (the grid spacing), mm.
    pub pitch: f64,
    /// Gap between neighbouring pieces, mm (0 for a tight fit).
    pub gap: f64,
    /// Diameter of the pin router bit, mm.
    pub bit: f64,
    /// How far the neck walls lean in under the head, degrees.
    pub lock: f64,
    pub grain: Grain,
    /// One per interior edge, in edge order.
    pub tabs: Vec<Tab>,
    /// Offset of every interior node from its grid position, row by row
    /// (`j` in `1..rows`, `i` in `1..cols`, index `(j - 1) * (cols - 1) + (i - 1)`).
    pub corners: Vec<Vec2>,
    /// A strip between rows, mm. Rows become bands a strip apart, so
    /// pieces of one colour never meet at a corner and a tight fit
    /// routes cleanly. The tabs on those edges keep their full shape and
    /// reach across the strip; the socket opposite is the tab grown by
    /// the strip width, so the tab sits in a moat as wide as the strip.
    /// Zero for a full jigsaw.
    pub row_gap: f64,
}

/// A line or an arc of an outline.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Seg {
    Line {
        a: Vec2,
        b: Vec2,
    },
    Arc {
        center: Vec2,
        radius: f64,
        start: Vec2,
        end: Vec2,
        /// Travels counter-clockwise from `start` to `end`.
        ccw: bool,
    },
}

impl Seg {
    pub fn start(&self) -> Vec2 {
        match self {
            Seg::Line { a, .. } => *a,
            Seg::Arc { start, .. } => *start,
        }
    }

    pub fn end(&self) -> Vec2 {
        match self {
            Seg::Line { b, .. } => *b,
            Seg::Arc { end, .. } => *end,
        }
    }

    fn reversed(&self) -> Seg {
        match *self {
            Seg::Line { a, b } => Seg::Line { a: b, b: a },
            Seg::Arc {
                center,
                radius,
                start,
                end,
                ccw,
            } => Seg::Arc {
                center,
                radius,
                start: end,
                end: start,
                ccw: !ccw,
            },
        }
    }

    fn set_start(&mut self, p: Vec2) {
        match self {
            Seg::Line { a, .. } => *a = p,
            Seg::Arc { start, .. } => *start = p,
        }
    }

    fn set_end(&mut self, p: Vec2) {
        match self {
            Seg::Line { b, .. } => *b = p,
            Seg::Arc { end, .. } => *end = p,
        }
    }
}

/// One piece of the puzzle.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Piece {
    pub col: u32,
    pub row: u32,
    /// The checkerboard colour: `(col + row)` even.
    pub light: bool,
    /// Counter-clockwise outline, the gap already taken off.
    pub outline: Vec<Seg>,
}

/// Where a tab's head is, for a plan to show and click.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TabHead {
    pub at: Vec2,
    pub out: bool,
}

/// The built puzzle.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Jigsaw {
    pub pieces: Vec<Piece>,
    pub width: f64,
    pub height: f64,
    /// Every tab's head, in edge order.
    pub tab_heads: Vec<TabHead>,
    /// Every interior node's position, in corner order (the middle of the
    /// strip when there is a row gap).
    pub nodes: Vec<Vec2>,
    /// What the pin router (or the geometry) cannot do with the design,
    /// one rule per entry; empty when the puzzle can be cut.
    pub problems: Vec<String>,
    /// What the bit will do to the design anyway, worth knowing: the
    /// rounds it leaves where pieces of one colour meet at a corner.
    pub notes: Vec<String>,
}

/// Number of interior edges (tabs) of a grid.
pub fn edge_count(cols: u32, rows: u32) -> usize {
    ((rows.saturating_sub(1)) * cols + (cols.saturating_sub(1)) * rows) as usize
}

/// Number of interior nodes (movable corners) of a grid.
pub fn node_count(cols: u32, rows: u32) -> usize {
    (cols.saturating_sub(1) * rows.saturating_sub(1)) as usize
}

/// A small deterministic generator (xorshift64*), so a seed always gives
/// the same puzzle.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in `[-1, 1)`.
    fn signed(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    }
}

/// Tabs for every interior edge with random directions from `seed`.
pub fn seed_tabs(cols: u32, rows: u32, seed: u64) -> Vec<Tab> {
    let mut rng = Rng::new(seed);
    (0..edge_count(cols, rows))
        .map(|_| Tab {
            out: rng.next() & 1 == 1,
            ..Tab::default()
        })
        .collect()
}

/// Offsets for every interior node, uniform within `±jitter` (mm) from
/// `seed`; zero jitter gives a regular grid.
pub fn seed_corners(cols: u32, rows: u32, jitter: f64, seed: u64) -> Vec<Vec2> {
    let mut rng = Rng::new(seed ^ 0x5EED);
    (0..node_count(cols, rows))
        .map(|_| Vec2::new(rng.signed() * jitter, rng.signed() * jitter))
        .collect()
}

/// The tab's proportions of the pitch (neck half-width, shoulder radius,
/// head radius): a tab about a third of the pitch tall, so sockets on
/// neighbouring edges of one piece keep clear of each other even when
/// the corners wander, and shoulders that clear a quarter-inch bit from
/// a pitch of 38 mm up.
const NECK: f64 = 0.09;
const SHOULDER: f64 = 0.085;
const HEAD: f64 = 0.12;
/// Clear of the piece corners, as a fraction of the edge length.
const MARGIN: f64 = 0.15;

/// Builds every piece outline, or says what the pin router (or the
/// geometry) cannot do with the design: every violated rule, one per
/// line, so a design can be fixed in one go.
pub fn build(p: &Params) -> Result<Jigsaw, String> {
    let jig = plan(p)?;
    if jig.problems.is_empty() {
        Ok(jig)
    } else {
        Err(jig.problems.join("\n"))
    }
}

/// Builds every piece outline whether or not the design can be cut,
/// listing the violated rules in `problems`, so a plan can be drawn and
/// fixed. Only a grid that cannot be laid out at all is an error.
pub fn plan(p: &Params) -> Result<Jigsaw, String> {
    let mut problems: Vec<String> = Vec::new();
    if p.cols < 1 || p.rows < 1 || p.cols > 64 || p.rows > 64 {
        return Err("a puzzle needs 1 to 64 columns and rows".into());
    }
    if p.pitch <= 0.0 || !p.pitch.is_finite() {
        return Err("pitch must be positive".into());
    }
    if p.gap < 0.0 || !p.gap.is_finite() {
        return Err("gap must be zero or positive".into());
    }
    if p.bit <= 0.0 || !p.bit.is_finite() {
        return Err("bit diameter must be positive".into());
    }
    if !(1.0..=45.0).contains(&p.lock) {
        return Err("lock angle must be between 1 and 45 degrees".into());
    }
    if p.row_gap < 0.0 || !p.row_gap.is_finite() {
        return Err("row gap must be zero or positive".into());
    }
    if p.tabs.len() != edge_count(p.cols, p.rows) {
        return Err(format!(
            "{} tabs for {} interior edges",
            p.tabs.len(),
            edge_count(p.cols, p.rows)
        ));
    }
    if p.corners.len() != node_count(p.cols, p.rows) {
        return Err(format!(
            "{} corner offsets for {} interior nodes",
            p.corners.len(),
            node_count(p.cols, p.rows)
        ));
    }
    let (cols, rows) = (p.cols as usize, p.rows as usize);
    let h = p.row_gap;
    let mut notes: Vec<String> = Vec::new();
    // Rows are bands `pitch` tall, `h` apart. A node on boundary `j` (the
    // top of row j - 1, the bottom of row j) sits at column i's x plus
    // the corner offset; with a row gap it splits into two, one on each
    // band, sharing the offset so the strip keeps its width.
    let band_bottom = |j: usize| j as f64 * (p.pitch + h);
    let node_at = |i: usize, boundary: usize, y: f64| -> Vec2 {
        let base = Vec2::new(i as f64 * p.pitch, y);
        if i > 0 && i < cols && boundary > 0 && boundary < rows {
            base + p.corners[(boundary - 1) * (cols - 1) + (i - 1)]
        } else {
            base
        }
    };
    // The bottom and top node of row j at column i.
    let bottom = |i: usize, j: usize| node_at(i, j, band_bottom(j));
    let top = |i: usize, j: usize| node_at(i, j + 1, band_bottom(j) + p.pitch);
    let max_jitter = 0.3 * p.pitch;
    for (k, c) in p.corners.iter().enumerate() {
        if c.length() > max_jitter + 1e-9 {
            problems.push(format!(
                "corner {k} is moved {:.1} mm, more than 30 % of the pitch",
                c.length()
            ));
        }
    }
    // Cells must stay convex and their edges reasonably long, or the
    // tabs run into the corners.
    for j in 0..rows {
        for i in 0..cols {
            let q = [bottom(i, j), bottom(i + 1, j), top(i + 1, j), top(i, j)];
            for k in 0..4 {
                let (a, b, c) = (q[k], q[(k + 1) % 4], q[(k + 2) % 4]);
                if (b - a).cross(c - b) <= 0.0 {
                    problems.push(format!("piece {},{} is not convex", i + 1, j + 1));
                    break;
                }
            }
            for k in 0..4 {
                if q[k].distance(q[(k + 1) % 4]) < 0.6 * p.pitch {
                    problems.push(format!(
                        "piece {},{} has an edge shorter than 60 % of the pitch",
                        i + 1,
                        j + 1
                    ));
                    break;
                }
            }
        }
    }
    let h_count = (rows - 1) * cols;
    let rb = p.bit / 2.0;
    let half_gap = p.gap / 2.0;
    // Every edge as segments from its first node to its second.
    let mut h_edges: Vec<Vec<Seg>> = Vec::new();
    let mut v_edges: Vec<Vec<Seg>> = Vec::new();
    let mut tab_heads: Vec<TabHead> = Vec::new();
    // `tab_in` and `socket_in` are how far each side's outline moves in
    // from the curve as drawn: half the gap each for a shared edge, or
    // nothing on the tab's side and the whole strip on the socket's side
    // across a row gap.
    let mut edge = |a: Vec2,
                    b: Vec2,
                    tab: Option<(usize, &Tab)>,
                    n_sign: f64,
                    tab_in: f64,
                    socket_in: f64|
     -> Vec<Seg> {
        let Some((index, tab)) = tab else {
            return vec![Seg::Line { a, b }];
        };
        let name = if index < h_count {
            format!(
                "tab on the edge above piece {},{}",
                index % cols + 1,
                index / cols + 1
            )
        } else {
            let k = index - h_count;
            format!(
                "tab on the edge right of piece {},{}",
                k / rows + 1,
                k % rows + 1
            )
        };
        let len = a.distance(b);
        let u = (b - a) * (1.0 / len);
        // The bulge side: the higher-index piece for `out`. When (u, v) is
        // left-handed the profile's turns come out mirrored.
        let v = u.perp() * n_sign * if tab.out { 1.0 } else { -1.0 };
        let right_handed = u.cross(v) > 0.0;
        let size = tab.size.max(0.05);
        let neck = NECK * p.pitch * size * tab.width.max(0.05);
        let s = SHOULDER * p.pitch * size;
        let rh = HEAD * p.pitch * size;
        let phi = p.lock.to_radians();
        let c = tab.shift.clamp(0.0, 1.0) * len;
        // The wall length that lets the head sit on the centre line.
        let wall = (rh * phi.cos() - neck - s * (1.0 - phi.cos())) / phi.sin();
        if wall < 0.0 {
            problems.push(format!(
                "{name}: the head is too small for its neck at a {:.0}° lock; make the tab bigger or the neck narrower",
                p.lock
            ));
        }
        // Pin router rules, on the outlines as cut. The socket's convex
        // shoulders may vanish under a wide offset (they trim to a sharp
        // corner, which a bit cuts fine); the tab's head may not.
        if 2.0 * neck + 2.0 * socket_in < p.bit - 1e-9 {
            problems.push(format!(
                "{name}: the socket opening ({:.2} mm) is narrower than the {:.2} mm bit",
                2.0 * neck + 2.0 * socket_in,
                p.bit
            ));
        }
        if s + tab_in < rb - 1e-9 {
            problems.push(format!(
                "{name}: the shoulder radius ({:.2} mm as cut) is under the bit's {:.2}",
                s + tab_in,
                rb
            ));
        }
        if rh + socket_in < rb - 1e-9 {
            problems.push(format!(
                "{name}: the socket's head radius ({:.2} mm as cut) is under the bit's {:.2}",
                rh + socket_in,
                rb
            ));
        }
        if tab_in >= rh - 0.25 {
            problems.push(format!(
                "{name}: the gap ({:.2} mm) leaves no head on the tab",
                2.0 * tab_in
            ));
        }
        // Necks across the grain are short grain: keep them wider.
        let across = match p.grain {
            Grain::X => index < h_count,
            Grain::Y => index >= h_count,
        };
        let min_neck = (0.05 * p.pitch).max(2.0) * if across { 1.5 } else { 1.0 };
        if 2.0 * neck - 2.0 * tab_in < min_neck - 1e-9 {
            problems.push(format!(
                "{name}: the neck ({:.2} mm as cut) is thinner than {:.2} mm{}",
                2.0 * neck - 2.0 * tab_in,
                min_neck,
                if across { " across the grain" } else { "" }
            ));
        }
        if c - rh < MARGIN * len || c + rh > len - MARGIN * len {
            problems.push(format!(
                "{name}: the head comes within {:.0} % of the edge length of a corner",
                MARGIN * 100.0
            ));
        }
        // Local profile, u along the edge, v towards the bulge.
        let at = |x: f64, y: f64| a + u * x + v * y;
        let c1 = (c - neck - s, s);
        let p0 = at(c1.0, 0.0);
        let p1 = at(c1.0 + s * phi.cos(), s + s * phi.sin());
        let p2 = at(
            c1.0 + s * phi.cos() - wall.max(0.0) * phi.sin(),
            s + s * phi.sin() + wall.max(0.0) * phi.cos(),
        );
        let head_v = s + s * phi.sin() + wall.max(0.0) * phi.cos() + rh * phi.sin();
        let head = at(c, head_v);
        let top = head_v + rh;
        if top > 0.45 * p.pitch {
            problems.push(format!(
                "{name}: the tab stands {:.1} mm proud, more than 45 % of the pitch",
                top
            ));
        }
        let mirror = |x: f64, y: f64| at(2.0 * c - x, y);
        let p3 = mirror(
            c1.0 + s * phi.cos() - wall.max(0.0) * phi.sin(),
            s + s * phi.sin() + wall.max(0.0) * phi.cos(),
        );
        let p4 = mirror(c1.0 + s * phi.cos(), s + s * phi.sin());
        let p5 = at(c + neck + s, 0.0);
        tab_heads.push(TabHead {
            at: head,
            out: tab.out,
        });
        vec![
            Seg::Line { a, b: p0 },
            Seg::Arc {
                center: at(c1.0, c1.1),
                radius: s,
                start: p0,
                end: p1,
                ccw: right_handed,
            },
            Seg::Line { a: p1, b: p2 },
            Seg::Arc {
                center: head,
                radius: rh,
                start: p2,
                end: p3,
                ccw: !right_handed,
            },
            Seg::Line { a: p3, b: p4 },
            Seg::Arc {
                center: at(c + neck + s, s),
                radius: s,
                start: p4,
                end: p5,
                ccw: right_handed,
            },
            Seg::Line { a: p5, b },
        ]
    };
    // Horizontal edges, one per boundary. An interior one carries a tab
    // drawn on the line of the piece that owns it: the lower piece's top
    // for a tab bulging up, the upper piece's bottom for one bulging
    // down; with no row gap those lines coincide.
    for j in 0..=rows {
        for i in 0..cols {
            let tab =
                (j > 0 && j < rows).then(|| ((j - 1) * cols + i, &p.tabs[(j - 1) * cols + i]));
            let (a, b) = match tab {
                Some((_, t)) if t.out => (top(i, j - 1), top(i + 1, j - 1)),
                Some(_) => (bottom(i, j), bottom(i + 1, j)),
                None if j == 0 => (bottom(i, 0), bottom(i + 1, 0)),
                None => (top(i, rows - 1), top(i + 1, rows - 1)),
            };
            let (tab_in, socket_in) = if h > 0.0 {
                (0.0, h)
            } else {
                (half_gap, half_gap)
            };
            h_edges.push(edge(a, b, tab, 1.0, tab_in, socket_in));
        }
    }
    for i in 0..=cols {
        for j in 0..rows {
            let tab = (i > 0 && i < cols).then(|| {
                (
                    h_count + (i - 1) * rows + j,
                    &p.tabs[h_count + (i - 1) * rows + j],
                )
            });
            v_edges.push(edge(bottom(i, j), top(i, j), tab, -1.0, half_gap, half_gap));
        }
    }
    // What the bit does at the corners. Around a convex corner it sweeps
    // a quarter disc of radius one bit diameter on the far side, which on
    // a checkerboard is the same colour's diagonal neighbour: a tight fit
    // gets a round hole at every node unless a gap or a row strip keeps
    // that neighbour clear.
    if h > 0.0 {
        if h < p.bit {
            notes.push(format!(
                "a {:.1} mm row gap under the bit's {:.2} mm leaves every piece corner rounded to {:.1} mm",
                h,
                p.bit,
                p.bit - h
            ));
        }
    } else {
        let bite = p.bit - p.gap * 2f64.sqrt();
        if bite > 0.05 && rows > 1 && cols > 1 {
            notes.push(format!(
                "cut from one board per colour, the bit rounds every interior corner back {:.1} mm ({:.1} mm holes at the nodes); a gap of {:.1} mm or a row gap of {:.2} mm avoids it",
                bite,
                2.0 * bite,
                p.bit / 2f64.sqrt(),
                p.bit
            ));
        }
    }
    // Pieces: bottom, right, top (reversed), left (reversed), then the gap.
    let mut pieces = Vec::new();
    for j in 0..rows {
        for i in 0..cols {
            // The board's own edges stay where they are. A shared edge
            // moves in half the gap on both pieces; across a row gap the
            // tab's piece keeps the curve and the socket's piece moves in
            // by the strip.
            let mut outline: Vec<Seg> = Vec::new();
            let mut inset: Vec<f64> = Vec::new();
            let mut side = |segs: Vec<Seg>, d: f64| {
                inset.extend(segs.iter().map(|_| d));
                outline.extend(segs);
            };
            let across = |boundary: usize, this_is_upper: bool| -> f64 {
                if boundary == 0 || boundary == rows {
                    0.0
                } else if h > 0.0 {
                    // The upper piece has the socket of a tab bulging up.
                    let out = p.tabs[(boundary - 1) * cols + i].out;
                    if out == this_is_upper {
                        h
                    } else {
                        0.0
                    }
                } else {
                    half_gap
                }
            };
            side(h_edges[j * cols + i].clone(), across(j, true));
            side(
                v_edges[(i + 1) * rows + j].clone(),
                if i + 1 == cols { 0.0 } else { half_gap },
            );
            side(
                h_edges[(j + 1) * cols + i]
                    .iter()
                    .rev()
                    .map(|s| s.reversed())
                    .collect(),
                across(j + 1, false),
            );
            side(
                v_edges[i * rows + j]
                    .iter()
                    .rev()
                    .map(|s| s.reversed())
                    .collect(),
                if i == 0 { 0.0 } else { half_gap },
            );
            let outline = if inset.iter().any(|d| *d > 0.0) {
                offset_loop(&outline, &inset)
            } else {
                outline
            };
            if self_intersects(&outline) {
                problems.push(format!(
                    "piece {},{}: its tabs or sockets run into each other; move, shrink or flip a tab, or move a corner back",
                    i + 1,
                    j + 1
                ));
            }
            pieces.push(Piece {
                col: i as u32,
                row: j as u32,
                light: (i + j) % 2 == 0,
                outline,
            });
        }
    }
    let nodes = (1..rows)
        .flat_map(|j| (1..cols).map(move |i| (i, j)))
        .map(|(i, j)| node_at(i, j, band_bottom(j) - h / 2.0))
        .collect();
    Ok(Jigsaw {
        pieces,
        width: cols as f64 * p.pitch,
        height: rows as f64 * p.pitch + (rows as f64 - 1.0) * h,
        tab_heads,
        nodes,
        problems,
        notes,
    })
}

/// An outline moved `d` inward all round (negative grows it): the
/// pocket a piece drops into, say.
pub fn offset(outline: &[Seg], d: f64) -> Vec<Seg> {
    offset_loop(outline, &vec![d; outline.len()])
}

/// Whether an outline crosses itself, tested on a 10° polygonisation of
/// its arcs (neighbouring segments share an endpoint and do not count).
fn self_intersects(outline: &[Seg]) -> bool {
    let mut pts: Vec<Vec2> = Vec::new();
    for s in outline {
        match *s {
            Seg::Line { a, .. } => pts.push(a),
            Seg::Arc {
                center,
                radius,
                start,
                end,
                ccw,
            } => {
                let a0 = (start - center).y.atan2((start - center).x);
                let mut sweep = (end - center).y.atan2((end - center).x) - a0;
                if ccw && sweep <= 0.0 {
                    sweep += std::f64::consts::TAU;
                }
                if !ccw && sweep >= 0.0 {
                    sweep -= std::f64::consts::TAU;
                }
                let n = ((sweep.abs() / 10f64.to_radians()).ceil() as usize).max(1);
                for k in 0..n {
                    let t = a0 + sweep * k as f64 / n as f64;
                    pts.push(center + Vec2::new(t.cos(), t.sin()) * radius);
                }
            }
        }
    }
    let n = pts.len();
    let crosses = |a: Vec2, b: Vec2, c: Vec2, d: Vec2| -> bool {
        let d1 = (b - a).cross(c - a);
        let d2 = (b - a).cross(d - a);
        let d3 = (d - c).cross(a - c);
        let d4 = (d - c).cross(b - c);
        d1 * d2 < 0.0 && d3 * d4 < 0.0
    };
    for i in 0..n {
        for j in i + 2..n {
            if i == 0 && j == n - 1 {
                continue;
            }
            if crosses(pts[i], pts[(i + 1) % n], pts[j], pts[(j + 1) % n]) {
                return true;
            }
        }
    }
    false
}

/// A counter-clockwise loop of tangent-joined segments, each moved its
/// own distance inward. Lines shift along their left normal, arcs change
/// radius; a convex arc that shrinks away is dropped and its neighbours
/// meet at a corner. Non-tangent joins between lines intersect.
fn offset_loop(segs: &[Seg], inset: &[f64]) -> Vec<Seg> {
    let mut out: Vec<Seg> = segs
        .iter()
        .zip(inset)
        .filter_map(|(s, &d)| match *s {
            Seg::Line { a, b } => {
                let n = (b - a).perp().normalized().unwrap_or(Vec2::ZERO);
                Some(Seg::Line {
                    a: a + n * d,
                    b: b + n * d,
                })
            }
            Seg::Arc {
                center,
                radius,
                start,
                end,
                ccw,
            } => {
                // Turning left, the inside is towards the centre.
                let r = if ccw { radius - d } else { radius + d };
                if r <= 1e-9 {
                    return None;
                }
                let k = if radius.abs() > 1e-12 {
                    r / radius
                } else {
                    1.0
                };
                Some(Seg::Arc {
                    center,
                    radius: r,
                    start: center + (start - center) * k,
                    end: center + (end - center) * k,
                    ccw,
                })
            }
        })
        .collect();
    let n = out.len();
    for k in 0..n {
        let (i, j) = (k, (k + 1) % n);
        let joint = match (out[i], out[j]) {
            (Seg::Line { a, b }, Seg::Line { a: c, b: e }) => {
                let (da, db) = (b - a, e - c);
                let den = da.cross(db);
                if den.abs() > 1e-12 {
                    let t = (c - a).cross(db) / den;
                    a + da * t
                } else {
                    (b + c) * 0.5
                }
            }
            (p, q) => (p.end() + q.start()) * 0.5,
        };
        out[i].set_end(joint);
        out[j].set_start(joint);
    }
    out
}

/// Draws an outline into a sketch as lines and arcs whose endpoints meet.
pub fn draw(outline: &[Seg], sk: &mut Sketch) {
    for s in outline {
        match *s {
            Seg::Line { a, b } => {
                sk.add_line(a, b);
            }
            Seg::Arc {
                center,
                start,
                end,
                ccw,
                ..
            } => {
                if ccw {
                    sk.add_arc(center, start, end);
                } else {
                    sk.add_arc(center, end, start);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProfileOptions;

    fn params(cols: u32, rows: u32, gap: f64) -> Params {
        Params {
            cols,
            rows,
            pitch: 40.0,
            gap,
            bit: 6.35,
            lock: 20.0,
            grain: Grain::X,
            tabs: seed_tabs(cols, rows, 7),
            corners: seed_corners(cols, rows, 0.0, 7),
            row_gap: 0.0,
        }
    }

    fn area(outline: &[Seg]) -> f64 {
        let mut sk = Sketch::new();
        draw(outline, &mut sk);
        let p = sk.profiles(&ProfileOptions::default());
        assert_eq!(p.len(), 1, "one region per piece, got {}", p.len());
        p[0].area()
    }

    #[test]
    fn pieces_tile_the_board_and_tabs_interlock() {
        let p = params(4, 3, 0.0);
        let j = build(&p).unwrap();
        assert_eq!(j.pieces.len(), 12);
        assert_eq!(j.tab_heads.len(), edge_count(4, 3));
        assert_eq!(edge_count(4, 3), 2 * 4 + 3 * 3);
        // Every outline closes and is counter-clockwise.
        for piece in &j.pieces {
            let n = piece.outline.len();
            for k in 0..n {
                let gap = piece.outline[k]
                    .end()
                    .distance(piece.outline[(k + 1) % n].start());
                assert!(
                    gap < 1e-9,
                    "piece {},{} has a break of {gap}",
                    piece.col,
                    piece.row
                );
            }
            assert!(area(&piece.outline) > 0.0);
        }
        // With no gap the pieces cover the board exactly: tabs and sockets
        // cancel over the whole set.
        let total: f64 = j.pieces.iter().map(|p| area(&p.outline)).sum();
        assert!((total - 4.0 * 3.0 * 1600.0).abs() < 0.5, "{total}");
        // Tabs move area between neighbours: a piece with more tabs than
        // sockets is bigger than its cell, and the amounts are multiples
        // of one tab's area.
        let deviations: Vec<f64> = j.pieces.iter().map(|p| area(&p.outline) - 1600.0).collect();
        let unit = deviations
            .iter()
            .map(|d| d.abs())
            .filter(|d| *d > 1.0)
            .fold(f64::MAX, f64::min);
        assert!(unit > 100.0 && unit < 400.0, "{deviations:?}");
        // (Polygonised arcs: a tab and its socket differ by a few thousandths.)
        for d in &deviations {
            let k = d / unit;
            assert!(
                (k - k.round()).abs() < 1e-3,
                "{d} is not a whole number of tabs ({unit})"
            );
        }
    }

    #[test]
    fn a_gap_shrinks_every_piece_by_its_perimeter() {
        let tight = build(&params(3, 2, 0.0)).unwrap();
        let loose = build(&params(3, 2, 1.0)).unwrap();
        for (a, b) in tight.pieces.iter().zip(&loose.pieces) {
            assert_eq!(a.outline.len(), b.outline.len());
            let (aa, ab) = (area(&a.outline), area(&b.outline));
            assert!(ab < aa - 40.0, "piece {},{}: {aa} vs {ab}", a.col, a.row);
            // Every joint still closes after the offset.
            let n = b.outline.len();
            for k in 0..n {
                assert!(b.outline[k].end().distance(b.outline[(k + 1) % n].start()) < 1e-9);
            }
        }
        // The board's edges stay put; the gap is between pieces only.
        let first = &loose.pieces[0].outline;
        assert!(
            first[0].start().distance(Vec2::ZERO) < 1e-9,
            "{:?}",
            first[0]
        );
        assert!(
            first[0].end().distance(Vec2::new(39.5, 0.0)) < 1e-9,
            "{:?}",
            first[0]
        );
        let last = &loose.pieces[5].outline;
        assert!(last
            .iter()
            .any(|s| s.end().distance(Vec2::new(120.0, 80.0)) < 1e-9));
    }

    #[test]
    fn the_pin_router_rules_name_what_it_cannot_cut() {
        let mut p = params(2, 2, 0.0);
        p.bit = 12.0;
        let err = build(&p).unwrap_err();
        assert!(err.contains("socket opening"), "{err}");
        assert!(err.contains("shoulder radius"), "{err}");
        let mut p = params(2, 2, 0.0);
        p.tabs[0].shift = 0.05;
        let err = build(&p).unwrap_err();
        assert!(err.contains("corner"), "{err}");
        let mut p = params(2, 2, 0.0);
        p.tabs[0].width = 0.2;
        let err = build(&p).unwrap_err();
        assert!(err.contains("neck"), "{err}");
        let mut p = params(2, 2, 0.0);
        p.corners[0] = Vec2::new(15.0, 15.0);
        let err = build(&p).unwrap_err();
        assert!(err.contains("30 %"), "{err}");
        let mut p = params(2, 2, 6.0);
        p.tabs[0].size = 0.6;
        assert!(build(&p).is_err());
        // The plan still draws a refused design, with the rules alongside.
        let j = plan(&p).unwrap();
        assert_eq!(j.pieces.len(), 4);
        assert!(!j.problems.is_empty());
        // Two sockets of one piece shifted into each other at its corner.
        let mut p = params(2, 2, 0.0);
        p.tabs = vec![
            Tab {
                out: false,
                ..Tab::default()
            };
            4
        ];
        p.tabs[0].shift = 0.7;
        p.tabs[2].shift = 0.7;
        let err = build(&p).unwrap_err();
        assert!(err.contains("run into each other"), "{err}");
    }

    #[test]
    fn a_row_gap_makes_bands_whose_tabs_cross_the_strip() {
        let mut p = params(3, 2, 0.0);
        p.row_gap = 8.0;
        let j = build(&p).unwrap();
        assert!((j.height - (80.0 + 8.0)).abs() < 1e-9);
        assert!(j.notes.is_empty(), "{:?}", j.notes);
        assert_eq!(j.tab_heads.len(), edge_count(3, 2));
        for piece in &j.pieces {
            let n = piece.outline.len();
            for k in 0..n {
                assert!(
                    piece.outline[k]
                        .end()
                        .distance(piece.outline[(k + 1) % n].start())
                        < 1e-9,
                    "piece {},{} breaks at {k}",
                    piece.col,
                    piece.row
                );
            }
            assert!(area(&piece.outline) > 0.0);
        }
        // Across each strip the tab's piece keeps its whole tab and gains
        // its area; the socket's piece is cut by the tab grown by the
        // strip, its shoulders trimmed to corners, and loses more.
        for i in 0..3 {
            let (lower, upper) = (&j.pieces[i], &j.pieces[3 + i]);
            let (owner, socket) = if p.tabs[i].out {
                (lower, upper)
            } else {
                (upper, lower)
            };
            assert!(socket.outline.len() < owner.outline.len());
            // (Most of the tab stands in the strip; the socket only takes
            // the head, grown by the strip.)
            assert!(
                area(&owner.outline) > 1600.0 + 50.0,
                "{}",
                area(&owner.outline)
            );
            assert!(
                area(&socket.outline) < 1600.0 - 100.0,
                "{}",
                area(&socket.outline)
            );
        }
        // A strip narrower than the bit rounds the corners: a note, not a refusal.
        p.row_gap = 4.0;
        let j = build(&p).unwrap();
        assert!(j.notes[0].contains("rounded to 2.3"), "{:?}", j.notes);
        // A full jigsaw cut tight gets holes at the nodes: also a note.
        p.row_gap = 0.0;
        let j = build(&p).unwrap();
        assert!(j.notes[0].contains("holes at the nodes"), "{:?}", j.notes);
        p.gap = 5.0;
        assert!(plan(&p).unwrap().notes.is_empty());
    }

    #[test]
    fn seeds_are_deterministic_and_jitter_moves_corners() {
        assert_eq!(seed_tabs(5, 4, 3), seed_tabs(5, 4, 3));
        assert_ne!(seed_tabs(5, 4, 3), seed_tabs(5, 4, 4));
        let outs = seed_tabs(8, 8, 1).iter().filter(|t| t.out).count();
        assert!(outs > 30 && outs < 82, "{outs} of 112 tabs out");
        let c = seed_corners(5, 4, 3.0, 9);
        assert_eq!(c.len(), 12);
        assert!(c.iter().all(|v| v.x.abs() <= 3.0 && v.y.abs() <= 3.0));
        assert!(c.iter().any(|v| v.length() > 0.5));
        let j = build(&Params {
            corners: c,
            ..params(5, 4, 0.5)
        })
        .unwrap();
        assert_eq!(j.nodes.len(), 12);
        assert!(j.nodes[0].distance(Vec2::new(40.0, 40.0)) > 0.1);
    }
}
