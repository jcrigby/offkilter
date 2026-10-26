#!/usr/bin/env python3
"""The turntable scanner, closed loop through real video files: the
frames rendered by the tests in `crates/ok-photo/tests/` (a lobed
gourd of known shape on the 2x2 field going round under a camera and
a line laser; a mushroom standing and on its side; a bucky ball) are
encoded with ffmpeg, the videos are scanned through the MCP server,
the mushroom's two scans are merged, and the meshes are imported and
pictured. Needs `cargo build --release -p ok-mcp` and ffmpeg."""
import glob
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
OUT = os.path.join(HERE, "out")
sys.path.insert(0, os.path.dirname(HERE))
from okmcp import Mcp  # noqa: E402

# (frame directory, video name, scan directory under out/)
SCANS = [
    ("frames", "turntable.mp4", ""),
    ("frames-mushroom", "mushroom.mp4", "mushroom"),
    ("frames-mushroom-side", "mushroom-side.mp4", "mushroom-side"),
    ("frames-bucky", "bucky.mp4", "bucky"),
]


def encode(frames_dir, video):
    """A 3 second video of the turn, compressed as a phone would."""
    frames = sorted(glob.glob(os.path.join(frames_dir, "*.png")))
    if not frames:
        sys.exit(f"no frames in {frames_dir}: render them first with `cargo test -p ok-photo`")
    subprocess.run(
        ["ffmpeg", "-y", "-loglevel", "error", "-framerate", "12", "-i", os.path.join(frames_dir, "%04d.png"),
         "-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "18", video],
        check=True,
    )
    print(f"encoded {len(frames)} frames into {video}, {os.path.getsize(video) // 1024} KB")


def shrink(png, jpg):
    """The frame picture the scanner writes is a full-size PNG; the committed copy is a smaller JPEG."""
    subprocess.run(
        ["ffmpeg", "-y", "-loglevel", "error", "-i", png, "-vf", "scale=900:-1", "-q:v", "4", jpg],
        check=True,
    )


os.makedirs(OUT, exist_ok=True)
doc = os.path.join(OUT, "scan.okpart")
if os.path.exists(doc):
    os.remove(doc)
mcp = Mcp(doc)
mcp.call("create_document", {"name": "turntable_scan"})
for frames_dir, video, sub in SCANS:
    video = os.path.join(HERE, video)
    encode(os.path.join(HERE, frames_dir), video)
    out = os.path.join(OUT, sub) if sub else OUT
    print(mcp.call("scan_turntable", {"video": video, "out": out, "fps": 12}))
    shrink(os.path.join(out, "scan-frame.png"), os.path.join(out, "scan-frame.jpg"))

# The mushroom's two scans into one: the second was taken on its side.
merged = os.path.join(OUT, "merged")
print(mcp.call("merge_scans", {
    "scans": [{"dir": os.path.join(OUT, "mushroom")}, {"dir": os.path.join(OUT, "mushroom-side"), "placed": "on its side"}],
    "out": merged,
}))

# Pictures: the gourd, the mushroom from one scan and from two, the bucky ball.
for tab, (stl, name) in enumerate([
    (os.path.join(OUT, "scan.stl"), "scan-mesh"),
    (os.path.join(OUT, "mushroom", "scan.stl"), "mushroom-mesh"),
    (os.path.join(merged, "merged.stl"), "merged-mesh"),
    (os.path.join(OUT, "bucky", "scan.stl"), "bucky-mesh"),
], start=1):
    if tab > 1:
        mcp.call("apply", {"ops": [{"type": "add_part_studio", "name": name}]})
    print(mcp.call("import", {"tab": tab, "path": stl, "name": name}))
    for view, suffix in (("iso", ""), ("front", "-front")):
        mcp.call("screenshot", {"tab": tab, "view": view, "width": 1000, "height": 800, "path": os.path.join(OUT, f"{name}{suffix}.png")})
mcp.close()
