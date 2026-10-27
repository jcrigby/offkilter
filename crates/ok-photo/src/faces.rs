//! Flat faces from a scan: the point cloud segmented into planar
//! patches by growing regions over points whose normals agree, each
//! patch a plane by least squares, and the planes closed into a solid
//! where they can be. A scanned cube is then a cube, six planes meeting
//! in twelve straight edges and eight corners, not a lattice of small
//! triangles with every corner chamfered: the points of a face land
//! within a tenth of a millimetre of it, and a plane through hundreds
//! of them is good to a few hundredths, so the edges, which are the
//! planes' intersections, come out sharper than any point.
//!
//! The solid is the intersection of the planes' half-spaces, which is
//! right for anything convex. The table the object stands on is a
//! plane the scan never sees, so it is supplied.

use super::register::{eigen, Tree};
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct FaceOptions {
    /// How far from its plane a point may lie and still belong, mm.
    pub tol: f64,
    /// How far a point's normal may lean from a patch's and still
    /// belong, degrees.
    pub angle: f64,
    /// The fewest points a patch is kept with.
    pub min_points: usize,
    /// Add the table's plane (z = 0) as a face when the cloud reaches
    /// down to it, since no scan from above sees the underside.
    pub table: bool,
}

impl Default for FaceOptions {
    fn default() -> FaceOptions {
        FaceOptions {
            tol: 0.5,
            angle: 12.0,
            min_points: 15,
            table: true,
        }
    }
}

/// A plane through a patch of the cloud: n · p = offset, n unit and
/// pointing out of the object.
#[derive(Debug, Clone, Serialize)]
pub struct PlaneFit {
    pub normal: [f64; 3],
    pub offset: f64,
    pub centroid: [f64; 3],
    /// Points in the patch, and how far they lie from the plane (rms).
    pub points: usize,
    pub rms: f64,
    /// Supplied as the table rather than scanned.
    pub from_table: bool,
}

/// A face of the closed solid: a convex polygon on one of the planes.
#[derive(Debug, Clone, Serialize)]
pub struct Face {
    pub plane: usize,
    /// Indices into the polyhedron's vertices, counter-clockwise seen
    /// from outside.
    pub vertices: Vec<usize>,
    pub area: f64,
}

/// An edge between two faces.
#[derive(Debug, Clone, Serialize)]
pub struct Edge {
    pub vertices: [usize; 2],
    pub faces: [usize; 2],
    pub length: f64,
    /// The angle inside the solid between the two faces, degrees.
    pub dihedral: f64,
}

/// The planes closed into a convex solid.
#[derive(Debug, Clone, Serialize)]
pub struct Polyhedron {
    pub vertices: Vec<[f64; 3]>,
    pub faces: Vec<Face>,
    pub edges: Vec<Edge>,
    pub volume: f64,
    pub area: f64,
}

