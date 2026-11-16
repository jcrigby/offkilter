//! The indexed stick mill (examples/stick-mill) as a regression suite:
//! the shafts are held at both ends and the carriage never leaves
//! them; the pattern and the milled stick are the kernel's leaning
//! half-ellipse tile; the index plate has twelve stops; the carriage
//! passes over the headstock and the index plate stands clear of it;
//! and the pass plan covers the outline within what the bit can reach,
//! inside the router's plunge and the cross slide's travel.

use ok_brep::{BoolOp, Solid};
use ok_model::{Document, TabId};
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/stick-mill")
}

fn load() -> Document {
    Document::from_json(&std::fs::read_to_string(dir().join("out/mill.okpart")).unwrap()).unwrap()
}

fn numbers() -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(dir().join("out/mill.json")).unwrap()).unwrap()
}

fn tab(doc: &Document, name: &str) -> TabId {
    doc.tabs
        .iter()
        .find(|t| t.name() == name)
        .unwrap_or_else(|| panic!("no tab {name}"))
        .id
}

/// A tab's bodies, regenerated.
fn bodies_of(doc: &mut Document, name: &str) -> Vec<Solid> {
    let r = doc.regenerate_studio(tab(doc, name), None).unwrap();
    assert!(
        r.statuses.iter().all(|f| f.error.is_none()),
        "{:?}",
        r.statuses
    );
    r.bodies
        .iter()
        .map(|b| {
            b.solid.validate().unwrap();
            b.solid.clone()
        })
        .collect()
}

fn one(doc: &mut Document, name: &str) -> Solid {
    let mut b = bodies_of(doc, name);
    assert_eq!(b.len(), 1, "one body in {name}");
    b.remove(0)
}

fn f(v: &serde_json::Value) -> f64 {
    v.as_f64().unwrap()
}

fn overlap(a: &Solid, b: &Solid) -> f64 {
    ok_brep::boolean(a, b, BoolOp::Intersection)
        .map(|s| s.volume().abs())
        .unwrap_or(0.0)
}

/// Two shafts, each through a support at each end, and the carriage's
/// blocks between the supports wherever the bit goes.
#[test]
fn the_shafts_are_held_at_both_ends_and_the_carriage_stays_between() {
    let (mut doc, n) = (load(), numbers());
    let shafts = bodies_of(&mut doc, "Shafts");
    let supports = bodies_of(&mut doc, "Shaft supports SK20");
    let blocks = bodies_of(&mut doc, "Blocks SC20UU");
    assert_eq!((shafts.len(), supports.len(), blocks.len()), (2, 4, 4));
    let (x0, x1) = (f(&n["supports_x"][0]), f(&n["supports_x"][1]));
    let half = f(&n["sk_t"]) / 2.0;
    for s in &shafts {
        let (lo, hi) = s.bounds().unwrap();
        assert!(
            lo.x < x0 - half + 1e-6 && hi.x > x1 + half - 1e-6,
            "a shaft runs through both its supports"
        );
        assert!((hi.y - lo.y - f(&n["shaft"]["d"])).abs() < 1e-6);
        // In the bores, not through the supports' metal.
        assert!(supports.iter().map(|k| overlap(s, k)).sum::<f64>() < 1.0);
        assert!(blocks.iter().map(|k| overlap(s, k)).sum::<f64>() < 1.0);
    }
    let reach = f(&n["carriage"]["block_x"]) + f(&n["carriage"]["blk_l"]) / 2.0;
    let (b0, b1) = (f(&n["bit_x"][0]), f(&n["bit_x"][1]));
    println!(
        "blocks from {:.0} to {:.0}, supports' faces at {:.0} and {:.0}",
        b0 - reach,
        b1 + reach,
        x0 + half,
        x1 - half
    );
    assert!(b0 - reach > x0 + half && b1 + reach < x1 - half);
    assert!(
        b0 < 0.0 && b0 > -f(&n["pattern_t"]),
        "the bit sets over the pattern"
    );
    assert!(b1 > f(&n["stick"]), "and runs off the stick's far end");
}

