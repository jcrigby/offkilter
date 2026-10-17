//! The folding truck ramp (examples/ramp) as a regression suite: every
//! part regenerates closed, the panel and the ramp place every
//! instance, the hinge's bars pair their struts on the bolts outside
//! the rails with the bands flat on them, the screws bite three inches
//! of plywood, the fold clears itself from open to folded, and the
//! open and folded packages measure what the brief says. Every number prints (`--nocapture`); inches
//! throughout, converted from the document's millimetres.

use ok_brep::{BoolOp, Solid, Surface};
use ok_math::Vec3;
use ok_model::{AssemblyResult, Body, Document, TabId};
use std::path::PathBuf;

const IN: f64 = 25.4;
/// The panels' width over the box, a rip along the OSB sheet, for a
/// 29 in aerator.
const W: f64 = 31.875;
/// The rails' height: the 2-13/16 box and the 1 in curb.
const RAIL_H: f64 = 2.8125 + 1.0;

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
        43 + 4 + 4 + 5 + 2,
        "shared, panel A, panel B, ramp parts and the two sheets"
    );
    for (name, want) in [("panel A", 47), ("panel B", 47)] {
        let panel = tab_named(&doc, name);
        let r = doc.regenerate_assembly(panel).unwrap();
        assert_eq!(r.bodies.len(), want, "{name}");
        assert!(r.instance_errors.is_empty() && r.mate_errors.is_empty());
    }
    let ramp = tab_named(&doc, "ramp");
    let r = doc.regenerate_assembly(ramp).unwrap();
    assert_eq!(
        r.bodies.len(),
        47 + 47 + 5,
        "two panels, two bolts, two end plates, the angle"
    );
    assert!(r.instance_errors.is_empty() && r.mate_errors.is_empty());
    assert_eq!(doc.assembly(ramp).unwrap().mates.len(), 1, "the fold");
    // Panel B's stubs stop flush with its skins at 45; panel A's run
    // bare to 48 for the end plates, their shims stopping with the skins.
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
    // The core is 1-15/16: a 2x4 or 2x8 flat and a 7/16 shim of the
    // skin stock over it, the shim ending with the skins; the ribs
    // the same height in one piece.
    for name in ["panel A / stub", "panel B / stub", "panel A / joint_block"] {
        let lumber = extent(&r, |b| b.name.starts_with(name) && b.material.is_none());
        let shim = extent(&r, |b| {
            b.name.starts_with(name)
                && b.material.as_ref().map(|m| m.name.as_str()) == Some("7/16 OSB")
        });
        assert!(
            ((lumber.1.z - lumber.0.z) / IN - 1.5).abs() < 1e-6
                && (shim.0.z - lumber.1.z).abs() < 1e-6
                && ((shim.1.z - shim.0.z) / IN - 0.4375).abs() < 1e-6,
            "{name}: lumber {:?}, shim {:?}",
            lumber,
            shim
        );
        let skin = extent(&r, |b| {
            b.name.starts_with(
                &name
                    .replace("stub", "skin_top")
                    .replace("joint_block", "skin_top"),
            )
        });
        assert!(
            (skin.0.z - shim.1.z).abs() < 1e-6,
            "{name}: the shim meets the top skin"
        );
        if name.ends_with("stub") {
            let far = shim.0.y.abs().max(shim.1.y.abs()) / IN;
            assert!((far - 45.0).abs() < 1e-6, "{name}: shim to {far}, not 45");
        }
    }
    let rib = extent(&r, |b| b.name == "panel A / rib 1");
    assert!(
        ((rib.1.z - rib.0.z) / IN - 1.9375).abs() < 1e-6,
        "ribs 1-15/16"
    );
    // The ground angle hangs on that flush end: one leg flat on the end
    // face, the other out past it flush with the top skin, with a screw
    // hole into each stub's end grain, two per stub, both below the
    // top skin, in the 2x8 under its shim.
    let (lo, hi) = extent(&r, |b| b.name == "ground_angle");
    assert!(
        (hi.y - stubs.0.y).abs() < 1e-6,
        "the leg is on the end face"
    );
    assert!(
        ((hi.y - lo.y) / IN - 2.0).abs() < 1e-6,
        "the lip reaches 2 in out"
    );
    assert!((hi.z - skins.1.z).abs() < 1e-6, "flush with the top skin");
    assert!(
        ((hi.z - lo.z) / IN - 2.0).abs() < 1e-6,
        "the leg is 2 in down"
    );
    let angle = r.bodies.iter().find(|b| b.name == "ground_angle").unwrap();
    let holes = cylinders(&angle.solid, 0.1875 / 2.0);
    assert_eq!(holes.len(), 4, "four #10 holes");
    for b in r
        .bodies
        .iter()
        .filter(|b| b.name.starts_with("panel B / stub") && b.material.is_none())
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
fn the_hinge_bars_pair_their_struts_on_the_bolts_outside_the_rails() {
    let mut doc = load();
    let ramp = tab_named(&doc, "ramp");
    let r = doc.regenerate_assembly(ramp).unwrap();
    // The two bolts' axis: along x, through (0, -1.5) in y and z, both
    // of them, each a 1/2 shank 1-1/2 long under a 3/4 head.
    let bolts: Vec<&Body> = r
        .bodies
        .iter()
        .filter(|b| b.name.starts_with("hinge_bolt") && !cylinders(&b.solid, 0.25).is_empty())
        .collect();
    assert_eq!(bolts.len(), 2, "two bolts");
    let (po, pa) = cylinders(&bolts[0].solid, 0.25)[0];
    assert!(pa.x.abs() > 1.0 - 1e-9, "bolt along x: {pa:?}");
    assert!(
        (po.y).abs() < 1e-6 && (po.z + 1.5 * IN).abs() < 1e-6,
        "{po:?}"
    );
    for b in &bolts {
        let (o, a) = cylinders(&b.solid, 0.25)[0];
        assert!(a.x.abs() > 1.0 - 1e-9 && (o.y - po.y).abs() < 1e-6 && (o.z - po.z).abs() < 1e-6);
        assert_eq!(
            cylinders(&b.solid, 0.375).len(),
            2,
            "{}: the head and the nut",
            b.name
        );
        assert_eq!(b.material.as_ref().map(|m| m.name.as_str()), Some("steel"));
    }
    let rails = extent(&r, |b| b.name.contains("/ rail"));
    let (rl, rh) = (rails.0.x / IN, rails.1.x / IN);
    assert!((rl + 0.75).abs() < 1e-6 && (rh - (W + 0.75)).abs() < 1e-6);
    // The bands: four, 1/4 steel, 2 tall from 0.4 above the panel's
    // bottom, 11-3/4 long from the joint, six 11/32 holes along x; on the
    // left of the ramp A's left band is against the rail and B's turned
    // right band a bar further out, on the right the other way about.
    let mut bands: Vec<(String, f64, f64)> = Vec::new();
    for name in ["hinge_left 1", "hinge_right 1"] {
        for b in named(&r, name) {
            let (lo, hi) = bounds(&b.solid);
            assert!(
                (hi.x - lo.x - 0.25 * IN).abs() < 1e-6,
                "{}: 1/4 bar",
                b.name
            );
            assert!(
                (lo.z / IN - 0.4).abs() < 1e-6 && (hi.z / IN - 2.4).abs() < 1e-6,
                "{}: z {}..{}",
                b.name,
                lo.z / IN,
                hi.z / IN
            );
            assert!(
                (lo.y.abs().min(hi.y.abs())) < 1e-6 && ((hi.y - lo.y) / IN - 11.75).abs() < 1e-6,
                "{}: from the joint back 11-3/4",
                b.name
            );
            assert_eq!(
                cylinders(&b.solid, 0.34375 / 2.0).len(),
                6,
                "{}: six screw holes",
                b.name
            );
            assert_eq!(b.material.as_ref().map(|m| m.name.as_str()), Some("steel"));
            bands.push((b.name.clone(), lo.x / IN, hi.x / IN));
        }
    }
    bands.sort_by(|a, b| a.1.total_cmp(&b.1));
    let want_bands = [
        ("panel B / hinge_right 1", rl - 0.5),
        ("panel A / hinge_left 1", rl - 0.25),
        ("panel B / hinge_left 1", rh),
        ("panel A / hinge_right 1", rh + 0.25),
    ];
    for ((name, lo, _), (wname, wlo)) in bands.iter().zip(want_bands) {
        assert_eq!(name, wname);
        assert!((lo - wlo).abs() < 1e-6, "{name} at {lo}, not {wlo}");
    }
    // The struts: four, 1/4 steel, 2-1/2 wide, their 17/32 holes on the
    // bolts' axis within a hundredth of a millimetre, the rounded end
    // about it, four screw holes, the long faces at 25 degrees; a
    // pair's two struts touch on the bolt, A's left outside B's turned
    // right on the left of the ramp, and inside it on the right.
    let mut struts: Vec<(String, f64, f64)> = Vec::new();
    for name in ["hinge_left 2", "hinge_right 2"] {
        for b in named(&r, name) {
            let cs = cylinders(&b.solid, 0.53125 / 2.0);
            assert_eq!(cs.len(), 1, "{}: {} pivot holes", b.name, cs.len());
            let (o, a) = cs[0];
            assert!(a.x.abs() > 1.0 - 1e-9, "{}: {a:?}", b.name);
            let off = Vec3::new(0.0, o.y - po.y, o.z - po.z);
            assert!(off.length() < 0.01, "{}: {off:?} off the bolt axis", b.name);
            let (ro, _) = cylinders(&b.solid, 1.25)[0];
            assert!(
                (ro.y - po.y).abs() < 1e-6 && (ro.z - po.z).abs() < 1e-6,
                "{}: the round end about the bolt",
                b.name
            );
            assert_eq!(
                cylinders(&b.solid, 0.34375 / 2.0).len(),
                4,
                "{}: four screw holes",
                b.name
            );
            let sloped = b.solid.surfaces.iter().any(|sf| match sf {
                Surface::Plane { normal, .. } => {
                    normal.x.abs() < 1e-6
                        && ((normal.z.abs() - 25f64.to_radians().cos()).abs() < 1e-6)
                }
                _ => false,
            });
            assert!(sloped, "{}: a face at 25 degrees", b.name);
            let (lo, hi) = bounds(&b.solid);
            assert!(
                (hi.x - lo.x - 0.25 * IN).abs() < 1e-6,
                "{}: 1/4 bar",
                b.name
            );
            assert_eq!(b.material.as_ref().map(|m| m.name.as_str()), Some("steel"));
            struts.push((b.name.clone(), lo.x / IN, hi.x / IN));
        }
    }
    struts.sort_by(|a, b| a.1.total_cmp(&b.1));
    println!("struts across the bolts: {struts:?}");
    let want_struts = [
        ("panel A / hinge_left 2", rl - 0.5),
        ("panel B / hinge_right 2", rl - 0.25),
        ("panel A / hinge_right 2", rh),
        ("panel B / hinge_left 2", rh + 0.25),
    ];
    for ((name, lo, _), (wname, wlo)) in struts.iter().zip(want_struts) {
        assert_eq!(name, wname);
        assert!((lo - wlo).abs() < 1e-6, "{name} at {lo}, not {wlo}");
    }
    for pair in [&struts[0..2], &struts[2..4]] {
        let gap = pair[1].1 - pair[0].2;
        assert!(
            gap.abs() < 1e-6,
            "{} to {}: {gap}, not touching",
            pair[0].0,
            pair[1].0
        );
    }
    let over = struts[3].2 - struts[0].1;
    println!("over the hinge bars: {over:.3} in");
    assert!((over - (W + 2.5)).abs() < 1e-6, "{over}");
    // The packer under each right band's far end, in the strut's plane
    // against the rail, 2-1/2 long to the band's end.
    for b in named(&r, "hinge_right 3") {
        let (lo, hi) = bounds(&b.solid);
        assert!(
            (hi.x - lo.x - 0.25 * IN).abs() < 1e-6,
            "{}: 1/4 bar",
            b.name
        );
        assert!(
            ((hi.y - lo.y) / IN - 2.5).abs() < 1e-6
                && (lo.y.abs().max(hi.y.abs()) / IN - 11.75).abs() < 1e-6,
            "{}: y {}..{}",
            b.name,
            lo.y / IN,
            hi.y / IN
        );
        let strut = struts
            .iter()
            .find(|s| s.0 == b.name.replace("hinge_right 3", "hinge_right 2"))
            .unwrap();
        assert!(
            (lo.x / IN - strut.1).abs() < 1e-6,
            "{}: in the strut's plane",
            b.name
        );
    }
    // The screws: six a side, 5/16 x 3-1/8 along x from the outermost
    // bar at each one, through the band always and the strut at the
    // first four, into the rail (3/4), the edge rib (3/4) and the
    // two-ply hinge block (1-1/2) of their own panel, an inch or more
    // into the block and short of its far face.
    let mut screws = 0;
    let mut through_strut = 0;
    for b in r
        .bodies
        .iter()
        .filter(|b| b.name.contains("/ hinge_left ") || b.name.contains("/ hinge_right "))
    {
        if cylinders(&b.solid, 0.3125 / 2.0).is_empty() {
            continue;
        }
        let (lo, hi) = bounds(&b.solid);
        assert!(
            ((hi.x - lo.x) / IN - 3.125).abs() < 1e-6,
            "{}: 3-1/8 long",
            b.name
        );
        let panel = b.name.split(" / ").next().unwrap();
        let side = if b.name.contains("hinge_left") {
            "hinge_left"
        } else {
            "hinge_right"
        };
        let (cy, cz) = ((lo.y + hi.y) / 2.0, (lo.z + hi.z) / 2.0);
        let through = |member: &str| {
            r.bodies
                .iter()
                .filter(|m| m.name.starts_with(panel) && m.name.contains(member))
                .filter(|m| {
                    let (ml, mh) = bounds(&m.solid);
                    cy > ml.y && cy < mh.y && cz > ml.z && cz < mh.z && lo.x < mh.x && hi.x > ml.x
                })
                .count()
        };
        assert_eq!(
            through(&format!("/ {side} 1")),
            1,
            "{}: through its band",
            b.name
        );
        let strut_too = through(&format!("/ {side} 2")) == 1;
        if strut_too {
            through_strut += 1;
        }
        assert_eq!(
            strut_too,
            cy.abs() / IN < 9.0,
            "{}: {} the strut",
            b.name,
            if strut_too { "through" } else { "misses" }
        );
        assert_eq!(through("/ rail"), 1, "{}: through a rail", b.name);
        assert_eq!(through("/ rib"), 1, "{}: through an edge rib", b.name);
        assert_eq!(
            through("/ hinge_block"),
            2,
            "{}: through both blocks",
            b.name
        );
        let blocks = extent(&r, |m| {
            m.name.starts_with(panel) && m.name.contains("/ hinge_block") && {
                let (ml, mh) = bounds(&m.solid);
                cy > ml.y && cy < mh.y && lo.x < mh.x && hi.x > ml.x
            }
        });
        let (bite, short) = if lo.x < W / 2.0 * IN {
            (hi.x - blocks.0.x, blocks.1.x - hi.x)
        } else {
            (blocks.1.x - lo.x, lo.x - blocks.0.x)
        };
        let (bite, short) = (bite / IN, short / IN);
        assert!(
            bite > 1.0 - 1e-6 && short > 0.0,
            "{}: {bite} in into the block, {short} short of its far face",
            b.name
        );
        screws += 1;
    }
    assert_eq!(screws, 24, "six a side");
    assert_eq!(through_strut, 16, "four a side through the strut");
    // The bolt: head, washer, strut, strut, washer, nut along x, 1-1/2
    // of shank from under the head, the head a washer off the outer
    // strut.
    for b in &bolts {
        let (lo, hi) = bounds(&b.solid);
        assert!(
            ((hi.x - lo.x) / IN - (1.5 + 0.3125)).abs() < 1e-6,
            "{}: 1-1/2 under the head",
            b.name
        );
        assert_eq!(
            cylinders(&b.solid, 1.0625 / 2.0).len(),
            2,
            "{}: two washers",
            b.name
        );
        let outer = if lo.x < 0.0 { lo.x } else { hi.x } / IN;
        let strut = if lo.x < 0.0 { struts[0].1 } else { struts[3].2 };
        assert!(
            ((outer - strut).abs() - (0.09 + 0.3125)).abs() < 1e-6,
            "{}: the head {} off its strut at {}",
            b.name,
            outer,
            strut
        );
    }
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
    // Folded: bottoms 3 in apart (twice the pin drop), the package the
    // width over the rails, 45 long over them, two rail heights and the
    // gap thick.
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
        (w - (W + 1.5)).abs() < 1e-6 && (t - (2.0 * RAIL_H + 3.0)).abs() < 1e-6,
        "{w} x {t}"
    );
    // The plain rails run to 45; the stubs make the panel's 48.
    assert!((l - 45.0).abs() < 1e-6, "{l} over the rails");
    let panels = extent(&r, |b| b.name.starts_with("panel"));
    let ly = (panels.1.y - panels.0.y) / IN;
    println!("folded package over the panels: {ly:.3} in long");
    assert!(
        (ly - 49.25).abs() < 1e-6,
        "{ly}: 48 from the joint to the stubs' ends and the struts' round ends past it"
    );
    // Folded, the struts still pair: each side's two share the bolt
    // and touch nothing (the sweep above), the turned strut's round
    // end hanging where the open one's did.
    for b in r
        .bodies
        .iter()
        .filter(|b| b.name.ends_with("/ hinge_left 2") || b.name.ends_with("/ hinge_right 2"))
    {
        let cs = cylinders(&b.solid, 0.53125 / 2.0);
        assert_eq!(cs.len(), 1, "{}", b.name);
        let (o, _) = cs[0];
        assert!(
            o.y.abs() < 0.01 && (o.z + 1.5 * IN).abs() < 0.01,
            "{}: pivot at {o:?}",
            b.name
        );
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

/// Sheet yield: the four skins come out of two 4 x 8s of OSB ripped
/// along their length, the shims from the first's offcut, and the
/// rails, ribs, cross blocks and hinge blocks out of under half a 4 x 8
/// of birch, all read off the parts' faces.
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
    // OSB sheathing is sized for spacing, 95-7/8 x 47-7/8: a sheet
    // ripped 31-7/8 along its length (the strength axis, which runs
    // along the ramp) gives two 45 in skins with a kerf, and a 16 in
    // strip over.
    let sheet = 95.875 * 47.875;
    println!("skins: {skins:.0} in2, {:.2} OSB sheets", skins / sheet);
    let (lo, hi) = bounds(&named(&r, "skin_top")[0].solid);
    let (sw, sl) = ((hi.x - lo.x) / IN, (hi.y - lo.y) / IN);
    assert!(
        (sw - W).abs() < 1e-6 && (sl - 45.0).abs() < 1e-6,
        "{sw} x {sl} skins"
    );
    assert!(
        2.0 * sl + 0.125 <= 95.875 + 1e-9 && sw + 16.0 <= 47.875,
        "two to a sheet, 16 in over"
    );
    assert!(skins > sheet && skins < 2.0 * sheet, "{skins}");
    let side = |b: &Body| {
        let (lo, hi) = bounds(&b.solid);
        (hi.y - lo.y) * (hi.z - lo.z) / (IN * IN)
    };
    // The birch: rails, ribs, cross blocks and hinge blocks of both
    // panels, each read as its side area (it stands on edge).
    let rb = doc.regenerate_assembly(tab_named(&doc, "panel B")).unwrap();
    let is_birch = |b: &Body| {
        let n = b.name.rsplit(" / ").next().unwrap();
        n.contains("rail")
            || n.contains("rib")
            || n.contains("cross_block")
            || n.contains("hinge_block")
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
        ("cut sheet, OSB 1", 0.4375, 8, "7/16 OSB", (95.875, 47.875)),
        ("cut sheet, OSB 2", 0.4375, 2, "7/16 OSB", (95.875, 47.875)),
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
