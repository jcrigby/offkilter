//! The Platonic solids and a bucky ball through the scanner: each
//! rendered standing on a face and scanned once, every point held
//! against the true surface and the mesh's volume against the true
//! one. Then the scans that merge: the same solid rolled onto a side,
//! turned over, set down elsewhere, or seen from a higher camera,
//! registered onto the first scan by what was done to it. A camera a
//! little above the table sees every face of these solids, even the
//! ones leaning out over it, so each scans whole in one pass; the
//! merges here check the registration, and `merge.rs` has the case a
//! second pose is for.

use ok_photo::faces::{fit_faces, FaceOptions};
use ok_photo::register::{merge, Placement, Registration};
use ok_photo::scan::{cloud_mesh, scan, Scan, ScanOptions};
use ok_photo::simulate::{
    distance_to_mesh, mesh_volume, on_side, radius_at, rotation, settle, transform, turntable,
    upside_down, Rig, Solid,
};
use ok_photo::{Field, SheetSize};
use ok_render::{Image, Rgb};

pub const AXIS: [f64; 2] = [210.0, 150.0];
/// The solids' height and the turn they stand at, as the example
/// renders them too.
pub const HEIGHT: f64 = 100.0;
pub const TURN: f64 = 13.0;
const FRAMES: usize = 24;

fn rig(elevation: f64) -> Rig {
    Rig {
        focal: 1300.0,
        width: 1400,
        height: 1050,
        distance: 650.0,
        elevation,
        laser_azimuth: 35.0,
        laser_distance: 650.0,
        laser_elevation: 30.0,
        axis: AXIS,
        look_height: HEIGHT / 2.0,
    }
}

fn render(tris: &[[[f64; 3]; 3]], elevation: f64, frames: usize) -> Vec<Image> {
    let field = Field::new(SheetSize::Letter, 2, 2).unwrap();
    turntable(&field, tris, &rig(elevation), frames, Rgb(200, 160, 90))
}

