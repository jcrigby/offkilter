# A snap-in plug for a mesh finch feeder

The finch feeders' mesh tubes end in a thin plastic ring with three
ears outside; the yellow tray twists onto the ears. The tray collects
rain, the seed that falls into it turns to ick, and it perches birds
that cannot cling, which the mesh never did. This is a plug in its
place. The ring's bore is a hair narrower than the mesh above it, so
its inner edge is a lip inside the tube: the plug is a cup through the
bore with a flange under the ring and three snap tabs that pass the
ring and catch over its top, so the ring is held between flange and
catches. Its floor is level with the ring's top and domed so water runs
to the mesh, and nothing stands outside the tube but the flange, hidden
under the ring. Print it in PETG, press it in from below until the tabs
click.

```sh
cargo build --release -p ok-mcp
python3 examples/finch-plug/build.py
cargo test -p ok-render --test finch_plug      # what CI runs
```

![the plug](out/plug_iso.png)

![in the ring, cut through a tab](out/plug_in_collar_section.png)

## The ring's numbers, to confirm

The ring is modelled from photographs with a rule across it, so the
plug can be checked against it; these are the numbers the plug follows
and what to check them with. Calipers on the real ring, then change
the constants at the top of `build.py` and run it again.

| what | mm in the model | how read | check |
|---|---|---|---|
| bore at the mouth | 48.5 | rule across the opening | inside jaws at the mouth |
| bore at the ring's top edge | 47.5 | the draft the photos show | inside jaws pushed in to the ring's top edge |
| ring thickness | 3 | the flange's edge in the photos | outside jaws on the flange's edge, between ears |
| inside the mesh above the ring | 50 | the mesh stands outside the ring's edge | inside jaws past the ring, opened to the mesh |
| ring outside, ears | 64, three 16 x 3 | rule across the ring and an ear | only drawn, the plug does not touch them |

The two that matter most are the bore at the ring's top edge and the
ring's thickness: the tabs are a hair inside the first and the catches
sit on the second. The mesh's inside diameter only has to clear the
catches, which reach to 48.7.

## The plug

- **Cup.** The skirt runs 10 below the mouth, straight and flush with
  the bore, carries a 2 x 1.5 flange just under the ring, and follows
  the ring's draft through it 0.3 off; the floor 2.4 thick with its top
  level with the ring's top at the edge and rising 1.5 to the centre,
  so water runs out to the first row of mesh, and six 1 mm drain holes
  near the edge, under a nyjer seed's size. The 10 mm below the mouth
  is where the tabs get their length; nothing of it is a ledge, and
  the flange sits under the ring's own flange, a quarter of its reach.
- **Tabs.** Three, 10 wide and 1.2 thick, cut from the skirt by 0.8
  slits from a 2.5 mm root band at the bottom, their faces 0.2 inside
  the ring's top edge, standing 10.7 from the root to the catch. The
  catch reaches 0.6 over the ring's top with 0.15 play, a 0.5 band under
  a 45 degree lead-in. Pushing the plug home bends each tab 0.8 inward,
  1.3 % strain, which PETG takes many times. The ring is then held
  between the flange under it and the catches over it. To take the
  plug out, press the three catches inward through the mesh with a
  small screwdriver; a pull alone will not do it.
- **Nothing to stand on.** Above the mouth the plug is inside the
  tube; below it the skirt is flush with the bore and the flange is
  under the ring.

## What the test checks

`crates/ok-render/tests/finch_plug.rs` regenerates the plug as one
closed body no wider than its flange and well inside the ring's
outline, hanging below the mouth by the skirt only, the flange's top at
the ring's underside; the skirt the clearance off the bore at the
mouth; nothing above the ring's top reaching past its top edge but the
catches, which reach over it by the engagement and sit above it; the
tab strain under 2 %; six drain holes of 1 mm; the floor's edge level
with the ring's top; and the plug home in the ring meeting it nowhere.
