#!/usr/bin/env python3
"""Setting the stick sled up: where the bit's height is against the stick's axis, and whether the stick runs true.

The positioner's two screws are straight, so there is little to find.
Depth needs nothing: a bit is set by touching the master every time
one goes in. Height needs its middle: the reading at which the bit's
axis is level with the stick's. A ground pin goes in the collet with
the probe's clip on it, and the controller brings it against the
master on the spindle from the front, from below and from above, and
again with the spindle half a turn round: six places, three touches
each. From them:

  - the height at which the pin is level with the stick's axis: the
    middle of where it met the master from below and from above;
  - the height screw's true travel: those two are the master and the
    pin apart, whatever the screw says;
  - the master's runout on the spindle: what half a turn changes.

    python3 calibrate.py holds check_log.csv [--kg 2.0] [--tail BEFORE AFTER]
        what the precheck found -> whether the belt holds the stick as stiffly as the bites were sized for,
        and whether anything slipped. Before anything else is set up, and before any real stick.
    python3 calibrate.py fit probe_log.csv [--master 15.98] [--pin 6.35]
        the log -> built.json. Then run build.py: it plans the table for that.
    python3 calibrate.py align FRONT_NEAR FRONT_FAR TOP_NEAR TOP_FAR [SPAN]
        the four band diameters bands.csv turned -> how far to move the tail centre.
    python3 calibrate.py bands RADIUS
        out/bands.csv again, to another radius.
    python3 calibrate.py study [TRIALS]
        how well this works, on machines built wrong on purpose -> out/calibration.json.

Two things it does not find: the depth screw's true travel, and whether
the two travels are square to each other. The study says what each
costs. build.py writes out/probe.csv and out/bands.csv from the
functions here. Pure Python: nothing to install.
"""
import json
import math
import os
import random
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import plan as P  # noqa: E402

OUT = os.path.join(HERE, "out")
BUILT = os.path.join(HERE, "built.json")

PIN_D = 6.35                                    # the ground pin in the collet, a quarter inch: a dowel pin or a length of drill rod. Say what it measures to `fit`
OVER = 4.0                                      # for the touches from below and above, the pin's end is this far past the stick's axis
TOUCH = 0.01                                    # how well a touch is taken to repeat
TOUCHES = 3                                     # each place is touched this many times
BAND_R = 10.0                                   # the bands turned on a scrap stick to see whether it runs true, in bites of 4
BAND_SPAN = 150.0                               # between the near band and the far one
BAND_AT = (25.0, 15.0)                          # the near band from the stick's tail end: for the two turned with the bit's end, and the two with its side
PRECHECK_KG = 2.0                               # the weight hung from the disc's rim to try what holds the stick: two 1 kg bags in a carrier, say
PRECHECK_DRY = 24                               # how many of the table's own passes the precheck makes with no bit in the collet, besides the four at the ends of the positioner's reach
ROD_HELD = 0.05                                 # a rod's end may be this far, by callipers, from where it was before the jack screw and the passes


def lines(*text):
    """Up to four lines of sixteen for the display, as one label."""
    if any(len(t) > 16 or "," in t for t in text) or len(text) > 4:
        raise ValueError(f"does not fit the display: {text}")
    return "".join(t.ljust(16) for t in text).rstrip()


class Machine:
    """A sled, as drawn or as it really is: its two axes, the master and where its middle is from the spindle's, and
    the pin's radius. `ecc` is in the spindle's own frame: toward the router and up with the spindle at its mark."""

    def __init__(self, axes, master_r, pin_r=PIN_D / 2, ecc=(0.0, 0.0)):
        self.axes, self.master_r, self.pin_r, self.ecc = axes, master_r, pin_r, ecc

    def master(self, turn):
        """The master's middle from the stick's axis: (toward the router, up)."""
        return P.rot(math.radians(turn), self.ecc)

    def gap(self, depth, height, turn):
        """Between the pin, a round bar with a flat end, and the master. Nothing at a touch."""
        tip, across = self.axes.tip(depth, height)
        c = self.master(turn)
        return math.hypot(max(tip - c[0], 0.0), max(abs(across - c[1]) - self.pin_r, 0.0)) - self.master_r

    def ball_touch(self, height, turn, ball_r):
        """The depth at which a ball-nose run out at this height meets the master: how the controller sets a bit."""
        c = self.master(turn)
        lo, hi = 0.0, (self.axes.tip_home - c[0]) / self.axes.scale[0]      # from drawn back, to the ball's tip level with the master's middle
        for _ in range(60):
            mid = (lo + hi) / 2
            tip, across = self.axes.tip(mid, height)
            if math.hypot(tip + ball_r - c[0], across - c[1]) > self.master_r + ball_r:
                lo = mid
            else:
                hi = mid
        return hi


