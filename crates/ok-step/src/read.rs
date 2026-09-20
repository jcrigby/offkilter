//! A STEP (ISO 10303-21) reader for faceted import: the solids of a file
//! come back as triangle meshes for `add_mesh`.
//!
//! The reader walks `MANIFOLD_SOLID_BREP` → `CLOSED_SHELL` →
//! `ADVANCED_FACE` → bounds → `EDGE_LOOP` → `ORIENTED_EDGE` →
//! `EDGE_CURVE` → `VERTEX_POINT` → `CARTESIAN_POINT`. Edges on lines are
//! kept straight; edges on circles and B-splines are sampled, once per
//! edge so the faces on both sides share the same points and the mesh
//! welds closed. Faces on planes are triangulated in the plane; faces on
//! cylinders in their (angle, height) parameters, with the angle
//! unwrapped along each loop, in facet-wide strips whose columns are
//! shared with the edges first so both sides of an edge agree. Other
//! surfaces are refused by name rather than tessellated wrongly. Lengths
//! are scaled to millimetres from the file's length unit.

use ok_brep::clip2d::{clip_and_close, Contour, Rect};
use ok_math::Vec3;
use std::collections::{HashMap, HashSet};

/// One solid of a STEP file as a mesh, in millimetres.
#[derive(Debug, Clone, PartialEq)]
pub struct StepBody {
    pub name: String,
    pub vertices: Vec<Vec3>,
    pub triangles: Vec<[u32; 3]>,
}

#[derive(Debug, Clone, PartialEq)]
enum Val {
    Str(String),
    Num(f64),
    Ref(usize),
    Enum(String),
    List(Vec<Val>),
    Typed(String, Vec<Val>),
    Null,
}

impl Val {
    fn num(&self) -> Option<f64> {
        match self {
            Val::Num(n) => Some(*n),
            Val::Typed(_, args) => args.first().and_then(Val::num),
            _ => None,
        }
    }

    fn reference(&self) -> Option<usize> {
        match self {
            Val::Ref(r) => Some(*r),
            _ => None,
        }
    }

    fn list(&self) -> &[Val] {
        match self {
            Val::List(v) => v,
            _ => &[],
        }
    }

    fn flag(&self) -> Option<bool> {
        match self {
            Val::Enum(e) if e == "T" => Some(true),
            Val::Enum(e) if e == "F" => Some(false),
            _ => None,
        }
    }

    fn text(&self) -> &str {
        match self {
            Val::Str(s) => s,
            _ => "",
        }
    }
}

/// One part of an entity instance: its type and arguments. A complex
/// instance has several.
type Part = (String, Vec<Val>);

struct File {
    entities: HashMap<usize, Vec<Part>>,
}

impl File {
    fn parts(&self, id: usize) -> Result<&[Part], String> {
        self.entities
            .get(&id)
            .map(|v| v.as_slice())
            .ok_or_else(|| format!("#{id} is not in the file"))
    }

    /// The arguments of the part of `id` whose type is `kind` (or ends
    /// with it, for a complex instance).
    fn part(&self, id: usize, kind: &str) -> Option<&[Val]> {
        self.parts(id)
            .ok()?
            .iter()
            .find(|(k, _)| k == kind)
            .map(|(_, a)| a.as_slice())
    }

    fn kind(&self, id: usize) -> Result<&str, String> {
        let parts = self.parts(id)?;
        Ok(parts.last().map(|(k, _)| k.as_str()).unwrap_or(""))
    }

    fn kinds(&self, id: usize) -> Vec<&str> {
        self.parts(id)
            .map(|p| p.iter().map(|(k, _)| k.as_str()).collect())
            .unwrap_or_default()
    }

    fn point(&self, id: usize) -> Result<Vec3, String> {
        let args = self
            .part(id, "CARTESIAN_POINT")
            .ok_or_else(|| format!("#{id} is not a CARTESIAN_POINT"))?;
        let c = args.get(1).map(Val::list).unwrap_or(&[]);
        let get = |i: usize| c.get(i).and_then(Val::num).unwrap_or(0.0);
        Ok(Vec3::new(get(0), get(1), get(2)))
    }

    fn direction(&self, id: usize) -> Result<Vec3, String> {
        let args = self
            .part(id, "DIRECTION")
            .ok_or_else(|| format!("#{id} is not a DIRECTION"))?;
        let c = args.get(1).map(Val::list).unwrap_or(&[]);
        let get = |i: usize| c.get(i).and_then(Val::num).unwrap_or(0.0);
        Vec3::new(get(0), get(1), get(2))
            .normalized()
            .ok_or_else(|| format!("#{id} is a zero direction"))
    }

    /// Origin, axis (normal) and reference direction of an
    /// `AXIS2_PLACEMENT_3D`; missing directions default to Z and X.
    fn placement(&self, id: usize) -> Result<(Vec3, Vec3, Vec3), String> {
        let args = self
            .part(id, "AXIS2_PLACEMENT_3D")
            .ok_or_else(|| format!("#{id} is not an AXIS2_PLACEMENT_3D"))?;
        let origin = args
            .get(1)
            .and_then(Val::reference)
            .map(|r| self.point(r))
            .transpose()?
            .unwrap_or(Vec3::ZERO);
        let axis = args
            .get(2)
            .and_then(Val::reference)
            .map(|r| self.direction(r))
            .transpose()?
            .unwrap_or(Vec3::Z);
        let hint = args
            .get(3)
            .and_then(Val::reference)
            .map(|r| self.direction(r))
            .transpose()?
            .unwrap_or(if axis.cross(Vec3::X).length() > 1e-6 {
                Vec3::X
            } else {
                Vec3::Y
            });
        let x = (hint - axis * hint.dot(axis))
            .normalized()
            .unwrap_or_else(|| axis.cross(Vec3::Z).normalized().unwrap_or(Vec3::X));
        Ok((origin, axis, x))
    }
}

// ---------------------------------------------------------------- parsing

struct Cursor<'a> {
    s: &'a [u8],
    at: usize,
}

