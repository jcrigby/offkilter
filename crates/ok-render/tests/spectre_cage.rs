//! The Spectre stack cage (examples/spectre-cage) as a regression
//! suite: each master is the tile the top's pieces are cut from, the
//! curved one with its knobs rounded for a ball-nose bit, the straight
//! one exact for a square-end bit, centred in one hex blank that holds
//! both; every edge of each profile is reachable by its bit at one of
//! the twelve indices the two cradles give (a ball on the edge with its
//! shank's column clear of the profile and the panels still in place; a
//! square-end bit parallel to the edge or flat on it, its body's column
//! clear); every finned panel clears its master and the other fins and
//! fills its sector to within the clearance; and the loaded cage reads
//! back as the stack it holds, the rods clear of everything, the flanges
//! the same hex as the masters' ends, resting on the cradles. Every
//! number prints (`--nocapture`).

use ok_brep::{BoolOp, Solid};
use ok_math::{Vec2, Vec3};
use ok_model::{Document, FeatureKind, RegenResult, SpectreFeature, TabId, TabKind};
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/spectre-cage")
}

fn load() -> Document {
    let json = std::fs::read_to_string(dir().join("out/spectre_cage.okpart")).unwrap();
    Document::from_json(&json).unwrap()
}

/// The numbers the build script settled: the hex, the clocking, the
/// stack, the clearance.
fn numbers() -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(dir().join("out/cage.json")).unwrap()).unwrap()
}

fn num(v: &serde_json::Value, k: &str) -> f64 {
    v[k].as_f64().unwrap_or_else(|| panic!("no {k}"))
}

fn tab_named(doc: &Document, name: &str) -> TabId {
    doc.tabs
        .iter()
        .find(|t| t.name() == name)
        .unwrap_or_else(|| panic!("no tab {name}"))
        .id
}

fn spectre(doc: &Document, tab: TabId) -> SpectreFeature {
    match &doc.tabs.iter().find(|t| t.id == tab).unwrap().kind {
        TabKind::PartStudio(ps) => match &ps.features()[0].kind {
            FeatureKind::Spectre(s) => s.clone(),
            other => panic!("{other:?}"),
        },
        _ => unreachable!(),
    }
}

fn regen(doc: &mut Document, name: &str) -> RegenResult {
    let tab = tab_named(doc, name);
    let r = doc.regenerate_studio(tab, None).unwrap();
    let errors: Vec<_> = r.errors().collect();
    assert!(errors.is_empty(), "{name}: {errors:?}");
    for b in &r.bodies {
        b.solid
            .validate()
            .unwrap_or_else(|e| panic!("{name} / {}: {e}", b.name));
    }
    r
}

fn body<'a>(r: &'a RegenResult, name: &str) -> &'a Solid {
    &r.bodies
        .iter()
        .find(|b| b.name == name)
        .unwrap_or_else(|| panic!("no body {name}"))
        .solid
}

fn bounds(s: &Solid) -> (Vec3, Vec3) {
    s.bounds().unwrap()
}

fn overlap(a: &Solid, b: &Solid) -> f64 {
    let (la, ha) = bounds(a);
    let (lb, hb) = bounds(b);
    if la.x > hb.x || lb.x > ha.x || la.y > hb.y || lb.y > ha.y || la.z > hb.z || lb.z > ha.z {
        return 0.0;
    }
    ok_brep::boolean(a, b, BoolOp::Intersection)
        .map(|s| s.volume().abs())
        .unwrap_or(0.0)
}

fn rot(q: Vec2, deg: f64) -> Vec2 {
    let (s, c) = deg.to_radians().sin_cos();
    Vec2::new(q.x * c - q.y * s, q.x * s + q.y * c)
}

/// The blank hex and its panels, as the build script lays them out: a
/// flat on top, face k's outward normal at 60k + 30 degrees.
struct Hex {
    f: f64,
    side: f64,
    apothem: f64,
    panel_t: f64,
    mitre_clear: f64,
}