def probe_rows(axes, master_r, windows, travel):
    """The probing table for the sled as drawn: the pin against the master from the front, from below and from above,
    at the spindle's mark and half a turn on."""
    rows = []

    def add(kind, label, cut=None, turn=0.0):
        depth, height = axes.screws(cut[1], cut[0]) if cut else (0.0, 0.0)
        # every one by the quill's home: the pin may stand further out than a bit, and the safe depth is a bit's
        rows.append({"n": len(rows) + 1, "kind": kind, "tool": "pin", "depth": round(depth, 4), "height": round(height, 4), "turn": turn, "via": "h", "along": 0.0, "label": label})

    reach = master_r + PIN_D / 2
    add("say", lines("router OFF. pin", "in the collet as", "far out as a bit", "clip on the pin"))
    add("say", lines("scrap blank on:", "it turns the", "master"))
    add("leads", "pin")
    for turn in (0.0, 180.0):
        add("in", "pin at the front", (0.0, master_r), turn)
        add("up", "pin from below", (-reach, -OVER), turn)
        add("dn", "pin from above", (reach, -OVER), turn)
    add("say", lines("done. park the", "clip. copy", "probe_log.csv", "calibrate.py fit"))
    for r in rows:
        if r["kind"] in ("up", "dn") and not windows[1] + 1.0 <= r["height"] <= travel[1] - windows[1] - 1.0:
            raise SystemExit("the probing does not fit the height's travel")
        if r["kind"] == "in" and not windows[0] + 1.0 <= r["depth"] <= travel[0] - windows[0] - 1.0:
            raise SystemExit("the probing does not fit the depth's travel")
        if r["kind"] in ("up", "dn") and not 1.0 <= r["depth"] <= travel[0] - 1.0:      # the depth stands still while the height feels for these
            raise SystemExit("the probing from below and above does not fit the depth's travel")
    return rows


def band_rows(axes, setting, back_x, radius=BAND_R):
    """The table that turns round bands on a scrap stick, near the head and near the tail: with the bit's end at the
    stick from the front, where a band's radius is the axis's distance from the bit's tip, and with the bit's side
    over the stick, where it is the axis's depth under the bit. The sled's own screw takes the stick to where each
    band goes and the wheel's motor turns it once round there. `back_x` is where the bit is along the stick with
    the sled drawn back."""
    rows, bites, r6 = [], (radius + 8.0, radius + 4.0, radius), P.ROUGH_BIT / 2

    def add(kind, tool, cut=None, label="", at=None):
        depth, height, turn = axes.row(*cut) if cut else (0.0, 0.0, 0.0)
        rows.append({"n": len(rows) + 1, "kind": kind, "tool": tool, "depth": round(depth, 4), "height": round(height, 4), "turn": round(turn, 4), "via": "s",
                     "along": 0.0 if at is None else round(at - back_x, 4), "label": label, "across": cut[1] if cut else 0.0, "tip": cut[2] if cut else 0.0, "t": cut[0] if cut else 0.0})

    add("say", "", label=lines("a scrap stick 30", "square or less", "between centres.", "straight bit"))
    add("set", "straight", setting, "set the straight bit on the master")
    for near, cuts in ((BAND_AT[0], [((0.0, 0.0, tip), f"front {tip:.0f}") for tip in bites]), (BAND_AT[1], [((0.0, tip + r6, -r6), f"top {tip:.0f}") for tip in bites])):
        for cut, name in cuts:
            add("place", "straight", cut, name)
            add("round", "", label=lines(name, "the far band"), at=near + BAND_SPAN)
            add("round", "", label=lines(name, "the near band"), at=near)
    add("say", "", label=lines("router OFF. the", "four bands: run", "calibrate.py", "align"))
    return rows


