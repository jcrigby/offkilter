//! The finch feeder cap with a perch ring (examples/finch-perch) as a
//! regression suite: the cap is one closed body that holds everything
//! the plain cap (examples/finch-plug) does, plus a ring of perch of a
//! small bird's size all the way round on three spokes, standing off
//! the mesh tube by a finch's room and not a dove's, with nothing to
//! stand on between the hooks and the ring; and on the collar it meets
//! the ring only by the hooks' bite, locked, and nowhere offered up.
//! Every number prints (`--nocapture`).

use ok_brep::{BoolOp, Solid, Transform};
use ok_math::Vec3;
use ok_model::{Document, TabId};
use std::path::PathBuf;

fn examples() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

fn load(example: &str, file: &str) -> Document {
    let json = std::fs::read_to_string(examples().join(example).join("out").join(file)).unwrap();
    Document::from_json(&json).unwrap()
}

fn numbers() -> serde_json::Value {
    serde_json::from_str(
        &std::fs::read_to_string(examples().join("finch-perch/out/perch.json")).unwrap(),
    )
    .unwrap()
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

fn overlap(a: &Solid, b: &Solid) -> f64 {
    ok_brep::boolean(a, b, BoolOp::Intersection)
        .map(|s| s.volume().abs())
        .unwrap_or(0.0)
}

fn vertices(s: &Solid) -> Vec<Vec3> {
    s.polygons()
        .iter()
        .flat_map(|p| p.loops.iter().flatten().copied().collect::<Vec<_>>())
        .collect()
}

fn radius(v: &Vec3) -> f64 {
    (v.x * v.x + v.y * v.y).sqrt()
}

fn angle_of(v: &Vec3) -> f64 {
    v.y.atan2(v.x).to_degrees().rem_euclid(360.0)
}

fn cap_of(doc: &mut Document) -> Solid {
    let r = doc.regenerate_studio(tab_named(doc, "cap"), None).unwrap();
    assert!(
        r.statuses.iter().all(|f| f.error.is_none()),
        "{:?}",
        r.statuses
    );
    assert_eq!(r.bodies.len(), 1, "one cap");
    let body = r.bodies[0].solid.clone();
    body.validate().unwrap();
    body
}

#[test]
fn the_perch_ring_rounds_the_cap_at_a_finch_s_distance_and_the_cap_is_still_the_cap() {
    let mut doc = load("finch-perch", "finch_perch.okpart");
    let n = numbers();
    let (collar, cap, perch) = (&n["collar"], &n["cap"], &n["perch"]);
    let body = cap_of(&mut doc);
    let (lo, hi) = body.bounds().unwrap();
    let (plate_t, r_wall1, r_ear, r_lip, hook_arc) = (
        num(cap, "plate_t"),
        num(cap, "r_wall1"),
        num(cap, "r_ear"),
        num(cap, "r_lip"),
        num(cap, "hook_arc"),
    );
    let (r, w, h, r_in, r_out, tube_r, stand_off) = (
        num(perch, "r"),
        num(perch, "w"),
        num(perch, "h"),
        num(perch, "r_in"),
        num(perch, "r_out"),
        num(perch, "tube_r"),
        num(perch, "stand_off"),
    );
    let (spoke_w, spoke_t, spoke_r0) = (
        num(perch, "spoke_w"),
        num(perch, "spoke_t"),
        num(perch, "spoke_r0"),
    );
    let spokes: Vec<f64> = perch["spoke_at"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    println!(
        "cap with perch {:.1} across, z {:.1} to {:.1}, {:.1} cm3; perch {w} x {h} at {r}, {stand_off:.2} off the tube",
        hi.x - lo.x,
        lo.z,
        hi.z,
        body.volume() / 1000.0
    );
    // The perch: a small bird's, all the way round, its flat bottom level
    // with the plate's so the cap prints flat, its round top the perch.
    assert!(
        (4.0..=7.0).contains(&w),
        "a pencil of a perch, not a dowel for a dove"
    );
    assert!((r_in - r + w / 2.0).abs() < 1e-9 && (r_out - r - w / 2.0).abs() < 1e-9);
    assert!(
        ((hi.x - lo.x) - 2.0 * r_out).abs() < 0.3 && ((hi.y - lo.y) - 2.0 * r_out).abs() < 0.3,
        "the ring sets the width"
    );
    assert!(
        (lo.z + plate_t).abs() < 1e-6,
        "flat on the bed with the plate"
    );
    let verts = vertices(&body);
    let bins = 36;
    for b in 0..bins {
        let (a0, a1) = (
            b as f64 * 360.0 / bins as f64,
            (b + 1) as f64 * 360.0 / bins as f64,
        );
        let in_bin = |v: &&Vec3| {
            let a = angle_of(v);
            a >= a0 && a < a1
        };
        let outer = verts.iter().filter(in_bin).map(radius).fold(0.0, f64::max);
        assert!(
            (outer - r_out).abs() < 0.3,
            "the ring reaches {outer:.2} between {a0} and {a1} degrees"
        );
        let top = verts
            .iter()
            .filter(in_bin)
            .filter(|v| (radius(v) - r).abs() < 0.6)
            .map(|v| v.z)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(
            (top - (h - plate_t)).abs() < 0.15,
            "the perch's top at {top:.2} between {a0} and {a1} degrees"
        );
    }
    // Where it stands: off the mesh tube by a finch's room and not a
    // dove's, clear of the hooks, under 110 across, and nothing to stand
    // on between the hooks and the ring but the three spokes.
    assert!((stand_off - (r_in - tube_r)).abs() < 1e-9);
    assert!(
        (12.0..=20.0).contains(&stand_off),
        "the ring stands {stand_off} off the tube: a finch has room, a dove's chest is in the mesh"
    );
    assert!(r_in > r_wall1 + 3.0, "the hooks are clear of the ring");
    assert!(2.0 * r_out <= 110.0, "{} across: small", 2.0 * r_out);
    assert_eq!(spokes.len(), 3);
    for v in verts
        .iter()
        .filter(|v| radius(v) > r_wall1 + 0.5 && radius(v) < r_in - 0.5)
    {
        let half = (spoke_w / 2.0 / radius(v)).asin().to_degrees() + 0.5;
        assert!(
            spokes
                .iter()
                .any(|s| ((angle_of(v) - s + 180.0).rem_euclid(360.0) - 180.0).abs() <= half),
            "something at {:.1} from the axis, {:.1} degrees, between the hooks and the ring",
            radius(v),
            angle_of(v)
        );
        assert!(
            v.z <= spoke_t - plate_t + 1e-6,
            "a spoke higher than its thickness"
        );
    }
    // Added to what we already have: every sample point inside the plain
    // cap is inside this one, and the volume grew by the ring and no more
    // than the ring and the spokes' full bars.
    let mut plain_doc = load("finch-plug", "finch_plug.okpart");
    let plain = cap_of(&mut plain_doc);
    let (plo, phi) = plain.bounds().unwrap();
    let step = 3.0;
    let (mut inside, mut kept) = (0usize, 0usize);
    let mut z = plo.z + step / 2.0;
    while z < phi.z {
        let mut x = plo.x + step / 2.0;
        while x < phi.x {
            let mut y = plo.y + step / 2.0;
            while y < phi.y {
                let p = Vec3::new(x, y, z);
                if plain.contains(p) {
                    inside += 1;
                    if body.contains(p) {
                        kept += 1;
                    }
                }
                y += step;
            }
            x += step;
        }
        z += step;
    }
    println!("{kept} of {inside} points of the plain cap are in the cap with perch");
    assert!(
        inside > 200 && kept == inside,
        "the plain cap is not all there: {kept} of {inside}"
    );
    let (ring_vol, plain_vol) = (num(perch, "ring_volume"), num(perch, "plain_volume"));
    assert!((plain.volume() - plain_vol).abs() < 1.0);
    let added = body.volume() - plain.volume();
    let spokes_full = spokes.len() as f64 * spoke_w * spoke_t * (r - spoke_r0);
    println!(
        "added {:.1} cm3: the ring {:.1}, the spokes at most {:.1}",
        added / 1000.0,
        ring_vol / 1000.0,
        spokes_full / 1000.0
    );
    assert!(
        added > 0.98 * ring_vol && added < ring_vol + spokes_full,
        "added {added:.0} mm3 for a ring of {ring_vol:.0} and spokes of at most {spokes_full:.0}"
    );
    // On the collar, locked, the only meeting is the hooks' bite, as for
    // the plain cap; offered up, turned by the gap, nothing meets.
    let c = doc
        .regenerate_studio(tab_named(&doc, "collar"), None)
        .unwrap();
    assert_eq!(c.bodies.len(), 1);
    let ring = &c.bodies[0].solid;
    let ramp: Vec<f64> = cap["ramp"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    let bite_max: f64 = ramp
        .iter()
        .filter(|c| **c < 0.0)
        .map(|c| {
            -c * (hook_arc / ramp.len() as f64).to_radians() / 2.0
                * (r_ear * r_ear - r_lip * r_lip)
                * spokes.len() as f64
        })
        .sum();
    let hit = overlap(&body, ring);
    println!("locked: {hit:.3} mm3 overlap, the bite at most {bite_max:.3}");
    assert!(
        hit > 0.3 * bite_max && hit < bite_max + 1e-3,
        "the cap meets the collar locked other than by the bite: {hit} mm3 of at most {bite_max}"
    );
    let offered = body.transformed(&Transform::rotation(
        Vec3::ZERO,
        Vec3::Z,
        num(cap, "offer").to_radians(),
    ));
    let hit = overlap(&offered, ring);
    println!("offered up: {hit:.3} mm3 overlap");
    assert!(
        hit < 1e-3,
        "the ears do not pass between the hooks: {hit} mm3"
    );
    assert!(
        r_in > num(collar, "od") / 2.0 + num(collar, "ear_r") + 5.0,
        "the ears turn well inside the ring"
    );
    for name in ["cap on collar", "cap offered up"] {
        let asm = doc.regenerate_assembly(tab_named(&doc, name)).unwrap();
        assert_eq!(asm.bodies.len(), 2);
        assert!(asm.instance_errors.is_empty());
    }
}
