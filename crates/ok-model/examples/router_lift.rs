//! Router lift parts built through the Op API, as a kernel test and as
//! .okpart documents to open in the web client.
//!
//!     cargo run -p ok-model --example router_lift
//!
//! Writes carriage.okpart, ring_align.okpart, post_round.okpart and
//! post_slot.okpart into the current directory and prints, for each part,
//! the number of bodies, any feature errors, and the solid volume next to
//! the volume of the same part from the OpenSCAD model (trimesh on the
//! exported STL, $fn = 96).  The kernel facets curves at 5 deg by default,
//! so a bore's volume lands ~0.2 % under the $fn = 96 mesh; anything past
//! about 0.5 % means a feature did something different from the CSG model.
//!
//! Frame is the OpenSCAD part frame: carriage z = 0 at its bottom face,
//! router bore on the Z axis, leadscrew nut at +Y, clamp ears at -Y.
//! Right plane sketch coordinates are (Y, Z).

use ok_math::Vec2;
use ok_model::{
    BodyOp, Counterbore, Document, ExtrudeDirection, ExtrudeEnd, FeatureId, Op, PartStudio,
    PlaneRef, ProfileSelection, RegenResult, SketchOp, StandardPlane,
};

struct Part {
    ps: PartStudio,
}

impl Part {
    fn new(name: &str) -> Part {
        Part {
            ps: PartStudio::new(name),
        }
    }

    fn op(&mut self, op: Op) -> FeatureId {
        let r = self
            .ps
            .apply(op.clone())
            .unwrap_or_else(|e| panic!("{op:?}: {e}"));
        r.feature.unwrap_or(FeatureId(0))
    }

    fn sketch(&mut self, base: StandardPlane, offset: f64, name: &str) -> FeatureId {
        self.op(Op::AddSketch {
            plane: PlaneRef::Standard { base, offset },
            name: Some(name.into()),
        })
    }

    fn draw(&mut self, sketch: FeatureId, op: SketchOp) {
        self.ps.apply(Op::Sketch { id: sketch, op }).unwrap();
    }

    fn rect(&mut self, s: FeatureId, a: (f64, f64), b: (f64, f64)) {
        self.draw(
            s,
            SketchOp::AddRectangle {
                a: Vec2::new(a.0, a.1),
                b: Vec2::new(b.0, b.1),
            },
        );
    }

    fn circle(&mut self, s: FeatureId, c: (f64, f64), r: f64) {
        self.draw(
            s,
            SketchOp::AddCircle {
                center: Vec2::new(c.0, c.1),
                radius: r,
            },
        );
    }

    fn point(&mut self, s: FeatureId, p: (f64, f64)) {
        self.draw(
            s,
            SketchOp::AddPoint {
                pos: Vec2::new(p.0, p.1),
            },
        );
    }

    fn polygon(&mut self, s: FeatureId, pts: &[(f64, f64)]) {
        for i in 0..pts.len() {
            let a = pts[i];
            let b = pts[(i + 1) % pts.len()];
            self.draw(
                s,
                SketchOp::AddLine {
                    a: Vec2::new(a.0, a.1),
                    b: Vec2::new(b.0, b.1),
                },
            );
        }
    }

    fn hexagon(&mut self, s: FeatureId, c: (f64, f64), circumradius: f64) {
        self.draw(
            s,
            SketchOp::AddPolygon {
                center: Vec2::new(c.0, c.1),
                vertex: Vec2::new(c.0 + circumradius, c.1),
                sides: 6,
            },
        );
    }

    fn extrude(
        &mut self,
        s: FeatureId,
        depth: f64,
        direction: ExtrudeDirection,
        profiles: ProfileSelection,
        op: BodyOp,
        name: &str,
    ) -> FeatureId {
        self.op(Op::AddExtrude {
            sketch: s,
            depth,
            direction,
            end: ExtrudeEnd::Blind,
            profiles,
            op,
            name: Some(name.into()),
        })
    }

    fn cut(&mut self, s: FeatureId, depth: f64, direction: ExtrudeDirection, name: &str) {
        self.extrude(
            s,
            depth,
            direction,
            ProfileSelection::All,
            BodyOp::Remove,
            name,
        );
    }

    fn hole(
        &mut self,
        s: FeatureId,
        diameter: f64,
        depth: f64,
        direction: ExtrudeDirection,
        counterbore: Option<Counterbore>,
        name: &str,
    ) {
        self.op(Op::AddHole {
            sketch: s,
            diameter,
            depth,
            through_all: depth <= 0.0,
            direction,
            counterbore,
            name: Some(name.into()),
        });
    }

