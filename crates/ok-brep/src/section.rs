//! Planar cross-sections of solids.
//!
//! The section of a solid by a plane is a set of oriented 2D loops in the
//! plane's coordinates, counter-clockwise around solid material. Vertices
//! that lie exactly on the plane are treated as being on one side, chosen
//! by `zero_is_above`; this is the classic simulation-of-simplicity trick
//! and lets callers ask for the section "just above" or "just below" a
//! plane without perturbing coordinates. Crossing points are computed once
//! per edge, so the loops chain together exactly by edge identity.
//! Vertices within `eps` of the plane are snapped onto it first, so that
//! faces built to be coplanar are treated as exactly coplanar.

use crate::{edge_key, BrepError, EdgeKey, Solid};
use ok_math::{Plane, Vec2, Vec3};
use std::collections::HashMap;

pub struct Section {
    /// Loops in plane coordinates; CCW encloses material.
    pub loops: Vec<Vec<Vec2>>,
}

impl Section {
    pub fn is_empty(&self) -> bool {
        self.loops.is_empty()
    }
}

struct Crossing {
    point: Vec3,
}

#[cfg(test)]
pub fn section(
    solid: &Solid,
    plane: &Plane,
    zero_is_above: bool,
    eps: f64,
) -> Result<Section, BrepError> {
    let bboxes = face_bboxes(solid);
    section_with_bboxes(solid, &bboxes, plane, zero_is_above, eps)
}

/// Bounding box of every face, for repeated sections of one solid.
pub fn face_bboxes(solid: &Solid) -> Vec<(Vec3, Vec3)> {
    solid
        .faces
        .iter()
        .map(|f| {
            crate::bounds_of(
                f.loops
                    .iter()
                    .flatten()
                    .map(|&v| solid.vertices[v as usize]),
            )
            .unwrap_or((Vec3::ZERO, Vec3::ZERO))
        })
        .collect()
}

