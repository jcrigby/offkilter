//! The folding truck ramp (examples/ramp) as a regression suite: every
//! part regenerates closed, the panel and the ramp place every
//! instance, the six lug bores share the pipe's axis, the interior lugs
//! obey the nesting rule and leave the handle bare, the fold clears
//! itself from open to folded, and the open and folded packages measure
//! what the brief says. Every number prints (`--nocapture`); inches
//! throughout, converted from the document's millimetres.

use ok_brep::{BoolOp, Solid, Surface};
use ok_math::Vec3;
use ok_model::{AssemblyResult, Body, Document, TabId};
use std::path::PathBuf;

const IN: f64 = 25.4;

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
        17 + 5 + 7 + 6,
        "shared, panel A, panel B and ramp parts"
    );
    for (name, want) in [("panel A", 22), ("panel B", 24)] {
        let panel = tab_named(&doc, name);
        let r = doc.regenerate_assembly(panel).unwrap();
        assert_eq!(r.bodies.len(), want, "{name}");
        assert!(r.instance_errors.is_empty() && r.mate_errors.is_empty());
    }
    let ramp = tab_named(&doc, "ramp");
    let r = doc.regenerate_assembly(ramp).unwrap();
    assert_eq!(
        r.bodies.len(),
        22 + 24 + 6,
        "two panels, the pipe, two caps, two end plates, the angle"
    );
    assert!(r.instance_errors.is_empty() && r.mate_errors.is_empty());
    assert_eq!(doc.assembly(ramp).unwrap().mates.len(), 1, "the fold");
}

#[test]
fn the_six_lugs_share_the_pipe_axis_and_nest_with_the_handle_bare() {
    let mut doc = load();
    let ramp = tab_named(&doc, "ramp");
    let r = doc.regenerate_assembly(ramp).unwrap();
    // The pipe's axis: along x, through (0, -1) in y and z.
    let pipe = &body(&r, "pipe").solid;
    let (po, pa) = cylinders(pipe, 1.05 / 2.0)[0];
    assert!(pa.x.abs() > 1.0 - 1e-9, "pipe along x: {pa:?}");
    assert!(
        (po.y).abs() < 1e-6 && (po.z + 1.5 * IN).abs() < 1e-6,
        "{po:?}"
    );
    // Six lug bores of 1-1/8 in on it, two in each panel's rails and two
    // interior per panel, within a hundredth of a millimetre.
    let mut bores = 0;
    for name in [
        "rail_lug_left",
        "rail_lug_right",
        "lug_stub_left",
        "lug_stub_right",
        "interior_lug_a1",
        "interior_lug_a2",
        "interior_lug_b1",
        "interior_lug_b2",
    ] {
        for b in named(&r, name) {
            let cs = cylinders(&b.solid, 1.125 / 2.0);
            assert_eq!(cs.len(), 1, "{}: {} bores", b.name, cs.len());
            let (o, a) = cs[0];
            assert!(a.x.abs() > 1.0 - 1e-9, "{}: {a:?}", b.name);
            let off = Vec3::new(0.0, o.y - po.y, o.z - po.z);
            assert!(off.length() < 0.01, "{}: {off:?} off the pipe axis", b.name);
            bores += 1;
        }
    }
    assert_eq!(
        bores, 8,
        "two rail lugs or stubs and two interior lugs per panel"
    );
    // The interior lugs: one ply each, panel B's outboard of panel A's,
    // none overlapping, each against a rib of its own panel, and the
    // bare pipe between the inner pair the handle.
    let mut lugs: Vec<(String, f64, f64)> = Vec::new();
    for name in [
        "interior_lug_a1",
        "interior_lug_a2",
        "interior_lug_b1",
        "interior_lug_b2",
    ] {
        for b in named(&r, name) {
            let (lo, hi) = bounds(&b.solid);
            lugs.push((b.name.clone(), lo.x / IN, hi.x / IN));
        }
    }
    lugs.sort_by(|a, b| a.1.total_cmp(&b.1));
    println!("lugs across the pipe: {lugs:?}");
    assert!(
        lugs[0].0.starts_with("panel B") && lugs[3].0.starts_with("panel B"),
        "B's lugs outboard"
    );
    assert!(lugs[1].0.starts_with("panel A") && lugs[2].0.starts_with("panel A"));
    for w in lugs.windows(2) {
        assert!(w[1].1 >= w[0].2 - 1e-6, "{} overlaps {}", w[0].0, w[1].0);
    }
    for (name, lo, hi) in &lugs {
        assert!((hi - lo - 0.75).abs() < 1e-6, "{name}: one ply");
        let panel = name.split(" / ").next().unwrap();
        let beside = r.bodies.iter().any(|b| {
            b.name.starts_with(panel) && b.name.contains("/ rib") && {
                let (rl, rh) = bounds(&b.solid);
                (rl.x / IN - hi).abs() < 1e-6 || (rh.x / IN - lo).abs() < 1e-6
            }
        });
        assert!(beside, "{name} lies against no rib");
    }
    let handle = lugs[2].1 - lugs[1].2;
    println!("bare pipe between the inner lugs: {handle} in");
    assert!(handle >= 7.0, "handle {handle}");
    // Each end of the pipe carries panel A's rail lug with panel B's
    // stub beside it, outboard.
    let mut ends: Vec<(String, f64, f64)> = Vec::new();
    for name in [
        "rail_lug_left",
        "rail_lug_right",
        "lug_stub_left",
        "lug_stub_right",
    ] {
        for b in named(&r, name) {
            let (lo, hi) = bounds(&b.solid);
            ends.push((b.name.clone(), lo.x / IN, hi.x / IN));
        }
    }
    ends.sort_by(|a, b| a.1.total_cmp(&b.1));
    println!("rail lugs and stubs across the pipe: {ends:?}");
    assert_eq!(ends.len(), 4);
    assert!(ends[0].0.contains("lug_stub") && ends[1].0.contains("rail_lug"));
    assert!(ends[2].0.contains("rail_lug") && ends[3].0.contains("lug_stub"));
    assert!(
        (ends[1].1 - ends[0].2).abs() < 1e-6 && (ends[3].1 - ends[2].2).abs() < 1e-6,
        "side by side"
    );
    assert!(
        (ends[3].2 - ends[0].1 - 24.0).abs() < 1e-6,
        "24 over the lugs"
    );
}

