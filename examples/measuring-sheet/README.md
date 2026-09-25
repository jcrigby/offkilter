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

- `out/measuring-sheet-Letter.pdf`, `-A4.pdf`, `-A3.pdf`,
  `-Tabloid.pdf`: the sheets. Print one at 100 % (no "fit to page") and
  check the bar at the bottom against a rule: it is 100 mm. A light
  10 mm grid, heavier every 50, and four bullseye marks in the corners,
  the double-ringed one at the origin, x to the right, y up. The row of
  dots beside the origin mark (one for A4, two for Letter, three A3,
  four A2, five Tabloid) tells the tool which sheet it is looking at.
  The marks and the dots are what the tool uses; the grid is for you.
- `scan.png`: a photograph of the Letter sheet with a plate, a washer, a
  disc and a photo scale on it, taken askew, of a print that came out at
  97 %. Synthetic, so the answer is known: the plate is 64 x 38 mm with
  two 6.5 mm holes 50 mm apart, the washer is 25 mm with a 10 mm bore,
  the disc is 18 mm, the scale's bars are 10 mm.
- `out/measured.png`: what `measure_photo` returns. The picture is
  squared up onto true millimetres at 4 px/mm, the grid drawn over it
  (it drifts from the printed grid, which is 3 % small), the scale boxed
  in orange, each part boxed in red and each hole crossed, so "the left
  hole in part 1" or "the hole at (37, 49)" names a feature without
  ambiguity.

## From a chat

```
measuring_sheet {path: "sheet.pdf"}                                # Letter unless told otherwise
measure_photo {path: "IMG_2231.jpg", reference: "bars 10", sketch: true}
```

The tool answers with the picture, how it read the sheet, and a line
per part:

```
Letter sheet (read from the dots by the origin mark): the four marks found (0.19 mm per photo pixel, fit 0.0 px). ...
Reference: a scale with 10 mm bars at (85, 141) measured 20.62 mm for 20.00, so the sheet was printed at 97.0 %; every size below is corrected by that, and the drawn grid is true millimetres.
Part 1: 64.0 x 38.2 mm, from (30.0, 29.9) to (94.0, 68.1), area 2363 mm2, centroid (62.0, 49.0), outline of 4 points; hole 6.5 mm across at (37.0, 48.9), round; hole 6.5 mm across at (87.0, 48.9), round
Part 2: 25.0 x 25.0 mm, from (157.5, 97.4) to (182.5, 122.4), area 412 mm2, centroid (170.0, 109.9), outline of 18 points; round, 25.0 mm across; hole 10.0 mm across at (170.0, 109.9), round
Part 3: 18.0 x 18.0 mm, from (101.0, 110.9) to (119.0, 128.9), area 254 mm2, centroid (110.0, 119.9), outline of 17 points; round, 18.0 mm across
```

Without `reference` the same picture reads every size 3 % big and says
so: it is trusting the print.

With `sketch: true` the outlines land in a sketch on the current tab:
a round part becomes a circle, anything else a closed run of lines,
and each hole a circle, all in sheet millimetres, ready to extrude to
the thickness the calipers say.

## The print scale, and what to buy

Printers rarely print at exactly 100 %, and the sheet cannot tell:
its own 100 mm bar shrinks with it. Something of known size lying on
the sheet can, and `reference` names it.

- **A forensic photo scale**, `reference: "bars 10"`. The ABFO No. 2
  photomacrographic scale is the standard: a rigid L, 105 mm a side,
  millimetre graduations accurate to 0.1 mm, and alternating 1 cm black
  and white bars along each leg, which are what the tool reads (an even
  run of at least three alike dark blocks; the pitch between them is
  20 mm). About $6 from
  [Tri-Tech Forensics](https://tritechforensics.com/photomacrographic-scales-abfo-no-2/)
  or [Crime Scene](https://shop.crimescene.com/product/no-2-photomacrographic-scale/).
  NIST reviewed these scales twice
  ([2013 and 2016](https://www.nist.gov/publications/dimensional-review-scales-forensic-photography))
  and found every vendor's length graduations within the ±0.1 mm / 1 %
  specification, which is better than this tool resolves. Adhesive
  versions exist for sticking to the sheet: EVIDENT's
  [2" adhesive photo scales](https://www.shopevident.com/category/photographic-scales/2-adhesive-photo-evidence-scales)
  (#5088, $8.25 for 50, accuracy verified against a
  [NIST-traceable standard](https://www.shopevident.com/category/photographic-scales/nistr-traceable-calibration))
  and Arrowhead's
  [certified scale note pads](https://arrowheadforensics.com/adhesive-certified-scale-note-pads.html)
  (A-6786, 15 cm, $9.85 for 50, calibration certificate on request).
  Check the design before relying on one: the tool needs the alternating
  bars, and the listings do not all say whether they carry them. Any
  scale with bars of another length works with that length: `bars 5`.
  Stick the scale flat, in the grid, not on the marks.
- **A coin**, `reference: "quarter"` (or `nickel`, `dime`, `penny`, or
  `disc 22` for any round thing of known diameter, such as a 608
  bearing). US coins are held to a few hundredths of a millimetre, but a
  coin is 1.75 mm thick and its edge shadow adds to its silhouette, so
  expect the scale to come out a few tenths of a percent low. Good
  enough to catch a print at 97 %; buy the scale for better.

The reference is taken out of the parts list and boxed in orange on
the picture. The picture and the sketch are then in true millimetres,
so the drawn grid no longer lands on the printed one.

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
around the sheet, the size dots counted in the origin mark's own frame
(its ring for scale, the x mark for direction), a homography from the
four centres to the marks' known millimetres, the picture resampled
square, dark components on the grid traced and simplified into
outlines, and the paper showing through them taken as holes. With a
reference the print scale is read from it and the picture resampled
again in true millimetres. No dependency beyond a JPEG decoder.