/// What the fitting found.
#[derive(Debug, Clone, Serialize)]
pub struct FaceFit {
    pub planes: Vec<PlaneFit>,
    /// The share of the points that lie on one of the planes.
    pub planar: f64,
    pub polyhedron: Option<Polyhedron>,
    pub warnings: Vec<String>,
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(a: [f64; 3]) -> [f64; 3] {
    let l = dot(a, a).sqrt();
    if l < 1e-15 {
        [0.0; 3]
    } else {
        [a[0] / l, a[1] / l, a[2] / l]
    }
}

/// The unit normal at every point of a cloud (zero where none could
/// be taken), as the face fitting sees them: for a scan, from each
/// laser line's tangent and the step to the neighbouring line.
pub fn point_normals(points: &[[f64; 3]], tol: f64) -> Vec<([f64; 3], f64, f64)> {
    Tree::new(points).normals_lined(tol)
}

/// The plane through points by least squares: the centroid and the
/// covariance's smallest eigenvector, with the rms distance.
fn fit_plane(pts: &[[f64; 3]]) -> ([f64; 3], [f64; 3], f64) {
    let n = pts.len() as f64;
    let c = pts.iter().fold([0.0; 3], |c, p| {
        [c[0] + p[0] / n, c[1] + p[1] / n, c[2] + p[2] / n]
    });
    let mut cov = [[0.0; 3]; 3];
    for p in pts {
        let d = sub(*p, c);
        for (r, row) in cov.iter_mut().enumerate() {
            for (k, v) in row.iter_mut().enumerate() {
                *v += d[r] * d[k];
            }
        }
    }
    let (_, normal) = eigen(cov);
    let rms = (pts
        .iter()
        .map(|p| dot(sub(*p, c), normal).powi(2))
        .sum::<f64>()
        / n)
        .sqrt();
    (c, normal, rms)
}

/// Segments the cloud into planar patches and fits each a plane. The
/// planes are taken one at a time: from the unclaimed point whose
/// neighbourhood lies most fully on its own plane (the middle of a
/// face), that plane claims every unclaimed point lying on it whose
/// normal agrees, is refitted to them and claims again, three times,
/// and keeps its points if there are enough and their normals do not
/// fan out as a curve's would; then the next. Patches on one plane are
/// merged, the table's plane is added when asked for and reached, and
/// the planes are closed into a solid where they close.
pub fn fit_faces(points: &[[f64; 3]], opts: &FaceOptions) -> FaceFit {
    let mut warnings = Vec::new();
    if points.len() < opts.min_points.max(10) {
        return FaceFit {
            planes: Vec::new(),
            planar: 0.0,
            polyhedron: None,
            warnings: vec![format!("too few points ({}) to fit faces", points.len())],
        };
    }
    let tree = Tree::new(points);
    // The tolerance opens up for a noisy scan (a compressed video's
    // line jitters more than clean frames'): never under three times
    // the noise along the laser lines.
    let noise = tree.line_noise();
    let tol = opts.tol.max(2.5 * noise);
    // A looser tolerance lets more chance alignments through, so a
    // patch must bring proportionally more points.
    let min_points = (opts.min_points as f64 * tol / opts.tol).round() as usize;
    let normals = tree.normals_lined(0.6 * tol);
    let cos_min = opts.angle.to_radians().cos();
    // Seeds: the points whose neighbourhoods lie most fully on their
    // own plane, the middle of a face before its edges.
    let mut order: Vec<usize> = (0..points.len())
        .filter(|&i| normals[i].0 != [0.0; 3] && normals[i].2 >= 0.15)
        .collect();
    order.sort_by(|&a, &b| normals[b].2.total_cmp(&normals[a].2));
    let mut claimed: Vec<bool> = vec![false; points.len()];
    // Each patch: the points it claims, and the ones its plane is
    // fitted on (those whose normals agree, so the sliver the next
    // face lends along an edge does not tilt a small face).
    let mut patches: Vec<(Vec<usize>, Vec<usize>)> = Vec::new();
    let mut misses = 0;
    for &seed in &order {
        if claimed[seed] {
            continue;
        }
        if misses > 1000 {
            break;
        }
        let (mut c, mut n) = (points[seed], normals[seed].0);
        let mut agreeing: Vec<usize> = Vec::new();
        for _ in 0..3 {
            agreeing = (0..points.len())
                .filter(|&k| {
                    !claimed[k]
                        && normals[k].0 != [0.0; 3]
                        && dot(normals[k].0, n).abs() >= cos_min
                        && dot(sub(points[k], c), n).abs() <= tol
                })
                .collect();
            if agreeing.len() < min_points {
                break;
            }
            let pts: Vec<[f64; 3]> = agreeing.iter().map(|&k| points[k]).collect();
            let (cc, nn, _) = fit_plane(&pts);
            c = cc;
            n = if dot(nn, n) < 0.0 {
                [-nn[0], -nn[1], -nn[2]]
            } else {
                nn
            };
        }
        if agreeing.len() < min_points {
            claimed[seed] = false;
            misses += 1;
            continue;
        }
        // A patch on a curve also fits a plane within the tolerance,
        // over a cap or a strip of it, but its normals fan out across
        // the whole allowed angle; a flat face's normals scatter only
        // by the noise, with a few blends from its edges. Keep a patch
        // whose normals lean from its plane by a third of the
        // allowance or less at the median.
        let mut leans: Vec<f64> = agreeing
            .iter()
            .map(|&k| {
                dot(normals[k].0, n)
                    .abs()
                    .clamp(0.0, 1.0)
                    .acos()
                    .to_degrees()
            })
            .collect();
        leans.sort_by(f64::total_cmp);
        let lean = leans[leans.len() / 2];
        if lean > opts.angle / 3.0 {
            misses += 1;
            continue;
        }
        // The patch: every unclaimed point on the plane, whatever its
        // normal (a normal near an edge is a blend of two faces, and a
        // second scan's points lie a registration's error off).
        let members: Vec<usize> = (0..points.len())
            .filter(|&k| !claimed[k] && dot(sub(points[k], c), n).abs() <= tol)
            .collect();
        for &k in &members {
            claimed[k] = true;
        }
        misses = 0;
        patches.push((members, agreeing));
    }
    let _ = &tree;
    // The planes, oriented out of the object (away from the cloud's
    // centre), then merged where two patches lie on one plane.
    let n_all = points.len() as f64;
    let centre = points.iter().fold([0.0; 3], |c, p| {
        [
            c[0] + p[0] / n_all,
            c[1] + p[1] / n_all,
            c[2] + p[2] / n_all,
        ]
    });
    let make = |group: &(Vec<usize>, Vec<usize>), from_table: bool| -> PlaneFit {
        let pts: Vec<[f64; 3]> = group.1.iter().map(|&k| points[k]).collect();
        let (c, mut n, rms) = fit_plane(&pts);
        if dot(n, sub(c, centre)) < 0.0 {
            n = [-n[0], -n[1], -n[2]];
        }
        PlaneFit {
            normal: n,
            offset: dot(n, c),
            centroid: c,
            points: group.0.len(),
            rms,
            from_table,
        }
    };
    let mut groups: Vec<(Vec<usize>, Vec<usize>)> = patches;
    let mut merged = true;
    while merged {
        merged = false;
        let planes: Vec<PlaneFit> = groups.iter().map(|g| make(g, false)).collect();
        'outer: for i in 0..planes.len() {
            for j in i + 1..planes.len() {
                let (a, b) = (&planes[i], &planes[j]);
                let parallel = dot(a.normal, b.normal) > 6f64.to_radians().cos();
                let same = (dot(a.normal, b.centroid) - a.offset).abs() <= 3.0 * tol
                    && (dot(b.normal, a.centroid) - b.offset).abs() <= 3.0 * tol;
                if parallel && same {
                    let gj = groups.remove(j);
                    groups[i].0.extend(gj.0);
                    groups[i].1.extend(gj.1);
                    merged = true;
                    break 'outer;
                }
            }
        }
    }
    groups.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
    let mut planes: Vec<PlaneFit> = groups.iter().map(|g| make(g, false)).collect();
    let on_planes: usize = groups.iter().map(|g| g.0.len()).sum();
    let planar = on_planes as f64 / n_all;
    // The table.
    let lowest = points.iter().map(|p| p[2]).fold(f64::MAX, f64::min);
    if opts.table && lowest < 4.0 {
        let table = PlaneFit {
            normal: [0.0, 0.0, -1.0],
            offset: 0.0,
            centroid: [centre[0], centre[1], 0.0],
            points: 0,
            rms: 0.0,
            from_table: true,
        };
        let seen = planes.iter().any(|p| {
            dot(p.normal, table.normal) > 3f64.to_radians().cos()
                && (dot(p.normal, table.centroid) - p.offset).abs() <= 2.0 * tol
        });
        if !seen {
            planes.push(table);
        }
    }
    let polyhedron = if planes.len() >= 4 {
        match polyhedron(&planes) {
            Ok(p) => Some(p),
            Err(e) => {
                warnings.push(e);
                None
            }
        }
    } else {
        if !planes.is_empty() {
            warnings.push(format!(
                "{} plane{} found, too few to close a solid",
                planes.len(),
                if planes.len() == 1 { "" } else { "s" }
            ));
        }
        None
    };
    FaceFit {
        planes,
        planar,
        polyhedron,
        warnings,
    }
}