    fn report(&mut self, reference_volume: f64) -> RegenResult {
        let r = self.ps.regenerate();
        let errors: Vec<String> = r
            .statuses
            .iter()
            .filter_map(|s| {
                s.error
                    .clone()
                    .map(|e| format!("  feature {:?}: {e}", s.id))
            })
            .collect();
        let volume: f64 = r.bodies.iter().map(|b| b.solid.volume()).sum();
        let valid = r.bodies.iter().all(|b| b.solid.validate().is_ok());
        println!(
            "{:<12} bodies {}  valid {}  volume {:>10.0} mm3  (OpenSCAD {:>10.0}, {:+.2} %)",
            self.ps.name,
            r.bodies.len(),
            valid,
            volume,
            reference_volume,
            (volume - reference_volume) / reference_volume * 100.0
        );
        for e in errors {
            println!("{e}");
        }
        r
    }

    fn save(&self, path: &str) {
        std::fs::write(path, Document::from_studio(self.ps.clone()).to_json())
            .unwrap_or_else(|e| panic!("write {path}: {e}"));
        println!("  wrote {path}");
    }
}

use ExtrudeDirection::{Normal, Reverse};
use StandardPlane::{Right, Top};

// ---------------------------------------------------------------------
// Carriage (trim_router_lift.scad rev C, defaults: router_d 65, blocks
// SC20UU 40 x 35 pattern, car_w 101, car_h 100)
// ---------------------------------------------------------------------
fn carriage() -> Part {
    let mut p = Part::new("carriage");
    let (car_w, car_h) = (101.0, 100.0);
    let (y0, y1) = (-40.5, 68.5); // body front (router boss) and back (nut boss)
    let ear_w = 25.3;
    let clamp_ear = 14.0;
    let ls_y = 51.5;
    let bore_r = 65.0 / 2.0 + 0.3;
    let zc = car_h / 2.0;

    // 1. Outline with the clamp ears, minus the router bore, extruded 100.
    let s = p.sketch(Top, 0.0, "outline");
    p.polygon(
        s,
        &[
            (-car_w / 2.0, y0),
            (-ear_w / 2.0, y0),
            (-ear_w / 2.0, y0 - clamp_ear),
            (ear_w / 2.0, y0 - clamp_ear),
            (ear_w / 2.0, y0),
            (car_w / 2.0, y0),
            (car_w / 2.0, y1),
            (-car_w / 2.0, y1),
        ],
    );
    p.circle(s, (0.0, 0.0), bore_r);
    p.extrude(
        s,
        car_h,
        Normal,
        ProfileSelection::Largest,
        BodyOp::New,
        "body",
    );

    // 2. Clamp slot from the front of the ears into the bore.
    let s = p.sketch(Top, -1.0, "slot");
    p.rect(s, (-1.25, y0 - clamp_ear - 1.0), (1.25, -31.5));
    p.cut(s, car_h + 2.0, Normal, "clamp slot");

    // 3. Leadscrew nut: 10.8 through with a 22.6 x 3.8 recess in the top face.
    let s = p.sketch(Top, car_h, "nut");
    p.point(s, (0.0, ls_y));
    p.hole(
        s,
        10.8,
        0.0,
        Reverse,
        Some(Counterbore {
            diameter: 22.6,
            depth: 3.8,
        }),
        "nut bore + recess",
    );

    // 4. Four M3 self-tapping holes on the nut flange PCD.
    let s = p.sketch(Top, car_h, "nut screws");
    for a in [45.0f64, 135.0, 225.0, 315.0] {
        let (sn, cs) = a.to_radians().sin_cos();
        p.point(s, (8.0 * cs, ls_y + 8.0 * sn));
    }
    p.hole(s, 2.5, 13.0, Reverse, None, "M3 tap holes");

    // 5. SC20UU bolt holes, eight per side face, 19 deep.
    let zs = [
        zc - 27.5 - 17.5,
        zc - 27.5 + 17.5,
        zc + 27.5 - 17.5,
        zc + 27.5 + 17.5,
    ];
    for (offset, dir, label) in [(car_w / 2.0, Reverse, "+X"), (-car_w / 2.0, Normal, "-X")] {
        let s = p.sketch(Right, offset, &format!("block holes {label}"));
        for y in [-20.0, 20.0] {
            for z in zs {
                p.point(s, (y, z));
            }
        }
        p.hole(s, 5.5, 19.0, dir, None, &format!("M5 clearance {label}"));
    }

    // 6. Nut traps: 8.4 x 4.6 slots, nut 8.3 behind the face; upper block's
    //    from the top face, lower block's from the bottom face.
    let (xi, xo) = (car_w / 2.0 - 6.0 - 4.6, car_w / 2.0 - 6.0);
    let z_in = zc + 27.5 - 17.5 - 5.0;
    let z_out = zc - 27.5 + 17.5 + 5.0;
    let s = p.sketch(Top, car_h, "upper nut traps");
    for sx in [-1.0, 1.0] {
        for y in [-20.0, 20.0] {
            p.rect(s, (sx * xi, y - 4.2), (sx * xo, y + 4.2));
        }
    }
    p.cut(s, car_h - z_in, Reverse, "upper nut traps");
    let s = p.sketch(Top, -1.0, "lower nut traps");
    for sx in [-1.0, 1.0] {
        for y in [-20.0, 20.0] {
            p.rect(s, (sx * xi, y - 4.2), (sx * xo, y + 4.2));
        }
    }
    p.cut(s, z_out + 1.0, Normal, "lower nut traps");

    // 7. Clamp bolts through both ears along X, and hex nut traps on the -X ear.
    let bolt_y = y0 - clamp_ear / 2.0;
    let bolt_z = [zc - 66.0 * 0.22, zc + 66.0 * 0.22];
    let s = p.sketch(Right, -ear_w / 2.0 - 5.0, "clamp bolts");
    for z in bolt_z {
        p.point(s, (bolt_y, z));
    }
    p.hole(s, 5.4, 0.0, Normal, None, "M5 clamp bolts");
    let s = p.sketch(Right, -ear_w / 2.0, "hex traps");
    for z in bolt_z {
        p.hexagon(s, (bolt_y, z), 8.0 / 30f64.to_radians().cos() / 2.0 + 0.2);
    }
    p.cut(s, 4.0, Normal, "M5 nut traps");

    // 8. Lightening pockets from the bottom, leaving 8 mm skins.
    let s = p.sketch(Top, -1.0, "pockets");
    for sx in [-1.0, 1.0] {
        p.rect(
            s,
            (sx * (car_w / 2.0 - 12.0), -15.0),
            (sx * (car_w / 2.0 - 6.0), 15.0),
        );
    }
    p.cut(s, car_h - 8.0 + 1.0, Normal, "lightening pockets");

    p
}

