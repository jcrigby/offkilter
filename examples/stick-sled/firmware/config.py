# The stick sled's wiring and its numbers. Edit this file; main.py reads it.
# Pins are the Pico's GP numbers.

# Each motor's driver: step pin, direction pin, and which way is positive.
LIFT_PINS = (2, 3)              # pusher 1, under the lift plate's free edge
SWING_PINS = (4, 5)             # pusher 2, against the hanging frame
TURN_PINS = (6, 7)              # the spindle's motor
ENABLE_PIN = 8                  # all three drivers' enable, low to run
LIFT_FLIP = False               # True if the motor runs the wrong way
SWING_FLIP = False
TURN_FLIP = False

# Switches, each from its pin to ground: closed reads 0.
# The three home switches are wired normally closed: they open at home, so a wire that comes off
# reads as home and the motor stops instead of driving into its end. Set False for switches that close at home.
HOME_OPENS = True
LIFT_HOME_PIN = 10              # pusher 1 at the top of its travel
SWING_HOME_PIN = 11             # pusher 2 drawn right back
TURN_HOME_PIN = 12              # once a turn of the spindle, at its mark
LIFT_MOST_MM = 110.0            # homing gives up if the switch has not spoken in this much travel
SWING_MOST_MM = 70.0
SLED_BACK_PIN = 13              # closed with the sled pulled right back, the stick clear of the cutter
NEXT_PIN = 14                   # the foot switch
LED_PIN = 25

# A display, if there is one: an SSD1306, 128 x 64, on I2C. Put MicroPython's ssd1306 driver on the Pico
# (mpremote mip install ssd1306). Without it everything is still said over USB.
OLED = True
OLED_I2C, OLED_SDA_PIN, OLED_SCL_PIN = 0, 16, 17

# Probing, to find out what a built sled is: the pin or block in the router's place is clipped to this pin,
# the spindle to ground; the master touching it closes the contact.
PROBE_PIN = 15
PROBE_LIFT_MM = 8.0             # a probe starts this much of its screw short of where the drawn sled would touch, and gives up
PROBE_SWING_MM = 6.0            # this much past it: the lift coming down on a thing, the swing coming against its side
PROBE_US = 1200                 # the pause between steps coming up to a touch
PROBE_TOUCHES = 3               # each place is touched this many times, the screw backed off this far between,
PROBE_BACK_MM = 0.5
PROBE_AGREE_MM = 0.08           # and the touches must agree this well: a chip, or a clip that slips, stops the probing
PROBE_FILE = "probe_log.csv"
ROUND_US = 1500                 # turning a band under the cutter

# Where the controller keeps its place in a stick.
PLACE_FILE = "at.txt"

# Steps for a millimetre of pusher and a degree of spindle.
# 200-step motors; a T8 screw of 2 mm lead; the drivers at 8 microsteps for the pushers
# and 16 for the spindle; a 16-tooth pulley driving a 60-tooth one.
LIFT_STEPS_PER_MM = 200 * 8 / 2.0
SWING_STEPS_PER_MM = 200 * 8 / 2.0
TURN_STEPS_PER_DEG = 200 * 16 * (60 / 16) / 360.0

# A nudge to every row: positive moves it further from home. Leave these at nothing. What a built sled is, its
# home switches' places with the rest, is found by probing (calibrate.py) and planned for in the table itself;
# these are for a switch that has been moved since, until the sled is probed again.
LIFT_TRIM_MM = 0.0
SWING_TRIM_MM = 0.0
TURN_TRIM_DEG = 0.0

# Turning back to an angle, the spindle goes this far past it and comes up to it forwards.
TURN_PAST_DEG = 3.0

# Speeds: the pause between steps, microseconds, at the start of a move and at its fastest.
STEP_SLOW_US = 900
STEP_FAST_US = 140
RAMP_STEPS = 400
