//! Scans in two poses merged. The case a second pose is for: a
//! mushroom's cap has a flat underside that no camera above the table
//! sees from any angle, so one scan bridges the stem to the rim with
//! a cone; scanned again on its side the underside is in plain view,
//! and registered onto the first scan the two together give the
//! whole shape. And the gourd upside down, which a camera above sees
//! whole either way, merging without harm to the profile.

use ok_photo::register::{merge, Placement};
use ok_photo::scan::{cloud_mesh, scan, ScanOptions};
use ok_photo::simulate::{distance_to_mesh, on_side, turntable, upside_down, Gourd, Mushroom, Rig};
use ok_photo::{Field, SheetSize};
use ok_render::{Image, Rgb};

const AXIS: [f64; 2] = [210.0, 150.0];

fn rig() -> Rig {
    Rig {
        focal: 1300.0,
        width: 1400,
        height: 1050,
        distance: 650.0,
        elevation: 22.0,
        laser_azimuth: 35.0,
        laser_distance: 650.0,
        laser_elevation: 30.0,
        axis: AXIS,
        look_height: 55.0,
    }
}

fn render(tris: &[[[f64; 3]; 3]], frames: usize) -> Vec<Image> {
    let field = Field::new(SheetSize::Letter, 2, 2).unwrap();
    turntable(&field, tris, &rig(), frames, Rgb(215, 150, 70))
}

/// The example's git-ignored frame directories, beside the gourd's.
fn save_frames(name: &str, frames: &[Image]) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/turntable-scan")
        .join(name);
    std::fs::create_dir_all(&dir).unwrap();
    for (k, f) in frames.iter().enumerate() {
        std::fs::write(dir.join(format!("{k:04}.png")), ok_render::to_png(f)).unwrap();
    }
}

/// Distances from a mesh's profile points to the true surface: median
/// and 90th percentile, mm.
fn profile_error(profiles: &[Vec<[f64; 3]>], truth: &[[[f64; 3]; 3]]) -> (f64, f64) {
    let mut errs: Vec<f64> = profiles
        .iter()
        .flatten()
        .map(|p| distance_to_mesh(*p, truth))
        .collect();
    errs.sort_by(f64::total_cmp);
    (errs[errs.len() / 2], errs[errs.len() * 9 / 10])
}

/// The mushroom: the example's shape for the merge, so its numbers
/// are shared with `example_scan.rs`.
pub fn mushroom() -> Mushroom {
    Mushroom {
        base: AXIS,
        height: 100.0,
        stem: 18.0,
        cap: 48.0,
    }
}

#[test]
fn a_mushroom_needs_its_side_scanned_to_show_what_is_under_the_cap() {
    let m = mushroom();
    let tris = m.mesh(40, 72);
    let n = 36;
    let up_frames = render(&tris, n);
    save_frames("frames-mushroom", &up_frames);
    // On its side: rolled over about a horizontal line and set down
    // a little off the axis, turned.
    let side = on_side(&tris, 30.0, 75.0, [AXIS[0] + 4.0, AXIS[1] - 6.0]);
    let side_frames = render(&side, n);
    save_frames("frames-mushroom-side", &side_frames);
    let up = scan(&up_frames, &ScanOptions::default()).unwrap();
    let on_its_side = scan(&side_frames, &ScanOptions::default()).unwrap();
    let (cloud, regs) = merge(&[
        (up.points.clone(), Placement::Same),
        (on_its_side.points.clone(), Placement::OnSide),
    ])
    .unwrap();
    let r = &regs[0];
    let both = cloud_mesh(&cloud, n, 0.0, 1.0).unwrap();
    let (one_med, one_p90) = profile_error(&up.profiles, &tris);
    let (two_med, two_p90) = profile_error(&both.profiles, &tris);
    let truth = m.volume();
    eprintln!(
        "mushroom: one scan volume {:.0} ({:.1} % of {truth:.0}), profile error median {one_med:.2} p90 {one_p90:.2}; on its side registered to {:.3} mm, overlap {:.2}, turned {:.1} degrees, {} seeds; merged volume {:.0} ({:.1} %), profile error median {two_med:.2} p90 {two_p90:.2}",
        up.volume, 100.0 * up.volume / truth, r.residual, r.overlap, r.transform.angle(), r.seeds, both.volume, 100.0 * both.volume / truth
    );
    // One scan: the cone under the cap adds volume and the profile is
    // off by the cone where the underside should be.
    assert!(
        up.volume > 1.06 * truth,
        "one scan volume {} of {truth}: the cap's underside was seen",
        up.volume
    );
    assert!(
        one_p90 > 3.0,
        "one scan profile p90 {one_p90}: the cap's underside was seen"
    );
    // Two: registered, and the underside in.
    assert!(r.residual < 0.4, "residual {}", r.residual);
    // (The rotation's total angle is not 90: a roll about one
    // horizontal axis then a turn about the vertical compose to more,
    // and a mushroom is round, so the turn is whatever ICP settled on.)
    assert!(
        (both.volume - truth).abs() / truth < 0.03,
        "merged volume {} vs {truth}",
        both.volume
    );
    assert!(
        two_med < 0.4 && two_p90 < 1.5,
        "merged profile error median {two_med} p90 {two_p90}"
    );
}

#[test]
fn the_gourd_upside_down_merges_without_harm() {
    let gourd = Gourd {
        base: AXIS,
        height: 120.0,
        lobes: 5,
        lobe: 0.06,
    };
    let tris = gourd.mesh(60, 72);
    let n = 36;
    let up_frames = render(&tris, n);
    let flipped = upside_down(&tris, 50.0, 160.0, [214.0, 147.0]);
    let down_frames = render(&flipped, n);
    let up = scan(&up_frames, &ScanOptions::default()).unwrap();
    let down = scan(&down_frames, &ScanOptions::default()).unwrap();
    let (cloud, regs) = merge(&[
        (up.points.clone(), Placement::Same),
        (down.points.clone(), Placement::UpsideDown),
    ])
    .unwrap();
    let r = &regs[0];
    let both = cloud_mesh(&cloud, n, 0.0, 1.0).unwrap();
    let (one_med, one_p90) = profile_error(&up.profiles, &tris);
    let (two_med, two_p90) = profile_error(&both.profiles, &tris);
    eprintln!(
        "gourd: registered upside down to {:.3} mm, overlap {:.2}, turned {:.1} degrees; profile error one scan median {one_med:.2} p90 {one_p90:.2}, merged {two_med:.2} p90 {two_p90:.2}; volume one scan {:.0}, merged {:.0}, true {:.0}",
        r.residual, r.overlap, r.transform.angle(), up.volume, both.volume, gourd.volume()
    );
    assert!(r.residual < 0.4, "residual {}", r.residual);
    assert!(r.overlap > 0.8, "overlap {}", r.overlap);
    assert!(
        (r.transform.angle() - 180.0).abs() < 1.0,
        "turned {} degrees, not over",
        r.transform.angle()
    );
    assert!(
        two_med < 0.4 && two_p90 < 1.2,
        "merged profile error median {two_med} p90 {two_p90}"
    );
    assert!(
        (both.volume - gourd.volume()).abs() <= (up.volume - gourd.volume()).abs() + 500.0,
        "the merge made the volume worse: {} vs {} (true {})",
        both.volume,
        up.volume,
        gourd.volume()
    );
}
