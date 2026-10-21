//! Ripping the straight Spectre: the tile as a glue-up of strip
//! profiles, each a convex shape a table saw rips through, assembled
//! into the tile. The tile sits on lines at every 30° through its
//! vertices; a cut runs along one of them between two grid points
//! (vertices and crossings) or from an inside corner until it meets the
//! outline or an earlier cut. The cuts partition the tile into pieces;
//! each piece is read as an extrusion's cross-section and checked
//! against the saw: a face whose line leaves the piece at both ends is
//! a through rip; a face that ends at an inside corner is a rabbet,
//! the blade coming in from the open end and stopping at the corner; a
//! face between two inside corners is a dado; and every cut needs a
//! face the whole piece sits to one side of, within a 45° blade tilt.
//! The search tries every set of up to four cuts from the inside
//! corners, then one more from any corner, and ranks the results by
//! distinct profiles (mirror images alike, since stock just flips),
//! pieces, rabbets and the thinnest piece.

use super::{vertex, ANGLES};
use ok_math::Vec2;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::OnceLock;

/// A cut: two points in tile units, on one 30° line.
pub type Cut = [Vec2; 2];

const EPS: f64 = 1e-7;

fn cross(a: Vec2, b: Vec2) -> f64 {
    a.x * b.y - a.y * b.x
}

fn same(a: Vec2, b: Vec2) -> bool {
    a.distance(b) < 1e-6
}

/// The straight tile, counter-clockwise, unit edges.
pub fn tile() -> Vec<Vec2> {
    (0..14).map(vertex).collect()
}

fn area(p: &[Vec2]) -> f64 {
    let n = p.len();
    (0..n).map(|i| cross(p[i], p[(i + 1) % n])).sum::<f64>() / 2.0
}

fn point_in(p: Vec2, poly: &[Vec2]) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn on_edge(p: Vec2, poly: &[Vec2]) -> bool {
    let n = poly.len();
    (0..n).any(|i| {
        let a = poly[i];
        let d = poly[(i + 1) % n] - a;
        let t = (p - a).dot(d) / d.dot(d);
        t > -1e-9 && t < 1.0 + 1e-9 && cross(d, p - a).abs() / d.length() < 1e-6
    })
}

/// Drops repeated and flat vertices.
fn clean(p: &[Vec2]) -> Vec<Vec2> {
    let mut q: Vec<Vec2> = Vec::new();
    for &v in p {
        if q.last().is_none_or(|l| !same(*l, v)) {
            q.push(v);
        }
    }
    while q.len() > 1 && same(q[0], *q.last().unwrap()) {
        q.pop();
    }
    let n = q.len();
    (0..n)
        .filter(|&i| {
            let (a, b, c) = (q[(i + n - 1) % n], q[i], q[(i + 1) % n]);
            cross(b - a, c - b).abs() > 1e-9
        })
        .map(|i| q[i])
        .collect()
}

/// The first point where a ray from `v` along `d` meets the polygon,
/// strictly beyond `v`.
fn ray_hit(p: &[Vec2], v: Vec2, d: Vec2) -> Option<Vec2> {
    let n = p.len();
    let mut best: Option<(f64, Vec2)> = None;
    for i in 0..n {
        let a = p[i];
        let e = p[(i + 1) % n] - a;
        let den = cross(d, e);
        if den.abs() < 1e-12 {
            continue;
        }
        let t = cross(a - v, e) / den;
        let u = cross(a - v, d) / den;
        if t > EPS && u > -1e-9 && u < 1.0 + 1e-9 && best.is_none_or(|(bt, _)| t < bt) {
            best = Some((t, v + d * t));
        }
    }
    best.map(|(_, q)| q)
}

/// Splits a polygon by the chord `a`–`b`, both on its boundary.
fn split_at(p: &[Vec2], a: Vec2, b: Vec2) -> Option<[Vec<Vec2>; 2]> {
    let mut q: Vec<Vec2> = p.to_vec();
    for pt in [a, b] {
        if q.iter().any(|v| same(*v, pt)) {
            continue;
        }
        let n = q.len();
        for i in 0..n {
            let s = q[i];
            let d = q[(i + 1) % n] - s;
            let t = (pt - s).dot(d) / d.dot(d);
            if t > 1e-9 && t < 1.0 - 1e-9 && cross(d, pt - s).abs() / d.length() < 1e-6 {
                q.insert(i + 1, pt);
                break;
            }
        }
    }
    let ia = q.iter().position(|v| same(*v, a))?;
    let ib = q.iter().position(|v| same(*v, b))?;
    let n = q.len();
    let walk = |from: usize, to: usize| {
        let mut out = Vec::new();
        let mut i = from;
        loop {
            out.push(q[i]);
            if i == to {
                break;
            }
            i = (i + 1) % n;
        }
        clean(&out)
    };
    Some([walk(ia, ib), walk(ib, ia)])
}

