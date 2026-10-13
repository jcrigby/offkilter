# Track saw edge gauge, 2 in

A fixed marking gauge for a track saw, printed in one piece: hook it
over the edge of a sheet, slide the track's edge up against the end of
its stem, lift it off, cut. The track lands 2 in plus the blade's kerf
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
edge and the strip comes out 2 in less a kerf. The stem is therefore
`WIDTH + KERF` long from the fence's inner face: 2 in plus 2.2 mm,
53.0 mm, for a Festool or Makita blade. `KERF` at the top of
`build.py` is the number to change for a thin-kerf blade (1.5 or
1.6 mm); `width` and `kerf` are also document variables, and the
stem's length is bound to their sum, so the client shows the same
thing.

## What it is

A T in plan, one printed body, `out/gauge.stl`:

| part | size | what it does |
|------|------|--------------|
| fence, the crossbar | 120 long x 6 thick, 18 tall | runs along the sheet edge; its inner face registers on the edge, and its length keeps the T square to it, so the stem cannot skew |
| stem | 40 wide x 6 thick, 53.0 long from the fence's inner face | lies on the sheet; its end face is what the track's edge butts against |
| hook | the fence hangs 12 below the stem's underside | over the sheet edge; the top is one flat face |
| dust groove | 2 mm square along the stem's inside bottom edge | so a crumb of sawdust in the corner cannot hold the fence off the edge |

6 mm walls print stiff in three or four perimeters; the track's edge
is 5 mm tall, so the stem's 6 mm end face catches all of it. The
fence is long so the gauge cannot rotate in plan against the edge,
which is the only way the 53.0 could read short.

Printing: PETG or PLA, 0.2 layers, three or four perimeters, any
infill. Print it on its top face, the one flat face, with no supports:
every surface is square to the axes, so nothing overhangs. The faces
that register (the fence's inner face, the stem's end, the stem's
underside) are vertical walls and a top surface of that print, away
from the first layer's elephant foot. Check the stem's end against the
fence's inner face with calipers: 53.0 mm. If the printer runs big or
small in the plane, scale the print, or set `KERF` to what the
calipers want and print again. Then check the first strip you cut.

![in use](out/in_use.png)

## The model

- `gauge`: the fence as the crossbar, the stem added to it, extruded
  from the fence's inner face by `#width + #kerf`, and the dust groove
  cut along the inside corner. One body, PETG, exported as the STL.
- `sheet` and `track`: a 24 x 24 corner of 3/4 ply with its top at
  z = 0 and its edge at x = 0, a 7-1/4 x 0.2 track bar with its edge at
  `WIDTH + KERF`, and the kerf as a 2.2 mm body through the sheet from
  `WIDTH` to `WIDTH + KERF`, so the picture shows where the blade goes.
- `in use`: an assembly of those with two gauges hooked on the edge, at
  each end of the track.
- Outputs in `out/`: `track_gauge.okpart`, `gauge.stl`, `gauge.png`,
  `gauge_below.png` (the hook and the groove), `in_use.png`,
  `gauge.pdf` (front, top, right and iso) and `in_use.pdf` (the gauge
  on the sheet, and a section across it through the track and kerf).
- The test checks the T at its sizes, the stem 53.0 mm from the
  fence's inner face, the fence's and stem's widths from its faces,
  the volume with the groove taken out, every face square to the axes
  (so it prints flat), the binary STL, and in use the sheet edge,
  fence, stem end, track edge and kerf lining up so the strip is
  exactly 2 in.