impl Cursor<'_> {
    fn skip_space(&mut self) {
        loop {
            while self.at < self.s.len() && self.s[self.at].is_ascii_whitespace() {
                self.at += 1;
            }
            if self.s[self.at..].starts_with(b"/*") {
                match self.s[self.at + 2..].windows(2).position(|w| w == b"*/") {
                    Some(p) => self.at += 2 + p + 2,
                    None => self.at = self.s.len(),
                }
                continue;
            }
            break;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.at).copied()
    }

    fn expect(&mut self, c: u8) -> Result<(), String> {
        self.skip_space();
        if self.peek() == Some(c) {
            self.at += 1;
            Ok(())
        } else {
            Err(format!(
                "expected '{}' at byte {} of the STEP file",
                c as char, self.at
            ))
        }
    }

    fn ident(&mut self) -> String {
        self.skip_space();
        let start = self.at;
        while self.at < self.s.len()
            && (self.s[self.at].is_ascii_alphanumeric() || self.s[self.at] == b'_')
        {
            self.at += 1;
        }
        String::from_utf8_lossy(&self.s[start..self.at]).to_string()
    }

    fn value(&mut self) -> Result<Val, String> {
        self.skip_space();
        let Some(c) = self.peek() else {
            return Err("unexpected end of STEP file".into());
        };
        match c {
            b'\'' => {
                self.at += 1;
                let mut out = String::new();
                loop {
                    let Some(ch) = self.peek() else {
                        return Err("unterminated string in STEP file".into());
                    };
                    self.at += 1;
                    if ch == b'\'' {
                        if self.peek() == Some(b'\'') {
                            self.at += 1;
                            out.push('\'');
                        } else {
                            break;
                        }
                    } else {
                        out.push(ch as char);
                    }
                }
                Ok(Val::Str(out))
            }
            b'#' => {
                self.at += 1;
                let start = self.at;
                while self.at < self.s.len() && self.s[self.at].is_ascii_digit() {
                    self.at += 1;
                }
                let id: usize = std::str::from_utf8(&self.s[start..self.at])
                    .ok()
                    .and_then(|t| t.parse().ok())
                    .ok_or("bad entity reference")?;
                Ok(Val::Ref(id))
            }
            b'.' => {
                self.at += 1;
                let start = self.at;
                while self.at < self.s.len() && self.s[self.at] != b'.' {
                    self.at += 1;
                }
                let e = String::from_utf8_lossy(&self.s[start..self.at]).to_string();
                self.at += 1;
                Ok(Val::Enum(e))
            }
            b'(' => {
                self.at += 1;
                let mut items = Vec::new();
                loop {
                    self.skip_space();
                    if self.peek() == Some(b')') {
                        self.at += 1;
                        break;
                    }
                    items.push(self.value()?);
                    self.skip_space();
                    if self.peek() == Some(b',') {
                        self.at += 1;
                    }
                }
                Ok(Val::List(items))
            }
            b'*' | b'$' => {
                self.at += 1;
                Ok(Val::Null)
            }
            b'-' | b'+' | b'0'..=b'9' => {
                let start = self.at;
                self.at += 1;
                while self.at < self.s.len()
                    && matches!(
                        self.s[self.at],
                        b'0'..=b'9' | b'.' | b'E' | b'e' | b'-' | b'+'
                    )
                {
                    self.at += 1;
                }
                let t = String::from_utf8_lossy(&self.s[start..self.at]);
                t.parse::<f64>()
                    .map(Val::Num)
                    .map_err(|_| format!("bad number {t:?} in STEP file"))
            }
            _ => {
                let kind = self.ident();
                if kind.is_empty() {
                    return Err(format!("unexpected '{}' in STEP file", c as char));
                }
                self.expect(b'(')?;
                let args = self.args()?;
                Ok(Val::Typed(kind, args))
            }
        }
    }

    /// Arguments up to and including the closing parenthesis.
    fn args(&mut self) -> Result<Vec<Val>, String> {
        let mut args = Vec::new();
        loop {
            self.skip_space();
            if self.peek() == Some(b')') {
                self.at += 1;
                return Ok(args);
            }
            args.push(self.value()?);
            self.skip_space();
            if self.peek() == Some(b',') {
                self.at += 1;
            }
        }
    }
}

fn parse(text: &str) -> Result<File, String> {
    let bytes = text.as_bytes();
    let data = text
        .find("DATA;")
        .ok_or("not a STEP file: no DATA section")?;
    let mut cur = Cursor {
        s: bytes,
        at: data + 5,
    };
    let mut entities = HashMap::new();
    loop {
        cur.skip_space();
        if cur.at >= bytes.len() || bytes[cur.at..].starts_with(b"ENDSEC") {
            break;
        }
        cur.expect(b'#')?;
        let start = cur.at;
        while cur.at < bytes.len() && bytes[cur.at].is_ascii_digit() {
            cur.at += 1;
        }
        let id: usize = std::str::from_utf8(&bytes[start..cur.at])
            .ok()
            .and_then(|t| t.parse().ok())
            .ok_or("bad entity number")?;
        cur.expect(b'=')?;
        cur.skip_space();
        let mut parts: Vec<Part> = Vec::new();
        if cur.peek() == Some(b'(') {
            // A complex instance: several typed parts in parentheses.
            cur.at += 1;
            loop {
                cur.skip_space();
                if cur.peek() == Some(b')') {
                    cur.at += 1;
                    break;
                }
                let kind = cur.ident();
                cur.expect(b'(')?;
                let args = cur.args()?;
                parts.push((kind, args));
            }
        } else {
            let kind = cur.ident();
            cur.expect(b'(')?;
            let args = cur.args()?;
            parts.push((kind, args));
        }
        cur.expect(b';')?;
        entities.insert(id, parts);
    }
    Ok(File { entities })
}

/// Millimetres per length unit of the file.
fn length_scale(file: &File) -> f64 {
    for parts in file.entities.values() {
        let is_length = parts.iter().any(|(k, _)| k == "LENGTH_UNIT");
        if !is_length {
            continue;
        }
        if let Some((_, args)) = parts.iter().find(|(k, _)| k == "SI_UNIT") {
            let prefix = args.first().map(|v| match v {
                Val::Enum(e) => e.as_str(),
                _ => "",
            });
            return match prefix {
                Some("MILLI") => 1.0,
                Some("CENTI") => 10.0,
                Some("DECI") => 100.0,
                Some("MICRO") => 1e-3,
                Some("KILO") => 1e6,
                _ => 1000.0,
            };
        }
        if let Some((_, args)) = parts.iter().find(|(k, _)| k == "CONVERSION_BASED_UNIT") {
            let name = args.first().map(Val::text).unwrap_or("").to_uppercase();
            return match name.as_str() {
                "INCH" => 25.4,
                "FOOT" => 304.8,
                "YARD" => 914.4,
                _ => 1.0,
            };
        }
    }
    1.0
}

// ---------------------------------------------------------------- geometry

/// Angle per sample along a curved edge.
const STEP_ANGLE: f64 = 5.0_f64.to_radians();

struct Reader<'a> {
    file: &'a File,
    scale: f64,
    vertices: Vec<Vec3>,
    /// Mesh vertex of every `VERTEX_POINT`.
    vertex_of: HashMap<usize, u32>,
    /// Sampled interior points of every edge, from its start vertex to
    /// its end vertex, as mesh vertices.
    edge_samples: HashMap<usize, Vec<u32>>,
    /// For every face on a surface of revolution, the grid it is cut
    /// into: columns in the angle about the axis and rows along the
    /// profile (in the face's unwrapped frame, mirrored when the face
    /// turns inward).
    columns: HashMap<usize, Grid>,
    /// Vertices that stay where they are: edge ends and the column
    /// crossings cylindrical faces put on their edges.
    fixed: HashSet<u32>,
}

#[derive(Clone, Copy)]
struct Grid {
    umin: f64,
    strips: usize,
    width: f64,
    vmin: f64,
    rows: usize,
    height: f64,
    outward: bool,
}

/// A face bound: whether it is the outer one, its orientation flag, and
/// its edges with their orientation flags.
type Bound = (bool, bool, Vec<(usize, bool)>);

/// A surface of revolution: its placement (centre on the axis, axis,
/// reference direction and the third axis) and profile, in millimetres.
/// Points are parametrised by the angle about the axis from `x` and a
/// profile parameter: height along the axis for a cylinder or cone, the
/// tube angle for a torus, the latitude for a sphere.
struct RevolvedGeom {
    centre: Vec3,
    axis: Vec3,
    x: Vec3,
    y: Vec3,
    kind: Profile,
}

enum Profile {
    Cylinder {
        radius: f64,
    },
    /// Radius at the placement's origin, growing along the axis.
    Cone {
        radius: f64,
        half_angle: f64,
    },
    Torus {
        major: f64,
        minor: f64,
    },
    Sphere {
        radius: f64,
    },
}

impl RevolvedGeom {
    fn radial(&self, t: f64) -> Vec3 {
        self.x * t.cos() + self.y * t.sin()
    }

    /// Length of a radian of angle at the surface's widest, so the angle
    /// coordinate is a length (facet-wide columns then have a width).
    fn u_scale(&self) -> f64 {
        match self.kind {
            Profile::Cylinder { radius } | Profile::Sphere { radius } => radius,
            // The widest the loops reach is not known here; the reference
            // radius plus the run of the half angle over a typical height
            // is near enough to keep columns fine.
            Profile::Cone { radius, half_angle } => radius.max(1.0) * (1.0 + half_angle.tan()),
            Profile::Torus { major, minor } => major + minor,
        }
    }

