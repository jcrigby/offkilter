//! The billet (examples/stick-billet): the stick sled's tile milled
//! from a stack of 3 mm pieces held on rods, in two stages. Checked
//! from the committed document and numbers against the sled's own
//! tile and table: the stack is the sled's stick, its pieces all inside
//! what the bit finishes; a piece is the grown blank with four holes,
//! all outside the tile; no first-stage cut comes near a rod nor into
//! the tab of wood that joins it to the tile, so every piece stays on
//! its rods; the comb lies in what the first stage cut away and no
//! second-stage cut comes near it; every finishing pass of the sled's
//! table is made once, in one stage or the other; the far plate's boss
//! is the sled's own blank, so the sled's socket takes it; the bits
//! stand out of the collet nut within what the sled allows; and the
//! animation has a frame for every pass and a still for every step.

use ok_brep::{BoolOp, Solid};
use ok_math::{Plane, Vec2};
use ok_model::{Document, TabId};
use ok_sketch::{Loop, Profile};
use serde_json::Value;
use std::path::PathBuf;

type P2 = (f64, f64);

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/stick-billet")
}

fn load() -> Document {
    Document::from_json(&std::fs::read_to_string(dir().join("out/billet.okpart")).unwrap()).unwrap()
}

fn json(path: PathBuf) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn numbers() -> Value {
    json(dir().join("out/billet.json"))
}

fn sled() -> Value {
    json(dir().join("../stick-sled/out/sled.json"))
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap()
}

fn pair(v: &Value) -> P2 {
    (f(&v[0]), f(&v[1]))
}

fn points(v: &Value) -> Vec<P2> {
    v.as_array().unwrap().iter().map(pair).collect()
}

fn tab(doc: &Document, name: &str) -> TabId {
    doc.tabs
        .iter()
        .find(|t| t.name() == name)
        .unwrap_or_else(|| panic!("no tab {name}"))
        .id
}

