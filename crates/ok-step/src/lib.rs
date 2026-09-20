//! STEP (ISO 10303-21, AP214) export of solids.
//!
//! A solid goes out as a `MANIFOLD_SOLID_BREP`. Planar faces lie on
//! `PLANE`s with `LINE` edges; the facets of a cylinder, cone, torus or
//! sphere go out as one `ADVANCED_FACE` per connected region on a
//! `CYLINDRICAL_SURFACE`, `CONICAL_SURFACE`, `TOROIDAL_SURFACE` or
//! `SPHERICAL_SURFACE`, its bounds the exact curves recovered from the
//! surface pairs (`ok_brep::exact`): circles, ellipses, lines, and fine
//! polyline B-splines where two cylinders meet, with a seam along a
//! meridian where a region closes round the axis and every vertex at its
//! exact position. Facets on other surfaces (revolved splines, ruled)
//! stay facets. Bodies keep their names; units are millimetres.
//!
//! Pure text generation: no I/O, builds for wasm32.

use ok_brep::{exact, Solid, Surface};
use ok_math::Vec3;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

/// Writes `solids` (name, solid) as one STEP file, as a string.
mod read;

pub use read::{read_step, StepBody};

pub fn write_step(solids: &[(&str, &Solid)], document: &str) -> String {
    let mut w = Writer::default();
    // Units and context, shared by every representation.
    let length = w.entity("( LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.) )");
    let angle = w.entity("( NAMED_UNIT(*) PLANE_ANGLE_UNIT() SI_UNIT($,.RADIAN.) )");
    let solid_angle = w.entity("( NAMED_UNIT(*) SI_UNIT($,.STERADIAN.) SOLID_ANGLE_UNIT() )");
    let uncertainty = w.entity(&format!(
        "UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-06),#{length},'distance_accuracy_value','confusion accuracy')"
    ));
    let context = w.entity(&format!(
        "( GEOMETRIC_REPRESENTATION_CONTEXT(3) GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#{uncertainty})) GLOBAL_UNIT_ASSIGNED_CONTEXT((#{length},#{angle},#{solid_angle})) REPRESENTATION_CONTEXT('Context #1','3D Context with UNIT and UNCERTAINTY') )"
    ));
    let app = w.entity("APPLICATION_CONTEXT('automotive design')");
    w.entity(&format!(
        "APPLICATION_PROTOCOL_DEFINITION('international standard','automotive_design',2010,#{app})"
    ));
    let product_context = w.entity(&format!("PRODUCT_CONTEXT('',#{app},'mechanical')"));
    let definition_context = w.entity(&format!(
        "PRODUCT_DEFINITION_CONTEXT('part definition',#{app},'design')"
    ));
    let origin = w.entity("CARTESIAN_POINT('',(0.,0.,0.))");
    let z = w.entity("DIRECTION('',(0.,0.,1.))");
    let x = w.entity("DIRECTION('',(1.,0.,0.))");
    let placement = w.entity(&format!("AXIS2_PLACEMENT_3D('',#{origin},#{z},#{x})"));

    for (index, (name, solid)) in solids.iter().enumerate() {
        let name = escape(name);
        let brep = w.brep(&name, solid);
        let shape = w.entity(&format!(
            "ADVANCED_BREP_SHAPE_REPRESENTATION('{name}',(#{brep},#{placement}),#{context})"
        ));
        let id = format!("{}-{}", escape(document), index + 1);
        let product = w.entity(&format!("PRODUCT('{id}','{name}','',(#{product_context}))"));
        w.entity(&format!(
            "PRODUCT_RELATED_PRODUCT_CATEGORY('part',$,(#{product}))"
        ));
        let formation = w.entity(&format!("PRODUCT_DEFINITION_FORMATION('','',#{product})"));
        let definition = w.entity(&format!(
            "PRODUCT_DEFINITION('design','',#{formation},#{definition_context})"
        ));
        let definition_shape = w.entity(&format!("PRODUCT_DEFINITION_SHAPE('','',#{definition})"));
        w.entity(&format!(
            "SHAPE_DEFINITION_REPRESENTATION(#{definition_shape},#{shape})"
        ));
    }

    let mut out = String::new();
    out.push_str("ISO-10303-21;\nHEADER;\n");
    let _ = writeln!(out, "FILE_DESCRIPTION(('{}'),'2;1');", escape(document));
    let _ = writeln!(
        out,
        "FILE_NAME('{}.step','',(''),(''),'offkilter','offkilter','');",
        escape(document)
    );
    out.push_str("FILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }'));\nENDSEC;\nDATA;\n");
    out.push_str(&w.data);
    out.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    out
}