impl Hex {
    fn frame(&self, k: usize) -> (Vec2, Vec2) {
        let n = rot(Vec2::X, 60.0 * k as f64 + 30.0);
        (Vec2::new(n.y, -n.x), n)
    }

    /// Panel k's section: a plate on the face, its edges mitred at 60
    /// degrees to meet its neighbours at the corners.
    fn panel(&self, k: usize) -> Vec<Vec2> {
        let (t, n) = self.frame(k);
        let half = self.side / 2.0 - self.mitre_clear;
        let lean = self.panel_t * 30f64.to_radians().tan();
        [
            (-half, 0.0),
            (half, 0.0),
            (half + lean, self.panel_t),
            (-half - lean, self.panel_t),
        ]
        .into_iter()
        .map(|(x, d)| t * x + n * (self.apothem + d))
        .collect()
    }

    /// The twelve indices: (direction the bit comes down, open panel).
    fn indices(&self) -> Vec<(Vec2, usize, String)> {
        let mut out = Vec::new();
        for k in 0..6 {
            out.push((self.frame(k).1, k, format!("flat {}", k + 1)));
        }
        for j in 0..6 {
            let d = rot(Vec2::X, 60.0 * j as f64);
            out.push((d, j, format!("V {} by panel {}", j + 1, j + 1)));
            out.push((
                d,
                (j + 5) % 6,
                format!("V {} by panel {}", j + 1, (j + 5) % 6 + 1),
            ));
        }
        out
    }
}

fn dist_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let d = b - a;
    let len2 = d.length_squared();
    let t = if len2 > 0.0 {
        ((p - a).dot(d) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    p.distance(a + d * t)
}

fn dist_to_polygon(p: Vec2, poly: &[Vec2]) -> f64 {
    let n = poly.len();
    (0..n)
        .map(|i| dist_to_segment(p, poly[i], poly[(i + 1) % n]))
        .fold(f64::INFINITY, f64::min)
}

fn ray_hit(poly: &[Vec2], o: Vec2, d: Vec2) -> Option<f64> {
    let n = poly.len();
    let mut best: Option<f64> = None;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let e = b - a;
        let denom = d.cross(e);
        if denom.abs() < 1e-12 {
            continue;
        }
        let w = a - o;
        let t = w.cross(e) / denom;
        let u = w.cross(d) / denom;
        if t > 1e-9 && (-1e-9..=1.0 + 1e-9).contains(&u) && best.is_none_or(|b| t < b) {
            best = Some(t);
        }
    }
    best
}

/// The profile in its hex with the panels round it and the bit: what
/// the reach check sweeps against.
struct Scene<'a> {
    outline: &'a [Vec2],
    hx: &'a Hex,
    panels: &'a [Vec<Vec2>],
    r: f64,
    flat: bool,
    /// An edge shorter than this a square-end bit leaves as a corner.
    short: f64,
}

impl Scene<'_> {
    /// Whether a bit standing at `c` can be drawn up direction `d` out of
    /// the window with panel `open` off, from `from` above `c` on: at
    /// every step its circle stays off the profile and off every panel
    /// but the open one.
    fn column_clear(&self, c: Vec2, d: Vec2, open: usize, from: f64) -> bool {
        let r = self.r;
        let steps = ((self.hx.apothem + self.hx.panel_t + r) / 0.25).ceil() as usize;
        for s in 0..=steps {
            let q = c + d * (from + s as f64 * 0.25);
            if dist_to_polygon(q, self.outline) < r - 0.06 {
                return false;
            }
            for (k, p) in self.panels.iter().enumerate() {
                if k != open
                    && (dist_to_polygon(q, p) < r - 0.06 || ok_sketch::point_in_polygon(q, p))
                {
                    return false;
                }
            }
        }
        true
    }

    /// Whether the bit coming down direction `d` with panel `open` off
    /// can cut the edge of length `len` along `e` at `m` with outward
    /// normal `ne`. A ball-nose bit: the edge does not face away, and
    /// with the ball on it the shank's column is clear. A square-end
    /// bit: the edge is parallel to the bit and the column beside it is
    /// clear, or the edge faces straight up and the column over it is
    /// clear with the bit somewhere over the edge; an edge shorter than
    /// `short` it leaves as the corner.
    fn reachable(&self, m: Vec2, e: Vec2, ne: Vec2, len: f64, d: Vec2, open: usize) -> bool {
        let r = self.r;
        if !self.flat {
            return ne.dot(d) >= -0.02 && self.column_clear(m + ne * r, d, open, 0.05);
        }
        if len < self.short {
            return true;
        }
        if e.dot(d).abs() > 0.9999 {
            return self.column_clear(m + ne * r, d, open, 0.05);
        }
        if ne.dot(d) > 0.9999 {
            let t = Vec2::new(d.y, -d.x);
            return [-r + 0.1, -r / 2.0, 0.0, r / 2.0, r - 0.1]
                .iter()
                .any(|off| self.column_clear(m + t * *off, d, open, r));
        }
        false
    }
}

