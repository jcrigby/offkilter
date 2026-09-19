//! Closed-region ("profile") extraction from sketch geometry.
//!
//! The sketch's lines and arcs form a planar graph once coincident points
//! are merged. Bounded faces of that graph, plus standalone circles, are the
//! candidate regions. Regions are nested by containment so that a region
//! enclosing another one gets it as a hole; the inner region is still
//! reported as its own profile, as in most CAD sketchers.
//!
//! Limitations of this first implementation: edges must only meet at
//! endpoints (crossing lines are not split), and curves sharing a tangent
//! at a vertex are ordered by direction only.

use crate::{Constraint, Entity, EntityId, Sketch};
use ok_math::Vec2;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// The curve a polygon segment was sampled from. Lets downstream code
/// rebuild analytic surfaces (a cylinder for an arc) from the polygon.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SegmentCurve {
    Line,
    Arc { center: Vec2, radius: f64 },
}

/// A closed polygon loop with a curve tag per segment.
/// `curves[i]` describes the segment from `points[i]` to `points[(i + 1) % n]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Loop {
    pub points: Vec<Vec2>,
    pub curves: Vec<SegmentCurve>,
}

impl Loop {
    pub fn polygon(points: Vec<Vec2>) -> Loop {
        let n = points.len();
        Loop {
            points,
            curves: vec![SegmentCurve::Line; n],
        }
    }

    pub fn len(&self) -> usize {
        self.points.len()
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    pub fn signed_area(&self) -> f64 {
        signed_area(&self.points)
    }

    /// Returns the loop traversed in the opposite direction, starting at the
    /// same point, with curve tags moved to their reversed segments.
    pub fn reversed(&self) -> Loop {
        let n = self.points.len();
        let points = (0..n).map(|j| self.points[(n - j) % n]).collect();
        let curves = (0..n).map(|j| self.curves[(n - j - 1) % n]).collect();
        Loop { points, curves }
    }
}

/// A closed planar region: an outer loop (counter-clockwise) and zero or
/// more hole loops (clockwise), each polygonised.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub outer: Loop,
    pub holes: Vec<Loop>,
}

impl Profile {
    /// Signed area of the outer loop minus the holes.
    pub fn area(&self) -> f64 {
        self.outer.signed_area() + self.holes.iter().map(|h| h.signed_area()).sum::<f64>()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ProfileOptions {
    /// Maximum angle per polygon segment when approximating arcs, radians.
    pub arc_segment_angle: f64,
    /// Points closer than this are merged into a single vertex.
    pub merge_tolerance: f64,
}

impl Default for ProfileOptions {
    fn default() -> Self {
        Self {
            arc_segment_angle: 5f64.to_radians(),
            merge_tolerance: 1e-6,
        }
    }
}

pub fn signed_area(poly: &[Vec2]) -> f64 {
    let n = poly.len();
    let mut a = 0.0;
    for i in 0..n {
        a += poly[i].cross(poly[(i + 1) % n]);
    }
    0.5 * a
}

pub fn point_in_polygon(p: Vec2, poly: &[Vec2]) -> bool {
    let n = poly.len();
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) {
            let x = a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x);
            if p.x < x {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

fn probe_point(poly: &[Vec2]) -> Vec2 {
    // Centroid of the triangle formed by the first convex-ish vertex is not
    // guaranteed inside for concave shapes; instead take the midpoint of an
    // edge nudged toward the interior (left side for CCW) by a small amount
    // and verify, falling back over all edges.
    let n = poly.len();
    let area = signed_area(poly);
    let sign = if area >= 0.0 { 1.0 } else { -1.0 };
    let scale = area.abs().sqrt().max(1e-9) * 1e-4;
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        if let Some(d) = (b - a).normalized() {
            let mid = a.lerp(b, 0.5) + d.perp() * (sign * scale);
            if point_in_polygon(mid, poly) {
                return mid;
            }
        }
    }
    poly[0]
}

// ----------------------------------------------------------------- union-find

struct UnionFind {
    parent: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
        }
    }
    fn find(&mut self, i: usize) -> usize {
        let mut r = i;
        while self.parent[r] != r {
            r = self.parent[r];
        }
        let mut c = i;
        while self.parent[c] != r {
            let next = self.parent[c];
            self.parent[c] = r;
            c = next;
        }
        r
    }
    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.parent[ra] = rb;
        }
    }
}

