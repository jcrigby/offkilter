//! Boolean operations on solids.
//!
//! Each face of one solid is classified against the other solid by taking
//! two cross-sections of that solid in the face's plane: one infinitesimally
//! above the plane (`X+`) and one below (`X-`). In the face's 2D frame:
//!
//! | region        | in X+ | in X- |
//! |---------------|-------|-------|
//! | inside        | yes   | yes   |
//! | outside       | no    | no    |
//! | coplanar face with the same normal     | no  | yes |
//! | coplanar face with the opposite normal | yes | no  |
//!
//! The kept part of the face is then a 2D polygon boolean of the face
//! region against those sections, which handles coplanar faces exactly
//! without any special casing. Fragments from both solids are reassembled
//! into a new solid by merging vertices, and the result is validated to be
//! closed; a non-closed result is reported as an error rather than shown.

use crate::section::{local_sections_above_below, sections_above_below, FaceIndex, LocalSection};
use crate::{
    bounds_of, bounds_overlap, flip_plane, merge_tolerance, BrepError, Face, Polygon, Solid,
    VertexMerger,
};
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::single::SingleFloatOverlay;
use ok_math::{Vec2, Vec3};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoolOp {
    Union,
    Difference,
    Intersection,
}

type Contour = Vec<[f64; 2]>;

fn to_contours(loops: &[Vec<Vec2>]) -> Vec<Contour> {
    loops
        .iter()
        .map(|l| l.iter().map(|p| [p.x, p.y]).collect())
        .collect()
}

/// Keeps the contours whose bounding box overlaps the region's (grown by
/// `tol`); a contour entirely outside neither encloses nor crosses it.
fn near_region(contours: Vec<Contour>, region: &[Contour], tol: f64) -> Vec<Contour> {
    let bbox = |cs: &[Contour]| {
        let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        for p in cs.iter().flatten() {
            b = [
                b[0].min(p[0]),
                b[1].min(p[1]),
                b[2].max(p[0]),
                b[3].max(p[1]),
            ];
        }
        b
    };
    let r = bbox(region);
    contours
        .into_iter()
        .filter(|c| {
            let b = bbox(std::slice::from_ref(c));
            b[2] >= r[0] - tol && b[0] <= r[2] + tol && b[3] >= r[1] - tol && b[1] <= r[3] + tol
        })
        .collect()
}

fn face_region(solid: &Solid, f: &Face) -> Vec<Contour> {
    f.loops
        .iter()
        .map(|l| {
            l.iter()
                .map(|&v| {
                    let q = f.plane.to_plane(solid.vertices[v as usize]);
                    [q.x, q.y]
                })
                .collect()
        })
        .collect()
}

/// Region ops on contour sets. `subject` is one polygon-with-holes; `clip`
/// is a set of consistently oriented loops combined with the non-zero rule.
/// The 64-bit engine keeps the internal grid far below model tolerance.
fn overlay(subject: &[Contour], clip: &[Contour], rule: OverlayRule) -> Vec<Vec<Contour>> {
    if clip.is_empty() {
        return match rule {
            OverlayRule::Difference | OverlayRule::Union | OverlayRule::Subject => {
                vec![subject.to_vec()]
            }
            _ => vec![],
        };
    }
    subject
        .to_vec()
        .overlay_as::<i64>(&clip.to_vec(), rule, FillRule::NonZero)
}

/// Moves clip vertices that lie within `tol` of a subject vertex or edge
/// exactly onto it, so shared boundaries are seen as identical by the
/// clipper instead of producing hairline slivers. The region's edges are
/// bucketed on a grid so a large outline (a plate face with many holes)
/// costs each clip point only the edges near it.
fn snap_to_region(clip: &mut [Contour], region: &[Contour], tol: f64) {
    let tol2 = tol * tol;
    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for p in region.iter().flatten() {
        b = [
            b[0].min(p[0]),
            b[1].min(p[1]),
            b[2].max(p[0]),
            b[3].max(p[1]),
        ];
    }
    if b[0] > b[2] {
        return;
    }
    let edges: Vec<([f64; 2], [f64; 2])> = region
        .iter()
        .flat_map(|r| {
            let n = r.len();
            (0..n).map(move |i| (r[i], r[(i + 1) % n]))
        })
        .collect();
    // About one edge per cell, with cells at least four tolerances wide
    // so a point's own cell and its neighbours hold every edge within a
    // tolerance of it (a facet gets a handful of cells, a plate face with
    // holes a few thousand).
    let extent = (b[2] - b[0]).max(b[3] - b[1]).max(tol);
    let across = (edges.len() as f64).sqrt().ceil().clamp(1.0, 256.0);
    let cell = (extent / across).max(4.0 * tol);
    let dims = [
        ((b[2] - b[0]) / cell).floor() as i64 + 1,
        ((b[3] - b[1]) / cell).floor() as i64 + 1,
    ];
    let key = |x: f64, y: f64| {
        (
            (((x - b[0]) / cell).floor() as i64).clamp(0, dims[0] - 1),
            (((y - b[1]) / cell).floor() as i64).clamp(0, dims[1] - 1),
        )
    };
    let mut cells: Vec<Vec<usize>> = vec![Vec::new(); (dims[0] * dims[1]) as usize];
    for (ei, (a, c)) in edges.iter().enumerate() {
        let (x0, y0) = key(a[0].min(c[0]), a[1].min(c[1]));
        let (x1, y1) = key(a[0].max(c[0]), a[1].max(c[1]));
        for x in x0..=x1 {
            for y in y0..=y1 {
                cells[(x * dims[1] + y) as usize].push(ei);
            }
        }
    }
    // Only points within the region's box (grown by the tolerance) can snap.
    for c in clip.iter_mut() {
        for p in c.iter_mut() {
            if p[0] < b[0] - tol || p[0] > b[2] + tol || p[1] < b[1] - tol || p[1] > b[3] + tol {
                continue;
            }
            let (kx, ky) = key(p[0], p[1]);
            let mut best: Option<(f64, [f64; 2])> = None;
            for x in (kx - 1).max(0)..=(kx + 1).min(dims[0] - 1) {
                for y in (ky - 1).max(0)..=(ky + 1).min(dims[1] - 1) {
                    for &ei in &cells[(x * dims[1] + y) as usize] {
                        let (a, b2) = edges[ei];
                        let (dx, dy) = (b2[0] - a[0], b2[1] - a[1]);
                        let len2 = dx * dx + dy * dy;
                        let t = if len2 > 0.0 {
                            ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2
                        } else {
                            0.0
                        };
                        let t = t.clamp(0.0, 1.0);
                        let q = [a[0] + dx * t, a[1] + dy * t];
                        let d2 = (p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2);
                        // Prefer vertices over edge interiors when both are in range.
                        let dv = (p[0] - a[0]).powi(2) + (p[1] - a[1]).powi(2);
                        let (d2, q) = if dv <= tol2 {
                            (dv - tol2 * 2.0, a)
                        } else {
                            (d2, q)
                        };
                        if d2 <= tol2 && best.is_none_or(|(bd, _)| d2 < bd) {
                            best = Some((d2, q));
                        }
                    }
                }
            }
            if let Some((_, q)) = best {
                *p = q;
            }
        }
    }
}