    /// Length of a radian of the profile parameter (one for heights).
    fn v_scale(&self) -> f64 {
        match self.kind {
            Profile::Torus { minor, .. } => minor,
            Profile::Sphere { radius } => radius,
            _ => 1.0,
        }
    }

    /// Whether the profile curves, so rows are needed as well as columns.
    fn curved_profile(&self) -> bool {
        matches!(self.kind, Profile::Torus { .. } | Profile::Sphere { .. })
    }

    /// The (angle, profile parameter) of a point on or near the surface.
    fn param(&self, p: Vec3) -> (f64, f64) {
        let d = p - self.centre;
        let h = d.dot(self.axis);
        let t = d.dot(self.y).atan2(d.dot(self.x));
        match self.kind {
            Profile::Cylinder { .. } | Profile::Cone { .. } => (t, h),
            Profile::Torus { major, .. } => {
                let r = (d - self.axis * h).length() - major;
                (t, h.atan2(r))
            }
            Profile::Sphere { .. } => {
                let r = (d - self.axis * h).length();
                (t, h.atan2(r))
            }
        }
    }

    /// The point at an angle and profile parameter.
    fn point(&self, t: f64, v: f64) -> Vec3 {
        match self.kind {
            Profile::Cylinder { radius } => self.centre + self.radial(t) * radius + self.axis * v,
            Profile::Cone { radius, half_angle } => {
                self.centre + self.radial(t) * (radius + v * half_angle.tan()) + self.axis * v
            }
            Profile::Torus { major, minor } => {
                self.centre
                    + self.radial(t) * (major + minor * v.cos())
                    + self.axis * (minor * v.sin())
            }
            Profile::Sphere { radius } => {
                self.centre + (self.radial(t) * v.cos() + self.axis * v.sin()) * radius
            }
        }
    }
}