def precheck_rows(rows):
    """The table that tries a built machine before it cuts anything. A sample of the table's own passes is made with
    no bit in the collet and a scrap blank on: the same moves, the same feed, nothing cut. Then the wheel's mark is
    looked for: after those passes it has to be where it was. Then a weight is hung from the disc's rim and the mark
    looked for again, and again with the weight off: how far the weight turned the stick is how stiffly it is held,
    which until then is an estimate; and it has to come back. `rows` are the table's."""
    cutting = [r for r in rows if r["kind"] in ("rough", "finish")]
    pick = {round(k * (len(cutting) - 1) / (PRECHECK_DRY - 1)) for k in range(PRECHECK_DRY)}
    for key in ("depth", "height"):                 # and the four passes at the ends of the positioner's reach
        pick |= {min(range(len(cutting)), key=lambda i: cutting[i][key]), max(range(len(cutting)), key=lambda i: cutting[i][key])}
    out = []

    def add(kind, label, like=None):
        like = like or {}
        out.append({"n": len(out) + 1, "kind": kind, "tool": "none", "depth": like.get("depth", 0.0), "height": like.get("height", 0.0), "turn": like.get("turn", 0.0), "via": "s",
                    "along": like.get("along", 0.0), "label": label, "of": like.get("n", 0)})

    add("say", lines("PRECHECK. router", "OFF and NO BIT", "in the collet. a", "scrap blank on"))
    add("measure", lines("callipers: tail", "rod's end from", "its block's face", "write it down"))
    for i in sorted(pick):
        add("dry", f"as row {cutting[i]['n']}", cutting[i])
    add("check", lines("after the dry", "passes"))
    add("gauge", lines("nothing hung"))
    add("say", lines(f"hang {PRECHECK_KG:g} kg from a", "cord round the", "disc's rim: the", "positioner side"))
    add("gauge", lines(f"{PRECHECK_KG:g} kg hung"))
    add("say", lines("take the weight", "off"))
    add("gauge", lines("nothing hung"))
    add("measure", lines("callipers again:", "tail rod's end", "from its block.", "write it down"))
    add("say", lines("done. copy", "check_log.csv", "calibrate.py", "holds"))
    return out


def holds_of(looks, kg, hold, disc_r, step_deg, swing):
    """What the precheck's four looks for the wheel's mark say. `looks` are degrees: after the dry passes, with nothing
    hung, with the weight hung, and with nothing hung again. `hold` is the build's own estimate of what holds the
    stick; `disc_r` where the cord pulls; `step_deg` one step of the wheel's motor, which is as fine as a look can tell."""
    dry, bare, hung, back = looks
    torque = kg * 9.81 * disc_r / 1000.0                          # N m
    give = abs(hung - (bare + back) / 2)                          # degrees the weight turned the stick
    stiff = torque / math.radians(max(give, step_deg / 2))        # no stiffer than half a step can show
    softer = hold["stiff"] / stiff                                # how many times softer than the estimate took it to be
    moves = {k: hold["worst"][k][0] * softer for k in ("rough", "face")}
    sure = max(0.0, give - step_deg) / give if give > 0.0 else 0.0       # a look is good to a step: it holds unless it is too soft even with a step taken off what the weight turned it
    return {"torque": torque, "give": give, "stiff": stiff, "softer": softer, "moves": moves, "dry": dry, "returned": abs(back - bare),
            "slipped": abs(dry) > 2 * step_deg + 0.02 or abs(back - bare) > 2 * step_deg + 0.02,
            "holds": moves["rough"] * sure <= hold["limit"][0] + 1e-9 and moves["face"] * sure <= hold["limit"][1] + 1e-9,
            "feed": hold["feed"] * min(1.0, hold["limit"][0] / moves["rough"], hold["limit"][1] / moves["face"])}


def write_table(path, rows, safe):
    with open(path, "w") as f:
        f.write(f"#safe,{round(safe, 4)}\n")
        f.write("n,kind,tool,depth,height,turn,via,along,label\n")
        for r in rows:
            f.write(f"{r['n']},{r['kind']},{r['tool']},{r['depth']},{r['height']},{r['turn']},{r['via']},{r['along']},{r['label']}\n")


