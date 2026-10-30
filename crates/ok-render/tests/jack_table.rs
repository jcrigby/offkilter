//! The scissor-jack router table (examples/jack-table): the router
//! plunged from retracted to its full 65 mm through the lever, at the
//! five poses the build writes to `out/poses.json`. At each one the jack
//! must close on itself, the link from the printed handle's tab must
//! land on the lever's pin, the lever must touch the saddle without
//! running into it, nothing else may touch, and the
//! crank's extension must run in its slot. Every number prints
//! (`--nocapture`); NOTE lines are findings, the requirements are asserted.

use ok_brep::{BoolOp, Solid};
use ok_math::Vec3;
use ok_model::{AssemblyResult, Document, InstanceId, MateId, TabId};
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/jack-table/out")
}

fn load() -> (Document, serde_json::Value) {
    let json = std::fs::read_to_string(dir().join("jack_table.okpart")).unwrap();
    let poses = std::fs::read_to_string(dir().join("poses.json")).unwrap();
    (
        Document::from_json(&json).unwrap(),
        serde_json::from_str(&poses).unwrap(),
    )
}

fn tab_named(doc: &Document, name: &str) -> TabId {
    doc.tabs
        .iter()
        .find(|t| t.name() == name)
        .unwrap_or_else(|| panic!("no tab {name}"))
        .id
}

fn body<'a>(r: &'a AssemblyResult, name: &str) -> &'a Solid {
    &r.bodies
        .iter()
        .find(|b| b.name == name)
        .unwrap_or_else(|| panic!("no body {name}"))
        .solid
}

fn lo(s: &Solid) -> Vec3 {
    s.bounds().unwrap().0
}

fn hi(s: &Solid) -> Vec3 {
    s.bounds().unwrap().1
}

fn overlap(a: &Solid, b: &Solid) -> f64 {
    let (alo, ahi) = a.bounds().unwrap();
    let (blo, bhi) = b.bounds().unwrap();
    if alo.x > bhi.x
        || blo.x > ahi.x
        || alo.y > bhi.y
        || blo.y > ahi.y
        || alo.z > bhi.z
        || blo.z > ahi.z
    {
        return 0.0;
    }
    match ok_brep::boolean(a, b, BoolOp::Intersection) {
        Ok(x) => x.volume(),
        Err(_) => f64::NAN,
    }
}

fn shifted(s: &Solid, dz: f64) -> Solid {
    s.transformed(&ok_brep::Transform::translation(Vec3::new(0.0, 0.0, dz)))
}

/// The jack's own parts ride on each other's pins and overlap at the
/// knuckles as nested channels do; the router motor's parts are one
/// rigid sub-assembly.
const JACK: [&str; 8] = [
    "jack foot",
    "upper front arm",
    "upper rear arm",
    "lower front arm",
    "lower rear arm",
    "saddle",
    "screw",
    "nut trunnion",
];

fn hits(r: &AssemblyResult) -> Vec<(String, String, f64)> {
    let mut out = Vec::new();
    for i in 0..r.bodies.len() {
        for j in i + 1..r.bodies.len() {
            let (a, b) = (&r.bodies[i], &r.bodies[j]);
            if r.placed[i] == r.placed[j]
                || (JACK.contains(&a.name.as_str()) && JACK.contains(&b.name.as_str()))
            {
                continue;
            }
            let v = overlap(&a.solid, &b.solid);
            if v.is_nan() || v > 1e-3 {
                out.push((a.name.clone(), b.name.clone(), v));
            }
        }
    }
    out
}

fn instance(doc: &Document, tab: TabId, name: &str) -> InstanceId {
    doc.assembly(tab)
        .unwrap()
        .instances
        .iter()
        .find(|i| i.name == name)
        .unwrap_or_else(|| panic!("no instance {name}"))
        .id
}

