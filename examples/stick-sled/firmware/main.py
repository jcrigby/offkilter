"""The stick sled's controller: MicroPython on a Raspberry Pi Pico.

It reads passes.csv, a row a cut, each row where the three motors
stand: the lift screw's millimetres down from its home switch, the
swing screw's millimetres out from its, and the spindle's degrees on
from its mark. For a pass it waits until the sled is pulled back and
the foot switch pressed, moves, and waits while the sled is pushed
through and pulled back by hand. Nothing moves while the stick is over
the cutter.

It says what it wants on a small display if one is fitted (an SSD1306,
128 x 64, with MicroPython's ssd1306 driver on the Pico), and always
over USB. It keeps its place in at.txt, so a stick stopped half way
can be taken up again after homing.

The same program runs the two tables that set a built sled up, copied
to the Pico as passes.csv in their turn: bands.csv, which turns two
round bands on a scrap stick to show whether the stick's axis runs
true to the table and the fence, and probe.csv, which brings the
master on the spindle onto a pin in the router and a block on the
table until an electrical contact closes, three times at each place,
and writes where the screws stood to probe_log.csv. calibrate.py makes both tables and
reads the log.

Copy main.py, config.py and passes.csv to the Pico. On a computer,
`python3 main.py --sim passes.csv steps.json [row]` runs the same code
on pins that do nothing and writes where every row left the motors;
with a row, as if taken up again after that row.
"""
try:
    import machine
    import utime
    SIM = False
except ImportError:                 # a computer: simulate
    SIM = True

import config as C


class Pins:
    """The Pico's pins, or nothing at all."""

    def __init__(self):
        self.out = {}
        self.inp = {}

    def output(self, n):
        if not SIM:
            self.out[n] = machine.Pin(n, machine.Pin.OUT)

    def input(self, n):
        if not SIM:
            self.inp[n] = machine.Pin(n, machine.Pin.IN, machine.Pin.PULL_UP)

    def set(self, n, v):
        if not SIM:
            self.out[n].value(1 if v else 0)

    def closed(self, n):
        return True if SIM else self.inp[n].value() == 0

    def home(self, n):
        """A home switch reads home. Wired normally closed, it opens at home, so a broken wire reads home and stops the motor."""
        if SIM:
            return True
        return (self.inp[n].value() == 1) if C.HOME_OPENS else (self.inp[n].value() == 0)

    def touching(self):
        """The probe's contact: the master, clipped to ground, on the pin or block, clipped to the probe's pin."""
        if SIM:
            return CONTACT() if CONTACT else True
        return self.inp[C.PROBE_PIN].value() == 0

    def pause(self, us):
        if not SIM:
            utime.sleep_us(us)


PINS = Pins()
CONTACT = None                      # on a computer: something to stand in for the probe's contact


class Axis:
    def __init__(self, name, pins, per_unit, flip, trim):
        self.name, self.step, self.dir = name, pins[0], pins[1]
        self.per_unit, self.flip, self.trim = per_unit, flip, trim
        self.at = 0                 # steps from home
        self.stepped = 0            # steps taken in all
        PINS.output(self.step)
        PINS.output(self.dir)

    def steps_for(self, units):
        """The table's number as steps, moved by the trim."""
        return int(round((units + self.trim) * self.per_unit))

    def run(self, steps, guard=None, slow=None):
        """Step by `steps`, ramping up and down. `guard` is asked every step; while it says no, wait."""
        if steps == 0:
            return
        up = steps > 0
        PINS.set(self.dir, up != self.flip)
        n = abs(steps)
        for i in range(n):
            while guard is not None and not guard():
                PINS.pause(20000)
            edge = min(i, n - 1 - i)
            if slow is not None:
                us = slow
            elif edge >= C.RAMP_STEPS:
                us = C.STEP_FAST_US
            else:
                us = C.STEP_SLOW_US - (C.STEP_SLOW_US - C.STEP_FAST_US) * edge // C.RAMP_STEPS
            PINS.set(self.step, 1)
            PINS.pause(2)
            PINS.set(self.step, 0)
            PINS.pause(us)
        self.at += steps
        self.stepped += n

    def to(self, units, guard=None, slow=None):
        self.run(self.steps_for(units) - self.at, guard, slow)

    def seek(self, pin, most):
        """Run toward home until its switch says so; that is zero. No further than `most` of its units: a switch that never speaks is a fault."""
        PINS.set(self.dir, self.flip)       # home is the negative way
        left = int(most * self.per_unit)
        while not PINS.home(pin):
            if left <= 0:
                PINS.set(C.ENABLE_PIN, 1)
                raise RuntimeError("no home switch found on the %s axis" % self.name)
            left -= 1
            PINS.set(self.step, 1)
            PINS.pause(2)
            PINS.set(self.step, 0)
            PINS.pause(C.STEP_SLOW_US)
        self.at = 0