# -- the fit: no more than arithmetic ------------------------------------------------

def fit(touches, master_r, pin_r=PIN_D / 2):
    """What six touches say. Each is (kind, depth, height, turn): 'in' the pin's end on the master, 'up' the pin come
    up under it, 'dn' come down on it; at the spindle's mark and half a turn on."""
    at = {(kind, round(turn)): (depth, height) for kind, depth, height, turn in touches}
    need = [(k, t) for t in (0, 180) for k in ("in", "up", "dn")]
    if any(key not in at for key in need):
        raise SystemExit(f"no touch for {[key for key in need if key not in at]}")
    reach = master_r + pin_r
    apart = [at[("dn", t)][1] - at[("up", t)][1] for t in (0, 180)]             # from under the master to over it, by the screw
    scale = 2 * reach / (sum(apart) / 2)                                        # the screw's true travel for one asked
    middle = [(at[("dn", t)][1] + at[("up", t)][1]) / 2 for t in (0, 180)]      # the height at which the pin is level with the master's middle
    level = sum(middle) / 2                                                     # and with the spindle's own axis: half a turn puts the runout the other way
    up = scale * (middle[0] - middle[1]) / 2                                    # the master's middle above the axis, the spindle at its mark
    toward = (at[("in", 180)][0] - at[("in", 0)][0]) / 2                        # and toward the router: the pin met it sooner
    return {"across_home": -scale * level, "scale_z": scale, "ecc": [toward, up], "level": level, "disagree": abs(apart[0] - apart[1]),
            "pin_tip_home": (at[("in", 0)][0] + at[("in", 180)][0]) / 2 + master_r}


# -- how well it works ---------------------------------------------------------------

def mis_built(drawn, rng):
    """A machine built wrong at random: the height's home anywhere in 3 mm, both screws a little off their pitch, the
    travels not quite square, the master not quite on the spindle's centre, and the pin and master not quite their
    sizes."""
    a, m = rng.uniform(0.0, 2 * math.pi), rng.uniform(0.0, 0.05)
    axes = P.Axes(drawn["tip_home"], drawn["across_home"] + rng.uniform(-3.0, 3.0), (1.0 + rng.uniform(-0.002, 0.002), 1.0 + rng.uniform(-0.003, 0.003)), rng.uniform(-0.002, 0.002))
    return Machine(axes, drawn["master_r"] + rng.uniform(-0.01, 0.01), PIN_D / 2 + rng.uniform(-0.003, 0.003), (m * math.cos(a), m * math.sin(a)))


def touch(true, row, windows, noise):
    """Where the controller finds a touch on the machine `true`: the screw run from a window short of the row to a
    window past it."""
    kind = row["kind"]
    way = -1.0 if kind == "dn" else 1.0
    if kind == "in":
        at, want, window = (lambda u: true.gap(u, row["height"], row["turn"]) - noise), row["depth"], windows[0]
    else:
        at, want, window = (lambda u: true.gap(row["depth"], u, row["turn"]) - noise), row["height"], windows[1]
    u0, u1 = max(0.0, want - way * window), want + way * window
    if at(u0) <= 0.0:
        raise ValueError("touching before the probe began")
    prev = u0
    for k in range(1, 65):
        u = u0 + (u1 - u0) * k / 64
        if at(u) <= 0.0:
            lo, hi = prev, u
            for _ in range(30):
                mid = (lo + hi) / 2
                if at(mid) <= 0.0:
                    hi = mid
                else:
                    lo = mid
            return hi
        prev = u
    return None


def touches_of(true, rows, windows, rng, noise=TOUCH):
    out = []
    for row in rows:
        if row["kind"] not in ("in", "up", "dn"):
            continue
        u = touch(true, row, windows, sum(rng.gauss(0.0, noise) for _ in range(TOUCHES)) / TOUCHES)
        if u is None:
            raise ValueError("no touch")
        out.append((row["kind"], u, row["height"], row["turn"]) if row["kind"] == "in" else (row["kind"], row["depth"], u, row["turn"]))
    return out


