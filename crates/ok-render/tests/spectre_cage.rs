//! The Spectre stack cage (examples/spectre-cage) as a regression
//! suite: the master is the tile the top's pieces are cut from, centred
//! in a hex blank that holds it; every edge of its profile is reachable
//! by a vertical bit at one of the twelve indices the two cradles give
//! (the bit's body swept down from the open window clear of the profile
//! and the panels still in place); every finned panel clears the master
//! and the other fins and fills its sector to within the clearance; and
//! the loaded cage reads back as the stack it holds, the rods clear of
//! everything, the flanges the same hex as the master's ends, resting
//! on the cradles. Every number prints (`--nocapture`).

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

/// The profile in its hex with the panels round it and the bit's radius:
/// what the reach check sweeps against.
struct Scene<'a> {
    outline: &'a [Vec2],
    hx: &'a Hex,
    panels: &'a [Vec<Vec2>],
    r: f64,
}

impl Scene<'_> {
    /// Whether the bit coming down direction `d` with panel `open` off
    /// can cut the surface point `m` with outward normal `ne`: the ball
    /// beside the point is swept up to the window in steps, and at every
    /// step its circle stays off the profile and off every panel but the
    /// open one.
    fn reachable(&self, m: Vec2, ne: Vec2, d: Vec2, open: usize) -> bool {
        if ne.dot(d) < -0.02 {
            return false;
        }
        let r = self.r;
        let c = m + ne * r;
        let steps = ((self.hx.apothem + self.hx.panel_t + r) / 0.25).ceil() as usize;
        for s in 0..=steps {
            let q = c + d * (0.05 + s as f64 * 0.25);
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
}

/// The profile of the master in the hex, clocked as the build chose.
fn profile(doc: &Document, n: &serde_json::Value) -> (Vec<Vec2>, ok_sketch::spectre::Master) {
    let sf = spectre(doc, tab_named(doc, "Top"));
    let m = ok_sketch::spectre::master(&sf.params(), num(n, "round"), 10f64.to_radians()).unwrap();
    let outline: Vec<Vec2> = m.outline.iter().map(|q| rot(*q, num(n, "phase"))).collect();
    (outline, m)
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
fn the_master_is_the_tile_the_top_is_cut_from_centred_in_a_hex_that_holds_it() {
    let mut doc = load();
    let n = numbers();
    let sf = spectre(&doc, tab_named(&doc, "Top"));
    assert_eq!(
        (sf.edge, sf.gap, sf.bulge),
        (num(&n, "edge"), num(&n, "gap"), num(&n, "bulge"))
    );
    assert!((sf.bit - num(&n, "bit")).abs() < 1e-9);
    let (outline, m) = profile(&doc, &n);
    let hx = hex_of(&n);
    // The knobs are rounded to the bit: the master is a little under the
    // same tile with its corners sharp, and no convex corner turns more
    // than the facets.
    let sharp = ok_sketch::spectre::master(&sf.params(), 0.0, 10f64.to_radians()).unwrap();
    assert!(
        num(&n, "round") > 0.0 && m.area < sharp.area && m.area > sharp.area - 20.0,
        "{} vs {}",
        m.area,
        sharp.area
    );
    let count = outline.len();
    let sharpest = (0..count)
        .map(|i| {
            let (a, b, c) = (
                outline[(i + count - 1) % count],
                outline[i],
                outline[(i + 1) % count],
            );
            (b - a).cross(c - b).atan2((b - a).dot(c - b)).to_degrees()
        })
        .fold(0.0, f64::max);
    assert!(sharpest < 25.0, "a convex turn of {sharpest} degrees");
    // The blank holds the master's circle with its margin, and every
    // point of the clocked outline is inside the hex by that margin.
    assert!(
        2.0 * m.radius + 2.0 < hx.f,
        "{} across in {}",
        2.0 * m.radius,
        hx.f
    );
    for q in &outline {
        for k in 0..6 {
            let (_, nk) = hx.frame(k);
            assert!(q.dot(nk) < hx.apothem - 1.0, "{q:?} against face {k}");
        }
    }
    // The master body is that profile over the stack, with a hex end
    // the flanges' size at each end.
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
    let master = regen(&mut doc, "Master");
    let solid = body(&master, "Master");
    let f_out = num(&n, "F_out");
    let end = num(&n, "master_end");
    let hex_area = 3f64.sqrt() / 2.0 * f_out * f_out;
    let expected = m.area * length + 2.0 * hex_area * end;
    let volume = solid.volume();
    assert!(
        (volume - expected).abs() < 0.01 * expected,
        "{volume} vs {expected}"
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
    // Seen from above on the flat cradle, the profile alone (the fins'
    // tab has it without the ends) has no corner tighter than the
    // ball-nose bit: the fillets are the bit's own, and the arcs twice
    // it. What a pilot from above never sees, the bays' undersides, is
    // for the other indices.
    let fins = regen(&mut doc, "Finned panels");
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
        "master {volume:.0} mm3, {} points, {:.2} across, clocked {} deg; from above a ball bit leaves {:.1} mm2 of residual, {:.0} mm3 under overhangs",
        outline.len(),
        2.0 * m.radius,
        num(&n, "phase"),
        report.residual_area,
        report.undercut_volume
    );
    // The fillets are polylines at 10 degree facets, chords a few
    // hundredths inside the ball's circle, so the residual is that high
    // and no more: nothing a bit of this radius cannot get into.
    assert!(report.residual_max < 0.1, "{:?}", report.problems);
    assert!(report.residual_area < 30.0, "{:?}", report.problems);
    assert!(
        report.undercut_volume > 0.0,
        "the bays must be undercuts from one side"
    );
}

#[test]
fn every_edge_of_the_profile_is_reachable_at_one_of_the_twelve_indices() {
    let doc = load();
    let n = numbers();
    let (outline, _) = profile(&doc, &n);
    let hx = hex_of(&n);
    let panels: Vec<Vec<Vec2>> = (0..6).map(|k| hx.panel(k)).collect();
    let idx = hx.indices();
    let scene = Scene {
        outline: &outline,
        hx: &hx,
        panels: &panels,
        r: num(&n, "bit") / 2.0,
    };
    let count = outline.len();
    let (mut total, mut covered) = (0.0, 0.0);
    let mut per = vec![0.0; idx.len()];
    let mut missed = Vec::new();
    for i in 0..count {
        let (a, b) = (outline[i], outline[(i + 1) % count]);
        let e = b - a;
        let len = e.length();
        if len < 1e-9 {
            continue;
        }
        let m = (a + b) * 0.5;
        let ne = Vec2::new(e.y, -e.x) * (1.0 / len);
        total += len;
        let mut ok = false;
        for (w, (d, open, _)) in idx.iter().enumerate() {
            if scene.reachable(m, ne, *d, *open) {
                ok = true;
                per[w] += len;
            }
        }
        if ok {
            covered += len;
        } else {
            missed.push(m);
        }
    }
    for ((_, _, label), l) in idx.iter().zip(&per) {
        eprintln!("{label}: {l:.0} mm of {total:.0}");
    }
    // Every window cuts something at its flat index, and the whole
    // outline is reached somewhere.
    for (w, (_, _, label)) in idx.iter().enumerate().take(6) {
        assert!(per[w] > 10.0, "{label} cuts only {} mm", per[w]);
    }
    assert!(
        missed.is_empty(),
        "{:.1}% reached; unreachable at {:?}",
        100.0 * covered / total,
        &missed[..missed.len().min(8)]
    );
    // And six flat indices alone would not do: the tile's bays open
    // toward the hex's corners.
    let flat_only: f64 = (0..count)
        .filter_map(|i| {
            let (a, b) = (outline[i], outline[(i + 1) % count]);
            let e = b - a;
            let len = e.length();
            let m = (a + b) * 0.5;
            let ne = Vec2::new(e.y, -e.x) * (1.0 / len);
            idx.iter()
                .take(6)
                .any(|(d, open, _)| scene.reachable(m, ne, *d, *open))
                .then_some(len)
        })
        .sum();
    eprintln!("flat indices alone reach {:.1}%", 100.0 * flat_only / total);
    assert!(flat_only < 0.99 * total);
}

#[test]
fn the_finned_panels_clear_the_master_and_each_other_and_fill_their_sectors() {
    let mut doc = load();
    let n = numbers();
    let (outline, _) = profile(&doc, &n);
    let hx = hex_of(&n);
    let clear = num(&n, "clear");
    let length = num(&n, "length");
    let r = regen(&mut doc, "Finned panels");
    let master = body(&r, "Master");
    let fins: Vec<&Solid> = (1..=6)
        .map(|k| body(&r, &format!("Finned panel {k}")))
        .collect();
    for (k, fin) in fins.iter().enumerate() {
        let hit = overlap(fin, master);
        assert!(
            hit < 1e-6,
            "finned panel {} meets the master: {hit} mm3",
            k + 1
        );
        for (j, other) in fins.iter().enumerate().skip(k + 1) {
            let hit = overlap(fin, other);
            assert!(
                hit < 1e-6,
                "finned panels {} and {} meet: {hit} mm3",
                k + 1,
                j + 1
            );
        }
        // The fin fills the column over the profile within its sector,
        // the clearance off every way: the envelope sampled finely
        // across the window, the least depth within the clearance taken,
        // as the build did, and compared with the body less its plate.
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
        let plate = {
            let p = hx.panel(k);
            let (lo, hi) = bounds(fin);
            let _ = (lo, hi);
            ok_sketch::signed_area(&p).abs() * (hi.x - lo.x)
        };
        let fin_volume = fin.volume() - plate;
        let expected = area * (length - 1.0);
        eprintln!(
            "finned panel {}: fin {fin_volume:.0} mm3, sector column {expected:.0} mm3",
            k + 1
        );
        // Lobes too thin to print are left off, so the body is a little
        // under the column; never over it.
        assert!(
            fin_volume <= expected * 1.03 + 1.0,
            "fin {} over its column: {fin_volume} vs {expected}",
            k + 1
        );
        assert!(
            fin_volume >= expected * 0.9,
            "fin {} under its column: {fin_volume} vs {expected}",
            k + 1
        );
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
}
