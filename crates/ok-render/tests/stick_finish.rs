//! Finishing the stick the sled mills (examples/stick-finish): one
//! sanding form for the tile's seven bumps, one for its seven flutes,
//! and the ends of two scraper blades. Checked from the committed
//! document and numbers, with the tile as the stick sled has it: the
//! tile is seven bumps and seven flutes of one curve each, a twelfth
//! of a turn or several apart and none mirrored, and alike only away
//! from their ends; each form, put on every one of its seven, is the
//! film's thickness from the wood all along its face and touches
//! nothing else; the flute form's neck is in the one way out that all
//! seven flutes share, and that way is narrow; and each blade's end is
//! the tile's own curve.

use ok_brep::{BoolOp, Solid};
use ok_math::{Plane, Vec2};
use ok_model::{Document, TabId};
use ok_sketch::{Loop, Profile};
use serde_json::Value;
use std::path::PathBuf;

type P2 = (f64, f64);

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/stick-finish")
}

fn load() -> Document {
    Document::from_json(&std::fs::read_to_string(dir().join("out/finish.okpart")).unwrap()).unwrap()
}

fn numbers() -> Value {
    serde_json::from_str(&std::fs::read_to_string(dir().join("out/finish.json")).unwrap()).unwrap()
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap()
}

fn points(v: &Value) -> Vec<P2> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|q| (f(&q[0]), f(&q[1])))
        .collect()
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

/// The tile as the stick sled cuts it: this example is built on it.
fn tile() -> Vec<P2> {
    let path = dir().join("../stick-sled/out/sled.json");
    let n: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut p = points(&n["tile"]["outline"]);
    if area2(&p) < 0.0 {
        p.reverse();
    }
    p
}

fn area2(p: &[P2]) -> f64 {
    (0..p.len())
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum()
}

/// How far a point is from a closed outline.
fn off(q: P2, ring: &[P2]) -> f64 {
    let mut best = f64::MAX;
    for i in 0..ring.len() {
        let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
        let d = (b.0 - a.0, b.1 - a.1);
        let l2 = d.0 * d.0 + d.1 * d.1;
        let t = if l2 < 1e-18 {
            0.0
        } else {
            (((q.0 - a.0) * d.0 + (q.1 - a.1) * d.1) / l2).clamp(0.0, 1.0)
        };
        best = best.min((q.0 - a.0 - d.0 * t).hypot(q.1 - a.1 - d.1 * t));
    }
    best
}

/// An edge's move onto the tile, as the build wrote it: mirrored or
/// not, the turn's cosine and sine, the shift.
#[derive(Clone, Copy)]
struct Move {
    mirrored: bool,
    c: f64,
    s: f64,
    x: f64,
    y: f64,
}

fn moves(v: &Value) -> Vec<Move> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|p| {
            let m = &p["move"];
            Move {
                mirrored: m[0].as_bool().unwrap(),
                c: f(&m[1]),
                s: f(&m[2]),
                x: f(&m[3]),
                y: f(&m[4]),
            }
        })
        .collect()
}

/// A point of an edge's own frame, on the tile.
fn onto(m: &Move, q: P2) -> P2 {
    let u = if m.mirrored { -q.0 } else { q.0 };
    (m.c * u - m.s * q.1 + m.x, m.s * u + m.c * q.1 + m.y)
}

/// A point of the tile in an edge's own frame: across, and out of the wood.
fn into(m: &Move, q: P2) -> P2 {
    let (x, y) = (q.0 - m.x, q.1 - m.y);
    let (u, v) = (m.c * x + m.s * y, -m.s * x + m.c * y);
    (if m.mirrored { -u } else { u }, v)
}

/// An edge's own frame as a part is drawn to print: lying on its back
/// the wood's side is up, standing it is as it is.
fn drawn(how: &Value, q: P2) -> P2 {
    let (mid, from) = (f(&how["mid"]), f(&how["from"]));
    if how["flipped"].as_bool().unwrap() {
        (q.0 - mid, from - q.1)
    } else {
        (q.0 - mid, q.1 - from)
    }
}

