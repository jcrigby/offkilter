//! The indexed stick mill (examples/stick-mill) as a regression suite:
//! the shafts are held at both ends and the carriage never leaves
//! them; the pattern and the milled stick are the kernel's leaning
//! half-ellipse tile; a tapered twelve-pointed star is locked at twelve
//! stops by one cradle that slides onto it along the axis, and with the
//! cradle back the stick turns in its bearings for two round journals;
//! the axis is as low as the star lets it be; the carriage's shelf passes clear of
//! the headstock, the star and the wall;
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
    // The wall's braces stand in front of it, outside the shafts' ends: nothing of the wall is in a shaft or a support.
    let wall = bodies_of(&mut doc, "Back wall");
    assert_eq!(wall.len(), 1);
    for part in shafts.iter().chain(supports.iter()).chain(blocks.iter()) {
        assert!(overlap(&wall[0], part) < 1e-6);
    }
    let reach = f(&n["carriage"]["block_x"]) + f(&n["carriage"]["blk_l"]) / 2.0;
    let (b0, b1) = (f(&n["bit_x"][0]), f(&n["bit_x"][1]));
    // The carriage goes a little further than the pattern, to set the straight bit on the journal's master.
    let far = f(&n["car_min"]);
    assert!(far < b0 && far > -f(&n["pattern_t"]) - f(&n["journal"]["master_t"]));
    println!(
        "blocks from {:.0} to {:.0}, supports' faces at {:.0} and {:.0}",
        b0 - reach,
        b1 + reach,
        x0 + half,
        x1 - half
    );
    assert!(far - reach > x0 + half && b1 + reach < x1 - half);
    assert!(
        b0 < 0.0 && b0 > -f(&n["pattern_t"]),
        "the bit sets over the pattern"
    );
    assert!(b1 > f(&n["stick"]), "and runs off the stick's far end");
}