fn solve3(a: [[f64; 3]; 3], b: [f64; 3]) -> Option<[f64; 3]> {
    let det = a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1])
        - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
        + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0]);
    if det.abs() < 1e-9 {
        return None;
    }
    let mut x = [0.0; 3];
    for (k, xk) in x.iter_mut().enumerate() {
        let mut m = a;
        for (row, bi) in m.iter_mut().zip(b) {
            row[k] = bi;
        }
        let d = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
        *xk = d / det;
    }
    Some(x)
}

/// The convex solid the planes' half-spaces bound: every point where
/// three planes meet and no plane excludes is a vertex, each plane's
/// vertices ordered round it are its face, and the faces must share
/// every edge in pairs or the planes do not close.
pub fn polyhedron(planes: &[PlaneFit]) -> Result<Polyhedron, String> {
    let n = planes.len();
    let size = planes
        .iter()
        .map(|p| p.offset.abs())
        .fold(0.0, f64::max)
        .max(1.0);
    let eps = 1e-6 * size + 0.02;
    // Vertices: triple intersections inside every half-space, merged
    // where several triples meet at one corner.
    let mut vertices: Vec<[f64; 3]> = Vec::new();
    let mut on: Vec<Vec<usize>> = Vec::new(); // planes through each vertex
    for i in 0..n {
        for j in i + 1..n {
            for k in j + 1..n {
                let a = [planes[i].normal, planes[j].normal, planes[k].normal];
                let b = [planes[i].offset, planes[j].offset, planes[k].offset];
                let Some(p) = solve3(a, b) else {
                    continue;
                };
                if planes.iter().any(|pl| dot(pl.normal, p) - pl.offset > eps) {
                    continue;
                }
                let touching: Vec<usize> = (0..n)
                    .filter(|&m| (dot(planes[m].normal, p) - planes[m].offset).abs() <= eps)
                    .collect();
                match vertices
                    .iter()
                    .position(|v| dot(sub(*v, p), sub(*v, p)).sqrt() <= 10.0 * eps)
                {
                    Some(v) => {
                        for t in touching {
                            if !on[v].contains(&t) {
                                on[v].push(t);
                            }
                        }
                    }
                    None => {
                        vertices.push(p);
                        on.push(touching);
                    }
                }
            }
        }
    }
    // Corners where four or more planes meet come out as a cluster of
    // near points with a sliver between, since fitted planes do not
    // meet at one point: cluster them, the corner at the point nearest
    // all its planes by least squares.
    let near = (0.01 * size).max(1.0);
    let mut merged_v: Vec<[f64; 3]> = Vec::new();
    let mut merged_on: Vec<Vec<usize>> = Vec::new();
    let mut taken = vec![false; vertices.len()];
    for i in 0..vertices.len() {
        if taken[i] {
            continue;
        }
        let mut cluster = vec![i];
        taken[i] = true;
        let mut grew = true;
        while grew {
            grew = false;
            for j in 0..vertices.len() {
                if taken[j] {
                    continue;
                }
                if cluster.iter().any(|&c| {
                    dot(sub(vertices[c], vertices[j]), sub(vertices[c], vertices[j])).sqrt() <= near
                }) {
                    taken[j] = true;
                    cluster.push(j);
                    grew = true;
                }
            }
        }
        let mut pls: Vec<usize> = cluster.iter().flat_map(|&c| on[c].clone()).collect();
        pls.sort_unstable();
        pls.dedup();
        let point = if cluster.len() == 1 {
            vertices[i]
        } else {
            // Least squares: minimise sum of (n·p - d)^2 over the planes.
            let mut a = [[0.0; 3]; 3];
            let mut b = [0.0; 3];
            for &pi in &pls {
                let nn = planes[pi].normal;
                for r in 0..3 {
                    for k in 0..3 {
                        a[r][k] += nn[r] * nn[k];
                    }
                    b[r] += nn[r] * planes[pi].offset;
                }
            }
            solve3(a, b).unwrap_or_else(|| {
                let m = cluster.len() as f64;
                cluster.iter().fold([0.0; 3], |c, &v| {
                    [
                        c[0] + vertices[v][0] / m,
                        c[1] + vertices[v][1] / m,
                        c[2] + vertices[v][2] / m,
                    ]
                })
            })
        };
        merged_v.push(point);
        merged_on.push(pls);
    }
    let (vertices, on) = (merged_v, merged_on);
    if vertices.len() < 4 {
        return Err(format!(
            "the {n} planes do not close a solid: {} corners",
            vertices.len()
        ));
    }
    // Faces: each plane's vertices round its centroid.
    let mut faces: Vec<Face> = Vec::new();
    for (pi, pl) in planes.iter().enumerate() {
        let mine: Vec<usize> = (0..vertices.len())
            .filter(|&v| on[v].contains(&pi))
            .collect();
        if mine.len() < 3 {
            continue;
        }
        let m = mine.len() as f64;
        let c = mine.iter().fold([0.0; 3], |c, &v| {
            [
                c[0] + vertices[v][0] / m,
                c[1] + vertices[v][1] / m,
                c[2] + vertices[v][2] / m,
            ]
        });
        let u = norm(sub(vertices[mine[0]], c));
        let v = cross(pl.normal, u);
        let mut ordered: Vec<(f64, usize)> = mine
            .iter()
            .map(|&k| {
                let d = sub(vertices[k], c);
                (dot(d, v).atan2(dot(d, u)), k)
            })
            .collect();
        ordered.sort_by(|a, b| a.0.total_cmp(&b.0));
        let ring: Vec<usize> = ordered.into_iter().map(|(_, k)| k).collect();
        let area = (0..ring.len())
            .map(|i| {
                let (a, b) = (
                    sub(vertices[ring[i]], c),
                    sub(vertices[ring[(i + 1) % ring.len()]], c),
                );
                dot(cross(a, b), pl.normal) / 2.0
            })
            .sum::<f64>();
        faces.push(Face {
            plane: pi,
            vertices: ring,
            area,
        });
    }
    // Edges: every face edge shared by exactly one other face.
    let mut by_pair: std::collections::BTreeMap<(usize, usize), Vec<usize>> = Default::default();
    for (fi, f) in faces.iter().enumerate() {
        for i in 0..f.vertices.len() {
            let (a, b) = (f.vertices[i], f.vertices[(i + 1) % f.vertices.len()]);
            by_pair.entry((a.min(b), a.max(b))).or_default().push(fi);
        }
    }
    let mut edges = Vec::new();
    for ((a, b), fs) in &by_pair {
        if fs.len() != 2 {
            return Err(format!(
                "the {n} planes do not close a solid: an edge lies on {} face{}",
                fs.len(),
                if fs.len() == 1 { "" } else { "s" }
            ));
        }
        let (na, nb) = (
            planes[faces[fs[0]].plane].normal,
            planes[faces[fs[1]].plane].normal,
        );
        let dihedral = 180.0 - dot(na, nb).clamp(-1.0, 1.0).acos().to_degrees();
        edges.push(Edge {
            vertices: [*a, *b],
            faces: [fs[0], fs[1]],
            length: dot(
                sub(vertices[*a], vertices[*b]),
                sub(vertices[*a], vertices[*b]),
            )
            .sqrt(),
            dihedral,
        });
    }
    // Euler: V - E + F = 2 for a closed convex solid.
    if vertices.len() + faces.len() != edges.len() + 2 {
        return Err(format!(
            "the {n} planes do not close a solid: {} corners, {} edges, {} faces",
            vertices.len(),
            edges.len(),
            faces.len()
        ));
    }
    let volume: f64 = faces
        .iter()
        .map(|f| {
            let v0 = vertices[f.vertices[0]];
            (1..f.vertices.len() - 1)
                .map(|i| {
                    let (a, b) = (vertices[f.vertices[i]], vertices[f.vertices[i + 1]]);
                    dot(v0, cross(a, b)) / 6.0
                })
                .sum::<f64>()
        })
        .sum::<f64>()
        .abs();
    let area = faces.iter().map(|f| f.area).sum();
    Ok(Polyhedron {
        vertices,
        faces,
        edges,
        volume,
        area,
    })
}