# What the touches cannot see: how far the stick has really turned. The master is round. A wheel off its turning
# centre by e lets the belt in and out by e once a turn, so the stick is e over the wheel's pitch radius from where the
# motor sent it; the motor's pulley does the same once a turn of its own, and the ratio brings it to the same figure.
# The wheel is on the stick's own spindle, so there is nothing else between the motor and the stick to add to that.
WHEEL_OFF = 0.02                                # the wheel off centre once it has been trued under an indicator: 0.04 on the dial
PULLEY_OFF = 0.03                               # the motor's pulley, as bought, on the motor's shaft
WHEEL_LOOSE = 0.10                              # the wheel as printed and screwed on, not trued


def turning(wheel, pulley, radius, ratio):
    """How far the stick is from where the motor sent it, radians, at spindle angle a: the wheel off centre by
    wheel[0] toward wheel[1], the pulley likewise."""
    return lambda a: (wheel[0] * math.sin(a - wheel[1]) + pulley[0] * math.sin(ratio * a - pulley[1])) / radius


def cut_error(true, planned, ecc, cuts, ball_r, turned=None):
    """The worst a finishing cut is from where it was wanted, on the machine `true` run from a table planned for the
    machine `planned` with the master's runout taken to be `ecc`. The bit is set as the controller sets it: run out,
    level with where the master's middle is taken to be, until it touches; its tip is then taken to be the master's
    radius and that runout from the axis, and every depth is counted from there. `turned`, if given, is how far the
    stick is from where the motor sent it at each angle."""
    set_tip, set_across = planned.master_r + ecc[0], ecc[1]
    _, set_height = planned.axes.screws(set_tip, set_across)
    found = true.ball_touch(set_height, 0.0, ball_r)
    worst = 0.0
    for t, across, tip in cuts:
        depth = found + (set_tip - tip) / planned.axes.scale[0]
        _, height = planned.axes.screws(tip, across)
        tip_real, across_real = true.axes.tip(depth, height)
        q = (-across_real * math.sin(t) + (tip_real + ball_r) * math.cos(t), across_real * math.cos(t) + (tip_real + ball_r) * math.sin(t))
        if turned:
            q = P.rot(turned(-t), q)
        p = (-across * math.sin(t) + (tip + ball_r) * math.cos(t), across * math.cos(t) + (tip + ball_r) * math.sin(t))
        worst = max(worst, math.hypot(q[0] - p[0], q[1] - p[1]))
    return worst


def firmware_touches(true, rows, windows, rng):
    """The probing table run by the controller's own program, on a computer, its contact closed by the machine `true`."""
    sys.path.insert(0, os.path.join(HERE, "firmware"))
    import main as fw
    if (fw.C.PROBE_DEPTH_MM, fw.C.PROBE_HEIGHT_MM) != tuple(windows) or fw.C.PROBE_TOUCHES != TOUCHES:
        raise SystemExit("the probing table was made for other windows than the controller's")
    fw.SCREEN.quiet = True
    table = fw.Table(os.path.join(OUT, "probe.csv"))           # as build.py wrote it, and as the Pico will read it
    if [(r[0], r[1]) for r in table] != [(r["n"], r["kind"]) for r in rows]:
        raise SystemExit("out/probe.csv is not the probing table in out/sled.json: run build.py")
    machine = fw.Sled(table.safe)
    noise = {(row["n"], k): rng.gauss(0.0, TOUCH) for row in rows for k in range(TOUCHES)}

    def contact():
        return true.gap(machine.depth.at / machine.depth.per_unit, machine.height.at / machine.height.per_unit, machine.turn.at / machine.turn.per_unit) <= noise[(machine.row, machine.touch)]
    fw.CONTACT = contact
    machine.run(table, None, 0)
    fw.CONTACT = None
    return [[n, kind, depth, height, turn, apart] for n, kind, depth, height, turn, apart in machine.probed]


