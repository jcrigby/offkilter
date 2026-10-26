//! The turntable scanner, closed loop: a lobed gourd of known shape is
//! rendered on the field going round under a camera and a line laser,
//! the frames are scanned, and the scan is held against the shape.

use ok_photo::scan::{scan, ScanOptions};
use ok_photo::simulate::{turntable, Gourd, Rig};
use ok_photo::{Field, SheetSize};
use ok_render::Rgb;

/// Where the frames go: the example's git-ignored `frames/`, which its
/// build script encodes into the video it scans, or `OK_SCAN_OUT`.
fn frames_dir() -> std::path::PathBuf {
    let dir = match std::env::var("OK_SCAN_OUT") {
        Ok(d) => std::path::PathBuf::from(d),
        Err(_) => std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/turntable-scan/frames"),
    };
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_gourd_scans_to_its_shape() {
    let field = Field::new(SheetSize::Letter, 2, 2).unwrap();
    let gourd = Gourd {
        base: [210.0, 150.0],
        height: 120.0,
        lobes: 5,
        lobe: 0.06,
    };
    let tris = gourd.mesh(60, 72);
    let rig = Rig {
        focal: 1300.0,
        width: 1400,
        height: 1050,
        distance: 650.0,
        elevation: 22.0,
        laser_azimuth: 35.0,
        laser_distance: 650.0,
        laser_elevation: 30.0,
        axis: [210.0, 150.0],
        look_height: 55.0,
    };
    let n = 36;
    let t0 = std::time::Instant::now();
    let frames = turntable(&field, &tris, &rig, n, Rgb(215, 150, 70));
    eprintln!("rendered {n} frames in {:.1} s", t0.elapsed().as_secs_f64());
    let dir = frames_dir();
    for (k, f) in frames.iter().enumerate() {
        std::fs::write(dir.join(format!("{k:04}.png")), ok_render::to_png(f)).unwrap();
    }
    let t0 = std::time::Instant::now();
    let s = scan(&frames, &ScanOptions::default()).unwrap();
    eprintln!("scanned in {:.1} s", t0.elapsed().as_secs_f64());
    std::fs::write(dir.join("../out/scan-frame-test.png"), &s.picture).ok();
    eprintln!(
        "frames used {} of {}; focal {:.1} (true {}); laser angle {:.1}, fit {:.3} mm over {} px; axis ({:.2}, {:.2}); {} points, {} triangles, volume {:.0} (true {:.0}); warnings {:?}",
        s.frames_used, n, s.focal, rig.focal, s.laser_angle, s.laser_fit, s.laser_points, s.axis[0], s.axis[1],
        s.points.len(), s.triangle_count, s.volume, gourd.volume(), s.warnings
    );
    for f in s.frames.iter().take(4) {
        eprintln!("  frame {}: marks {} fit {:.2} focal {:?} cam ({:.0},{:.0},{:.0}) az {:.1} line {} paper {} points {} {:?}", f.index, f.marks, f.fit, f.focal.map(|v| v.round()), f.camera[0], f.camera[1], f.camera[2], f.azimuth, f.line_pixels, f.on_paper, f.points, f.skipped);
    }
    // Every point against the gourd's surface at its height and azimuth.
    let mut errs: Vec<f64> = s
        .points
        .iter()
        .map(|p| {
            let (dx, dy) = (p[0] - gourd.base[0], p[1] - gourd.base[1]);
            let r = (dx * dx + dy * dy).sqrt();
            let phi = dy.atan2(dx);
            (r - gourd.radius(p[2].min(gourd.height), phi)).abs()
        })
        .collect();
    errs.sort_by(f64::total_cmp);
    let (med, p90, max) = (
        errs[errs.len() / 2],
        errs[errs.len() * 9 / 10],
        errs[errs.len() - 1],
    );
    eprintln!("radial error: median {med:.3} 90% {p90:.3} max {max:.3} mm");
    // The gourd's volume over the heights the scan covered.
    let (z0, z1) = (s.bounds[2], s.bounds[5]);
    let partial = {
        let (nz, nphi) = (400, 360);
        let mut v = 0.0;
        for iz in 0..nz {
            let z = z0 + (z1 - z0) * (iz as f64 + 0.5) / nz as f64;
            for ip in 0..nphi {
                let phi = 2.0 * std::f64::consts::PI * (ip as f64 + 0.5) / nphi as f64;
                v += 0.5
                    * gourd.radius(z, phi).powi(2)
                    * (2.0 * std::f64::consts::PI / nphi as f64)
                    * ((z1 - z0) / nz as f64);
            }
        }
        v
    };
    eprintln!("scanned z {z0:.1} to {z1:.1}: mesh volume {:.0}, gourd over that {partial:.0}, whole gourd {:.0}", s.volume, gourd.volume());
    for (k, prof) in s.profiles.iter().enumerate().take(6) {
        let zs: Vec<f64> = prof.iter().map(|p| p[2]).collect();
        let gaps = zs.windows(2).filter(|w| w[1] - w[0] > 1.6).count();
        eprintln!(
            "  profile {k}: {} bins, z {:.1} to {:.1}, {gaps} gaps",
            prof.len(),
            zs.first().unwrap_or(&0.0),
            zs.last().unwrap_or(&0.0)
        );
    }
    let o = [gourd.base[0], gourd.base[1], 60.0];
    let rel = |p: [f64; 3]| [p[0] - o[0], p[1] - o[1], p[2] - o[2]];
    let signed: f64 = s
        .triangles
        .iter()
        .map(|t| {
            let (a, b, c) = (rel(t[0]), rel(t[1]), rel(t[2]));
            let cr = [
                b[1] * c[2] - b[2] * c[1],
                b[2] * c[0] - b[0] * c[2],
                b[0] * c[1] - b[1] * c[0],
            ];
            (a[0] * cr[0] + a[1] * cr[1] + a[2] * cr[2]) / 6.0
        })
        .sum();
    let flipped = s
        .triangles
        .iter()
        .filter(|t| {
            let (a, b, c) = (rel(t[0]), rel(t[1]), rel(t[2]));
            let n = [
                (b[1] - a[1]) * (c[2] - a[2]) - (b[2] - a[2]) * (c[1] - a[1]),
                (b[2] - a[2]) * (c[0] - a[0]) - (b[0] - a[0]) * (c[2] - a[2]),
                (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]),
            ];
            let m = [
                (a[0] + b[0] + c[0]) / 3.0,
                (a[1] + b[1] + c[1]) / 3.0,
                (a[2] + b[2] + c[2]) / 3.0,
            ];
            n[0] * m[0] + n[1] * m[1] + n[2] * m[2] < 0.0
        })
        .count();
    eprintln!(
        "signed volume about the axis {signed:.0}; {flipped} of {} triangles face inward",
        s.triangles.len()
    );
    assert_eq!(s.frames_used, n, "every frame read the field");
    assert!(
        (s.focal - rig.focal).abs() / rig.focal < 0.015,
        "focal {}",
        s.focal
    );
    assert!(
        (s.axis[0] - 210.0).abs() < 1.0 && (s.axis[1] - 150.0).abs() < 1.0,
        "axis {:?}",
        s.axis
    );
    assert!(
        med < 0.3 && p90 < 0.8,
        "radial error median {med:.3}, 90% {p90:.3}"
    );
    assert!(
        (s.volume - partial).abs() / partial < 0.02,
        "volume {} vs {partial} over the scanned heights",
        s.volume
    );
    assert!(
        (s.volume - gourd.volume()).abs() / gourd.volume() < 0.03,
        "volume {} vs {}",
        s.volume,
        gourd.volume()
    );
    assert!(s.warnings.is_empty(), "{:?}", s.warnings);
}
