"""A kinematic schematic of the lift: the router and the jack's saddle on
straight guides, the lever on its pivot, the link between, drawn from
build.py's own geometry at the bottom, middle and top of the travel,
with two insets magnifying the drift at each end of the lever and what
takes it up. Writes out/schematic.svg.

    python3 examples/jack-table/schematic.py
"""
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.dirname(HERE))
import build as B  # noqa: E402

W, H = 1200, 800
INK, MUTED, GRID = "#1f2328", "#6e7781", "#d0d7de"
POSES = [(0.0, "#2f81f7", "retracted"), (B.PLUNGE / 2, INK, "mid"), (B.PLUNGE, "#cf222e", "full plunge")]
FONT = "font-family='Helvetica, Arial, sans-serif'"


def drift():
    """What the build's geometry gives: how far each pin comes in toward
    the pivot at the ends of the travel, and how far the link leans."""
    tilt = max(abs(B.lever_angle(0.0)), abs(B.lever_angle(B.PLUNGE)))
    lean = max(abs(math.degrees(B.link_angle(p) + math.pi / 2)) for p in (0.0, B.PLUNGE))
    return {"tilt": math.degrees(tilt), "r": B.ROUTER_ARM * (1 - math.cos(tilt)),
            "j": B.JACK_ARM * (1 - math.cos(tilt)), "lean": lean, "link": B.LINK_L}


def pose(p):
    a = B.lever_angle(p)
    piv, r, j = B.lever_points(math.sin(a))
    return {"piv": piv, "r": r, "j": j, "t": B.tab_point(p), "face": j[1] + B.PIN_R, "a": a}


class View:
    """World (x, z) millimetres to SVG pixels: a window onto the side view."""

    def __init__(self, x0, z0, scale, ox, oy):
        self.x0, self.z0, self.s, self.ox, self.oy = x0, z0, scale, ox, oy

    def __call__(self, x, z):
        return self.ox + (x - self.x0) * self.s, self.oy - (z - self.z0) * self.s


def line(v, a, b, color=INK, w=2.0, dash=None, cap="round"):
    (x1, y1), (x2, y2) = v(*a), v(*b)
    d = f" stroke-dasharray='{dash}'" if dash else ""
    return f"<line x1='{x1:.1f}' y1='{y1:.1f}' x2='{x2:.1f}' y2='{y2:.1f}' stroke='{color}' stroke-width='{w}' stroke-linecap='{cap}'{d}/>"


def pin(v, c, r=5.0, color=INK, fill="#ffffff"):
    x, y = v(*c)
    return f"<circle cx='{x:.1f}' cy='{y:.1f}' r='{r}' fill='{fill}' stroke='{color}' stroke-width='2'/>"


def text(x, y, s, size=13, color=INK, anchor="start", weight="normal"):
    return f"<text x='{x:.1f}' y='{y:.1f}' {FONT} font-size='{size}' fill='{color}' text-anchor='{anchor}' font-weight='{weight}'>{s}</text>"


def arc(v, centre, r, a0, a1, color=MUTED, w=1.5, dash="5 4"):
    pts = [v(centre[0] + r * math.cos(a), centre[1] + r * math.sin(a)) for a in
           [a0 + (a1 - a0) * k / 40 for k in range(41)]]
    d = "M" + " L".join(f"{x:.1f},{y:.1f}" for x, y in pts)
    return f"<path d='{d}' fill='none' stroke='{color}' stroke-width='{w}' stroke-dasharray='{dash}'/>"


def ground(v, a, b, below=True):
    """A fixed surface: a line with hatching on its fixed side."""
    out = [line(v, a, b, INK, 2.5)]
    (x1, y1), (x2, y2) = v(*a), v(*b)
    n = int(abs(x2 - x1) / 10) or int(abs(y2 - y1) / 10)
    for k in range(n + 1):
        x = x1 + (x2 - x1) * k / max(n, 1)
        y = y1 + (y2 - y1) * k / max(n, 1)
        dy = 8 if below else -8
        out.append(f"<line x1='{x:.1f}' y1='{y:.1f}' x2='{x - 8:.1f}' y2='{y + dy:.1f}' stroke='{MUTED}' stroke-width='1.2'/>")
    return "".join(out)