/// The pattern and the stick are the tile's section; the index plate
/// is a disc with the spindle's hole and twelve stops.
#[test]
fn the_pattern_is_the_tile_and_the_plate_has_twelve_stops() {
    let (mut doc, n) = (load(), numbers());
    let area = f(&n["tile"]["area"]);
    let outline = n["tile"]["outline"].as_array().unwrap();
    let shoelace: f64 = (0..outline.len())
        .map(|i| {
            let (a, b) = (&outline[i], &outline[(i + 1) % outline.len()]);
            f(&a[0]) * f(&b[1]) - f(&b[0]) * f(&a[1])
        })
        .sum::<f64>()
        .abs()
        / 2.0;
    assert!((shoelace - area).abs() < 0.01);
    let stick = one(&mut doc, "Stick");
    assert!((stick.volume() - area * f(&n["stick"])).abs() < 1.0);
    let pattern = one(&mut doc, "Pattern");
    let i = &n["index"];
    let bore = std::f64::consts::PI * ((f(&i["spindle"]) + 0.2) / 2.0).powi(2);
    println!(
        "pattern {:.0} mm3, the tile's section gives {:.0}",
        pattern.volume(),
        (area - bore) * f(&n["pattern_t"])
    );
    assert!((pattern.volume() - (area - bore) * f(&n["pattern_t"])).abs() < 6.0);
    let plate = one(&mut doc, "Index plate");
    let pi = std::f64::consts::PI;
    let stops = n["stops"].as_u64().unwrap() as f64;
    let expect = pi / 4.0
        * (f(&i["d"]).powi(2)
            - (f(&i["spindle"]) + 0.4).powi(2)
            - stops * (f(&i["pin"]) + 0.2).powi(2))
        * f(&i["t"]);
    println!(
        "index plate {:.0} mm3, a disc less its {stops} stops gives {expect:.0}",
        plate.volume()
    );
    assert_eq!(stops, 12.0);
    // The kernel's circles are many-sided: a 120 mm disc comes out a tenth of a percent small.
    assert!((plate.volume() - expect).abs() < 0.003 * expect);
    assert!(dir().join("out/pattern.stl").exists() && dir().join("out/index_plate.stl").exists());
}

/// Drawn with the carriage over the pattern: the bit stands on it, the
/// carriage clears the headstock, and the index plate is out of reach.
#[test]
fn the_carriage_passes_over_the_headstock_and_the_bit_stands_on_the_pattern() {
    let (mut doc, n) = (load(), numbers());
    let pattern = one(&mut doc, "Pattern");
    let bit = one(&mut doc, "Bit");
    let bite = overlap(&bit, &pattern);
    println!("bit into the pattern: {bite:.3} mm3");
    // The bit is drawn flat-ended, so its rim dips into a curved top by a fraction of a cubic millimetre.
    assert!(bite < 1.0);
    let (lo, _) = bit.bounds().unwrap();
    assert!((lo.z - f(&n["tip_drawn"])).abs() < 1e-6);
    assert!(lo.z - f(&n["axis_z"]) <= f(&n["tile"]["swing"]) + 1e-6);
    let head = one(&mut doc, "Headstock");
    let plate = one(&mut doc, "Index plate");
    let slide = one(&mut doc, "Cross slide");
    assert!(overlap(&slide, &head) < 1e-6 && overlap(&slide, &plate) < 1e-6);
    for g in bodies_of(&mut doc, "Guides")
        .iter()
        .chain(bodies_of(&mut doc, "Bolsters").iter())
    {
        assert!(
            overlap(g, &head) < 1e-6,
            "the carriage clears the headstock"
        );
        assert!(overlap(g, &plate) < 1e-6, "and the index plate");
        assert!(overlap(g, &pattern) < 1e-6);
    }
    // A blank sawn within 2 mm of the tile swings under the cross slide.
    assert!(f(&n["tile"]["swing"]) + 2.0 < f(&n["slide"]["z"]) - f(&n["axis_z"]));
    assert!(f(&n["upright_top"]) < f(&n["slide"]["z"]));
}

/// The plan: a few hundred passes over all twelve stops, the outline
/// nearly all within a ridge of one, and every pass inside the
/// router's reach and plunge and the cross slide's travel.
#[test]
fn the_pass_plan_covers_the_outline_within_the_machine() {
    let n = numbers();
    let p = &n["plan"];
    let passes = p["list"].as_array().unwrap();
    assert_eq!(passes.len() as u64, p["passes"].as_u64().unwrap());
    let per: Vec<u64> = p["per_stop"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    println!(
        "{} passes, per stop {per:?}; {:.1} % within a ridge, the worst {:.2} mm",
        passes.len(),
        100.0 * f(&p["covered"]),
        f(&p["worst"])
    );
    assert!(passes.len() > 150 && passes.len() < 450);
    assert_eq!(per.len(), 12);
    assert!(per.iter().all(|c| *c >= 8));
    assert!(f(&p["covered"]) > 0.94);
    assert!(
        f(&p["worst"]) < 0.4,
        "nothing is left more than a few tenths proud"
    );
    let (low, high) = (f(&p["tip_low"]), f(&p["tip_high"]));
    let sole = f(&n["slide"]["z"]) + f(&n["slide"]["t"]);
    let need = sole - (f(&n["axis_z"]) + low);
    println!("the tip from {low:.1} to {high:.1} off the axis: {need:.1} mm under the base's sole at the deepest");
    assert!(need <= f(&n["reach"]) + 1e-9);
    assert!(high - low + 6.0 <= f(&n["plunge"]));
    let mark = f(&n["mark"]);
    for q in passes {
        let y = f(&q["across"]);
        assert!(y.abs() <= f(&n["slide"]["travel"]));
        assert!(
            ((y / mark).round() * mark - y).abs() < 1e-6,
            "every pass is on a mark of the handwheel"
        );
        assert!(f(&q["tip"]) <= f(&n["tile"]["swing"]) + 1e-6);
    }
}