/// A section pushed along its plane's normal from `lo` to `hi`.
fn prism(section: &[P2], plane: &Plane, lo: f64, hi: f64) -> Solid {
    let mut p: Vec<Vec2> = section.iter().map(|q| Vec2::new(q.0, q.1)).collect();
    if area2(section) < 0.0 {
        p.reverse();
    }
    let profile = Profile {
        outer: Loop::polygon(p),
        holes: Vec::new(),
    };
    ok_brep::extrude(&profile, plane, lo, hi, 1).unwrap()
}

fn common(a: &Solid, b: &Solid) -> f64 {
    ok_brep::boolean(a, b, BoolOp::Intersection)
        .map(|s| s.volume().abs())
        .unwrap_or(f64::MAX)
}

/// The stick, as a part drawn to print sees it when the part is on
/// one of the seven: the tile's outline laid back into the edge's own
/// frame, moved `toward` the part (out of the wood), drawn as the part
/// is, and pushed along the part's length and past both its ends.
fn stick(outline: &[P2], m: &Move, how: &Value, toward: f64, plane: &Plane, long: f64) -> Solid {
    let section: Vec<P2> = outline
        .iter()
        .map(|q| {
            let own = into(m, *q);
            drawn(how, (own.0, own.1 + toward))
        })
        .collect();
    prism(&section, plane, -5.0, long + 5.0)
}

/// The tile is seven bumps and seven flutes. The kernel draws every
/// edge as one curve, a bump leaning back the way the outline came and
/// a flute forward, so a bump is a bump turned and never mirrored. Cut
/// 0.35 mm inside its edge for the gap, with corners at different
/// angles, they are alike only away from their ends, and a form is the
/// stretch all seven share.
#[test]
fn the_seven_bumps_are_one_curve_away_from_their_ends_and_so_are_the_seven_flutes() {
    let (n, outline) = (numbers(), tile());
    // the tile this was built on is the one the sled cuts now
    let mine = points(&n["tile"]["outline"]);
    assert_eq!(mine.len(), outline.len());
    assert!(mine
        .iter()
        .zip(&outline)
        .all(|(a, b)| (a.0 - b.0).hypot(a.1 - b.1) < 1e-9));
    let (same, cover) = (f(&n["same"]), f(&n["cover"]));
    assert!(same <= 0.05);
    for name in ["bump", "flute"] {
        let e = &n["edges"][name];
        let (places, work) = (moves(&e["places"]), points(&e["work"]));
        assert_eq!(places.len(), 7);
        // a twelfth of a turn or several from the first, and none mirrored
        let first = places[0].s.atan2(places[0].c);
        for m in &places {
            let twelfths = (m.s.atan2(m.c) - first) / std::f64::consts::FRAC_PI_6;
            assert!(!m.mirrored && (twelfths - twelfths.round()).abs() < 1e-9);
            assert!((m.c * m.c + m.s * m.s - 1.0).abs() < 1e-12);
        }
        // what the form covers, laid on each of the seven, is on the tile
        let worst = places
            .iter()
            .map(|m| {
                work.iter()
                    .map(|q| off(onto(m, *q), &outline))
                    .fold(0.0, f64::max)
            })
            .fold(0.0, f64::max);
        let (round, covered) = (f(&e["round"]), f(&e["covered"]));
        // and the whole of one, laid on the others, is not: the ends differ, which is why a form stops short
        let whole = e["places"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| f(&p["off_whole"]))
            .fold(0.0, f64::max);
        println!(
            "{name}s: {round:.1} mm round, the form covers {covered:.1}; laid on each of the seven that stretch is off by {worst:.3} at most, the whole of one by {whole:.2}"
        );
        assert!(worst <= same + 1e-9, "{name}: {worst}");
        assert!(covered >= cover * round && covered < round);
        assert!(whole > 0.2, "{name}: the ends are alike too");
        // the stretch is one piece about the edge's middle
        let length: f64 = work
            .windows(2)
            .map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1))
            .sum();
        assert!((length - covered).abs() < 0.05);
    }
    // a bump is higher than a flute is deep by less than nothing: the gap makes bumps smaller and flutes larger
    assert!(f(&n["edges"]["bump"]["high"]) < f(&n["edges"]["flute"]["high"]));
}