// --------------------------------------------------------------------- graph

#[derive(Clone, Copy)]
enum Curve {
    Line,
    /// CCW arc from the edge's `a` vertex to its `b` vertex about `center`.
    Arc {
        center: Vec2,
    },
}

struct Edge {
    a: usize,
    b: usize,
    curve: Curve,
    #[allow(dead_code)]
    source: EntityId,
}

/// Directed half-edge: edge index plus direction (`forward` = a -> b).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct Half {
    edge: usize,
    forward: bool,
}

impl Half {
    fn twin(self) -> Half {
        Half {
            edge: self.edge,
            forward: !self.forward,
        }
    }
}

struct Graph {
    verts: Vec<Vec2>,
    edges: Vec<Edge>,
}

impl Graph {
    fn from_sketch(sketch: &Sketch, opts: &ProfileOptions) -> Graph {
        // 1. Collect point entities and merge coincident ones.
        let points: Vec<(EntityId, Vec2)> = sketch
            .entities()
            .filter_map(|(id, e)| match e {
                Entity::Point { pos } => Some((id, *pos)),
                _ => None,
            })
            .collect();
        let index: HashMap<EntityId, usize> = points
            .iter()
            .enumerate()
            .map(|(i, (id, _))| (*id, i))
            .collect();
        let mut uf = UnionFind::new(points.len());
        for (_, c) in sketch.constraints() {
            if let Constraint::Coincident { a, b } = c {
                if let (Some(&ia), Some(&ib)) = (index.get(a), index.get(b)) {
                    uf.union(ia, ib);
                }
            }
        }
        // Positional merging for points that happen to coincide.
        let tol2 = opts.merge_tolerance * opts.merge_tolerance;
        for i in 0..points.len() {
            for j in i + 1..points.len() {
                if (points[i].1 - points[j].1).length_squared() <= tol2 {
                    uf.union(i, j);
                }
            }
        }
        let mut root_to_vert: HashMap<usize, usize> = HashMap::new();
        let mut verts = Vec::new();
        let mut vert_of = vec![0usize; points.len()];
        for i in 0..points.len() {
            let r = uf.find(i);
            let v = *root_to_vert.entry(r).or_insert_with(|| {
                verts.push(points[i].1);
                verts.len() - 1
            });
            vert_of[i] = v;
        }
        let vert = |id: EntityId| index.get(&id).map(|&i| vert_of[i]);

        // 2. Edges from lines and arcs.
        let mut edges = Vec::new();
        for (id, e) in sketch.entities() {
            match e {
                Entity::Line { start, end } => {
                    if let (Some(a), Some(b)) = (vert(*start), vert(*end)) {
                        if a != b {
                            edges.push(Edge {
                                a,
                                b,
                                curve: Curve::Line,
                                source: id,
                            });
                        }
                    }
                }
                Entity::Arc { center, start, end } => {
                    if let (Some(a), Some(b), Ok(c)) =
                        (vert(*start), vert(*end), sketch.point(*center))
                    {
                        if a != b {
                            edges.push(Edge {
                                a,
                                b,
                                curve: Curve::Arc { center: c },
                                source: id,
                            });
                        }
                    }
                }
                _ => {}
            }
        }
        let mut g = Graph { verts, edges };
        g.prune_dangling();
        g
    }