def guide(v, x, z0, z1, gap=7.0):
    """A straight guide: two rails either side of a vertical path."""
    (xa, ya), (_, yb) = v(x, z0), v(x, z1)
    return (f"<line x1='{xa - gap:.1f}' y1='{ya:.1f}' x2='{xa - gap:.1f}' y2='{yb:.1f}' stroke='{MUTED}' stroke-width='2'/>"
            f"<line x1='{xa + gap:.1f}' y1='{ya:.1f}' x2='{xa + gap:.1f}' y2='{yb:.1f}' stroke='{MUTED}' stroke-width='2'/>")


def main_view(out):
    ps = [pose(p) for p, _, _ in POSES]
    v = View(B.JACK_X - 40, 40.0, 1.1, 50, 450)
    mid = ps[1]
    piv = mid["piv"]
    # Fixed: the table's underside, the hanger to the pivot.
    out.append(ground(v, (B.JACK_X - 40, B.TOP_U), (60, B.TOP_U), below=False))
    out.append(text(*v(B.JACK_X - 35, B.TOP_U + 8), "table underside (fixed)", 12, MUTED))
    out.append(line(v, (piv[0], B.TOP_U), (piv[0], piv[1] + 8), MUTED, 6))
    out.append(text(v(piv[0], 0)[0] + 10, v(0, B.TOP_U - 40)[1], "hanger", 12, MUTED))
    # Straight paths: the router (its tab bolt) and the saddle, on rails.
    tx = mid["t"][0]
    out.append(guide(v, tx, ps[0]["t"][1] - 15, ps[2]["t"][1] + 25))
    gx, gy = v(tx, ps[2]["t"][1] + 25)
    out.append(text(gx + 14, gy + 8, "router on its posts", 12, MUTED))
    out.append(text(gx + 14, gy + 23, "goes straight", 12, MUTED))
    jx = B.JACK_X
    out.append(guide(v, jx, ps[2]["face"] - 15, ps[0]["face"] + 50, 22))
    gx, gy = v(jx, ps[0]["face"] + 50)
    out.append(text(gx + 28, gy + 8, "jack saddle", 12, MUTED))
    out.append(text(gx + 28, gy + 23, "goes straight", 12, MUTED))
    # Arcs the lever's pins swing on.
    a0, a1 = ps[0]["a"], ps[2]["a"]
    out.append(arc(v, piv, B.ROUTER_ARM, a0 - 0.08, a1 + 0.08))
    out.append(arc(v, piv, B.JACK_ARM, math.pi - a0 + 0.05, math.pi - a1 - 0.05))
    # The three poses: lever, link, router tab, saddle face.
    for (p, color, label), q in zip(POSES, ps):
        w = 3.0 if label == "mid" else 2.0
        op = "" if label == "mid" else " opacity='0.8'"
        out.append(f"<g{op}>")
        out.append(line(v, q["j"], q["r"], color, w + 2))
        out.append(line(v, q["t"], q["r"], color, w))
        x, y = v(*q["t"])
        out.append(f"<rect x='{x - 11:.1f}' y='{y - 20:.1f}' width='22' height='14' fill='{color}' fill-opacity='0.15' stroke='{color}' stroke-width='1.5'/>")
        fx0, fy = v(jx - 16, q["face"])
        fx1, _ = v(jx + 16, q["face"])
        out.append(f"<rect x='{fx0:.1f}' y='{fy - 14:.1f}' width='{fx1 - fx0:.1f}' height='14' fill='{color}' fill-opacity='0.15' stroke='{color}' stroke-width='1.5'/>")
        out.append(pin(v, q["t"], 4, color))
        out.append(pin(v, q["r"], 4, color))
        out.append(pin(v, q["j"], 4, color))
        out.append("</g>")
    out.append(pin(v, piv, 6, INK, "#ffffff"))
    out.append(ground(v, (piv[0] - 14, piv[1] - 9), (piv[0] + 14, piv[1] - 9)))
    # Labels.
    lx, ly = v(*ps[2]["j"])
    out.append(text(lx - 8, ly + 30, "jack pin under the saddle's flat lips:", 13, INK, "start", "bold"))
    d = drift()
    out.append(text(lx - 8, ly + 46, f"slides {d['j']:.1f} mm — a Scotch yoke", 12, MUTED))
    rx, ry = v(*mid["r"])
    out.append(text(rx + 18, ry + 44, "link, pinned both ends:", 13, INK, "start", "bold"))
    out.append(text(rx + 18, ry + 60, f"leans {d['lean']:.1f}° — a slider-crank", 12, MUTED))
    px, py = v(*piv)
    out.append(text(px, py + 32, "pivot", 13, INK, "middle", "bold"))
    out.append(text(px, py + 48, f"lever 2 : 1 ({B.JACK_ARM:.0f} : {B.ROUTER_ARM:.0f} mm), tilts ±{math.degrees(a1):.0f}°", 12, MUTED, "middle"))
    out.append(text(50, 540, "Dashed arcs: where the lever's pins swing. Rails: where the router and the saddle must go.", 12, MUTED))
    out.append(text(50, 556, "Where an arc meets a rail, something has to give: the link leans, the pin slides.", 12, MUTED))


