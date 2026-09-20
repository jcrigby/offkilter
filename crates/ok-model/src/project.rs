//! Projecting body geometry into sketches ("Use").
//!
//! A [`Projection`] names an edge or a face of the bodies that exist before
//! the sketch. On every regeneration its display segments are projected
//! onto the sketch plane, chained, recognised as circles or arcs where the
//! source lies on a cylinder whose axis is normal to the plane (or where the
//! chain fits a circle), and written into the sketch as fixed entities with
//! ids from the projection's reserved block. Entities keep their ids across
//! regenerations when the shape is unchanged, so constraints the user
//! attached to them survive model edits.

use crate::feature::{Projection, ProjectionSource, SketchFeature, PROJECTION_BLOCK};
use crate::regen::RegenResult;
use ok_brep::Surface;
use ok_math::{tol, Plane, Vec2};

/// Projected endpoints closer than this are one sketch point.
const MERGE: f64 = 1e-6;
use ok_sketch::{Entity, EntityId};

/// A projected display segment with the circle it lies on, if known.
struct Seg {
    a: Vec2,
    b: Vec2,
    circle: Option<(Vec2, f64)>,
}

/// Rebuilds every projection of `sf` from `result`'s bodies.
pub(crate) fn update_projections(
    result: &RegenResult,
    plane: &Plane,
    sf: &mut SketchFeature,
) -> Result<(), String> {
    let mut projections = std::mem::take(&mut sf.projections);
    let mut error = None;
    for p in &mut projections {
        if let Err(e) = update_one(result, plane, sf, p) {
            error.get_or_insert(e);
        }
    }
    sf.projections = projections;
    error.map_or(Ok(()), Err)
}

fn update_one(
    result: &RegenResult,
    plane: &Plane,
    sf: &mut SketchFeature,
    p: &mut Projection,
) -> Result<(), String> {
    let segs = source_segments(result, plane, &p.source);
    if segs.is_empty() {
        return Err(match p.source {
            ProjectionSource::Edge { .. } => {
                "projected edge not found: its faces are missing or created after this sketch"
                    .into()
            }
            ProjectionSource::Face { .. } => {
                "projected face not found: it is missing or created after this sketch".into()
            }
        });
    }
    let entities = build_entities(&segs, plane, p.block)?;
    // Same ids and kinds as before: update in place so user constraints on
    // the projected points survive. Otherwise rebuild the block.
    let same_shape = entities.len() == p.entities.len()
        && entities.iter().zip(&p.entities).all(|((id, e), old)| {
            id == old && sf.sketch.entity(*id).is_some_and(|cur| same_kind(cur, e))
        });
    if !same_shape {
        for id in p.entities.drain(..) {
            sf.sketch.remove_entity(id);
        }
        // Points first so dependents never dangle; ids are already ordered that way.
        for (id, e) in &entities {
            sf.sketch
                .insert_projected(*id, e.clone())
                .map_err(|e| e.to_string())?;
        }
        p.entities = entities.iter().map(|(id, _)| *id).collect();
    } else {
        for (id, e) in &entities {
            sf.sketch.replace_projected(*id, e.clone());
        }
    }
    Ok(())
}

fn same_kind(a: &Entity, b: &Entity) -> bool {
    match (a, b) {
        (Entity::Point { .. }, Entity::Point { .. }) => true,
        (Entity::Line { start: s0, end: e0 }, Entity::Line { start: s1, end: e1 }) => {
            s0 == s1 && e0 == e1
        }
        (Entity::Circle { center: c0, .. }, Entity::Circle { center: c1, .. }) => c0 == c1,
        (
            Entity::Arc {
                center: c0,
                start: s0,
                end: e0,
            },
            Entity::Arc {
                center: c1,
                start: s1,
                end: e1,
            },
        ) => c0 == c1 && s0 == s1 && e0 == e1,
        _ => false,
    }
}