#[test]
fn the_lever_plunges_the_router_through_its_travel() {
    let (mut doc, poses) = load();
    let table = tab_named(&doc, "router table");
    let mates: Vec<(String, MateId)> = doc
        .assembly(table)
        .unwrap()
        .mates
        .iter()
        .map(|m| (m.name.clone(), m.id))
        .collect();
    let at = |doc: &mut Document, pose: &serde_json::Value| {
        let set: Vec<(MateId, f64, f64)> = mates
            .iter()
            .map(|(n, id)| {
                let m = &pose["mates"][n.as_str()];
                (
                    *id,
                    m["angle"].as_f64().unwrap(),
                    m["offset"].as_f64().unwrap(),
                )
            })
            .collect();
        doc.preview_assembly_at(table, &set).unwrap()
    };
    let upper_rear = instance(&doc, table, "upper rear arm");
    let lower_rear = instance(&doc, table, "lower rear arm");
    let link = instance(&doc, table, "link");
    let lever_id = instance(&doc, table, "lever");
    let link_lower = {
        let v = poses["link_lower"].as_array().unwrap();
        Vec3::new(
            v[0].as_f64().unwrap(),
            v[1].as_f64().unwrap(),
            v[2].as_f64().unwrap(),
        )
    };
    let positions = poses["positions"].as_array().unwrap();
    assert_eq!(positions.len(), 5);

    // In the drawn pose the nut trunnion sits on the rear knuckle, where
    // both rear arms were drawn to meet.
    let r0 = at(&mut doc, &positions[0]);
    let knuckle = {
        let s = body(&r0, "nut trunnion");
        (lo(s) + hi(s)) * 0.5
    };
    let top_under = lo(body(&r0, "top")).z;
    let table_z = hi(body(&r0, "top")).z;
    let front = body(&r0, "front").clone();
    let bit0 = hi(body(&r0, "router motor / bit")).z;
    let motor0 = lo(body(&r0, "router motor / router motor")).z;

    let mut failures = Vec::new();
    let mut must = |ok: bool, what: String| {
        eprintln!("{} {what}", if ok { "PASS" } else { "FAIL" });
        if !ok {
            failures.push(what);
        }
    };
    let mut screw_h = None;
    for pose in positions {
        let plunge = pose["plunge"].as_f64().unwrap();
        let jack = pose["jack"].as_f64().unwrap();
        let r = at(&mut doc, pose);
        assert!(r.mate_errors.is_empty(), "{plunge}: {:?}", r.mate_errors);
        assert!(
            r.instance_errors.is_empty(),
            "{plunge}: {:?}",
            r.instance_errors
        );
        let tag = format!("plunged {plunge:.1} mm");

        // The router rises by the plunge.
        let bit = hi(body(&r, "router motor / bit")).z;
        must(
            (bit - bit0 - plunge).abs() < 0.01
                && (lo(body(&r, "router motor / router motor")).z - motor0 - plunge).abs() < 0.01,
            format!("{tag}: the bit is {:.1} mm above the table", bit - table_z),
        );

        // The jack closes: both rear arms reach the same knuckle, the
        // screw stays level, the saddle hangs the jack's height down.
        let pu = r.transforms[&upper_rear].apply_point(knuckle);
        let pl = r.transforms[&lower_rear].apply_point(knuckle);
        must(
            (pu - pl).length() < 0.05,
            format!(
                "{tag}: the rear arms meet at their knuckle ({:.3} mm apart)",
                (pu - pl).length()
            ),
        );
        let screw = body(&r, "screw");
        let h = hi(screw).z - lo(screw).z;
        must(
            screw_h.is_none_or(|s: f64| (s - h).abs() < 0.01),
            format!("{tag}: the screw stays level"),
        );
        screw_h = Some(h);
        let saddle = body(&r, "saddle");
        must(
            (top_under - lo(saddle).z - jack).abs() < 0.05,
            format!(
                "{tag}: the jack stands {:.1} mm ({:.2} in) from foot to saddle",
                top_under - lo(saddle).z,
                jack / 25.4
            ),
        );
        must(
            (3.375 * 25.4 + 10.0..15.125 * 25.4 - 10.0).contains(&jack),
            format!("{tag}: the jack is inside its 3-3/8 to 15-1/8 in range"),
        );

        // The link's lower hole, carried by the link from the handle's
        // tab, lands on the lever's pin, carried by the lever: the loop
        // the mates leave open closes.
        let a = r.transforms[&link].apply_point(link_lower);
        let b = r.transforms[&lever_id].apply_point(link_lower);
        must(
            (a - b).length() < 0.05,
            format!(
                "{tag}: the link's lower hole is on the lever's pin ({:.3} mm apart)",
                (a - b).length()
            ),
        );

        // The lever touches the saddle's lips (its pin): within 0.3 mm,
        // and not into them.
        let lever = body(&r, "lever");
        must(
            overlap(&shifted(saddle, -0.3), lever) > 1e-3,
            format!("{tag}: the lever is on the saddle"),
        );

        // Nothing else touches.
        let bad = hits(&r);
        must(bad.is_empty(), format!("{tag}: no interference: {bad:?}"));

        // Above the bench, and the crank out through the front.
        let low = [
            ("lever", lo(lever).z),
            ("saddle", lo(saddle).z),
            ("router", lo(body(&r, "router motor / router motor")).z),
        ];
        for (n, z) in low {
            must(
                z > 5.0,
                format!("{tag}: the {n} is {z:.1} mm over the bench"),
            );
        }
        let out = lo(&front).y - lo(screw).y;
        must(
            out > 50.0,
            format!("{tag}: the extension stands {out:.0} mm out of the front"),
        );
        eprintln!(
            "NOTE {tag}: {:.2} turns per mm of bit ({:.2} mm a turn), arms at {:.1} degrees",
            pose["turns_per_mm_bit"].as_f64().unwrap(),
            1.0 / pose["turns_per_mm_bit"].as_f64().unwrap(),
            pose["theta"].as_f64().unwrap()
        );
    }
    let first = positions[0]["jack"].as_f64().unwrap();
    let last = positions[4]["jack"].as_f64().unwrap();
    must(
        ((last - first) / 65.0 - 2.0).abs() < 0.02,
        format!(
            "the jack moves {:.1} mm for the router's 65: 2 to 1 within 1 %",
            last - first
        ),
    );
    eprintln!(
        "NOTE {:.0} turns of the crank from retracted to fully plunged",
        poses["total_turns"].as_f64().unwrap()
    );
    assert!(failures.is_empty(), "{failures:#?}");
}
