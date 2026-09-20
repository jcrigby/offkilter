//! STEP (ISO 10303-21, AP214) export of solids.
//!
//! Every face of a kernel solid is a planar polygon, so a solid goes out as
//! a `MANIFOLD_SOLID_BREP` whose `ADVANCED_FACE`s lie on `PLANE`s and whose
//! edges are `LINE`s between `VERTEX_POINT`s: a faceted but exact B-rep
//! that other CAD systems read as a solid (facets on curved surfaces stay
//! facets). Bodies keep their names; units are millimetres.
//!
//! Pure text generation: no I/O, builds for wasm32.

use ok_brep::Solid;
use ok_math::Vec3;
use std::collections::HashMap;
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

    /// One `MANIFOLD_SOLID_BREP` for `solid`; returns its entity number.
    fn brep(&mut self, name: &str, solid: &Solid) -> usize {
        // Vertices, shared by every edge that meets there.
        let vertex_points: Vec<usize> = solid
            .vertices
            .iter()
            .map(|&p| {
                let point = self.point(p);
                self.entity(&format!("VERTEX_POINT('',#{point})"))
            })
            .collect();
        // Edges, one per undirected pair, oriented from the lower index.
        let mut edges: HashMap<(u32, u32), usize> = HashMap::new();
        let mut edge_of = |w: &mut Writer, a: u32, b: u32| -> (usize, bool) {
            let key = ok_brep::edge_key(a, b);
            let forward = a == key.0;
            let id = *edges.entry(key).or_insert_with(|| {
                let (pa, pb) = (
                    solid.vertices[key.0 as usize],
                    solid.vertices[key.1 as usize],
                );
                let start = w.point(pa);
                let dir = w.direction((pb - pa).normalized().unwrap_or(Vec3::new(1.0, 0.0, 0.0)));
                let vector = w.entity(&format!("VECTOR('',#{dir},{})", num((pb - pa).length())));
                let line = w.entity(&format!("LINE('',#{start},#{vector})"));
                w.entity(&format!(
                    "EDGE_CURVE('',#{},#{},#{line},.T.)",
                    vertex_points[key.0 as usize], vertex_points[key.1 as usize]
                ))
            });
            (id, forward)
        };
        let mut faces: Vec<usize> = Vec::with_capacity(solid.faces.len());
        for f in &solid.faces {
            let mut bounds: Vec<usize> = Vec::with_capacity(f.loops.len());
            for (li, l) in f.loops.iter().enumerate() {
                let mut oriented: Vec<usize> = Vec::with_capacity(l.len());
                for i in 0..l.len() {
                    let (a, b) = (l[i], l[(i + 1) % l.len()]);
                    let (edge, forward) = edge_of(self, a, b);
                    oriented.push(self.entity(&format!(
                        "ORIENTED_EDGE('',*,*,#{edge},{})",
                        if forward { ".T." } else { ".F." }
                    )));
                }
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
            let origin = self.point(f.plane.origin);
            let normal = self.direction(f.plane.normal);
            let x_axis = self.direction(f.plane.x_axis);
            let axis = self.entity(&format!(
                "AXIS2_PLACEMENT_3D('',#{origin},#{normal},#{x_axis})"
            ));
            let plane = self.entity(&format!("PLANE('',#{axis})"));
            faces.push(self.entity(&format!(
                "ADVANCED_FACE('',({}),#{plane},.T.)",
                refs(&bounds)
            )));
        }
        let shell = self.entity(&format!("CLOSED_SHELL('',({}))", refs(&faces)));
        self.entity(&format!("MANIFOLD_SOLID_BREP('{name}',#{shell})"))
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
