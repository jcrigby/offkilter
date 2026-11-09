# Printing a part

Parts are printed on a Bambu Lab P1S at the owner's house. A session
cannot reach the printer; it asks for a print by opening a GitHub issue
on this repo. A watcher on the print computer picks the issue up,
slices the STL with a fixed template, and starts the print through
Bambu Studio's networking plugin, reporting back as comments on the
issue.

## Asking for a print

1. Push the STL. The watcher fetches it from GitHub, so it must be in a
   pushed commit; a file only in the session's checkout cannot print.
2. **Ask the owner whether the plate is clear**, and wait for a yes.
   The last print, and the purge line along the plate's front edge, have
   to be taken off by hand. Never write `plate_clear: yes` without the
   owner saying so for this print.
3. Open an issue with the label `print` and a body of `key: value`
   lines, nothing else needed:

   ```
   file: examples/lint-blower/out/slot_gauge.stl
   ref: claude/some-branch
   copies: 1
   plate_clear: yes
   ```

   | key | | |
   |---|---|---|
   | `file` | required | path in this repo, an `.stl` |
   | `ref` | if not on `main` | branch, tag or commit the file is on |
   | `copies` | default 1 | 1 to 16, arranged on one plate |
   | `flip` | optional | `yes` turns the part upside down (180° about X) before slicing |
   | `plate_clear` | required | `yes`, from the owner, see above |
   | `dry_run` | optional | `yes` slices and stops; nothing is sent |
   | `template` | default `default` | slicer settings on the print computer; `default` is the only one |

   The title is free text; say what the part is. For `copies` above 1,
   send a `dry_run: yes` issue first: its comment reports the time and
   grams for the whole plate, so you can see whether the job fits the
   limits below before asking the owner to clear the plate.
4. The issue has to be opened by **jcrigby**. Issues by any other
   account are ignored without a comment. If the session cannot open
   issues as jcrigby (or at all), write the issue out for the owner to
   paste into the GitHub app, with the label.

## What happens

Labels track it: `print` (queued) → `printing` (taken) → `printed`
(the print **started**; the issue is closed) or `print-stuck` (it
stopped; the issue stays open).

Within a minute or two the watcher comments "Picked up" with the part's
size and the sliced time and grams. The sliced file then goes to the
printer by `bambu-send`, a small program on the print computer that
loads the networking plugin Bambu Studio itself uses, so the print
command is signed the way Studio's Print button signs it and the
printer accepts it in cloud mode; it refuses to send while the printer
reports a job running. Its result is the second comment. No comment
after five minutes means the print computer's watcher is not running,
or the issue's author or label is wrong.

`print-stuck` means nothing was printed, or the printer was found busy.
Read the comment, fix the cause (often a new STL pushed, or a key
changed in the issue body), then swap the label back to `print` to
retry. Ask the owner first if the plate may not be clear any more.

## Making a part that prints unattended

The template is fixed: PLA, 0.4 mm nozzle, 0.2 mm layers, Textured PEI
plate, 2 walls, 15% infill, **no supports**. So:

- **Export in print orientation.** The slicer only drops the part onto
  the bed at its lowest point; it does not turn it. Put the big flat
  face at the bottom (min Z). `slot_gauge.stl` was first exported with
  its teeth down and its spine in the air while the README said spine
  on the bed; Studio warned "floating cantilever" and the print
  stopped. `flip: yes` rescues a part that is exactly upside down;
  better to export it right.
- No overhangs steeper than about 45° from vertical and no long
  bridges; Studio's overhang or cantilever warnings stop the job.
- Fits 256 × 256 × 250 mm (it may be turned on the plate in X/Y).
  Bigger parts are split, as `examples/lint-blower` does.
- At most 8 hours and 500 g per issue, or the watcher refuses it.
- One filament, whatever is loaded; colour is not choosable.
- Several different parts on one plate: put them in one STL, apart
  from each other, laid out in print orientation.

## One plate at a time

The watcher takes the oldest open `print` issue each minute, so do not
queue a second print while one is running: it will find the printer
busy and go to `print-stuck`, and its `plate_clear: yes` was about a
plate that now holds the first print. Open the next issue after the
owner has taken the last print off. The `printed` comment gives the
estimated finish time.

## The print computer

The watcher and its tools are not in this repo; they live on the print
computer in `~/fun/autoprint` (`watch.py`, `slice.py`, `send/bambu-send`
with its README, `templates/`, and `printer.py`, which follows the
printer over Bambu's cloud and comments when a print finishes or
fails). Slicing is Bambu Studio's command line; sending is the plugin,
so nothing needs a screen and the computer just has to be on. The
earlier path, a headless Claude driving the Studio GUI on a virtual
display through `gui.sh` and `driver_prompt.md`, is kept as a fallback
(`AUTOPRINT_SENDER=gui`) in case a plugin update stops signing for a
host that is not Studio. Each job's log stays under `state/issue-N/`.