impl Reader<'_> {
    fn vertex(&mut self, id: usize) -> Result<u32, String> {
        if let Some(&v) = self.vertex_of.get(&id) {
            return Ok(v);
        }
        let args = self
            .file
            .part(id, "VERTEX_POINT")
            .ok_or_else(|| format!("#{id} is not a VERTEX_POINT"))?;
        let p = self.file.point(
            args.get(1)
                .and_then(Val::reference)
                .ok_or("VERTEX_POINT without a point")?,
        )? * self.scale;
        let v = self.vertices.len() as u32;
        self.vertices.push(p);
        self.vertex_of.insert(id, v);
        self.fixed.insert(v);
        Ok(v)
    }

    fn push(&mut self, p: Vec3) -> u32 {
        self.vertices.push(p);
        self.vertices.len() as u32 - 1
    }

    /// The mesh vertices along an edge from its start to its end,
    /// including both ends.
    fn edge(&mut self, id: usize) -> Result<Vec<u32>, String> {
        let args = self
            .file
            .part(id, "EDGE_CURVE")
            .ok_or_else(|| format!("#{id} is not an EDGE_CURVE"))?
            .to_vec();
        let start = self.vertex(
            args.get(1)
                .and_then(Val::reference)
                .ok_or("edge without start")?,
        )?;
        let end = self.vertex(
            args.get(2)
                .and_then(Val::reference)
                .ok_or("edge without end")?,
        )?;
        if !self.edge_samples.contains_key(&id) {
            let curve = args.get(3).and_then(Val::reference);
            let same_sense = args.get(4).and_then(Val::flag).unwrap_or(true);
            let interior = match curve {
                Some(c) => self.sample_curve(c, start, end, same_sense)?,
                None => Vec::new(),
            };
            self.edge_samples.insert(id, interior);
        }
        let mut out = vec![start];
        out.extend(self.edge_samples[&id].iter().copied());
        out.push(end);
        Ok(out)
    }

    /// Interior sample points of a curve between two mesh vertices.
    fn sample_curve(
        &mut self,
        curve: usize,
        start: u32,
        end: u32,
        same_sense: bool,
    ) -> Result<Vec<u32>, String> {
        let kinds = self.file.kinds(curve);
        let (a, b) = (self.vertices[start as usize], self.vertices[end as usize]);
        if kinds.contains(&"LINE") {
            return Ok(Vec::new());
        }
        if kinds.contains(&"CIRCLE") {
            let args = self.file.part(curve, "CIRCLE").unwrap();
            let (centre, axis, x) = self.file.placement(
                args.get(1)
                    .and_then(Val::reference)
                    .ok_or("CIRCLE without a placement")?,
            )?;
            let centre = centre * self.scale;
            let radius = args
                .get(2)
                .and_then(Val::num)
                .ok_or("CIRCLE without a radius")?
                * self.scale;
            let y = axis.cross(x);
            let angle = |p: Vec3| (p - centre).dot(y).atan2((p - centre).dot(x));
            let (a0, a1) = (angle(a), angle(b));
            // Sweep counter-clockwise about the axis when the edge runs
            // with the curve, clockwise otherwise; a closed edge sweeps
            // the whole circle.
            let mut sweep = if same_sense {
                (a1 - a0).rem_euclid(std::f64::consts::TAU)
            } else {
                -((a0 - a1).rem_euclid(std::f64::consts::TAU))
            };
            if sweep.abs() < 1e-9 || start == end {
                sweep = if same_sense {
                    std::f64::consts::TAU
                } else {
                    -std::f64::consts::TAU
                };
            }
            let n = ((sweep.abs() / STEP_ANGLE).ceil() as usize).max(1);
            let mut out = Vec::with_capacity(n);
            for i in 1..n {
                let t = a0 + sweep * i as f64 / n as f64;
                let p = centre + (x * t.cos() + y * t.sin()) * radius;
                out.push(self.push(p));
            }
            return Ok(out);
        }
        if kinds.contains(&"ELLIPSE") {
            let args = self.file.part(curve, "ELLIPSE").unwrap();
            let (centre, axis, major) = self.file.placement(
                args.get(1)
                    .and_then(Val::reference)
                    .ok_or("ELLIPSE without a placement")?,
            )?;
            let centre = centre * self.scale;
            let ra = args
                .get(2)
                .and_then(Val::num)
                .ok_or("ELLIPSE without semi-axes")?
                * self.scale;
            let rb = args
                .get(3)
                .and_then(Val::num)
                .ok_or("ELLIPSE without semi-axes")?
                * self.scale;
            let minor = axis.cross(major);
            let param =
                |p: Vec3| ((p - centre).dot(minor) / rb).atan2((p - centre).dot(major) / ra);
            let (a0, a1) = (param(a), param(b));
            let mut sweep = if same_sense {
                (a1 - a0).rem_euclid(std::f64::consts::TAU)
            } else {
                -((a0 - a1).rem_euclid(std::f64::consts::TAU))
            };
            if sweep.abs() < 1e-9 || start == end {
                sweep = if same_sense {
                    std::f64::consts::TAU
                } else {
                    -std::f64::consts::TAU
                };
            }
            let n = ((sweep.abs() / STEP_ANGLE).ceil() as usize).max(1);
            let mut out = Vec::with_capacity(n);
            for i in 1..n {
                let t = a0 + sweep * i as f64 / n as f64;
                let p = centre + major * (ra * t.cos()) + minor * (rb * t.sin());
                out.push(self.push(p));
            }
            return Ok(out);
        }
        if kinds.iter().any(|k| k.starts_with("B_SPLINE_CURVE")) {
            let pts = self.bspline_points(curve)?;
            // The edge runs the whole curve; the samples start at the
            // curve's start, so reverse them when the edge runs the other way.
            let mut interior: Vec<Vec3> = pts[1..pts.len() - 1].to_vec();
            let forward = pts[0].distance(a) + pts[pts.len() - 1].distance(b)
                <= pts[0].distance(b) + pts[pts.len() - 1].distance(a);
            if !forward {
                interior.reverse();
            }
            return Ok(interior.into_iter().map(|p| self.push(p)).collect());
        }
        Err(format!(
            "edges on a {} are not supported (lines, circles and B-splines are)",
            kinds.last().copied().unwrap_or("unknown curve")
        ))
    }

    /// Points along a B-spline curve, evaluated by de Boor at 16 samples
    /// per span, from its first to its last knot.
    fn bspline_points(&self, curve: usize) -> Result<Vec<Vec3>, String> {
        let file = self.file;
        let base = file
            .part(curve, "B_SPLINE_CURVE")
            .or_else(|| file.part(curve, "B_SPLINE_CURVE_WITH_KNOTS"))
            .ok_or("B-spline without control points")?;
        // In a simple instance the arguments are name, degree, points,
        // form, closed, self-intersect, multiplicities, knots, spec; in a
        // complex one B_SPLINE_CURVE holds the first six and
        // B_SPLINE_CURVE_WITH_KNOTS the last three.
        let degree = base
            .get(1)
            .and_then(Val::num)
            .ok_or("B-spline without a degree")? as usize;
        let control: Vec<Vec3> = base
            .get(2)
            .map(Val::list)
            .unwrap_or(&[])
            .iter()
            .filter_map(Val::reference)
            .map(|r| file.point(r).map(|p| p * self.scale))
            .collect::<Result<_, _>>()?;
        let knots_part = file
            .part(curve, "B_SPLINE_CURVE_WITH_KNOTS")
            .ok_or("B-spline without knots")?;
        let (mults, knots) = if knots_part.len() >= 9 {
            (knots_part[6].list(), knots_part[7].list())
        } else {
            (knots_part[0].list(), knots_part[1].list())
        };
        let weights: Vec<f64> = file
            .part(curve, "RATIONAL_B_SPLINE_CURVE")
            .and_then(|a| a.first())
            .map(|w| w.list().iter().filter_map(Val::num).collect())
            .unwrap_or_else(|| vec![1.0; control.len()]);
        let mut knot_vector: Vec<f64> = Vec::new();
        for (m, k) in mults.iter().zip(knots) {
            let (m, k) = (m.num().unwrap_or(1.0) as usize, k.num().unwrap_or(0.0));
            knot_vector.extend(std::iter::repeat_n(k, m));
        }
        let n = control.len();
        if n < degree + 1 || knot_vector.len() != n + degree + 1 {
            return Err("B-spline knots do not match its control points".into());
        }
        let (t0, t1) = (knot_vector[degree], knot_vector[n]);
        let spans = knots.len().max(2) - 1;
        // A degree-1 curve is a polyline: its poles are its samples, and
        // points between them would be collinear, which the triangulation
        // must not see (a sliver triangle on both sides of a shared edge
        // makes it look non-manifold).
        let mut out = Vec::new();
        if degree == 1 {
            for k in knots.iter().filter_map(|k| k.num()) {
                if k >= t0 && k <= t1 {
                    out.push(de_boor(&control, &weights, &knot_vector, degree, k));
                }
            }
            return Ok(out);
        }
        let samples = spans * 16;
        for i in 0..=samples {
            let t = t0 + (t1 - t0) * i as f64 / samples as f64;
            out.push(de_boor(&control, &weights, &knot_vector, degree, t));
        }
        Ok(out)
    }

    /// The bounds of a face: whether each is the outer one, its
    /// orientation flag, and its edges with their orientation flags.
    fn face_bounds(&self, face: usize) -> Result<Vec<Bound>, String> {
        let args = self
            .file
            .part(face, "ADVANCED_FACE")
            .or_else(|| self.file.part(face, "FACE_SURFACE"))
            .ok_or_else(|| format!("#{face} is not an ADVANCED_FACE"))?
            .to_vec();
        let mut out = Vec::new();
        for bound in args.get(1).map(Val::list).unwrap_or(&[]) {
            let Some(bid) = bound.reference() else {
                continue;
            };
            let kind = self.file.kind(bid)?.to_string();
            let bargs = self.file.parts(bid)?.last().unwrap().1.clone();
            let loop_id = bargs
                .get(1)
                .and_then(Val::reference)
                .ok_or("face bound without a loop")?;
            let orientation = bargs.get(2).and_then(Val::flag).unwrap_or(true);
            let largs = self
                .file
                .part(loop_id, "EDGE_LOOP")
                .ok_or_else(|| format!("#{loop_id} is not an EDGE_LOOP"))?
                .to_vec();
            let mut edges = Vec::new();
            for oe in largs.get(1).map(Val::list).unwrap_or(&[]) {
                let Some(oid) = oe.reference() else { continue };
                let oargs = self
                    .file
                    .part(oid, "ORIENTED_EDGE")
                    .ok_or_else(|| format!("#{oid} is not an ORIENTED_EDGE"))?
                    .to_vec();
                let edge = oargs
                    .get(3)
                    .and_then(Val::reference)
                    .ok_or("oriented edge without an edge")?;
                let forward = oargs.get(4).and_then(Val::flag).unwrap_or(true);
                edges.push((edge, forward));
            }
            out.push((kind == "FACE_OUTER_BOUND", orientation, edges));
        }
        Ok(out)
    }

    /// The loops of a face as mesh vertices, oriented as the bounds say,
    /// outer loop first.
    fn face_loops(&mut self, face: usize) -> Result<Vec<Vec<u32>>, String> {
        let bounds = self.face_bounds(face)?;
        let mut loops: Vec<(bool, Vec<u32>)> = Vec::new();
        for (outer, orientation, edges) in bounds {
            let mut poly: Vec<u32> = Vec::new();
            for (edge, forward) in edges {
                let mut pts = self.edge(edge)?;
                if !forward {
                    pts.reverse();
                }
                // The last point is the next edge's first.
                pts.pop();
                poly.extend(pts);
            }
            if !orientation {
                poly.reverse();
            }
            poly.dedup();
            if poly.len() >= 3 {
                loops.push((outer, poly));
            }
        }
        if loops.is_empty() {
            return Err(format!("face #{face} has no bounds"));
        }
        // Outer loop first: the one marked, else the largest.
        if let Some(k) = loops.iter().position(|l| l.0) {
            loops.swap(0, k);
        } else {
            let area = |l: &[u32]| -> f64 {
                let pts: Vec<Vec3> = l.iter().map(|&v| self.vertices[v as usize]).collect();
                newell(&pts).length()
            };
            let mut best = 0;
            for k in 1..loops.len() {
                if area(&loops[k].1) > area(&loops[best].1) {
                    best = k;
                }
            }
            loops.swap(0, best);
        }
        Ok(loops.into_iter().map(|l| l.1).collect())
    }

    /// The surface of revolution a face lies on, if it is one.
    fn revolved_of(&self, face: usize) -> Result<Option<RevolvedGeom>, String> {
        let args = self
            .file
            .part(face, "ADVANCED_FACE")
            .or_else(|| self.file.part(face, "FACE_SURFACE"))
            .ok_or_else(|| format!("#{face} is not an ADVANCED_FACE"))?;
        let surface = args
            .get(2)
            .and_then(Val::reference)
            .ok_or("face without a surface")?;
        let kinds = [
            "CYLINDRICAL_SURFACE",
            "CONICAL_SURFACE",
            "TOROIDAL_SURFACE",
            "SPHERICAL_SURFACE",
        ];
        let Some((kind, sargs)) = kinds
            .iter()
            .find_map(|k| self.file.part(surface, k).map(|a| (*k, a)))
        else {
            return Ok(None);
        };
        let (centre, axis, x) = self.file.placement(
            sargs
                .get(1)
                .and_then(Val::reference)
                .ok_or_else(|| format!("{kind} without a placement"))?,
        )?;
        let number = |i: usize| -> Result<f64, String> {
            sargs
                .get(i)
                .and_then(Val::num)
                .ok_or_else(|| format!("{kind} without its measures"))
        };
        let profile = match kind {
            "CYLINDRICAL_SURFACE" => Profile::Cylinder {
                radius: number(2)? * self.scale,
            },
            "CONICAL_SURFACE" => Profile::Cone {
                radius: number(2)? * self.scale,
                half_angle: number(3)?,
            },
            "TOROIDAL_SURFACE" => Profile::Torus {
                major: number(2)? * self.scale,
                minor: number(3)? * self.scale,
            },
            _ => Profile::Sphere {
                radius: number(2)? * self.scale,
            },
        };
        Ok(Some(RevolvedGeom {
            centre: centre * self.scale,
            axis,
            x,
            y: axis.cross(x),
            kind: profile,
        }))
    }

    /// The loops of a face on a surface of revolution in its (angle ×
    /// scale, profile parameter × scale) coordinates, the angle unwrapped
    /// along each loop so a loop around the seam stays continuous (and the
    /// tube angle of a torus likewise) and holes shifted into the outer
    /// loop's turn.
    fn revolved_uvs(&self, loops: &[Vec<u32>], geom: &RevolvedGeom) -> Vec<Vec<[f64; 2]>> {
        let (us, vs) = (geom.u_scale(), geom.v_scale());
        let periodic_v = matches!(geom.kind, Profile::Torus { .. });
        let unwrap = |t: f64, prev: f64| {
            let mut t = t;
            while t - prev > std::f64::consts::PI {
                t -= std::f64::consts::TAU;
            }
            while prev - t > std::f64::consts::PI {
                t += std::f64::consts::TAU;
            }
            t
        };
        let mut uvs: Vec<Vec<[f64; 2]>> = Vec::new();
        for l in loops {
            let (mut pu, mut pv) = (0.0, 0.0);
            let mut uv = Vec::with_capacity(l.len());
            for (k, &v) in l.iter().enumerate() {
                let (mut t, mut h) = geom.param(self.vertices[v as usize]);
                if k > 0 {
                    t = unwrap(t, pu);
                    if periodic_v {
                        h = unwrap(h, pv);
                    }
                }
                (pu, pv) = (t, h);
                uv.push([t * us, h * vs]);
            }
            uvs.push(uv);
        }
        let shift_into = |uvs: &mut Vec<Vec<[f64; 2]>>, c: usize, period: f64| {
            let outer_mid = {
                let (lo, hi) = uvs[0].iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
                    (lo.min(p[c]), hi.max(p[c]))
                });
                (lo + hi) / 2.0
            };
            for uv in uvs.iter_mut().skip(1) {
                let mid = uv.iter().map(|p| p[c]).sum::<f64>() / uv.len() as f64;
                let shift = ((outer_mid - mid) / period).round() * period;
                for p in uv.iter_mut() {
                    p[c] += shift;
                }
            }
        };
        shift_into(&mut uvs, 0, std::f64::consts::TAU * us);
        if periodic_v {
            shift_into(&mut uvs, 1, std::f64::consts::TAU * vs);
        }
        uvs
    }

    /// First pass over a face on a surface of revolution: decides the grid
    /// it will be cut into (facet-wide columns in the angle about the
    /// axis, and rows along the profile where it curves) and inserts the
    /// grid lines' crossings into every edge of the face, so the faces on
    /// the other side of those edges use the same points and the mesh
    /// welds closed. A crossing between two samples is placed on the
    /// surface at the interpolated other coordinate, which both faces
    /// then share.
    fn prepare_revolved(&mut self, face: usize) -> Result<(), String> {
        let Some(geom) = self.revolved_of(face)? else {
            return Ok(());
        };
        let loops = self.face_loops(face)?;
        let uvs = self.revolved_uvs(&loops, &geom);
        let (us, vs) = (geom.u_scale(), geom.v_scale());
        let range = |c: usize| {
            uvs[0].iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
                (lo.min(p[c]), hi.max(p[c]))
            })
        };
        let cells = |lo: f64, hi: f64, step: f64| -> (usize, f64) {
            let raw = (hi - lo) / step;
            let n = if (raw - raw.round()).abs() < 1e-6 {
                raw.round()
            } else {
                raw.ceil()
            }
            .max(1.0) as usize;
            (n, (hi - lo) / n as f64)
        };
        let (umin, umax) = range(0);
        let (strips, width) = cells(umin, umax, us * STEP_ANGLE);
        let (vmin, vmax) = range(1);
        let (rows, height) = if geom.curved_profile() {
            cells(vmin, vmax, vs * STEP_ANGLE)
        } else {
            (1, vmax - vmin)
        };
        let outward = signed_area(&uvs[0]) > 0.0;
        self.columns.insert(
            face,
            Grid {
                umin,
                strips,
                width,
                vmin,
                rows,
                height,
                outward,
            },
        );
        // Grid lines: column angles modulo a turn, and row parameters.
        let columns: Vec<f64> = (0..=strips)
            .map(|k| ((umin + width * k as f64) / us).rem_euclid(std::f64::consts::TAU))
            .collect();
        let rows_at: Vec<f64> = if geom.curved_profile() {
            (0..=rows)
                .map(|j| (vmin + height * j as f64) / vs)
                .collect()
        } else {
            Vec::new()
        };
        let periodic_v = matches!(geom.kind, Profile::Torus { .. });
        // Samples closer than this along an edge would make triangles
        // too thin to keep (the mesh importer drops those and the mesh
        // no longer closes), so nearby samples move onto a grid line or
        // go.
        let delta = 1e-2 * width.min(if rows_at.is_empty() { width } else { height });
        let unwrap = |t: f64, prev: f64| {
            let mut t = t;
            while t - prev > std::f64::consts::PI {
                t -= std::f64::consts::TAU;
            }
            while prev - t > std::f64::consts::PI {
                t += std::f64::consts::TAU;
            }
            t
        };
        let mut seen: Vec<usize> = Vec::new();
        for (_, _, edges) in self.face_bounds(face)? {
            for (edge, _) in edges {
                if seen.contains(&edge) {
                    continue;
                }
                seen.push(edge);
                let pts = self.edge(edge)?;
                // A sample within `delta` of a grid line moves onto it and
                // stays there, so no crossing lands right next to it.
                for &v in &pts {
                    if self.fixed.contains(&v) {
                        continue;
                    }
                    let (t, h) = geom.param(self.vertices[v as usize]);
                    let near_col = columns.iter().copied().find(|&c| {
                        let d = (t - c).rem_euclid(std::f64::consts::TAU);
                        d.min(std::f64::consts::TAU - d) * us < delta
                    });
                    let near_row = rows_at.iter().copied().find(|&r| {
                        let d = if periodic_v {
                            let d = (h - r).rem_euclid(std::f64::consts::TAU);
                            d.min(std::f64::consts::TAU - d)
                        } else {
                            (h - r).abs()
                        };
                        d * vs < delta
                    });
                    if near_col.is_some() || near_row.is_some() {
                        let (t, h) = (near_col.unwrap_or(t), near_row.unwrap_or(h));
                        self.vertices[v as usize] = geom.point(t, h);
                        self.fixed.insert(v);
                    }
                }
                // Movable samples within `delta` of the last kept sample
                // or of the next fixed one go.
                let mut kept: Vec<u32> = vec![pts[0]];
                for (i, &v) in pts.iter().enumerate().skip(1) {
                    if !self.fixed.contains(&v) {
                        let p = self.vertices[v as usize];
                        let prev = self.vertices[*kept.last().unwrap() as usize];
                        let next_fixed = pts[i + 1..]
                            .iter()
                            .find(|w| self.fixed.contains(w))
                            .map(|&w| self.vertices[w as usize]);
                        if (p - prev).length() < delta
                            || next_fixed.is_some_and(|q| (p - q).length() < delta)
                        {
                            continue;
                        }
                    }
                    kept.push(v);
                }
                let mut enriched: Vec<u32> = Vec::with_capacity(kept.len());
                for w in kept.windows(2) {
                    let (a, b) = (self.vertices[w[0] as usize], self.vertices[w[1] as usize]);
                    enriched.push(w[0]);
                    let (ta, ha) = geom.param(a);
                    let (tb, hb) = geom.param(b);
                    let tb = unwrap(tb, ta);
                    let hb = if periodic_v { unwrap(hb, ha) } else { hb };
                    let mut crossings: Vec<(f64, Vec3)> = Vec::new();
                    if (tb - ta).abs() >= 1e-12 {
                        let (lo, hi) = (ta.min(tb), ta.max(tb));
                        for &c in &columns {
                            for k in -1..=1 {
                                let cc = c + k as f64 * std::f64::consts::TAU;
                                if cc <= lo + 1e-9 || cc >= hi - 1e-9 {
                                    continue;
                                }
                                let f = (cc - ta) / (tb - ta);
                                crossings.push((f, geom.point(cc, ha + (hb - ha) * f)));
                            }
                        }
                    }
                    if (hb - ha).abs() >= 1e-12 {
                        let (lo, hi) = (ha.min(hb), ha.max(hb));
                        for &r in &rows_at {
                            for k in -1..=1 {
                                let rr = r + k as f64 * std::f64::consts::TAU;
                                if !periodic_v && k != 0 {
                                    continue;
                                }
                                if rr <= lo + 1e-9 || rr >= hi - 1e-9 {
                                    continue;
                                }
                                let f = (rr - ha) / (hb - ha);
                                crossings.push((f, geom.point(ta + (tb - ta) * f, rr)));
                            }
                        }
                    }
                    crossings.sort_by(|p, q| p.0.partial_cmp(&q.0).unwrap());
                    // A column and a row crossing at one point are one.
                    crossings.dedup_by(|p, q| (p.0 - q.0).abs() < 1e-9);
                    for (_, p) in crossings {
                        let v = self.push(p);
                        self.fixed.insert(v);
                        enriched.push(v);
                    }
                }
                // Interior samples only: drop the two end vertices.
                let interior: Vec<u32> = enriched[1..].to_vec();
                self.edge_samples.insert(edge, interior);
            }
        }
        Ok(())
    }

    /// Triangulates a face into `out`.
    fn face(&mut self, face: usize, out: &mut Vec<[u32; 3]>) -> Result<(), String> {
        let args = self
            .file
            .part(face, "ADVANCED_FACE")
            .or_else(|| self.file.part(face, "FACE_SURFACE"))
            .ok_or_else(|| format!("#{face} is not an ADVANCED_FACE"))?
            .to_vec();
        let surface = args
            .get(2)
            .and_then(Val::reference)
            .ok_or("face without a surface")?;
        let same_sense = args.get(3).and_then(Val::flag).unwrap_or(true);
        let loops = self.face_loops(face)?;
        let kinds = self.file.kinds(surface);
        if kinds.contains(&"PLANE") {
            let pargs = self.file.part(surface, "PLANE").unwrap();
            let (_, axis, _) = self.file.placement(
                pargs
                    .get(1)
                    .and_then(Val::reference)
                    .ok_or("PLANE without a placement")?,
            )?;
            let normal = if same_sense { axis } else { -axis };
            let outer: Vec<Vec3> = loops[0]
                .iter()
                .map(|&v| self.vertices[v as usize])
                .collect();
            // Honour the loop's own winding if it disagrees with the flags.
            let normal = if newell(&outer).dot(normal) < 0.0 {
                -normal
            } else {
                normal
            };
            let x = (outer[1] - outer[0])
                .normalized()
                .and_then(|d| (d - normal * d.dot(normal)).normalized())
                .unwrap_or_else(|| normal.cross(Vec3::Z).normalized().unwrap_or(Vec3::X));
            let y = normal.cross(x);
            let uv = |p: Vec3| [p.dot(x), p.dot(y)];
            let uvs: Vec<Vec<[f64; 2]>> = loops
                .iter()
                .map(|l| l.iter().map(|&v| uv(self.vertices[v as usize])).collect())
                .collect();
            triangulate(&loops, &uvs, out);
            return Ok(());
        }
        if let Some(geom) = self.revolved_of(face)? {
            let grid = *self
                .columns
                .get(&face)
                .ok_or("face on a surface of revolution was not prepared")?;
            let (us, vs) = (geom.u_scale(), geom.v_scale());
            let mut uvs = self.revolved_uvs(&loops, &geom);
            // Loop points the first pass put on a grid line sit on it
            // exactly, give or take float noise.
            let snap = |p: &mut [f64; 2], c: usize, min: f64, step: f64, n: usize| {
                if n == 0 {
                    return;
                }
                let k = ((p[c] - min) / step).round();
                let on = min + step * k;
                if (p[c] - on).abs() < 1e-7 * step {
                    p[c] = on;
                }
            };
            for uv in uvs.iter_mut() {
                for p in uv.iter_mut() {
                    snap(p, 0, grid.umin, grid.width, grid.strips);
                    if geom.curved_profile() {
                        snap(p, 1, grid.vmin, grid.height, grid.rows);
                    }
                }
            }
            // Outward normals go with counter-clockwise (angle, profile)
            // loops; the loop's winding decides, like the plane above.
            let _ = same_sense;
            let sign = if grid.outward { 1.0 } else { -1.0 };
            if !grid.outward {
                for uv in uvs.iter_mut() {
                    for p in uv.iter_mut() {
                        p[0] = -p[0];
                    }
                }
            }
            // The clipper wants material on the left: counter-clockwise
            // outer loop, clockwise holes.
            if signed_area(&uvs[0]) < 0.0 {
                for uv in uvs.iter_mut() {
                    uv.reverse();
                }
            }
            // A triangle must not span more than a facet in either
            // direction, or it cuts a chord through the surface, so the
            // polygon is cut into the grid's cells decided in the first
            // pass and each cell triangulated on its own. Every grid line
            // crosses the loops at vertices the first pass put there,
            // which are reused.
            let to_3d = |p: [f64; 2]| geom.point(sign * p[0] / us, p[1] / vs);
            let quantum = 1e-9 * us.max(1.0);
            let key = |p: [f64; 2]| {
                (
                    (p[0] / quantum).round() as i64,
                    (p[1] / quantum).round() as i64,
                )
            };
            let mut made: HashMap<(i64, i64), u32> = HashMap::new();
            for (l, uv) in loops.iter().zip(&uvs) {
                for (&v, p) in l.iter().zip(uv) {
                    made.entry(key(*p)).or_insert(v);
                }
            }
            let umin = if grid.outward {
                grid.umin
            } else {
                -(grid.umin + grid.width * grid.strips as f64)
            };
            let (vlo, vhi) = uvs
                .iter()
                .flatten()
                .fold((f64::MAX, f64::MIN), |(lo, hi), p| {
                    (lo.min(p[1]), hi.max(p[1]))
                });
            let rows: Vec<(f64, f64)> = if geom.curved_profile() {
                (0..grid.rows)
                    .map(|j| {
                        (
                            grid.vmin + grid.height * j as f64,
                            grid.vmin + grid.height * (j + 1) as f64,
                        )
                    })
                    .collect()
            } else {
                vec![(vlo - 1.0, vhi + 1.0)]
            };
            // Whether a point of the parameter plane is in material.
            let inside =
                |p: [f64; 2]| uvs.iter().filter(|c| point_in_contour(p, c)).count() % 2 == 1;
            for k in 0..grid.strips {
                let (u0, u1) = (
                    umin + grid.width * k as f64,
                    umin + grid.width * (k + 1) as f64,
                );
                for &(v0, v1) in &rows {
                    // Clip lines exactly on the grid: the loop vertices
                    // there (put or snapped there by the first pass) come
                    // out in the neighbouring cells as the same points.
                    let rect: Rect = [u0, v0, u1, v1];
                    let corner = [u0 + 1e-6 * grid.width, v0 + 1e-6 * (v1 - v0)];
                    let pieces = clip_and_close(&uvs, &[], rect, quantum, || Some(inside(corner)))
                        .ok_or_else(|| format!("face #{face}: its loops do not close in a cell"))?;
                    // Holes (clockwise) go with the outer contour that holds them.
                    let (outers, holes): (Vec<Contour>, Vec<Contour>) = pieces
                        .into_iter()
                        .filter(|c| c.len() >= 3)
                        .partition(|c| signed_area(c) > 0.0);
                    let mut groups: Vec<Vec<Contour>> =
                        outers.into_iter().map(|o| vec![o]).collect();
                    for h in holes {
                        let Some(g) = groups.iter_mut().find(|g| point_in_contour(h[0], &g[0]))
                        else {
                            continue;
                        };
                        g.push(h);
                    }
                    for piece in groups {
                        let mut flat: Vec<f64> = Vec::new();
                        let mut holes: Vec<usize> = Vec::new();
                        let mut index: Vec<u32> = Vec::new();
                        for (li, l) in piece.iter().enumerate() {
                            if li > 0 {
                                holes.push(flat.len() / 2);
                            }
                            for p in l {
                                flat.extend(p);
                                let v = match made.get(&key(*p)) {
                                    Some(&v) => v,
                                    None => {
                                        let v = self.push(to_3d(*p));
                                        made.insert(key(*p), v);
                                        v
                                    }
                                };
                                index.push(v);
                            }
                        }
                        let tris = earcutr::earcut(&flat, &holes, 2).unwrap_or_default();
                        for t in tris.chunks_exact(3) {
                            out.push([index[t[0]], index[t[1]], index[t[2]]]);
                        }
                    }
                }
            }
            return Ok(());
        }
        Err(format!(
            "face #{face} lies on a {}, which this reader cannot tessellate yet (planes, cylinders, cones, tori and spheres only)",
            kinds.last().copied().unwrap_or("unknown surface")
        ))
    }
}

