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
# The built hinges are never quite the drawn ones, and most of what that does is stretch each screw's travel:
# a scale, about the middle of the travel, found from the same test stick. 1.0 until then.
LIFT_SCALE, LIFT_MID_MM = 1.0, 45.0
SWING_SCALE, SWING_MID_MM = 1.0, 20.0

# Turning back to an angle, the spindle goes this far past it and comes up to it forwards.
TURN_PAST_DEG = 3.0

# Speeds: the pause between steps, microseconds, at the start of a move and at its fastest.
STEP_SLOW_US = 900
STEP_FAST_US = 140
RAMP_STEPS = 400