/// The pattern and the stick are the tile's section; the star and the
/// one cradle that slides onto it give twelve stops.
#[test]
fn the_pattern_is_the_tile_and_one_sliding_cradle_locks_the_star() {
    let (mut doc, n) = (load(), numbers());
    let area = f(&n["tile"]["area"]);
    let outline = n["tile"]["outline"].as_array().unwrap();
    let shoelace = |pts: &[serde_json::Value]| -> f64 {
        (0..pts.len())
            .map(|i| {
                let (a, b) = (&pts[i], &pts[(i + 1) % pts.len()]);
                f(&a[0]) * f(&b[1]) - f(&b[0]) * f(&a[1])
            })
            .sum::<f64>()
            .abs()
            / 2.0
    };
    assert!((shoelace(outline) - area).abs() < 0.01);
    let pi = std::f64::consts::PI;
    // The stick is the tile's section but for two round journals about the axis.
    let j = &n["journal"];
    let (radius, jl) = (f(&j["radius"]), f(&j["length"]));
    let round = pi * radius * radius;
    let stick = one(&mut doc, "Stick");
    let expect = area * (f(&n["stick"]) - 2.0 * jl) + round * 2.0 * jl;
    println!(
        "stick {:.0} mm3, tile and two journals give {expect:.0}",
        stick.volume()
    );
    assert!((stick.volume() - expect).abs() < 0.002 * expect);
    let pattern = one(&mut doc, "Pattern");
    let h = &n["star"];
    let bore = pi * ((f(&h["spindle"]) + 0.2) / 2.0).powi(2);
    let expect = (area - bore) * f(&n["pattern_t"]) + (round - bore) * f(&j["master_t"]);
    println!(
        "pattern {:.0} mm3, the tile's section and the journal's master give {expect:.0}",
        pattern.volume()
    );
    assert!((pattern.volume() - expect).abs() < 0.002 * expect);
    // The star: a hexagon and the same hexagon turned 30°, its points blunted, tapering along the axis.
    let (af, r, inside, t) = (f(&h["af"]), f(&h["r"]), f(&h["inside"]), f(&h["t"]));
    assert!((r - af / 3f64.sqrt()).abs() < 1e-9);
    assert!((inside - af / 2.0 / 15f64.to_radians().cos()).abs() < 1e-9);
    let (taper, k) = (f(&h["taper"]), f(&h["k"]));
    assert!((k - (1.0 - t * taper.to_radians().tan() / r)).abs() < 1e-9);
    assert!(
        (7.0..=15.0).contains(&taper),
        "steep enough to let go, shallow enough to seat"
    );
    let section = h["section"].as_array().unwrap();
    let cutter = h["cutter"].as_array().unwrap();
    assert_eq!((section.len(), cutter.len()), (36, 36));
    let star = one(&mut doc, "Star");
    // A frustum of similar sections: the mean of the ends' areas and their geometric mean.
    let expect = (shoelace(section) * (1.0 + k + k * k) / 3.0 - bore) * t;
    println!(
        "star {:.0} mm3, its section tapering to {k:.3} gives {expect:.0}",
        star.volume()
    );
    assert!((star.volume() - expect).abs() < 0.002 * expect);
    assert_eq!(n["stops"].as_u64(), Some(12));
    // Only flanks meet: the star's points stop short of the pocket's, the pocket's crests short of the star's corners.
    let far = |pts: &[serde_json::Value]| {
        pts.iter()
            .map(|q| f(&q[0]).hypot(f(&q[1])))
            .fold(0.0, f64::max)
    };
    let near = |pts: &[serde_json::Value]| {
        pts.iter()
            .map(|q| f(&q[0]).hypot(f(&q[1])))
            .fold(f64::MAX, f64::min)
    };
    println!(
        "points {:.2} in a pocket of {:.2}; corners {:.2} over crests of {:.2}",
        far(section),
        far(cutter),
        near(section),
        near(cutter)
    );
    assert!((far(cutter) - r).abs() < 1e-9 && far(section) < r - 0.3);
    assert!((near(section) - inside).abs() < 1e-9 && near(cutter) > inside + 0.2);
    // The cradle, drawn home: on the guide's floor between its lips, the star in its pocket and not through it.
    let cradle = one(&mut doc, "Cradle");
    let guide = one(&mut doc, "Cradle guide");
    let g = &h["guide"];
    let (floor, lip, play) = (f(&g["floor"]), f(&g["lip"]), f(&g["play"]));
    let seated = overlap(&star, &cradle);
    println!("star in its cradle: {seated:.3} mm3 through it");
    assert!(seated < 1.0);
    assert!(overlap(&cradle, &guide) < 1e-6);
    let axis = f(&n["axis_z"]);
    let (lo, hi) = cradle.bounds().unwrap();
    assert!(
        (lo.z - floor).abs() < 1e-6,
        "the cradle stands on the guide's floor"
    );
    assert!((hi.z - f(&h["cradle_top"])).abs() < 1e-6 && hi.z < axis);
    assert!((hi.y - lo.y - f(&h["cradle_w"])).abs() < 1e-6 && (hi.x - lo.x - t).abs() < 1e-6);
    let (slo, shi) = star.bounds().unwrap();
    assert!((slo.x - lo.x).abs() < 1e-6 && (shi.x - hi.x).abs() < 1e-6);
    // The axis is as low as the star lets it be: a few millimetres of cradle under the point, on the guide's floor.
    let under = axis - r - floor;
    println!("axis {axis:.0} over the base, {under:.1} of cradle under the star's point");
    assert!((4.0..10.0).contains(&under));
    // Its way off: clear of the star with room to spare, still between the lips, short of the end wall.
    let travel = f(&h["travel"]);
    let (glo, ghi) = guide.bounds().unwrap();
    assert!(travel >= t + 5.0, "the cradle comes right off the star");
    assert!(glo.x <= lo.x && hi.x + travel <= ghi.x - f(&g["end"][0]) + 1e-6);
    assert!(glo.z.abs() < 1e-6 && lip >= 5.0 && play > 0.05 && play < 0.3);
    assert!(
        (ghi.y - glo.y) > f(&h["cradle_w"]) + 2.0 * (play + f(&g["wall"])),
        "the guide has ears for its screws"
    );
    // The push screw is home on the cradle's face, and through the end wall's hole, not its plastic.
    let screw = one(&mut doc, "Push screw");
    let (plo, _) = screw.bounds().unwrap();
    assert!((plo.x - hi.x).abs() < 1e-6);
    assert!(overlap(&screw, &guide) < 1e-6 && overlap(&screw, &cradle) < 1e-6);
    // The spindle runs in a bearing in each face of the headstock.
    let bearings = bodies_of(&mut doc, "Bearings 6201");
    assert_eq!(bearings.len(), 2);
    let spindle = &bodies_of(&mut doc, "Spindle and centre")[0];
    let (plo, phi) = spindle.bounds().unwrap();
    for b in &bearings {
        let (blo, bhi) = b.bounds().unwrap();
        assert!(plo.x < blo.x && phi.x > bhi.x);
        assert!(((blo.z + bhi.z) / 2.0 - axis).abs() < 1e-6);
        assert!(overlap(b, spindle) < 1e-6);
    }
    for name in ["pattern", "star", "cradle", "cradle_guide", "post"] {
        assert!(dir().join(format!("out/{name}.stl")).exists(), "{name}.stl");
    }
}