// -- the grid --------------------------------------------------------------

/// The six directions, 30° apart.
fn dirs() -> [Vec2; 6] {
    std::array::from_fn(|k| {
        let a = (k as f64) * 30f64.to_radians();
        Vec2::new(a.cos(), a.sin())
    })
}

/// The twelve rays, 30° apart.
fn rays() -> [Vec2; 12] {
    std::array::from_fn(|k| {
        let a = (k as f64) * 30f64.to_radians();
        Vec2::new(a.cos(), a.sin())
    })
}

/// A grid line: a point on it and its direction.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct Line {
    pub p: Vec2,
    pub d: Vec2,
    pub deg: u32,
}

/// Every line at a 30° multiple through a vertex, deduplicated.
pub fn lines() -> Vec<Line> {
    let mut out: Vec<Line> = Vec::new();
    for v in tile() {
        for (k, d) in dirs().into_iter().enumerate() {
            let off = cross(v, d);
            if !out
                .iter()
                .any(|l| l.deg == k as u32 * 30 && (cross(l.p, l.d) - off).abs() < 1e-6)
            {
                out.push(Line {
                    p: v,
                    d,
                    deg: k as u32 * 30,
                });
            }
        }
    }
    out
}

/// The grid points: the vertices and every crossing of two lines in or
/// on the tile.
pub fn nodes() -> Vec<Vec2> {
    let t = tile();
    let mut out = t.clone();
    let ls = lines();
    for i in 0..ls.len() {
        for j in i + 1..ls.len() {
            let (a, b) = (ls[i], ls[j]);
            let den = cross(a.d, b.d);
            if den.abs() < 1e-9 {
                continue;
            }
            let s = cross(b.p - a.p, b.d) / den;
            let q = a.p + a.d * s;
            if (point_in(q, &t) || on_edge(q, &t)) && !out.iter().any(|n| same(*n, q)) {
                out.push(q);
            }
        }
    }
    out
}

/// Whether a cut from `a` to `b` is allowed: along a 30° line, through
/// the tile, not along its boundary.
pub fn cut_ok(a: Vec2, b: Vec2) -> bool {
    if same(a, b) {
        return false;
    }
    let t = tile();
    let d = b - a;
    let deg = d.y.atan2(d.x).to_degrees();
    let r = (deg % 30.0).abs();
    if r.min(30.0 - r) > 0.5 {
        return false;
    }
    for k in 1..24 {
        let p = a + d * (k as f64 / 24.0);
        if !(point_in(p, &t) || on_edge(p, &t)) {
            return false;
        }
    }
    !on_edge((a + b) * 0.5, &t)
}

// -- the pieces the cuts make -----------------------------------------------------