/// Each form, as the document has it, is one solid that stands on the
/// plate, and its section is the one the build wrote down. Put on
/// every one of its seven edges it shares nothing with the stick; the
/// stick moved toward it by a little less than the film's thickness
/// still shares nothing, and moved a little more than the film's
/// thickness meets it. That is the film's room, found with solids.
/// And all along its face it is the film's thickness from the wood.
#[test]
fn each_form_on_every_one_of_its_seven_is_the_film_s_thickness_from_the_wood_and_touches_nothing_else(
) {
    let (mut doc, n, outline) = (load(), numbers(), tile());
    let (film, same) = (f(&n["film"]), f(&n["same"]));
    assert!(film > 0.1 && film < 0.4);
    for name in ["bump", "flute"] {
        let (e, form) = (&n["edges"][name], &n["forms"][name]);
        let bodies = bodies_of(&mut doc, form["tab"].as_str().unwrap());
        assert_eq!(bodies.len(), 1);
        let solid = &bodies[0];
        let (volume, long) = (solid.volume(), f(&form["long"]));
        assert!((volume - f(&form["volume"])).abs() < 1e-3 * volume);
        let lying = form["plane"].as_str().unwrap() == "right";
        let plane = if lying { Plane::YZ } else { Plane::XY };
        // its section is the one written down: pushed along its length again, it is the same solid
        let section = points(&form["section"]);
        let again = prism(&section, &plane, 0.0, long);
        assert!((common(solid, &again) - volume).abs() < 1e-3 * volume);
        // it stands on the plate: on its back, or on its end
        let b = solid.bounds().unwrap();
        assert!(b.0.z.abs() < 1e-9);
        let (across, deep) = (f(&form["across"]), f(&form["deep"]));
        if lying {
            assert!((b.1.x - b.0.x - long).abs() < 1e-6 && (b.1.z - deep).abs() < 1e-6);
        } else {
            assert!((b.1.z - long).abs() < 1e-6 && (b.1.x - b.0.x - across).abs() < 1e-6);
        }
        assert!(long <= 200.0 && across < 30.0 && deep < 30.0);
        // on each of the seven
        let (places, face) = (moves(&e["places"]), points(&e["face"]));
        let (mut least, mut most, mut bites) = (f64::MAX, 0.0f64, f64::MAX);
        for m in &places {
            let on = common(solid, &stick(&outline, m, &form["how"], 0.0, &plane, long));
            let near = common(
                solid,
                &stick(&outline, m, &form["how"], film - 0.06, &plane, long),
            );
            let past = common(
                solid,
                &stick(&outline, m, &form["how"], film + 0.1, &plane, long),
            );
            assert!(on < 1e-3 && near < 1e-3, "{name}: {on} {near}");
            assert!(past > 1.0, "{name}: {past}");
            bites = bites.min(past);
            for q in &face {
                let d = off(onto(m, *q), &outline);
                least = least.min(d);
                most = most.max(d);
            }
        }
        println!(
            "{name} form: on each of the seven it shares nothing with the stick, nor with the stick {:.2} nearer; {:.2} nearer it shares {bites:.1} mm3 or more; its face is {least:.3} to {most:.3} from the wood",
            film - 0.06,
            film + 0.1
        );
        assert!(least >= film - same - 1e-6 && most <= film + same + 1e-6);
        // a strip of film as wide as the face is long round
        let round: f64 = face
            .windows(2)
            .map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1))
            .sum();
        assert!((round - f(&form["strip"])).abs() < 1e-6 && round > 10.0);
    }
}