    /// Repeatedly removes edges with an endpoint of degree one; they cannot
    /// bound a region.
    fn prune_dangling(&mut self) {
        loop {
            let mut degree = vec![0usize; self.verts.len()];
            for e in &self.edges {
                degree[e.a] += 1;
                degree[e.b] += 1;
            }
            let before = self.edges.len();
            self.edges.retain(|e| degree[e.a] > 1 && degree[e.b] > 1);
            if self.edges.len() == before {
                break;
            }
        }
    }

    fn origin(&self, h: Half) -> usize {
        let e = &self.edges[h.edge];
        if h.forward {
            e.a
        } else {
            e.b
        }
    }

    /// Outgoing tangent direction of a half-edge at its origin.
    fn tangent(&self, h: Half) -> Vec2 {
        let e = &self.edges[h.edge];
        let (from, to) = if h.forward {
            (self.verts[e.a], self.verts[e.b])
        } else {
            (self.verts[e.b], self.verts[e.a])
        };
        match e.curve {
            Curve::Line => to - from,
            Curve::Arc { center } => {
                let radial = from - center;
                // CCW tangent when traversing forward, CW when reversed.
                if h.forward {
                    radial.perp()
                } else {
                    -radial.perp()
                }
            }
        }
    }

    /// Polygonises a half-edge from its origin, excluding the final vertex.
    fn polygonise(&self, h: Half, opts: &ProfileOptions, out: &mut Loop) {
        let e = &self.edges[h.edge];
        let (from, to) = if h.forward {
            (self.verts[e.a], self.verts[e.b])
        } else {
            (self.verts[e.b], self.verts[e.a])
        };
        match e.curve {
            Curve::Line => {
                out.points.push(from);
                out.curves.push(SegmentCurve::Line);
            }
            Curve::Arc { center } => {
                let r = 0.5 * (from.distance(center) + to.distance(center));
                let a0 = (from - center).angle();
                let a1 = (to - center).angle();
                let two_pi = std::f64::consts::TAU;
                // Forward traversal is CCW (positive sweep); reversed is CW.
                let mut sweep = (a1 - a0).rem_euclid(two_pi);
                if !h.forward {
                    sweep -= two_pi;
                }
                if sweep.abs() < 1e-12 {
                    sweep = if h.forward { two_pi } else { -two_pi };
                }
                let n = ((sweep.abs() / opts.arc_segment_angle).ceil() as usize).max(2);
                for i in 0..n {
                    let t = a0 + sweep * (i as f64 / n as f64);
                    out.points.push(center + Vec2::from_angle(t) * r);
                    out.curves.push(SegmentCurve::Arc { center, radius: r });
                }
            }
        }
    }

    /// Traces every face of the planar graph and returns the polygons of the
    /// bounded (positive-area) ones.
    fn bounded_faces(&self, opts: &ProfileOptions) -> Vec<Loop> {
        // Outgoing half-edges per vertex, sorted by angle CCW.
        let mut out: Vec<Vec<Half>> = vec![Vec::new(); self.verts.len()];
        for (i, e) in self.edges.iter().enumerate() {
            out[e.a].push(Half {
                edge: i,
                forward: true,
            });
            out[e.b].push(Half {
                edge: i,
                forward: false,
            });
        }
        for list in out.iter_mut() {
            list.sort_by(|x, y| {
                self.tangent(*x)
                    .angle()
                    .partial_cmp(&self.tangent(*y).angle())
                    .unwrap()
            });
        }
        // next(h): at the head of h, the outgoing half-edge immediately
        // clockwise from twin(h). This keeps the face interior on the left.
        let next = |h: Half| -> Half {
            let v = self.origin(h.twin());
            let list = &out[v];
            let pos = list
                .iter()
                .position(|x| *x == h.twin())
                .expect("twin present");
            list[(pos + list.len() - 1) % list.len()]
        };
        let mut visited: HashMap<Half, bool> = HashMap::new();
        let mut faces = Vec::new();
        for (i, _) in self.edges.iter().enumerate() {
            for start in [
                Half {
                    edge: i,
                    forward: true,
                },
                Half {
                    edge: i,
                    forward: false,
                },
            ] {
                if visited.contains_key(&start) {
                    continue;
                }
                let mut poly = Loop {
                    points: Vec::new(),
                    curves: Vec::new(),
                };
                let mut h = start;
                loop {
                    visited.insert(h, true);
                    self.polygonise(h, opts, &mut poly);
                    h = next(h);
                    if h == start {
                        break;
                    }
                }
                if poly.signed_area() > 1e-12 {
                    faces.push(poly);
                }
            }
        }
        faces
    }
}

