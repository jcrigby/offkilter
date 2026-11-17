"""The stick sled's controller: MicroPython on a Raspberry Pi Pico.

It reads passes.csv, a row a cut, each row where the three motors
stand: the lift pusher's millimetres down from its home switch, the
swing pusher's millimetres out from its, and the spindle's degrees on
from its mark. For a pass it waits until the sled is pulled back and
the foot switch pressed, moves, and waits again while the sled is
pushed and pulled by hand. Nothing moves while the stick is over the
cutter.

Copy main.py, config.py and passes.csv to the Pico. On a computer,
`python3 main.py --sim passes.csv steps.json` runs the same code on
pins that do nothing and writes where every row left the motors.
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

    def pause(self, us):
        if not SIM:
            utime.sleep_us(us)


PINS = Pins()


class Axis:
    def __init__(self, name, pins, per_unit, flip, trim, scale=1.0, mid=0.0):
        self.name, self.step, self.dir = name, pins[0], pins[1]
        self.per_unit, self.flip, self.trim = per_unit, flip, trim
        self.scale, self.mid = scale, mid
        self.at = 0                 # steps from home
        self.stepped = 0            # steps taken in all
        PINS.output(self.step)
        PINS.output(self.dir)

    def steps_for(self, units):
        """The table's number as steps: stretched about `mid` by the scale a test stick found, moved by the trim."""
        return int(round((self.mid + (units - self.mid) * self.scale + self.trim) * self.per_unit))

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


def say(text):
    print(text)


def wait(pin):
    """Until a switch closes, and for the foot switch until it has been let go first."""
    if SIM:
        return
    if pin == C.NEXT_PIN:
        while PINS.closed(pin):
            utime.sleep_ms(20)
    while not PINS.closed(pin):
        utime.sleep_ms(20)
    utime.sleep_ms(30)


def read_table(path):
    """passes.csv: '#safe,<mm>' then rows of n,kind,tool,lift,swing,turn,via,label.
    via is 'd' for a move that may go straight to its row, 's' for one that must go by the safe height."""
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
            rows.append((int(p[0]), p[1], p[2], float(p[3]), float(p[4]), float(p[5]), p[6] == "d", p[7] if len(p) > 7 else ""))
    return safe, rows


class Sled:
    def __init__(self, safe):
        self.lift = Axis("lift", C.LIFT_PINS, C.LIFT_STEPS_PER_MM, C.LIFT_FLIP, C.LIFT_TRIM_MM, C.LIFT_SCALE, C.LIFT_MID_MM)
        self.swing = Axis("swing", C.SWING_PINS, C.SWING_STEPS_PER_MM, C.SWING_FLIP, C.SWING_TRIM_MM, C.SWING_SCALE, C.SWING_MID_MM)
        self.turn = Axis("turn", C.TURN_PINS, C.TURN_STEPS_PER_DEG, C.TURN_FLIP, C.TURN_TRIM_DEG)
        self.safe = safe            # the lift at or above which the spindle clears the cutter wherever the frame swings
        self.deepest_crossing = 0   # the lowest the lift ever was while the frame swung or the spindle turned, on a move by the safe height
        self.went_straight = 0
        self.turned = 0             # steps the spindle has turned forwards coming up to its angles
        for n in (C.LIFT_HOME_PIN, C.SWING_HOME_PIN, C.TURN_HOME_PIN, C.SLED_BACK_PIN, C.NEXT_PIN):
            PINS.input(n)
        PINS.output(C.ENABLE_PIN)
        PINS.output(C.LED_PIN)
        PINS.set(C.ENABLE_PIN, 0)

    def back(self):
        return PINS.closed(C.SLED_BACK_PIN)

    def home(self):
        say("pull the sled back, then the foot switch: homing")
        wait(C.SLED_BACK_PIN)
        wait(C.NEXT_PIN)
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

    def go(self, lift, swing, turn, straight=False):
        """To a pass, the sled back: across and round, then to its height. Unless the table says this move
        may go straight, up to the safe height first if below it: the master is over the cutter."""
        guard = self.back
        if straight:
            self.went_straight += 1
        else:
            safe_steps = self.lift.steps_for(self.safe)
            if self.lift.at > safe_steps:
                self.lift.run(safe_steps - self.lift.at, guard)
            self.deepest_crossing = max(self.deepest_crossing, self.lift.at)
        self.swing.to(swing, guard)
        self.turn_on_to(turn, guard)
        self.lift.to(lift, guard)

    def run(self, rows, record=None):
        self.home()
        total = len(rows)
        for n, kind, tool, lift, swing, turn, straight, label in rows:
            say("%d/%d %s: sled back, then the foot switch" % (n, total, label))
            wait(C.SLED_BACK_PIN)
            wait(C.NEXT_PIN)
            self.go(lift, swing, turn, straight)
            PINS.set(C.LED_PIN, 1)
            if kind == "set":
                say("bring the %s bit up until it touches the master, then the foot switch" % tool)
                wait(C.NEXT_PIN)
            else:
                say("cut: push the sled through and pull it back")
            PINS.set(C.LED_PIN, 0)
            if record is not None:
                record.append([n, self.lift.at, self.swing.at, self.turn.at % int(round(360.0 * self.turn.per_unit))])
        say("done")


def main(path="passes.csv", out=None):
    safe, rows = read_table(path)
    sled = Sled(safe)
    record = [] if out else None
    sled.run(rows, record)
    if out:
        import json
        with open(out, "w") as f:
            json.dump({"rows": record, "deepest_crossing": sled.deepest_crossing, "turned": sled.turned, "went_straight": sled.went_straight, "stepped": [sled.lift.stepped, sled.swing.stepped, sled.turn.stepped], "safe_steps": sled.lift.steps_for(safe),
                       "per": [C.LIFT_STEPS_PER_MM, C.SWING_STEPS_PER_MM, C.TURN_STEPS_PER_DEG]}, f)


if SIM:
    import sys
    if len(sys.argv) >= 4 and sys.argv[1] == "--sim":
        _say = say
        say = lambda text: None
        main(sys.argv[2], sys.argv[3])
else:
    main()