/// A flute is wider inside than at its mouth, and at some corners the
/// lobe next door hooks over it. There is one way out that all seven
/// flutes share, straight out of the mouth, and it is narrow: found
/// here from the tile and the seven moves. The flute form's neck is in
/// it with air each side, and its grip is past the lobes; the bump
/// form leans in as it leaves the stick for the same reason.
#[test]
fn the_flute_form_s_neck_is_in_the_one_way_out_that_all_seven_flutes_share() {
    let (n, outline) = (numbers(), tile());
    let places = moves(&n["edges"]["flute"]["places"]);
    // the wood of all seven, in the first flute's own frame
    let mut wood: Vec<P2> = Vec::new();
    for m in &places {
        for i in 0..outline.len() {
            let (a, b) = (outline[i], outline[(i + 1) % outline.len()]);
            let steps = ((b.0 - a.0).hypot(b.1 - a.1) / 0.05).ceil().max(1.0) as usize;
            for k in 0..steps {
                let t = k as f64 / steps as f64;
                wood.push(into(m, (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)));
            }
        }
    }
    let (neck, air, shoulder) = (&n["neck"], f(&n["air"]), f(&n["shoulder"]));
    let (left, right, top) = (f(&neck[0]), f(&neck[1]), f(&neck[2]));
    let (mut narrowest, mut least) = (f64::MAX, f64::MAX);
    let mut v = shoulder;
    while v <= top {
        let near = |side: f64| {
            wood.iter()
                .filter(|w| (w.1 - v).abs() <= 0.15 && w.0 * side > 0.0)
                .map(|w| w.0.abs())
                .fold(f64::MAX, f64::min)
        };
        let (l, r) = (-near(-1.0), near(1.0));
        narrowest = narrowest.min(r - l);
        least = least.min(left - l).min(r - right);
        v += 0.1;
    }
    println!(
        "the way out of a flute is {narrowest:.1} mm wide at its narrowest; the neck is {:.1} wide with {least:.2} or more each side, {top:.1} long",
        right - left
    );
    assert!(narrowest < 8.0, "any neck would do");
    assert!(least >= air - 0.15 && right - left >= 3.0);
    // the neck is far narrower than the bulb it carries, and the grip wider than the neck
    let face = points(&n["edges"]["flute"]["face"]);
    let bulb = face.iter().map(|q| q.0).fold(f64::MIN, f64::max)
        - face.iter().map(|q| q.0).fold(f64::MAX, f64::min);
    assert!(bulb > 2.0 * (right - left) && f(&n["grip"][0]) > 3.0 * (right - left));
    // past the neck the way is open to the grip's width
    let grip = (f(&n["grip"][0]), f(&n["grip"][1]));
    let mid = (left + right) / 2.0;
    assert!(wood
        .iter()
        .filter(|w| w.1 >= top && w.1 <= top + grip.1)
        .all(|w| (w.0 - mid).abs() >= grip.0 / 2.0 + air - 0.15));
}

/// The scraper's two blades are the ends of the same two shapes with
/// no film: each template's edge is the tile's own curve. On every one
/// of its seven, the stick drawn back a twentieth of a millimetre
/// shares nothing with the plate, and pushed a fifth into it does.
#[test]
fn each_blade_template_ends_in_the_tile_s_own_curve() {
    let (mut doc, n, outline) = (load(), numbers(), tile());
    let plates = bodies_of(&mut doc, "Blade templates");
    assert_eq!(plates.len(), 2);
    let thick = f(&n["plate"][1]);
    for (k, name) in ["bump", "flute"].iter().enumerate() {
        let (t, plate) = (&n["templates"][name], &plates[k]);
        let volume = plate.volume();
        assert!((volume - f(&t["volume"])).abs() < 1e-3 * volume);
        let b = plate.bounds().unwrap();
        assert!(b.0.z.abs() < 1e-9 && (b.1.z - thick).abs() < 1e-9);
        let again = prism(&points(&t["outline"]), &Plane::XY, 0.0, thick);
        assert!((common(plate, &again) - volume).abs() < 1e-3 * volume);
        let (mut back, mut into_it) = (0.0f64, f64::MAX);
        for m in &moves(&n["edges"][name]["places"]) {
            back = back.max(common(
                plate,
                &stick(&outline, m, &t["how"], -0.05, &Plane::XY, thick),
            ));
            into_it = into_it.min(common(
                plate,
                &stick(&outline, m, &t["how"], 0.2, &Plane::XY, thick),
            ));
        }
        println!(
            "{name} blade: the stick 0.05 back shares {back:.4} mm3 with it at most, 0.2 in {into_it:.2} at least"
        );
        // The seven are alike to a few hundredths, not exactly, and a flank is too steep for drawing the stick
        // back to clear it: a sliver that thin, down a millimetre of flank, is all that may be shared.
        assert!(back < 0.02 && into_it > 0.5);
        // the plate is the blade's end: it goes as far from the stick as the build says, and is no wider than passes the lobes
        assert!(b.1.y - b.0.y >= f(&n["plate"][0]) && b.1.x - b.0.x < 16.0);
    }
}

