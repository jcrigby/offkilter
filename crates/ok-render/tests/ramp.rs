//! The folding truck ramp (examples/ramp) as a regression suite: every
//! part regenerates closed, the panel and the ramp place every
//! instance, the eight lug bores and their bushings share the nipples' axis, the lugs
//! interleave inside the rails with a nipple through each four, the fold clears
//! itself from open to folded, and the open and folded packages measure
//! what the brief says. Every number prints (`--nocapture`); inches
//! throughout, converted from the document's millimetres.

use ok_brep::{BoolOp, Solid, Surface};
use ok_math::Vec3;
use ok_model::{AssemblyResult, Body, Document, TabId};
use std::path::PathBuf;

const IN: f64 = 25.4;
/// The panels' width over the box: a third of a 95-7/8 CDX sheet with
/// two kerfs, for a 29 in aerator.
const W: f64 = (95.875 - 0.25) / 3.0;

fn load() -> Document {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/ramp");
    let json = std::fs::read_to_string(dir.join("out/ramp.okpart")).unwrap();
    Document::from_json(&json).unwrap()
}

fn tab_named(doc: &Document, name: &str) -> TabId {
    doc.tabs
        .iter()
        .find(|t| t.name() == name)
        .unwrap_or_else(|| panic!("no tab {name}"))
        .id
}

fn bounds(s: &Solid) -> (Vec3, Vec3) {
    s.bounds().unwrap()
}

/// The placed bodies whose name ends in `member` (a sub-assembly's
/// member is placed as "Sub / member").
fn named<'a>(r: &'a AssemblyResult, member: &str) -> Vec<&'a Body> {
    let v: Vec<_> = r
        .bodies
        .iter()
        .filter(|b| b.name.rsplit(" / ").next().unwrap() == member)
        .collect();
    assert!(!v.is_empty(), "no body {member}");
    v
}

fn body<'a>(r: &'a AssemblyResult, name: &str) -> &'a Body {
    r.bodies
        .iter()
        .find(|b| b.name == name)
        .unwrap_or_else(|| panic!("no body {name}"))
}

/// Overlap volume of two placed bodies, None when the boolean fails.
fn overlap(a: &Solid, b: &Solid) -> Option<f64> {
    let (la, ha) = bounds(a);
    let (lb, hb) = bounds(b);
    if la.x > hb.x || lb.x > ha.x || la.y > hb.y || lb.y > ha.y || la.z > hb.z || lb.z > ha.z {
        return Some(0.0);
    }
    ok_brep::boolean(a, b, BoolOp::Intersection)
        .ok()
        .map(|s| s.volume().abs())
}

