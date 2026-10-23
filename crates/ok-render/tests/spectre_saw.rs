//! The Spectre saw station (examples/spectre-saw) as a regression
//! suite: the kernel's program for the example's settings is ten rips,
//! two through and four pairs of rabbets, on a blank that already has
//! three faces, every turn on the 30° grid, every cassette's floor at
//! least the thinnest, and the section it leaves is the tile less the
//! nicks past the corners and nothing else; the first cassette takes
//! the blank either way round and no other does; each cassette in the
//! document holds its stick and offcuts with nothing overlapping, the
//! lid clear of the blade's sweep and the base in it only under the
//! stick, where the blade cuts its slit; and the last stick is the tile.
//! Every number prints (`--nocapture`).

use ok_brep::{BoolOp, Solid};
use ok_math::{Plane, Vec2};
use ok_model::{Document, RegenResult, TabId};
use ok_sketch::spectre::saw::{program, Kind, Settings};
use ok_sketch::{signed_area, Loop, Profile};
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/spectre-saw")
}

fn load() -> Document {
    let json = std::fs::read_to_string(dir().join("out/spectre_saw.okpart")).unwrap();
    Document::from_json(&json).unwrap()
}

fn numbers() -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(dir().join("out/saw.json")).unwrap()).unwrap()
}

fn num(v: &serde_json::Value, k: &str) -> f64 {
    v[k].as_f64().unwrap_or_else(|| panic!("no {k}"))
}

fn settings(n: &serde_json::Value) -> Settings {
    let s = &n["program"]["settings"];
    Settings {
        edge: num(s, "edge"),
        gap: num(s, "gap"),
        kerf: num(s, "kerf"),
        margin: num(s, "margin"),
        floor: num(s, "floor"),
        bias: num(s, "bias"),
    }
}

