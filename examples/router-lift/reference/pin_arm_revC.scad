// =====================================================================
//  Overarm Pin Attachment  —  hinged arm  (rev C)
//
//  The arm is a laminated-ply plate, wide at the back where a piano hinge
//  joins it to a rear rail on the table, tapering to a nose that carries
//  the guide-pin chuck over the bit.  Lift the arm to move the template;
//  drop it and one registration pin behind the chuck seats in a socket on
//  top of a short post on the centreline.  The hinge fixes Y and rotation;
//  the post fixes X and the nose height.  The arm rests on the rail and
//  the post, so height is fixed by construction.
//
//  Printed:  post_round, post_slot (registration sockets), chuck (guide-pin
//            clamp under the nose).  Plus ring_align in the lift file.
//  Wood:     arm (2 x 3/4" ply laminated, cut from arm_2d), rear rail.
//  Bought:   1-1/2" piano hinge ~250 long, 2x 10 mm dowel pins, 1/4" drill
//            rod pins, lid stay, 2x M4 clamp bolts, wood screws.
//
//  Frame: z = 0 table surface, origin = bit axis, +Y toward the back.  mm.
// =====================================================================

/* [Part to render] */
part = "assembly";   // [assembly, exploded, post, post_test, chuck, arm_2d, print_plate]

/* [Table (from trim_router_lift.scad)] */
top_w     = 400;
top_d     = 380;
top_t     = 38;
top_y_off = 50;
disc_d    = 90;

/* [Arm and rail] */
ply_t      = 19;
arm_t      = 2*ply_t;    // laminated plate thickness
arm_clear  = 75;         // arm underside above the table = rail height = post height
rail_d     = 60;         // rail depth (Y); the arm's tail lies on it
rail_w     = 250;        // rail and hinge length (X)
rail_y1    = 240;        // back edge of the top
nose_w     = 80;         // arm width at the nose
nose_y     = -50;        // nose front edge
hinge_w    = 38;         // 1-1/2" piano hinge, open
hinge_t    = 1.2;

/* [Registration] */
reg_x      = 0;          // single post on the centreline
reg_y      = 70;         // behind the ring; the hinge fixes Y and rotation, the post fixes X and nose height
reg_pin_d  = 10;         // 10 mm dowel pins
reg_pin_l  = 40;         // 20 pressed into the arm, 20 exposed
post_w     = 40;
post_h     = 75;         // = arm_clear
socket_d   = 10.3;
slot_len   = 4;          // extra length of the slotted socket (X)
cone_h     = 6;
cone_d     = 16;

/* [Chuck] */
chuck_w    = 36;
chuck_dp   = 30;
chuck_h    = 25;
pin_d      = 6.35;
pin_l      = 75;
pin_out    = 45;         // exposed below the chuck as drawn (slide to set)

/* [General] */
$fn = 96;

// ---------------------------------------------------------------------
// Derived
// ---------------------------------------------------------------------
rail_y0 = rail_y1 - rail_d;                 // hinge line
z_arm   = arm_clear;                        // arm underside when down
arm_pts = [[-rail_w/2, rail_y1], [rail_w/2, rail_y1], [rail_w/2, rail_y0],
           [nose_w/2, nose_y], [-nose_w/2, nose_y], [-rail_w/2, rail_y0]];

echo(str("Arm blank: ", rail_w, " x ", rail_y1 - nose_y, " x ", arm_t, " mm (X x Y x thickness)"));
echo(str("Rail: ", rail_w, " x ", rail_d, " x ", arm_clear, " mm;  piano hinge ", rail_w, " mm long on the hinge line y = ", rail_y0));
echo(str("Post at x = ", reg_x, ", y = ", reg_y, "; ", post_h, " tall.  Chuck bottom ", z_arm - chuck_h, " mm above the table"));