/// What to keep of a face of `subject_solid` given the other solid.
#[derive(Clone, Copy, Debug)]
enum Keep {
    /// Keep the part outside the other solid, plus coplanar same-normal (A side of union).
    NotAbove,
    /// Keep the part outside, excluding all coplanar (B side of union).
    NotAboveNorBelow,
    /// Keep the part outside plus coplanar opposite-normal (A side of difference).
    NotBelow,
    /// Keep the part strictly inside (B side of difference / intersection).
    Inside,
    /// Keep the part inside plus coplanar same-normal (A side of intersection).
    Below,
}

/// What a face contributes to the result.
enum Kept {
    /// The face unchanged.
    Whole,
    Nothing,
    /// Fragments of the face, as 2D shapes in its plane.
    Fragments(Vec<Vec<Contour>>),
}

impl Kept {
    /// The verdict for a face uniformly inside or outside the other
    /// solid a hair above and a hair below its plane, per the table at
    /// the top of the file.
    fn uniform(keep: Keep, above: bool, below: bool) -> Kept {
        let kept = match keep {
            Keep::NotAbove => !above,
            Keep::NotBelow => !below,
            Keep::Below => below,
            Keep::Inside => above && below,
            Keep::NotAboveNorBelow => !above && !below,
        };
        if kept {
            Kept::Whole
        } else {
            Kept::Nothing
        }
    }
}

fn classify_face(
    solid: &Solid,
    f: &Face,
    other: &Solid,
    other_boxes: &FaceIndex,
    other_bounds: (ok_math::Vec3, ok_math::Vec3),
    keep: Keep,
    tol: f64,
) -> Result<Kept, BrepError> {
    let fb = bounds_of(
        f.loops
            .iter()
            .flatten()
            .map(|&v| solid.vertices[v as usize]),
    )
    .unwrap();
    if !bounds_overlap(fb, other_bounds, tol) {
        return Ok(Kept::uniform(keep, false, false));
    }
    let region = face_region(solid, f);
    // The section of the other solid in this face's plane, cut only from
    // the faces near this face and closed along a rectangle around it;
    // the whole solid is sectioned only when that is too close to call.
    let local = local_section_loops(other, other_boxes, &f.plane, &region, tol);
    let (above, below) = match local {
        Some(Local::Uniform { above, below }) => {
            return Ok(Kept::uniform(keep, above, below));
        }
        Some(Local::Loops(above, below)) => (above, below),
        None => {
            let (above, below) = sections_above_below(other, other_boxes, &f.plane, tol)?;
            (to_contours(&above.loops), to_contours(&below.loops))
        }
    };
    if above.is_empty() && below.is_empty() {
        return Ok(Kept::uniform(keep, false, false));
    }
    // Loops of the section that stay clear of this face cannot enclose or
    // cut it, so they are left out of the overlay (a large part sectioned
    // by one of its facets otherwise drags every hole into every overlay).
    let mut above = near_region(above, &region, tol);
    let mut below = near_region(below, &region, tol);
    if above.is_empty() && below.is_empty() {
        // Every loop stays clear of the face, so none encloses it either.
        return Ok(Kept::uniform(keep, false, false));
    }
    snap_to_region(&mut above, &region, tol);
    snap_to_region(&mut below, &region, tol);
    let shapes = match keep {
        Keep::NotAbove => overlay(&region, &above, OverlayRule::Difference),
        Keep::NotBelow => overlay(&region, &below, OverlayRule::Difference),
        Keep::Below => overlay(&region, &below, OverlayRule::Intersect),
        Keep::NotAboveNorBelow => {
            let mut both = above.clone();
            both.extend(below);
            overlay(&region, &both, OverlayRule::Difference)
        }
        // With no coplanar faces the two sections are the same loops, and
        // one intersection is exact where a second against identical
        // edges can leave grid-unit slivers.
        Keep::Inside if above == below => overlay(&region, &above, OverlayRule::Intersect),
        Keep::Inside => {
            let mut out = Vec::new();
            for shape in overlay(&region, &above, OverlayRule::Intersect) {
                out.extend(overlay(&shape, &below, OverlayRule::Intersect));
            }
            out
        }
    };
    // A face the loops leave whole passes through as it is.
    if let [shape] = &shapes[..] {
        if same_shape(shape, &region, tol) {
            return Ok(Kept::Whole);
        }
    }
    if shapes.iter().all(|s| s.is_empty()) {
        return Ok(Kept::Nothing);
    }
    Ok(Kept::Fragments(shapes))
}

/// Whether two polygons-with-holes are the same loops, allowing the
/// overlay to have restarted a loop at another corner.
fn same_shape(a: &[Contour], b: &[Contour], tol: f64) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let tol2 = tol * tol * 1e-6;
    let same_loop = |x: &Contour, y: &Contour| -> bool {
        if x.len() != y.len() || x.is_empty() {
            return false;
        }
        let n = x.len();
        let close =
            |p: &[f64; 2], q: &[f64; 2]| (p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) <= tol2;
        (0..n).any(|shift| (0..n).all(|i| close(&x[(i + shift) % n], &y[i])))
    };
    a.iter().all(|x| b.iter().any(|y| same_loop(x, y)))
}

use crate::clip2d::{clip_and_close, Rect};

/// The sections of `other` a hair above and below `plane`, as closed
/// loops covering a rectangle around `region`, built from the faces near
/// that rectangle alone. Chains that leave the rectangle are closed along
/// its boundary (material lies to the left of a chain, so from where a
/// chain exits, the boundary is followed counter-clockwise to the next
/// entry); a rectangle no chain touches is filled or left empty by a
/// point test at its corner. `None` when the local picture is ambiguous
/// (a chain vertex on the rectangle's edge, endpoints that do not
/// alternate), in which case the caller sections the whole solid.
/// The local picture of the other solid around a face.
enum Local {
    /// No face of the other solid comes near the plane around the face:
    /// the face is wholly inside or outside it, a hair above and below.
    Uniform { above: bool, below: bool },
    /// The section loops a hair above and below, closed within the
    /// rectangle around the face.
    Loops(Vec<Contour>, Vec<Contour>),
}