fn tab_named(doc: &Document, name: &str) -> TabId {
    doc.tabs
        .iter()
        .find(|t| t.name() == name)
        .unwrap_or_else(|| panic!("no tab {name}"))
        .id
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

fn rot(q: Vec2, deg: f64) -> Vec2 {
    let (s, c) = deg.to_radians().sin_cos();
    Vec2::new(q.x * c - q.y * s, q.x * s + q.y * c)
}

fn overlap(a: &Solid, b: &Solid) -> f64 {
    let (la, ha) = a.bounds().unwrap();
    let (lb, hb) = b.bounds().unwrap();
    if la.x > hb.x || lb.x > ha.x || la.y > hb.y || lb.y > ha.y || la.z > hb.z || lb.z > ha.z {
        return 0.0;
    }
    ok_brep::boolean(a, b, BoolOp::Intersection)
        .map(|s| s.volume().abs())
        .unwrap_or(0.0)
}

/// A section in the station's frame (across the sled, up) extruded
/// along the feed, as the build script makes every body.
fn prism(pts: &[Vec2], x0: f64, x1: f64) -> Solid {
    let p = Profile {
        outer: Loop::polygon(pts.to_vec()),
        holes: Vec::new(),
    };
    ok_brep::extrude(&p, &Plane::YZ, x0, x1, 1).unwrap()
}

#[test]
fn the_program_is_ten_rips_that_leave_the_tile_and_nothing_else() {
    let n = numbers();
    let s = settings(&n);
    let p = program(&s).unwrap();
    let stored = &n["program"];
    assert_eq!(p.cuts.len(), 10);
    assert_eq!(stored["cuts"].as_array().unwrap().len(), 10);
    assert!((p.height - num(stored, "height")).abs() < 1e-9);
    let through: Vec<usize> = p
        .cuts
        .iter()
        .filter(|c| c.kind == Kind::Through)
        .map(|c| c.face)
        .collect();
    assert_eq!(through, vec![2, 9]);
    let mut corners: Vec<usize> = p.cuts.iter().filter_map(|c| c.corner).collect();
    corners.dedup();
    assert_eq!(corners, vec![1, 4, 8, 13], "each corner's pair together");
    // The blank: the tile's thickness between faces 5 and 11 and its
    // width to face 6, plus the margin past the notched end.
    let unit = 1.5 * (1.0 + 3f64.sqrt());
    assert!(
        (p.thickness - (unit * s.edge - s.gap)).abs() < 1e-9,
        "{}",
        p.thickness
    );
    let reach = 3f64.sqrt() + 1.5; // vertex 6 along the 60° axis
                                   // Less the erosion: half the gap at the end face, more at the notched
                                   // end's convex corners.
    assert!(
        (p.width - (reach * s.edge + s.margin)).abs() < 2.0 * s.gap,
        "{}",
        p.width
    );
    let xs: Vec<f64> = p.tile.iter().map(|q| q.x).collect();
    assert!((xs.iter().cloned().fold(f64::INFINITY, f64::min) - s.margin).abs() < 1e-9);
    assert!((xs.iter().cloned().fold(0.0, f64::max) - p.width).abs() < 1e-9);
    let ys: Vec<f64> = p.tile.iter().map(|q| q.y).collect();
    assert!(ys.iter().cloned().fold(f64::INFINITY, f64::min).abs() < 1e-9);
    assert!((ys.iter().cloned().fold(0.0, f64::max) - p.thickness).abs() < 1e-9);
    for (i, c) in p.cuts.iter().enumerate() {
        let stored_cut = &stored["cuts"][i];
        let name = format!("cut {}", i + 1);
        println!(
            "{:2}. face {:2} {:?} turn {:3.0} bed {:5.2} offcuts {} thin {:.2}{}",
            i + 1,
            c.face,
            c.kind,
            c.turn,
            c.bed,
            c.offcuts.len(),
            c.thin,
            if c.symmetric { " symmetric" } else { "" }
        );
        assert_eq!(stored_cut["face"].as_u64().unwrap() as usize, c.face);
        assert!((num(stored_cut, "bed") - c.bed).abs() < 1e-9);
        assert!((c.turn / 30.0 - (c.turn / 30.0).round()).abs() < 1e-9);
        assert!(c.bed >= s.floor - 1e-9);
        assert!((c.top - p.height).abs() < 1e-9);
        // A rabbet's corner sits on the blade's near face, the bias under
        // its top, and the stock stands above it; a through cut's stock is
        // wholly under the blade's top.
        match c.corner {
            Some(v) => {
                let corner = rot(p.tile[v], c.turn) + c.shift;
                assert!(
                    corner.x.abs() < 1e-9,
                    "{name}: corner off the blade by {}",
                    corner.x
                );
                assert!((corner.y + s.bias - p.height).abs() < 1e-9, "{}", corner.y);
                let top = c.before.iter().map(|q| q.y).fold(0.0, f64::max);
                assert!(top > p.height, "a rabbet's stock stands above the blade");
                assert_eq!(c.offcuts.len(), if i % 2 == 0 { 0 } else { 1 });
            }
            None => {
                let top = c.before.iter().map(|q| q.y).fold(0.0, f64::max);
                assert!(top <= p.height + 1e-9);
                assert_eq!(c.offcuts.len(), 1);
            }
        }
        // Nothing comes off thinner than a sliver, and nothing is lost.
        assert!(c.thin > 2.0, "{}", c.thin);
        let taken: f64 = c.offcuts.iter().map(|o| signed_area(o)).sum();
        let kerf = signed_area(&c.before) - signed_area(&c.after) - taken;
        assert!(
            kerf > 0.0 && kerf < s.kerf * (p.width + p.thickness) * 2.0,
            "{kerf}"
        );
    }
    assert!(p.cuts[0].symmetric, "the blank is a rectangle");
    assert!(p.cuts[1..].iter().all(|c| !c.symmetric));
    // The section: the tile, less a kerf-wide nick the bias deep at
    // each cut of the two square corners (the 120° corners' overshoot
    // runs under the far face), and nothing left over.
    let nicks = 4.0 * s.kerf * s.bias;
    assert!(p.left < 1e-6, "{}", p.left);
    assert!(
        p.nicked > nicks && p.nicked < 1.25 * nicks,
        "{} vs {nicks}",
        p.nicked
    );
    let last = p.cuts.last().unwrap();
    assert!((signed_area(&last.after) - signed_area(&p.tile) + p.nicked).abs() < 1e-6);
    println!(
        "blank {:.2} x {:.2}, blade {:.2} over the plate, {:.2} mm2 nicked",
        p.width, p.thickness, p.height, p.nicked
    );
    // Short of the corner, the sliver appears.
    let short = program(&Settings { bias: 0.0, ..s }).unwrap();
    assert!(short.nicked < 1e-6 && short.left < 1e-6);
}

#[test]
fn every_cassette_holds_its_stick_with_the_lid_clear_of_the_blade() {
    let mut doc = load();
    let n = numbers();
    let s = settings(&n);
    let p = program(&s).unwrap();
    let length = num(&n, "length");
    let clear = num(&n, "clear");
    let wall = num(&n, "wall");
    let half = num(&n, "half_width");
    let heights = n["heights"].as_array().unwrap();
    for (i, c) in p.cuts.iter().enumerate() {
        let name = format!("Cut {}", i + 1);
        let r = regen(&mut doc, &name);
        let base = body(&r, "Base");
        let lid = body(&r, "Lid");
        let stick = body(&r, "Stick");
        let offcuts: Vec<&Solid> = (1..=c.offcuts.len())
            .map(|k| body(&r, &format!("Offcut {k}")))
            .collect();
        assert_eq!(r.bodies.len(), 3 + c.offcuts.len(), "{name}");
        // The stick and the offcuts are the program's sections over the
        // stick's length.
        assert!(
            (stick.volume() - signed_area(&c.after) * length).abs() < 1e-6 * stick.volume(),
            "{name}: {} vs {}",
            stick.volume(),
            signed_area(&c.after) * length
        );
        for (o, poly) in offcuts.iter().zip(&c.offcuts) {
            assert!(
                (o.volume() - signed_area(poly) * length).abs() < 1e-6 * o.volume(),
                "{name}: {} vs {}",
                o.volume(),
                signed_area(poly) * length
            );
        }
        // The cassette's box.
        let (lo, hi) = base.bounds().unwrap();
        assert!(
            (lo.y + half).abs() < 1e-6 && (hi.y - half).abs() < 1e-6,
            "{name}"
        );
        assert!((lo.x + wall).abs() < 1e-6 && (hi.x - length - wall).abs() < 1e-6);
        assert!((hi.z - heights[i].as_f64().unwrap()).abs() < 1e-6);
        // Nothing overlaps: stick, offcuts, base, lid.
        let mut wood = vec![stick];
        wood.extend(offcuts.iter().copied());
        for (k, w) in wood.iter().enumerate() {
            assert!(overlap(w, base) < 1e-6, "{name}: wood {k} in the base");
            assert!(overlap(w, lid) < 1e-6, "{name}: wood {k} in the lid");
            for w2 in &wood[k + 1..] {
                assert!(overlap(w, w2) < 1e-6, "{name}: the wood overlaps itself");
            }
        }
        assert!(overlap(base, lid) < 1e-6, "{name}: the lid is in the base");
        // The stick sits on the floor, the clearance over it.
        let (slo, _) = stick.bounds().unwrap();
        assert!(
            (slo.z - c.bed).abs() < 1e-6,
            "{name}: bed {} vs {}",
            slo.z,
            c.bed
        );
        // The blade's sweep over the stick's length: the kerf wide, from
        // the plate to its top. It never meets the lid or the wood that
        // stays, and meets the base only under the stick's lower envelope,
        // the slit it cuts on the first pass.
        let (x0, x1) = if c.waste_right {
            (0.0, s.kerf)
        } else {
            (-s.kerf, 0.0)
        };
        let sweep = prism(
            &[
                Vec2::new(x0, 0.0),
                Vec2::new(x1, 0.0),
                Vec2::new(x1, p.height),
                Vec2::new(x0, p.height),
            ],
            0.0,
            length,
        );
        assert!(
            overlap(&sweep, lid) < 1e-6,
            "{name}: the blade meets the lid"
        );
        for w in &wood {
            assert!(
                overlap(&sweep, w) < 1e-6,
                "{name}: the blade meets what stays"
            );
        }
        let slit = overlap(&sweep, base);
        // The floor under the kerf: the lower envelope less the clearance,
        // integrated across the kerf, capped at the blade's top.
        let under = |x: f64| -> f64 {
            let e = &c.lower;
            if x <= e[0].x || x >= e[e.len() - 1].x {
                return p.height;
            }
            let mut y = f64::INFINITY;
            for w in e.windows(2) {
                let (a, b) = (w[0], w[1]);
                if (a.x - x) * (b.x - x) <= 0.0 && (b.x - a.x).abs() > 1e-9 {
                    y = y.min(a.y + (b.y - a.y) * (x - a.x) / (b.x - a.x));
                }
            }
            (y - clear).min(p.height)
        };
        let steps = 2000;
        let dx = (x1 - x0) / steps as f64;
        let expected: f64 = (0..steps)
            .map(|k| under(x0 + (k as f64 + 0.5) * dx) * dx)
            .sum::<f64>()
            * length;
        println!(
            "{name}: slit {slit:.1} mm3 of the base against {expected:.1} under the stick; bed {:.2}",
            c.bed
        );
        assert!(
            (slit - expected).abs() < 0.01 * expected + 1.0,
            "{name}: {slit} vs {expected}"
        );
        assert!(
            slit < s.kerf * p.height * length,
            "{name}: the blade goes through the base"
        );
    }
    // The last stick is the tile.
    let r = regen(&mut doc, "Cut 10");
    let stick = body(&r, "Stick");
    let tile = signed_area(&p.tile);
    assert!(
        (stick.volume() - (tile - p.nicked) * length).abs() < 1e-6 * stick.volume(),
        "{} vs {}",
        stick.volume(),
        (tile - p.nicked) * length
    );
    let seq = regen(&mut doc, "Sequence");
    assert_eq!(
        seq.bodies
            .iter()
            .filter(|b| b.name.starts_with("Stick "))
            .count(),
        10
    );
    assert_eq!(
        seq.bodies
            .iter()
            .filter(|b| b.name.starts_with("Blade "))
            .count(),
        10
    );
}