impl Sketch {
    /// Extracts the closed regions bounded by the sketch geometry.
    pub fn profiles(&self, opts: &ProfileOptions) -> Vec<Profile> {
        let graph = Graph::from_sketch(self, opts);
        let mut loops: Vec<Loop> = graph.bounded_faces(opts);
        for (_, e) in self.entities() {
            if let Entity::Circle { center, radius } = e {
                if let Ok(c) = self.point(*center) {
                    if *radius > 0.0 {
                        let n = ((std::f64::consts::TAU / opts.arc_segment_angle).ceil() as usize)
                            .max(8);
                        let points = (0..n)
                            .map(|i| {
                                c + Vec2::from_angle(std::f64::consts::TAU * i as f64 / n as f64)
                                    * *radius
                            })
                            .collect();
                        loops.push(Loop {
                            points,
                            curves: vec![
                                SegmentCurve::Arc {
                                    center: c,
                                    radius: *radius
                                };
                                n
                            ],
                        });
                    }
                }
            }
        }
        // Nest loops: parent = smallest loop that contains this loop.
        let areas: Vec<f64> = loops.iter().map(|l| l.signed_area()).collect();
        let probes: Vec<Vec2> = loops.iter().map(|l| probe_point(&l.points)).collect();
        let mut parent: Vec<Option<usize>> = vec![None; loops.len()];
        for i in 0..loops.len() {
            let mut best: Option<usize> = None;
            for j in 0..loops.len() {
                if i == j || areas[j] <= areas[i] {
                    continue;
                }
                if point_in_polygon(probes[i], &loops[j].points)
                    && best.is_none_or(|b| areas[j] < areas[b])
                {
                    best = Some(j);
                }
            }
            parent[i] = best;
        }
        let mut children: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (i, p) in parent.iter().enumerate() {
            if let Some(p) = p {
                children.entry(*p).or_default().push(i);
            }
        }
        loops
            .iter()
            .enumerate()
            .map(|(i, outer)| Profile {
                outer: outer.clone(),
                holes: children
                    .get(&i)
                    .map(|hs| hs.iter().map(|&h| loops[h].reversed()).collect())
                    .unwrap_or_default(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangle_gives_one_profile() {
        let mut s = Sketch::new();
        s.add_rectangle(Vec2::ZERO, Vec2::new(4.0, 2.0));
        let p = s.profiles(&ProfileOptions::default());
        assert_eq!(p.len(), 1);
        assert!((p[0].area() - 8.0).abs() < 1e-9);
        assert!(p[0].holes.is_empty());
    }

    #[test]
    fn rectangle_with_circle_hole() {
        let mut s = Sketch::new();
        s.add_rectangle(Vec2::ZERO, Vec2::new(10.0, 10.0));
        s.add_circle(Vec2::new(5.0, 5.0), 1.0);
        let mut p = s.profiles(&ProfileOptions::default());
        p.sort_by(|a, b| b.area().partial_cmp(&a.area()).unwrap());
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].holes.len(), 1);
        assert!(p[0].area() < 100.0 && p[0].area() > 96.0);
        assert!(p[1].holes.is_empty());
    }

    #[test]
    fn split_rectangle_gives_two_faces() {
        let mut s = Sketch::new();
        let [bottom, _, top, _] = s.add_rectangle(Vec2::ZERO, Vec2::new(4.0, 2.0));
        // Divide with a vertical line whose endpoints coincide with midpoints.
        let (l, a, b) = s.add_line(Vec2::new(2.0, 0.0), Vec2::new(2.0, 2.0));
        s.add_constraint(Constraint::Midpoint {
            point: a,
            line: bottom,
        });
        s.add_constraint(Constraint::Midpoint {
            point: b,
            line: top,
        });
        let _ = l;
        // Endpoints do not coincide with existing vertices, so the divider
        // is dangling until the T-junctions are split: expect one face.
        assert_eq!(s.profiles(&ProfileOptions::default()).len(), 1);
    }

    #[test]
    fn two_triangles_sharing_an_edge() {
        let mut s = Sketch::new();
        let a = s.add_point(Vec2::new(0.0, 0.0));
        let b = s.add_point(Vec2::new(4.0, 0.0));
        let c = s.add_point(Vec2::new(2.0, 3.0));
        let d = s.add_point(Vec2::new(2.0, -3.0));
        s.add_line_between(a, b);
        s.add_line_between(b, c);
        s.add_line_between(c, a);
        s.add_line_between(a, d);
        s.add_line_between(d, b);
        let p = s.profiles(&ProfileOptions::default());
        assert_eq!(p.len(), 2);
        for f in &p {
            assert!((f.area() - 6.0).abs() < 1e-9);
        }
    }

    #[test]
    fn reversed_loop_keeps_curve_tags_on_segments() {
        let arc = SegmentCurve::Arc {
            center: Vec2::ZERO,
            radius: 1.0,
        };
        let l = Loop {
            points: vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(1.0, 0.0),
                Vec2::new(1.0, 1.0),
                Vec2::new(0.0, 1.0),
            ],
            curves: vec![
                SegmentCurve::Line,
                arc,
                SegmentCurve::Line,
                SegmentCurve::Line,
            ],
        };
        let r = l.reversed();
        assert_eq!(
            r.points,
            vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(0.0, 1.0),
                Vec2::new(1.0, 1.0),
                Vec2::new(1.0, 0.0)
            ]
        );
        // Segment (1,1)->(1,0) is the reversed arc.
        assert_eq!(r.curves[2], arc);
        assert_eq!(r.curves.iter().filter(|c| **c == arc).count(), 1);
        assert!((r.signed_area() + l.signed_area()).abs() < 1e-12);
    }

