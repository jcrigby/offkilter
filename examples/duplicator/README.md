# A carving duplicator on round rail

The Woodsmith carving duplicator (plan SN12918) rebuilt on the 20 mm
round rail already bought for the router lift: SFC20 shafts, SK20
supports and SC20UU blocks in place of the plan's conduit and rollers.
A trim router and a pilot the size of its bit sit side by side on one
tool plate; the pilot traces a pattern on one platform while the bit
cuts the same path in a blank on the other. The first use is the puzzle
top: the printed pattern plate from `examples/puzzle-top` has the gap
lattice as grooves, a 2 mm pilot rides them, and a 2 mm bit cuts every
piece out of one board, the kerf being the resin gap. After that, any
part printed as a master copies into wood the same way.

```sh
cargo build --release -p ok-mcp
python3 examples/duplicator/build.py          # the document, pictures and sheets
cargo test -p ok-render --test duplicator     # what CI runs
```

`out/duplicator.pdf` is the whole machine on an A2 with a parts list;
`base_y.pdf`, `x_axis.pdf`, `z_axis.pdf` and `tool_holder.pdf` are the
four sub-assemblies on A3s at a larger scale, each with its own
balloons and list.

## How it is built

- **Base and Y.** A 900 x 760 ply base. The two Y shafts (600 mm) run
  front to back at x = ±380 on SK20s standing straight on the ply, with
  two blocks 230 apart on each. The two work platforms (330 x 300) sit
  between the rails, the pattern's 3.5 mm lower so the plate's top is
  level with a 12 mm blank.
- **The gantry.** One piece on the four Y blocks: a deck (860 x 280,
  one ply) across both rails, a wall (860 x 270, one ply) standing on
  its back edge, and a triangular gusset in each corner. Two separate
  beds with a post each would rack, one side running ahead of the
  other with only the X shafts' bending to stop it (about 90 N/mm for
  the pair, half a millimetre for a lopsided 50 N push); as an angle
  the deck takes racking as in-plane shear, the wall the fore-and-aft
  bending, and the gussets close each end into a triangle. The deck is
  an H in plan, a 240 mm bite out of its front edge where the Z
  carriage plate hangs to the board top, and it rides 45 mm above the
  blank and clears the Y supports at full travel. One bridge on two
  round rails needs them parallel to within the blocks' clearance: set
  the SK20s by sliding the finished gantry end to end before the last
  tightening.
- **X.** Two X shafts (760 mm) one above the other, 150 apart, on
  SK20s on the wall's front face.
- **Z.** The carriage plate (220 x 314, one ply) rides the X shafts on
  four blocks bolted to its back, no bed and no joint in bending. Two
  Z shafts (250 mm) stand on its face on SK20s, and the tool support
  rides them on four blocks: a support plate with the tool plate hung
  on three webs in front of it.
- **Tools.** The tool plate is two plies (38 mm) with a 66 mm bore for
  the router motor's barrel and a 9.7 mm bore for the pilot chuck's
  3/8 in shank, both drilled in one setup. Each bore has a slit to the
  plate's front edge and a knob bolt across it (M6 and M4), the slits
  to the front so tightening shifts a tool in y, never in the x spacing
  calibration sets. The pilot is 2 mm drill rod in a keyless mini chuck;
  its height is set by sliding the shank in its clamp.
- **Depth.** An M8 stop screw through an ear on the tool support lands
  on a stop block on the carriage plate: set for a 4 mm first pass, it
  backs off 13 turns for the last, where the pilot bottoming in the
  12.5 mm groove is the limit.
- **Calibration.** An L fence on each platform, the blank's 30 mm wide
  with slots across its legs. The pattern plate has a 2 mm reference
  hole 5 mm inside its front-left corner, outside the board's outline:
  pilot in the hole, bit plunged into the blank platform, the mark
  should sit 5 mm outside the blank's corner both ways, and the slotted
  fence moves by the difference.

## The numbers the test checks

| | |
|---|---|
| Travel | X ±277.5, Y ±142.5, Z 62.5 down from the drawn raised pose |
| Gantry | deck on all four Y blocks, wall on the deck full width, deck 45 above the blank, the carriage plate through the notch |
| Reach | bit over the 304 x 260 blank, pilot over the 304 x 260 lattice |
| Depth | tips 40 above the board raised; first pass 4 mm; last pass 12.5 mm, 52.5 of the 62.5 mm of travel |
| Clearance at the last pass | chuck 5.5 mm above the plate; tool plate 36 mm above the blank |
| Shafts | X 760 x 2, Y 600 x 2, Z 250 x 2: the four 1000 mm shafts cut 760 + 240 and 600 + 400, Z from the 400s |

Every bought-part size (SK20, SC20UU, the router barrel, the chuck) is
a catalogue number to measure before cutting.
