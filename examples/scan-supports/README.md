# Scan supports: 20 mm cubes and 40 mm posts

Three blocks to stand an object on for a turntable scan
(`examples/turntable-scan/`), and three taller ones for the same object
turned over. Set each on a bullseye mark of the measuring field and it
is a block of known height at a known place.

    cargo build --release -p ok-mcp
    python3 examples/scan-supports/build.py       # the document, the STLs, pictures, numbers
    cargo test -p ok-render --test scan_supports  # what CI runs

![a cube and a post](out/cube.png)

## Why

- **The lift.** The scanner takes everything under `min_height` as
  the table, and the lowest centimetre of an object standing on the
  paper is lost to the table's glow and to the line meeting the paper
  at a grazing angle. On 20 mm blocks the base is at 20 mm; set
  `min_height` to 22 and the blocks and the table drop out of the
  cloud entirely, since the blocks are 20 mm tall.
- **The second pose.** Turned over onto the blocks and scanned again,
  the object's underside is seen and `merge_scans` with
  `placed: "upside down"` joins the two. A pumpkin turned over rests
  on its shoulders with the stem down, and stems run to 35 mm, so the
  posts are 40 mm; `min_height` is then 42.
- **The check on the laser's plane.** Every point of a block's top
  face is at 20 (or 40) mm wherever the block stands. The laser's line
  on that face, triangulated, should come out at that height; where it
  does not, the plane is off, which the line on the paper alone cannot
  say. The line on the top face and the line on the paper together fix
  the plane outright, with no need for the block's position or for the
  aim through the axis. This is the calibration object the scanner's
  roadmap asks for.

## What it is

| part | size | what it does |
|------|------|--------------|
| cube | 20 x 20 x 20 mm | one of three supports under the object, standing on a mark |
| post | 20 x 20 x 40 mm | the same for the object turned over, with room for a stem |

Both are plain blocks: every face square to the axes, so they print
flat on any side and their heights are what the scanner checks. Print
them white or light grey and matte: the marks are read as dark ink
against their surroundings, and a black block is a dark blob with
edges; a red line reads on light PLA as it does on the paper. Three of
each, `out/cube20.stl` and `out/post40.stl`.

## Placing them

Pick the three marks nearest the turntable's centre that form a
triangle inside the object's outline, say the marks printed
`200, 150`, `250, 150` and `200, 200` (every mark's coordinates are
printed beside it), which spread the supports 50 and 71 mm apart. A
block covers its mark whole: the bullseye's ring is 16 mm across and
the block 20, so only the code dots peek out beyond two of its edges.
Centre it by the grid instead: the field prints a 10 mm grid and every
mark sits on a grid crossing, so a centred block has all four edges on
grid lines. It need not be exact. The check on the laser's plane uses
only the block's height, which is the same wherever it stands, and the
scanner can take the block's footprint from its base edges in the
picture; the three marks under the blocks are lost to the pose, which
has the eight to fifteen others. Tell the scan which marks the blocks
stand on.

## Measure these first

| what | in the model | how to check |
|------|--------------|--------------|
| the cube's height | 20.00 mm | calipers across the print; the scanner checks the laser's plane against this number, so a printer that runs 0.2 mm tall is 0.2 mm of error at the top of the object too |
| the post's height | 40.00 mm | the same |