/// Display segments of the source across all bodies, projected onto `plane`.
///
/// Faces are matched by origin, then widened to every face of the body on
/// the same surface, so one rim segment picked on a faceted cylinder
/// projects the whole rim, as the user sees it.
fn source_segments(result: &RegenResult, plane: &Plane, source: &ProjectionSource) -> Vec<Seg> {
    let mut out = Vec::new();
    for body in &result.bodies {
        let solid = &body.solid;
        let surfaces_of = |face: &crate::FaceRef| -> Vec<usize> {
            let mut v: Vec<usize> = crate::regen::faces_of_ref(solid, face)
                .into_iter()
                .map(|i| solid.faces[i].surface)
                .collect();
            v.sort_unstable();
            v.dedup();
            v
        };
        let (sa, sb) = match source {
            ProjectionSource::Edge { edge } => (surfaces_of(&edge.a), surfaces_of(&edge.b)),
            ProjectionSource::Face { face } => (surfaces_of(face), Vec::new()),
        };
        if sa.is_empty() {
            continue;
        }
        for ((va, vb), faces) in solid.edge_faces() {
            let [f0, f1] = faces.as_slice() else {
                continue;
            };
            let (s0, s1) = (solid.faces[*f0].surface, solid.faces[*f1].surface);
            let wanted = match source {
                ProjectionSource::Edge { .. } => {
                    (sa.contains(&s0) && sb.contains(&s1)) || (sa.contains(&s1) && sb.contains(&s0))
                }
                ProjectionSource::Face { .. } => sa.contains(&s0) != sa.contains(&s1),
            };
            if !wanted {
                continue;
            }
            let a = plane.to_plane(solid.vertices[va as usize]);
            let b = plane.to_plane(solid.vertices[vb as usize]);
            if a.distance(b) <= MERGE {
                continue; // edge along the plane normal: projects to a point
            }
            let circle = [s0, s1].iter().find_map(|s| match solid.surfaces[*s] {
                Surface::Cylinder {
                    origin,
                    axis,
                    radius,
                } if (axis.dot(plane.normal).abs() - 1.0).abs() <= tol::ANGULAR => {
                    Some((plane.to_plane(origin), radius))
                }
                _ => None,
            });
            out.push(Seg { a, b, circle });
        }
    }
    out
}

/// A chain of projected segments through shared endpoints.
struct Chain {
    points: Vec<Vec2>,
    closed: bool,
    /// Circle every segment lies on, if the surface tags say so.
    circle: Option<(Vec2, f64)>,
}

fn chains(segs: &[Seg]) -> Vec<Chain> {
    // Merge endpoints into nodes.
    let mut nodes: Vec<Vec2> = Vec::new();
    let node_of = |p: Vec2, nodes: &mut Vec<Vec2>| -> usize {
        if let Some(i) = nodes.iter().position(|n| n.distance(p) <= MERGE) {
            return i;
        }
        nodes.push(p);
        nodes.len() - 1
    };
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for s in segs {
        let a = node_of(s.a, &mut nodes);
        let b = node_of(s.b, &mut nodes);
        if a != b && !edges.contains(&(a, b)) && !edges.contains(&(b, a)) {
            edges.push((a, b));
        }
    }
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for (i, (a, b)) in edges.iter().enumerate() {
        adj[*a].push(i);
        adj[*b].push(i);
    }
    let seg_circle = |ei: usize| -> Option<(Vec2, f64)> {
        let (a, b) = edges[ei];
        segs.iter()
            .find(|s| {
                let (na, nb) = (nodes[a], nodes[b]);
                (s.a.distance(na) <= MERGE && s.b.distance(nb) <= MERGE)
                    || (s.a.distance(nb) <= MERGE && s.b.distance(na) <= MERGE)
            })
            .and_then(|s| s.circle)
    };
    let mut used = vec![false; edges.len()];
    let mut out = Vec::new();
    // Walk from a node along unused edges through degree-2 nodes.
    let walk = |start: usize, used: &mut Vec<bool>| -> Option<Chain> {
        let first = *adj[start].iter().find(|e| !used[**e])?;
        let mut points = vec![nodes[start]];
        let mut circle = seg_circle(first);
        let mut all_circle = circle.is_some();
        let mut cur = start;
        let mut e = first;
        loop {
            used[e] = true;
            let (a, b) = edges[e];
            let next = if a == cur { b } else { a };
            points.push(nodes[next]);
            if let (Some(c0), Some(c1)) = (circle, seg_circle(e)) {
                if c0.0.distance(c1.0) > MERGE || !tol::approx_eq(c0.1, c1.1) {
                    all_circle = false;
                }
            } else {
                all_circle = false;
            }
            cur = next;
            if cur == start {
                points.pop();
                return Some(Chain {
                    points,
                    closed: true,
                    circle: circle.filter(|_| all_circle),
                });
            }
            if adj[cur].len() != 2 {
                break;
            }
            match adj[cur].iter().find(|x| !used[**x]) {
                Some(n) => e = *n,
                None => break,
            }
        }
        if !all_circle {
            circle = None;
        }
        Some(Chain {
            points,
            closed: false,
            circle,
        })
    };
    // Open chains first (from nodes that are not simple pass-throughs), then
    // whatever remains forms closed loops.
    let ends: Vec<usize> = (0..nodes.len()).filter(|n| adj[*n].len() != 2).collect();
    for n in ends.into_iter().chain(0..nodes.len()) {
        while let Some(c) = walk(n, &mut used) {
            out.push(c);
        }
    }
    out
}

