"""The stick sled's controller: MicroPython on a Raspberry Pi Pico.

It reads passes.csv, a row a cut, each row where the three motors
stand: the positioner's quill run out so many millimetres from its
home (the depth), its stage so many from its (the height), and the
spindle's degrees on from its mark. For a pass it waits until the sled
is against its back stop and the foot switch pressed, moves, and waits
while the sled is pushed past the bit and drawn back by hand. Nothing
moves while the stick is beside the bit.

The stick's motor is not on the sled. It turns a large wheel that
stays at the end of the base, and a tapered star on the sled's spindle
is in that wheel only while the sled is back. So a row at a new angle
goes: sled back, unlock the spindle's clamp, the wheel turns the stick,
lock the clamp, make the pass. Rows at the same angle need neither.
The clamp's switch says which it is, and the wheel is never turned
with the clamp thrown.

A bit is set by touch. The probe's clip goes on the bit, the router
off; the controller runs the bit out until it touches the master on
the spindle, and from then on every depth is taken from where that
was. It will not go to a cut until the clip is parked on its post.

It says what it wants on a small display if one is fitted (an SSD1306,
128 x 64, with MicroPython's ssd1306 driver on the Pico), and always
over USB. It keeps its place in at.txt, so a stick stopped half way
can be taken up again after homing and setting the bit.

The same program runs the two tables that set a built machine up,
copied to the Pico as passes.csv in their turn: probe.csv, which
touches a pin in the collet on the master from in front, above and
below, and writes where the screws stood to probe_log.csv; and
bands.csv, which has round bands turned by hand on a scrap stick to
show whether the stick runs true to the sled's fence. calibrate.py reads both.

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

PROBES = ("in", "up", "dn")         # a touch run out along the bit, brought up from below, brought down from above


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
            return HOME(n) if HOME else True
        return (self.inp[n].value() == 1) if C.HOME_OPENS else (self.inp[n].value() == 0)

    def touching(self):
        """The probe's contact: its clip on a bit or a pin that touches the master, or parked on its post."""
        if SIM:
            return CONTACT() if CONTACT else True
        return self.inp[C.PROBE_PIN].value() == 0

    def pause(self, us):
        if not SIM:
            utime.sleep_us(us)


PINS = Pins()
CONTACT = None                      # on a computer: something to stand in for the probe's contact
HOME = None                         # and for the home switches


class Axis:
    def __init__(self, name, pins, per_unit, flip, trim, past=0.0):
        self.name, self.step, self.dir = name, pins[0], pins[1]
        self.per_unit, self.flip, self.trim = per_unit, flip, trim
        self.past = int(round(past * per_unit))     # how far past a place it goes when coming back to it
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
        """To a place, arriving going away from home: a move back goes a little past and comes up to it."""
        steps = self.steps_for(units) - self.at
        if steps < 0 and self.past and self.at + steps - self.past >= 0:
            self.run(steps - self.past, guard, slow)
            steps = self.past
        self.run(steps, guard, slow)

    def seek(self, pin, most, clear):
        """Run toward home until its switch says so; that is zero. If it says so before a step is taken, first run
        away until it stops saying so and `clear` further: home is then always the same edge of the switch, come to
        from the same side, wherever the axis was left. No further than `most` of its units either way: a switch
        that never changes is a fault."""
        def step(left):
            if left <= 0:
                PINS.set(C.ENABLE_PIN, 1)
                raise RuntimeError("the %s axis's home switch never changes" % self.name)
            PINS.set(self.step, 1)
            PINS.pause(2)
            PINS.set(self.step, 0)
            PINS.pause(C.STEP_SLOW_US)
            return left - 1
        if PINS.home(pin) and (HOME or not SIM):
            PINS.set(self.dir, not self.flip)       # away from home
            left, more = int(most * self.per_unit), int(clear * self.per_unit)
            while more > 0:
                if not PINS.home(pin):
                    more -= 1
                left = step(left)
        PINS.set(self.dir, self.flip)       # home is the negative way
        left = int(most * self.per_unit)
        while not PINS.home(pin):
            left = step(left)
        self.at = 0


