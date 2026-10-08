"""A dummy PCI Express x4 add-in card, low profile, for mechanical
mounting only: the board outline with its edge fingers, key notch and
bracket holes but no copper, as an STL to print. A second studio fuses
a printable I/O bracket to it. README.md says where every number comes
from (the PCI Express CEM specification's card and bracket figures).

    cargo build --release -p ok-mcp
    python3 examples/pcie-dummy/build.py          # the document, the STLs, a sheet and a picture
    cargo test -p ok-render --test pcie_dummy      # what CI runs

Frame: x along the card from the bracket end (the card's end face at
x = 0), y up from the bottom of the edge fingers (the CEM's card
height datum), z through the thickness from the solder side (z = 0) to
the component side. Millimetres throughout.
"""

import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from okmcp import Mcp, Part  # noqa: E402

OUT = os.path.join(os.path.dirname(__file__), "out")

# -- the card (CEM Figure 6-7, low profile card; 6-1 for standard height)
T = 1.57                    # board thickness
H = 68.90                   # low profile height, finger bottom to top edge (standard height: 111.15)
L = 167.65                  # MD2 length, the low profile maximum (MD1 is 119.91); any shorter length mounts the same
TAB_H = 8.25                # the body's bottom edge above the finger bottom; the fingers stand this far proud
DATUM_A = 57.15             # the key notch centre from the bracket-end face (59.05 from the bracket's outer face)
END_TAB_L, END_TAB_Y = 15.00, 4.85   # the bracket-end foot: this long, its bottom this far above the finger bottom
PCI_BLOCK_W = 3.65          # the block beside the fingers that keeps the card out of a PCI slot (CEM note 2)
HOLE_D, HOLE_X, HOLE_Y, HOLE_SPAN = 3.3, 7.25, 7.25, 53.90   # the two bracket screw holes (CEM: 3.18; 3.3 prints to size)

# -- the edge fingers (KiCad's BUS_PCIexpress_x4 edge, which follows the connector drawings)
PITCH = 1.0
KEY_FROM_PIN1 = 11.5        # the key notch centre, between pin 11 and pin 12
KEY_W, KEY_R = 1.9, 0.95    # the notch, round-topped
PINS = 32                   # per side for x4 (x1: 18, x8: 49, x16: 82)
TAB_X0 = DATUM_A - KEY_FROM_PIN1 - 0.65            # 45.0: the tab starts 0.65 before pin 1
TAB_X1 = DATUM_A - KEY_FROM_PIN1 + PINS + 1.65     # 79.3: pin 12 sits 3 pitches after pin 11, so the tab is pins + 2.3
TAB_CHAMFER = 0.5

# -- the bracket (CEM Figure 6-9, low profile I/O bracket), simplified to print in one piece with the card
PLATE_T = 1.6               # the sheet is 0.86 steel; a printed wall wants this
PLATE_W = 17.56             # 18.42 wide less the ear's 0.86 beyond the solder face, which a flat print cannot have
BOTTOM_TAB_W = 13.44        # 14.30 less the same
TOP_Y = H + 0.5             # the top tab's underside sits just over the card's top edge
CHAMFER_Y = TOP_Y - 71.46   # the 45 degree run-in to the bottom tab starts here
BOTTOM_Y = TOP_Y - 79.20    # the bottom tab's end
TOP_TAB_L = 11.84           # the top tab, out from the plate's outer face
SLOT_W, SLOT_Z, SLOT_IN = 4.4, 8.8, 6.35   # the #6-32 screw slot: width, its centre across the plate, its round end in from the outer face


def card(p):
    """The board: outline, the two screw holes and the key notch."""
    s = p.sketch("top", 0.0, "outline")
    c = TAB_CHAMFER
    p.polygon(s, [(0.0, H), (L, H), (L, TAB_H), (TAB_X1, TAB_H), (TAB_X1, c), (TAB_X1 - c, 0.0), (TAB_X0 + c, 0.0), (TAB_X0, c),
                  (TAB_X0, END_TAB_Y), (TAB_X0 - PCI_BLOCK_W, END_TAB_Y), (TAB_X0 - PCI_BLOCK_W, TAB_H),
                  (END_TAB_L, TAB_H), (END_TAB_L, END_TAB_Y), (0.0, END_TAB_Y)])
    f = p.extrude(s, T, name="board, 1.57")
    p.apply({"type": "rename_part", "source": f, "name": "dummy x4 card, low profile"})
    s = p.sketch("top", T + 1.0, "bracket holes")
    for y in (HOLE_Y, HOLE_Y + HOLE_SPAN):
        p.point(s, (HOLE_X, y))
    p.hole(s, HOLE_D, direction="reverse", name="bracket screw holes, #4-40")
    s = p.sketch("top", T + 1.0, "key")
    p.rect(s, (DATUM_A - KEY_W / 2, -1.0), (DATUM_A + KEY_W / 2, TAB_H - KEY_R))
    p.circle(s, (DATUM_A, TAB_H - KEY_R), KEY_R)
    p.cut(s, T + 2.0, direction="reverse", name="key notch")