#[derive(Default)]
struct Writer {
    data: String,
    next: usize,
}

impl Writer {
    /// Appends an entity and returns its number.
    fn entity(&mut self, text: &str) -> usize {
        self.next += 1;
        let _ = writeln!(self.data, "#{}={text};", self.next);
        self.next
    }

    fn point(&mut self, p: Vec3) -> usize {
        self.entity(&format!(
            "CARTESIAN_POINT('',({},{},{}))",
            num(p.x),
            num(p.y),
            num(p.z)
        ))
    }

    fn direction(&mut self, d: Vec3) -> usize {
        self.entity(&format!(
            "DIRECTION('',({},{},{}))",
            num(d.x),
            num(d.y),
            num(d.z)
        ))
    }

    fn placement(&mut self, origin: Vec3, axis: Vec3, x: Vec3) -> usize {
        let o = self.point(origin);
        let a = self.direction(axis);
        let r = self.direction(x);
        self.entity(&format!("AXIS2_PLACEMENT_3D('',#{o},#{a},#{r})"))
    }

    /// One `MANIFOLD_SOLID_BREP` for `solid`; returns its entity number.
    ///
    /// Faces on planes, cylinders, cones, tori and spheres go out
    /// exactly: every vertex at its position on all of its surfaces,
    /// every run of facet edges between two surfaces as one edge on the
    /// pair's curve (a line, circle, ellipse, or a fine polyline B-spline
    /// for two cylinders), and each connected region of a curved surface
    /// as one face on its surface entity, with a seam along a meridian
    /// where the region closes round the axis. Facets on other surfaces
    /// (revolved splines, ruled) and surfaces the seam cannot be placed
    /// on stay facets.
    fn brep(&mut self, name: &str, solid: &Solid) -> usize {
        // Planar facets in one plane (a revolve's flats) are one face.
        let merged = {
            let mut m = solid.clone();
            m.merge_coplanar_faces();
            m
        };
        let solid = &merged;
        let vf = exact::vertex_faces(solid);
        let positions: Vec<Vec3> = (0..solid.vertices.len() as u32)
            .map(|v| exact::vertex_position(solid, &vf, v))
            .collect();
        let mut runs = exact::edge_runs(solid);
        let regions = exact::surface_regions(solid);
        let edge_faces: HashMap<(u32, u32), Vec<usize>> = solid.edge_faces().into_iter().collect();

        // Curved surfaces written exactly: those whose regions can be
        // bounded, each with the frame its surface entity is placed in.
        let mut exact_curved: HashMap<usize, (Frame, Vec<Region>)> = HashMap::new();
        // A closed run can start at one vertex only, so two surfaces
        // sharing a rim must seam it at the same vertex: later surfaces
        // prefer the seam vertices earlier ones chose.
        let mut preferred: HashSet<u32> = HashSet::new();
        for (surface, s) in solid.surfaces.iter().enumerate() {
            if !exact::is_curved(s) {
                continue;
            }
            if let Some(found) = revolved_regions(
                solid,
                surface,
                &regions,
                &edge_faces,
                &positions,
                &preferred,
            ) {
                for region in &found.1 {
                    if let Some((top, bottom)) = region.seam {
                        preferred.insert(top);
                        preferred.insert(bottom);
                    }
                }
                exact_curved.insert(surface, found);
            }
        }
        // A closed run used by a seam must start at the seam vertex, so the
        // seamed loop chains: rotate such runs to begin there.
        for (_, regs) in exact_curved.values() {
            for region in regs {
                let Some((top, bottom)) = region.seam else {
                    continue;
                };
                for v in [top, bottom] {
                    for r in runs.iter_mut() {
                        if !r.closed || r.vertices[0] == v {
                            continue;
                        }
                        if let Some(k) = r.vertices.iter().position(|&x| x == v) {
                            let n = r.vertices.len() - 1; // the last repeats the first
                            let mut rotated: Vec<u32> = r.vertices[k..n].to_vec();
                            rotated.extend_from_slice(&r.vertices[..k]);
                            rotated.push(v);
                            r.vertices = rotated;
                        }
                    }
                }
            }
        }
        let exact_surface = |sf: usize| match solid.surfaces[sf] {
            Surface::Plane { .. } => true,
            _ => exact_curved.contains_key(&sf),
        };
        let curve_of: Vec<exact::Curve> = runs
            .iter()
            .map(|r| {
                if exact_surface(r.surfaces.0) && exact_surface(r.surfaces.1) {
                    exact::run_curve(solid, &vf, r)
                } else {
                    exact::Curve::Polyline
                }
            })
            .collect();
        // Which run each facet edge lies on, and where along it.
        let mut on_run: HashMap<(u32, u32), (usize, usize)> = HashMap::new();
        for (i, r) in runs.iter().enumerate() {
            for (k, w) in r.vertices.windows(2).enumerate() {
                on_run.insert(ok_brep::edge_key(w[0], w[1]), (i, k));
            }
        }

        let mut vertex_points: HashMap<u32, usize> = HashMap::new();
        let mut run_edges: HashMap<usize, usize> = HashMap::new();
        let mut segment_edges: HashMap<(u32, u32), usize> = HashMap::new();
        let mut ctx = Edges {
            solid,
            positions: &positions,
            runs: &runs,
            curves: &curve_of,
            on_run: &on_run,
            vertex_points: &mut vertex_points,
            run_edges: &mut run_edges,
            segment_edges: &mut segment_edges,
        };

        let mut faces: Vec<usize> = Vec::new();
        // Planar faces, and facets of surfaces not written exactly.
        for f in &solid.faces {
            if exact_curved.contains_key(&f.surface) {
                continue;
            }
            let mut bounds = Vec::with_capacity(f.loops.len());
            for (li, l) in f.loops.iter().enumerate() {
                let oriented = ctx.loop_edges(self, l);
                let edge_loop = self.entity(&format!("EDGE_LOOP('',({}))", refs(&oriented)));
                bounds.push(self.entity(&format!(
                    "{}('',#{edge_loop},.T.)",
                    if li == 0 {
                        "FACE_OUTER_BOUND"
                    } else {
                        "FACE_BOUND"
                    }
                )));
            }
            let axis = self.placement(f.plane.origin, f.plane.normal, f.plane.x_axis);
            let plane = self.entity(&format!("PLANE('',#{axis})"));
            faces.push(self.entity(&format!(
                "ADVANCED_FACE('',({}),#{plane},.T.)",
                refs(&bounds)
            )));
        }
        // Exact curved surfaces: one face per connected region.
        let mut curved: Vec<(&usize, &(Frame, Vec<Region>))> = exact_curved.iter().collect();
        curved.sort_by_key(|(s, _)| **s);
        for (&surface, (frame, regs)) in curved {
            let placement = self.placement(frame.origin, frame.axis, frame.refdir);
            let entity = match solid.surfaces[surface] {
                Surface::Cylinder { radius, .. } => {
                    format!("CYLINDRICAL_SURFACE('',#{placement},{})", num(radius))
                }
                // Placed at the apex, where the radius is nought.
                Surface::Cone { half_angle, .. } => {
                    format!("CONICAL_SURFACE('',#{placement},0.0,{})", num(half_angle))
                }
                Surface::Torus { major, minor, .. } => {
                    format!(
                        "TOROIDAL_SURFACE('',#{placement},{},{})",
                        num(major),
                        num(minor)
                    )
                }
                Surface::Sphere { radius, .. } => {
                    format!("SPHERICAL_SURFACE('',#{placement},{})", num(radius))
                }
                _ => continue,
            };
            let surf = self.entity(&entity);
            for region in regs {
                let mut bounds = Vec::new();
                for (li, l) in region.loops.iter().enumerate() {
                    let oriented = match &region.seam {
                        Some(seam) if li == 0 => {
                            ctx.seamed_loop(self, l, &region.loops[1], seam, surface)
                        }
                        // The second wrapping loop is part of the seamed outer loop.
                        Some(_) if li == 1 => continue,
                        _ => ctx.loop_edges(self, l),
                    };
                    let edge_loop = self.entity(&format!("EDGE_LOOP('',({}))", refs(&oriented)));
                    bounds.push(self.entity(&format!(
                        "{}('',#{edge_loop},.T.)",
                        if li == 0 {
                            "FACE_OUTER_BOUND"
                        } else {
                            "FACE_BOUND"
                        }
                    )));
                }
                faces.push(self.entity(&format!(
                    "ADVANCED_FACE('',({}),#{surf},.T.)",
                    refs(&bounds)
                )));
            }
        }
        let shell = self.entity(&format!("CLOSED_SHELL('',({}))", refs(&faces)));
        self.entity(&format!("MANIFOLD_SOLID_BREP('{name}',#{shell})"))
    }
}