/// The faces of the outline plus the cuts: a small planar subdivision,
/// the bounded faces counter-clockwise.
pub fn pieces(cuts: &[Cut]) -> Vec<Vec<Vec2>> {
    let t = tile();
    let mut segs: Vec<[Vec2; 2]> = (0..14).map(|i| [t[i], t[(i + 1) % 14]]).collect();
    segs.extend(cuts.iter().copied());
    let mut pts: Vec<Vec2> = Vec::new();
    let mut pid = |p: Vec2| -> usize {
        if let Some(i) = pts.iter().position(|q| same(*q, p)) {
            i
        } else {
            pts.push(p);
            pts.len() - 1
        }
    };
    let mut splits: Vec<Vec<f64>> = vec![Vec::new(); segs.len()];
    for i in 0..segs.len() {
        for j in i + 1..segs.len() {
            let ([a, b], [c, d]) = (segs[i], segs[j]);
            let (r, s) = (b - a, d - c);
            let den = cross(r, s);
            if den.abs() < 1e-12 {
                continue;
            }
            let t_ = cross(c - a, s) / den;
            let u = cross(c - a, r) / den;
            if t_ > -1e-9 && t_ < 1.0 + 1e-9 && u > -1e-9 && u < 1.0 + 1e-9 {
                splits[i].push(t_);
                splits[j].push(u);
            }
        }
    }
    for i in 0..segs.len() {
        for j in 0..segs.len() {
            if i == j {
                continue;
            }
            for p in segs[j] {
                let [a, b] = segs[i];
                let d = b - a;
                let t_ = (p - a).dot(d) / d.dot(d);
                if t_ > 1e-9 && t_ < 1.0 - 1e-9 && cross(d, p - a).abs() / d.length() < 1e-6 {
                    splits[i].push(t_);
                }
            }
        }
    }
    let mut edges: Vec<[usize; 2]> = Vec::new();
    for (k, [a, b]) in segs.iter().enumerate() {
        let mut ts = vec![0.0, 1.0];
        ts.extend(&splits[k]);
        ts.sort_by(|x, y| x.total_cmp(y));
        let mut prev: Option<usize> = None;
        for t_ in ts {
            let id = pid(*a + (*b - *a) * t_);
            if let Some(p) = prev {
                if p != id {
                    edges.push([p, id]);
                }
            }
            prev = Some(id);
        }
    }
    // Half-edges sorted by angle round each point; walk faces keeping
    // the face on the left.
    #[derive(Clone, Copy)]
    struct Half {
        to: usize,
        e: usize,
        fwd: bool,
    }
    let mut out: Vec<Vec<Half>> = vec![Vec::new(); pts.len()];
    for (k, [i, j]) in edges.iter().enumerate() {
        out[*i].push(Half {
            to: *j,
            e: k,
            fwd: true,
        });
        out[*j].push(Half {
            to: *i,
            e: k,
            fwd: false,
        });
    }
    for (i, list) in out.iter_mut().enumerate() {
        list.sort_by(|a, b| {
            let (pa, pb) = (pts[a.to] - pts[i], pts[b.to] - pts[i]);
            pa.y.atan2(pa.x).total_cmp(&pb.y.atan2(pb.x))
        });
    }
    let mut seen: HashSet<(usize, bool)> = HashSet::new();
    let mut result = Vec::new();
    for i in 0..pts.len() {
        for h in out[i].clone() {
            if seen.contains(&(h.e, h.fwd)) {
                continue;
            }
            let mut poly = Vec::new();
            let (mut cur, mut he) = (i, h);
            for _ in 0..10_000 {
                seen.insert((he.e, he.fwd));
                poly.push(pts[cur]);
                let nxt = he.to;
                let back = out[nxt]
                    .iter()
                    .position(|x| x.e == he.e && x.fwd != he.fwd)
                    .unwrap();
                let k = (back + out[nxt].len() - 1) % out[nxt].len();
                cur = nxt;
                he = out[nxt][k];
                if cur == i && he.e == h.e && he.fwd == h.fwd {
                    break;
                }
            }
            if area(&poly) > 1e-9 {
                result.push(clean(&poly));
            }
        }
    }
    result
}

// -- what the saw thinks -----------------------------------------------------------

/// How a face is cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// The face's line leaves the piece at both ends: one pass.
    Through,
    /// The face ends at an inside corner: a depth-limited pass from the
    /// open end.
    Rabbet,
    /// The face lies between two inside corners: a dado from the open
    /// side.
    Dado,
    /// Nothing to rest on, or boxed in.
    No,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Face {
    pub deg: u32,
    pub kind: Kind,
    /// Blade tilt from vertical, degrees, with the piece on its best
    /// rest face.
    pub tilt: f64,
}

/// A piece as the saw sees it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Saw {
    pub convex: bool,
    pub faces: Vec<Face>,
    /// Distinct face directions, degrees from the tile's first edge.
    pub face_dirs: Vec<u32>,
    pub worst: Kind,
    /// Why the saw cannot make it, when it cannot.
    pub why: String,
    pub max_tilt: f64,
    pub rabbets: usize,
    pub dados: usize,
    /// Thinnest width: the least extent normal to one of its faces.
    pub width: f64,
    pub area: f64,
    /// Interior angles, degrees, flat ones dropped.
    pub angles: Vec<f64>,
}