fn local_section_loops(
    other: &Solid,
    index: &FaceIndex,
    plane: &ok_math::Plane,
    region: &[Contour],
    tol: f64,
) -> Option<Local> {
    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for p in region.iter().flatten() {
        b = [
            b[0].min(p[0]),
            b[1].min(p[1]),
            b[2].max(p[0]),
            b[3].max(p[1]),
        ];
    }
    let diag = ((b[2] - b[0]).powi(2) + (b[3] - b[1]).powi(2)).sqrt();
    let mut margin = (0.05 * diag).max(20.0 * tol);
    let clearance = 4.0 * tol;
    for _ in 0..4 {
        let rect: Rect = [b[0] - margin, b[1] - margin, b[2] + margin, b[3] + margin];
        let corners = [
            Vec2::new(rect[0], rect[1]),
            Vec2::new(rect[2], rect[1]),
            Vec2::new(rect[2], rect[3]),
            Vec2::new(rect[0], rect[3]),
        ];
        let within = bounds_of(corners.iter().map(|c| plane.to_world(*c)))?;
        let (above, below) = local_sections_above_below(other, index, plane, tol, within).ok()?;
        let near_edge = |p: &Vec2| {
            (p.x - rect[0]).abs() <= clearance
                || (p.x - rect[2]).abs() <= clearance
                || (p.y - rect[1]).abs() <= clearance
                || (p.y - rect[3]).abs() <= clearance
        };
        let crowded = [&above, &below].iter().any(|s| {
            s.loops
                .iter()
                .chain(s.chains.iter())
                .flatten()
                .any(near_edge)
        });
        if crowded {
            margin = margin * 1.37 + 7.0 * tol;
            continue;
        }
        // Whether the rectangle's corner is inside the other solid a hair
        // above (or below) the plane, from the faces a ray along the
        // normal meets: a face through the corner decides by its
        // orientation (it is coplanar, pulled onto the plane like the
        // section does); one within a few tolerances is too close to call
        // against the section's infinitesimal hair, so the whole solid is
        // sectioned instead; otherwise parity.
        let n = plane.normal;
        let corner_inside = |hair_above: bool| -> Option<bool> {
            let d = if hair_above { n } else { -n };
            let p = plane.to_world(corners[0]);
            let hits = index.hits_along(other, p, d, tol)?;
            let at_corner: Vec<usize> = hits
                .iter()
                .filter(|h| h.0.abs() <= tol)
                .map(|h| h.1)
                .collect();
            if let [fi] = at_corner[..] {
                let fnormal = other.faces[fi].plane.normal;
                if fnormal.cross(n).length() > 1e-6 {
                    return None;
                }
                return Some(fnormal.dot(d) < 0.0);
            }
            if !at_corner.is_empty() || hits.iter().any(|h| h.0 > tol && h.0 <= 4.0 * tol) {
                return None;
            }
            Some(hits.iter().filter(|h| h.0 > 4.0 * tol).count() % 2 == 1)
        };
        if above.loops.is_empty()
            && above.chains.is_empty()
            && below.loops.is_empty()
            && below.chains.is_empty()
        {
            // Nothing of the other solid crosses the plane within the
            // rectangle: one probe settles both sides, unless a face lies
            // in the plane at the corner (then the sides differ).
            let above = corner_inside(true)?;
            let below = corner_inside(false)?;
            return Some(Local::Uniform { above, below });
        }
        let above = close_in_rect(above, rect, tol, || corner_inside(true))?;
        let below = close_in_rect(below, rect, tol, || corner_inside(false))?;
        // Chains that were clipped away entirely leave the rectangle
        // full or empty: the face is uniformly inside or outside.
        let full = |loops: &Vec<Contour>| {
            loops.len() == 1 && loops[0].len() == 4 && loops[0][0] == [rect[0], rect[1]]
        };
        let verdict = |loops: &Vec<Contour>| -> Option<bool> {
            if loops.is_empty() {
                Some(false)
            } else if full(loops) {
                Some(true)
            } else {
                None
            }
        };
        if let (Some(a), Some(b)) = (verdict(&above), verdict(&below)) {
            return Some(Local::Uniform { above: a, below: b });
        }
        return Some(Local::Loops(above, below));
    }
    None
}

/// Closes a local section within `rect` (see `local_section_loops`).
fn close_in_rect(
    local: LocalSection,
    rect: Rect,
    tol: f64,
    corner_inside: impl Fn() -> Option<bool>,
) -> Option<Vec<Contour>> {
    let loops = to_contours(&local.loops);
    let chains = to_contours(&local.chains);
    clip_and_close(&loops, &chains, rect, tol, corner_inside)
}

/// Lifts the kept 2D fragments of `f` back to 3D. A fragment corner that
/// is one of the face's own vertices takes that vertex's exact position
/// rather than its projection onto the plane: a vertex may sit a hair off
/// the plane (within the planarity the solid allows), and the other faces
/// at that vertex keep it where it is, so lifting it onto the plane would
/// leave the fragments of neighbouring faces disagreeing by more than the
/// merge tolerance.
fn fragments_to_polygons(
    solid: &Solid,
    f: &Face,
    shapes: Vec<Vec<Contour>>,
    flip: bool,
    surface_offset: usize,
    tol: f64,
) -> Vec<Polygon> {
    let own: Vec<(Vec2, ok_math::Vec3)> = f
        .loops
        .iter()
        .flatten()
        .map(|&v| {
            let p = solid.vertices[v as usize];
            (f.plane.to_plane(p), p)
        })
        .collect();
    let lift = |p: &[f64; 2]| {
        let q = Vec2::new(p[0], p[1]);
        own.iter()
            .find(|(o, _)| o.distance(q) <= tol)
            .map(|(_, w)| *w)
            .unwrap_or_else(|| f.plane.to_world(q))
    };
    // The overlay drops collinear corners, so a fragment edge that runs
    // along the face's original boundary may have lost the vertices the
    // neighbouring faces still share there; put the face's own vertices
    // back on any edge they lie on.
    let restore = |l: Vec<ok_math::Vec3>| -> Vec<ok_math::Vec3> {
        let mut out: Vec<ok_math::Vec3> = Vec::with_capacity(l.len());
        let n = l.len();
        for i in 0..n {
            let (a, b) = (l[i], l[(i + 1) % n]);
            out.push(a);
            let d = b - a;
            let len2 = d.length_squared();
            if len2 == 0.0 {
                continue;
            }
            let mut on_edge: Vec<(f64, ok_math::Vec3)> = Vec::new();
            for &(_, w) in &own {
                let t = (w - a).dot(d) / len2;
                if t <= 0.0 || t >= 1.0 {
                    continue;
                }
                if (w - (a + d * t)).length() <= tol && w.distance(a) > tol && w.distance(b) > tol {
                    on_edge.push((t, w));
                }
            }
            on_edge.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap());
            out.extend(on_edge.into_iter().map(|(_, w)| w));
        }
        out
    };
    shapes
        .into_iter()
        .filter(|s| !s.is_empty())
        .map(|shape| {
            let mut loops: Vec<Vec<ok_math::Vec3>> = shape
                .iter()
                .map(|c| restore(c.iter().map(lift).collect()))
                .collect();
            let plane = if flip {
                for l in &mut loops {
                    l.reverse();
                }
                flip_plane(&f.plane)
            } else {
                f.plane
            };
            Polygon {
                plane,
                loops,
                surface: f.surface + surface_offset,
                origin: f.origin,
            }
        })
        .collect()
}

