# Measuring parts from a photograph

Some parts of a project exist before the model does: a router clamp,
a bearing, a bracket from the scrap bin. Print the measuring sheet, lay
them on it, take a picture from above, and offkilter reads their sizes
off the picture in millimetres. It is good to a fraction of a
millimetre on flat things and it is not a substitute for calipers; it
is a first sketch and a picture to point at when asking for the
measurement that matters.

```sh
cargo build --release -p ok-mcp
python3 examples/measuring-sheet/build.py       # writes the sheets and measures scan.png
cargo test -p ok-photo --test example           # what CI runs; regenerates scan.png and out/measured.png
```

- `out/measuring-sheet-A4.pdf`, `-Letter.pdf`, `-A3.pdf`: the sheets.
  Print one at 100 % (no "fit to page") and check the bar at the bottom
  against a rule: it is 100 mm. A light 10 mm grid, heavier every 50,
  and four bullseye marks in the corners, the double-ringed one at the
  origin, x to the right, y up. The marks are what the tool uses; the
  grid is for you.
- `scan.png`: a photograph of the A4 sheet with a plate, a washer and a
  disc on it, taken askew. Synthetic, so the answer is known: the plate
  is 64 x 38 mm with two 6.5 mm holes 50 mm apart, the washer is 25 mm
  with a 10 mm bore, the disc is 18 mm.
- `out/measured.png`: what `measure_photo` returns. The picture is
  squared up onto the sheet's millimetres at 4 px/mm, the grid drawn
  over it, each part boxed and each hole crossed, so "the left hole in
  part 1" or "the hole at (37, 49)" names a feature without ambiguity.

## From a chat

```
measuring_sheet {path: "sheet.pdf", sheet: "Letter"}
measure_photo {path: "IMG_2231.jpg", sheet: "Letter", sketch: true}
```

The tool answers with the picture and a line per part:

```
Part 1: 64.0 x 38.0 mm, from (30.0, 30.0) to (94.0, 68.0), area 2366 mm2, centroid (62.0, 49.0), outline of 4 points; hole 6.5 mm across at (37.0, 49.0), round; hole 6.5 mm across at (87.0, 49.0), round
Part 2: 25.0 x 25.0 mm, from (157.5, 97.5) to (182.5, 122.5), area 412 mm2, centroid (170.0, 110.0), outline of 19 points; round, 25.0 mm across; hole 10.0 mm across at (170.0, 109.9), round
Part 3: 18.0 x 18.0 mm, from (101.0, 111.0) to (119.0, 129.0), area 254 mm2, centroid (110.0, 119.9), outline of 16 points; round, 18.0 mm across
```

With `sketch: true` the outlines land in a sketch on the current tab:
a round part becomes a circle, anything else a closed run of lines,
and each hole a circle, all in sheet millimetres, ready to extrude to
the thickness the calipers say.

## Taking the picture

- All four marks in the frame, nothing on top of them, the sheet flat.
- From straight above, as far back as the phone's zoom allows. The
  four marks fix the sheet's plane, so a tilted picture is squared up
  correctly, but anything with height is shifted by parallax: the top
  of a 20 mm block photographed from 400 mm away and 100 mm off centre
  reads 5 mm out of place. Thin parts and straight-down pictures are
  what it is good at.
- Dark parts on the white sheet. A bright aluminium part can read as
  paper; a piece of dark card under it fixes that.
- Daylight or even lighting, no shadows sharper than the part.
- JPEG straight from the phone is fine. The threshold is chosen from
  the picture, so exposure does not matter much.

## How it works

`crates/ok-photo`: Otsu threshold on a shrunk copy, connected
components, the marks found as rings with a dot inside them (the
origin's as a ring around such a ring), the four centres ordered
around the sheet, a homography from them to the marks' known
millimetres, the picture resampled square, dark components on the grid
traced and simplified into outlines, and the paper showing through
them taken as holes. No dependency beyond a JPEG decoder.
