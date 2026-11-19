"""The stick sled's controller: MicroPython on a Raspberry Pi Pico.

It reads passes.csv, a row a cut, each row where three motors stand
and how far the fourth goes: the positioner's quill run out so many
millimetres from its home (the depth), its stage so many from its (the
height), the spindle's degrees on from its mark, and how far along the
sled is fed. For a pass the positioner and the spindle go to the row
with the sled drawn back, where the bit is beside the stick's end and
not in it; then the sled's own screw feeds the stick past the bit and
brings it back. Nobody pushes anything. The positioner and the spindle
do not move while the stick is beside the bit.

Coming back, the sled runs fast to a little short of home and then
finds its switch, every pass. If that is not where its steps said, it
stalled in a cut, and the controller stops there.

The stick's motor is on the sled, and its wheel is on the sled's own
spindle: the belt holds the stick while it is cut. A lock on the
spindle is an option (config.LOCK), for a machine whose belt turns out
not to hold: with it the controller will not turn the stick until the
lock is let go, nor cut until it is thrown.

The foot switch starts each stage: homing, a bit set, the router on.
Held down while a pass is made, it stops the table after that pass
until it is pressed again.

A bit is set by touch. The probe's clip goes on the bit, the router
off; the controller runs the bit out until it touches the master on
the spindle, and from then on every depth is taken from where that
was. It will not go to a cut until the clip is parked on its post.

It says what it wants on a small display if one is fitted (an SSD1306,
128 x 64, with MicroPython's ssd1306 driver on the Pico), and always
over USB. It keeps its place in at.txt, so a stick stopped half way
can be taken up again after homing and setting the bit.

The same program runs the three tables that try a built machine and
set it up, copied to the Pico as passes.csv in their turn:
precheck.csv, which runs a sample of the table's own passes with no
bit in the collet and then has a weight hung on the disc, looking for
the wheel's mark before and after and writing what it finds to
check_log.csv; probe.csv, which
touches a pin in the collet on the master from in front, above and
below, and writes where the screws stood to probe_log.csv; and
bands.csv, which turns round bands on a scrap stick to show whether
the stick runs true to the sled's fence. calibrate.py reads both.

Copy main.py, config.py and passes.csv to the Pico. On a computer,
`python3 main.py --sim passes.csv steps.json [row]` runs the same code
on pins that do nothing and writes where every row left the motors;
with a row, as if taken up again after that row. `--lock` anywhere
after the script's name runs it as if a lock were fitted.
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
        self.clock = 0              # on a computer: the pauses asked for, added up, microseconds

    def output(self, n):
        if not SIM:
            self.out[n] = machine.Pin(n, machine.Pin.OUT)

    def input(self, n):
        if not SIM:
            self.inp[n] = machine.Pin(n, machine.Pin.IN, machine.Pin.PULL_UP)

    def set(self, n, v):
        if not SIM:
            self.out[n].value(1 if v else 0)
        elif STEPS:
            STEPS(n, v)

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
        else:
            self.clock += us


PINS = Pins()
CONTACT = None                      # on a computer: something to stand in for the probe's contact
HOME = None                         # and for the home switches
STEPS = None                        # and something to be told of every pin set, to follow a motor step by step


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

    def run(self, steps, guard=None, slow=None, fast=None):
        """Step by `steps`, ramping up to a pause of `fast` between steps (the config's fastest, unless given) and
        down again; or at a steady `slow`. `guard` is asked every step; while it says no, wait."""
        if steps == 0:
            return
        up = steps > 0
        n = abs(steps)
        top = C.STEP_FAST_US if fast is None else fast

        def pause(edge):
            if slow is not None:
                return slow
            if edge >= C.RAMP_STEPS or top >= C.STEP_SLOW_US:
                return top
            return C.STEP_SLOW_US - (C.STEP_SLOW_US - top) * edge // C.RAMP_STEPS
        if SIM and STEPS is None:   # on a computer nothing is listening: the same steps and the same pauses, counted and not taken
            ramp = min(C.RAMP_STEPS, (n + 1) // 2)
            PINS.clock += 2 * n + sum(pause(e) * (2 if 2 * e + 1 < n else 1) for e in range(ramp)) + pause(C.RAMP_STEPS) * max(0, n - 2 * ramp)
        else:
            PINS.set(self.dir, up != self.flip)
            for i in range(n):
                while guard is not None and not guard():
                    PINS.pause(20000)
                PINS.set(self.step, 1)
                PINS.pause(2)
                PINS.set(self.step, 0)
                PINS.pause(pause(min(i, n - 1 - i)))
        self.at += steps
        self.stepped += n

    def to(self, units, guard=None, slow=None, fast=None):
        """To a place, arriving going away from home: a move back goes a little past and comes up to it."""
        steps = self.steps_for(units) - self.at
        if steps < 0 and self.past and self.at + steps - self.past >= 0:
            self.run(steps - self.past, guard, slow, fast)
            steps = self.past
        self.run(steps, guard, slow, fast)

    def find(self, pin, most):
        """Run toward home until its switch says so, and call that zero. How many steps it took. No further than
        `most` of its units: a switch that never speaks is a fault."""
        PINS.set(self.dir, self.flip)       # home is the negative way
        left, taken = int(most * self.per_unit), 0
        while not PINS.home(pin):
            if left <= 0:
                PINS.set(C.ENABLE_PIN, 1)
                raise RuntimeError("the %s axis's home switch never changes" % self.name)
            PINS.set(self.step, 1)
            PINS.pause(2)
            PINS.set(self.step, 0)
            PINS.pause(C.STEP_SLOW_US)
            left, taken = left - 1, taken + 1
        self.stepped += taken
        self.at = 0
        return taken

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
        self.find(pin, most)


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


FEED_US = int(round(1e6 / (C.FEED_MM_S * C.TRAVEL_STEPS_PER_MM)))        # the pause between the sled's steps, cutting
RETURN_US = int(round(1e6 / (C.RETURN_MM_S * C.TRAVEL_STEPS_PER_MM)))    # and coming back


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
    """passes.csv: '#safe,<mm>' then rows of n,kind,tool,depth,height,turn,via,along,label.
    via is 'd' for a move that may go straight to its row, 's' for one that must first draw the bit back to the safe
    depth, 'h' for one that must draw it right back. along is how far the sled is fed for the row, millimetres from
    where it is drawn back: the length of a pass, or where a band is turned.
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
                yield (int(p[0]), p[1], p[2], float(p[3]), float(p[4]), float(p[5]), p[6], float(p[7]), p[8] if len(p) > 8 else "")


class Sled:
    def __init__(self, safe):
        self.depth = Axis("depth", C.DEPTH_PINS, C.DEPTH_STEPS_PER_MM, C.DEPTH_FLIP, C.DEPTH_TRIM_MM, C.PAST_MM)
        self.height = Axis("height", C.HEIGHT_PINS, C.HEIGHT_STEPS_PER_MM, C.HEIGHT_FLIP, C.HEIGHT_TRIM_MM, C.PAST_MM)
        self.turn = Axis("turn", C.TURN_PINS, C.TURN_STEPS_PER_DEG, C.TURN_FLIP, C.TURN_TRIM_DEG)
        self.travel = Axis("travel", C.TRAVEL_PINS, C.TRAVEL_STEPS_PER_MM, C.TRAVEL_FLIP, 0.0)
        self.safe = safe            # the depth at or short of which the bit clears the master whatever the height
        self.bit = 0.0              # how much further out than the table takes it to be the bit in the collet has to be run: found by touch
        self.bits = []              # every setting: row, and what was found
        self.deepest_crossing = 0   # the furthest out the bit ever was while the height changed or the spindle turned, on a move by the safe depth
        self.went_straight = 0
        self.turned = 0             # steps the spindle has turned forwards coming up to its angles
        self.probed = []            # what a probing table found: row, kind, depth, height, turn, how far apart its touches
        self.row = 0                # the row being done
        self.touch = 0              # and, probing, which of its touches
        self.locked = None          # the spindle's lock, if there is one, as last seen: on a computer, as last asked for
        self.unlocks = 0            # how many times it has been let go
        self.rounds = 0             # bands turned
        self.passes = 0             # passes made
        self.checks = []            # every look for the wheel's mark: row, kind, how far it was from where the steps said, and how far apart the looks were
        self.fed = 0                # steps the sled has been fed forward in them
        self.slip = 0.0             # the most the sled has come home from where its steps said, mm
        for n in (C.DEPTH_HOME_PIN, C.HEIGHT_HOME_PIN, C.TURN_HOME_PIN, C.TRAVEL_HOME_PIN, C.LOCK_PIN, C.NEXT_PIN, C.PROBE_PIN):
            PINS.input(n)
        for n in (C.ENABLE_PIN, C.LED_PIN, C.LOCK_OUT_PIN):
            PINS.output(n)
        PINS.set(C.ENABLE_PIN, 0)

    def back(self):
        """The sled drawn back: the bit beside the master and clear of the stick. The controller put it there and
        found its switch, or it is not there."""
        return self.travel.at == 0

    def is_locked(self):
        """What the lock's switch says. On a computer, what was last asked for; None before anything was. With no
        lock fitted the spindle is never locked."""
        if not C.LOCK:
            return False
        return self.locked if SIM else PINS.closed(C.LOCK_PIN)

    def unlock(self, head=""):
        """The spindle let go, if it has a lock: only then may the motor turn the stick."""
        if not C.LOCK:
            return
        PINS.set(C.LOCK_OUT_PIN, 1)
        if self.is_locked() is not False:
            SCREEN.show(head, "UNLOCK the", "spindle")
            while not SIM and PINS.closed(C.LOCK_PIN):
                utime.sleep_ms(20)
            self.unlocks += 1
        self.locked = False

    def lock(self, head=""):
        """The spindle locked where the wheel has put it, if it has a lock. Nothing is cut until it is."""
        if not C.LOCK:
            return
        PINS.set(C.LOCK_OUT_PIN, 0)
        if self.is_locked() is not True:
            SCREEN.show(head, "LOCK the", "spindle")
            while not SIM and not PINS.closed(C.LOCK_PIN):
                utime.sleep_ms(20)
        self.locked = True

    def turns(self, degrees):
        """Whether going to this angle moves the wheel at all."""
        return (self.turn.steps_for(degrees) - self.turn.at) % int(round(360.0 * self.turn.per_unit)) != 0

    def home(self):
        SCREEN.show("router OFF.", "foot switch", "to find home")
        wait(C.NEXT_PIN)
        SCREEN.show("finding home")
        self.depth.seek(C.DEPTH_HOME_PIN, C.DEPTH_MOST_MM, C.HOME_CLEAR_MM)     # the bit drawn back first, out of whatever it was left in
        self.travel.seek(C.TRAVEL_HOME_PIN, C.TRAVEL_MOST_MM, C.HOME_CLEAR_MM)  # then the sled: with the bit drawn back the stick passes it clear
        self.height.seek(C.HEIGHT_HOME_PIN, C.HEIGHT_MOST_MM, C.HOME_CLEAR_MM)
        self.unlock()
        SCREEN.show("finding home")
        self.turn.seek(C.TURN_HOME_PIN, 400.0, C.TURN_CLEAR_DEG)

    def come_home(self):
        """The sled drawn back: fast to a little short of home, then slowly until its switch says so. How far that
        was from where its steps said is how many it lost on the way out and back. More than a few and the pass was
        not the table's: stop."""
        short = int(round(C.HOME_SHORT_MM * self.travel.per_unit))
        if self.travel.at > short:
            self.travel.run(short - self.travel.at, None, None, RETURN_US)
        if SIM and HOME is None:            # nothing to find on a computer, unless something stands in for the switch
            self.travel.run(-self.travel.at, None, C.STEP_SLOW_US)
            return
        expected = self.travel.at
        taken = self.travel.find(C.TRAVEL_HOME_PIN, 2 * C.HOME_SHORT_MM + expected / self.travel.per_unit)
        slip = abs(taken - expected) / self.travel.per_unit
        self.slip = max(self.slip, slip)
        if slip > C.HOME_SLIP_MM:
            raise RuntimeError("the sled came home %d.%d mm out" % (int(slip), int(10 * slip) % 10))

    def cut(self, along):
        """A pass: the stick fed past the bit, and brought back with the bit where it was."""
        steps = self.travel.steps_for(along)
        self.travel.run(steps - self.travel.at, None, None, FEED_US)
        self.come_home()
        self.passes += 1
        self.fed += steps

    def look(self):
        """Where the wheel's mark is, against where the steps say. The stick is turned to a little past the mark and
        brought back until the sensor says home, as homing does, but that is not taken as the new home: the steps'
        own count is kept. Degrees: more than nothing if the spindle is ahead of where the motor's steps put it."""
        clear = int(round(C.TURN_CLEAR_DEG * self.turn.per_unit))
        self.turn_on_to(C.TURN_CLEAR_DEG, self.back)
        if SIM and HOME is None:            # nothing to find on a computer, unless something stands in for the sensor
            self.turn.run(-clear, None, C.STEP_SLOW_US)
            return 0.0
        if PINS.home(C.TURN_HOME_PIN):
            raise RuntimeError("the wheel's mark is not where it was")
        taken = self.turn.find(C.TURN_HOME_PIN, 3 * C.TURN_CLEAR_DEG)
        self.turn.at = clear - taken        # where the steps say it is, not where the mark was found
        return (taken - clear) / self.turn.per_unit

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
        per_turn = int(round(360.0 * self.turn.per_unit))
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
        for n, kind, tool, depth, height, turn, via, along, label in rows:
            if n <= done and n != again:
                continue
            self.row = n
            head = "%d/%d %s" % (n, total, kind)
            if kind != "round" and not self.back():
                self.come_home()            # a band was being turned: nothing else is done with the sled away
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
            elif kind in ("check", "gauge"):    # the wheel's mark looked for again: has anything slipped between the motor and the spindle?
                SCREEN.show(head, label[:16], label[16:32], "looking for the", "wheel's mark")
                self.unlock(head)
                found = [self.look() for _ in range(C.CHECK_TIMES)]
                self.lock(head)
                slip, apart = sum(found) / len(found), max(found) - min(found)
                self.checks.append([n, kind, slip, apart])
                if not SIM:
                    with open(C.CHECK_FILE, "a") as f:
                        f.write("%d,%s,%.4f,%.4f\n" % (n, kind, slip, apart))
                if not SCREEN.quiet:
                    print("check,%d,%s,%.4f,%.4f" % (n, kind, slip, apart))
                SCREEN.show(head, label[:16], label[16:32], "mark %+.3f deg" % slip)
                if kind == "check" and abs(slip) > C.TURN_SLIP_DEG:
                    raise RuntimeError("the wheel's mark is %d.%02d deg off" % (int(abs(slip)), int(100 * abs(slip)) % 100))
            elif kind == "round":           # a band: the sled fed to where it goes, the bit where the row before left it, and the stick turned once round
                SCREEN.show(head, label[:16], label[16:32], "turning a band")
                self.travel.to(along, None, None, FEED_US)
                self.unlock(head)
                if self.is_locked():
                    raise RuntimeError("the spindle is locked")
                self.turn.run(per_turn, None, C.BAND_US)
                self.lock(head)
                self.rounds += 1
            elif kind == "set":             # a bit set by touch: run out at the master until the clip on it reads
                SCREEN.show(head, tool, "router OFF. bit", "in, clip on it,", "then foot switch")
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
                SCREEN.show(head, "ROUTER ON.", "stand clear,", "then foot switch", "it cuts by", "itself from here")
                wait(C.NEXT_PIN)
            elif kind in PROBES:
                SCREEN.show(head, label[:16], "foot switch")
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
                if not SIM and PINS.closed(C.NEXT_PIN):     # the foot switch held down through the last pass: stop here
                    SCREEN.show(head, "PAUSED", "foot switch", "to go on")
                    wait(C.NEXT_PIN)
                SCREEN.show(head, tool, "moving")
                self.go(depth + self.bit, height, turn, "s" if from_home and via == "d" else via)
                from_home = False
                PINS.set(C.LED_PIN, 1)
                if kind == "place":         # brought to a place for turning bands: the rows after it say where
                    pass
                elif kind == "load":        # the spindle turned to where the blank goes on with a wide face level and down
                    SCREEN.show(head, label[:16], label[16:32], label[32:48], label[48:64], "then foot switch")
                    wait(C.NEXT_PIN)
                else:                       # a pass; "dry" is one made with no bit in the collet, to try the machine
                    SCREEN.show(head, tool, label[:16], "dry pass" if kind == "dry" else "cutting")
                    self.cut(along)
                PINS.set(C.LED_PIN, 0)
            if n > done:
                keep_place(n, total)
            if record is not None and n > done:
                record.append([n, self.depth.at, self.height.at, self.turn.at % per_turn, self.travel.at])
        if not self.back():
            self.come_home()
        keep_place(0, total)
        SCREEN.show("done.", "ROUTER OFF")


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
                       "lock": C.LOCK, "probed": sled.probed, "bits": sled.bits, "checks": sled.checks, "stepped": [sled.depth.stepped, sled.height.stepped, sled.turn.stepped, sled.travel.stepped],
                       "passes": sled.passes, "fed": sled.fed, "seconds": PINS.clock / 1e6,
                       "safe_steps": sled.depth.steps_for(rows.safe), "per": [C.DEPTH_STEPS_PER_MM, C.HEIGHT_STEPS_PER_MM, C.TURN_STEPS_PER_DEG, C.TRAVEL_STEPS_PER_MM]}, f)


if SIM:
    import sys
    if "--lock" in sys.argv:
        C.LOCK = True
        sys.argv.remove("--lock")
    if len(sys.argv) >= 4 and sys.argv[1] == "--sim":
        SCREEN.quiet = True
        main(sys.argv[2], sys.argv[3], int(sys.argv[4]) if len(sys.argv) > 4 else None)
else:
    main()