fn bodies_of(doc: &mut Document, name: &str) -> Vec<Solid> {
    let r = doc.regenerate_studio(tab(doc, name), None).unwrap();
    assert!(
        r.statuses.iter().all(|f| f.error.is_none()),
        "{name}: {:?}",
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

fn area2(p: &[P2]) -> f64 {
    (0..p.len())
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum()
}

/// A section pushed along x, the stick's axis, from `lo` to `hi`.
fn prism(section: &[P2], lo: f64, hi: f64) -> Solid {
    let mut p: Vec<Vec2> = section.iter().map(|q| Vec2::new(q.0, q.1)).collect();
    if area2(section) < 0.0 {
        p.reverse();
    }
    let profile = Profile {
        outer: Loop::polygon(p),
        holes: Vec::new(),
    };
    ok_brep::extrude(&profile, &Plane::YZ, lo, hi, 1).unwrap()
}

/// What two solids share. A boolean that fails is a failure here, not nothing shared.
fn common(a: &Solid, b: &Solid) -> f64 {
    match ok_brep::boolean(a, b, BoolOp::Intersection) {
        Ok(s) => s.volume().abs(),
        Err(e) => {
            let msg = format!("{e:?}");
            assert!(
                msg.to_lowercase().contains("empty"),
                "the boolean failed: {msg}"
            );
            0.0
        }
    }
}

fn inside(q: P2, poly: &[P2]) -> bool {
    let mut hit = false;
    for i in 0..poly.len() {
        let (a, b) = (poly[(i + poly.len() - 1) % poly.len()], poly[i]);
        if (a.1 > q.1) != (b.1 > q.1) && q.0 < a.0 + (b.0 - a.0) * (q.1 - a.1) / (b.1 - a.1) {
            hit = !hit;
        }
    }
    hit
}

fn seg_dist(q: P2, a: P2, b: P2) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let t = (((q.0 - a.0) * dx + (q.1 - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    ((q.0 - a.0 - t * dx).powi(2) + (q.1 - a.1 - t * dy).powi(2)).sqrt()
}

/// How far q is from the tile's edge, less than nothing inside it.
fn off_edge(q: P2, tile: &[P2]) -> f64 {
    let m = (0..tile.len())
        .map(|i| seg_dist(q, tile[(i + tile.len() - 1) % tile.len()], tile[i]))
        .fold(f64::MAX, f64::min);
    if inside(q, tile) {
        -m
    } else {
        m
    }
}

/// A cut as the billet's tables have it: the bit, the direction the stick is turned to, its axis across, its tip.
struct Cut {
    ball: bool,
    t: f64,
    across: f64,
    tip: f64,
}

fn cuts(v: &Value) -> Vec<Cut> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|c| Cut {
            ball: c[0].as_str().unwrap() == "b",
            t: f(&c[1]),
            across: f(&c[2]),
            tip: f(&c[3]),
        })
        .collect()
}

/// The bits, written again here from the sled's numbers so the plan is checked by other code than made it.
struct Bits {
    ball: f64,
    shank: f64,
    taper: f64,
    straight: f64,
}

impl Bits {
    fn new(n: &Value) -> Bits {
        Bits {
            ball: f(&n["bit"]) / 2.0,
            shank: f(&n["shank"]) / 2.0,
            taper: f(&n["taper"]).to_radians(),
            straight: f(&n["rough"]["bit"]) / 2.0,
        }
    }

    /// Whether q is in the bit of cut c, the bit grown by `grow` (its corners taken square).
    fn holds(&self, c: &Cut, q: P2, grow: f64) -> bool {
        let (d, e) = ((c.t.cos(), c.t.sin()), (-c.t.sin(), c.t.cos()));
        let off = (q.0 * e.0 + q.1 * e.1 - c.across).abs();
        let h = q.0 * d.0 + q.1 * d.1;
        if !c.ball {
            return off <= self.straight + grow && h >= c.tip - grow;
        }
        if off >= self.shank + grow {
            return false;
        }
        let o = (off - grow).clamp(0.0, self.shank - 1e-9);
        let fl = if o <= self.ball * self.taper.cos() {
            -(self.ball * self.ball - o * o).sqrt()
        } else {
            (o * self.taper.cos() - self.ball) / self.taper.sin()
        };
        h >= c.tip + self.ball + fl - grow
    }
}

fn ring(c: P2, r: f64, n: usize) -> Vec<P2> {
    (0..n)
        .map(|k| {
            let a = std::f64::consts::TAU * k as f64 / n as f64;
            (c.0 + r * a.cos(), c.1 + r * a.sin())
        })
        .chain(std::iter::once(c))
        .collect()
}

#[test]
fn the_stack_is_the_sled_s_stick_and_every_piece_is_inside_what_the_bit_finishes() {
    let (b, n) = (numbers(), sled());
    let l = &b["layout"];
    let (thick, count) = (
        f(&b["piece"]["thick"]),
        b["piece"]["count"].as_u64().unwrap() as f64,
    );
    let (cap, pieces, spacer, plate, boss) = (
        pair(&l["near_cap"]),
        pair(&l["pieces"]),
        pair(&l["spacer"]),
        pair(&l["far_plate"]),
        pair(&l["boss"]),
    );
    println!(
        "cap {cap:?}, {count} pieces {pieces:?}, spacer {spacer:?}, plate {plate:?}, boss {boss:?}"
    );
    // end to end, nothing between, and as long as the sled's stick, the boss as deep in its socket as the blank was
    assert!(cap.0.abs() < 1e-9 && (cap.1 - pieces.0).abs() < 1e-9);
    assert!((pieces.1 - pieces.0 - count * thick).abs() < 1e-9);
    assert!((spacer.0 - pieces.1).abs() < 1e-9 && (spacer.1 - spacer.0 - thick).abs() < 1e-9);
    assert!((plate.0 - spacer.1).abs() < 1e-9 && (plate.1 - boss.0).abs() < 1e-9);
    assert!((boss.1 - f(&n["stick"])).abs() < 1e-9);
    assert!((boss.1 - boss.0 - f(&n["sled"]["socket"]["in"])).abs() < 1e-9);
    // the ball-nose finishes the edge to its tip's radius past where a pass stops: every piece is inside that, and
    // the spacer takes the rest of what the straight bit reaches, so the bit never meets the printed plate
    let reach = f(&n["sled"]["forward_x"]) + f(&n["bit"]) / 2.0;
    assert!(pieces.1 <= reach, "{} past {reach}", pieces.1);
    assert!(pieces.1 + thick > reach);
    assert!(f(&n["sled"]["cut_to"]) < spacer.1);
    assert!(count >= 50.0);
}

#[test]
fn a_piece_is_the_grown_blank_with_four_holes_all_outside_the_tile() {
    let (b, n) = (numbers(), sled());
    let mut doc = load();
    let piece = bodies_of(&mut doc, "Piece");
    assert_eq!(piece.len(), 1);
    let corners = points(&b["piece"]["corners"]);
    let (thick, hole) = (f(&b["piece"]["thick"]), f(&b["piece"]["hole"]) / 2.0);
    let expect =
        area2(&corners).abs() / 2.0 * thick - 4.0 * std::f64::consts::PI * hole * hole * thick;
    let v = piece[0].volume();
    println!("a piece {v:.1} mm3, {expect:.1} expected");
    assert!((v - expect).abs() < 0.005 * expect);
    // 55 x 48: the sled's blank grown by the same on every side
    let size = pair(&b["piece"]["size"]);
    let sb = points(&b["piece"]["sled_blank"]);
    let grow = f(&b["piece"]["grow"]);
    let side =
        |p: &[P2], i: usize| ((p[i].0 - p[i + 1].0).powi(2) + (p[i].1 - p[i + 1].1).powi(2)).sqrt();
    assert!(
        (size.0 - side(&sb, 0) - 2.0 * grow).abs() < 1e-3
            && (size.1 - side(&sb, 1) - 2.0 * grow).abs() < 1e-3
    );
    assert!((size.0 - 55.0).abs() < 1e-6 && (size.1 - 48.0).abs() < 1e-6);
    // the tile, pushed through the piece, is whole: no hole is in it
    let tile = points(&n["tile"]["outline"]);
    let tile_v = area2(&tile).abs() / 2.0 * thick;
    let got = common(&piece[0], &prism(&tile, 0.0, thick));
    assert!((got - tile_v).abs() < 1e-3 * tile_v, "{got} of {tile_v}");
    // and every rod, with its hole, stands clear of the tile and inside the piece
    for c in points(&b["first"]["rods"]) {
        let off = off_edge(c, &tile) - hole;
        println!("rod at {c:?}: its hole {off:.2} off the tile");
        assert!(off > 1.0);
        assert!(inside(c, &corners));
        assert!(corners.iter().enumerate().all(|(i, _)| seg_dist(
            c,
            corners[i],
            corners[(i + 1) % 4]
        ) > hole + 1.0));
    }
}

#[test]
fn no_first_stage_cut_comes_near_a_rod_nor_into_its_tab_so_every_piece_stays_on_its_rods() {
    let (b, n) = (numbers(), sled());
    let bits = Bits::new(&n);
    let first = cuts(&b["cuts"]["first"]);
    let tile = points(&n["tile"]["outline"]);
    let (rod, tab) = (f(&b["rod"]["d"]) / 2.0, f(&b["rod"]["tab"]));
    let rods = points(&b["first"]["rods"]);
    let ends = points(&b["first"]["tab_ends"]);
    assert_eq!(rods.len(), 4);
    for (c, e) in rods.iter().zip(ends.iter()) {
        // the rod, and half a millimetre round it
        let near = first
            .iter()
            .filter(|k| ring(*c, rod, 48).iter().any(|q| bits.holds(k, *q, 0.5)))
            .count();
        assert_eq!(near, 0, "a cut within 0.5 of the rod at {c:?}");
        // the tab: a strip from the rod to the tile, which it ends inside, and no cut enters it
        assert!(inside(*e, &tile));
        let long = ((e.0 - c.0).powi(2) + (e.1 - c.1).powi(2)).sqrt();
        let u = ((e.0 - c.0) / long, (e.1 - c.1) / long);
        let mut strip = Vec::new();
        for i in 0..=(long / 0.25) as usize {
            let s = (i as f64 * 0.25).min(long);
            for j in 0..=8 {
                let o = -tab / 2.0 + tab * j as f64 / 8.0;
                strip.push((c.0 + u.0 * s - u.1 * o, c.1 + u.1 * s + u.0 * o));
            }
        }
        let into = first
            .iter()
            .filter(|k| strip.iter().any(|q| bits.holds(k, *q, 0.0)))
            .count();
        println!("rod at {c:?}: its tab {long:.1} long to {e:?}; {into} first-stage cuts enter it");
        assert_eq!(into, 0);
    }
    assert!(b["first"]["joined"]
        .as_array()
        .unwrap()
        .iter()
        .all(|j| j.as_bool().unwrap()));
}

#[test]
fn every_finishing_pass_of_the_sled_s_table_is_made_once_in_one_stage() {
    let (b, n) = (numbers(), sled());
    // the sled's passes, each at its last bite
    let mut passes: Vec<(f64, f64, f64)> = Vec::new();
    for r in n["rows"].as_array().unwrap() {
        if r["kind"].as_str() != Some("finish") {
            continue;
        }
        let (t, a, tip) = (f(&r["t"]), f(&r["across"]), f(&r["tip"]));
        match passes.last_mut() {
            Some(p) if p.0 == t && p.1 == a => p.2 = p.2.min(tip),
            _ => passes.push((t, a, tip)),
        }
    }
    let both: Vec<Cut> = cuts(&b["cuts"]["first"])
        .into_iter()
        .chain(cuts(&b["cuts"]["second"]))
        .collect();
    for (t, a, tip) in &passes {
        let made: Vec<&Cut> = both
            .iter()
            .filter(|c| c.ball && (c.t - t).abs() < 1e-5 && (c.across - a).abs() < 1e-3)
            .collect();
        assert!(
            !made.is_empty(),
            "the pass at t {t}, across {a} is not made"
        );
        let deepest = made.iter().map(|c| c.tip).fold(f64::MAX, f64::min);
        assert!(
            (deepest - tip).abs() < 1e-3,
            "made to {deepest}, the table's {tip}"
        );
    }
    let first = cuts(&b["cuts"]["first"]).iter().filter(|c| c.ball).count();
    let second = cuts(&b["cuts"]["second"]).iter().filter(|c| c.ball).count();
    let (f1, f2) = (
        b["first"]["finish_rows"].as_u64().unwrap() as usize,
        b["second"]["finish_rows"].as_u64().unwrap() as usize,
    );
    assert_eq!((first, second), (f1, f2));
    // and every pass in one stage only: the passes of the two do not share a direction and a place
    let one: Vec<(f64, f64)> = cuts(&b["cuts"]["first"])
        .iter()
        .filter(|c| c.ball)
        .map(|c| (c.t, c.across))
        .collect();
    for c in cuts(&b["cuts"]["second"]).iter().filter(|c| c.ball) {
        assert!(!one
            .iter()
            .any(|q| (q.0 - c.t).abs() < 1e-5 && (q.1 - c.across).abs() < 1e-3));
    }
    println!("{} of the sled's passes: {f1} rows in the first stage, {f2} in the second; {} runs in all where the solid stick takes {}",
             passes.len(), b["first"]["rows"].as_u64().unwrap() + b["second"]["rows"].as_u64().unwrap(), b["sled_rows"]);
    // what is left standing at the end is no more than the sled leaves of its own blank, near enough
    assert!(f(&b["second"]["left"]) < f(&n["rough"]["left_at_end"]) + 1.0);
}

/// A stage's table as the controller reads it: the lines after the two of heading, split at their commas.
fn table(name: &str) -> (f64, Vec<Vec<String>>) {
    let text = std::fs::read_to_string(dir().join(name)).unwrap();
    let mut lines = text.lines();
    let safe: f64 = lines
        .next()
        .unwrap()
        .strip_prefix("#safe,")
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(
        lines.next().unwrap(),
        "n,kind,tool,depth,height,turn,via,along,label"
    );
    (
        safe,
        lines
            .map(|l| l.splitn(9, ',').map(String::from).collect())
            .collect(),
    )
}

#[test]
fn each_stage_is_a_table_the_sled_s_controller_takes_and_its_rows_are_the_stage_s_cuts() {
    let (b, n) = (numbers(), sled());
    let (tip_home, across_home) = (f(&n["axes"]["tip_home"]), f(&n["axes"]["across_home"]));
    for (stage, name) in [("first", "out/stage1.csv"), ("second", "out/stage2.csv")] {
        let (safe, rows) = table(name);
        assert!((safe - f(&n["axes"]["safe"])).abs() < 1e-3);
        let kinds: Vec<&str> = rows.iter().map(|r| r[1].as_str()).collect();
        let cutting: Vec<&Vec<String>> = rows
            .iter()
            .filter(|r| r[1] == "rough" || r[1] == "finish")
            .collect();
        let want = cuts(&b["cuts"][stage]);
        assert_eq!(cutting.len(), want.len());
        // the billet put on, the straight bit set, the roughing, a look, the ball-nose set, the finishing, a look
        let rough = want.iter().filter(|c| !c.ball).count();
        assert_eq!(&kinds[..2], ["load", "set"]);
        assert!(kinds[2..2 + rough].iter().all(|k| *k == "rough"));
        assert_eq!(&kinds[2 + rough..4 + rough], ["check", "set"]);
        assert!(kinds[4 + rough..kinds.len() - 1]
            .iter()
            .all(|k| *k == "finish"));
        assert_eq!(*kinds.last().unwrap(), "check");
        assert_eq!(
            rows.len() as u64,
            b["tables"][stage]["rows"].as_u64().unwrap()
        );
        // every cutting row, read back through the positioner's two screws and the spindle, is its cut
        for (r, c) in cutting.iter().zip(want.iter()) {
            let (depth, height, turn): (f64, f64, f64) = (
                r[3].parse().unwrap(),
                r[4].parse().unwrap(),
                r[5].parse().unwrap(),
            );
            let (tip, across) = (tip_home - depth, across_home + height);
            let t = (-turn.to_radians()).rem_euclid(std::f64::consts::TAU);
            let dt = (t - c.t.rem_euclid(std::f64::consts::TAU)).abs();
            assert!(
                (tip - c.tip).abs() < 1e-3 && (across - c.across).abs() < 1e-3,
                "row {}",
                r[0]
            );
            assert!(
                dt < 1e-5 || (dt - std::f64::consts::TAU).abs() < 1e-5,
                "row {}: {t} for {}",
                r[0],
                c.t
            );
            assert_eq!(r[2], if c.ball { "ball" } else { "straight" });
            assert!((r[7].parse::<f64>().unwrap() - f(&b["tables"]["along"])).abs() < 1e-9);
            assert!(depth <= f(&n["axes"]["travel"][0]) - f(&n["axes"]["bit_margin"]) + 1e-6);
        }
        // the billet goes on with the bit drawn back clear of its corners, which swing further than the stick's did
        let load_tip = tip_home - rows[0][3].parse::<f64>().unwrap();
        let swing = points(&b["piece"]["corners"])
            .iter()
            .map(|q| (q.0 * q.0 + q.1 * q.1).sqrt())
            .fold(0.0, f64::max);
        assert!((load_tip - swing).abs() < 1e-3 && swing > f(&n["blank_swing"]));
        // the controller's own program made every pass of it
        assert_eq!(
            b["tables"][stage]["passes"].as_u64().unwrap() as usize,
            want.len()
        );
        let straight = rows.iter().filter(|r| r[6] == "d").count();
        println!("{stage} stage: {} rows, {} cutting, {straight} moves straight, {:.0} min by the controller's pauses",
                 rows.len(), want.len(), f(&b["tables"][stage]["seconds"]) / 60.0);
        assert!(straight * 10 > want.len() * 8);
    }
    // a dry run for each stage: rows of its own table, no tool, at the ends of the positioner's reach among them
    for (stage, name, full) in [
        ("first", "out/dry1.csv", "out/stage1.csv"),
        ("second", "out/dry2.csv", "out/stage2.csv"),
    ] {
        let (_, rows) = table(name);
        let (_, whole) = table(full);
        let dry: Vec<&Vec<String>> = rows.iter().filter(|r| r[1] == "dry").collect();
        assert_eq!(
            dry.len() as u64,
            b["tables"]["dry"][stage]["passes"].as_u64().unwrap()
        );
        assert!(rows.first().unwrap()[1] == "say" && rows.last().unwrap()[1] == "say");
        let num = |r: &Vec<String>, k: usize| r[k].parse::<f64>().unwrap();
        for r in &dry {
            assert_eq!(r[2], "none");
            let of: usize = r[8].strip_prefix("as row ").unwrap().parse().unwrap();
            let like = &whole[of - 1];
            assert!(like[1] == "rough" || like[1] == "finish");
            assert!((3..=5)
                .chain(7..=7)
                .all(|k| (num(r, k) - num(like, k)).abs() < 1e-9));
        }
        let deepest = whole
            .iter()
            .filter(|r| r[1] == "rough" || r[1] == "finish")
            .map(|r| num(r, 3))
            .fold(0.0, f64::max);
        assert!(
            dry.iter().any(|r| (num(r, 3) - deepest).abs() < 1e-9),
            "the deepest row is tried dry"
        );
    }
    // a tab alone would take the stage's heaviest push with room to spare
    let t = &b["tab"];
    let stress = f(&t["push"]) / (f(&t["wide"]) * f(&t["thick"]));
    println!(
        "a tab: {:.1} N on {} x {} mm is {stress:.2} MPa",
        f(&t["push"]),
        t["wide"],
        t["thick"]
    );
    assert!((stress - f(&t["stress"])).abs() < 1e-9 && stress * 2.0 < f(&t["across_grain"]));
    // drawn back between passes, the bit is past everything that stands out of the billet's tail end
    assert!(f(&b["tables"]["bit_waits"]) - f(&b["tables"]["stands_out"]) >= 1.0);
}

#[test]
fn the_belt_holds_every_bite_of_both_stages_by_the_sled_s_own_estimate() {
    let (b, n) = (numbers(), sled());
    let h = &n["hold"];
    let (stiff, swing) = (f(&h["stiff"]), f(&n["tile"]["swing"]));
    let limit = pair(&h["limit"]);
    let (ball, shank, taper, straight) = (
        f(&n["bit"]),
        f(&n["shank"]),
        f(&n["taper"]).to_radians(),
        f(&n["rough"]["bit"]),
    );
    let mut worst = (0.0f64, 0.0f64);
    for stage in ["first", "second"] {
        for c in b["cuts"][stage].as_array().unwrap() {
            let (is_ball, area, over, far, face) = (
                c[0].as_str().unwrap() == "b",
                f(&c[4]),
                f(&c[5]),
                f(&c[6]),
                c[7].as_bool().unwrap(),
            );
            // the diameter the bit cuts at: the straight bit's own; the ball-nose's half way up what it is cutting
            let dia = if !is_ball {
                straight
            } else {
                let (r, a) = (ball / 2.0, over / 2.0);
                if a < r {
                    (2.0 * (2.0 * r * a - a * a).max(0.0).sqrt()).max(1.0)
                } else {
                    shank.min(ball + 2.0 * (a - r) * taper.tan())
                }
            };
            let edge_speed = std::f64::consts::PI * dia * f(&h["rpm"]) / 60.0;
            let push =
                f(&h["peak"]) * f(&h["cut_energy"]) * 1000.0 * area * f(&h["feed"]) / edge_speed;
            let give = push * far / 1000.0 / stiff * swing;
            if face {
                worst.1 = worst.1.max(give);
            } else {
                worst.0 = worst.0.max(give);
            }
            assert!(
                give <= if face { limit.1 } else { limit.0 } + 2e-5,
                "{stage}: a bite of {area} mm2 gives {give}"
            );
        }
    }
    println!("the tile's edge moves at most {:.4} mm in a bite and {:.4} in one that leaves a face; the sled allows {limit:?}", worst.0, worst.1);
    assert!(b["hold"]["extra_runs"].as_u64().unwrap() < 40);
}

#[test]
fn the_comb_lies_in_what_the_first_stage_cut_away_and_no_second_stage_cut_comes_near_it() {
    let (b, n) = (numbers(), sled());
    let bits = Bits::new(&n);
    let second = cuts(&b["cuts"]["second"]);
    let tile = points(&n["tile"]["outline"]);
    let mut doc = load();
    let comb = bodies_of(&mut doc, "Comb");
    assert_eq!(comb.len(), 1, "the comb is one print");
    let stack = bodies_of(&mut doc, "Stack after the first stage");
    assert_eq!(stack.len(), 1);
    let (cv, sv) = (comb[0].volume(), stack[0].volume());
    let both = common(&comb[0], &stack[0]);
    println!("comb {cv:.0} mm3, the stack after the first stage {sv:.0}; they share {both:.2}");
    assert!(both < 1e-3 * cv);
    // the second stage's rods go through the comb's holes and touch nothing of it
    let rods = bodies_of(&mut doc, "Rods, second stage");
    let rod_v: f64 = rods.iter().map(|r| common(&comb[0], r)).sum();
    assert!(rod_v < 1e-6, "{rod_v}");
    // three fingers, round the axis, each bearing on finished edge, and no second-stage cut within half a millimetre
    let fingers = b["second"]["fingers"].as_array().unwrap();
    assert_eq!(fingers.len(), 3);
    for fg in fingers {
        let room = points(&fg["room"]);
        let near = second
            .iter()
            .filter(|k| room.iter().any(|q| bits.holds(k, *q, 0.5)))
            .count();
        let c = pair(&fg["rod"]);
        let wall = (0..room.len())
            .map(|i| seg_dist(c, room[(i + room.len() - 1) % room.len()], room[i]))
            .fold(f64::MAX, f64::min)
            - f(&b["rod"]["d"]) / 2.0;
        println!("finger with its rod at {c:?}: bears on {:.0} mm of edge, its wall round the rod {wall:.2}, {near} cuts within 0.5", f(&fg["bears"]));
        assert_eq!(near, 0);
        assert!(f(&fg["bears"]) > 20.0);
        assert!(wall >= 1.0 - 0.2);
        assert!(off_edge(c, &tile) > 0.0);
    }
    let p: Vec<P2> = fingers.iter().map(|fg| pair(&fg["rod"])).collect();
    let side = |a: P2, b: P2| (b.0 - a.0) * (0.0 - a.1) - (b.1 - a.1) * (0.0 - a.0);
    let s = [side(p[0], p[1]), side(p[1], p[2]), side(p[2], p[0])];
    assert!(
        s.iter().all(|x| *x > 0.0) || s.iter().all(|x| *x < 0.0),
        "the axis is inside the three rods"
    );
    // the plate lies over the tile only: no cut goes there
    for q in points(&b["comb"]["plate_loop"]) {
        assert!(off_edge(q, &tile) < 0.0);
    }
}

#[test]
fn the_far_plate_s_boss_is_the_sled_s_blank_so_the_sled_s_socket_takes_it() {
    let b = numbers();
    let mut doc = load();
    let plate = bodies_of(&mut doc, "Far plate");
    assert_eq!(plate.len(), 1);
    let boss = pair(&b["layout"]["boss"]);
    let sled_blank = points(&b["piece"]["sled_blank"]);
    let big: Vec<P2> = vec![(-60.0, -60.0), (60.0, -60.0), (60.0, 60.0), (-60.0, 60.0)];
    let all = common(&plate[0], &prism(&big, boss.0, boss.1 + 1.0));
    let within = common(&plate[0], &prism(&sled_blank, boss.0, boss.1 + 1.0));
    println!("the boss {all:.0} mm3, of it inside the sled's blank {within:.0}");
    assert!(all > 0.8 * area2(&sled_blank).abs() / 2.0 * (boss.1 - boss.0));
    assert!((all - within).abs() < 1e-3 * all);
    // the plate itself is the piece's size, past the bit's reach
    let (lo, hi) = plate[0].bounds().unwrap();
    assert!((lo.x - f(&b["layout"]["far_plate"][0])).abs() < 1e-6 && (hi.x - boss.1).abs() < 1e-6);
    assert!(lo.x > f(&b["layout"]["cut_to"]));
}

#[test]
fn the_bits_stand_out_of_the_collet_nut_within_what_the_sled_allows() {
    let (b, n) = (numbers(), sled());
    for tool in ["straight", "ball"] {
        let bit = &b["bits"][tool];
        let (set, need, out) = (f(&bit["set"]), f(&bit["need"]), f(&bit["sled_out"]));
        let range = pair(&bit["range"]);
        println!("{tool}: needs {need:.2} out of the nut, set {set:.1}, the sled sets {out:.2} and allows {range:?}");
        assert!(set >= need && set <= range.1);
        assert!(set - out <= f(&n["axes"]["bit_margin"]) + 1e-9);
    }
}

#[test]
fn the_animation_has_a_frame_for_every_pass_and_a_still_for_every_step() {
    let b = numbers();
    let a = json(dir().join("out/billet_anim.json"));
    let rows = b["first"]["rows"].as_u64().unwrap() + b["second"]["rows"].as_u64().unwrap();
    assert_eq!(a["passes"].as_u64().unwrap(), rows);
    assert_eq!(
        a["first"].as_u64().unwrap(),
        b["first"]["rows"].as_u64().unwrap()
    );
    let frames = a["cut_frames"].as_array().unwrap();
    assert_eq!(frames.len() as u64, rows);
    assert!(frames.windows(2).all(|w| w[0].as_u64() < w[1].as_u64()));
    let stills = a["stills"].as_object().unwrap();
    assert_eq!(stills.len(), 7);
    for k in 1..=7 {
        let path = dir().join(format!("out/billet_{k}.png"));
        assert!(path.exists(), "{path:?}");
        assert!(stills[&k.to_string()].as_u64().unwrap() < a["frames"].as_u64().unwrap());
    }
    // the swap of the rods is between the stages: after the first stage's last pass and before the second's first
    let first = a["first"].as_u64().unwrap() as usize;
    let (last1, first2) = (
        frames[first - 1].as_u64().unwrap(),
        frames[first].as_u64().unwrap(),
    );
    assert!(first2 - last1 > a["swap"].as_u64().unwrap());
    assert!(dir().join("out/billet.gif").exists());
}