/// Circle through three points, if they are not collinear.
fn circumcircle(a: Vec2, b: Vec2, c: Vec2) -> Option<(Vec2, f64)> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if d.abs() <= MERGE {
        return None;
    }
    let (a2, b2, c2) = (a.dot(a), b.dot(b), c.dot(c));
    let ux = (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d;
    let uy = (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d;
    let center = Vec2::new(ux, uy);
    Some((center, center.distance(a)))
}

/// The circle a chain lies on: from the surface tags, or by fitting when
/// every point is equidistant from the circumcentre of three of them.
fn chain_circle(c: &Chain) -> Option<(Vec2, f64)> {
    if let Some(k) = c.circle {
        return Some(k);
    }
    // Fitting: many points, all on one circle, none of them turning sharply
    // (a rectangle's corners are concyclic too, but 90° apart).
    let n = c.points.len();
    if n < 8 {
        return None;
    }
    let (center, r) = circumcircle(c.points[0], c.points[n / 2], c.points[n - 1])?;
    let on_circle = c
        .points
        .iter()
        .all(|p| (p.distance(center) - r).abs() <= 1e-6 * r.max(1.0));
    let gentle = c.points.windows(2).all(|w| {
        let (u, v) = (w[0] - center, w[1] - center);
        u.dot(v) >= r * r * (30.0f64).to_radians().cos()
    });
    (on_circle && gentle).then_some((center, r))
}

/// Sketch entities for the chains, with ids `block + k`.
fn build_entities(
    segs: &[Seg],
    _plane: &Plane,
    block: EntityId,
) -> Result<Vec<(EntityId, Entity)>, String> {
    let mut out: Vec<(EntityId, Entity)> = Vec::new();
    let mut next = block.0;
    let mut alloc = |out: &mut Vec<(EntityId, Entity)>, e: Entity| -> Result<EntityId, String> {
        if next - block.0 >= PROJECTION_BLOCK {
            return Err(format!("projection is too detailed ({} segments); lower the facet resolution or project fewer edges", segs.len()));
        }
        let id = EntityId(next);
        next += 1;
        out.push((id, e));
        Ok(id)
    };
    for c in chains(segs) {
        if let Some((center, radius)) = chain_circle(&c) {
            let cid = alloc(&mut out, Entity::Point { pos: center })?;
            if c.closed {
                alloc(
                    &mut out,
                    Entity::Circle {
                        center: cid,
                        radius,
                    },
                )?;
            } else {
                let (first, last) = (c.points[0], *c.points.last().unwrap());
                // Arcs run counter-clockwise from start to end.
                let sweep: f64 = c
                    .points
                    .windows(2)
                    .map(|w| (w[0] - center).cross(w[1] - center))
                    .sum();
                let (s, e) = if sweep >= 0.0 {
                    (first, last)
                } else {
                    (last, first)
                };
                let sid = alloc(&mut out, Entity::Point { pos: s })?;
                let eid = alloc(&mut out, Entity::Point { pos: e })?;
                alloc(
                    &mut out,
                    Entity::Arc {
                        center: cid,
                        start: sid,
                        end: eid,
                    },
                )?;
            }
            continue;
        }
        let mut ids = Vec::with_capacity(c.points.len());
        for p in &c.points {
            ids.push(alloc(&mut out, Entity::Point { pos: *p })?);
        }
        let n = ids.len();
        let lines = if c.closed { n } else { n - 1 };
        for i in 0..lines {
            alloc(
                &mut out,
                Entity::Line {
                    start: ids[i],
                    end: ids[(i + 1) % n],
                },
            )?;
        }
    }
    Ok(out)
}
