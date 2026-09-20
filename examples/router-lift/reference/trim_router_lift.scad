// =====================================================================
//  Trim Router Lift  —  bench-top table with integral lift  (rev C)
//
//  rev C: linear motion is now two 20 mm hardened shafts in SK20 end
//  supports (bolted to the ply side rails) with two SC20UU closed
//  pillow blocks per shaft on the carriage — the VEVOR SFC20-1000 kit.
//  That removes the 12 mm rods, LM12UU bearings and both printed
//  brackets of rev B.  The leadscrew's 608 bearings press straight
//  into Forstner pockets in the top and the baseplate.
//
//  Printed (ABS/ASA, 6 walls, 50%+ infill):
//      carriage   router clamp with flat side faces for the SC20UU blocks
//                 (M5 bolts into printed nut traps) + T8 nut mount
//      ring       flush reducer rings for the 90 mm opening
//
//  Wood:  top (2 x 3/4" ply laminated), baseplate, two side rails
//  Units mm.  z = 0 is the baseplate top face.  Set `part`, F6, F7.
// =====================================================================

/* [Part to render] */
part = "assembly";   // [assembly, exploded, carriage, carriage_test, ring, print_plate]
test_h = 30;         // height of the carriage_test slice (top of the carriage: bore, nut recess, block holes, nut traps)
ring_open = 40;

/* [Router body] */
router_d   = 65;     // Bosch Colt PR20EVS / Makita: 65 — measure yours
router_clr = 0.3;
rack_w     = 0;      // rack/key strip on the housing (0 = none)
rack_d     = 0;
clamp_h    = 66;     // clamp band height on the motor

/* [SC20UU pillow block — measure yours] */
blk_w      = 50;     // width (along Y when mounted on the carriage side)
blk_l      = 45;     // length along the shaft (Z)
blk_h      = 42;     // base face to far face
blk_c      = 25;     // base face to shaft centre
blk_bx     = 40;     // bolt spacing across (Y)
blk_bz     = 35;     // bolt spacing along the shaft (Z)
blk_bolt   = 5;      // M5
blk_gap    = 10;     // gap between the two blocks on a shaft

/* [SK20 shaft support — measure yours] */
sk_w       = 60;     // base width (along Y on the side rail)
sk_t       = 20;     // thickness along the shaft (Z)
sk_h       = 51;     // base face to shaft centre
sk_htot    = 70;     // overall height
sk_hole    = 42;     // base hole spacing
sk_bolt    = 6;      // M6 through the side rail

/* [Shafts] */
shaft_d    = 20;
travel     = 45;

/* [Leadscrew: T8 + flanged nut, 608 bearings in ply pockets, 8 mm collars, hex coupling nut] */
ls_d         = 8;
nut_flange_d = 22;
nut_body_d   = 10.2;
nut_flange_t = 3.5;
nut_pcd      = 16;
brg_od       = 22;
brg_t        = 7;
collar_od    = 16;
collar_h     = 9;
hexnut_af    = 14.3;
hexnut_len   = 28.6;

/* [Clamp] */
slot_w       = 2.5;
clamp_bolt_d = 5.4;
clamp_ear    = 14;

/* [Top and box] */
top_w     = 400;
top_d     = 380;     // deeper at the back: the rear 155 mm sits on the bench, the box hangs off the edge
top_t     = 38;
top_y_off = 50;
disc_d    = 90;
rabbet_w  = 8;
rabbet_d  = 5;
ring_opens = [0, 30, 40, 55];
base_dp   = 145;
ply_t     = 19;
rail_t    = 19;

/* [General] */
wall = 8;
fit  = 0.2;
tap3 = 2.5;
nut_af   = 8.4;      // M5 nut across flats + clearance
nut_t    = 4.6;      // M5 nut thickness + clearance
$fn  = 96;

