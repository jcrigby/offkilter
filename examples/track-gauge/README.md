# Track saw edge gauge, 2 in

A fixed marking gauge for a track saw, printed in one piece: hook it
over the edge of a sheet, slide the track's edge up against the end of
its depth leg, lift it off, cut. The track lands 2 in plus the blade's kerf
from the edge, so the strip the blade leaves between the edge and the
cut is 2 in. Not settable, and that is the point: no measuring, no
pencil line, the same strip every time. Print two and put one at each
end of the track, and the track is parallel to the edge as well.

    cargo build --release -p ok-mcp
    python3 examples/track-gauge/build.py       # the document, the STL, pictures and sheets
    cargo test -p ok-render --test track_gauge  # what CI runs

![the gauge](out/gauge.png)

## Why 2 in plus the kerf

A track saw cuts along the track's splinter-guard edge, with the whole
kerf outside the track. Put the track's edge at 2 in from the sheet
edge and the strip comes out 2 in less a kerf. The depth leg is
therefore `WIDTH + KERF` long from the fence's inner face: 2 in plus
2.2 mm, 53.0 mm, for a Festool or Makita blade. `KERF` at the top of
`build.py` is the number to change for a thin-kerf blade (1.5 or
1.6 mm); `width` and `kerf` are also document variables, and the
depth leg's length is bound to their sum, so the client shows the
same thing.

## What it is

An L in section, extruded 60 mm along the sheet edge, one printed
body, `out/gauge.stl`:

| part | size | what it does |
|------|------|--------------|
| fence, the short leg | 6 thick, 18 tall, hanging 12 below the depth leg | hooks over the sheet edge; its inner face registers on the edge |
| depth, the long leg | 6 thick, 53.0 long from the fence's inner face | lies on the sheet; its end face is what the track's edge butts against |
| dust groove | 2 mm square along the inside corner | so a crumb of sawdust in the corner cannot hold the fence off the edge |
| corners | R4 on the four corners in plan | nothing sharp in the hand or to catch on the track |
| labels | `2"` and `2.2mm`, engraved 0.6 deep in the top face | the strip it cuts and the kerf it allows for, in case a second one is ever made for another blade; seven-segment bars, so the sketch needs no font |

6 mm legs print stiff in three or four perimeters; the track's edge
is 5 mm tall, so the 6 mm end face catches all of it. 60 mm along the
edge is enough bearing on the edge to sit square; print two and use
one at each end of the track.

Printing: PETG or PLA, 0.2 layers, three or four perimeters, any
infill. It prints on its top face, the one flat face, with no
supports: every surface is square to the axes or a vertical round, so
nothing overhangs, and the labels are three layers of recess in the
face on the bed, which prints cleanly. `out/gauge.stl` is already
turned over that way, the flat face at z = 0 and the fence standing
up, so it drops into a slicer as it should print; the model's own
frame is the in-use orientation, flat face up. The faces
that register (the fence's inner face, the depth leg's end and its
underside) are vertical walls and a top surface of that print, away
from the first layer's elephant foot. Check the depth leg's end
against the fence's inner face with calipers: 53.0 mm. If the printer
runs big or small in the plane, scale the print, or set `KERF` to what
the calipers want and print again. Then check the first strip you
cut.

![in use](out/in_use.png)

## The model

- `gauge`: the fence as the short leg, the depth leg added to it,
  extruded from the fence's inner face by `#width + #kerf`, the dust
  groove cut along the inside corner, a fillet on the four corner
  edges (picked from the faces the report lists by their normals),
  and the labels cut 0.6 into the top from one sketch of bars: each
  glyph is a few seven-segment bars with 0.3 between them, so no two
  overlap and the sketch's regions stay simple. One body, PETG,
  exported as the STL.
- `sheet` and `track`: a 24 x 24 corner of 3/4 ply with its top at
  z = 0 and its edge at x = 0, a 7-1/4 x 0.2 track bar with its edge at
  `WIDTH + KERF`, and the kerf as a 2.2 mm body through the sheet from
  `WIDTH` to `WIDTH + KERF`, so the picture shows where the blade goes.
- `in use`: an assembly of those with two gauges hooked on the edge, at
  each end of the track.
- `print`: the gauge turned 180 degrees onto its top face, that face
  at z = 0; the STL is exported from this tab, so it is bed-down.
- Outputs in `out/`: `track_gauge.okpart`, `gauge.stl` (bed-down),
  `gauge_print.png` (as it prints), `gauge.png`,
  `gauge_top.png` (the labels), `gauge_below.png` (the hook and the
  groove), `in_use.png`,
  `gauge.pdf` (front, top, right and iso) and `in_use.pdf` (the gauge
  on the sheet, and a section across it through the track and kerf).
- The test checks the L at its sizes, the depth leg 53.0 mm from the
  fence's inner face, that it is one extrusion (two end faces), the
  four R4 rounds at the corners, the labels' floors 0.6 down and their
  volume, the volume with the groove and corners taken out, every face
  square to the axes or a vertical round (so it prints flat), the
  binary STL, and in use the sheet edge, fence, depth leg's end, track
  edge and kerf lining up so the strip is exactly 2 in.