impl Polyhedron {
    /// The faces as triangle fans over the shared vertices, for a mesh
    /// body: coplanar triangles merge back into one face each.
    pub fn triangles(&self) -> (Vec<[f64; 3]>, Vec<[u32; 3]>) {
        let mut tris = Vec::new();
        for f in &self.faces {
            for i in 1..f.vertices.len() - 1 {
                tris.push([
                    f.vertices[0] as u32,
                    f.vertices[i] as u32,
                    f.vertices[i + 1] as u32,
                ]);
            }
        }
        (self.vertices.clone(), tris)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Points over a box's faces but the bottom, as lines a laser
    /// would draw, with a little noise.
    fn box_cloud(w: f64, d: f64, h: f64, noise: f64) -> Vec<[f64; 3]> {
        let mut pts = Vec::new();
        let mut rng = 12345u64;
        let mut jitter = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            ((rng % 2001) as f64 / 1000.0 - 1.0) * noise
        };
        let lines = 24;
        let along = 80;
        for l in 0..lines {
            let t = (l as f64 + 0.5) / lines as f64;
            for k in 0..=along {
                let s = k as f64 / along as f64;
                // Four sides, lines running up each, and the top.
                pts.push([t * w, jitter(), s * h]);
                pts.push([t * w, d + jitter(), s * h]);
                pts.push([jitter(), t * d, s * h]);
                pts.push([w + jitter(), t * d, s * h]);
                pts.push([t * w, s * d, h + jitter()]);
            }
        }
        pts
    }