#[test]
fn the_fold_clears_itself_and_the_packages_measure_up() {
    let mut doc = load();
    let ramp = tab_named(&doc, "ramp");
    let asm = doc.assembly(ramp).unwrap();
    let fold = asm.mates.iter().find(|m| m.name == "fold").unwrap();
    let (id, angle0, offset) = (fold.id, fold.angle, fold.offset);
    let r0 = doc.regenerate_assembly(ramp).unwrap();
    // Open: 96 by 24 over the rails, decks coplanar, joint faces touching.
    let (lo, hi) = extent(&r0, |b| {
        !b.name.contains("end_plate") && !b.name.contains("ground_angle")
    });
    println!(
        "open: {:.2} x {:.2} x {:.2} in",
        (hi.x - lo.x) / IN,
        (hi.y - lo.y) / IN,
        (hi.z - lo.z) / IN
    );
    assert!(((hi.y - lo.y) / IN - 96.0).abs() < 1e-6);
    let rails = extent(&r0, |b| b.name.contains("/ rail"));
    assert!(
        ((rails.1.x - rails.0.x) / IN - 22.5).abs() < 1e-6,
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
    let b1 = bounds(&named(&r, "skin_bottom_a")[0].solid);
    let b2 = bounds(&named(&r, "skin_bottom_b")[0].solid);
    let gap = (b1.0.z - b2.1.z) / IN;
    println!("folded: bottoms {gap:.3} in apart");
    assert!((gap - 3.0).abs() < 1e-6, "twice the pin drop");
    let (lo, hi) = extent(&r, |b| {
        b.name.contains("/ rail") || b.name.contains("lug_stub")
    });
    let (w, l, t) = ((hi.x - lo.x) / IN, (hi.y - lo.y) / IN, (hi.z - lo.z) / IN);
    println!("folded package over the rails: {w:.3} x {l:.3} x {t:.3} in");
    assert!(
        (w - 24.0).abs() < 1e-6 && (t - (2.0 * 3.375 + 3.0)).abs() < 1e-6,
        "{w} x {t}"
    );
    // The rails run to 45 and their half-rounds an inch past the joint;
    // the stubs make the panel's 48.
    assert!((l - 46.5).abs() < 1e-6, "{l} over the rails");
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
    for b in r.bodies.iter().filter(|b| b.name.contains("interior_lug")) {
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
/// rails, cheeks, spacers and lug plies out of about half a 4 x 8 of
/// birch, both read off the parts' faces.
#[test]
fn the_skins_fill_one_sheet_and_the_birch_half_of_another() {
    let mut doc = load();
    let panel = tab_named(&doc, "panel A");
    let r = doc.regenerate_assembly(panel).unwrap();
    let area = |b: &Body| {
        let (lo, hi) = bounds(&b.solid);
        (hi.x - lo.x) * (hi.y - lo.y) / (IN * IN)
    };
    let skins: f64 = 2.0 * (area(named(&r, "skin_top")[0]) + area(named(&r, "skin_bottom_a")[0]));
    let sheet = 48.0 * 96.0;
    println!(
        "skins: {skins:.0} in2 of a {sheet:.0} in2 sheet ({:.0} %)",
        100.0 * skins / sheet
    );
    assert!(skins <= sheet, "the skins do not fit one sheet");
    // Two rips of the skins' width from a 48 in sheet, each crosscut
    // into two skins' lengths from 96.
    let (lo, hi) = bounds(&named(&r, "skin_top")[0].solid);
    let (sw, sl) = ((hi.x - lo.x) / IN, (hi.y - lo.y) / IN);
    assert!(2.0 * sw <= 48.0 && 2.0 * sl <= 96.0, "{sw} x {sl} skins");
    let side = |b: &Body| {
        let (lo, hi) = bounds(&b.solid);
        (hi.y - lo.y) * (hi.z - lo.z) / (IN * IN)
    };
    let mut birch = 0.0;
    for name in [
        "rail_lug_left",
        "rail_lug_right",
        "interior_lug_a1",
        "interior_lug_a2",
    ] {
        birch += side(named(&r, name)[0]);
    }
    let rb = doc.regenerate_assembly(tab_named(&doc, "panel B")).unwrap();
    for name in [
        "lug_stub_left",
        "lug_stub_right",
        "interior_lug_b1",
        "interior_lug_b2",
        "rail_plain 1",
        "rail_plain 2",
    ] {
        birch += side(named(&rb, name)[0]);
    }
    println!(
        "birch: {birch:.0} in2, {:.0} % of a sheet",
        100.0 * birch / sheet
    );
    assert!(birch > 0.15 * sheet && birch < 0.4 * sheet, "{birch}");
}