/// A connected region of a curved surface, ready to write: its loops
/// (vertices with the neighbouring surface across each edge), and, when
/// the region closes round the axis, the seam joining loops 0 and 1
/// along one meridian (the two vertices, top on loop 0 and bottom on
/// loop 1).
struct Region {
    loops: Vec<Vec<u32>>,
    seam: Option<(u32, u32)>,
}

/// The placement of a surface entity: its axis (the surface's own for a
/// surface of revolution; for a sphere one across the patch, with the
/// seam meridian on the far side) and reference direction.
struct Frame {
    origin: Vec3,
    axis: Vec3,
    refdir: Vec3,
}

/// Groups a curved surface's boundary loops into connected regions and
/// finds a seam for each region that wraps round the axis; `None` when
/// the surface cannot be written exactly (a region wraps but no meridian
/// is clear of its holes, or it wraps with other than two loops).
fn revolved_regions(
    solid: &Solid,
    surface: usize,
    regions: &HashMap<usize, Vec<exact::RegionLoop>>,
    edge_faces: &HashMap<(u32, u32), Vec<usize>>,
    positions: &[Vec3],
    preferred: &HashSet<u32>,
) -> Option<(Frame, Vec<Region>)> {
    let loops = regions.get(&surface)?;
    if loops.is_empty() {
        return None;
    }
    let frame = match solid.surfaces[surface] {
        Surface::Sphere { center, .. } => {
            // Across the patch: the loops then never sweep round the axis,
            // and the seam meridian (through the reference direction)
            // passes the patch's antipode.
            let mut m = Vec3::ZERO;
            for l in loops {
                for &(v, _) in l {
                    m += positions[v as usize] - center;
                }
            }
            let m = m.normalized().unwrap_or(Vec3::Z);
            Frame {
                origin: center,
                axis: exact::perpendicular(m),
                refdir: -m,
            }
        }
        ref s => {
            let (origin, axis) = exact::axis_of(s)?;
            Frame {
                origin,
                axis,
                refdir: exact::cylinder_frame(axis).0,
            }
        }
    };
    let (origin, axis) = (frame.origin, frame.axis);
    let loops = regions.get(&surface)?;
    if loops.is_empty() {
        return None;
    }
    // Connected components of the facets through their seams.
    let facets: Vec<usize> = (0..solid.faces.len())
        .filter(|&i| solid.faces[i].surface == surface)
        .collect();
    let mut parent: HashMap<usize, usize> = facets.iter().map(|&f| (f, f)).collect();
    fn find(p: &mut HashMap<usize, usize>, i: usize) -> usize {
        let mut r = i;
        while p[&r] != r {
            r = p[&r];
        }
        r
    }
    for fs in edge_faces.values() {
        if fs.len() == 2 && parent.contains_key(&fs[0]) && parent.contains_key(&fs[1]) {
            let (a, b) = (find(&mut parent, fs[0]), find(&mut parent, fs[1]));
            if a != b {
                parent.insert(a, b);
            }
        }
    }
    // Which facet each loop's first edge belongs to.
    let loop_component = |l: &exact::RegionLoop| -> Option<usize> {
        let (a, _) = l[0];
        let (b, _) = l[1 % l.len()];
        let fs = edge_faces.get(&ok_brep::edge_key(a, b))?;
        let f = fs
            .iter()
            .copied()
            .find(|f| solid.faces[*f].surface == surface)?;
        Some(find(&mut parent.clone(), f))
    };
    let mut by_component: HashMap<usize, Vec<&exact::RegionLoop>> = HashMap::new();
    for l in loops {
        by_component.entry(loop_component(l)?).or_default().push(l);
    }
    let sweep = |l: &exact::RegionLoop| -> f64 {
        let mut total = 0.0;
        let n = l.len();
        for i in 0..n {
            let a = exact::cylinder_angle(origin, axis, positions[l[i].0 as usize]);
            let b = exact::cylinder_angle(origin, axis, positions[l[(i + 1) % n].0 as usize]);
            let mut d = b - a;
            while d > std::f64::consts::PI {
                d -= std::f64::consts::TAU;
            }
            while d < -std::f64::consts::PI {
                d += std::f64::consts::TAU;
            }
            total += d;
        }
        total
    };
    let mut out = Vec::new();
    let mut components: Vec<(usize, Vec<&exact::RegionLoop>)> = by_component.into_iter().collect();
    components.sort_by_key(|(c, _)| *c);
    for (_, comp) in components {
        let mut wrapping: Vec<&exact::RegionLoop> = Vec::new();
        let mut holes: Vec<&exact::RegionLoop> = Vec::new();
        for l in comp {
            if (sweep(l).abs() - std::f64::consts::TAU).abs() < 1e-6 {
                wrapping.push(l);
            } else {
                holes.push(l);
            }
        }
        match wrapping.len() {
            0 => {
                let mut loops: Vec<&exact::RegionLoop> = holes;
                // The outer loop is the one with the largest extent.
                let extent = |l: &exact::RegionLoop| -> f64 {
                    let pts = l.iter().map(|e| positions[e.0 as usize]);
                    let (lo, hi) = pts.fold(
                        (
                            Vec3::new(f64::MAX, f64::MAX, f64::MAX),
                            Vec3::new(f64::MIN, f64::MIN, f64::MIN),
                        ),
                        |(lo, hi), p| {
                            (
                                Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z)),
                                Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z)),
                            )
                        },
                    );
                    (hi - lo).length()
                };
                loops.sort_by(|a, b| extent(b).total_cmp(&extent(a)));
                out.push(Region {
                    loops: loops
                        .iter()
                        .map(|l| l.iter().map(|e| e.0).collect())
                        .collect(),
                    seam: None,
                });
            }
            2 => {
                // A ruling with a vertex on both wrapping loops and clear of
                // the holes' angle ranges.
                let angle = |v: u32| exact::cylinder_angle(origin, axis, positions[v as usize]);
                let (first, second) = (wrapping[0], wrapping[1]);
                let mut seam: Option<(u32, u32)> = None;
                // Candidates chosen before, on either rim, come first.
                let mut candidates: Vec<u32> = first.iter().map(|e| e.0).collect();
                candidates.sort_by_key(|&v| {
                    let t = angle(v);
                    let partner_preferred = second
                        .iter()
                        .any(|(w, _)| preferred.contains(w) && (angle(*w) - t).abs() < 1e-9);
                    !(preferred.contains(&v) || partner_preferred)
                });
                'candidates: for v in candidates {
                    let t = angle(v);
                    let Some(&(w, _)) = second.iter().find(|(w, _)| (angle(*w) - t).abs() < 1e-9)
                    else {
                        continue;
                    };
                    for h in &holes {
                        let mut prev = angle(h[0].0);
                        let mut lo = prev;
                        let mut hi = prev;
                        for &(hv, _) in h.iter().skip(1) {
                            let mut a = angle(hv);
                            while a - prev > std::f64::consts::PI {
                                a -= std::f64::consts::TAU;
                            }
                            while prev - a > std::f64::consts::PI {
                                a += std::f64::consts::TAU;
                            }
                            lo = lo.min(a);
                            hi = hi.max(a);
                            prev = a;
                        }
                        for k in -1..=1 {
                            let tt = t + k as f64 * std::f64::consts::TAU;
                            if tt >= lo - 1e-9 && tt <= hi + 1e-9 {
                                continue 'candidates;
                            }
                        }
                    }
                    seam = Some((v, w));
                    break;
                }
                let seam = seam?;
                let mut loops: Vec<Vec<u32>> = vec![
                    first.iter().map(|e| e.0).collect(),
                    second.iter().map(|e| e.0).collect(),
                ];
                loops.extend(
                    holes
                        .iter()
                        .map(|l| l.iter().map(|e| e.0).collect::<Vec<u32>>()),
                );
                out.push(Region {
                    loops,
                    seam: Some(seam),
                });
            }
            _ => return None,
        }
    }
    Some((frame, out))
}