def percentiles(v):
    v = sorted(v)
    return [round(v[len(v) // 2], 4), round(v[min(len(v) - 1, int(0.95 * len(v)))], 4), round(v[-1], 4)]


def study(trials):
    numbers = json.load(open(os.path.join(OUT, "sled.json")))
    d = numbers["calibration"]["drawn"]
    rows, windows, ball_r = numbers["calibration"]["rows"], numbers["axes"]["probe"], numbers["bit"] / 2
    cuts = [(r["t"], r["across"], r["tip"]) for r in numbers["rows"] if r["kind"] == "finish" and r["label"].split()[2] == r["label"].split()[4]]
    drawn = Machine(P.Axes(d["tip_home"], d["across_home"]), d["master_r"])
    rng = random.Random(11)
    wheel_t, pinion_t = numbers["turn"]["wheel"][0], numbers["turn"]["wheel"][1]
    wheel_r, ratio = wheel_t / math.pi, wheel_t / pinion_t        # GT2: 2 mm a tooth
    spin = random.Random(23)                                      # its own dice: the machines are the same ones as before
    before, after, square, both, trued, loose = [], [], [], [], [], []
    for k in range(trials):
        true = mis_built(d, rng)
        found = fit(touches_of(true, rows, windows, rng), d["master_r"])
        planned = Machine(P.Axes(d["tip_home"], found["across_home"], (1.0, found["scale_z"])), d["master_r"])
        before.append(cut_error(true, drawn, (0.0, 0.0), cuts, ball_r))
        after.append(cut_error(true, planned, found["ecc"], cuts, ball_r))
        # the same machine and table with the wheel and the motor's pulley off their centres, trued and not
        w, wa, u, ua = spin.uniform(0.0, 1.0), spin.uniform(0.0, 2 * math.pi), spin.uniform(0.0, PULLEY_OFF), spin.uniform(0.0, 2 * math.pi)
        trued.append(cut_error(true, planned, found["ecc"], cuts, ball_r, turning((w * WHEEL_OFF, wa), (u, ua), wheel_r, ratio)))
        loose.append(cut_error(true, planned, found["ecc"], cuts, ball_r, turning((w * WHEEL_LOOSE, wa), (u, ua), wheel_r, ratio)))
        # and what is left to the two things the touches do not find: the same machine with its depth screw true, and
        # then square as well, each probed and planned for afresh
        for out, change in ((square, lambda a: setattr(a, "scale", (1.0, a.scale[1]))), (both, lambda a: setattr(a, "shear", 0.0))):
            change(true.axes)
            again = fit(touches_of(true, rows, windows, rng), d["master_r"])
            out.append(cut_error(true, Machine(P.Axes(d["tip_home"], again["across_home"], (1.0, again["scale_z"])), d["master_r"]), again["ecc"], cuts, ball_r))
    # One machine, the whole way: the controller's own program probing it, the arithmetic, the table for what was found.
    rng = random.Random(4)
    true = mis_built(d, rng)
    log = firmware_touches(true, rows, windows, rng)
    found = fit([(kind, depth, height, turn) for _, kind, depth, height, turn, _ in log], d["master_r"])
    planned = Machine(P.Axes(d["tip_home"], found["across_home"], (1.0, found["scale_z"])), d["master_r"])
    example = {"true": {"across_home": true.axes.across_home, "scale": list(true.axes.scale), "shear": true.axes.shear, "ecc": list(true.ecc), "master_r": true.master_r, "pin_r": true.pin_r},
               "touches": log, "found": found, "error_as_drawn": cut_error(true, drawn, (0.0, 0.0), cuts, ball_r),
               "error": cut_error(true, planned, found["ecc"], cuts, ball_r)}
    out = {"drawn": d, "pin_d": PIN_D, "over": OVER, "touch": TOUCH, "touches": TOUCHES, "windows": windows, "ball_r": ball_r,
           "spread": {"height_home": 3.0, "height_scale": 0.003, "depth_scale": 0.002, "shear": 0.002, "runout": 0.05, "master_r": 0.01, "pin_r": 0.003},
           "study": {"trials": trials, "as_drawn": percentiles(before), "fitted": percentiles(after), "depth_screw_true": percentiles(square), "and_square": percentiles(both),
                     "turning": {"wheel_r": wheel_r, "ratio": ratio, "wheel_off": WHEEL_OFF, "pulley_off": PULLEY_OFF, "wheel_loose": WHEEL_LOOSE,
                                 "trued": percentiles(trued), "loose": percentiles(loose),
                                 "per_hundredth": 0.01 / wheel_r * max(math.hypot(across, tip + ball_r) for _, across, tip in cuts)}},
           "example": example}
    with open(os.path.join(OUT, "calibration.json"), "w") as f:
        json.dump(out, f, indent=1)
    print(f"{trials} machines built wrong. Worst finishing cut, mm (median, 95th, worst): from the drawing's table {percentiles(before)}; after six touches {percentiles(after)}")
    print(f"   of what is left: with the depth screw's travel true {percentiles(square)}; and the two travels square as well {percentiles(both)}")
    print(f"   and with the turning: the wheel trued to {WHEEL_OFF} off centre, the pulley {PULLEY_OFF} {percentiles(trued)}; "
          f"the wheel left {WHEEL_LOOSE} off {percentiles(loose)}; 0.01 mm off the wheel's centre is {out['study']['turning']['per_hundredth']:.4f} at the tile's edge")
    print(f"one machine through the controller's own program: {len(log)} touches; worst cut {example['error_as_drawn']:.3f} from the drawing's table, {example['error']:.3f} planned for")


# -- the owner's commands ------------------------------------------------------------

def fit_log(path, master_d=None, pin_d=None):
    numbers = json.load(open(os.path.join(OUT, "sled.json")))
    d = numbers["calibration"]["drawn"]
    kinds = {r["n"]: r["kind"] for r in numbers["calibration"]["rows"] if r["kind"] in ("in", "up", "dn")}
    found = {}
    for line in open(path):
        p = line.strip().split(",")
        if len(p) >= 5 and p[0].isdigit() and int(p[0]) in kinds:
            found[int(p[0])] = (kinds[int(p[0])], float(p[2]), float(p[3]), float(p[4]))        # a row probed twice: the later
    master_r = (master_d or 2 * d["master_r"]) / 2
    got = fit(list(found.values()), master_r, (pin_d or PIN_D) / 2)
    print(f"the pin is level with the stick's axis at height {got['level']:.3f} (drawn: {-d['across_home']:.1f}); the height's screw travels {got['scale_z']:.4f} mm for one asked; "
          f"the master's middle is {got['ecc'][0]:+.3f} toward the router and {got['ecc'][1]:+.3f} up from the spindle's, at its mark")
    if got["disagree"] > 0.03:
        print(f"THE TWO HALF TURNS DISAGREE by {got['disagree']:.3f} mm about how far apart under and over the master are: a touch on a chip, or a clip that slipped. Probe again.")
    with open(BUILT, "w") as f:
        json.dump({"across_home": got["across_home"], "scale_z": got["scale_z"], "ecc": got["ecc"], "master_r": master_r, "pin_d": pin_d or PIN_D}, f, indent=1)
    print(f"wrote {BUILT}. Now run build.py: it plans the table for this. Delete the file to go back to the drawing.")


def holds(path, kg=None, tail=None):
    numbers = json.load(open(os.path.join(OUT, "sled.json")))
    kinds = {r["n"]: r["kind"] for r in numbers["precheck"]["rows"] if r["kind"] in ("check", "gauge")}
    found = {}
    for line in open(path):
        p = line.strip().split(",")
        if len(p) >= 4 and p[0].isdigit() and int(p[0]) in kinds:
            found[int(p[0])] = float(p[2])                         # a row looked at twice: the later
    if sorted(found) != sorted(kinds):
        raise SystemExit(f"the log has rows {sorted(found)} of the precheck's {sorted(kinds)}: run out/precheck.csv to its end")
    turn, hold = numbers["turn"], numbers["hold"]
    got = holds_of([found[n] for n in sorted(found)], kg or numbers["precheck"]["kg"], hold, turn["disc"][0] / 2, turn["homing"]["step_deg"], numbers["tile"]["swing"])
    bad = False
    print(f"after the dry passes the wheel's mark was {got['dry']:+.3f} deg from where the steps said; with the weight off again it came back to within {got['returned']:.3f}.")
    if got["slipped"]:
        bad = True
        print("   SOMETHING SLIPPED between the motor and the spindle. Look at the pulley's set screws on the motor's flat, the belt's tension, the hub's set screws on the rod's flat, and the four bolts and four screws of the disc and the wheel.")
    print(f"{kg or numbers['precheck']['kg']:g} kg at the disc's rim is {got['torque']:.2f} N m, {got['torque'] / (hold['motor_hold'] * turn['ratio'] / hold['stall_over']):.1f} times the heaviest cut's estimate. It turned the stick {got['give']:.3f} deg: "
          f"held {got['stiff']:.0f} N m a radian, where the bites were sized for {hold['stiff']:.0f}.")
    print(f"   at that the tile's edge moves {got['moves']['rough']:.4f} mm at the heaviest cut (the limit is {hold['limit'][0]}) and {got['moves']['face']:.4f} where a face is left ({hold['limit'][1]}).")
    if got["holds"]:
        inside = got["moves"]["rough"] <= hold["limit"][0] and got["moves"]["face"] <= hold["limit"][1]
        print("   The belt holds the stick as the table needs." if inside else
              "   That is within one step of the wheel's motor of what the table needs, and a step is as fine as a look can tell: the belt holds the stick.")
    else:
        bad = True
        print(f"   SOFTER THAN THE TABLE WAS SIZED FOR. Tension the belt and look at the bracket's foot. If it stays, set FEED_MM_S to {got['feed']:.0f} or less in firmware/config.py, or fit the lock.")
    if tail:
        moved = abs(tail[1] - tail[0])
        print(f"the tail rod's end moved {moved:.2f} mm by callipers: " + ("held." if moved <= ROD_HELD else "ITS SET SCREWS LET IT SLIDE. Grind a small flat or a dimple under one screw of each pillow block, or put a collar on the rod between the two blocks."))
        bad = bad or moved > ROD_HELD
    else:
        print("give the two calliper readings of the tail rod's end with --tail BEFORE AFTER to have them judged.")
    print("DO NOT CUT A REAL STICK YET: fix what is in capitals and run the precheck again." if bad else "Next: out/probe.csv, then out/bands.csv, then one whole stick in scrap, measured.")


def align(front_near, front_far, top_near, top_far, span=BAND_SPAN):
    stick = json.load(open(os.path.join(OUT, "sled.json")))["stick"]
    out = (front_far - front_near) / 2            # from the front, a band's radius is the axis's distance from the bit's tip
    low = (top_far - top_near) / 2                # from above, the axis's depth under the bit
    again = False
    for off, name, ways, cures in ((out, "from the front", ("away from the router", "toward the router"), ("toward the router", "away from the router")),
                                   (low, "from above", ("low", "high"), ("up", "down"))):
        way = 0 if off > 0 else 1
        line = f"{name}: over the {span:.0f} mm between the bands the stick's far end runs {abs(off):.2f} mm {ways[way]}."
        if abs(off) < 0.03:
            print(line + " Within what callipers can tell: leave it.")
        else:
            again = True
            print(line + f" Move the tail centre {abs(off) * stick / span:.2f} mm {cures[way]}.")
    if again:
        print("Then `calibrate.py bands` a millimetre under the last radius, turn them again and measure again.")


def bands(radius):
    numbers = json.load(open(os.path.join(OUT, "sled.json")))
    a = numbers["axes"]
    axes = P.Axes(a["tip_home"], a["across_home"], tuple(a["scale"]), a["shear"])
    write_table(os.path.join(OUT, "bands.csv"), band_rows(axes, tuple(numbers["calibration"]["setting"]), numbers["sled"]["back_x"], radius), a["safe"])
    print(f"wrote out/bands.csv: bands {2 * radius:.0f} across")


if __name__ == "__main__":
    arg = sys.argv[1:]
    opt = lambda name: float(arg[arg.index(name) + 1]) if name in arg else None
    if len(arg) >= 2 and arg[0] == "holds":
        holds(arg[1], opt("--kg"), [float(arg[arg.index("--tail") + 1]), float(arg[arg.index("--tail") + 2])] if "--tail" in arg else None)
    elif len(arg) >= 2 and arg[0] == "fit":
        fit_log(arg[1], opt("--master"), opt("--pin"))
    elif len(arg) >= 5 and arg[0] == "align":
        align(*[float(a) for a in arg[1:6]])
    elif len(arg) >= 2 and arg[0] == "bands":
        bands(float(arg[1]))
    elif arg and arg[0] == "study":
        study(int(arg[1]) if len(arg) > 1 else 300)
    else:
        print(__doc__)