/// Combines two solids. The result keeps the surface tags and face origins
/// of both inputs.
/// Snaps the vertices of `s` that lie within `tol` of a vertex or a face
/// plane of `other` exactly onto it, so that nearly coincident geometry
/// becomes exactly coincident, which the classification handles exactly.
/// Faces whose vertices moved get their plane recomputed.
fn snap_to(original: &Solid, other: &Solid, index: &FaceIndex, tol: f64) -> Option<Solid> {
    if original.is_empty() || other.is_empty() {
        return None;
    }
    // Only vertices within the other solid's box can be near any of it.
    let (olo, ohi) = other.bounds()?;
    let near = |p: Vec3| {
        p.x >= olo.x - tol
            && p.x <= ohi.x + tol
            && p.y >= olo.y - tol
            && p.y <= ohi.y + tol
            && p.z >= olo.z - tol
            && p.z <= ohi.z + tol
    };
    let candidates: Vec<usize> = (0..original.vertices.len())
        .filter(|&vi| near(original.vertices[vi]))
        .collect();
    if candidates.is_empty() {
        return None;
    }
    let grid = crate::PointGrid::new(&other.vertices, tol);
    let mut moves: Vec<(usize, Vec3)> = Vec::new();
    for &vi in &candidates {
        let v = original.vertices[vi];
        // Vertex-to-vertex snap wins outright.
        if let Some(q) = grid
            .candidates_near_segment(v, v)
            .into_iter()
            .map(|i| other.vertices[i as usize])
            .find(|q| q.distance(v) <= tol && q.distance(v) > 0.0)
        {
            moves.push((vi, q));
            continue;
        }
        // Then onto up to three nearby face planes, one after another, so a
        // vertex near a corner of `other` lands on the corner. The box test
        // limits plane snaps to vertices near the face, not just its plane.
        let mut p = v;
        let mut hits = 0;
        for fi in index.faces_near_point(v, tol) {
            let f = &other.faces[fi];
            let d = f.plane.normal.dot(p - f.plane.origin);
            if d != 0.0 && d.abs() <= tol {
                p -= f.plane.normal * d;
                hits += 1;
                if hits == 3 {
                    break;
                }
            }
        }
        if hits > 0 {
            moves.push((vi, p));
        }
    }
    if moves.is_empty() {
        return None;
    }
    let mut snapped = original.clone();
    let s = &mut snapped;
    let mut moved = vec![false; s.vertices.len()];
    for &(vi, p) in &moves {
        s.vertices[vi] = p;
        moved[vi] = true;
    }
    // A face of `s` that is nearly coplanar with a face of `other` may have
    // had only some of its vertices snapped (the plane snap is limited to
    // the other face's box); the rest would leave it tilted by a hair and
    // no longer planar. Put every vertex of such a face onto that plane.
    for fi in 0..s.faces.len() {
        let verts: Vec<u32> = s.faces[fi].loops.iter().flatten().copied().collect();
        let Some(&anchor) = verts.iter().find(|&&v| moved[v as usize]) else {
            continue;
        };
        let on = |plane: &ok_math::Plane, p: Vec3| plane.normal.dot(p - plane.origin).abs() <= tol;
        let plane = index
            .faces_near_point(s.vertices[anchor as usize], tol)
            .into_iter()
            .map(|g| other.faces[g].plane)
            .find(|pl| {
                pl.normal.dot(s.faces[fi].plane.normal).abs() > 0.999
                    && verts.iter().all(|&v| on(pl, s.vertices[v as usize]))
            });
        let Some(plane) = plane else {
            continue;
        };
        for &v in &verts {
            let p = s.vertices[v as usize];
            let d = plane.normal.dot(p - plane.origin);
            if d != 0.0 {
                s.vertices[v as usize] = p - plane.normal * d;
                moved[v as usize] = true;
            }
        }
    }
    for f in &mut s.faces {
        if !f.loops[0].iter().any(|&v| moved[v as usize]) {
            continue;
        }
        let pts: Vec<Vec3> = f.loops[0].iter().map(|&v| s.vertices[v as usize]).collect();
        let Some(n) = crate::revolve::newell_normal(&pts).normalized() else {
            continue;
        };
        let x = f.plane.x_axis - n * f.plane.x_axis.dot(n);
        let Some(x) = x.normalized() else { continue };
        f.plane = ok_math::Plane {
            origin: pts[0],
            x_axis: x,
            y_axis: n.cross(x),
            normal: n,
        };
    }
    Some(snapped)
}

fn face_bounds(solid: &Solid, f: &Face) -> (Vec3, Vec3) {
    bounds_of(
        f.loops
            .iter()
            .flatten()
            .map(|&v| solid.vertices[v as usize]),
    )
    .unwrap_or((Vec3::ZERO, Vec3::ZERO))
}

/// Whether a face kept whole by `keep` when nothing of the other solid
/// comes near it (it is outside that solid).
fn keeps_outside(keep: Keep) -> bool {
    matches!(
        keep,
        Keep::NotAbove | Keep::NotAboveNorBelow | Keep::NotBelow
    )
}