/// The edge entities of one solid, made on demand.
struct Edges<'a> {
    solid: &'a Solid,
    positions: &'a [Vec3],
    runs: &'a [exact::Run],
    curves: &'a [exact::Curve],
    on_run: &'a HashMap<(u32, u32), (usize, usize)>,
    vertex_points: &'a mut HashMap<u32, usize>,
    run_edges: &'a mut HashMap<usize, usize>,
    segment_edges: &'a mut HashMap<(u32, u32), usize>,
}

impl Edges<'_> {
    fn vertex(&mut self, w: &mut Writer, v: u32) -> usize {
        if let Some(&id) = self.vertex_points.get(&v) {
            return id;
        }
        let p = w.point(self.positions[v as usize]);
        let id = w.entity(&format!("VERTEX_POINT('',#{p})"));
        self.vertex_points.insert(v, id);
        id
    }

    /// The edge from vertex `a` to `b` along a straight segment (made once
    /// per unordered pair, from the lower vertex); returns it and whether
    /// `a` is its start.
    fn segment(&mut self, w: &mut Writer, a: u32, b: u32) -> (usize, bool) {
        let key = ok_brep::edge_key(a, b);
        if let Some(&id) = self.segment_edges.get(&key) {
            return (id, a == key.0);
        }
        let (pa, pb) = (
            self.positions[key.0 as usize],
            self.positions[key.1 as usize],
        );
        let start = w.point(pa);
        let dir = w.direction((pb - pa).normalized().unwrap_or(Vec3::X));
        let vector = w.entity(&format!("VECTOR('',#{dir},{})", num((pb - pa).length())));
        let line = w.entity(&format!("LINE('',#{start},#{vector})"));
        let (va, vb) = (self.vertex(w, key.0), self.vertex(w, key.1));
        let id = w.entity(&format!("EDGE_CURVE('',#{va},#{vb},#{line},.T.)"));
        self.segment_edges.insert(key, id);
        (id, a == key.0)
    }

    /// The edge of a whole run on its exact curve, from the run's first
    /// vertex to its last, with the curve parametrised in that direction.
    fn run_edge(&mut self, w: &mut Writer, run: usize) -> usize {
        if let Some(&id) = self.run_edges.get(&run) {
            return id;
        }
        let r = &self.runs[run];
        let (first, last) = (r.vertices[0], *r.vertices.last().unwrap());
        let (p0, p1) = (
            self.positions[first as usize],
            self.positions[r.vertices[1] as usize],
        );
        let curve = match &self.curves[run] {
            exact::Curve::Line { .. } => {
                let pend = self.positions[last as usize];
                let start = w.point(p0);
                let dir = w.direction((pend - p0).normalized().unwrap_or(Vec3::X));
                let vector = w.entity(&format!("VECTOR('',#{dir},{})", num((pend - p0).length())));
                w.entity(&format!("LINE('',#{start},#{vector})"))
            }
            exact::Curve::Circle {
                center,
                axis,
                radius,
            } => {
                // Parametrised counter-clockwise about the axis, so the axis
                // is turned to make the run go that way; the reference
                // direction puts the first vertex at parameter zero.
                let refdir = (p0 - *center)
                    .normalized()
                    .unwrap_or_else(|| exact::perpendicular(*axis));
                let ccw = axis.dot((p0 - *center).cross(p1 - *center)) > 0.0;
                let axis = if ccw { *axis } else { -*axis };
                let placement = w.placement(*center, axis, refdir);
                w.entity(&format!("CIRCLE('',#{placement},{})", num(*radius)))
            }
            exact::Curve::Ellipse {
                center,
                axis,
                major,
                a,
                b,
            } => {
                let ccw = axis.dot((p0 - *center).cross(p1 - *center)) > 0.0;
                let axis = if ccw { *axis } else { -*axis };
                let placement = w.placement(*center, axis, *major);
                w.entity(&format!("ELLIPSE('',#{placement},{},{})", num(*a), num(*b)))
            }
            exact::Curve::Quartic => {
                // A fine polyline as a degree-one B-spline: the run's own
                // points plus the crossings of the first cylinder's rulings
                // every degree.
                let fine: Vec<f64> = (0..360)
                    .map(|k| (k as f64).to_radians() - std::f64::consts::PI)
                    .collect();
                let cyl = r.surfaces.0;
                let vf = exact::vertex_faces(self.solid);
                let pts = exact::run_points(self.solid, &vf, r, Some((cyl, &fine)));
                let ids: Vec<usize> = pts.iter().map(|p| w.point(*p)).collect();
                let n = ids.len();
                let knots: Vec<String> = (0..n).map(|k| num(k as f64)).collect();
                let mults: Vec<String> = (0..n)
                    .map(|k| {
                        if k == 0 || k == n - 1 {
                            "2".into()
                        } else {
                            "1".into()
                        }
                    })
                    .collect();
                w.entity(&format!(
                    "B_SPLINE_CURVE_WITH_KNOTS('',1,({}),.UNSPECIFIED.,.F.,.F.,({}),({}),.UNSPECIFIED.)",
                    refs(&ids),
                    mults.join(","),
                    knots.join(",")
                ))
            }
            exact::Curve::Polyline => unreachable!("polyline runs are written by segment"),
        };
        let (va, vb) = (self.vertex(w, first), self.vertex(w, last));
        let id = w.entity(&format!("EDGE_CURVE('',#{va},#{vb},#{curve},.T.)"));
        self.run_edges.insert(run, id);
        id
    }

    fn oriented(&mut self, w: &mut Writer, edge: usize, forward: bool) -> usize {
        w.entity(&format!(
            "ORIENTED_EDGE('',*,*,#{edge},{})",
            if forward { ".T." } else { ".F." }
        ))
    }

    /// The oriented edges of a loop of vertices: runs of edges on one
    /// exact curve become one edge each; the rest go segment by segment.
    fn loop_edges(&mut self, w: &mut Writer, l: &[u32]) -> Vec<usize> {
        let n = l.len();
        let run_of = |k: usize| -> Option<(usize, usize)> {
            let key = ok_brep::edge_key(l[k], l[(k + 1) % n]);
            let (run, pos) = *self.on_run.get(&key)?;
            matches!(self.curves[run], exact::Curve::Polyline)
                .then_some(())
                .map_or(Some((run, pos)), |_| None)
        };
        // Start at a segment whose run differs from the previous one's.
        let start = (0..n)
            .find(|&k| run_of(k).map(|r| r.0) != run_of((k + n - 1) % n).map(|r| r.0))
            .unwrap_or(0);
        let mut out = Vec::new();
        let mut k = 0;
        while k < n {
            let i = (start + k) % n;
            match run_of(i) {
                None => {
                    let (edge, forward) = self.segment(w, l[i], l[(i + 1) % n]);
                    out.push(self.oriented(w, edge, forward));
                    k += 1;
                }
                Some((run, pos)) => {
                    // How many consecutive segments of the loop lie on this run.
                    let mut len = 1;
                    while k + len < n && run_of((start + k + len) % n).map(|r| r.0) == Some(run) {
                        len += 1;
                    }
                    let forward = self.runs[run].vertices[pos] == l[i];
                    let edge = self.run_edge(w, run);
                    out.push(self.oriented(w, edge, forward));
                    k += len;
                }
            }
        }
        out
    }

    /// The outer loop of a region that wraps round the axis: loop `a`
    /// round from the seam's top vertex, the seam down, loop `b` round
    /// from its bottom vertex, the seam back up.
    fn seamed_loop(
        &mut self,
        w: &mut Writer,
        a: &[u32],
        b: &[u32],
        seam: &(u32, u32),
        surface: usize,
    ) -> Vec<usize> {
        let rotate = |l: &[u32], v: u32| -> Vec<u32> {
            let k = l.iter().position(|&x| x == v).unwrap_or(0);
            let mut out: Vec<u32> = l[k..].to_vec();
            out.extend_from_slice(&l[..k]);
            out
        };
        let (top, bottom) = *seam;
        let mut out = self.loop_edges(w, &rotate(a, top));
        let (edge, forward) = self.seam_edge(w, top, bottom, surface);
        out.push(self.oriented(w, edge, forward));
        out.extend(self.loop_edges(w, &rotate(b, bottom)));
        let (edge, forward) = self.seam_edge(w, bottom, top, surface);
        out.push(self.oriented(w, edge, forward));
        out
    }

    /// The seam from `a` to `b`, made once: a ruling of a cylinder or
    /// cone, the meridian arc of a torus through the facet vertices
    /// between the two.
    fn seam_edge(&mut self, w: &mut Writer, a: u32, b: u32, surface: usize) -> (usize, bool) {
        let Surface::Torus {
            origin,
            axis,
            major,
            minor,
        } = self.solid.surfaces[surface]
        else {
            return self.segment(w, a, b);
        };
        let key = ok_brep::edge_key(a, b);
        if let Some(&id) = self.segment_edges.get(&key) {
            return (id, a == key.0);
        }
        let (pa, pb) = (
            self.positions[key.0 as usize],
            self.positions[key.1 as usize],
        );
        // The tube circle at the seam's angle about the axis.
        let d = pa - origin;
        let radial = (d - axis * d.dot(axis))
            .normalized()
            .unwrap_or_else(|| exact::perpendicular(axis));
        let center = origin + radial * major;
        // A vertex of the region on the same meridian between the two
        // tells which way round the tube the seam runs; failing one, the
        // shorter way.
        let angle = |p: Vec3| exact::cylinder_angle(origin, axis, p);
        let t = angle(pa);
        let witness = self
            .solid
            .faces
            .iter()
            .filter(|f| f.surface == surface)
            .flat_map(|f| f.loops.iter().flatten().copied())
            .find(|&v| {
                v != key.0 && v != key.1 && {
                    let p = self.positions[v as usize];
                    (angle(p) - t).abs() < 1e-9 && p.distance(pa) > 1e-9 && p.distance(pb) > 1e-9
                }
            })
            .map(|v| self.positions[v as usize])
            .unwrap_or_else(|| {
                center
                    + ((pa - center) + (pb - center))
                        .normalized()
                        .unwrap_or(radial)
                        * minor
            });
        let normal = radial.cross(axis);
        let ccw = normal.dot((pa - center).cross(witness - center)) > 0.0;
        let normal = if ccw { normal } else { -normal };
        let refdir = (pa - center)
            .normalized()
            .unwrap_or_else(|| exact::perpendicular(normal));
        let placement = w.placement(center, normal, refdir);
        let circle = w.entity(&format!("CIRCLE('',#{placement},{})", num(minor)));
        let (va, vb) = (self.vertex(w, key.0), self.vertex(w, key.1));
        let id = w.entity(&format!("EDGE_CURVE('',#{va},#{vb},#{circle},.T.)"));
        self.segment_edges.insert(key, id);
        (id, a == key.0)
    }
}