/// Drawn with the carriage over the pattern: the bit stands on it, and
/// the square, its shelf and the cross slide clear everything fixed.
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
    let star = one(&mut doc, "Star");
    let cradle = one(&mut doc, "Cradle");
    let wall = one(&mut doc, "Back wall");
    for name in ["Cross slide", "Shelf", "Carriage square"] {
        let part = one(&mut doc, name);
        for (other, what) in [
            (&head, "headstock"),
            (&star, "star"),
            (&cradle, "cradle"),
            (&pattern, "pattern"),
            (&wall, "back wall"),
        ] {
            assert!(overlap(&part, other) < 1e-6, "{name} is in the {what}");
        }
    }
    // A blank sawn within 2 mm of the tile swings under the cross slide.
    assert!(f(&n["tile"]["swing"]) + 2.0 < f(&n["slide"]["z"]) - f(&n["axis_z"]));
    assert!(f(&n["upright_top"]) < f(&n["slide"]["z"]));
}

/// The journals: the axis is where the tile is fattest, the journal a
/// round inside the tile there, so no pass of the plan touches it; a V
/// post under each, and no pass comes near a post. They are turned:
/// the crank's turns take the blank down in bites.
#[test]
fn the_journals_are_inside_the_tile_and_the_posts_clear_of_every_pass() {
    let (mut doc, n) = (load(), numbers());
    let j = &n["journal"];
    let (radius, circle) = (f(&j["radius"]), f(&n["tile"]["circle"]));
    println!(
        "a circle of {circle:.2} about the axis fits in the tile; the journal {:.1} round",
        2.0 * radius
    );
    assert!(circle > 9.5, "the axis is at the tile's fattest place");
    assert!(radius < circle - 0.05 && radius > circle - 0.2);
    // The circle is the tile's: no point of the outline is nearer the axis.
    let near = n["tile"]["outline"]
        .as_array()
        .unwrap()
        .iter()
        .map(|q| f(&q[0]).hypot(f(&q[1])))
        .fold(f64::MAX, f64::min);
    assert!(near >= circle - 1e-9);
    // No pass's ball reaches the journal, nor comes within the clearance of a post.
    let r = f(&n["bit"]) / 2.0;
    let post = &j["post"];
    let (half, top) = (f(&post["half"]), f(&post["top"]));
    let mut least = f64::MAX;
    for q in n["plan"]["list"].as_array().unwrap() {
        let (y, tip) = (f(&q["across"]), f(&q["tip"]));
        assert!(
            y.hypot(tip + r) >= radius + r - 1e-6,
            "a pass cuts a journal"
        );
        if y.abs() < half + r {
            let d = (y.abs() - half).max(0.0);
            least = least.min(tip + r - (r * r - d * d).sqrt() - top);
        }
    }
    println!(
        "posts {:.1} wide, their lips {:.1} under the axis; the bit never nearer than {least:.2}",
        2.0 * half,
        -top
    );
    assert!(least >= f(&post["least"]));
    assert!((least - f(&post["clear"])).abs() < 0.05);
    // The V: a right angle whose walls touch the journal, its lips above where they touch.
    let seat: Vec<(f64, f64)> = post["seat"]
        .as_array()
        .unwrap()
        .iter()
        .map(|q| (f(&q[0]), f(&q[1])))
        .collect();
    assert_eq!(seat.len(), 3);
    let (lip, foot) = (seat[0], seat[1]);
    let wall = (lip.0 - foot.0).hypot(lip.1 - foot.1);
    let off = (lip.0 * foot.1 - foot.0 * lip.1).abs() / wall;
    assert!(
        (off - radius).abs() < 1e-9,
        "the wall is tangent to the journal"
    );
    assert!(((lip.0 - foot.0) - (lip.1 - foot.1)).abs() < 1e-9 && foot.0.abs() < 1e-9);
    assert!(top > -radius * 0.5f64.sqrt() + 1.0 && top < 0.0);
    // The posts: one under each journal, inside its length, not through the stick.
    let stick = one(&mut doc, "Stick");
    let axis = f(&n["axis_z"]);
    for (i, name) in ["Post 1", "Post 2"].iter().enumerate() {
        let p = one(&mut doc, name);
        let through = overlap(&p, &stick);
        assert!(through < 1.0, "{name} is {through:.2} mm3 into the stick");
        let (lo, hi) = p.bounds().unwrap();
        let at = f(&j["at"][i]);
        assert!(lo.z.abs() < 1e-6 && (hi.z - (axis + top)).abs() < 1e-6);
        assert!(f(&post["t"]) + 3.0 <= f(&j["length"]));
        assert!(lo.x < at && hi.x > at);
    }
    for name in ["Shelf", "Carriage square", "Cross slide"] {
        let part = one(&mut doc, name);
        let (lo, _) = part.bounds().unwrap();
        assert!(
            lo.z > axis + top || lo.y > half + 30.0,
            "{name} passes over the posts"
        );
    }
    // Turning a journal: bites from the blank's swing to the journal, at places that cover its length.
    let (bites, places) = (j["bites"].as_u64().unwrap(), j["places"].as_u64().unwrap());
    let bit = f(&j["bit"]);
    println!(
        "a journal: {bites} bites at {places} places, {} turns of the crank",
        j["turns"]
    );
    assert_eq!(j["turns"].as_u64().unwrap(), bites * places);
    assert!(bites as f64 * f(&n["rough"]["bite"]) >= f(&j["blank_swing"]) - radius);
    assert!(places as f64 * (bit - 1.0) >= f(&j["length"]));
    assert!(dir().join("out/post.stl").exists());
}