// ---------------------------------------------------------------------
// Derived
// ---------------------------------------------------------------------
ls_y      = router_d/2 + wall + nut_flange_d/2 + 2;
car_w     = router_d + 2*wall + 20;                // carriage width across X (flat side faces)
car_h     = 2*blk_l + blk_gap;                     // carriage height = two blocks + gap
body_y0   = -(router_d/2 + wall);
body_y1   = ls_y + nut_flange_d/2 + wall;
ear_w     = 2*(slot_w/2 + wall + clamp_bolt_d);
shaft_x   = car_w/2 + blk_c;                       // shaft axis from centre
rail_x    = shaft_x + sk_h;                        // side rail inner face
base_w    = 2*rail_x;                              // baseplate width between the rails
z_top     = sk_t + 3 + car_h + travel + 3 + sk_t;  // underside of the top
shaft_len = z_top;                                 // bottom SK sits on the baseplate, top SK against the top
z_car     = sk_t + 3 + travel/2;                   // carriage bottom, mid travel
ls_z0     = -(ply_t + collar_h + 1);
ls_len    = (z_top + top_t + 2 + hexnut_len - 3) - ls_z0;
open_d    = disc_d - 2*rabbet_w;
base_y0   = -60;
rail_h    = z_top + ply_t;

echo(str("SHAFT LENGTH (x2, 20 mm): ", shaft_len, " mm  (cut from the 1000 mm kit shafts)"));
echo(str("LEADSCREW LENGTH (T8): ", ls_len, " mm"));
echo(str("Shaft spacing: ", 2*shaft_x, "   box inner width: ", base_w, "   baseplate: ", base_w, " x ", base_dp));
echo(str("Box height below top underside: ", z_top + ply_t, " mm  (overall incl. top: ", z_top + ply_t + top_t, ")"));
echo(str("Carriage: ", car_w, " x ", body_y1 - body_y0 + clamp_ear, " x ", car_h, " mm"));

// ---------------------------------------------------------------------
// Carriage
// ---------------------------------------------------------------------
module carriage() {
    zc = car_h/2;
    difference() {
        union() {
            translate([-car_w/2, body_y0, 0]) cube([car_w, body_y1 - body_y0, car_h]);
            translate([-ear_w/2, body_y0 - clamp_ear, 0]) cube([ear_w, clamp_ear + 4, car_h]);
        }
        translate([0, 0, -1]) cylinder(d = router_d + 2*router_clr, h = car_h + 2);
        if (rack_w > 0) translate([-(rack_w + 1)/2, router_d/2 - 1, -1]) cube([rack_w + 1, rack_d + 1.5, car_h + 2]);
        translate([-slot_w/2, body_y0 - clamp_ear - 1, -1]) cube([slot_w, clamp_ear + wall + 2, car_h + 2]);
        for (z = [zc - clamp_h*0.22, zc + clamp_h*0.22]) {
            translate([0, body_y0 - clamp_ear/2, z]) rotate([0, 90, 0]) cylinder(d = clamp_bolt_d, h = ear_w + 2, center = true);
            translate([-ear_w/2 - 1, body_y0 - clamp_ear/2, z]) rotate([0, 90, 0]) cylinder(d = 8/cos(30) + 0.4, h = 5, $fn = 6);
        }
        // SC20UU mounting: M5 clearance holes in from the side faces + nut traps.
        // One slot per bolt column, entered from the top face (upper block) or the
        // bottom face (lower block); the nut sits 8.3 mm behind the face.
        for (sx = [-1, 1]) for (i = [-1, 1]) for (yy = [-1, 1]) {
            for (zz = [-1, 1])
                translate([sx*car_w/2, yy*blk_bx/2, zc + i*(blk_l + blk_gap)/2 + zz*blk_bz/2]) rotate([0, sx*90, 0])
                    translate([0, 0, -18]) cylinder(d = blk_bolt + 0.5, h = 19);
            z_in = zc + i*(blk_l + blk_gap)/2 - blk_bz/2 - 5;          // lowest bolt of this block, minus a nut's worth
            z_out = zc + i*(blk_l + blk_gap)/2 + blk_bz/2 + 5;
            translate([sx > 0 ? car_w/2 - 6 - nut_t : -car_w/2 + 6, yy*blk_bx/2 - nut_af/2, i > 0 ? z_in : -1])
                cube([nut_t, nut_af, i > 0 ? car_h - z_in + 1 : z_out + 1]);
        }
        // leadscrew nut
        translate([0, ls_y, -1]) cylinder(d = nut_body_d + 0.6, h = car_h + 2);
        translate([0, ls_y, car_h - nut_flange_t - 0.3]) cylinder(d = nut_flange_d + 0.6, h = nut_flange_t + 1);
        for (a = [45, 135, 225, 315])
            translate([nut_pcd/2*cos(a), ls_y + nut_pcd/2*sin(a), car_h - 12]) cylinder(d = tap3, h = 13);
        // lightening: pockets from below between the bore and the side faces (leave 8 mm skins)
        for (sx = [-1, 1]) translate([sx*(car_w/2 - 9) - 3, -blk_w/2 + 10, -1]) cube([6, blk_w - 20, car_h - 8]);
    }
}

