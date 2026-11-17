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
LIFT_HOME_PIN = 10              # closed with pusher 1 at the top of its travel
SWING_HOME_PIN = 11             # closed with pusher 2 drawn right back
TURN_HOME_PIN = 12              # closed once a turn of the spindle, at its mark
SLED_BACK_PIN = 13              # closed with the sled pulled right back, the stick clear of the cutter
NEXT_PIN = 14                   # the foot switch
LED_PIN = 25

# Steps for a millimetre of pusher and a degree of spindle.
# 200-step motors; a T8 screw of 2 mm lead; the drivers at 8 microsteps for the pushers
# and 16 for the spindle; a 16-tooth pulley driving a 60-tooth one.
LIFT_STEPS_PER_MM = 200 * 8 / 2.0
SWING_STEPS_PER_MM = 200 * 8 / 2.0
TURN_STEPS_PER_DEG = 200 * 16 * (60 / 16) / 360.0

# Where the home switches really are, against where the table of passes takes them to be.
# Found from a test stick: positive moves every pass further from home.
LIFT_TRIM_MM = 0.0
SWING_TRIM_MM = 0.0
TURN_TRIM_DEG = 0.0

# Turning back to an angle, the spindle goes this far past it and comes up to it forwards.
TURN_PAST_DEG = 3.0

# Speeds: the pause between steps, microseconds, at the start of a move and at its fastest.
STEP_SLOW_US = 900
STEP_FAST_US = 140
RAMP_STEPS = 400
PLUNGE_US = 2500                # lowering onto the cutter for a journal
JOURNAL_US = 1500               # turning a journal under the cutter