pub fn saw(p0: &[Vec2]) -> Saw {
    let mut p = clean(p0);
    if area(&p) < 0.0 {
        p.reverse();
    }
    let n = p.len();
    let mut reflex = vec![false; n];
    let mut angles = Vec::new();
    for i in 0..n {
        let (a, b, c) = (p[(i + n - 1) % n], p[i], p[(i + 1) % n]);
        let (u, v) = (b - a, c - b);
        let turn = cross(u, v).atan2(u.dot(v)).to_degrees();
        angles.push(180.0 - turn);
        reflex[i] = turn < -1e-6;
    }
    struct F {
        a: Vec2,
        b: Vec2,
        u: Vec2,
        nin: Vec2,
        deg: u32,
        clear_s: bool,
        clear_e: bool,
    }
    let fs: Vec<F> = (0..n)
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % n]);
            let d = b - a;
            let u = d * (1.0 / d.length());
            let deg = ((d.y.atan2(d.x).to_degrees().rem_euclid(180.0)).round() as u32) % 180;
            F {
                a,
                b,
                u,
                nin: Vec2::new(-u.y, u.x),
                deg,
                clear_s: !reflex[i] && ray_hit(&p, a, u * -1.0).is_none(),
                clear_e: !reflex[(i + 1) % n] && ray_hit(&p, b, u).is_none(),
            }
        })
        .collect();
    let angle_to = |u: Vec2, v: Vec2| u.dot(v).clamp(-1.0, 1.0).acos().to_degrees();
    // The piece can only rest on a face its whole profile lies to one side of.
    let rests: Vec<&F> = fs.iter().filter(|f| f.clear_s && f.clear_e).collect();
    let (mut worst, mut max_tilt, mut rabbets, mut dados) = (Kind::Through, 0.0f64, 0, 0);
    let mut why = String::new();
    let mut faces = Vec::new();
    for f in &fs {
        let mut tilt = f64::INFINITY;
        let mut kind = if f.clear_s && f.clear_e {
            for g in &rests {
                if !std::ptr::eq(*g, f) {
                    let x = angle_to(f.u, g.u);
                    let ang = x.min(180.0 - x);
                    tilt = tilt.min((90.0 - ang).abs());
                }
            }
            Kind::Through
        } else if f.clear_s || f.clear_e {
            rabbets += 1;
            let up = if f.clear_s { f.u * -1.0 } else { f.u };
            for g in &rests {
                if !std::ptr::eq(*g, f) {
                    tilt = tilt.min(angle_to(up, g.nin));
                }
            }
            Kind::Rabbet
        } else {
            dados += 1;
            let up = f.nin * -1.0;
            let mid = (f.a + f.b) * 0.5;
            if ray_hit(&p, mid, up).is_some() {
                if why.is_empty() {
                    why = format!("the {}° face is boxed in", f.deg);
                }
                Kind::No
            } else {
                for g in &rests {
                    if !std::ptr::eq(*g, f) {
                        tilt = tilt.min(angle_to(up, g.nin));
                    }
                }
                Kind::Dado
            }
        };
        if kind != Kind::No && tilt > 45.0 + 1e-6 {
            kind = Kind::No;
            if why.is_empty() {
                why = format!("nothing to rest on for the {}° face", f.deg);
            }
        }
        if kind != Kind::No {
            max_tilt = max_tilt.max(tilt);
        }
        worst = worst.max(kind);
        faces.push(Face {
            deg: f.deg,
            kind,
            tilt: if tilt.is_finite() { tilt } else { -1.0 },
        });
    }
    let mut width = f64::INFINITY;
    for f in &fs {
        let far = p
            .iter()
            .map(|q| cross(f.u, *q - f.a).abs())
            .fold(0.0, f64::max);
        width = width.min(far);
    }
    let mut face_dirs: Vec<u32> = Vec::new();
    for f in &fs {
        if !face_dirs.contains(&f.deg) {
            face_dirs.push(f.deg);
        }
    }
    Saw {
        convex: !reflex.iter().any(|&r| r),
        faces,
        face_dirs,
        worst,
        why,
        max_tilt,
        rabbets,
        dados,
        width,
        area: area(&p),
        angles: angles
            .into_iter()
            .filter(|a| (a - 180.0).abs() > 0.5)
            .collect(),
    }
}