fn refs(ids: &[usize]) -> String {
    ids.iter()
        .map(|id| format!("#{id}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// A real number in Part 21 form: always with a decimal point, no exponent
/// unless needed, and `-0.` folded to `0.`.
fn num(v: f64) -> String {
    if v == 0.0 {
        return "0.0".into();
    }
    if v.abs() >= 1e-4 && v.abs() < 1e12 {
        let mut s = format!("{v:.9}");
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.push('0');
        }
        s
    } else {
        let s = format!("{v:E}");
        if s.contains('.') {
            s
        } else {
            s.replacen('E', ".E", 1)
        }
    }
}

/// Part 21 strings: apostrophes double, non-ASCII becomes a `\X2\` escape.
fn escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '\'' => out.push_str("''"),
            '\\' => out.push_str("\\\\"),
            c if c.is_ascii() && !c.is_ascii_control() => out.push(c),
            c => {
                let _ = write!(out, "\\X2\\{:04X}\\X0\\", c as u32);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ok_brep::extrude;
    use ok_math::{Plane, Vec2};
    use ok_sketch::{ProfileOptions, Sketch};

    fn count(text: &str, entity: &str) -> usize {
        text.lines()
            .filter(|l| l.contains(&format!("={entity}(")) || l.contains(&format!("={entity}'")))
            .count()
    }

    #[test]
    fn a_box_is_a_closed_shell_of_six_planar_faces() {
        let mut sk = Sketch::new();
        sk.add_rectangle(Vec2::new(0.0, 0.0), Vec2::new(10.0, 20.0));
        let profile = sk
            .profiles(&ProfileOptions::default())
            .into_iter()
            .next()
            .unwrap();
        let plane = Plane {
            origin: Vec3::ZERO,
            x_axis: Vec3::new(1.0, 0.0, 0.0),
            y_axis: Vec3::new(0.0, 1.0, 0.0),
            normal: Vec3::new(0.0, 0.0, 1.0),
        };
        let solid = extrude(&profile, &plane, 0.0, 5.0, 1).unwrap();
        let text = write_step(&[("Part 1", &solid)], "doc");
        assert!(text.starts_with("ISO-10303-21;\nHEADER;\n"));
        assert!(text.ends_with("ENDSEC;\nEND-ISO-10303-21;\n"));
        assert!(text.contains("FILE_SCHEMA(('AUTOMOTIVE_DESIGN"));
        assert_eq!(count(&text, "VERTEX_POINT"), 8);
        assert_eq!(count(&text, "EDGE_CURVE"), 12);
        assert_eq!(count(&text, "ORIENTED_EDGE"), 24);
        assert_eq!(count(&text, "ADVANCED_FACE"), 6);
        assert_eq!(count(&text, "PLANE"), 6);
        assert_eq!(count(&text, "CLOSED_SHELL"), 1);
        assert_eq!(count(&text, "MANIFOLD_SOLID_BREP"), 1);
        assert!(text.contains("MANIFOLD_SOLID_BREP('Part 1',#"));
        // Every edge is used once forwards and once backwards.
        let forwards = text.matches(",.T.);").count() - count(&text, "EDGE_CURVE") - 6 - 6;
        let backwards = text.matches(",.F.);").count();
        assert_eq!((forwards, backwards), (12, 12));
        // Entity numbers are dense and every reference resolves.
        let last: usize = text.lines().filter(|l| l.starts_with('#')).count();
        for token in text.split(|c: char| !(c.is_ascii_digit() || c == '#')) {
            if let Some(n) = token.strip_prefix('#') {
                if let Ok(n) = n.parse::<usize>() {
                    assert!(n >= 1 && n <= last, "dangling reference #{n}");
                }
            }
        }
    }

    #[test]
    fn numbers_and_strings_follow_part_21() {
        assert_eq!(num(0.0), "0.0");
        assert_eq!(num(-0.0), "0.0");
        assert_eq!(num(1.0), "1.0");
        assert_eq!(num(12.5), "12.5");
        assert_eq!(num(-0.125), "-0.125");
        assert_eq!(num(1e-7), "1.E-7");
        assert_eq!(escape("it's"), "it''s");
        assert_eq!(escape("é"), "\\X2\\00E9\\X0\\");
    }
}