/// One master's settings from the build's numbers.
struct MasterCfg {
    name: String,
    params: ok_sketch::spectre::Params,
    round: f64,
    flat: bool,
    phase: f64,
}

fn masters(doc: &Document, n: &serde_json::Value) -> Vec<MasterCfg> {
    let sf = spectre(doc, tab_named(doc, "Top"));
    let mut out = Vec::new();
    for (name, cfg) in n["masters"].as_object().unwrap() {
        let curve = match cfg["curve"].as_str().unwrap() {
            "arc" => ok_model::SpectreCurve::Arc,
            _ => ok_model::SpectreCurve::Cubic,
        };
        out.push(MasterCfg {
            name: name.clone(),
            params: ok_sketch::spectre::Params {
                curve,
                bulge: num(cfg, "bulge"),
                bit: num(cfg, "bit"),
                ..sf.params()
            },
            round: num(cfg, "round"),
            flat: cfg["shape"].as_str().unwrap() == "flat",
            phase: num(cfg, "phase"),
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// A master's profile in the hex, clocked as the build chose.
fn profile(m: &MasterCfg) -> (Vec<Vec2>, ok_sketch::spectre::Master) {
    let master = ok_sketch::spectre::master(&m.params, m.round, 10f64.to_radians()).unwrap();
    let outline: Vec<Vec2> = master.outline.iter().map(|q| rot(*q, m.phase)).collect();
    (outline, master)
}

fn hex_of(n: &serde_json::Value) -> Hex {
    let f = num(n, "F");
    Hex {
        f,
        side: f / 3f64.sqrt(),
        apothem: f / 2.0,
        panel_t: num(n, "panel_t"),
        mitre_clear: num(n, "mitre_clear"),
    }
}

#[test]
fn each_master_is_the_tile_the_top_is_cut_from_centred_in_a_hex_that_holds_both() {
    let mut doc = load();
    let n = numbers();
    let sf = spectre(&doc, tab_named(&doc, "Top"));
    assert_eq!((sf.edge, sf.gap), (num(&n, "edge"), num(&n, "gap")));
    assert!((sf.bit - num(&n, "bit")).abs() < 1e-9);
    let hx = hex_of(&n);
    let top = regen(&mut doc, "Top");
    let pieces = top
        .bodies
        .iter()
        .filter(|b| b.name.starts_with("Piece "))
        .count();
    assert_eq!(pieces as f64, num(&n, "pieces"));
    let count = num(&n, "count");
    assert_eq!(count, pieces as f64 + 2.0);
    let length = count * num(&n, "laminate");
    assert_eq!(length, num(&n, "length"));
    let f_out = num(&n, "F_out");
    let end = num(&n, "master_end");
    let hex_area = 3f64.sqrt() / 2.0 * f_out * f_out;
    let all = masters(&doc, &n);
    assert_eq!(all.len(), 2);
    for mc in &all {
        let (outline, m) = profile(mc);
        let count = outline.len();
        let turn = |i: usize| {
            let (a, b, c) = (
                outline[(i + count - 1) % count],
                outline[i],
                outline[(i + 1) % count],
            );
            (b - a).cross(c - b).atan2((b - a).dot(c - b)).to_degrees()
        };
        if mc.round > 0.0 {
            // The curved master: its knobs rounded to the bit, so a
            // little under the same tile with its corners sharp, and no
            // convex corner turning more than the facets.
            let sharp = ok_sketch::spectre::master(&mc.params, 0.0, 10f64.to_radians()).unwrap();
            assert!(
                m.area < sharp.area && m.area > sharp.area - 20.0,
                "{}: {} vs {}",
                mc.name,
                m.area,
                sharp.area
            );
            let sharpest = (0..count).map(turn).fold(0.0, f64::max);
            assert!(
                sharpest < 25.0,
                "{}: a convex turn of {sharpest} degrees",
                mc.name
            );
        } else {
            // The straight master: the tile exactly but for the gap, so
            // every edge longer than the gap's rounding of a corner lies
            // on the 30 degree grid, where a square-end bit can be
            // parallel or square to it at one of the twelve indices.
            assert!(mc.flat && mc.params.bulge == 0.0 && mc.params.bit == 0.0);
            let short = num(&n, "short");
            let mut on_grid = 0.0;
            for i in 0..count {
                let e = outline[(i + 1) % count] - outline[i];
                if e.length() < short {
                    continue;
                }
                let deg = e.angle().to_degrees().rem_euclid(30.0);
                let off = deg.min(30.0 - deg);
                assert!(
                    off < 0.01,
                    "{}: an edge {off} degrees off the grid",
                    mc.name
                );
                on_grid += e.length();
            }
            assert!(on_grid > 14.0 * sf.edge * 0.9, "{} mm on the grid", on_grid);
            assert!(
                (m.area
                    - (ok_sketch::spectre::TILE_AREA * sf.edge * sf.edge
                        - 0.5 * sf.gap * 14.0 * sf.edge))
                    .abs()
                    < 5.0,
                "{}: {} mm2",
                mc.name,
                m.area
            );
        }
        // The blank holds the master's circle with its margin, and every
        // point of the clocked outline is inside the hex by that margin.
        assert!(
            2.0 * m.radius + 2.0 < hx.f,
            "{}: {} across in {}",
            mc.name,
            2.0 * m.radius,
            hx.f
        );
        for q in &outline {
            for k in 0..6 {
                let (_, nk) = hx.frame(k);
                assert!(
                    q.dot(nk) < hx.apothem - 1.0,
                    "{}: {q:?} against face {k}",
                    mc.name
                );
            }
        }
        // The master body is that profile over the stack, with a hex end
        // the flanges' size at each end.
        let master = regen(&mut doc, &format!("Master {}", mc.name));
        let solid = body(&master, "Master");
        let expected = m.area * length + 2.0 * hex_area * end;
        let volume = solid.volume();
        assert!(
            (volume - expected).abs() < 0.01 * expected,
            "{}: {volume} vs {expected}",
            mc.name
        );
        let (lo, hi) = bounds(solid);
        assert!(
            (lo.x + end).abs() < 1e-6 && (hi.x - length - end).abs() < 1e-6,
            "{lo:?} {hi:?}"
        );
        assert!(
            (hi.z - lo.z - f_out).abs() < 1e-6,
            "ends {} across",
            hi.z - lo.z
        );
        eprintln!(
            "{} master {volume:.0} mm3, {} points, {:.2} across, clocked {} deg",
            mc.name,
            outline.len(),
            2.0 * m.radius,
            mc.phase
        );
        if !mc.flat {
            // Seen from above on the flat cradle, the curved profile
            // alone (the fins' tab has it without the ends) has no corner
            // tighter than the ball-nose bit: the fillets are the bit's
            // own, and the arcs twice it. What a pilot from above never
            // sees, the bays' undersides, is for the other indices.
            let fins = regen(&mut doc, &format!("Fins {}", mc.name));
            let prism = body(&fins, "Master");
            let report = ok_brep::duplicate_check(
                prism,
                &ok_brep::Bit {
                    diameter: sf.bit,
                    reach: 200.0,
                    shape: ok_brep::BitShape::Ball,
                },
                Some(0.2),
            )
            .unwrap();
            eprintln!(
                "  from above a ball bit leaves {:.1} mm2 of residual, {:.0} mm3 under overhangs",
                report.residual_area, report.undercut_volume
            );
            // The fillets are polylines at 10 degree facets, chords a few
            // hundredths inside the ball's circle, so the residual is that
            // high and no more.
            assert!(report.residual_max < 0.1, "{:?}", report.problems);
            assert!(report.residual_area < 30.0, "{:?}", report.problems);
            assert!(
                report.undercut_volume > 0.0,
                "the bays must be undercuts from one side"
            );
        }
    }
}

#[test]
fn every_edge_of_each_profile_is_reachable_by_its_bit_at_one_of_the_twelve_indices() {
    let doc = load();
    let n = numbers();
    let hx = hex_of(&n);
    let panels: Vec<Vec<Vec2>> = (0..6).map(|k| hx.panel(k)).collect();
    let idx = hx.indices();
    for mc in masters(&doc, &n) {
        let (outline, _) = profile(&mc);
        let scene = Scene {
            outline: &outline,
            hx: &hx,
            panels: &panels,
            r: num(&n, "bit") / 2.0,
            flat: mc.flat,
            short: num(&n, "short"),
        };
        let count = outline.len();
        let edges: Vec<(Vec2, Vec2, Vec2, f64)> = (0..count)
            .filter_map(|i| {
                let (a, b) = (outline[i], outline[(i + 1) % count]);
                let e = b - a;
                let len = e.length();
                (len > 1e-9).then(|| {
                    (
                        (a + b) * 0.5,
                        e * (1.0 / len),
                        Vec2::new(e.y, -e.x) * (1.0 / len),
                        len,
                    )
                })
            })
            .collect();
        let total: f64 = edges.iter().map(|e| e.3).sum();
        let mut per = vec![0.0; idx.len()];
        let mut covered = 0.0;
        let mut missed = Vec::new();
        for (m, e, ne, len) in &edges {
            let mut ok = false;
            for (w, (d, open, _)) in idx.iter().enumerate() {
                if scene.reachable(*m, *e, *ne, *len, *d, *open) {
                    ok = true;
                    per[w] += len;
                }
            }
            if ok {
                covered += len;
            } else {
                missed.push(*m);
            }
        }
        for ((_, _, label), l) in idx.iter().zip(&per) {
            eprintln!("{} / {label}: {l:.0} mm of {total:.0}", mc.name);
        }
        // Every window cuts something at its flat index, and the whole
        // outline is reached somewhere.
        for (w, (_, _, label)) in idx.iter().enumerate().take(6) {
            assert!(per[w] > 1.0, "{}: {label} cuts only {} mm", mc.name, per[w]);
        }
        assert!(
            missed.is_empty(),
            "{}: {:.1}% reached; unreachable at {:?}",
            mc.name,
            100.0 * covered / total,
            &missed[..missed.len().min(8)]
        );
        // And six flat indices alone would not do: the curved tile's bays
        // open toward the hex's corners, and the straight tile has edges
        // on every other line of the grid.
        let flat_only: f64 = edges
            .iter()
            .filter(|(m, e, ne, len)| {
                idx.iter()
                    .take(6)
                    .any(|(d, open, _)| scene.reachable(*m, *e, *ne, *len, *d, *open))
            })
            .map(|e| e.3)
            .sum();
        eprintln!(
            "{}: flat indices alone reach {:.1}%",
            mc.name,
            100.0 * flat_only / total
        );
        assert!(flat_only < 0.99 * total);
    }
}

#[test]
fn the_finned_panels_clear_their_master_and_each_other_and_fill_their_sectors() {
    let mut doc = load();
    let n = numbers();
    let hx = hex_of(&n);
    let clear = num(&n, "clear");
    let length = num(&n, "length");
    for mc in masters(&doc, &n) {
        let (outline, _) = profile(&mc);
        let r = regen(&mut doc, &format!("Fins {}", mc.name));
        let master = body(&r, "Master");
        let fins: Vec<&Solid> = (1..=6)
            .map(|k| body(&r, &format!("Finned panel {k}")))
            .collect();
        for (k, fin) in fins.iter().enumerate() {
            let hit = overlap(fin, master);
            assert!(
                hit < 1e-6,
                "{}: finned panel {} meets the master: {hit} mm3",
                mc.name,
                k + 1
            );
            for (j, other) in fins.iter().enumerate().skip(k + 1) {
                let hit = overlap(fin, other);
                assert!(
                    hit < 1e-6,
                    "{}: finned panels {} and {} meet: {hit} mm3",
                    mc.name,
                    k + 1,
                    j + 1
                );
            }
            // The fin fills the column over the profile within its
            // sector, the clearance off every way: the envelope sampled
            // finely across the window, the least depth within the
            // clearance taken, as the build did, and compared with the
            // body less its plate.
            let (t, nn) = hx.frame(k);
            let half = hx.side / 2.0 - hx.mitre_clear - 0.2;
            let step = 0.02;
            let count = ((2.0 * half) / step).ceil() as usize;
            let raw: Vec<f64> = (0..=count)
                .map(|i| {
                    let x = -half + i as f64 * step;
                    let o = t * x + nn * hx.apothem;
                    let hit = ray_hit(&outline, o, nn * -1.0).unwrap_or(f64::INFINITY);
                    let sector = hx.apothem * (1.0 - x.abs() / (hx.side / 2.0));
                    hit.min(sector)
                })
                .collect();
            let reach = (clear / step).round() as usize;
            let mut area = 0.0;
            for i in 0..=count {
                let lo = i.saturating_sub(reach);
                let hi = (i + reach).min(count);
                let d = raw[lo..=hi].iter().cloned().fold(f64::INFINITY, f64::min) - clear;
                area += d.max(0.0) * step;
            }
            let (lo, hi) = bounds(fin);
            let plate = ok_sketch::signed_area(&hx.panel(k)).abs() * (hi.x - lo.x);
            let fin_volume = fin.volume() - plate;
            let expected = area * (length - 1.0);
            eprintln!(
                "{} / finned panel {}: fin {fin_volume:.0} mm3, sector column {expected:.0} mm3",
                mc.name,
                k + 1
            );
            // Lobes too thin to print are left off, so the body is a
            // little under the column; never over it.
            assert!(
                fin_volume <= expected * 1.03 + 1.0,
                "{}: fin {} over its column: {fin_volume} vs {expected}",
                mc.name,
                k + 1
            );
            assert!(
                fin_volume >= expected * 0.9,
                "{}: fin {} under its column: {fin_volume} vs {expected}",
                mc.name,
                k + 1
            );
        }
    }
}

#[test]
fn the_loaded_cage_holds_the_stack_with_the_rods_clear_and_sits_in_the_cradles() {
    let mut doc = load();
    let n = numbers();
    let hx = hex_of(&n);
    let f_out = num(&n, "F_out");
    let count = num(&n, "count") as usize;
    let lam = num(&n, "laminate");
    let length = num(&n, "length");
    let r = regen(&mut doc, "Cage");
    let laminates: Vec<&ok_model::Body> = r
        .bodies
        .iter()
        .filter(|b| b.name.starts_with("Laminate "))
        .collect();
    assert_eq!(laminates.len(), count);
    let mut woods = std::collections::BTreeMap::new();
    for (i, b) in laminates.iter().enumerate() {
        let (lo, hi) = bounds(&b.solid);
        assert!(
            (lo.x - i as f64 * lam).abs() < 1e-6 && (hi.x - lo.x - lam).abs() < 1e-6,
            "{}: {lo:?}",
            b.name
        );
        assert!(
            (hi.z - lo.z - (hx.f - num(&n, "clear"))).abs() < 1e-6,
            "{} is {} across",
            b.name,
            hi.z - lo.z
        );
        let wood = b.material.as_ref().map(|m| m.name.clone()).unwrap();
        assert!(b.name.ends_with(&wood.to_lowercase()), "{}", b.name);
        *woods.entry(wood).or_insert(0) += 1;
    }
    for (wood, c) in &woods {
        let wanted = n["counts"][wood].as_u64().unwrap_or(0) as usize;
        assert!(*c == wanted || *c == wanted + 2, "{wood}: {c} vs {wanted}");
    }
    eprintln!("stack of {count}: {woods:?}, {length} mm");
    // The plugs: a boss in each end of the stack, a flange the master's
    // ends' hex; the rods through the flanges clear of the panels and
    // the stack; the nuts outside.
    let plug_a = body(&r, "Plug A");
    let plug_b = body(&r, "Plug B");
    let (la, ha) = bounds(plug_a);
    let (lb, hb) = bounds(plug_b);
    assert!(
        (ha.x - 0.0).abs() < 1e-6 && (lb.x - length).abs() < 1e-6,
        "{ha:?} {lb:?}"
    );
    assert!((ha.z - la.z - f_out).abs() < 1e-6 && (hb.z - lb.z - f_out).abs() < 1e-6);
    let panels: Vec<&Solid> = r
        .bodies
        .iter()
        .filter(|b| b.name.starts_with("Panel "))
        .map(|b| &b.solid)
        .collect();
    assert_eq!(panels.len(), 5, "the top window open");
    let rods: Vec<&Solid> = r
        .bodies
        .iter()
        .filter(|b| b.name.starts_with("Rod "))
        .map(|b| &b.solid)
        .collect();
    assert_eq!(rods.len(), 6);
    for (i, rod) in rods.iter().enumerate() {
        for (j, p) in panels.iter().enumerate() {
            assert!(
                overlap(rod, p) < 1e-6,
                "rod {} meets panel {}",
                i + 1,
                j + 1
            );
        }
        for b in &laminates {
            assert!(
                overlap(rod, &b.solid) < 1e-6,
                "rod {} meets {}",
                i + 1,
                b.name
            );
        }
        assert!(
            overlap(rod, plug_a) < 1e-6 && overlap(rod, plug_b) < 1e-6,
            "rod {} meets a plug",
            i + 1
        );
        let (lo, hi) = bounds(rod);
        let centre = Vec2::new((lo.y + hi.y) / 2.0, (lo.z + hi.z) / 2.0);
        assert!((centre.length() - num(&n, "rod_radius")).abs() < 1e-6);
        // Inside the flange's corners with its nut.
        assert!(
            centre.length() + 3.5 < f_out / 3f64.sqrt(),
            "rod {} past the flange",
            i + 1
        );
    }
    // The panels sit on the stack's hex and the plugs' bosses, clear of
    // both, and the fins' tab has the same panels.
    for (j, p) in panels.iter().enumerate() {
        for b in &laminates {
            assert!(
                overlap(p, &b.solid) < 1e-6,
                "panel {} meets {}",
                j + 1,
                b.name
            );
        }
        assert!(
            overlap(p, plug_a) < 1e-6 && overlap(p, plug_b) < 1e-6,
            "panel {} meets a plug",
            j + 1
        );
    }
    // The cradles hold the flanges: no overlap, the flange's flat on the
    // trough's floor, which is the cradle's floor up.
    for (plug, cradle) in [(plug_a, "Cradle A"), (plug_b, "Cradle B")] {
        let c = body(&r, cradle);
        assert!(overlap(plug, c) < 1e-6, "{cradle} meets its plug");
        let (lo, _) = bounds(plug);
        let (clo, _) = bounds(c);
        assert!(
            (lo.z - clo.z - 10.0).abs() < 1e-6,
            "{cradle}: flange {} over floor {}",
            lo.z,
            clo.z
        );
    }
    // And the cradles on their own tab: the flat one's trough is the
    // flange hex, the V one's a 120 degree V the hex sits in on a corner,
    // both printable flat.
    let cr = regen(&mut doc, "Cradles");
    let flat = body(&cr, "Flat cradle");
    let vee = body(&cr, "V cradle");
    let hex_area = 3f64.sqrt() / 2.0 * f_out * f_out;
    let (flo, fhi) = bounds(flat);
    let flat_block = (fhi.y - flo.y) * (fhi.z - flo.z) * (fhi.x - flo.x);
    assert!(
        (flat_block - flat.volume() - (hex_area / 2.0) * (fhi.x - flo.x)).abs() < 0.02 * flat_block,
        "the flat trough is half the hex"
    );
    let (vlo, vhi) = bounds(vee);
    let v_block = (vhi.y - vlo.y) * (vhi.z - vlo.z) * (vhi.x - vlo.x);
    let r_hex = f_out / 3f64.sqrt();
    let v_area = r_hex * 30f64.to_radians().cos() * (r_hex / 2.0); // the triangle under the hex's lower two faces
    assert!(
        (v_block - vee.volume() - v_area * (vhi.x - vlo.x)).abs() < 0.02 * v_block,
        "the V trough is the hex's lower corner"
    );
    eprintln!(
        "cradles: flat {:.0} mm3, V {:.0} mm3",
        flat.volume(),
        vee.volume()
    );
    // Registration cones: two 90 degree cones in every cradle, one in
    // each rim at mid-length, mirrored about the trough's axis and the
    // same on the two cradles of a kind, so the duplicator's stylus and
    // bit dropped into the master's cradle and the cage's register the
    // stylus to the bit; a 3/16 ball seats below the rim, clear of the
    // flange's corners.
    let bit = num(&n, "bit");
    let cones = |s: &Solid| -> Vec<(Vec3, f64)> {
        s.surfaces
            .iter()
            .filter_map(|sf| match sf {
                ok_brep::Surface::Cone {
                    apex, half_angle, ..
                } => Some((*apex, *half_angle)),
                _ => None,
            })
            .collect()
    };
    let mut flat_cones = None;
    for (name, c) in [
        ("Flat cradle", flat),
        ("V cradle", vee),
        ("Cradle A", body(&r, "Cradle A")),
        ("Cradle B", body(&r, "Cradle B")),
    ] {
        let mut cs = cones(c);
        cs.sort_by(|a, b| a.0.y.partial_cmp(&b.0.y).unwrap());
        assert_eq!(cs.len(), 2, "{name}: {} cones", cs.len());
        let (lo, hi) = bounds(c);
        for (apex, half) in &cs {
            assert!(
                (half - 45f64.to_radians()).abs() < 1e-9,
                "{name}: a 90 degree cone"
            );
            assert!(
                (apex.x - (lo.x + hi.x) / 2.0).abs() < 1e-6,
                "{name}: the cone at mid-length"
            );
            let seat = hi.z - apex.z;
            assert!(
                seat > bit / 2.0 * 2f64.sqrt() + 0.5,
                "{name}: the apex {seat} below the rim seats a {bit} ball"
            );
            // The rim's inner edge at its top: the trough's edge, where the
            // hex's corner (flat cradle) or side (V cradle) comes up to.
            let edge = c
                .polygons()
                .iter()
                .flat_map(|p| p.loops.iter().flatten())
                .filter(|v| (v.z - hi.z).abs() < 1e-6 && (v.y.abs() - apex.y.abs()).abs() > 4.5)
                .map(|v| v.y.abs())
                .fold(f64::INFINITY, f64::min);
            assert!(
                apex.y.abs() - 4.0 > edge + 1.0,
                "{name}: the cone's mouth at {} clears the trough's edge at {edge}",
                apex.y.abs() - 4.0
            );
        }
        assert!(
            (cs[0].0.y + cs[1].0.y).abs() < 1e-6,
            "{name}: mirrored about the axis"
        );
        if name.ends_with("cradle") && name.starts_with("Flat") || name.starts_with("Cradle") {
            match flat_cones {
                None => flat_cones = Some((cs[0].0.y, hi.z - cs[0].0.z)),
                Some((y, d)) => assert!(
                    (cs[0].0.y - y).abs() < 1e-6 && (hi.z - cs[0].0.z - d).abs() < 1e-6,
                    "{name}: the cones sit where the other flat cradle's do"
                ),
            }
        }
        eprintln!(
            "{name}: cones at y {:.1}, apex {:.2} below the rim",
            cs[1].0.y,
            hi.z - cs[1].0.z
        );
    }
}