/// `section` with the faces' bounding boxes precomputed (`face_bboxes`):
/// faces whose box lies entirely on one side of the plane are skipped
/// without visiting their edges.
pub fn section_with_bboxes(
    solid: &Solid,
    bboxes: &[(Vec3, Vec3)],
    plane: &Plane,
    zero_is_above: bool,
    eps: f64,
) -> Result<Section, BrepError> {
    let n = plane.normal;
    let d0 = n.dot(plane.origin);
    // Extreme signed distances of a box's corners along the normal.
    let straddles = |(lo, hi): &(Vec3, Vec3)| -> bool {
        let pick = |c: f64, l: f64, h: f64| if c >= 0.0 { (h, l) } else { (l, h) };
        let (xh, xl) = pick(n.x, lo.x, hi.x);
        let (yh, yl) = pick(n.y, lo.y, hi.y);
        let (zh, zl) = pick(n.z, lo.z, hi.z);
        let dmax = n.x * xh + n.y * yh + n.z * zh - d0;
        let dmin = n.x * xl + n.y * yl + n.z * zl - d0;
        dmin <= eps && dmax >= -eps
    };
    let mut dist: Vec<f64> = solid
        .vertices
        .iter()
        .map(|v| {
            let d = n.dot(*v) - d0;
            if d.abs() <= eps {
                0.0
            } else {
                d
            }
        })
        .collect();
    // A face parallel to the section plane is skipped below (it has no
    // crossing line), so its vertices must agree on which side they are:
    // when any of them sits on the plane, they all do. Otherwise one
    // vertex a rounding error past the snap band would leave the
    // neighbouring faces producing crossings through this face that
    // nothing closes. Repeated until stable, as snapping one face's
    // vertex can bring another parallel face onto the plane. Vertices
    // moved this way are remembered so the bounding-box prefilter below
    // (which sees the unsnapped coordinates) does not drop their faces.
    let parallel: Vec<usize> = solid
        .faces
        .iter()
        .enumerate()
        .filter(|(_, f)| n.cross(f.plane.normal).length() <= 1e-9)
        .map(|(i, _)| i)
        .collect();
    let mut pulled: Vec<u32> = Vec::new();
    loop {
        let mut changed = false;
        for &fi in &parallel {
            let verts = || solid.faces[fi].loops.iter().flatten().copied();
            let any_on = verts().any(|v| dist[v as usize] == 0.0);
            let any_off = verts().any(|v| dist[v as usize] != 0.0);
            if any_on && any_off {
                for v in verts() {
                    if dist[v as usize] != 0.0 {
                        dist[v as usize] = 0.0;
                        pulled.push(v);
                    }
                }
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let dist = dist;
    let touches_pulled = |f: &crate::Face| -> bool {
        !pulled.is_empty() && f.loops.iter().flatten().any(|v| pulled.contains(v))
    };
    let above = |v: u32| -> bool {
        let d = dist[v as usize];
        if zero_is_above {
            d >= 0.0
        } else {
            d > 0.0
        }
    };

    // Crossing nodes. A crossing that lands exactly on a vertex is shared by
    // every edge through that vertex, so a face merely touching the plane at
    // a vertex contributes a zero-length (skipped) segment and chains pass
    // through the vertex consistently.
    let mut crossings: Vec<Crossing> = Vec::new();
    let mut by_edge: HashMap<EdgeKey, usize> = HashMap::new();
    let mut by_vertex: HashMap<u32, usize> = HashMap::new();
    let mut crossing_of = |a: u32, b: u32| -> Option<(usize, Vec3)> {
        if above(a) == above(b) {
            return None;
        }
        let key = edge_key(a, b);
        let (lo, hi) = key;
        let (dl, dh) = (dist[lo as usize], dist[hi as usize]);
        let t = dl / (dl - dh);
        let id = if t <= 0.0 || t >= 1.0 {
            let v = if t <= 0.0 { lo } else { hi };
            *by_vertex.entry(v).or_insert_with(|| {
                crossings.push(Crossing {
                    point: solid.vertices[v as usize],
                });
                crossings.len() - 1
            })
        } else {
            *by_edge.entry(key).or_insert_with(|| {
                let p = solid.vertices[lo as usize]
                    + (solid.vertices[hi as usize] - solid.vertices[lo as usize]) * t;
                crossings.push(Crossing { point: p });
                crossings.len() - 1
            })
        };
        Some((id, crossings[id].point))
    };

    // Directed segments between crossing nodes: start -> ends.
    let mut next: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut segment_count = 0usize;
    for (fi, f) in solid.faces.iter().enumerate() {
        if let Some(bb) = bboxes.get(fi) {
            if !straddles(bb) && !touches_pulled(f) {
                continue;
            }
        }
        let Some(dir) = n.cross(f.plane.normal).normalized() else {
            continue; // parallel to the section plane
        };
        let mut hits: Vec<(f64, usize)> = Vec::new();
        for l in &f.loops {
            let count = l.len();
            for i in 0..count {
                if let Some((c, p)) = crossing_of(l[i], l[(i + 1) % count]) {
                    hits.push((p.dot(dir), c));
                }
            }
        }
        if hits.is_empty() {
            continue;
        }
        if !hits.len().is_multiple_of(2) {
            return Err(BrepError::OpenSection(format!(
                "face crossed an odd number of times ({})",
                hits.len()
            )));
        }
        hits.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        for pair in hits.chunks_exact(2) {
            let (s, e) = (pair[0].1, pair[1].1);
            if s == e {
                continue;
            }
            next.entry(s).or_default().push(e);
            segment_count += 1;
        }
    }
    let _ = &mut crossing_of;

    // Chain segments into loops. Every node has as many outgoing as
    // incoming segments in a valid section, so following unused outgoing
    // segments from any node always returns to it.
    let mut loops = Vec::new();
    let mut remaining = segment_count;
    while remaining > 0 {
        let Some((&start, _)) = next.iter().find(|(_, v)| !v.is_empty()) else {
            break;
        };
        let mut poly = Vec::new();
        let mut cur = start;
        loop {
            poly.push(plane.to_plane(crossings[cur].point));
            let Some(nx) = next.get_mut(&cur).and_then(|v| v.pop()) else {
                return Err(BrepError::OpenSection(format!(
                    "chain broke after {} of {} segments",
                    poly.len(),
                    segment_count
                )));
            };
            remaining -= 1;
            cur = nx;
            if cur == start {
                break;
            }
            if poly.len() > segment_count + 1 {
                return Err(BrepError::OpenSection("chain did not close".into()));
            }
        }
        if poly.len() >= 3 && ok_sketch::signed_area(&poly).abs() > 1e-18 {
            loops.push(poly);
        }
    }
    Ok(Section { loops })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extrude;
    use ok_math::Vec2;
    use ok_sketch::{ProfileOptions, Sketch};

    fn box_solid(w: f64, h: f64, d: f64) -> Solid {
        let mut s = Sketch::new();
        s.add_rectangle(Vec2::ZERO, Vec2::new(w, h));
        let p = s.profiles(&ProfileOptions::default()).remove(0);
        extrude(&p, &Plane::XY, 0.0, d, 1).unwrap()
    }

    #[test]
    fn mid_section_of_box_is_ccw_rectangle() {
        let b = box_solid(4.0, 2.0, 3.0);
        let plane = Plane::XY.offset(1.5);
        let sec = section(&b, &plane, true, 1e-9).unwrap();
        assert_eq!(sec.loops.len(), 1);
        assert!((ok_sketch::signed_area(&sec.loops[0]) - 8.0).abs() < 1e-9);
    }

    #[test]
    fn section_through_top_face_depends_on_side() {
        let b = box_solid(4.0, 2.0, 3.0);
        let top = Plane::XY.offset(3.0);
        // Just below the top: material.
        assert_eq!(section(&b, &top, true, 1e-9).unwrap().loops.len(), 1);
        // Just above the top: nothing.
        assert!(section(&b, &top, false, 1e-9).unwrap().is_empty());
    }

    #[test]
    fn vertical_section_is_ccw_about_material() {
        let b = box_solid(4.0, 2.0, 3.0);
        let plane = Plane::YZ.offset(2.0);
        let sec = section(&b, &plane, true, 1e-9).unwrap();
        assert_eq!(sec.loops.len(), 1);
        assert!((ok_sketch::signed_area(&sec.loops[0]) - 6.0).abs() < 1e-9);
    }
}