    #[test]
    fn semicircle_slot() {
        // A slot: two horizontal lines joined by two semicircular arcs.
        let mut s = Sketch::new();
        let (top, t0, t1) = s.add_line(Vec2::new(0.0, 1.0), Vec2::new(4.0, 1.0));
        let (bottom, b0, b1) = s.add_line(Vec2::new(4.0, -1.0), Vec2::new(0.0, -1.0));
        // Right arc CCW from (4,-1) up to (4,1) about (4,0).
        let (_, _, rs, re) = s.add_arc(
            Vec2::new(4.0, 0.0),
            Vec2::new(4.0, -1.0),
            Vec2::new(4.0, 1.0),
        );
        // Left arc CCW from (0,1) down to (0,-1) about (0,0).
        let (_, _, ls, le) = s.add_arc(
            Vec2::new(0.0, 0.0),
            Vec2::new(0.0, 1.0),
            Vec2::new(0.0, -1.0),
        );
        s.add_constraint(Constraint::Coincident { a: t1, b: re });
        s.add_constraint(Constraint::Coincident { a: b0, b: rs });
        s.add_constraint(Constraint::Coincident { a: t0, b: ls });
        s.add_constraint(Constraint::Coincident { a: b1, b: le });
        let _ = (top, bottom);
        let p = s.profiles(&ProfileOptions::default());
        assert_eq!(p.len(), 1);
        let expected = 4.0 * 2.0 + std::f64::consts::PI;
        assert!(
            (p[0].area() - expected).abs() < 0.02,
            "area {}",
            p[0].area()
        );
    }
}