class Stretch:
    """Sideways magnified, height nearly true: drift shows, poses fit."""

    def __init__(self, cx, cz, kx, kz, ox, oy):
        self.cx, self.cz, self.kx, self.kz, self.ox, self.oy = cx, cz, kx, kz, ox, oy

    def __call__(self, x, z):
        return self.ox + (x - self.cx) * self.kx, self.oy - (z - self.cz) * self.kz


def panel(out, ox, oy, title):
    out.append(f"<rect x='{ox}' y='{oy}' width='480' height='330' rx='10' fill='#f6f8fa' stroke='{GRID}'/>")
    out.append(text(ox + 16, oy + 26, title, 15, INK, "start", "bold"))


def inset_router(out, ox, oy):
    """The router end: the tab bolt's straight path, the lever pin's arc,
    the link leaning to bridge them."""
    ps = [pose(p) for p, _, _ in POSES]
    panel(out, ox, oy, "Router end, sideways ×9: the link leans")
    mid = ps[1]
    v = Stretch(mid["r"][0], mid["r"][1], 9.0, 1.0, ox + 300, oy + 230)
    piv = mid["piv"]
    out.append(arc(v, piv, B.ROUTER_ARM, ps[0]["a"] - 0.02, ps[2]["a"] + 0.02, MUTED, 1.5))
    out.append(line(v, (mid["t"][0], ps[0]["t"][1] - 8), (mid["t"][0], ps[2]["t"][1] + 8), MUTED, 1.5, "3 3"))
    for (p, color, label), q in zip(POSES, ps):
        out.append(line(v, q["t"], q["r"], color, 3))
        out.append(pin(v, q["t"], 4, color))
        out.append(pin(v, q["r"], 4, color))
    xa, ya = v(*ps[0]["r"])
    xb, _ = v(mid["r"][0], ps[0]["r"][1])
    out.append(f"<line x1='{xa:.1f}' y1='{ya + 16:.1f}' x2='{xb:.1f}' y2='{ya + 16:.1f}' stroke='{INK}' stroke-width='1.2' marker-start='url(#arr)' marker-end='url(#arr)'/>")
    d = drift()
    out.append(text(xb + 10, ya + 21, f"{d['r']:.1f} mm", 13, INK, "start", "bold"))
    tx, ty = v(*ps[2]["t"])
    out.append(text(tx + 12, ty + 4, "router's tab (straight, dotted)", 11, MUTED))
    rx, ry = v(*ps[1]["r"])
    out.append(text(rx + 12, ry + 4, "lever pin (on its arc, dashed)", 11, MUTED))
    out.append(text(ox + 16, oy + 300, f"At both ends of the travel the pin comes {d['r']:.1f} mm toward the pivot.", 12, MUTED))
    out.append(text(ox + 16, oy + 316, f"The link, {d['link']:.0f} mm long, leans {d['lean']:.1f}° to follow it; the router stays on its posts.", 12, MUTED))