module ring(open = 0) {
    difference() {
        union() { cylinder(d = disc_d - 0.4, h = rabbet_d); translate([0, 0, -4]) cylinder(d = open_d - 0.6, h = 4.01); }
        if (open > 0) translate([0, 0, -5]) cylinder(d = open, h = rabbet_d + 6);
        for (a = [0, 180]) translate([(disc_d/2 - 9)*cos(a), (disc_d/2 - 9)*sin(a), -5]) cylinder(d = 5, h = rabbet_d + 6);
    }
}

// ---------------------------------------------------------------------
// Wood parts
// ---------------------------------------------------------------------
module top() {
    difference() {
        translate([-top_w/2, -top_d/2 + top_y_off, 0]) cube([top_w, top_d, top_t]);
        translate([0, 0, -1]) cylinder(d = open_d, h = top_t + 2);
        translate([0, 0, top_t - rabbet_d]) cylinder(d = disc_d + 0.4, h = rabbet_d + 1);
        translate([0, ls_y, -1]) cylinder(d = ls_d + 4, h = top_t + 2);          // leadscrew through
        translate([0, ls_y, -1]) cylinder(d = brg_od, h = brg_t + 1);            // 608 pocket, Forstner 22 mm
    }
}
module baseplate() {
    difference() {
        translate([-base_w/2, base_y0, 0]) cube([base_w, base_dp, ply_t]);
        translate([0, ls_y, -1]) cylinder(d = ls_d + 4, h = ply_t + 2);
        translate([0, ls_y, ply_t - brg_t]) cylinder(d = brg_od, h = brg_t + 1); // 608 pocket
    }
}
module side_rail() {
    difference() {
        translate([-rail_t/2, base_y0, 0]) cube([rail_t, base_dp, rail_h]);
        for (z = [ply_t + sk_t/2, ply_t + z_top - sk_t/2]) for (yy = [-1, 1])
            translate([0, yy*sk_hole/2, z]) rotate([0, 90, 0]) cylinder(d = sk_bolt + 0.6, h = rail_t + 2, center = true);
    }
}