/// A profile's identity up to rotation and mirroring: its cycle of
/// side lengths and corner angles, read from the lowest starting point.
pub fn shape_key(p0: &[Vec2]) -> String {
    let p = clean(p0);
    let n = p.len();
    if n < 3 {
        return "degenerate".into();
    }
    let mut seqs: Vec<String> = Vec::new();
    let rev: Vec<Vec2> = p.iter().rev().copied().collect();
    for q in [&p, &rev] {
        let e: Vec<String> = (0..n)
            .map(|i| {
                let (a, b, c) = (q[i], q[(i + 1) % n], q[(i + 2) % n]);
                let (u, v) = (b - a, c - b);
                format!(
                    "{:.3}@{}",
                    u.length(),
                    cross(u, v).atan2(u.dot(v)).to_degrees().round().abs()
                )
            })
            .collect();
        for s in 0..n {
            let mut rot = e[s..].to_vec();
            rot.extend_from_slice(&e[..s]);
            seqs.push(rot.join(" "));
        }
    }
    seqs.sort();
    seqs.remove(0)
}

// -- the search -------------------------------------------------------------------

/// One decomposition the search found.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Found {
    pub cuts: Vec<Cut>,
    pub pieces: usize,
    /// The worst face kind over the pieces, 0 through to 2 dado.
    pub worst: u8,
    pub rabbets: usize,
    pub dados: usize,
    /// The thinnest piece's width, edges.
    pub minw: f64,
    pub tilt: f64,
    /// The pieces' shares of the tile, percent, largest first.
    pub shares: Vec<u32>,
    /// Distinct profiles.
    pub kinds: usize,
}

fn rank(k: Kind) -> u8 {
    match k {
        Kind::Through => 0,
        Kind::Rabbet => 1,
        Kind::Dado => 2,
        Kind::No => 3,
    }
}

fn sig(cuts: &[Cut]) -> String {
    let mut v: Vec<String> = cuts
        .iter()
        .map(|c| {
            let mut ends: Vec<String> =
                c.iter().map(|p| format!("{:.4},{:.4}", p.x, p.y)).collect();
            ends.sort();
            ends.join(";")
        })
        .collect();
    v.sort();
    v.join("|")
}

/// Every way of cutting the tile with up to `max_cuts` cuts, each along
/// a 30° line from an inside corner until it meets the outline or an
/// earlier cut, then one more cut from any corner of a through-rip
/// decomposition; the sawable ones.
pub fn search(max_cuts: usize) -> Vec<Found> {
    let t = tile();
    let tile_area = area(&t);
    let mut memo: HashMap<String, Saw> = HashMap::new();
    let key_of = |p: &[Vec2]| -> String {
        p.iter()
            .map(|q| format!("{:.5},{:.5}", q.x, q.y))
            .collect::<Vec<_>>()
            .join(";")
    };
    let mut saw_m = |p: &[Vec2]| -> Saw {
        let k = key_of(p);
        memo.entry(k).or_insert_with(|| saw(p)).clone()
    };
    let mut seen: HashSet<String> = HashSet::new();
    let mut results: Vec<(Found, Vec<Vec<Vec2>>)> = Vec::new();
    let record =
        |pieces: &[Vec<Vec2>], cuts: &[Cut], saw_m: &mut dyn FnMut(&[Vec2]) -> Saw| -> Found {
            let an: Vec<Saw> = pieces.iter().map(|p| saw_m(p)).collect();
            let mut shares: Vec<u32> = an
                .iter()
                .map(|a| (100.0 * a.area / tile_area).round() as u32)
                .collect();
            shares.sort_by(|a, b| b.cmp(a));
            let kinds = pieces
                .iter()
                .map(|p| shape_key(p))
                .collect::<BTreeSet<_>>()
                .len();
            Found {
                cuts: cuts.to_vec(),
                pieces: pieces.len(),
                worst: an.iter().map(|a| rank(a.worst)).max().unwrap_or(0),
                rabbets: an.iter().map(|a| a.rabbets).sum(),
                dados: an.iter().map(|a| a.dados).sum(),
                minw: an.iter().map(|a| a.width).fold(f64::INFINITY, f64::min),
                tilt: an.iter().map(|a| a.max_tilt).fold(0.0, f64::max),
                shares,
                kinds,
            }
        };
    let rays = rays();
    // Stage one: cuts from the inside corners, depth first.
    let mut stack: Vec<(Vec<Vec<Vec2>>, Vec<Cut>)> = vec![(vec![t.clone()], Vec::new())];
    while let Some((pieces, cuts)) = stack.pop() {
        let s = sig(&cuts);
        if !seen.insert(s) {
            continue;
        }
        let f = record(&pieces, &cuts, &mut saw_m);
        results.push((f, pieces.clone()));
        if cuts.len() >= max_cuts {
            continue;
        }
        for (pi, p) in pieces.iter().enumerate() {
            let n = p.len();
            for i in 0..n {
                let (a, b, c) = (p[(i + n - 1) % n], p[i], p[(i + 1) % n]);
                if cross(b - a, c - b) > -1e-9 {
                    continue;
                }
                for d in rays {
                    if !point_in(b + d * 1e-4, p) {
                        continue;
                    }
                    let Some(h) = ray_hit(p, b, d) else { continue };
                    let Some(sp) = split_at(p, b, h) else {
                        continue;
                    };
                    if sp.iter().any(|q| q.len() < 3) {
                        continue;
                    }
                    let mut next: Vec<Vec<Vec2>> = pieces
                        .iter()
                        .enumerate()
                        .filter(|(k, _)| *k != pi)
                        .map(|(_, q)| q.clone())
                        .collect();
                    next.extend(sp);
                    let mut nc = cuts.clone();
                    nc.push([b, h]);
                    stack.push((next, nc));
                }
            }
        }
    }
    // Stage two: one more cut from any corner of a through-rip decomposition.
    let mut more: Vec<Found> = Vec::new();
    for (f, pieces) in &results {
        if f.worst != 0 || f.cuts.is_empty() {
            continue;
        }
        for (pi, p) in pieces.iter().enumerate() {
            for i in 0..p.len() {
                for d in rays {
                    if !point_in(p[i] + d * 1e-4, p) {
                        continue;
                    }
                    let Some(h) = ray_hit(p, p[i], d) else {
                        continue;
                    };
                    let Some(sp) = split_at(p, p[i], h) else {
                        continue;
                    };
                    if sp.iter().any(|q| q.len() < 3) {
                        continue;
                    }
                    let mut cuts = f.cuts.clone();
                    cuts.push([p[i], h]);
                    if !seen.insert(sig(&cuts)) {
                        continue;
                    }
                    let mut next: Vec<Vec<Vec2>> = pieces
                        .iter()
                        .enumerate()
                        .filter(|(k, _)| *k != pi)
                        .map(|(_, q)| q.clone())
                        .collect();
                    next.extend(sp);
                    more.push(record(&next, &cuts, &mut saw_m));
                }
            }
        }
    }
    let mut out: Vec<Found> = results.into_iter().map(|(f, _)| f).collect();
    out.extend(more);
    out.retain(|f| f.worst < 3);
    out
}