/// Earcut over loops given in 2D, outer loop first, all counter-clockwise
/// for the outer one; triangles come out counter-clockwise in that space.
fn triangulate(loops: &[Vec<u32>], uvs: &[Vec<[f64; 2]>], out: &mut Vec<[u32; 3]>) {
    let mut flat: Vec<f64> = Vec::new();
    let mut holes: Vec<usize> = Vec::new();
    let mut index: Vec<u32> = Vec::new();
    for (k, (l, uv)) in loops.iter().zip(uvs).enumerate() {
        if k > 0 {
            holes.push(flat.len() / 2);
        }
        for (v, p) in l.iter().zip(uv) {
            flat.extend(p);
            index.push(*v);
        }
    }
    let tris = earcutr::earcut(&flat, &holes, 2).unwrap_or_default();
    for t in tris.chunks_exact(3) {
        out.push([index[t[0]], index[t[1]], index[t[2]]]);
    }
}

/// Even-odd point-in-polygon test.
fn point_in_contour(p: [f64; 2], c: &[[f64; 2]]) -> bool {
    let n = c.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (c[i], c[(i + 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let x = a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
            if p[0] < x {
                inside = !inside;
            }
        }
    }
    inside
}

fn signed_area(uv: &[[f64; 2]]) -> f64 {
    let n = uv.len();
    (0..n)
        .map(|i| {
            let (a, b) = (uv[i], uv[(i + 1) % n]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f64>()
        / 2.0
}

fn newell(pts: &[Vec3]) -> Vec3 {
    let mut n = Vec3::ZERO;
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        n += a.cross(b);
    }
    n
}

/// De Boor's algorithm for a (rational) B-spline at parameter `t`.
fn de_boor(control: &[Vec3], weights: &[f64], knots: &[f64], degree: usize, t: f64) -> Vec3 {
    let n = control.len();
    // The span k with knots[k] <= t < knots[k + 1].
    let mut k = degree;
    while k + 1 < n && t >= knots[k + 1] {
        k += 1;
    }
    let mut d: Vec<(Vec3, f64)> = (0..=degree)
        .map(|j| {
            let i = j + k - degree;
            (control[i] * weights[i], weights[i])
        })
        .collect();
    for r in 1..=degree {
        for j in (r..=degree).rev() {
            let i = j + k - degree;
            let denom = knots[i + degree - r + 1] - knots[i];
            let alpha = if denom.abs() < 1e-15 {
                0.0
            } else {
                (t - knots[i]) / denom
            };
            d[j] = (
                d[j - 1].0 * (1.0 - alpha) + d[j].0 * alpha,
                d[j - 1].1 * (1.0 - alpha) + d[j].1 * alpha,
            );
        }
    }
    let (p, w) = d[degree];
    if w.abs() < 1e-15 {
        p
    } else {
        p * (1.0 / w)
    }
}

/// Reads every solid of a STEP file as a triangle mesh.
pub fn read_step(text: &str) -> Result<Vec<StepBody>, String> {
    let file = parse(text)?;
    let scale = length_scale(&file);
    let mut ids: Vec<usize> = file
        .entities
        .iter()
        .filter(|(_, parts)| parts.iter().any(|(k, _)| k == "MANIFOLD_SOLID_BREP"))
        .map(|(id, _)| *id)
        .collect();
    ids.sort_unstable();
    if ids.is_empty() {
        return Err("the STEP file has no MANIFOLD_SOLID_BREP solids".into());
    }
    let mut bodies = Vec::new();
    for (k, id) in ids.into_iter().enumerate() {
        let args = file.part(id, "MANIFOLD_SOLID_BREP").unwrap();
        let name = args.first().map(Val::text).unwrap_or("").trim().to_string();
        let name = if name.is_empty() {
            format!("Solid {}", k + 1)
        } else {
            name
        };
        let shell = args
            .get(1)
            .and_then(Val::reference)
            .ok_or("solid without a shell")?;
        let sargs = file
            .part(shell, "CLOSED_SHELL")
            .or_else(|| file.part(shell, "OPEN_SHELL"))
            .ok_or_else(|| format!("#{shell} is not a CLOSED_SHELL"))?;
        let mut reader = Reader {
            file: &file,
            scale,
            vertices: Vec::new(),
            vertex_of: HashMap::new(),
            edge_samples: HashMap::new(),
            columns: HashMap::new(),
            fixed: HashSet::new(),
        };
        let face_ids: Vec<usize> = sargs
            .get(1)
            .map(Val::list)
            .unwrap_or(&[])
            .iter()
            .filter_map(Val::reference)
            .collect();
        // Cylindrical faces first decide their strips and share the
        // columns with their edges; then every face is triangulated.
        for &fid in &face_ids {
            reader.prepare_revolved(fid)?;
        }
        let mut triangles = Vec::new();
        for &fid in &face_ids {
            reader.face(fid, &mut triangles)?;
        }
        bodies.push(StepBody {
            name,
            vertices: reader.vertices,
            triangles,
        });
    }
    Ok(bodies)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cylinder of radius 10 and height 5 written by hand in metres:
    /// two planar caps bounded by circles and a cylindrical wall cut by a
    /// seam, as another CAD system writes it.
    const CYLINDER: &str = r#"ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('cylinder'),'2;1');
FILE_NAME('c.step','2026-01-01',(''),(''),'','','');
FILE_SCHEMA(('AUTOMOTIVE_DESIGN'));
ENDSEC;
DATA;
#1 = ( LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT($,.METRE.) );
#10 = CARTESIAN_POINT('',(0.,0.,0.));
#11 = DIRECTION('',(0.,0.,1.));
#12 = DIRECTION('',(1.,0.,0.));
#13 = AXIS2_PLACEMENT_3D('',#10,#11,#12);
#14 = CARTESIAN_POINT('',(0.,0.,0.005));
#15 = AXIS2_PLACEMENT_3D('',#14,#11,#12);
#20 = CARTESIAN_POINT('',(0.01,0.,0.));
#21 = VERTEX_POINT('',#20);
#22 = CARTESIAN_POINT('',(0.01,0.,0.005));
#23 = VERTEX_POINT('',#22);
#30 = CIRCLE('',#13,0.01);
#31 = EDGE_CURVE('',#21,#21,#30,.T.);
#32 = CIRCLE('',#15,0.01);
#33 = EDGE_CURVE('',#23,#23,#32,.T.);
#34 = VECTOR('',#11,1.);
#35 = LINE('',#20,#34);
#36 = EDGE_CURVE('',#21,#23,#35,.T.);
#40 = ORIENTED_EDGE('',*,*,#31,.F.);
#41 = EDGE_LOOP('',(#40));
#42 = FACE_OUTER_BOUND('',#41,.T.);
#43 = DIRECTION('',(0.,0.,-1.));
#44 = AXIS2_PLACEMENT_3D('',#10,#43,#12);
#45 = PLANE('',#44);
#46 = ADVANCED_FACE('',(#42),#45,.T.);
#50 = ORIENTED_EDGE('',*,*,#33,.T.);
#51 = EDGE_LOOP('',(#50));
#52 = FACE_OUTER_BOUND('',#51,.T.);
#53 = PLANE('',#15);
#54 = ADVANCED_FACE('',(#52),#53,.T.);
#60 = ORIENTED_EDGE('',*,*,#31,.T.);
#61 = ORIENTED_EDGE('',*,*,#36,.T.);
#62 = ORIENTED_EDGE('',*,*,#33,.F.);
#63 = ORIENTED_EDGE('',*,*,#36,.F.);
#64 = EDGE_LOOP('',(#60,#61,#62,#63));
#65 = FACE_OUTER_BOUND('',#64,.T.);
#66 = CYLINDRICAL_SURFACE('',#13,0.01);
#67 = ADVANCED_FACE('',(#65),#66,.T.);
#70 = CLOSED_SHELL('',(#46,#54,#67));
#71 = MANIFOLD_SOLID_BREP('Puck',#70);
ENDSEC;
END-ISO-10303-21;
"#;

    fn mesh_volume(b: &StepBody) -> f64 {
        b.triangles
            .iter()
            .map(|t| {
                let (a, b2, c) = (
                    b.vertices[t[0] as usize],
                    b.vertices[t[1] as usize],
                    b.vertices[t[2] as usize],
                );
                a.dot(b2.cross(c)) / 6.0
            })
            .sum()
    }

    #[test]
    fn a_cylinder_in_metres_comes_back_in_millimetres_and_closed() {
        let bodies = read_step(CYLINDER).unwrap();
        assert_eq!(bodies.len(), 1);
        let b = &bodies[0];
        assert_eq!(b.name, "Puck");
        let expected = std::f64::consts::PI * 100.0 * 5.0;
        let v = mesh_volume(b);
        assert!(
            (v - expected).abs() / expected < 0.01,
            "volume {v} vs {expected}"
        );
        // Every edge is used twice, once each way, once vertices made
        // from the same point (a rim sample and a strip corner) are
        // welded as the kernel welds them: a closed surface.
        let mut canon: HashMap<(i64, i64, i64), u32> = HashMap::new();
        let weld: Vec<u32> = b
            .vertices
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let key = (
                    (p.x * 1e6).round() as i64,
                    (p.y * 1e6).round() as i64,
                    (p.z * 1e6).round() as i64,
                );
                *canon.entry(key).or_insert(i as u32)
            })
            .collect();
        let mut edges: HashMap<(u32, u32), i32> = HashMap::new();
        for t in &b.triangles {
            for k in 0..3 {
                let (a, c) = (weld[t[k] as usize], weld[t[(k + 1) % 3] as usize]);
                if a == c {
                    continue;
                }
                *edges.entry((a.min(c), a.max(c))).or_default() += if a < c { 1 } else { -1 };
            }
        }
        assert!(edges.values().all(|s| *s == 0), "unmatched edges");
        assert!(b.vertices.iter().all(|p| p.z >= -1e-9 && p.z <= 5.0 + 1e-9));
    }

    #[test]
    fn strings_numbers_and_complex_instances_parse() {
        let f = parse("DATA;\n#1 = FOO('it''s',1.5E-3,#2,.T.,$,(1,2),BAR(3));\n#2 = ( A(1) B('x') );\nENDSEC;").unwrap();
        let (kind, args) = &f.parts(1).unwrap()[0];
        assert_eq!(kind, "FOO");
        assert_eq!(args[0], Val::Str("it's".into()));
        assert_eq!(args[1], Val::Num(1.5e-3));
        assert_eq!(args[2], Val::Ref(2));
        assert_eq!(args[3], Val::Enum("T".into()));
        assert_eq!(args[4], Val::Null);
        assert_eq!(args[5], Val::List(vec![Val::Num(1.0), Val::Num(2.0)]));
        assert_eq!(args[6], Val::Typed("BAR".into(), vec![Val::Num(3.0)]));
        assert_eq!(f.kinds(2), vec!["A", "B"]);
        assert!(read_step("garbage").is_err());
    }

    #[test]
    fn a_b_spline_edge_is_sampled_along_the_curve() {
        // A quadratic B-spline through (0,0,0) (1,1,0) (2,0,0): its middle
        // sample is the curve point at t = 0.5, (1, 0.5, 0).
        let text = "DATA;\n#1 = CARTESIAN_POINT('',(0.,0.,0.));\n#2 = CARTESIAN_POINT('',(1.,1.,0.));\n#3 = CARTESIAN_POINT('',(2.,0.,0.));\n#4 = B_SPLINE_CURVE_WITH_KNOTS('',2,(#1,#2,#3),.UNSPECIFIED.,.F.,.F.,(3,3),(0.,1.),.UNSPECIFIED.);\nENDSEC;";
        let file = parse(text).unwrap();
        let reader = Reader {
            file: &file,
            scale: 1.0,
            vertices: Vec::new(),
            vertex_of: HashMap::new(),
            edge_samples: HashMap::new(),
            columns: HashMap::new(),
            fixed: HashSet::new(),
        };
        let pts = reader.bspline_points(4).unwrap();
        assert_eq!(pts.len(), 17);
        assert!(pts[0].distance(Vec3::ZERO) < 1e-12);
        assert!(pts[8].distance(Vec3::new(1.0, 0.5, 0.0)) < 1e-12);
        assert!(pts[16].distance(Vec3::new(2.0, 0.0, 0.0)) < 1e-12);
    }
}