// ---------------------------------------------------------------------
// Hardware
// ---------------------------------------------------------------------
module screw(d, l, head_d, head_h) { cylinder(d = head_d, h = head_h); translate([0, 0, head_h]) cylinder(d = d, h = l); }
module brg608() { difference() { cylinder(d = brg_od, h = brg_t); translate([0, 0, -1]) cylinder(d = ls_d, h = brg_t + 2); } }
module hexnut() { difference() { cylinder(d = hexnut_af/cos(30), h = hexnut_len, $fn = 6); translate([0, 0, -1]) cylinder(d = ls_d + 0.5, h = hexnut_len + 2); } }
// SC20UU: base at x = 0 facing -X, shaft axis vertical at x = blk_c
module sc20() {
    difference() {
        translate([0, -blk_w/2, -blk_l/2]) cube([blk_h, blk_w, blk_l]);
        translate([blk_c, 0, -blk_l/2 - 1]) cylinder(d = shaft_d, h = blk_l + 2);
        for (yy = [-1, 1]) for (zz = [-1, 1]) translate([-1, yy*blk_bx/2, zz*blk_bz/2]) rotate([0, 90, 0]) cylinder(d = blk_bolt, h = 14);
    }
}
// SK20: base at x = 0 facing -X, shaft axis vertical at x = sk_h, thickness sk_t along Z
module sk20() {
    difference() {
        union() {
            translate([0, -sk_w/2, -sk_t/2]) cube([12, sk_w, sk_t]);
            translate([0, -16, -sk_t/2]) cube([sk_htot, 32, sk_t]);
        }
        translate([sk_h, 0, -sk_t/2 - 1]) cylinder(d = shaft_d, h = sk_t + 2);
        translate([sk_h, -1, -sk_t/2 - 1]) cube([sk_htot, 2, sk_t + 2]);                 // clamp split
        for (yy = [-1, 1]) translate([-1, yy*sk_hole/2, 0]) rotate([0, 90, 0]) cylinder(d = sk_bolt + 0.6, h = 14);
    }
}
module router_body() {
    cylinder(d = router_d, h = clamp_h + 60);
    translate([0, 0, clamp_h + 60]) cylinder(d = 24, h = 14);
    translate([0, 0, clamp_h + 74]) cylinder(d = 6.35, h = 12);
}

module blocks_and_supports(explode = 0) {
    for (sx = [-1, 1]) {
        // blocks on the carriage sides
        for (i = [-1, 1]) translate([sx*(car_w/2 + explode), 0, z_car + car_h/2 + i*(blk_l + blk_gap)/2]) mirror([sx < 0 ? 1 : 0, 0, 0]) sc20();
        // supports on the side rails
        for (z = [sk_t/2, z_top - sk_t/2]) translate([sx*(rail_x + explode*2), 0, z]) mirror([sx > 0 ? 1 : 0, 0, 0]) sk20();
    }
}

// ---------------------------------------------------------------------
// Assembly
// ---------------------------------------------------------------------
module assembly() {
    color("BurlyWood") translate([0, 0, -ply_t]) baseplate();
    color("BurlyWood") for (sx = [-1, 1]) translate([sx*(rail_x + rail_t/2), 0, -ply_t]) side_rail();
    color("BurlyWood", 0.55) translate([0, 0, z_top]) top();
    color("Orange") translate([0, 0, z_top + top_t - rabbet_d]) ring(40);
    color("Silver") for (sx = [-1, 1]) translate([sx*shaft_x, 0, 0]) cylinder(d = shaft_d, h = shaft_len);
    color("SlateGray") blocks_and_supports();
    color("Gold") translate([0, ls_y, ls_z0]) cylinder(d = ls_d, h = ls_len);
    color("Silver") { translate([0, ls_y, -brg_t]) brg608(); translate([0, ls_y, z_top]) brg608(); }
    color("DimGray") { translate([0, ls_y, 0]) cylinder(d = collar_od, h = collar_h); translate([0, ls_y, -ply_t - collar_h]) cylinder(d = collar_od, h = collar_h); }
    color("Orange") translate([0, 0, z_car]) carriage();
    color("DarkSlateGray") translate([0, ls_y, z_top + top_t + 2]) hexnut();
    color("DimGray", 0.35) translate([0, 0, z_car + car_h/2 - clamp_h/2 - 10]) router_body();
}