/// The search's results ranked three ways: fewest distinct profiles,
/// fewest pieces (through rips only), and with rabbets allowed, the best
/// few per piece count.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Ranked {
    pub kit: Vec<Found>,
    pub through: Vec<Found>,
    pub rabbet: Vec<Found>,
}

fn distinct(rows: Vec<Found>) -> Vec<Found> {
    let mut seen = HashSet::new();
    rows.into_iter()
        .filter(|r| {
            let label = format!(
                "{} {} {} {} {:.2} {} {:?}",
                r.pieces,
                r.rabbets,
                r.dados,
                r.kinds,
                r.minw,
                r.tilt.round(),
                r.shares
            );
            seen.insert(label)
        })
        .collect()
}

/// The ranked decompositions, computed once.
pub fn ranked() -> &'static Ranked {
    static RANKED: OnceLock<Ranked> = OnceLock::new();
    RANKED.get_or_init(|| {
        let all = search(4);
        let by_pieces = |a: &Found, b: &Found| {
            a.pieces
                .cmp(&b.pieces)
                .then(a.rabbets.cmp(&b.rabbets))
                .then(a.dados.cmp(&b.dados))
                .then(a.kinds.cmp(&b.kinds))
                .then(b.minw.total_cmp(&a.minw))
                .then(a.tilt.total_cmp(&b.tilt))
        };
        let by_kinds = |a: &Found, b: &Found| {
            a.kinds
                .cmp(&b.kinds)
                .then(a.pieces.cmp(&b.pieces))
                .then(b.minw.total_cmp(&a.minw))
                .then(a.tilt.total_cmp(&b.tilt))
        };
        let mut through: Vec<Found> = all
            .iter()
            .filter(|r| r.worst == 0 && !r.cuts.is_empty())
            .cloned()
            .collect();
        through.sort_by(by_pieces);
        let mut kit = through.clone();
        kit.sort_by(by_kinds);
        let mut rabbet = Vec::new();
        for k in 1..=5 {
            let mut rows: Vec<Found> = all
                .iter()
                .filter(|r| r.pieces == k && r.worst <= 1)
                .cloned()
                .collect();
            rows.sort_by(by_pieces);
            rabbet.extend(distinct(rows).into_iter().take(3));
        }
        Ranked {
            kit: distinct(kit).into_iter().take(10).collect(),
            through: distinct(through).into_iter().take(10).collect(),
            rabbet,
        }
    })
}