pub fn boolean(a: &Solid, b: &Solid, op: BoolOp) -> Result<Solid, BrepError> {
    if a.is_empty() || b.is_empty() {
        return Ok(match op {
            BoolOp::Union => a.merged(b),
            BoolOp::Difference => a.clone(),
            BoolOp::Intersection => Solid::default(),
        });
    }
    let (ab, bb) = (a.bounds().unwrap(), b.bounds().unwrap());
    let diag = ((ab.1 - ab.0).length()).max((bb.1 - bb.0).length());
    let tol = merge_tolerance(diag) * 10.0;
    if !bounds_overlap(ab, bb, tol) {
        return Ok(match op {
            BoolOp::Union => a.merged(b),
            BoolOp::Difference => a.clone(),
            BoolOp::Intersection => Solid::default(),
        });
    }

    // Faces whose box reaches the other solid's box are the only ones the
    // operation can change; the rest pass through with their vertices.
    let touched = |s: &Solid, other_bounds: (Vec3, Vec3)| -> Vec<bool> {
        s.faces
            .iter()
            .map(|f| bounds_overlap(face_bounds(s, f), other_bounds, tol))
            .collect()
    };
    let (touched_a, touched_b) = (touched(a, bb), touched(b, ab));
    if !touched_a.iter().any(|t| *t) || !touched_b.iter().any(|t| *t) {
        // No face of one solid comes near the other's box, so no two faces
        // can meet: the two are apart, or one sits wholly inside the other
        // (or in one of its cavities).
        let b_in_a = a.contains(b.vertices[0]);
        let a_in_b = !b_in_a && b.contains(a.vertices[0]);
        return Ok(match op {
            BoolOp::Union if b_in_a => a.clone(),
            BoolOp::Union if a_in_b => b.clone(),
            BoolOp::Union => a.merged(b),
            BoolOp::Difference if b_in_a => a.merged(&b.flipped()),
            BoolOp::Difference if a_in_b => Solid::default(),
            BoolOp::Difference => a.clone(),
            BoolOp::Intersection if b_in_a => b.clone(),
            BoolOp::Intersection if a_in_b => a.clone(),
            BoolOp::Intersection => Solid::default(),
        });
    }

    // Nearly coincident geometry is made exactly coincident first (within
    // the tolerance the classification already treats as "on").
    let boxes_a0 = FaceIndex::new(a);
    let b_snapped = snap_to(b, a, &boxes_a0, tol);
    let b = b_snapped.as_ref().unwrap_or(b);
    let boxes_b = FaceIndex::new(b);
    let a_snapped = snap_to(a, b, &boxes_b, tol);
    let a = a_snapped.as_ref().unwrap_or(a);
    let boxes_a = if a_snapped.is_some() {
        FaceIndex::new(a)
    } else {
        boxes_a0
    };

    let (keep_a, keep_b, flip_b) = match op {
        BoolOp::Union => (Keep::NotAbove, Keep::NotAboveNorBelow, false),
        BoolOp::Difference => (Keep::NotBelow, Keep::Inside, true),
        BoolOp::Intersection => (Keep::Below, Keep::Inside, false),
    };

    // The result starts from both vertex arrays. Fragments are welded
    // against the vertices of the touched faces (and any vertex near the
    // other solid's box, in case a new corner lands within tolerance of
    // one); untouched faces keep their vertex ids, so nothing far from the
    // operation is rebuilt.
    let voff = a.vertices.len() as u32;
    let soff = a.surfaces.len();
    let mut vertices = a.vertices.clone();
    vertices.extend_from_slice(&b.vertices);
    let mut seed = vec![false; vertices.len()];
    let grown = (
        bb.0 - Vec3::new(3.0 * tol, 3.0 * tol, 3.0 * tol),
        bb.1 + Vec3::new(3.0 * tol, 3.0 * tol, 3.0 * tol),
    );
    for (v, p) in a.vertices.iter().enumerate() {
        if p.x >= grown.0.x
            && p.x <= grown.1.x
            && p.y >= grown.0.y
            && p.y <= grown.1.y
            && p.z >= grown.0.z
            && p.z <= grown.1.z
        {
            seed[v] = true;
        }
    }
    for (f, t) in a.faces.iter().zip(&touched_a) {
        if *t {
            for &v in f.loops.iter().flatten() {
                seed[v as usize] = true;
            }
        }
    }
    for s in seed.iter_mut().skip(voff as usize) {
        *s = true;
    }
    let mut merger = VertexMerger::seeded(
        vertices,
        seed.iter()
            .enumerate()
            .filter(|(_, s)| **s)
            .map(|(v, _)| v as u32),
        tol,
    );

    let mut faces: Vec<Face> = Vec::with_capacity(a.faces.len() + b.faces.len());
    for (f, t) in a.faces.iter().zip(&touched_a) {
        if !*t {
            if keeps_outside(keep_a) {
                faces.push(f.clone());
            }
            continue;
        }
        let kept = classify_face(a, f, b, &boxes_b, bb, keep_a, tol)?;
        match kept {
            Kept::Whole => faces.push(f.clone()),
            Kept::Nothing => {}
            Kept::Fragments(shapes) => {
                for poly in fragments_to_polygons(a, f, shapes, false, 0, tol) {
                    faces.extend(merger.weld(&poly));
                }
            }
        }
    }
    // A face of `b` kept whole, renumbered into the result (and turned
    // over for a difference).
    let whole_b = |f: &Face| -> Face {
        let mut loops: Vec<Vec<u32>> = f
            .loops
            .iter()
            .map(|l| l.iter().map(|v| v + voff).collect())
            .collect();
        let plane = if flip_b {
            for l in &mut loops {
                l.reverse();
            }
            flip_plane(&f.plane)
        } else {
            f.plane
        };
        Face {
            plane,
            loops,
            surface: f.surface + soff,
            origin: f.origin,
        }
    };
    for (f, t) in b.faces.iter().zip(&touched_b) {
        if !*t {
            if keeps_outside(keep_b) {
                faces.push(whole_b(f));
            }
            continue;
        }
        let kept = classify_face(b, f, a, &boxes_a, ab, keep_b, tol)?;
        match kept {
            Kept::Whole => faces.push(whole_b(f)),
            Kept::Nothing => {}
            Kept::Fragments(shapes) => {
                for poly in fragments_to_polygons(b, f, shapes, flip_b, soff, tol) {
                    faces.extend(merger.weld(&poly));
                }
            }
        }
    }
    let mut surfaces = a.surfaces.clone();
    surfaces.extend_from_slice(&b.surfaces);
    // Fragments outside the overlap of the two boxes are whole faces of a
    // valid input, so only edges reaching into it can have T-junctions.
    let overlap = (
        Vec3::new(ab.0.x.max(bb.0.x), ab.0.y.max(bb.0.y), ab.0.z.max(bb.0.z)),
        Vec3::new(ab.1.x.min(bb.1.x), ab.1.y.min(bb.1.y), ab.1.z.min(bb.1.z)),
    );
    let mut solid = Solid::assemble_incremental(merger, faces, surfaces, tol, overlap)?;
    solid.merge_coplanar_faces();
    solid.compact_surfaces();
    solid.validate()?;
    // New vertices lie where facet planes met; put them on the curves the
    // surfaces meet on, keeping the result as it is if that cannot close.
    let grown = (
        overlap.0 - Vec3::new(3.0 * tol, 3.0 * tol, 3.0 * tol),
        overlap.1 + Vec3::new(3.0 * tol, 3.0 * tol, 3.0 * tol),
    );
    if let Ok(Some(refitted)) = crate::exact::refit_within(&solid, Some(grown)) {
        solid = refitted;
    }
    Ok(solid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extrude;
    use ok_math::{Plane, Vec2};
    use ok_sketch::{ProfileOptions, Sketch};
    use std::f64::consts::PI;

    fn rect(plane: &Plane, a: Vec2, b: Vec2, start: f64, end: f64, id: u32) -> Solid {
        let mut s = Sketch::new();
        s.add_rectangle(a, b);
        let p = s.profiles(&ProfileOptions::default()).remove(0);
        extrude(&p, plane, start, end, id).unwrap()
    }

    /// Replays a case the fuzz dumped (`OK_FUZZ_DUMP`):
    /// `OK_BREP_CASE=<dir>/seed-59-step-3 OK_BREP_OP=union cargo test -p ok-brep --lib replay_dumped_case -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn replay_dumped_case() {
        let Ok(stem) = std::env::var("OK_BREP_CASE") else {
            return;
        };
        let load = |what: &str| -> Solid {
            serde_json::from_str(&std::fs::read_to_string(format!("{stem}-{what}.json")).unwrap())
                .unwrap()
        };
        let (a, b) = (load("body"), load("tool"));
        let op = match std::env::var("OK_BREP_OP").as_deref() {
            Ok("difference") => BoolOp::Difference,
            Ok("intersection") => BoolOp::Intersection,
            _ => BoolOp::Union,
        };
        eprintln!("a: {} faces, valid {:?}", a.faces.len(), a.validate());
        eprintln!("b: {} faces, valid {:?}", b.faces.len(), b.validate());
        let r = boolean(&a, &b, op).unwrap();
        eprintln!("result: {} faces, volume {}", r.faces.len(), r.volume());
        r.validate().unwrap();
    }

    /// A box whose bottom sits a hair above another box's bottom and whose
    /// side pokes out of it leaves a sliver strip of the first box's side
    /// face; every boolean must still close, whatever the gap size.
    #[test]
    fn slivers_of_every_size_stay_closed() {
        let a = rect(&Plane::XY, Vec2::ZERO, Vec2::new(3.0, 3.0), 0.0, 3.0, 1);
        for gap in [1e-2, 1e-3, 1e-4, 3e-5, 1.5e-5, 1e-5, 5e-6, 1e-6, 1e-7] {
            let b = rect(
                &Plane::XY,
                Vec2::new(2.0, 1.0),
                Vec2::new(4.0, 2.0),
                gap,
                1.0,
                2,
            );
            for (op, expect) in [
                (BoolOp::Union, 27.0 + (1.0 - gap)),
                (BoolOp::Difference, 27.0 - (1.0 - gap)),
                (BoolOp::Intersection, 1.0 - gap),
            ] {
                let r = boolean(&a, &b, op).unwrap_or_else(|e| panic!("gap {gap} {op:?}: {e}"));
                r.validate()
                    .unwrap_or_else(|e| panic!("gap {gap} {op:?}: {e}"));
                let v = r.volume();
                assert!(
                    (v - expect).abs() < 1e-3,
                    "gap {gap} {op:?}: volume {v} vs {expect}"
                );
            }
        }
    }

    fn cylinder(plane: &Plane, c: Vec2, r: f64, start: f64, end: f64, id: u32) -> Solid {
        let mut s = Sketch::new();
        s.add_circle(c, r);
        let p = s.profiles(&ProfileOptions::default()).remove(0);
        extrude(&p, plane, start, end, id).unwrap()
    }

    fn assert_vol(s: &Solid, expected: f64, rel: f64) {
        s.validate().unwrap();
        let v = s.volume();
        assert!(
            ((v - expected) / expected).abs() < rel,
            "volume {v}, expected {expected}"
        );
    }

    #[test]
    fn union_of_overlapping_boxes() {
        let a = rect(&Plane::XY, Vec2::ZERO, Vec2::new(4.0, 4.0), 0.0, 4.0, 1);
        let b = rect(
            &Plane::XY,
            Vec2::new(2.0, 2.0),
            Vec2::new(6.0, 6.0),
            2.0,
            6.0,
            2,
        );
        let u = boolean(&a, &b, BoolOp::Union).unwrap();
        assert_vol(&u, 64.0 + 64.0 - 8.0, 1e-9);
        assert_eq!(
            u.vertices.len(),
            8 + 8 + 6 - 2,
            "six new corners, two buried corners gone"
        );
    }

    #[test]
    fn difference_and_intersection_of_overlapping_boxes() {
        let a = rect(&Plane::XY, Vec2::ZERO, Vec2::new(4.0, 4.0), 0.0, 4.0, 1);
        let b = rect(
            &Plane::XY,
            Vec2::new(2.0, 2.0),
            Vec2::new(6.0, 6.0),
            2.0,
            6.0,
            2,
        );
        assert_vol(
            &boolean(&a, &b, BoolOp::Difference).unwrap(),
            64.0 - 8.0,
            1e-9,
        );
        assert_vol(&boolean(&a, &b, BoolOp::Intersection).unwrap(), 8.0, 1e-9);
    }

    #[test]
    fn union_of_stacked_boxes_removes_touching_faces() {
        // Boss sitting exactly on a plate: coplanar opposite-normal faces.
        let plate = rect(&Plane::XY, Vec2::ZERO, Vec2::new(10.0, 10.0), 0.0, 2.0, 1);
        let boss = rect(
            &Plane::XY,
            Vec2::new(3.0, 3.0),
            Vec2::new(7.0, 7.0),
            2.0,
            5.0,
            2,
        );
        let u = boolean(&plate, &boss, BoolOp::Union).unwrap();
        assert_vol(&u, 200.0 + 48.0, 1e-9);
        // Plate top became a face with a hole; boss bottom vanished: 6 + 5 faces.
        assert_eq!(u.faces.len(), 11);
    }

    #[test]
    fn union_of_flush_boxes_merges_side_by_side() {
        // Same-normal coplanar faces (tops flush) and a shared internal wall.
        let a = rect(&Plane::XY, Vec2::ZERO, Vec2::new(4.0, 4.0), 0.0, 4.0, 1);
        let b = rect(
            &Plane::XY,
            Vec2::new(4.0, 0.0),
            Vec2::new(8.0, 4.0),
            0.0,
            4.0,
            2,
        );
        let u = boolean(&a, &b, BoolOp::Union).unwrap();
        assert_vol(&u, 128.0, 1e-9);
        assert_eq!(u.faces.len(), 6, "internal wall gone; flush faces merged");
        assert_eq!(
            u.vertices.len(),
            12,
            "merged faces keep the mid-edge vertices"
        );
    }

    #[test]
    fn through_hole_cut() {
        let plate = rect(&Plane::XY, Vec2::ZERO, Vec2::new(20.0, 20.0), 0.0, 5.0, 1);
        let drill = cylinder(&Plane::XY, Vec2::new(10.0, 10.0), 3.0, -1.0, 6.0, 2);
        let cut = boolean(&plate, &drill, BoolOp::Difference).unwrap();
        assert_vol(&cut, 2000.0 - PI * 9.0 * 5.0, 2e-3);
        // Top and bottom each have one hole loop now.
        let holed = cut.faces.iter().filter(|f| f.loops.len() == 2).count();
        assert_eq!(holed, 2);
    }

    #[test]
    fn flush_hole_cut_from_top_face() {
        // Drill starts exactly on the top face (coplanar, same normal).
        let plate = rect(&Plane::XY, Vec2::ZERO, Vec2::new(20.0, 20.0), 0.0, 5.0, 1);
        let drill = cylinder(&Plane::XY, Vec2::new(10.0, 10.0), 3.0, 5.0, 2.0, 2);
        let cut = boolean(&plate, &drill, BoolOp::Difference).unwrap();
        assert_vol(&cut, 2000.0 - PI * 9.0 * 3.0, 2e-3);
        assert_eq!(
            cut.faces.iter().filter(|f| f.loops.len() == 2).count(),
            1,
            "only the top has a hole"
        );
    }

    #[test]
    fn cut_that_splits_a_body_in_two() {
        let bar = rect(&Plane::XY, Vec2::ZERO, Vec2::new(10.0, 2.0), 0.0, 2.0, 1);
        let saw = rect(
            &Plane::XY,
            Vec2::new(4.0, -1.0),
            Vec2::new(6.0, 3.0),
            -1.0,
            3.0,
            2,
        );
        let cut = boolean(&bar, &saw, BoolOp::Difference).unwrap();
        assert_vol(&cut, 32.0, 1e-9);
        assert_eq!(cut.shells().len(), 2);
    }

    #[test]
    fn boss_with_hole_on_plate_then_drill_through() {
        // The demo part: plate with a hole, boss with the same hole on top,
        // union, then a side cut through the boss.
        let mut s = Sketch::new();
        s.add_rectangle(Vec2::ZERO, Vec2::new(60.0, 40.0));
        s.add_circle(Vec2::new(30.0, 20.0), 6.0);
        let mut p = s.profiles(&ProfileOptions::default());
        p.sort_by(|a, b| b.area().partial_cmp(&a.area()).unwrap());
        let plate = extrude(&p[0], &Plane::XY, 0.0, 8.0, 1).unwrap();

        let mut s2 = Sketch::new();
        s2.add_circle(Vec2::new(30.0, 20.0), 10.0);
        s2.add_circle(Vec2::new(30.0, 20.0), 6.0);
        let mut p2 = s2.profiles(&ProfileOptions::default());
        p2.sort_by(|a, b| b.area().partial_cmp(&a.area()).unwrap());
        let boss = extrude(&p2[0], &Plane::XY.offset(8.0), 0.0, 6.0, 2).unwrap();

        let u = boolean(&plate, &boss, BoolOp::Union).unwrap();
        let expected = 60.0 * 40.0 * 8.0 - PI * 36.0 * 8.0 + PI * (100.0 - 36.0) * 6.0;
        assert_vol(&u, expected, 2e-3);

        let slot = rect(
            &Plane::XZ,
            Vec2::new(25.0, 10.0),
            Vec2::new(35.0, 20.0),
            -50.0,
            50.0,
            3,
        );
        let cut = boolean(&u, &slot, BoolOp::Difference).unwrap();
        assert!(cut.volume() < u.volume());
        cut.validate().unwrap();
    }

    #[test]
    fn cylinder_union_cylinder_crosswise() {
        let a = cylinder(&Plane::XY, Vec2::ZERO, 2.0, -5.0, 5.0, 1);
        let b = cylinder(&Plane::XZ, Vec2::ZERO, 2.0, -5.0, 5.0, 2);
        let u = boolean(&a, &b, BoolOp::Union).unwrap();
        // Steinmetz solid volume for the intersection: 16 r^3 / 3.
        let expected = 2.0 * PI * 4.0 * 10.0 - 16.0 * 8.0 / 3.0;
        assert_vol(&u, expected, 5e-3);
        assert_vol(
            &boolean(&a, &b, BoolOp::Intersection).unwrap(),
            16.0 * 8.0 / 3.0,
            5e-3,
        );
    }

    #[test]
    fn disjoint_union_keeps_both_lumps() {
        let a = rect(&Plane::XY, Vec2::ZERO, Vec2::new(1.0, 1.0), 0.0, 1.0, 1);
        let b = rect(
            &Plane::XY,
            Vec2::new(5.0, 5.0),
            Vec2::new(6.0, 6.0),
            0.0,
            1.0,
            2,
        );
        let u = boolean(&a, &b, BoolOp::Union).unwrap();
        assert_vol(&u, 2.0, 1e-9);
        assert!(boolean(&a, &b, BoolOp::Intersection).unwrap().is_empty());
    }

    #[test]
    fn identical_solids() {
        let a = rect(&Plane::XY, Vec2::ZERO, Vec2::new(3.0, 2.0), 0.0, 1.0, 1);
        let b = rect(&Plane::XY, Vec2::ZERO, Vec2::new(3.0, 2.0), 0.0, 1.0, 2);
        assert_vol(&boolean(&a, &b, BoolOp::Union).unwrap(), 6.0, 1e-9);
        assert_vol(&boolean(&a, &b, BoolOp::Intersection).unwrap(), 6.0, 1e-9);
        assert!(boolean(&a, &b, BoolOp::Difference).unwrap().is_empty());
    }

    #[test]
    fn contained_solids() {
        let big = rect(&Plane::XY, Vec2::ZERO, Vec2::new(10.0, 10.0), 0.0, 10.0, 1);
        let small = rect(
            &Plane::XY,
            Vec2::new(2.0, 2.0),
            Vec2::new(4.0, 4.0),
            2.0,
            4.0,
            2,
        );
        assert_vol(&boolean(&big, &small, BoolOp::Union).unwrap(), 1000.0, 1e-9);
        assert_vol(
            &boolean(&small, &big, BoolOp::Intersection).unwrap(),
            8.0,
            1e-9,
        );
        assert!(boolean(&small, &big, BoolOp::Difference)
            .unwrap()
            .is_empty());
        // A fully buried void: outer shell plus inverted inner shell.
        let hollow = boolean(&big, &small, BoolOp::Difference).unwrap();
        assert_vol(&hollow, 992.0, 1e-9);
        assert_eq!(hollow.shells().len(), 2);
    }

    #[test]
    fn apart_and_nested_solids_take_the_fast_path() {
        // Boxes whose overall boxes overlap but whose faces stay apart:
        // the hole drills of a pattern, unioned into one tool.
        let a = rect(&Plane::XY, Vec2::ZERO, Vec2::new(1.0, 1.0), 0.0, 1.0, 1);
        let far = rect(
            &Plane::XY,
            Vec2::new(5.0, 5.0),
            Vec2::new(6.0, 6.0),
            0.0,
            1.0,
            2,
        );
        let tool = boolean(&a, &far, BoolOp::Union).unwrap();
        let c = rect(
            &Plane::XY,
            Vec2::new(5.0, 0.0),
            Vec2::new(6.0, 1.0),
            0.0,
            1.0,
            3,
        );
        let tool = boolean(&tool, &c, BoolOp::Union).unwrap();
        assert_vol(&tool, 3.0, 1e-9);
        assert_eq!(tool.faces.len(), 18);
        assert!(boolean(&tool, &c, BoolOp::Intersection).unwrap().volume() > 0.0);
        // A box wholly inside another, no faces near: union swallows it,
        // difference leaves a void, intersection is the inner box.
        let big = rect(&Plane::XY, Vec2::ZERO, Vec2::new(10.0, 10.0), 0.0, 10.0, 1);
        let inner = rect(
            &Plane::XY,
            Vec2::new(4.0, 4.0),
            Vec2::new(6.0, 6.0),
            4.0,
            6.0,
            2,
        );
        assert!(big.contains(Vec3::new(5.0, 5.0, 5.0)));
        assert!(!big.contains(Vec3::new(11.0, 5.0, 5.0)));
        assert_eq!(boolean(&big, &inner, BoolOp::Union).unwrap().faces.len(), 6);
        assert_eq!(boolean(&inner, &big, BoolOp::Union).unwrap().faces.len(), 6);
        let hollow = boolean(&big, &inner, BoolOp::Difference).unwrap();
        assert_vol(&hollow, 992.0, 1e-9);
        assert_eq!(hollow.shells().len(), 2);
        assert!(
            !hollow.contains(Vec3::new(5.0, 5.0, 5.0)),
            "the void is outside"
        );
        assert!(hollow.contains(Vec3::new(1.0, 1.0, 1.0)));
        assert!(boolean(&inner, &big, BoolOp::Difference)
            .unwrap()
            .is_empty());
        assert_vol(
            &boolean(&big, &inner, BoolOp::Intersection).unwrap(),
            8.0,
            1e-9,
        );
        assert_vol(
            &boolean(&inner, &big, BoolOp::Intersection).unwrap(),
            8.0,
            1e-9,
        );
        // A box in the void of the hollow one is apart from its material.
        let speck = rect(
            &Plane::XY,
            Vec2::new(4.5, 4.5),
            Vec2::new(5.5, 5.5),
            4.5,
            5.5,
            3,
        );
        let u = boolean(&hollow, &speck, BoolOp::Union).unwrap();
        assert_vol(&u, 993.0, 1e-9);
        assert_eq!(u.shells().len(), 3);
    }

    #[test]
    fn untouched_faces_keep_their_vertices_and_the_rest_is_welded() {
        // A drill through one end of a long bar: the far end's faces and
        // vertices come through unchanged, and the result is compact.
        let bar = rect(&Plane::XY, Vec2::ZERO, Vec2::new(100.0, 10.0), 0.0, 5.0, 1);
        let drill = cylinder(&Plane::XY, Vec2::new(90.0, 5.0), 2.0, -1.0, 6.0, 2);
        let cut = boolean(&bar, &drill, BoolOp::Difference).unwrap();
        assert_vol(&cut, 5000.0 - PI * 4.0 * 5.0, 2e-3);
        let far_end = cut
            .faces
            .iter()
            .find(|f| f.plane.normal.x < -0.99)
            .expect("x = 0 end face");
        assert_eq!(far_end.loops[0].len(), 4);
        let used: std::collections::BTreeSet<u32> = cut
            .faces
            .iter()
            .flat_map(|f| f.loops.iter().flatten().copied())
            .collect();
        assert_eq!(used.len(), cut.vertices.len(), "no unused vertices");
        // Two rims of the drill (top and bottom of the hole) plus the bar's
        // eight corners.
        assert_eq!(cut.vertices.len(), 8 + 2 * (drill.vertices.len() / 2));
    }

    #[test]
    fn stacked_boxes_with_identical_footprint() {
        let a = rect(&Plane::XY, Vec2::ZERO, Vec2::new(3.0, 3.0), 0.0, 1.0, 1);
        let b = rect(&Plane::XY, Vec2::ZERO, Vec2::new(3.0, 3.0), 1.0, 2.0, 2);
        let u = boolean(&a, &b, BoolOp::Union).unwrap();
        assert_vol(&u, 18.0, 1e-9);
        assert_eq!(u.faces.len(), 6, "touching caps vanish, walls merge");
    }

    #[test]
    fn blind_pocket_cut_from_top() {
        let block = rect(&Plane::XY, Vec2::ZERO, Vec2::new(10.0, 10.0), 0.0, 5.0, 1);
        let pocket = rect(
            &Plane::XY.offset(5.0),
            Vec2::new(2.0, 2.0),
            Vec2::new(8.0, 8.0),
            0.0,
            -3.0,
            2,
        );
        let cut = boolean(&block, &pocket, BoolOp::Difference).unwrap();
        assert_vol(&cut, 500.0 - 108.0, 1e-9);
        assert_eq!(cut.faces.len(), 6 + 5);
    }

    #[test]
    fn edge_to_edge_touching_boxes_do_not_merge_volume() {
        // Boxes sharing only an edge: union is a valid two-lump solid.
        let a = rect(&Plane::XY, Vec2::ZERO, Vec2::new(2.0, 2.0), 0.0, 2.0, 1);
        let b = rect(
            &Plane::XY,
            Vec2::new(2.0, 2.0),
            Vec2::new(4.0, 4.0),
            0.0,
            2.0,
            2,
        );
        let u = boolean(&a, &b, BoolOp::Union).unwrap();
        assert_vol(&u, 16.0, 1e-9);
    }

    #[test]
    fn merged_faces_survive_further_booleans() {
        let a = rect(&Plane::XY, Vec2::ZERO, Vec2::new(4.0, 4.0), 0.0, 4.0, 1);
        let b = rect(
            &Plane::XY,
            Vec2::new(4.0, 0.0),
            Vec2::new(8.0, 4.0),
            0.0,
            4.0,
            2,
        );
        let u = boolean(&a, &b, BoolOp::Union).unwrap();
        let drill = cylinder(&Plane::XY, Vec2::new(4.0, 2.0), 1.0, -1.0, 5.0, 3);
        let cut = boolean(&u, &drill, BoolOp::Difference).unwrap();
        assert_vol(&cut, 128.0 - PI * 4.0, 2e-3);
        assert_eq!(
            cut.faces.iter().filter(|f| f.loops.len() == 2).count(),
            2,
            "one hole in the merged top and bottom"
        );
    }

    #[test]
    fn repeated_operations_stay_valid() {
        let mut body = rect(&Plane::XY, Vec2::ZERO, Vec2::new(30.0, 30.0), 0.0, 10.0, 1);
        for i in 0..4 {
            let x = 6.0 + 6.0 * i as f64;
            let drill = cylinder(&Plane::XY, Vec2::new(x, 15.0), 2.0, -1.0, 11.0, 10 + i);
            body = boolean(&body, &drill, BoolOp::Difference).unwrap();
        }
        assert_vol(&body, 9000.0 - 4.0 * PI * 4.0 * 10.0, 2e-3);
    }
}