    #[test]
    fn a_box_cloud_fits_six_planes_and_closes() {
        let (w, d, h) = (60.0, 40.0, 80.0);
        let pts = box_cloud(w, d, h, 0.1);
        let f = fit_faces(&pts, &FaceOptions::default());
        assert_eq!(f.planes.len(), 6, "{:?}", f.warnings);
        assert!(f.planar > 0.75, "planar {}", f.planar);
        let p = f
            .polyhedron
            .as_ref()
            .unwrap_or_else(|| panic!("{:?}", f.warnings));
        assert_eq!(p.faces.len(), 6);
        assert_eq!(p.vertices.len(), 8);
        assert_eq!(p.edges.len(), 12);
        for e in &p.edges {
            assert!((e.dihedral - 90.0).abs() < 0.2, "dihedral {}", e.dihedral);
            let ok = [w, d, h].iter().any(|l| (e.length - l).abs() < 0.3);
            assert!(ok, "edge {}", e.length);
        }
        assert!(
            (p.volume - w * d * h).abs() / (w * d * h) < 0.005,
            "volume {}",
            p.volume
        );
        let (v, t) = p.triangles();
        assert_eq!(v.len(), 8);
        assert_eq!(t.len(), 12);
    }

    #[test]
    fn a_round_cloud_has_no_planes_but_the_table() {
        let mut pts = Vec::new();
        for i in 0..60 {
            for j in 0..60 {
                let (a, b) = (
                    std::f64::consts::PI * (i as f64 + 0.5) / 60.0,
                    2.0 * std::f64::consts::PI * j as f64 / 60.0,
                );
                pts.push([
                    40.0 * a.sin() * b.cos(),
                    40.0 * a.sin() * b.sin(),
                    40.0 + 40.0 * a.cos(),
                ]);
            }
        }
        let f = fit_faces(&pts, &FaceOptions::default());
        assert!(
            f.planes.iter().all(|p| p.from_table),
            "{:?}",
            f.planes.len()
        );
        assert!(f.polyhedron.is_none());
        assert!(f.planar < 0.05, "planar {}", f.planar);
    }
}
