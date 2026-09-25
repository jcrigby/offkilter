// =====================================================================
//  Overarm Pin Attachment  —  shaft-pivot arm  (rev D)
//
//  The arm is a laminated-ply plate that pivots on a 20 mm shaft held in
//  two SK20 supports screwed to the back of the table.  Two SC20UU blocks
//  bolted under the arm's tail are the pivot bushings; a shaft collar
//  against each block removes sideways float, so nothing on the table
//  locates the arm and stock can be as wide as you like and up to the
//  pivot line deep.  A leveling bolt in the tail lands on the table and
//  sets the arm level (nose height); pin depth is set in the chuck.
//
//  Printed:  chuck (guide-pin clamp under the nose, slotted for Y adjust)
//  Wood:     arm (2 x 3/4" ply laminated, cut from arm_2d)
//  Bought:   20 mm shaft 250 (kit), 2x SK20 (kit), 2x SC20UU (kit),
//            2x 20 mm shaft collars, M5 x 60 x 8, M8 x 100 + jam nut,
//            1/4" drill-rod pins, lid stay or magnet
//
//  Frame: z = 0 table surface, origin = bit axis, +Y toward the back.  mm.
// =====================================================================

/* [Part to render] */
part = "assembly";   // [assembly, exploded, chuck, arm_2d, print_plate]

/* [Table (from trim_router_lift.scad)] */
top_w     = 400;
top_d     = 380;
top_t     = 38;
top_y_off = 50;
disc_d    = 90;

/* [Arm] */
ply_t      = 19;
arm_t      = 2*ply_t;
nose_w     = 80;
nose_y     = -50;
arm_w      = 250;        // full width at the tail
pivot_y    = 170;        // shaft axis behind the bit
tail_y     = 225;        // tail end (table back edge is at 240)
level_y    = 215;        // leveling bolt

/* [Pivot hardware — measure yours] */
shaft_d    = 20;
shaft_len  = 250;
sk_w       = 60;         // SK20 base length (Y)
sk_t       = 20;         // thickness along the shaft (X)
sk_h       = 51;         // base to shaft centre
sk_htot    = 70;
sk_x       = 110;        // SK20 centres at +/- sk_x
blk_w      = 50;         // SC20UU width (Y)
blk_l      = 45;         // length along the shaft (X)
blk_h      = 42;
blk_c      = 25;         // base to shaft centre
blk_bx     = 35;         // bolt spacing along the shaft (X)
blk_by     = 40;         // bolt spacing across (Y)
blk_x      = 60;         // block centres at +/- blk_x
collar_od  = 32;
collar_h   = 12;

/* [Chuck] */
chuck_w    = 36;
chuck_dp   = 30;
chuck_h    = 25;
pin_d      = 6.35;
pin_l      = 75;
pin_out    = 45;
adj        = 3;          // +/- Y slot travel

/* [General] */
$fn = 96;

// ---------------------------------------------------------------------
// Derived
// ---------------------------------------------------------------------
z_shaft = sk_h;                     // SK20 base on the table
z_arm   = z_shaft + blk_c;          // arm underside when level (blocks base-up under the tail)
arm_pts = [[-arm_w/2, tail_y], [arm_w/2, tail_y], [arm_w/2, 100],
           [nose_w/2, nose_y], [-nose_w/2, nose_y], [-arm_w/2, 100]];

echo(str("Arm blank: ", arm_w, " x ", tail_y - nose_y, " x ", arm_t, " mm;  underside ", z_arm, " above the table"));
echo(str("Shaft ", shaft_len, " mm at y = ", pivot_y, ", z = ", z_shaft, ";  SK20s at x = +/-", sk_x, ";  blocks at x = +/-", blk_x));
echo(str("Chuck bottom ", z_arm - chuck_h, " above the table;  stock clearance under the arm ", z_arm, " mm, up to ", pivot_y - blk_w/2, " mm behind the bit"));