class Screen:
    """Up to four short lines: on the display if there is one, and over USB."""

    def __init__(self):
        self.oled = None
        self.quiet = False
        if not SIM and C.OLED:
            try:
                import ssd1306
                i2c = machine.I2C(C.OLED_I2C, sda=machine.Pin(C.OLED_SDA_PIN), scl=machine.Pin(C.OLED_SCL_PIN))
                self.oled = ssd1306.SSD1306_I2C(128, 64, i2c)
            except Exception as why:        # no driver, no display, wrong pins: carry on over USB
                print("no display:", why)

    def show(self, *lines):
        if not self.quiet:
            print(" | ".join(lines))
        if self.oled:
            self.oled.fill(0)
            for i, line in enumerate(lines[:6]):
                self.oled.text(line[:16], 0, i * 10)
            self.oled.show()


SCREEN = Screen()


def wait(pin):
    """Until a switch closes, and for the foot switch until it has been let go first.
    Returns how long it was then held, in milliseconds, up to three seconds."""
    if SIM:
        return 0
    if pin == C.NEXT_PIN:
        while PINS.closed(pin):
            utime.sleep_ms(20)
    while not PINS.closed(pin):
        utime.sleep_ms(20)
    utime.sleep_ms(30)
    if pin != C.NEXT_PIN:
        return 0
    held = 0
    while PINS.closed(pin) and held < 3000:
        utime.sleep_ms(20)
        held += 20
    return held


def wait_for_the_cut():
    """Until the sled has left its back stop and come back to it: the cut has been made."""
    if SIM:
        return
    while PINS.closed(C.SLED_BACK_PIN):
        utime.sleep_ms(20)
    utime.sleep_ms(300)
    while not PINS.closed(C.SLED_BACK_PIN):
        utime.sleep_ms(20)
    utime.sleep_ms(100)


def place(total):
    """The row a stick was left at: the last one finished, if at.txt is of this table."""
    try:
        with open(C.PLACE_FILE) as f:
            n, of = [int(v) for v in f.read().split(",")]
        return n if of == total and 0 < n < total else 0
    except (OSError, ValueError):
        return 0


def keep_place(n, total):
    if not SIM:
        with open(C.PLACE_FILE, "w") as f:
            f.write("%d,%d" % (n, total))


def read_table(path):
    """passes.csv: '#safe,<mm>' then rows of n,kind,tool,lift,swing,turn,via,label.
    via is 'd' for a move that may go straight to its row, 's' for one that must go by the safe height,
    'h' for one that must go by the top of the lift: over a pin or a block."""
    safe, rows = 0.0, []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            if line.startswith("#"):
                parts = line[1:].split(",")
                if parts[0] == "safe":
                    safe = float(parts[1])
                continue
            p = line.split(",")
            if p[0] == "n":
                continue
            rows.append((int(p[0]), p[1], p[2], float(p[3]), float(p[4]), float(p[5]), p[6], p[7] if len(p) > 7 else ""))
    return safe, rows