/// Frames per turn: the bucky ball, with thirty-two faces each 22 mm
/// across, gets twice as many so each face holds enough laser lines
/// for its plane to come out within half a degree, through the
/// example's video too.
fn frames_for(solid: Solid) -> usize {
    if solid == Solid::Bucky {
        48
    } else {
        FRAMES
    }
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

struct Fit {
    median: f64,
    p90: f64,
    volume: f64,
    /// The true solid's volume as the same profiles would give it: a
    /// polygon per slice through the true surface at the profiles'
    /// own azimuths, so the chords a polygon cuts off a corner count
    /// against neither.
    sampled_volume: f64,
    true_volume: f64,
    covered: f64,
}

/// A cloud against the true surface: the distance of each of its
/// points to the mesh, the volume of the mesh built through it, and
/// the share of the height that mesh spans.
fn fit(points: &[[f64; 3]], truth: &[[[f64; 3]; 3]], sectors: usize) -> Fit {
    let mut ds: Vec<f64> = points.iter().map(|p| distance_to_mesh(*p, truth)).collect();
    ds.sort_by(f64::total_cmp);
    let m = cloud_mesh(points, sectors, 0.0, 1.0).unwrap();
    let zs: Vec<f64> = m.profiles.iter().flatten().map(|p| p[2]).collect();
    let (lo, hi) = zs
        .iter()
        .fold((f64::MAX, f64::MIN), |(l, h), z| (l.min(*z), h.max(*z)));
    // The reference: the true surface point at every measured cell's
    // azimuth and height, meshed the same way, so the same cells are
    // filled across the same gaps.
    let c = m.centre;
    let ideal: Vec<[f64; 3]> = m
        .profiles
        .iter()
        .zip(&m.measured)
        .flat_map(|(prof, meas)| {
            prof.iter().zip(meas).filter(|(_, m)| **m).map(|(p, _)| {
                let az = (p[1] - c[1]).atan2(p[0] - c[0]).to_degrees();
                let r = radius_at(truth, c, p[2].clamp(0.01, HEIGHT - 0.01), az).unwrap_or(0.0);
                [
                    c[0] + r * az.to_radians().cos(),
                    c[1] + r * az.to_radians().sin(),
                    p[2],
                ]
            })
        })
        .collect();
    let sampled_volume = cloud_mesh(&ideal, sectors, 0.0, 1.0).unwrap().volume;
    Fit {
        median: ds[ds.len() / 2],
        p90: ds[ds.len() * 9 / 10],
        volume: m.volume,
        sampled_volume,
        true_volume: mesh_volume(truth),
        covered: (hi - lo) / HEIGHT,
    }
}

/// The cloud's flat faces fitted and closed into a solid, held
/// against the textbook: the counts of faces, edges and corners, every
/// dihedral angle, every edge's length, every corner's position, and
/// the volume. This is what "recognisable as a cube" means.
fn faces_check(solid: Solid, tris: &[[[f64; 3]; 3]], points: &[[f64; 3]], label: &str) {
    let (faces, edges, vertices, dihedrals): (usize, usize, usize, Vec<f64>) = match solid {
        Solid::Tetrahedron => (4, 6, 4, vec![70.5288]),
        Solid::Cube => (6, 12, 8, vec![90.0]),
        Solid::Octahedron => (8, 12, 6, vec![109.4712]),
        Solid::Dodecahedron => (12, 30, 20, vec![116.5651]),
        Solid::Icosahedron => (20, 30, 12, vec![138.1897]),
        Solid::Bucky => (32, 90, 60, vec![138.1897, 142.6226]),
    };
    // The true corners and edge length from the mesh.
    let mut truth: Vec<[f64; 3]> = Vec::new();
    for p in tris.iter().flatten() {
        if !truth.iter().any(|q| dist(*q, *p) < 1e-6) {
            truth.push(*p);
        }
    }
    let edge = truth
        .iter()
        .enumerate()
        .flat_map(|(i, a)| truth.iter().skip(i + 1).map(move |b| dist(*a, *b)))
        .fold(f64::MAX, f64::min);
    let f = fit_faces(points, &FaceOptions::default());
    let scanned = f.planes.iter().filter(|p| !p.from_table).count();
    let p = f.polyhedron.as_ref().unwrap_or_else(|| {
        panic!(
            "{label}: {} planes ({scanned} scanned) did not close: {:?}",
            f.planes.len(),
            f.warnings
        )
    });
    let worst_rms = f.planes.iter().map(|p| p.rms).fold(0.0, f64::max);
    let dihedral_err = p
        .edges
        .iter()
        .map(|e| {
            dihedrals
                .iter()
                .map(|d| (e.dihedral - d).abs())
                .fold(f64::MAX, f64::min)
        })
        .fold(0.0, f64::max);
    let edge_err = p
        .edges
        .iter()
        .map(|e| (e.length - edge).abs())
        .fold(0.0, f64::max);
    let corner_err = p
        .vertices
        .iter()
        .map(|v| truth.iter().map(|t| dist(*v, *t)).fold(f64::MAX, f64::min))
        .fold(0.0, f64::max);
    let true_volume = mesh_volume(tris);
    eprintln!(
        "{label} faces: {} planes ({scanned} scanned, {:.0} % of points on them, worst rms {worst_rms:.3} mm); {} faces, {} edges, {} corners; dihedral error {dihedral_err:.2} deg, edge error {edge_err:.2} mm on {edge:.1}, corner error {corner_err:.2} mm, volume {:.0} of {true_volume:.0} ({:+.2} %)",
        f.planes.len(), 100.0 * f.planar, p.faces.len(), p.edges.len(), p.vertices.len(), p.volume, 100.0 * (p.volume - true_volume) / true_volume
    );
    assert_eq!(
        (p.faces.len(), p.edges.len(), p.vertices.len()),
        (faces, edges, vertices),
        "{label}: counts"
    );
    // A plane's tilt error scales with the noise over the face's
    // span: the bucky ball's faces are a fifth the size of the cube's.
    let (angle_limit, length_limit) = if solid == Solid::Bucky {
        (0.8, 0.8)
    } else {
        (0.5, (0.01 * edge).max(0.5))
    };
    assert!(
        dihedral_err < angle_limit,
        "{label}: dihedral error {dihedral_err}"
    );
    assert!(edge_err < length_limit, "{label}: edge error {edge_err}");
    assert!(corner_err < 0.5, "{label}: corner error {corner_err}");
    assert!(
        (p.volume - true_volume).abs() / true_volume < 0.005,
        "{label}: volume {} of {true_volume}",
        p.volume
    );
    assert!(f.warnings.is_empty(), "{label}: {:?}", f.warnings);
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// A solid standing on a face, scanned once. The points lie on its
/// surface to a tenth of a millimetre; the mesh is a polygon per
/// slice through 24 frames, so it cuts the corners and its volume
/// comes out a few per cent under.
fn upright(solid: Solid) -> (Vec<[[f64; 3]; 3]>, Scan) {
    let tris = solid.standing(HEIGHT, AXIS, TURN);
    let frames = render(&tris, 22.0, frames_for(solid));
    if solid == Solid::Bucky {
        save_frames("frames-bucky", &frames);
    }
    let s = scan(&frames, &ScanOptions::default()).unwrap();
    let f = fit(&s.points, &tris, frames_for(solid));
    eprintln!(
        "{} upright: {} points, surface error median {:.3} p90 {:.3} mm, volume {:.0} of {:.0} ({:.1} %; {:.1} % of the shape sampled at the same azimuths), height covered {:.0} %, mesh filled {:.0} %, warnings {:?}",
        solid.name(), s.points.len(), f.median, f.p90, f.volume, f.true_volume, 100.0 * f.volume / f.true_volume, 100.0 * f.volume / f.sampled_volume, 100.0 * f.covered, 100.0 * s.filled, s.warnings
    );
    assert!(
        f.median < 0.3 && f.p90 < 1.0,
        "{}: surface error median {} p90 {}",
        solid.name(),
        f.median,
        f.p90
    );
    assert!(
        (f.volume - f.sampled_volume).abs() / f.sampled_volume < 0.02,
        "{}: volume {} against {} sampled the same way (true {})",
        solid.name(),
        f.volume,
        f.sampled_volume,
        f.true_volume
    );
    assert!(
        f.covered > 0.95,
        "{}: height covered {}",
        solid.name(),
        f.covered
    );
    assert!(s.warnings.is_empty(), "{}: {:?}", solid.name(), s.warnings);
    faces_check(
        solid,
        &tris,
        &s.points,
        &format!("{} upright", solid.name()),
    );
    (tris, s)
}

/// A second scan of the solid in another pose merged with the first:
/// the registration lands, the merged points still lie on the true
/// surface (so the two scans agree), and the volume is not worse.
fn merged(
    solid: Solid,
    tris: &[[[f64; 3]; 3]],
    up: &Scan,
    other: &[[[f64; 3]; 3]],
    elevation: f64,
    placement: Placement,
) -> (Registration, Fit) {
    let frames = render(other, elevation, frames_for(solid));
    let again = scan(&frames, &ScanOptions::default()).unwrap();
    let (cloud, regs) = merge(&[
        (up.points.clone(), Placement::Same),
        (again.points.clone(), placement),
    ])
    .unwrap();
    let r = regs[0].clone();
    let f = fit(&cloud, tris, frames_for(solid));
    eprintln!(
        "{} merged with {}: residual {:.3} mm, overlap {:.2}, {} seeds; {} points, surface error median {:.3} p90 {:.3}, volume {:.0} of {:.0} ({:.1} %; {:.1} % sampled the same way)",
        solid.name(), placement.name(), r.residual, r.overlap, r.seeds, cloud.len(), f.median, f.p90, f.volume, f.true_volume, 100.0 * f.volume / f.true_volume, 100.0 * f.volume / f.sampled_volume
    );
    assert!(
        r.residual < 0.4,
        "{}: registration residual {}",
        solid.name(),
        r.residual
    );
    // The overlap is the share of the second scan's points within
    // 2 mm of the first's surface: less than all of them, since the
    // second pose shows faces the first scan had to fill across.
    assert!(r.overlap > 0.6, "{}: overlap {}", solid.name(), r.overlap);
    assert!(
        f.median < 0.3 && f.p90 < 1.0,
        "{}: merged surface error median {} p90 {}",
        solid.name(),
        f.median,
        f.p90
    );
    assert!(
        (f.volume - f.sampled_volume).abs() / f.sampled_volume < 0.02,
        "{}: merged volume {} against {} sampled the same way (true {})",
        solid.name(),
        f.volume,
        f.sampled_volume,
        f.true_volume
    );
    faces_check(
        solid,
        tris,
        &cloud,
        &format!("{} merged with {}", solid.name(), placement.name()),
    );
    (r, f)
}

#[test]
fn a_tetrahedron_scans_and_registers_rolled_onto_a_side() {
    let (tris, up) = upright(Solid::Tetrahedron);
    let rolled = on_side(&tris, 20.0, 55.0, [AXIS[0] - 5.0, AXIS[1] + 7.0]);
    merged(
        Solid::Tetrahedron,
        &tris,
        &up,
        &rolled,
        22.0,
        Placement::OnSide,
    );
}

#[test]
fn a_cube_scans_and_merges_with_a_higher_camera() {
    let (tris, up) = upright(Solid::Cube);
    let (r, _) = merged(Solid::Cube, &tris, &up, &tris, 50.0, Placement::Same);
    assert!(
        r.transform.angle() < 0.3,
        "the object did not move, but the registration turned it {} degrees",
        r.transform.angle()
    );
}

#[test]
fn an_octahedron_scans_and_registers_set_down_elsewhere() {
    let (tris, up) = upright(Solid::Octahedron);
    let moved = settle(
        &transform(&tris, rotation([0.0, 0.0, 1.0], 100.0), [0.0; 3]),
        [AXIS[0] + 9.0, AXIS[1] - 6.0],
    );
    merged(
        Solid::Octahedron,
        &tris,
        &up,
        &moved,
        22.0,
        Placement::Moved,
    );
}

#[test]
fn a_dodecahedron_scans_and_registers_upside_down() {
    let (tris, up) = upright(Solid::Dodecahedron);
    let flipped = upside_down(&tris, 70.0, 140.0, [AXIS[0] + 6.0, AXIS[1] - 4.0]);
    merged(
        Solid::Dodecahedron,
        &tris,
        &up,
        &flipped,
        22.0,
        Placement::UpsideDown,
    );
}

#[test]
fn an_icosahedron_scans() {
    upright(Solid::Icosahedron);
}

#[test]
fn a_bucky_ball_scans_and_registers_upside_down() {
    let (tris, up) = upright(Solid::Bucky);
    let flipped = upside_down(&tris, 30.0, 200.0, [AXIS[0] - 3.0, AXIS[1] + 8.0]);
    merged(
        Solid::Bucky,
        &tris,
        &up,
        &flipped,
        22.0,
        Placement::UpsideDown,
    );
}