def inset_jack(out, ox, oy):
    """The jack end: the pin's arc under the saddle's flat, sliding along it."""
    ps = [pose(p) for p, _, _ in POSES]
    panel(out, ox, oy, "Jack end, sideways ×9: the pin slides on the flat")
    mid = ps[1]
    v = Stretch(mid["j"][0] + 6, mid["j"][1], 9.0, 1.6, ox + 240, oy + 170)
    piv = mid["piv"]
    out.append(arc(v, piv, B.JACK_ARM, math.pi - ps[0]["a"] + 0.01, math.pi - ps[2]["a"] - 0.01, MUTED, 1.5))
    out.append(line(v, (B.JACK_X, ps[2]["face"] - 10), (B.JACK_X, ps[0]["face"] + 10), MUTED, 1.5, "3 3"))
    for (p, color, label), q in zip(POSES, ps):
        x0, y = v(B.JACK_X - 16, q["face"])
        x1, _ = v(B.JACK_X + 16, q["face"])
        out.append(f"<rect x='{x0:.1f}' y='{y - 10:.1f}' width='{x1 - x0:.1f}' height='10' fill='{color}' fill-opacity='0.18' stroke='{color}' stroke-width='1.5'/>")
        out.append(pin(v, q["j"], B.PIN_R * 1.6, color))
        out.append(text(x1 + 8, y - 1, label, 11, color))
    xa, ya = v(*ps[0]["j"])
    xb, _ = v(mid["j"][0], 0)
    out.append(f"<line x1='{xb:.1f}' y1='{ya - 22:.1f}' x2='{xa:.1f}' y2='{ya - 22:.1f}' stroke='{INK}' stroke-width='1.2' marker-start='url(#arr)' marker-end='url(#arr)'/>")
    d = drift()
    out.append(text((xa + xb) / 2, ya - 28, f"{d['j']:.1f} mm", 13, INK, "middle", "bold"))
    out.append(text(ox + 16, oy + 284, "The saddle (its flat lips, shaded) goes straight down the dotted line; the pin", 12, MUTED))
    out.append(text(ox + 16, oy + 300, f"swings on its arc and slides {d['j']:.1f} mm along the lips. The lips are square to the", 12, MUTED))
    out.append(text(ox + 16, oy + 316, f"motion, so the saddle's height is exactly {B.JACK_ARM:.0f} mm × sin(tilt): the 2 : 1 holds.", 12, MUTED))


def main():
    out = [f"<svg xmlns='http://www.w3.org/2000/svg' width='{W}' height='{H}' viewBox='0 0 {W} {H}'>",
           "<defs><marker id='arr' viewBox='0 0 10 10' refX='9' refY='5' markerWidth='7' markerHeight='7' orient='auto-start-reverse'>"
           f"<path d='M0,0 L10,5 L0,10 z' fill='{INK}'/></marker></defs>",
           f"<rect width='{W}' height='{H}' fill='#ffffff'/>",
           text(40, 36, "Router table lift: the mechanism, side view", 20, INK, "start", "bold"),
           text(40, 58, "Drawn from build.py's geometry. Three poses: ", 13, MUTED)]
    x = 40 + 282
    for p, color, label in POSES:
        out.append(text(x, 58, f"{label} ({p:g} mm)", 13, color, "start", "bold"))
        x += 150
    main_view(out)
    inset_router(out, 690, 80)
    inset_jack(out, 690, 430)
    # Key to the symbols.
    kx, ky = 50, 600
    out.append(text(kx, ky, "Key", 13, INK, "start", "bold"))
    out.append(f"<circle cx='{kx + 10}' cy='{ky + 22}' r='5' fill='#ffffff' stroke='{INK}' stroke-width='2'/>")
    out.append(text(kx + 28, ky + 27, "pin joint: the parts either side turn about it", 12, MUTED))
    out.append(f"<line x1='{kx + 4}' y1='{ky + 40}' x2='{kx + 4}' y2='{ky + 62}' stroke='{MUTED}' stroke-width='2'/>"
               f"<line x1='{kx + 16}' y1='{ky + 40}' x2='{kx + 16}' y2='{ky + 62}' stroke='{MUTED}' stroke-width='2'/>")
    out.append(text(kx + 28, ky + 56, "rails: a straight guide (the router's posts; the jack's own scissor)", 12, MUTED))
    out.append(f"<line x1='{kx}' y1='{ky + 80}' x2='{kx + 20}' y2='{ky + 80}' stroke='{INK}' stroke-width='2.5'/>")
    for k in range(3):
        out.append(f"<line x1='{kx + 4 + 7 * k}' y1='{ky + 80}' x2='{kx - 2 + 7 * k}' y2='{ky + 87}' stroke='{MUTED}' stroke-width='1.2'/>")
    out.append(text(kx + 28, ky + 85, "fixed to the table", 12, MUTED))
    out.append(f"<path d='M{kx},{ky + 106} q10,-8 20,0' fill='none' stroke='{MUTED}' stroke-width='1.5' stroke-dasharray='5 4'/>")
    out.append(text(kx + 28, ky + 110, "arc a pin swings on", 12, MUTED))
    out.append(text(40, H - 24, "Crank → jack screw → saddle (straight) → lever (arc, 2 : 1) → link → router (straight).", 13, INK, "start", "bold"))
    out.append("</svg>")
    path = os.path.join(HERE, "out", "schematic.svg")
    with open(path, "w") as f:
        f.write("\n".join(out))
    print(path)


if __name__ == "__main__":
    main()