// ---------------------------------------------------------------------
// Exploded view
// ---------------------------------------------------------------------
only = "";
module item(name) { if (only == "" || only == name) children(); }

module exploded() {
    ex = 45;
    item("baseplate")  color("BurlyWood") translate([0, 0, -ply_t - ex]) baseplate();
    item("side_rail")  color("BurlyWood") for (sx = [-1, 1]) translate([sx*(rail_x + rail_t/2 + 110), 0, -ply_t - ex]) side_rail();
    item("shaft")      color("Silver") for (sx = [-1, 1]) translate([sx*shaft_x, 0, 0]) cylinder(d = shaft_d, h = shaft_len);
    item("sc20uu")     color("SlateGray") for (sx = [-1, 1]) for (i = [-1, 1])
        translate([sx*(car_w/2 + 55), 0, z_car + car_h/2 + i*(blk_l + blk_gap)/2]) mirror([sx < 0 ? 1 : 0, 0, 0]) sc20();
    item("sk20")       color("SlateGray") for (sx = [-1, 1]) for (z = [sk_t/2, z_top - sk_t/2])
        translate([sx*(rail_x + 60), 0, z]) mirror([sx > 0 ? 1 : 0, 0, 0]) sk20();
    item("bearing608") color("Silver") { translate([0, ls_y, -brg_t - 25]) brg608(); translate([0, ls_y, z_top + 20]) brg608(); }
    item("collar")     color("DimGray") { translate([0, ls_y, 12]) cylinder(d = collar_od, h = collar_h); translate([0, ls_y, -ply_t - ex - ply_t - 25 - collar_h]) cylinder(d = collar_od, h = collar_h); }   // lower collar: under the (exploded) baseplate
    item("leadscrew")  color("Gold") translate([0, ls_y, ls_z0 - ex - ply_t - 40]) cylinder(d = ls_d, h = ls_len);
    item("carriage")   color("Orange") translate([0, 0, z_car]) carriage();
    item("nut")        color("Gold") translate([0, ls_y, z_car + car_h + 26]) { cylinder(d = nut_flange_d, h = nut_flange_t); translate([0, 0, -12]) cylinder(d = nut_body_d, h = 12); }
    item("clamp_bolt") color("DarkSlateGray") for (z = [car_h/2 - clamp_h*0.22, car_h/2 + clamp_h*0.22])
        translate([-ear_w/2 - 34, body_y0 - clamp_ear/2, z_car + z]) rotate([0, 90, 0]) screw(5, 40, 8.5, 5);
    item("top")        color("BurlyWood") translate([0, 0, z_top + ex]) top();
    item("ring")       color("Orange") { translate([0, 0, z_top + ex + top_t + ex]) ring(0);
        for (i = [1 : len(ring_opens) - 1]) translate([-(disc_d + 20) - (i - 1)*(disc_d + 12), -30, z_top + ex + top_t + ex]) ring(ring_opens[i]); }
    item("hex_nut")    color("DarkSlateGray") translate([0, ls_y, z_top + ex + top_t + ex + 40]) hexnut();
    item("router")     color("DimGray") translate([-(rail_x + rail_t + 110 + 60), -60, z_car - 120]) router_body();
}

module print_plate() {
    carriage();
    for (i = [0 : len(ring_opens) - 1]) translate([car_w/2 + 70 + (i % 2)*(disc_d + 10), -40 + floor(i/2)*(disc_d + 10), 4]) ring(ring_opens[i]);
}

if      (part == "assembly") assembly();
else if (part == "exploded") exploded();
else if (part == "carriage") carriage();
else if (part == "carriage_test")            // top slice, flipped so the top face is on the bed
    translate([0, 0, car_h]) mirror([0, 0, 1]) intersection() { carriage(); translate([-200, -200, car_h - test_h]) cube([400, 400, test_h + 1]); }
else if (part == "ring")     translate([0, 0, 4]) ring(ring_open);
else                         print_plate();