/// The kit of a set of pieces: distinct profiles, largest first, each
/// with its count and the index of every piece that is one.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Profile {
    pub outline: Vec<Vec2>,
    pub count: usize,
    pub pieces: Vec<usize>,
}

pub fn kit(pieces: &[Vec<Vec2>]) -> Vec<Profile> {
    let mut groups: Vec<(String, Profile)> = Vec::new();
    for (i, p) in pieces.iter().enumerate() {
        let k = shape_key(p);
        if let Some((_, g)) = groups.iter_mut().find(|(gk, _)| *gk == k) {
            g.count += 1;
            g.pieces.push(i);
        } else {
            groups.push((
                k,
                Profile {
                    outline: clean(p),
                    count: 1,
                    pieces: vec![i],
                },
            ));
        }
    }
    groups.sort_by(|a, b| area(&b.1.outline).total_cmp(&area(&a.1.outline)));
    groups.into_iter().map(|(_, g)| g).collect()
}

/// Which profile (by the kit's order) each piece is.
pub fn profile_of(pieces: &[Vec<Vec2>]) -> Vec<u8> {
    let k = kit(pieces);
    let mut out = vec![0u8; pieces.len()];
    for (j, g) in k.iter().enumerate() {
        for &i in &g.pieces {
            out[i] = j as u8;
        }
    }
    out
}

/// The faces a cut set makes, clipped to a polygon: the polygon (a tile
/// piece, in the board's frame) intersected with each face mapped by
/// `map`, the empty ones dropped. The faces are convex, so a
/// Sutherland–Hodgman clip of the piece by each does.
pub fn split_by(
    poly: &[Vec2],
    faces: &[Vec<Vec2>],
    map: impl Fn(Vec2) -> Vec2,
) -> Vec<(usize, Vec<Vec2>)> {
    let mut out = Vec::new();
    for (j, f) in faces.iter().enumerate() {
        let mut clipper: Vec<Vec2> = f.iter().map(|q| map(*q)).collect();
        if area(&clipper) < 0.0 {
            clipper.reverse();
        }
        let mut subject: Vec<Vec2> = poly.to_vec();
        let n = clipper.len();
        for i in 0..n {
            if subject.is_empty() {
                break;
            }
            let (a, b) = (clipper[i], clipper[(i + 1) % n]);
            let inside = |p: Vec2| cross(b - a, p - a) >= -1e-9;
            let mut next = Vec::new();
            let m = subject.len();
            for k in 0..m {
                let (cur, prev) = (subject[k], subject[(k + m - 1) % m]);
                let (ci, pi) = (inside(cur), inside(prev));
                if ci {
                    if !pi {
                        if let Some(x) = segment_line(prev, cur, a, b) {
                            next.push(x);
                        }
                    }
                    next.push(cur);
                } else if pi {
                    if let Some(x) = segment_line(prev, cur, a, b) {
                        next.push(x);
                    }
                }
            }
            subject = next;
        }
        let subject = clean(&subject);
        if subject.len() >= 3 && area(&subject) > 1e-6 {
            out.push((j, subject));
        }
    }
    out
}

fn segment_line(p: Vec2, q: Vec2, a: Vec2, b: Vec2) -> Option<Vec2> {
    let d = q - p;
    let e = b - a;
    let den = cross(d, e);
    if den.abs() < 1e-12 {
        return None;
    }
    let t = cross(a - p, e) / den;
    Some(p + d * t)
}