// ---------------------------------------------------------------------
// Parts
// ---------------------------------------------------------------------
module arm_2d() { polygon(arm_pts); }
module arm() {
    difference() {
        translate([0, 0, z_arm]) linear_extrude(arm_t) arm_2d();
        // SC20UU bolts, M5 through, heads on top
        for (sx = [-1, 1]) for (dx = [-1, 1]) for (dy = [-1, 1])
            translate([sx*blk_x + dx*blk_bx/2, pivot_y + dy*blk_by/2, z_arm - 1]) cylinder(d = 5.5, h = arm_t + 2);
        // leveling bolt, M8 through an insert
        translate([0, level_y, z_arm - 1]) cylinder(d = 8.5, h = arm_t + 2);
        // chuck screws: slotted in Y
        for (sx = [-1, 1]) hull() for (dy = [-adj, adj])
            translate([sx*12, dy, z_arm - 1]) cylinder(d = 3.5, h = 30);
    }
}
module sk20() {   // base on the table (z = 0), shaft along X at z = sk_h
    difference() {
        union() {
            translate([-sk_t/2, -sk_w/2, 0]) cube([sk_t, sk_w, 12]);
            translate([-sk_t/2, -16, 0]) cube([sk_t, 32, sk_htot]);
        }
        translate([0, 0, sk_h]) rotate([0, 90, 0]) cylinder(d = shaft_d, h = sk_t + 2, center = true);
        translate([-sk_t/2 - 1, -1, sk_h]) cube([sk_t + 2, 2, sk_htot]);
        for (dy = [-1, 1]) translate([0, dy*21, -1]) cylinder(d = 6.6, h = 14);
    }
}
module sc20() {   // base up against the arm underside, bore along X at z_shaft
    difference() {
        translate([-blk_l/2, -blk_w/2, z_arm - blk_h]) cube([blk_l, blk_w, blk_h]);
        translate([0, 0, z_shaft]) rotate([0, 90, 0]) cylinder(d = shaft_d, h = blk_l + 2, center = true);
        for (dx = [-1, 1]) for (dy = [-1, 1]) translate([dx*blk_bx/2, dy*blk_by/2, z_arm - 15]) cylinder(d = 5, h = 16);
    }
}
module collar() { rotate([0, 90, 0]) difference() { cylinder(d = collar_od, h = collar_h, center = true); cylinder(d = shaft_d, h = collar_h + 2, center = true); } }
module chuck() {
    difference() {
        translate([-chuck_w/2, -chuck_dp/2, 0]) cube([chuck_w, chuck_dp, chuck_h]);
        translate([0, 0, -1]) cylinder(d = pin_d + 0.2, h = chuck_h + 2);
        translate([-1, 0, -1]) cube([2, chuck_dp, chuck_h + 2]);
        translate([0, chuck_dp/2 - 8, chuck_h/2]) rotate([0, 90, 0]) cylinder(d = 4.3, h = chuck_w + 2, center = true);
        for (sx = [-1, 1]) translate([sx*12, 0, -1]) { cylinder(d = 4.5, h = chuck_h + 2); cylinder(d1 = 9, d2 = 4.5, h = 4); }
    }
}
module level_bolt() { translate([0, level_y, 0]) { cylinder(d = 8, h = z_arm + arm_t + 10); translate([0, 0, z_arm + arm_t + 2]) cylinder(d = 13/cos(30), h = 6.5, $fn = 6); } }
module guide_pin() { cylinder(d = pin_d, h = pin_l); }
module table_ghost() {
    color("BurlyWood", 0.35) translate([-top_w/2, -top_d/2 + top_y_off, -top_t]) cube([top_w, top_d, top_t]);
    color("Orange") translate([0, 0, -4.7]) difference() { cylinder(d = disc_d, h = 5); translate([0, 0, -1]) cylinder(d = pin_d + 0.3, h = 7); }
}

// ---------------------------------------------------------------------
// Assembly (arm level, pin in the alignment ring)
// ---------------------------------------------------------------------
module assembly() {
    table_ghost();
    color("SlateGray") for (sx = [-1, 1]) translate([sx*sk_x, pivot_y, 0]) sk20();
    color("Silver") translate([0, pivot_y, z_shaft]) rotate([0, 90, 0]) cylinder(d = shaft_d, h = shaft_len, center = true);
    color("SlateGray") for (sx = [-1, 1]) translate([sx*blk_x, pivot_y, 0]) sc20();
    color("DimGray") for (sx = [-1, 1]) translate([sx*(blk_x + blk_l/2 + collar_h/2), pivot_y, z_shaft]) collar();
    color("BurlyWood") arm();
    color("DarkSlateGray") level_bolt();
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
    item("table")      table_ghost();
    item("sk20")       color("SlateGray") for (sx = [-1, 1]) translate([sx*(sk_x + 40), pivot_y, 0]) sk20();
    item("shaft")      color("Silver") translate([0, pivot_y, z_shaft + ex]) rotate([0, 90, 0]) cylinder(d = shaft_d, h = shaft_len, center = true);
    item("sc20uu")     color("SlateGray") for (sx = [-1, 1]) translate([sx*blk_x, pivot_y, 2*ex]) sc20();
    item("collar")     color("DimGray") for (sx = [-1, 1]) translate([sx*(blk_x + blk_l/2 + collar_h/2 + 20), pivot_y, z_shaft + ex]) collar();
    item("arm")        color("BurlyWood") translate([0, 0, 3*ex]) arm();
    item("level_bolt") color("DarkSlateGray") translate([0, 0, 4*ex]) level_bolt();
    item("chuck")      color("Orange") translate([0, 0, z_arm - chuck_h + ex]) chuck();
    item("guide_pin")  color("DarkSlateGray") translate([0, 0, 5]) guide_pin();
}
module print_plate() { chuck(); }

if      (part == "assembly")  assembly();
else if (part == "exploded")  exploded();
else if (part == "chuck")     chuck();
else if (part == "arm_2d")    arm_2d();
else                          print_plate();
