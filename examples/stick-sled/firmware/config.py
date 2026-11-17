# The stick sled's wiring and its numbers. Edit this file; main.py reads it.
# Pins are the Pico's GP numbers.

# Each motor's driver: step pin, direction pin, and which way is positive.
DEPTH_PINS = (2, 3)             # the positioner's X: the quill, along the bit. Positive runs the bit out toward the stick
HEIGHT_PINS = (4, 5)            # the positioner's Z: the stage. Positive is away from its home, which in this machine is up
TURN_PINS = (6, 7)              # the spindle's motor
ENABLE_PIN = 8                  # all three drivers' enable, low to run
DEPTH_FLIP = False              # True if the motor runs the wrong way
HEIGHT_FLIP = False
TURN_FLIP = False

# Switches, each from its pin to ground: closed reads 0.
# The three home switches are wired normally closed: they open at home, so a wire that comes off
# reads as home and the motor stops instead of driving into its end. Set False for switches that close at home.
HOME_OPENS = True
DEPTH_HOME_PIN = 10             # the quill drawn right back
HEIGHT_HOME_PIN = 11            # the stage at its home end
TURN_HOME_PIN = 12              # once a turn of the spindle, at its mark
DEPTH_MOST_MM = 70.0            # homing gives up if the switch has not spoken in this much travel
HEIGHT_MOST_MM = 90.0
# An axis found already at home is first run away until its switch lets go and this much further, then brought
# back: home is the same edge from the same side every time. The spindle's fin is 10 degrees wide.
HOME_CLEAR_MM = 1.0
TURN_CLEAR_DEG = 5.0
SLED_BACK_PIN = 13              # closed with the sled against its back stop, the bit beside the master and clear of the stick
NEXT_PIN = 14                   # the foot switch
LED_PIN = 25

# A display, if there is one: an SSD1306, 128 x 64, on I2C. Put MicroPython's ssd1306 driver on the Pico
# (mpremote mip install ssd1306). Without it everything is still said over USB.
OLED = True
OLED_I2C, OLED_SDA_PIN, OLED_SCL_PIN = 0, 16, 17

# The probe: a lead from this pin with a clip on its end, and the spindle wired to ground. Clipped on a bit (the
# router off), the bit touching the master closes the contact: that is how a bit is set, and how a built machine is
# measured, with a pin in the collet. Its home is a post wired to ground: parked there the contact is closed, and the
# controller will not go to a cut until it is, so the router is never started with the clip on its bit.
PROBE_PIN = 15
PROBE_DEPTH_MM = 8.0            # a probe starts this much of its screw short of where the touch is expected, and gives up
PROBE_HEIGHT_MM = 6.0           # this much past it: the bit run out at the master, or brought up or down against it
PROBE_US = 1200                 # the pause between steps coming up to a touch
PROBE_TOUCHES = 3               # each place is touched this many times, the screw backed off this far between,
PROBE_BACK_MM = 0.5
PROBE_AGREE_MM = 0.05           # and the touches must agree this well: a chip, or a clip that slips, stops the probing
PROBE_FILE = "probe_log.csv"
ROUND_US = 1500                 # turning a band past the cutter

# Where the controller keeps its place in a stick.
PLACE_FILE = "at.txt"

# Steps for a millimetre of screw and a degree of spindle.
# 200-step motors; the positioner's screws M8 rod, 1.25 mm a turn (a T8 of 2 mm lead: 200 * 8 / 2.0);
# the drivers at 8 microsteps for the screws and 16 for the spindle; a 20-tooth pulley driving a wheel of 160.
DEPTH_STEPS_PER_MM = 200 * 8 / 1.25
HEIGHT_STEPS_PER_MM = 200 * 8 / 1.25
TURN_STEPS_PER_DEG = 200 * 16 * (160 / 20) / 360.0

# A nudge to every row: positive moves it further from home. Leave these at nothing: where the stick's axis is
# from the positioner's home switches is found by touching (calibrate.py) and planned for in the table itself.
DEPTH_TRIM_MM = 0.0
HEIGHT_TRIM_MM = 0.0
TURN_TRIM_DEG = 0.0

# Every move ends going the same way, away from home: one that goes back goes this far past and comes up to its
# place. The spindle the same, in degrees.
PAST_MM = 0.5
TURN_PAST_DEG = 3.0

# Speeds: the pause between steps, microseconds, at the start of a move and at its fastest.
STEP_SLOW_US = 900
STEP_FAST_US = 140
RAMP_STEPS = 400