class Sled:
    def __init__(self, safe):
        self.lift = Axis("lift", C.LIFT_PINS, C.LIFT_STEPS_PER_MM, C.LIFT_FLIP, C.LIFT_TRIM_MM)
        self.swing = Axis("swing", C.SWING_PINS, C.SWING_STEPS_PER_MM, C.SWING_FLIP, C.SWING_TRIM_MM)
        self.turn = Axis("turn", C.TURN_PINS, C.TURN_STEPS_PER_DEG, C.TURN_FLIP, C.TURN_TRIM_DEG)
        self.safe = safe            # the lift at or above which the spindle clears the cutter wherever the frame swings
        self.deepest_crossing = 0   # the lowest the lift ever was while the frame swung or the spindle turned, on a move by the safe height
        self.went_straight = 0
        self.turned = 0             # steps the spindle has turned forwards coming up to its angles
        self.probed = []            # what a probing table found: row, kind, lift, swing, turn
        self.row = 0                # the row being done
        self.touch = 0              # and, probing, which of its touches
        for n in (C.LIFT_HOME_PIN, C.SWING_HOME_PIN, C.TURN_HOME_PIN, C.SLED_BACK_PIN, C.NEXT_PIN, C.PROBE_PIN):
            PINS.input(n)
        PINS.output(C.ENABLE_PIN)
        PINS.output(C.LED_PIN)
        PINS.set(C.ENABLE_PIN, 0)

    def back(self):
        return PINS.closed(C.SLED_BACK_PIN)

    def home(self):
        SCREEN.show("sled back,", "then foot switch", "to find home")
        wait(C.SLED_BACK_PIN)
        wait(C.NEXT_PIN)
        SCREEN.show("finding home")
        self.lift.seek(C.LIFT_HOME_PIN, C.LIFT_MOST_MM)     # up first: the stick away from the cutter
        self.swing.seek(C.SWING_HOME_PIN, C.SWING_MOST_MM)
        self.turn.seek(C.TURN_HOME_PIN, 400.0)

    def turn_on_to(self, degrees, guard, slow=None):
        """The shorter way round, but always arriving forwards, so the belt is pulled from the same side:
        a move back goes a little past and comes up to the angle."""
        per_turn = int(round(360.0 * self.turn.per_unit))
        ahead = (self.turn.steps_for(degrees) - self.turn.at) % per_turn
        if ahead > per_turn // 2:
            past = int(round(C.TURN_PAST_DEG * self.turn.per_unit))
            self.turn.run(ahead - per_turn - past, guard, slow)
            ahead = past
        self.turned += abs(ahead)
        self.turn.run(ahead, guard, slow)

    def go(self, lift, swing, turn, via="s"):
        """To a pass, the sled back: across and round, then to its height. Unless the table says this move
        may go straight ('d'), up to the safe height first if below it: the master is over the cutter.
        'h' is by the top of the lift, over anything standing on the table."""
        guard = self.back
        if via == "d":
            self.went_straight += 1
        elif via == "h":
            self.lift.run(-self.lift.at, guard)
        else:
            safe_steps = self.lift.steps_for(self.safe)
            if self.lift.at > safe_steps:
                self.lift.run(safe_steps - self.lift.at, guard)
            self.deepest_crossing = max(self.deepest_crossing, self.lift.at)
        self.swing.to(swing, guard)
        self.turn_on_to(turn, guard)
        self.lift.to(lift, guard)

    def probe(self, axis, expected, way, window):
        """One screw brought slowly on, from a window short of where the drawn sled would touch to a window past
        it, until the contact closes; then backed off a little and brought on again, for PROBE_TOUCHES touches.
        Where it stood at them, in the table's units: the mean, and how far apart they were. None if it never closed."""
        start = max(0.0, expected - way * window)
        axis.to(start, self.back)
        if PINS.touching():
            raise RuntimeError("touching before the probe began")
        last = axis.steps_for(expected + way * window)
        step = 1 if last > axis.at else -1
        found = []
        for k in range(C.PROBE_TOUCHES):
            self.touch = k
            while axis.at != last and not PINS.touching():
                axis.run(step, self.back, C.PROBE_US)
            if not PINS.touching():
                break
            found.append(axis.at)
            axis.run(-step * int(C.PROBE_BACK_MM * axis.per_unit), self.back)
        axis.to(start, self.back)
        if len(found) < C.PROBE_TOUCHES:
            return None
        return sum(found) / len(found) / axis.per_unit, (max(found) - min(found)) / axis.per_unit

    def run(self, rows, record=None, after=None):
        """The whole table, or what is left of it after row `after`."""
        total = len(rows)
        done = place(total) if after is None else after
        if done and after is None:
            SCREEN.show("left at row %d" % done, "of %d" % total, "press: go on", "hold 3 s: restart")
            if wait(C.NEXT_PIN) >= 3000:
                done = 0
        self.home()
        from_home = True                    # the first move is from home, whatever the table took it to be from
        for n, kind, tool, lift, swing, turn, via, label in rows:
            if n <= done:
                continue
            self.row = n
            head = "%d/%d %s" % (n, total, kind)
            if kind in ("measure", "say"):  # nothing moves: something to do or to read off, then the foot switch
                SCREEN.show(head, label[:16], label[16:32], label[32:48], label[48:64], "then foot switch")
                wait(C.NEXT_PIN)
            elif kind == "leads":           # the probe's two clips, on: the master bridged to the thing it will touch must read, and not once the bridge is gone
                SCREEN.show(head, "bridge master", "and %s with" % label[:12], "something metal")
                while not SIM and not PINS.touching():
                    utime.sleep_ms(20)
                SCREEN.show(head, "good. take it", "away, then foot", "switch")
                wait(C.NEXT_PIN)
                if not SIM and PINS.touching():
                    raise RuntimeError("still touching")
            elif kind == "round":           # a band turned under the cutter, the sled at a stop: only while the foot switch is held
                SCREEN.show(head, label[:16], label[16:32], "hold foot switch")
                while not SIM and not PINS.closed(C.NEXT_PIN):
                    utime.sleep_ms(20)
                self.turn.run(int(round(360.0 * self.turn.per_unit)), lambda: PINS.closed(C.NEXT_PIN), C.ROUND_US)
                SCREEN.show(head, "round. let go")
                while not SIM and PINS.closed(C.NEXT_PIN):
                    utime.sleep_ms(20)
            elif kind in ("br+", "br-"):    # the master put where a block's face should be: the block is slid up to it until the lamp lights
                SCREEN.show(head, "block off the", "table. sled back", "then foot switch")
                wait(C.SLED_BACK_PIN)
                wait(C.NEXT_PIN)
                way = 1.0 if kind == "br+" else -1.0
                short = max(0.0, swing - way * C.PROBE_SWING_MM)
                self.go(lift, short, turn, "h")
                self.swing.to(swing, self.back)
                SCREEN.show(head, label[:16], label[16:32], label[32:48], "to the master:", "lamp, then foot")
                while not SIM:
                    PINS.set(C.LED_PIN, PINS.touching())
                    if PINS.closed(C.NEXT_PIN) and PINS.touching():
                        break
                    utime.sleep_ms(20)
                PINS.set(C.LED_PIN, 0)
                self.swing.to(short, self.back)     # away from it before anything is lifted
                from_home = False
            elif kind in ("down", "sw+", "sw-"):
                SCREEN.show(head, label[:16], "sled back,", "then foot switch")
                wait(C.SLED_BACK_PIN)
                if from_home:
                    wait(C.NEXT_PIN)        # the first probe waits to be told; the rest follow on by themselves
                SCREEN.show(head, "probing")
                if kind == "down":
                    self.go(max(0.0, lift - C.PROBE_LIFT_MM), swing, turn, via)
                    at = self.probe(self.lift, lift, 1.0, C.PROBE_LIFT_MM)
                    found = None if at is None else (at[0], self.swing.at / self.swing.per_unit, at[1])
                else:
                    way = 1.0 if kind == "sw+" else -1.0
                    self.go(lift, max(0.0, swing - way * C.PROBE_SWING_MM), turn, via)
                    at = self.probe(self.swing, swing, way, C.PROBE_SWING_MM)
                    found = None if at is None else (self.lift.at / self.lift.per_unit, at[0], at[1])
                from_home = False
                if found is None:
                    raise RuntimeError("no touch at row %d: check the clips" % n)
                if found[2] > C.PROBE_AGREE_MM:
                    raise RuntimeError("row %d: touches %d um apart" % (n, int(1000 * found[2])))
                self.probed.append([n, kind, found[0], found[1], turn, found[2]])
                if not SIM:
                    with open(C.PROBE_FILE, "a") as f:
                        f.write("%d,%s,%.5f,%.5f,%.4f,%.5f\n" % (n, kind, found[0], found[1], turn, found[2]))
                if not SCREEN.quiet:
                    print("probe,%d,%s,%.5f,%.5f,%.4f,%.5f" % (n, kind, found[0], found[1], turn, found[2]))
            else:
                SCREEN.show(head, tool, "sled back,", "then foot switch")
                wait(C.SLED_BACK_PIN)
                wait(C.NEXT_PIN)
                SCREEN.show(head, "moving")
                self.go(lift, swing, turn, "s" if from_home and via == "d" else via)
                from_home = False
                PINS.set(C.LED_PIN, 1)
                if kind == "set":
                    SCREEN.show(head, "raise the bit", "to the master,", "then foot switch")
                    wait(C.NEXT_PIN)
                elif kind == "place":       # brought to a height for turning bands: the rows after it say what to do
                    pass
                else:
                    SCREEN.show(head, label[:16], "push through", "and pull back")
                    wait_for_the_cut()
                PINS.set(C.LED_PIN, 0)
            keep_place(n, total)
            if record is not None:
                record.append([n, self.lift.at, self.swing.at, self.turn.at % int(round(360.0 * self.turn.per_unit))])
        keep_place(0, total)
        SCREEN.show("done")


def main(path="passes.csv", out=None, after=None):
    safe, rows = read_table(path)
    sled = Sled(safe)
    record = [] if out else None
    try:
        sled.run(rows, record, after)
    except RuntimeError as why:
        SCREEN.show("stopped:", str(why)[:16], str(why)[16:32])
        raise
    if out:
        import json
        with open(out, "w") as f:
            json.dump({"rows": record, "after": after or 0, "deepest_crossing": sled.deepest_crossing, "turned": sled.turned, "went_straight": sled.went_straight, "probed": sled.probed,
                       "stepped": [sled.lift.stepped, sled.swing.stepped, sled.turn.stepped], "safe_steps": sled.lift.steps_for(safe),
                       "per": [C.LIFT_STEPS_PER_MM, C.SWING_STEPS_PER_MM, C.TURN_STEPS_PER_DEG]}, f)


if SIM:
    import sys
    if len(sys.argv) >= 4 and sys.argv[1] == "--sim":
        SCREEN.quiet = True
        main(sys.argv[2], sys.argv[3], int(sys.argv[4]) if len(sys.argv) > 4 else None)
else:
    main()