// ---------------------------------------------------------------------
// Parts
// ---------------------------------------------------------------------
module arm_2d() { polygon(arm_pts); }
module arm() {
    difference() {
        translate([0, 0, z_arm]) linear_extrude(arm_t) arm_2d();
        translate([reg_x, reg_y, z_arm - 1]) cylinder(d = reg_pin_d - 0.1, h = 21);   // press-fit pin hole
        for (sx = [-1, 1]) translate([sx*12, 0, z_arm - 1]) cylinder(d = 3.5, h = 30);                    // chuck screws
    }
}
module rail() {
    translate([-rail_w/2, rail_y0, 0]) cube([rail_w, rail_d, arm_clear]);
}
module hinge() {   // knuckle on the rail's front top corner; leaves down the rail face and under the arm
    color("Silver") {
        translate([0, rail_y0, z_arm]) rotate([0, 90, 0]) cylinder(d = 5, h = rail_w, center = true);
        translate([-rail_w/2, rail_y0 - hinge_t, z_arm - hinge_w/2]) cube([rail_w, hinge_t, hinge_w/2]);
        translate([-rail_w/2, rail_y0 - hinge_w/2, z_arm - hinge_t]) cube([rail_w, hinge_w/2, hinge_t]);
    }
}
module post(slot = false) {
    difference() {
        translate([-post_w/2, -post_w/2, 0]) cube([post_w, post_w, post_h]);
        // socket: cone entry then straight, slotted in X on the slot post
        hull() for (sx = (slot ? [-1, 1] : [0])) translate([sx*slot_len/2, 0, post_h - cone_h]) cylinder(d1 = socket_d, d2 = cone_d, h = cone_h + 0.01);
        hull() for (sx = (slot ? [-1, 1] : [0])) translate([sx*slot_len/2, 0, post_h - cone_h - 16]) cylinder(d = socket_d, h = 17);
        // two countersunk screws to the top
        for (sy = [-1, 1]) translate([0, sy*13, -1]) { cylinder(d = 4.5, h = post_h + 2); translate([0, 0, 1]) cylinder(d1 = 4.5, d2 = 9, h = 3); }
    }
}
module chuck() {
    difference() {
        translate([-chuck_w/2, -chuck_dp/2, 0]) cube([chuck_w, chuck_dp, chuck_h]);
        translate([0, 0, -1]) cylinder(d = pin_d + 0.2, h = chuck_h + 2);
        translate([-1, 0, -1]) cube([2, chuck_dp, chuck_h + 2]);                                 // split to the back face
        translate([0, chuck_dp/2 - 8, chuck_h/2]) rotate([0, 90, 0]) cylinder(d = 4.3, h = chuck_w + 2, center = true);   // M4 clamp
        for (sx = [-1, 1]) translate([sx*12, 0, -1]) { cylinder(d = 4.5, h = chuck_h + 2); cylinder(d1 = 9, d2 = 4.5, h = 4); }  // screws up into the arm
    }
}
module reg_pin() { cylinder(d = reg_pin_d, h = reg_pin_l - 2); translate([0, 0, reg_pin_l - 2]) cylinder(d1 = reg_pin_d, d2 = reg_pin_d - 4, h = 2); }
module guide_pin() { cylinder(d = pin_d, h = pin_l); }
module table_ghost() {
    color("BurlyWood", 0.35) translate([-top_w/2, -top_d/2 + top_y_off, -top_t]) cube([top_w, top_d, top_t]);
    color("Orange") translate([0, 0, -4.7]) difference() { cylinder(d = disc_d, h = 5); translate([0, 0, -1]) cylinder(d = pin_d + 0.3, h = 7); }   // alignment ring
}

// ---------------------------------------------------------------------
// Assembly (arm down, pin in the alignment ring)
// ---------------------------------------------------------------------
module assembly() {
    table_ghost();
    color("BurlyWood") rail();
    color("BurlyWood") arm();
    hinge();
    color("Orange") translate([reg_x, reg_y, 0]) post();
    color("Silver") translate([reg_x, reg_y, z_arm + 20]) mirror([0, 0, 1]) reg_pin();
    color("Orange") translate([0, 0, z_arm - chuck_h]) chuck();
    color("DarkSlateGray") translate([0, 0, z_arm - chuck_h - pin_out]) guide_pin();
}

// ---------------------------------------------------------------------
// Exploded view
// ---------------------------------------------------------------------
only = "";
module item(name) { if (only == "" || only == name) children(); }
module exploded() {
    ex = 50;
    item("table")     table_ghost();
    item("rail")      color("BurlyWood") rail();
    item("post")      color("Orange") translate([reg_x, reg_y, 0]) post();
    item("arm")       color("BurlyWood") translate([0, 0, 2*ex]) arm();
    item("hinge")     translate([0, -40, ex]) hinge();
    item("reg_pin")   color("Silver") translate([reg_x, reg_y, ex + 15]) reg_pin();
    item("chuck")     color("Orange") translate([0, 0, z_arm - chuck_h + ex - 20]) chuck();
    item("guide_pin") color("DarkSlateGray") translate([0, 0, 5]) guide_pin();
}
module print_plate() {
    post();
    translate([70, 0, 0]) chuck();
}

if      (part == "assembly")   assembly();
else if (part == "exploded")   exploded();
else if (part == "post")       post();
else if (part == "post_test")  translate([0, 0, -post_h + 25]) intersection() { post(); translate([-30, -30, post_h - 25]) cube([60, 60, 26]); }   // top 25 mm of the socket
else if (part == "chuck")      chuck();
else if (part == "arm_2d")     arm_2d();
else                           print_plate();