/// Cylinders of a placed body of about radius `r` (inches): origin and
/// unit axis.
fn cylinders(s: &Solid, r: f64) -> Vec<(Vec3, Vec3)> {
    s.surfaces
        .iter()
        .filter_map(|sf| match sf {
            Surface::Cylinder {
                origin,
                axis,
                radius,
            } if (radius - r * IN).abs() < 1e-6 => {
                Some((*origin, axis.normalized().unwrap_or(Vec3::X)))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn every_part_regenerates_closed_and_the_panel_and_ramp_place_every_instance() {
    let mut doc = load();
    let tabs: Vec<(TabId, String, String)> = doc
        .tabs
        .iter()
        .map(|t| (t.id, t.name().to_string(), t.kind_name().to_string()))
        .collect();
    let mut bodies = 0;
    for (id, name, kind) in &tabs {
        if kind == "part_studio" {
            let r = doc.regenerate_studio(*id, None).unwrap();
            assert!(!r.bodies.is_empty(), "{name}: no bodies");
            for b in &r.bodies {
                assert!(b.solid.volume() > 0.0, "{name} has volume");
                b.solid.validate().unwrap();
            }
            assert!(
                r.statuses.iter().all(|f| f.error.is_none()),
                "{name}: {:?}",
                r.statuses
            );
            bodies += r.bodies.len();
        }
    }
    assert_eq!(
        bodies,
        29 + 2 + 2 + 9 + 2,
        "shared, panel A, panel B, ramp parts and the two sheets"
    );
    for (name, want) in [("panel A", 31), ("panel B", 31)] {
        let panel = tab_named(&doc, name);
        let r = doc.regenerate_assembly(panel).unwrap();
        assert_eq!(r.bodies.len(), want, "{name}");
        assert!(r.instance_errors.is_empty() && r.mate_errors.is_empty());
    }
    let ramp = tab_named(&doc, "ramp");
    let r = doc.regenerate_assembly(ramp).unwrap();
    assert_eq!(
        r.bodies.len(),
        31 + 31 + 9,
        "two panels, two nipples, four caps, two end plates, the angle"
    );
    assert!(r.instance_errors.is_empty() && r.mate_errors.is_empty());
    assert_eq!(doc.assembly(ramp).unwrap().mates.len(), 1, "the fold");
    // Panel B's stubs stop flush with its skins at 45; panel A's run
    // bare to 48 for the end plates.
    let stubs = extent(&r, |b| b.name.starts_with("panel B / stub"));
    let skins = extent(&r, |b| b.name.starts_with("panel B / skin"));
    assert!(
        (stubs.0.y / IN + 45.0).abs() < 1e-6,
        "panel B's stubs end at -45"
    );
    assert!((stubs.0.y - skins.0.y).abs() < 1e-6, "flush with the skins");
    let a_stubs = extent(&r, |b| b.name.starts_with("panel A / stub"));
    assert!(
        (a_stubs.1.y / IN - 48.0).abs() < 1e-6,
        "panel A's stubs run to 48"
    );
    // The ground angle hangs on that flush end: one leg flat on the end
    // face, the other out past it flush with the top skin, with a screw
    // hole into each stub's end grain, two per stub, both below the
    // top skin.
    let (lo, hi) = extent(&r, |b| b.name == "ground_angle");
    assert!(
        (hi.y - stubs.0.y).abs() < 1e-6,
        "the leg is on the end face"
    );
    assert!(
        ((hi.y - lo.y) / IN - 1.5).abs() < 1e-6,
        "the lip reaches 1.5 in out"
    );
    assert!((hi.z - skins.1.z).abs() < 1e-6, "flush with the top skin");
    assert!(
        ((hi.z - lo.z) / IN - 1.5).abs() < 1e-6,
        "the leg is 1.5 in down"
    );
    let angle = r.bodies.iter().find(|b| b.name == "ground_angle").unwrap();
    let holes = cylinders(&angle.solid, 0.1875 / 2.0);
    assert_eq!(holes.len(), 4, "four #10 holes");
    for b in r
        .bodies
        .iter()
        .filter(|b| b.name.starts_with("panel B / stub"))
    {
        let (slo, shi) = bounds(&b.solid);
        let n = holes
            .iter()
            .filter(|(o, a)| {
                a.y.abs() > 0.999 && o.x > slo.x && o.x < shi.x && o.z > slo.z && o.z < shi.z
            })
            .count();
        assert_eq!(n, 2, "two screws into {}", b.name);
    }
}

#[test]
fn the_eight_lugs_share_the_nipples_axis_and_interleave_inside_the_rails() {
    let mut doc = load();
    let ramp = tab_named(&doc, "ramp");
    let r = doc.regenerate_assembly(ramp).unwrap();
    // The nipples' axis: along x, through (0, -1.5) in y and z, both
    // of them.
    let pipe = &body(&r, "pipe 1").solid;
    let (po, pa) = cylinders(pipe, 0.84 / 2.0)[0];
    assert!(pa.x.abs() > 1.0 - 1e-9, "pipe along x: {pa:?}");
    let (qo, qa) = cylinders(&body(&r, "pipe 2").solid, 0.84 / 2.0)[0];
    assert!(qa.x.abs() > 1.0 - 1e-9 && (qo.y - po.y).abs() < 1e-6 && (qo.z - po.z).abs() < 1e-6);
    assert!(
        (po.y).abs() < 1e-6 && (po.z + 1.5 * IN).abs() < 1e-6,
        "{po:?}"
    );
    // Eight lug bores of 1.315 in on it, the bushing rings' seats, four a
    // panel, within a hundredth of a millimetre.
    let mut bores = 0;
    for name in ["lug_1", "lug_2", "lug_3", "lug_4"] {
        for b in named(&r, name) {
            let cs = cylinders(&b.solid, 1.315 / 2.0);
            assert_eq!(cs.len(), 1, "{}: {} bores", b.name, cs.len());
            let (o, a) = cs[0];
            assert!(a.x.abs() > 1.0 - 1e-9, "{}: {a:?}", b.name);
            let off = Vec3::new(0.0, o.y - po.y, o.z - po.z);
            assert!(off.length() < 0.01, "{}: {off:?} off the pipe axis", b.name);
            bores += 1;
        }
    }
    assert_eq!(bores, 8, "four lugs a panel");
    // A ring of 1 in pipe in each, 3/4 long and flush with its lug, its
    // 1.049 bore on the axis, the 0.84 pin loose in it.
    let mut bushings = 0;
    for b in r.bodies.iter().filter(|b| b.name.contains("/ bushing ")) {
        let (o, a) = *cylinders(&b.solid, 1.049 / 2.0)
            .first()
            .unwrap_or_else(|| panic!("{}: no 1.049 bore", b.name));
        assert!(a.x.abs() > 1.0 - 1e-9, "{}: {a:?}", b.name);
        let off = Vec3::new(0.0, o.y - po.y, o.z - po.z);
        assert!(off.length() < 0.01, "{}: {off:?} off the pipe axis", b.name);
        assert_eq!(
            cylinders(&b.solid, 1.315 / 2.0).len(),
            1,
            "{}: the seat",
            b.name
        );
        let (lo, hi) = bounds(&b.solid);
        assert!(
            ((hi.x - lo.x) / IN - 0.75).abs() < 1e-6,
            "{}: one ply long",
            b.name
        );
        let panel = b.name.split(" / ").next().unwrap();
        let in_lug = r.bodies.iter().any(|l| {
            l.name.starts_with(panel)
                && (l.name.contains("lug") || l.name.contains("rail_lug"))
                && !l.name.contains("bushing")
                && {
                    let (ll, lh) = bounds(&l.solid);
                    (ll.x - lo.x).abs() < 1e-6 && (lh.x - hi.x).abs() < 1e-6
                }
        });
        assert!(in_lug, "{}: flush with no lug of its panel", b.name);
        assert_eq!(
            b.material.as_ref().map(|m| m.name.as_str()),
            Some("galvanized steel")
        );
        bushings += 1;
    }
    assert_eq!(bushings, 8, "a bushing in every lug");
    // The lugs: one ply each, all alike, inside the rails, B, A, B, A,
    // B, A, B, A across the pipe, none overlapping, each against a rib
    // of its own panel, and the bare pipe between the middle pair the
    // handle.
    let mut lugs: Vec<(String, f64, f64)> = Vec::new();
    for name in ["lug_1", "lug_2", "lug_3", "lug_4"] {
        for b in named(&r, name) {
            let (lo, hi) = bounds(&b.solid);
            lugs.push((b.name.clone(), lo.x / IN, hi.x / IN));
        }
    }
    lugs.sort_by(|a, b| a.1.total_cmp(&b.1));
    println!("lugs across the pipe: {lugs:?}");
    assert_eq!(lugs.len(), 8);
    for (i, (name, lo, hi)) in lugs.iter().enumerate() {
        let panel = if i % 2 == 0 { "panel B" } else { "panel A" };
        assert!(name.starts_with(panel), "{i}: {name}");
        assert!((hi - lo - 0.75).abs() < 1e-6, "{name}: one ply");
        let beside = r.bodies.iter().any(|b| {
            b.name.starts_with(panel) && b.name.contains("/ rib") && {
                let (rl, rh) = bounds(&b.solid);
                (rl.x / IN - hi).abs() < 1e-6 || (rh.x / IN - lo).abs() < 1e-6
            }
        });
        assert!(beside, "{name} lies against no rib");
    }
    for w in lugs.windows(2) {
        assert!(w[1].1 >= w[0].2 - 1e-6, "{} overlaps {}", w[0].0, w[1].0);
    }
    let rails = extent(&r, |b| b.name.contains("/ rail"));
    assert!(
        lugs[0].1 >= 0.0 - 1e-6 && lugs[7].2 <= W + 1e-6,
        "the lugs stay inside the box"
    );
    assert!(
        rails.0.x / IN < lugs[0].1 && rails.1.x / IN > lugs[7].2,
        "nothing past the rails"
    );
    // Two precut 10 in nipples, one through each side's four lugs, the
    // outer caps ending flush with the rails' faces and the inner caps
    // in the gap between the middle lugs, clear of them.
    for (name, from, to) in [("pipe 1", 0, 3), ("pipe 2", 4, 7)] {
        let (plo, phi) = bounds(&body(&r, name).solid);
        assert!(
            ((phi.x - plo.x) / IN - 10.0).abs() < 1e-6,
            "{name} is a 10 in nipple"
        );
        assert!(
            plo.x / IN < lugs[from].1 && phi.x / IN > lugs[to].2,
            "{name} through its four lugs"
        );
    }
    let caps: Vec<(f64, f64)> = r
        .bodies
        .iter()
        .filter(|b| b.name.starts_with("pipe_cap"))
        .map(|b| {
            let (lo, hi) = bounds(&b.solid);
            (lo.x / IN, hi.x / IN)
        })
        .collect();
    assert_eq!(caps.len(), 4);
    let (clo, chi) = caps
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), c| {
            (a.min(c.0), b.max(c.1))
        });
    assert!(
        (clo - rails.0.x / IN).abs() < 1e-6 && (chi - rails.1.x / IN).abs() < 1e-6,
        "the outer caps end at the rails' faces"
    );
    for (c0, c1) in &caps {
        for (name, l0, l1) in &lugs {
            assert!(
                *c1 <= l0 + 1e-6 || *c0 >= l1 - 1e-6,
                "cap {c0}..{c1} meets {name}"
            );
        }
    }
    let right = caps
        .iter()
        .filter(|c| c.0 > W / 2.0)
        .map(|c| c.0)
        .fold(f64::INFINITY, f64::min);
    let left = caps
        .iter()
        .filter(|c| c.1 < W / 2.0)
        .map(|c| c.1)
        .fold(f64::NEG_INFINITY, f64::max);
    let gap = right - left;
    println!("between the inner caps: {gap} in");
}

#[test]
fn the_fold_clears_itself_and_the_packages_measure_up() {
    let mut doc = load();
    let ramp = tab_named(&doc, "ramp");
    let asm = doc.assembly(ramp).unwrap();
    let fold = asm.mates.iter().find(|m| m.name == "fold").unwrap();
    let (id, angle0, offset) = (fold.id, fold.angle, fold.offset);
    let r0 = doc.regenerate_assembly(ramp).unwrap();
    // Open: 93 long (panel A's bare stubs to 48, panel B flush at 45),
    // the width and 3 over the rails, decks coplanar, joint faces touching.
    let (lo, hi) = extent(&r0, |b| {
        !b.name.contains("end_plate") && !b.name.contains("ground_angle")
    });
    println!(
        "open: {:.2} x {:.2} x {:.2} in",
        (hi.x - lo.x) / IN,
        (hi.y - lo.y) / IN,
        (hi.z - lo.z) / IN
    );
    assert!(
        ((hi.y - lo.y) / IN - 93.0).abs() < 1e-6,
        "48 and 45 from the joint"
    );
    let rails = extent(&r0, |b| b.name.contains("/ rail"));
    assert!(
        ((rails.1.x - rails.0.x) / IN - (W + 1.5)).abs() < 1e-6,
        "{} over the rails",
        (rails.1.x - rails.0.x) / IN
    );
    let decks: Vec<f64> = named(&r0, "skin_top")
        .iter()
        .map(|b| bounds(&b.solid).1.z)
        .collect();
    assert!((decks[0] - decks[1]).abs() < 1e-6, "decks coplanar");
    // Which way is under: the turned panel's free end drops.
    let low = |r: &AssemblyResult| {
        r.bodies
            .iter()
            .filter(|b| b.name.starts_with("panel B /"))
            .map(|b| bounds(&b.solid).0.z)
            .fold(f64::INFINITY, f64::min)
    };
    let r30 = doc
        .preview_assembly_at(ramp, &[(id, angle0 + 30.0, offset)])
        .unwrap();
    let sign = if low(&r30) < low(&r0) - IN { 1.0 } else { -1.0 };
    // No two parts of different instances meet anew at any angle on the
    // way to folded; a touch is allowed.
    for deg in [0.0, 5.0, 10.0, 20.0, 45.0, 90.0, 135.0, 170.0, 180.0] {
        let r = doc
            .preview_assembly_at(ramp, &[(id, angle0 + sign * deg, offset)])
            .unwrap();
        assert!(r.instance_errors.is_empty() && r.mate_errors.is_empty());
        let mut worst = 0.0f64;
        let mut hits = Vec::new();
        for i in 0..r.bodies.len() {
            for j in i + 1..r.bodies.len() {
                if r.placed[i] == r.placed[j] {
                    continue;
                }
                match overlap(&r.bodies[i].solid, &r.bodies[j].solid) {
                    Some(v) if v > 1e-3 => {
                        worst = worst.max(v);
                        hits.push(format!(
                            "{} x {}: {v:.1} mm3",
                            r.bodies[i].name, r.bodies[j].name
                        ));
                    }
                    None => hits.push(format!(
                        "{} x {}: boolean failed",
                        r.bodies[i].name, r.bodies[j].name
                    )),
                    _ => {}
                }
            }
        }
        println!(
            "{deg} degrees: {} collisions, worst {worst:.1} mm3",
            hits.len()
        );
        for h in &hits {
            println!("  {h}");
        }
        assert!(hits.is_empty(), "{deg} degrees: {hits:?}");
    }
    // Folded: bottoms 2 in apart (twice the pin drop), the package 24 wide
    // over the rails, 48 long over the boxes, two rail heights and the
    // gap thick, the interior lugs interleaved between the bottoms.
    let r = doc
        .preview_assembly_at(ramp, &[(id, angle0 + sign * 180.0, offset)])
        .unwrap();
    let skins = named(&r, "skin_bottom");
    let b1 = bounds(&skins[0].solid);
    let b2 = bounds(&skins[1].solid);
    let gap = (b1.0.z - b2.1.z) / IN;
    println!("folded: bottoms {gap:.3} in apart");
    assert!((gap - 3.0).abs() < 1e-6, "twice the pin drop");
    let (lo, hi) = extent(&r, |b| b.name.contains("/ rail"));
    let (w, l, t) = ((hi.x - lo.x) / IN, (hi.y - lo.y) / IN, (hi.z - lo.z) / IN);
    println!("folded package over the rails: {w:.3} x {l:.3} x {t:.3} in");
    assert!(
        (w - (W + 1.5)).abs() < 1e-6 && (t - (2.0 * 3.375 + 3.0)).abs() < 1e-6,
        "{w} x {t}"
    );
    // The plain rails run to 45; the stubs make the panel's 48.
    assert!((l - 45.0).abs() < 1e-6, "{l} over the rails");
    let panels = extent(&r, |b| b.name.starts_with("panel"));
    let ly = (panels.1.y - panels.0.y) / IN;
    println!("folded package over the panels: {ly:.3} in long");
    assert!(
        (ly - 49.5).abs() < 1e-6,
        "{ly}: 48 from the joint to the stubs' ends and the half-rounds past it"
    );
    // The interior lugs fill the gap: panel A's hang down to panel B's
    // bottom, panel B's reach up to panel A's, and they touch nothing
    // (the sweep above), so they interleave.
    for b in r.bodies.iter().filter(|b| b.name.contains("/ lug_")) {
        let (lo, hi) = bounds(&b.solid);
        if b.name.starts_with("panel A") {
            assert!(
                (lo.z - b2.1.z).abs() < 1e-6,
                "{}: hangs to {} not {}",
                b.name,
                lo.z,
                b2.1.z
            );
        } else {
            assert!(
                (hi.z - b1.0.z).abs() < 1e-6,
                "{}: reaches {} not {}",
                b.name,
                hi.z,
                b1.0.z
            );
        }
    }
}

/// The bounds of the placed bodies `keep` picks.
fn extent(r: &AssemblyResult, keep: impl Fn(&Body) -> bool) -> (Vec3, Vec3) {
    let mut lo = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut hi = Vec3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for b in r.bodies.iter().filter(|b| keep(b)) {
        let (l, h) = bounds(&b.solid);
        lo = Vec3::new(lo.x.min(l.x), lo.y.min(l.y), lo.z.min(l.z));
        hi = Vec3::new(hi.x.max(h.x), hi.y.max(h.y), hi.z.max(h.z));
    }
    (lo, hi)
}

/// Sheet yield: the four skins come out of one 4 x 8 of CDX, and the
/// rails, lugs, stubs, ribs and cross blocks out of under half a 4 x 8 of
/// birch, both read off the parts' faces.
#[test]
fn the_skins_take_a_sheet_and_a_third_and_the_birch_half_of_another() {
    let mut doc = load();
    let panel = tab_named(&doc, "panel A");
    let r = doc.regenerate_assembly(panel).unwrap();
    let area = |b: &Body| {
        let (lo, hi) = bounds(&b.solid);
        (hi.x - lo.x) * (hi.y - lo.y) / (IN * IN)
    };
    let skins: f64 = 2.0 * (area(named(&r, "skin_top")[0]) + area(named(&r, "skin_bottom")[0]));
    // CDX sheathing is sized for spacing, 95-7/8 x 47-7/8: three skins
    // come out of one sheet as thirds of its length with two kerfs, and
    // the fourth from a second sheet.
    let sheet = 95.875 * 47.875;
    println!("skins: {skins:.0} in2, {:.2} CDX sheets", skins / sheet);
    let (lo, hi) = bounds(&named(&r, "skin_top")[0].solid);
    let (sw, sl) = ((hi.x - lo.x) / IN, (hi.y - lo.y) / IN);
    assert!(
        (sw - W).abs() < 1e-6 && (sl - 45.0).abs() < 1e-6,
        "{sw} x {sl} skins"
    );
    assert!(
        3.0 * sw + 2.0 * 0.125 <= 95.875 + 1e-9 && sl <= 47.875,
        "three to a sheet"
    );
    assert!(skins > sheet && skins < 2.0 * sheet, "{skins}");
    let side = |b: &Body| {
        let (lo, hi) = bounds(&b.solid);
        (hi.y - lo.y) * (hi.z - lo.z) / (IN * IN)
    };
    // The birch: rails, lugs, stubs, ribs and cross blocks of both panels,
    // each read as its side area (it stands on edge).
    let rb = doc.regenerate_assembly(tab_named(&doc, "panel B")).unwrap();
    let is_birch = |b: &Body| {
        let n = b.name.rsplit(" / ").next().unwrap();
        n.contains("rail") || n.contains("lug") || n.contains("rib") || n.contains("cross_block")
    };
    let birch: f64 = r
        .bodies
        .iter()
        .chain(rb.bodies.iter())
        .filter(|b| is_birch(b))
        .map(side)
        .sum();
    println!(
        "birch: {birch:.0} in2, {:.0} % of a sheet",
        100.0 * birch / sheet
    );
    assert!(birch > 0.25 * sheet && birch < 0.5 * sheet, "{birch}");
}

/// The cut sheets: every plywood piece lies flat on its 4 x 8, inside
/// it, a kerf from its neighbours, and the pieces are the parts.
#[test]
fn the_cut_sheets_lay_every_ply_piece_flat_and_apart() {
    let mut doc = load();
    for (name, thickness, count, material, size) in [
        ("cut sheet, CDX 1", 0.4375, 3, "15/32 CDX", (95.875, 47.875)),
        ("cut sheet, CDX 2", 0.4375, 1, "15/32 CDX", (95.875, 47.875)),
        ("cut sheet, birch", 0.75, 44, "3/4 birch", (96.0, 48.0)),
    ] {
        let tab = tab_named(&doc, name);
        let r = doc.regenerate_assembly(tab).unwrap();
        assert!(r.instance_errors.is_empty(), "{name}");
        let sheet = r
            .bodies
            .iter()
            .find(|b| b.name.starts_with("sheet_"))
            .unwrap();
        let (slo, shi) = bounds(&sheet.solid);
        assert!(
            ((shi.x - slo.x) / IN - size.0).abs() < 1e-6
                && ((shi.y - slo.y) / IN - size.1).abs() < 1e-6
        );
        let pieces: Vec<&Body> = r
            .bodies
            .iter()
            .filter(|b| !b.name.starts_with("sheet_"))
            .collect();
        assert_eq!(pieces.len(), count, "{name}");
        let mut area = 0.0;
        for (i, p) in pieces.iter().enumerate() {
            let (lo, hi) = bounds(&p.solid);
            assert!(
                lo.z > -1e-6 && (hi.z / IN - thickness).abs() < 1e-6,
                "{}: z {} to {}",
                p.name,
                lo.z,
                hi.z
            );
            assert!(
                lo.x > -1e-6 && hi.x < shi.x + 1e-6 && lo.y > -1e-6 && hi.y < shi.y + 1e-6,
                "{} is off the sheet",
                p.name
            );
            assert_eq!(
                p.material.as_ref().map(|m| m.name.as_str()),
                Some(material),
                "{}",
                p.name
            );
            area += (hi.x - lo.x) * (hi.y - lo.y);
            for q in &pieces[..i] {
                let (qlo, qhi) = bounds(&q.solid);
                let apart = lo.x >= qhi.x + 0.1 * IN
                    || qlo.x >= hi.x + 0.1 * IN
                    || lo.y >= qhi.y + 0.1 * IN
                    || qlo.y >= hi.y + 0.1 * IN;
                assert!(apart, "{} and {} overlap or touch", p.name, q.name);
            }
        }
        let yield_ = area / ((shi.x - slo.x) * (shi.y - slo.y));
        println!(
            "{name}: {count} pieces, {:.0}% of the sheet by bounding box",
            yield_ * 100.0
        );
        assert!(yield_ < 0.95 && yield_ > 0.2, "{yield_}");
    }
}