// ---------------------------------------------------------------------
// Alignment ring: 89.6 x 5 disc on a 73.4 x 4 spigot, 1/4" centre hole,
// two 5 mm spanner holes.
// ---------------------------------------------------------------------
fn ring_align() -> Part {
    let mut p = Part::new("ring_align");
    let s = p.sketch(Top, 0.0, "disc");
    p.circle(s, (0.0, 0.0), 89.6 / 2.0);
    p.extrude(s, 5.0, Normal, ProfileSelection::All, BodyOp::New, "disc");
    let s = p.sketch(Top, -4.0, "spigot");
    p.circle(s, (0.0, 0.0), 73.4 / 2.0);
    p.extrude(
        s,
        4.01,
        Normal,
        ProfileSelection::All,
        BodyOp::Add,
        "spigot",
    );
    let s = p.sketch(Top, 5.0, "holes");
    p.point(s, (0.0, 0.0));
    p.hole(s, 6.6, 0.0, Reverse, None, "pin hole");
    let s = p.sketch(Top, 5.0, "spanner holes");
    p.point(s, (36.0, 0.0));
    p.point(s, (-36.0, 0.0));
    p.hole(s, 5.0, 0.0, Reverse, None, "spanner holes");
    p
}

// ---------------------------------------------------------------------
// Registration post: 40 x 40 x 75, 10.3 socket 22 deep with a 16 x 6 step
// (the SCAD version has a cone; a step is the closest hole feature), two
// 4.5 screw holes.  `slot` lengthens the socket 4 mm in X.
// ---------------------------------------------------------------------
fn post(slot: bool) -> Part {
    let mut p = Part::new(if slot { "post_slot" } else { "post_round" });
    let s = p.sketch(Top, 0.0, "block");
    p.rect(s, (-20.0, -20.0), (20.0, 20.0));
    p.extrude(s, 75.0, Normal, ProfileSelection::All, BodyOp::New, "block");
    if slot {
        for (w, d, name) in [(10.3, 22.0, "socket"), (16.0, 6.0, "step")] {
            let s = p.sketch(Top, 75.0, name);
            p.draw(
                s,
                SketchOp::AddSlot {
                    a: Vec2::new(-2.0, 0.0),
                    b: Vec2::new(2.0, 0.0),
                    width: w,
                },
            );
            p.cut(s, d, Reverse, name);
        }
    } else {
        let s = p.sketch(Top, 75.0, "socket");
        p.point(s, (0.0, 0.0));
        p.hole(
            s,
            10.3,
            22.0,
            Reverse,
            Some(Counterbore {
                diameter: 16.0,
                depth: 6.0,
            }),
            "socket + step",
        );
    }
    let s = p.sketch(Top, 75.0, "screws");
    p.point(s, (0.0, 13.0));
    p.point(s, (0.0, -13.0));
    p.hole(s, 4.5, 0.0, Reverse, None, "screw holes");
    p
}

fn main() {
    // Reference volumes: trimesh on the STLs exported from the OpenSCAD
    // models ($fn = 96).  The posts' cone vs step differ by design:
    // step - cone = pi*(8^2 - (8^2 + 8*5.15 + 5.15^2)/3)*6 ~ +140 mm3.
    let mut c = carriage();
    c.report(775_369.0);
    c.save("carriage.okpart");

    let mut r = ring_align();
    r.report(47_809.0);
    r.save("ring_align.okpart");

    let mut pr = post(false);
    pr.report(115_330.0 - 140.0);
    pr.save("post_round.okpart");

    let mut ps = post(true);
    ps.report(114_356.0 - 140.0);
    ps.save("post_slot.okpart");
}