/// The tile's interior angles, for the panel.
pub fn angles() -> [u32; 14] {
    ANGLES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grid_has_the_lines_and_points_the_tile_needs() {
        let ls = lines();
        assert!(ls.len() > 30 && ls.len() <= 84, "{}", ls.len());
        let ns = nodes();
        assert!(ns.len() > 100, "{}", ns.len());
        let t = tile();
        assert!(ns.iter().all(|n| point_in(*n, &t) || on_edge(*n, &t)));
        // A cut along the first edge's line from the first vertex runs
        // along the boundary: refused; one across the tile is fine.
        assert!(!cut_ok(t[0], t[1]));
        assert!(cut_ok(t[13], Vec2::new(0.0, 1.0 + 1.366)));
        assert!(!cut_ok(t[8], t[4]), "not on a 30° line");
    }

    #[test]
    fn the_saw_reads_through_rips_rabbets_and_dados() {
        let square = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 1.0),
        ];
        let s = saw(&square);
        assert!(s.convex && s.worst == Kind::Through && s.max_tilt < 1e-9);
        assert!((s.width - 1.0).abs() < 1e-9);
        // An L: two rabbets, the rest through.
        let l = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(2.0, 0.0),
            Vec2::new(2.0, 1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(1.0, 2.0),
            Vec2::new(0.0, 2.0),
        ];
        let s = saw(&l);
        assert_eq!(s.worst, Kind::Rabbet);
        assert_eq!(s.rabbets, 2);
        // A channel: the floor between two inside corners is a dado.
        let u = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(3.0, 0.0),
            Vec2::new(3.0, 2.0),
            Vec2::new(2.0, 2.0),
            Vec2::new(2.0, 1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(1.0, 2.0),
            Vec2::new(0.0, 2.0),
        ];
        let s = saw(&u);
        assert_eq!(s.worst, Kind::Dado);
        assert_eq!(s.dados, 1);
        // The straight tile alone has inside corners nothing rests for.
        let s = saw(&tile());
        assert!(!s.convex);
        assert_eq!(s.worst, Kind::No, "{}", s.why);
    }

    #[test]
    fn mirror_images_are_one_profile() {
        let a = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(2.0, 0.0),
            Vec2::new(2.0, 1.0),
            Vec2::new(0.0, 1.5),
        ];
        let b: Vec<Vec2> = a.iter().map(|p| Vec2::new(-p.x, p.y)).collect();
        assert_eq!(shape_key(&a), shape_key(&b));
        let c = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(2.0, 0.0),
            Vec2::new(2.0, 1.0),
            Vec2::new(0.0, 1.2),
        ];
        assert_ne!(shape_key(&a), shape_key(&c));
    }

    #[test]
    fn the_search_finds_the_two_profile_kit() {
        let r = ranked();
        assert!(!r.through.is_empty() && !r.kit.is_empty());
        // Through rips only need every inside corner cut: four cuts,
        // five pieces, no fewer.
        assert!(r.through.iter().all(|f| f.pieces >= 5));
        assert_eq!(r.through[0].pieces, 5);
        // One more cut gets the kit down to two profiles: two of one
        // and four of another.
        let best = &r.kit[0];
        assert_eq!(best.kinds, 2, "{best:?}");
        assert_eq!(best.pieces, 6);
        let ps = pieces(&best.cuts);
        assert_eq!(ps.len(), 6);
        let k = kit(&ps);
        assert_eq!(k.len(), 2);
        assert_eq!((k[0].count, k[1].count), (2, 4));
        let total: f64 = ps.iter().map(|p| area(p)).sum();
        assert!((total - area(&tile())).abs() < 1e-6);
        assert!(ps.iter().all(|p| saw(p).worst == Kind::Through));
        // Rabbets allowed: two pieces from one cut.
        assert_eq!(r.rabbet[0].pieces, 2);
        assert_eq!(r.rabbet[0].cuts.len(), 1);
    }

    #[test]
    fn a_piece_splits_by_the_faces_that_cover_it() {
        let best = &ranked().kit[0];
        let faces = pieces(&best.cuts);
        // The tile itself, mapped by a turn and a scale, splits into
        // every face with the areas scaled.
        let map = |p: Vec2| {
            let (s, c) = 40f64.to_radians().sin_cos();
            Vec2::new(c * p.x - s * p.y, s * p.x + c * p.y) * 12.0 + Vec2::new(30.0, 20.0)
        };
        let placed: Vec<Vec2> = tile().iter().map(|p| map(*p)).collect();
        let parts = split_by(&placed, &faces, map);
        assert_eq!(parts.len(), 6);
        let total: f64 = parts.iter().map(|(_, p)| area(p)).sum();
        assert!((total - area(&placed)).abs() < 1e-6 * total);
        // The tile's left end keeps only the faces reaching it.
        let half: Vec<Vec2> = vec![
            Vec2::new(-100.0, -100.0),
            Vec2::new(20.0, -100.0),
            Vec2::new(20.0, 100.0),
            Vec2::new(-100.0, 100.0),
        ];
        let clipped = split_by(&half, &faces, map);
        assert!(clipped.len() < 6 && !clipped.is_empty());
    }
}