def bracket(p):
    """The I/O bracket fused to the card's bracket end: the plate with
    its run-in to the bottom tab, and the top tab with the screw slot."""
    s = p.sketch("right", -PLATE_T, "plate")
    p.polygon(s, [(TOP_Y, 0.0), (TOP_Y, PLATE_W), (CHAMFER_Y, PLATE_W), (CHAMFER_Y - (PLATE_W - BOTTOM_TAB_W), BOTTOM_TAB_W),
                  (BOTTOM_Y, BOTTOM_TAB_W), (BOTTOM_Y, 0.0)])
    p.extrude(s, PLATE_T + 0.5, op="add", name="bracket plate")
    s = p.sketch("front", -(TOP_Y + PLATE_T), "top tab")
    x_out = -PLATE_T - TOP_TAB_L
    p.polygon(s, [(x_out, 0.0), (-PLATE_T + 0.5, 0.0), (-PLATE_T + 0.5, PLATE_W), (x_out, PLATE_W)])
    p.extrude(s, PLATE_T + 0.1, op="add", name="top tab")
    s = p.sketch("front", -(TOP_Y + PLATE_T + 1.0), "screw slot")
    p.rect(s, (x_out - 1.0, SLOT_Z - SLOT_W / 2), (-PLATE_T - SLOT_IN, SLOT_Z + SLOT_W / 2))
    p.circle(s, (-PLATE_T - SLOT_IN, SLOT_Z), SLOT_W / 2)
    p.cut(s, PLATE_T + 2.0, name="screw slot, #6-32")


def main():
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, "pcie_dummy.okpart")
    if os.path.exists(path):
        os.remove(path)
    mcp = Mcp(path)
    mcp.call("create_document", {"name": "pcie_dummy"})
    mcp.call("apply", {"ops": [{"type": "rename_document", "name": "Dummy PCIe x4 card, low profile"},
                               {"type": "rename_tab", "tab": 1, "name": "card"},
                               {"type": "add_part_studio", "name": "card with bracket"}]})
    for tab, build in ((1, (card,)), (2, (card, bracket))):
        p = Part(mcp, tab, str(tab))
        for fn in build:
            fn(p)
        r = p.report()
        for b in r["bodies"]:
            lo, hi = b["bounds"]
            print(f"tab {tab}: {b['name']}: {lo['x']:.2f}..{hi['x']:.2f} x {lo['y']:.2f}..{hi['y']:.2f} x {lo['z']:.2f}..{hi['z']:.2f} mm, {b['volume'] / 1000:.1f} cm3")
        assert len(r["bodies"]) == 1, f"tab {tab}: {len(r['bodies'])} bodies"
    print(mcp.call("export", {"tab": 1, "format": "stl", "path": os.path.join(OUT, "card.stl")}))
    print(mcp.call("export", {"tab": 2, "format": "stl", "path": os.path.join(OUT, "card_with_bracket.stl")}))
    print(mcp.call("export", {"tab": 1, "format": "pdf", "sheet": "A4", "views": ["top", "front"],
                              "note": "PCIe x4 dummy, low profile, 1.57 thick; fingers 8.25 proud, key at 57.15 from the bracket end",
                              "path": os.path.join(OUT, "card.pdf")}))
    mcp.call("screenshot", {"tab": 1, "view": "iso", "width": 1400, "height": 800, "path": os.path.join(OUT, "card.png")})
    mcp.call("screenshot", {"tab": 2, "view": "-0.6,-0.7,0.5", "width": 1400, "height": 800, "path": os.path.join(OUT, "card_with_bracket.png")})


if __name__ == "__main__":
    main()