class Screen:
    """Up to six short lines: on the display if there is one, and over USB."""

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


class Table:
    """passes.csv: '#safe,<mm>' then rows of n,kind,tool,depth,height,turn,via,label.
    via is 'd' for a move that may go straight to its row, 's' for one that must first draw the bit back to the safe
    depth, 'h' for one that must draw it right back.
    Read a row at a time, every time it is gone through: a Pico has not the memory for five hundred rows at once."""

    def __init__(self, path):
        self.path = path
        self.safe = 0.0
        self.total = 0
        self.follows = bytearray()  # for each row, whether it is a touch come to straight from the touch before it
        self.sets = []              # the rows that set a bit
        for row in self:
            self.total += 1
            self.follows.append(1 if row[1] in PROBES and row[6] == "d" else 0)
            if row[1] == "set":
                self.sets.append(row[0])

    def __iter__(self):
        with open(self.path) as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                if line.startswith("#"):
                    parts = line[1:].split(",")
                    if parts[0] == "safe":
                        self.safe = float(parts[1])
                    continue
                p = line.split(",")
                if p[0] == "n":
                    continue
                yield (int(p[0]), p[1], p[2], float(p[3]), float(p[4]), float(p[5]), p[6], p[7] if len(p) > 7 else "")


class Sled:
    def __init__(self, safe):
        self.depth = Axis("depth", C.DEPTH_PINS, C.DEPTH_STEPS_PER_MM, C.DEPTH_FLIP, C.DEPTH_TRIM_MM, C.PAST_MM)
        self.height = Axis("height", C.HEIGHT_PINS, C.HEIGHT_STEPS_PER_MM, C.HEIGHT_FLIP, C.HEIGHT_TRIM_MM, C.PAST_MM)
        self.turn = Axis("turn", C.TURN_PINS, C.TURN_STEPS_PER_DEG, C.TURN_FLIP, C.TURN_TRIM_DEG)
        self.safe = safe            # the depth at or short of which the bit clears the master whatever the height
        self.bit = 0.0              # how much further out than the table takes it to be the bit in the collet has to be run: found by touch
        self.bits = []              # every setting: row, and what was found
        self.deepest_crossing = 0   # the furthest out the bit ever was while the height changed or the spindle turned, on a move by the safe depth
        self.went_straight = 0
        self.turned = 0             # steps the spindle has turned forwards coming up to its angles
        self.probed = []            # what a probing table found: row, kind, depth, height, turn, how far apart its touches
        self.row = 0                # the row being done
        self.touch = 0              # and, probing, which of its touches
        self.locked = None          # the spindle's clamp, as last seen: on a computer, as last asked for
        self.unlocks = 0            # how many times it has been let go
        self.rounds = 0             # bands turned by hand
        for n in (C.DEPTH_HOME_PIN, C.HEIGHT_HOME_PIN, C.TURN_HOME_PIN, C.SLED_BACK_PIN, C.LOCK_PIN, C.NEXT_PIN, C.PROBE_PIN):
            PINS.input(n)
        PINS.output(C.ENABLE_PIN)
        PINS.output(C.LED_PIN)
        PINS.set(C.ENABLE_PIN, 0)

    def back(self):
        return PINS.closed(C.SLED_BACK_PIN)

    def is_locked(self):
        """What the clamp's switch says. On a computer, what was last asked for; None before anything was."""
        return self.locked if SIM else PINS.closed(C.LOCK_PIN)

    def unlock(self, head=""):
        """The spindle let go, with the sled back: the star is then in the index wheel, and only there can the motor
        turn the stick."""
        if self.is_locked() is not False:
            SCREEN.show(head, "sled back, then", "UNLOCK the", "spindle")
            while not SIM and (PINS.closed(C.LOCK_PIN) or not self.back()):
                utime.sleep_ms(20)
            self.unlocks += 1
        self.locked = False

    def lock(self, head=""):
        """The spindle clamped where the wheel has put it. Nothing goes on until it is: the sled takes the stick away
        from the wheel, and the clamp is then all that holds its angle."""
        if self.is_locked() is not True:
            SCREEN.show(head, "LOCK the", "spindle")
            while not SIM and not PINS.closed(C.LOCK_PIN):
                utime.sleep_ms(20)
        self.locked = True

    def turns(self, degrees):
        """Whether going to this angle moves the wheel at all."""
        return (self.turn.steps_for(degrees) - self.turn.at) % int(round(360.0 * self.turn.per_unit)) != 0

    def home(self):
        SCREEN.show("sled back,", "then foot switch", "to find home")
        wait(C.SLED_BACK_PIN)
        wait(C.NEXT_PIN)
        SCREEN.show("finding home")
        self.depth.seek(C.DEPTH_HOME_PIN, C.DEPTH_MOST_MM, C.HOME_CLEAR_MM)     # the bit drawn back first: away from everything
        self.height.seek(C.HEIGHT_HOME_PIN, C.HEIGHT_MOST_MM, C.HOME_CLEAR_MM)
        self.unlock()               # the wheel takes the stick round with it to its mark
        SCREEN.show("finding home")
        self.turn.seek(C.TURN_HOME_PIN, 400.0, C.TURN_CLEAR_DEG)

    def turn_on_to(self, degrees, guard, slow=None):
        """The shorter way round, but always arriving forwards, so the belt is pulled from the same side:
        a move back goes a little past and comes up to the angle."""
        per_turn = int(round(360.0 * self.turn.per_unit))
        ahead = (self.turn.steps_for(degrees) - self.turn.at) % per_turn
        if ahead == 0:
            return
        self.unlock()
        if self.is_locked():
            raise RuntimeError("the spindle is locked")
        if ahead > per_turn // 2:
            past = int(round(C.TURN_PAST_DEG * self.turn.per_unit))
            self.turn.run(ahead - per_turn - past, guard, slow)
            ahead = past
        self.turned += abs(ahead)
        self.turn.run(ahead, guard, slow)

    def go(self, depth, height, turn, via="s"):
        """To a row, the sled back: up or down and round, then out to its depth. Unless the table says this move may
        go straight ('d'), the bit is first drawn back to the safe depth if it is further out: with the sled back
        it is beside the master. 'h' draws it right back."""
        guard = self.back
        if via == "d":
            self.went_straight += 1
        elif via == "h":
            self.depth.run(-self.depth.at, guard)
        else:
            safe_steps = self.depth.steps_for(self.safe + self.bit)
            if self.depth.at > safe_steps:
                self.depth.run(safe_steps - self.depth.at, guard)
            self.deepest_crossing = max(self.deepest_crossing, self.depth.at)
        self.height.to(height, guard)
        self.turn_on_to(turn, guard)
        self.lock()
        self.depth.to(depth, guard)

    def probe(self, axis, expected, way, window):
        """One screw brought slowly on, from a window short of where the touch is expected to a window past it, until
        the contact closes; then backed off a little and brought on again, for PROBE_TOUCHES touches.
        Where it stood at them, in the table's units: the mean, and how far apart they were. None if it never closed."""
        start = max(0.0, expected - way * window)
        axis.run(axis.steps_for(start) - axis.at, self.back)
        if SIM and CONTACT is None:
            return expected, 0.0            # nothing to touch on a computer, unless something stands in for it
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
        axis.run(axis.steps_for(start) - axis.at, self.back)
        if len(found) < C.PROBE_TOUCHES:
            return None
        return sum(found) / len(found) / axis.per_unit, (max(found) - min(found)) / axis.per_unit

    def touch_at(self, kind, depth, height, turn, via):
        """A probing row: to its start by the way the table says, then the touch. (depth, height, apart), or None."""
        if kind == "in":
            self.go(max(0.0, depth - C.PROBE_DEPTH_MM), height, turn, via)
            at = self.probe(self.depth, depth, 1.0, C.PROBE_DEPTH_MM)
            return None if at is None else (at[0], self.height.at / self.height.per_unit, at[1])
        way = 1.0 if kind == "up" else -1.0
        self.go(depth, max(0.0, height - way * C.PROBE_HEIGHT_MM), turn, via)
        at = self.probe(self.height, height, way, C.PROBE_HEIGHT_MM)
        return None if at is None else (self.depth.at / self.depth.per_unit, at[0], at[1])

    def run(self, rows, record=None, after=None):
        """The whole table, or what is left of it after row `after`."""
        total = rows.total
        done = place(total) if after is None else after
        # A touch that follows straight on from the one before it cannot be come to from home: go back to where
        # that run of touches began, and make them again.
        while 0 < done < total and rows.follows[done]:
            done -= 1
        if done and after is None:
            SCREEN.show("left at row %d" % done, "of %d" % total, "press: go on", "hold 3 s: restart")
            if wait(C.NEXT_PIN) >= 3000:
                done = 0
        # Taken up part way, the bit has still to be set: homing has forgotten where its tip is. The last setting row
        # before the place is done again.
        again = max([n for n in rows.sets if n <= done] or [0])
        self.home()
        from_home = True                    # the first move is from home, whatever the table took it to be from
        for n, kind, tool, depth, height, turn, via, label in rows:
            if n <= done and n != again:
                continue
            self.row = n
            head = "%d/%d %s" % (n, total, kind)
            if kind in ("measure", "say"):  # nothing moves: something to do or to read off, then the foot switch
                SCREEN.show(head, label[:16], label[16:32], label[32:48], label[48:64], "then foot switch")
                wait(C.NEXT_PIN)
            elif kind == "leads":           # the probe's clip on: the thing it is on bridged to the master must read, and not once the bridge is gone
                SCREEN.show(head, "bridge master", "and %s with" % label[:12], "something metal")
                while not SIM and not PINS.touching():
                    utime.sleep_ms(20)
                SCREEN.show(head, "good. take it", "away, then foot", "switch")
                wait(C.NEXT_PIN)
                if not SIM and PINS.touching():
                    raise RuntimeError("still touching")
            elif kind == "round":           # a band turned past the cutter by hand, the sled held at a stop: away from its back
                # stop the star is out of the wheel, and the motor cannot reach the stick. One whole turn of the lock
                # disc, back to its mark, so that the star goes home in the wheel as it left it.
                SCREEN.show(head, label[:16], label[16:32], "UNLOCK. disc one", "turn to its mark", "LOCK. then foot")
                while not SIM and PINS.closed(C.LOCK_PIN):
                    utime.sleep_ms(20)
                while not SIM and not PINS.closed(C.LOCK_PIN):
                    utime.sleep_ms(20)
                wait(C.NEXT_PIN)
                self.rounds += 1
            elif kind == "set":             # a bit set by touch: run out at the master until the clip on it reads
                SCREEN.show(head, tool, "router OFF. clip", "on the bit. sled", "back, then foot")
                wait(C.SLED_BACK_PIN)
                wait(C.NEXT_PIN)
                if not SIM and PINS.touching():
                    raise RuntimeError("the clip reads before the bit has moved")
                SCREEN.show(head, "setting the bit")
                self.bit = 0.0
                found = self.touch_at("in", depth, height, turn, "h" if from_home else "s")
                from_home = False
                if found is None:
                    raise RuntimeError("the bit never touched: is the clip on it?")
                if found[2] > C.PROBE_AGREE_MM:
                    raise RuntimeError("row %d: touches %d um apart" % (n, int(1000 * found[2])))
                self.bit = found[0] - depth
                self.bits.append([n, self.bit])
                SCREEN.show(head, "bit %+.2f" % self.bit, "PARK THE CLIP on", "its post, then", "foot switch")
                while not SIM and not PINS.touching():
                    utime.sleep_ms(20)
                wait(C.NEXT_PIN)
            elif kind in PROBES:
                SCREEN.show(head, label[:16], "sled back,", "then foot switch")
                wait(C.SLED_BACK_PIN)
                if from_home:
                    wait(C.NEXT_PIN)        # the first probe waits to be told; the rest follow on by themselves
                SCREEN.show(head, "probing")
                found = self.touch_at(kind, depth, height, turn, "h" if from_home else via)
                from_home = False
                if found is None:
                    raise RuntimeError("no touch at row %d: check the clip" % n)
                if found[2] > C.PROBE_AGREE_MM:
                    raise RuntimeError("row %d: touches %d um apart" % (n, int(1000 * found[2])))
                self.probed.append([n, kind, found[0], found[1], turn, found[2]])
                if not SIM:
                    with open(C.PROBE_FILE, "a") as f:
                        f.write("%d,%s,%.5f,%.5f,%.4f,%.5f\n" % (n, kind, found[0], found[1], turn, found[2]))
                if not SCREEN.quiet:
                    print("probe,%d,%s,%.5f,%.5f,%.4f,%.5f" % (n, kind, found[0], found[1], turn, found[2]))
            else:
                if n == again and n <= done:
                    continue                # only a setting is done again
                while not SIM and kind in ("rough", "finish") and not PINS.touching():
                    SCREEN.show(head, "PARK THE CLIP on", "its post")
                    utime.sleep_ms(200)
                # A row at a new angle is begun by letting the clamp go, which no one does by accident; a row at the
                # angle the stick is already at, by the foot switch.
                turning = self.turns(turn) and self.is_locked()
                SCREEN.show(head, tool, "sled back, then", "UNLOCK spindle" if turning else "foot switch")
                wait(C.SLED_BACK_PIN)
                if turning:
                    self.unlock(head)
                else:
                    wait(C.NEXT_PIN)
                SCREEN.show(head, "moving")
                self.go(depth + self.bit, height, turn, "s" if from_home and via == "d" else via)
                from_home = False
                PINS.set(C.LED_PIN, 1)
                if kind == "place":         # brought to a place for turning bands: the rows after it say what to do
                    pass
                elif kind == "load":        # the spindle turned to where the blank goes on with a wide face level and down
                    SCREEN.show(head, label[:16], label[16:32], label[32:48], label[48:64], "then foot switch")
                    wait(C.NEXT_PIN)
                else:
                    SCREEN.show(head, label[:16], "push past the", "bit and back")
                    wait_for_the_cut()
                PINS.set(C.LED_PIN, 0)
            if n > done:
                keep_place(n, total)
            if record is not None and n > done:
                record.append([n, self.depth.at, self.height.at, self.turn.at % int(round(360.0 * self.turn.per_unit))])
        keep_place(0, total)
        SCREEN.show("done")


def main(path="passes.csv", out=None, after=None):
    rows = Table(path)
    sled = Sled(rows.safe)
    record = [] if out else None
    try:
        sled.run(rows, record, after)
    except RuntimeError as why:
        SCREEN.show("stopped:", str(why)[:16], str(why)[16:32])
        raise
    if out:
        import json
        with open(out, "w") as f:
            json.dump({"rows": record, "after": after or 0, "deepest_crossing": sled.deepest_crossing, "turned": sled.turned, "went_straight": sled.went_straight, "unlocks": sled.unlocks, "rounds": sled.rounds,
                       "probed": sled.probed, "bits": sled.bits, "stepped": [sled.depth.stepped, sled.height.stepped, sled.turn.stepped],
                       "safe_steps": sled.depth.steps_for(rows.safe), "per": [C.DEPTH_STEPS_PER_MM, C.HEIGHT_STEPS_PER_MM, C.TURN_STEPS_PER_DEG]}, f)


if SIM:
    import sys
    if len(sys.argv) >= 4 and sys.argv[1] == "--sim":
        SCREEN.quiet = True
        main(sys.argv[2], sys.argv[3], int(sys.argv[4]) if len(sys.argv) > 4 else None)
else:
    main()