/// The whole job as one animation, read back frame by frame: the
/// machine's coarse passes, a held frame that says so and says what is
/// next, its detail passes, another, and then the sanding: each form
/// shown on each of its seven edges, a pause after each, the stretch
/// it has been along left in its colour. The tile's outline is amber
/// until it is done, so what is amber at the end is what the forms do
/// not reach, and that is the share of the outline the numbers say.
#[test]
fn the_whole_job_s_animation_is_captioned_at_its_pauses_and_shows_every_use_of_the_two_forms() {
    let n = numbers();
    let a: Value =
        serde_json::from_str(&std::fs::read_to_string(dir().join("out/whole.json")).unwrap())
            .unwrap();
    let colour = |v: &Value| {
        let c = u32::from_str_radix(&v.as_str().unwrap()[1..], 16).unwrap();
        [(c >> 16) as u8, (c >> 8) as u8, c as u8]
    };
    let near = |c: ok_render::Rgb, want: [u8; 3]| {
        [c.0, c.1, c.2]
            .iter()
            .zip(want)
            .all(|(p, q)| (*p as i32 - q as i32).abs() <= 8)
    };
    let (cut, todo) = (colour(&a["cut"]), colour(&a["todo"]));
    let lines = [colour(&a["form"]["bump"]), colour(&a["form"]["flute"])];
    let fills = [
        colour(&a["form_fill"]["bump"]),
        colour(&a["form_fill"]["flute"]),
    ];
    let size = a["size"].as_u64().unwrap() as usize;
    let band = (f(&a["band"]) * size as f64 / (f(&a["view"][2]) - f(&a["view"][0]))) as usize;
    /// What a frame shows: how long, and how many of its pixels are the cutter, each form's body, each
    /// form's line, the outline still to do, and lettering in the band above and the band below.
    #[derive(Default, Clone)]
    struct Seen {
        ms: u32,
        cutter: usize,
        body: [usize; 2],
        line: [usize; 2],
        todo: usize,
        above: usize,
        below: usize,
    }
    let mut seen: Vec<Seen> = Vec::new();
    let bytes = std::fs::read(dir().join("out/whole.gif")).unwrap();
    assert!(bytes.len() < 5_000_000, "{} bytes", bytes.len());
    let (w, h, frames) = ok_render::flipbook::read_gif(&bytes, |_, image, ms| {
        let mut s = Seen {
            ms,
            ..Seen::default()
        };
        for y in (0..image.height).step_by(3) {
            for x in (0..image.width).step_by(3) {
                let c = image.pixel(x, y);
                if c.0 < 110 && c.1 < 110 && c.2 < 110 {
                    if y < band {
                        s.above += 1;
                    } else if y >= image.height - band {
                        s.below += 1;
                    }
                } else if near(c, cut) {
                    s.cutter += 1;
                } else if near(c, todo) {
                    s.todo += 1;
                } else {
                    for k in 0..2 {
                        if near(c, lines[k]) {
                            s.line[k] += 1;
                        } else if near(c, fills[k]) {
                            s.body[k] += 1;
                        }
                    }
                }
            }
        }
        seen.push(s);
    })
    .unwrap();
    assert_eq!((w, h), (size, size));
    assert_eq!(frames, a["frames"].as_u64().unwrap() as usize);
    assert_eq!(seen.len(), frames);
    let seconds: f64 = seen.iter().map(|s| s.ms as f64 / 1000.0).sum();
    assert!((seconds - f(&a["seconds"])).abs() < 0.5 && seconds > 30.0 && seconds < 120.0);
    let at = |v: &Value| v.as_u64().unwrap() as usize;
    // The passes are the sled's table: as many coarse as it has roughing rows, as many detail as finishing.
    let sled: Value = serde_json::from_str(
        &std::fs::read_to_string(dir().join("../stick-sled/out/sled.json")).unwrap(),
    )
    .unwrap();
    let rows = |kind: &str| {
        sled["rows"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["kind"].as_str() == Some(kind))
            .count()
    };
    let (coarse, detail) = (at(&a["coarse"]), at(&a["detail"]));
    assert_eq!((coarse, detail), (rows("rough"), rows("finish")));
    let cuts: Vec<usize> = a["cut_frames"].as_array().unwrap().iter().map(at).collect();
    assert_eq!(cuts.len(), coarse + detail);
    // every pass has its cutter in the frame and the chapter's name over it, and nothing written under it
    for (k, c) in cuts.iter().enumerate() {
        let s = &seen[*c];
        assert!(s.cutter > 20 && s.above > 20 && s.below == 0, "pass {k}");
        assert!(s.ms < 1500);
    }
    // The two pauses: held, no cutter, and written on above and below. The first is between the last coarse
    // pass and the first detail pass; the second after the last pass, the tile by then alone.
    let pauses: Vec<usize> = a["pauses"].as_array().unwrap().iter().map(at).collect();
    assert!(
        cuts[coarse - 1] < pauses[0]
            && pauses[0] < cuts[coarse]
            && pauses[1] > cuts[cuts.len() - 1]
    );
    for p in &pauses {
        let s = &seen[*p];
        assert_eq!(s.ms as u64, a["pause"].as_u64().unwrap());
        assert!(
            s.ms >= 3000 && s.cutter == 0 && s.above > 40 && s.below > 60,
            "{p}"
        );
        // two lines under, one over
        assert!(s.below > s.above);
    }
    // The sanding. Fourteen uses, the bump form's seven and then the flute form's seven; a form is in its frame
    // and in no other; after each, more of the outline is in that form's colour and less of it is still to do.
    let uses = a["uses"].as_array().unwrap();
    assert_eq!(uses.len(), 14);
    let from = at(&a["sanding_from"]);
    assert_eq!(from, pauses[1] + 1);
    let whole = seen[from].todo;
    assert!(whole > 300 && seen[from].line == [0, 0]);
    let mut before = seen[from].clone();
    for (i, u) in uses.iter().enumerate() {
        let k = if u["form"].as_str() == Some("bump") {
            0
        } else {
            1
        };
        assert_eq!(k, i / 7);
        assert_eq!(at(&u["edge"]), i % 7 + 1);
        let (show, rest) = (&seen[at(&u["show"])], &seen[at(&u["rest"])]);
        assert_eq!(at(&u["rest"]), at(&u["show"]) + 1);
        assert_eq!(show.ms as u64, a["use"].as_u64().unwrap());
        assert_eq!(rest.ms as u64, a["rest"].as_u64().unwrap());
        assert!(show.ms >= 800 && rest.ms >= 300);
        // a form's body is its own pale colour; the few pixels of that colour in other frames are the soft edges of
        // the lines already drawn
        assert!(
            show.body[k] > 100 && show.body[1 - k] < 30,
            "use {i}: {:?}",
            show.body
        );
        assert!(rest.body[k] * 4 < show.body[k] && rest.body[1 - k] < 30 && show.cutter == 0);
        assert!(
            show.below > 40 && rest.below > show.below,
            "use {i}: it is named, and then said done"
        );
        assert!(rest.line[k] > before.line[k] + 10 && rest.todo < before.todo);
        assert_eq!(rest.line[1 - k], before.line[1 - k]);
        before = rest.clone();
    }
    // The end: held longest, both colours on the tile, and amber where neither form has been. That is the
    // corners: as much of the outline as the build's own lengths say, found here from the tile and the forms.
    let last = &seen[frames - 1];
    assert_eq!(last.ms as u64, a["end"].as_u64().unwrap());
    assert!(last.ms >= 4000 && last.line[0] > 100 && last.line[1] > 100);
    assert!(last.body[0] < 30 && last.body[1] < 30);
    let outline = tile();
    let round: f64 = (0..outline.len())
        .map(|i| {
            let (p, q) = (outline[i], outline[(i + 1) % outline.len()]);
            (q.0 - p.0).hypot(q.1 - p.1)
        })
        .sum();
    let covered = 7.0 * (f(&n["edges"]["bump"]["covered"]) + f(&n["edges"]["flute"]["covered"]));
    assert!((round - f(&a["round"])).abs() < 1e-6 && (covered - f(&a["covered"])).abs() < 1e-6);
    let left = (round - covered) / round;
    let shown = last.todo as f64 / whole as f64;
    println!(
        "{frames} frames, {seconds:.0} s: {coarse} coarse passes, {detail} detail passes, 14 uses of the forms; the forms leave {:.1} of the {round:.1} mm round the tile, {:.0} % of it, and {:.0} % of the outline is still amber at the end",
        round - covered,
        100.0 * left,
        100.0 * shown
    );
    // a little less is amber than is left: each line is drawn with some width, and its ends go that much over
    assert!(left > 0.1 && left < 0.25 && shown > 0.05 && shown < left);
}