/// Roughing: the straight bit goes down at each place until it would
/// touch the pattern, less an allowance, so it never cuts the tile; it
/// and the ball-nose after it take the waste in bites within their
/// limits, and most of the waste goes to the straight bit.
#[test]
fn roughing_takes_most_of_the_waste_in_bites_and_never_reaches_the_tile() {
    let n = numbers();
    let g = &n["rough"];
    let outline: Vec<(f64, f64)> = n["tile"]["outline"]
        .as_array()
        .unwrap()
        .iter()
        .map(|q| (f(&q[0]), f(&q[1])))
        .collect();
    let (r, allow, bite) = (f(&g["bit"]) / 2.0, f(&g["allow"]), f(&g["bite"]));
    let (mark, travel) = (f(&n["mark"]), f(&n["slide"]["travel"]));
    let places = g["list"].as_array().unwrap();
    assert_eq!(places.len() as u64, g["places"].as_u64().unwrap());
    let mut runs = 0;
    let (mut low, mut high) = (f64::MAX, f64::MIN);
    for q in places {
        let (k, y, tip) = (q["stop"].as_u64().unwrap(), f(&q["across"]), f(&q["tip"]));
        let t = std::f64::consts::TAU * k as f64 / 12.0;
        // The tile under the bit at this stop: nothing of it stands over the bit's end less the allowance.
        let under = outline
            .iter()
            .map(|p| {
                (
                    -p.0 * t.sin() + p.1 * t.cos(),
                    p.0 * t.cos() + p.1 * t.sin(),
                )
            })
            .filter(|(u, _)| (u - y).abs() <= r)
            .map(|(_, h)| h)
            .fold(f64::MIN, f64::max);
        assert!(
            under <= tip - allow + 1e-3,
            "a roughing bite reaches the tile"
        );
        assert!((f(&q["touch"]) + allow - tip).abs() < 1e-9);
        let (depth, bites) = (f(&q["depth"]), q["bites"].as_u64().unwrap());
        assert!(bites >= 1 && depth / bites as f64 <= bite + 1e-6);
        assert!(y.abs() <= travel && ((y / mark).round() * mark - y).abs() < 1e-6);
        runs += bites;
        low = low.min(tip);
        high = high.max(tip + depth);
    }
    assert_eq!(runs, g["runs"].as_u64().unwrap());
    let (waste, rough, left, end) = (
        f(&g["waste"]),
        f(&g["rough_area"]),
        f(&g["left_after_rough"]),
        f(&g["left_at_end"]),
    );
    println!(
        "waste {waste:.0} mm2: {} places and {runs} runs of the straight bit take {rough:.0}, the ball-nose {:.0}, {end:.1} stands",
        places.len(),
        left - end
    );
    assert!((rough + left - waste).abs() < 1e-6);
    assert!(
        rough > 0.7 * waste,
        "the straight bit takes most of the waste"
    );
    assert!(end < 0.01 * waste, "all but the corners' ridges is cut");
    // The blank: the tile's hull and a little over, so the waste is the hull's area less the tile's and a band round it.
    let hull: Vec<(f64, f64)> = g["hull"]
        .as_array()
        .unwrap()
        .iter()
        .map(|q| (f(&q[0]), f(&q[1])))
        .collect();
    let (mut area, mut around) = (0.0, 0.0);
    for i in 0..hull.len() {
        let (a, b) = (hull[i], hull[(i + 1) % hull.len()]);
        area += (a.0 * b.1 - b.0 * a.1) / 2.0;
        around += (a.0 - b.0).hypot(a.1 - b.1);
    }
    let over = f(&g["blank"]);
    let expect =
        area.abs() + around * over + std::f64::consts::PI * over * over - f(&n["tile"]["area"]);
    println!("the blank less the tile is {expect:.0} mm2");
    assert!((waste - expect).abs() < 0.02 * expect);
    // The ball-nose after it: every pass in bites within its limit, and far fewer deep ones than passes.
    let ball = f(&g["ball_bite"]);
    let passes = n["plan"]["list"].as_array().unwrap();
    let mut finish = 0;
    for q in passes {
        let (depth, bites) = (f(&q["depth"]), q["bites"].as_u64().unwrap());
        assert!(bites >= 1 && depth / bites as f64 <= ball + 1e-6);
        finish += bites;
    }
    assert_eq!(finish, g["finish_runs"].as_u64().unwrap());
    let deep = passes
        .iter()
        .filter(|q| q["bites"].as_u64().unwrap() > 1)
        .count();
    println!(
        "{finish} runs of the ball-nose over {} passes, {deep} of them in more than one bite",
        passes.len()
    );
    assert!(deep * 4 < passes.len());
    // Inside the machine: the straight bit's range within the plunge, and clear of the posts.
    assert!(high - low + 6.0 <= f(&n["plunge"]));
    assert!(high <= f(&g["blank"]) + f(&n["tile"]["swing"]) + 1e-6);
    assert!(f(&g["over_post"]) >= f(&n["journal"]["post"]["least"]));
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
